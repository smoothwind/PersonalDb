//! PostgreSQL 驱动（tokio-postgres + deadpool 连接池）。
//!
//! 注意：`database` 参数在此驱动中解释为 **schema**（search_path），
//! PostgreSQL 的 database 在连接建立时即已确定。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use deadpool_postgres::{Manager, ManagerConfig, RecyclingMethod};
use futures_util::TryStreamExt;
use tokio_postgres::types::Type;
use tokio_postgres::{Client, Config as PgConfig, NoTls, Row};

use crate::db::templates;
use crate::error::{AppError, DbResult};

use super::dialect::{PostgresDialect, SqlDialect};
use super::driver::DatabaseDriver;
use super::objects::{CatalogQuery, DbObjectType, ObjectInfo};
use super::types::{
    CellValue, ColumnInfo, ColumnMeta, ConnectionConfig, IndexInfo, QueryResult, RoutineInfo,
    RoutineParam, SemanticType, ServerInfo, TableInfo,
};

pub struct PostgresDriver {
    pool: deadpool_postgres::Pool,
    /// 手动提交模式
    manual: AtomicBool,
    /// 手动模式下的专用连接（承载未提交事务）
    tx_conn: tokio::sync::Mutex<Option<PgTx>>,
}

/// 手动事务下的专用连接与事务状态
struct PgTx {
    conn: deadpool_postgres::Object,
    in_tx: bool,
}

impl PostgresDriver {
    pub fn new(cfg: &ConnectionConfig) -> DbResult<Self> {
        let mut pg = PgConfig::new();
        pg.host(&cfg.host)
            .port(cfg.effective_port())
            .user(&cfg.username)
            .password(&cfg.password);
        // 未指定库时默认连 postgres 库：tokio-postgres 会以用户名作为默认库名，
        // 若同名库不存在将直接导致连接失败（表现为无法查看数据库）
        let dbname = cfg
            .database
            .as_deref()
            .filter(|d| !d.is_empty())
            .unwrap_or("postgres");
        pg.dbname(dbname);
        if let Some(timeout) = cfg.params.get("connect_timeout").and_then(|v| v.parse::<u64>().ok()) {
            pg.connect_timeout(std::time::Duration::from_secs(timeout));
        }
        let mgr = Manager::from_config(
            pg,
            NoTls,
            ManagerConfig {
                recycling_method: RecyclingMethod::Fast,
            },
        );
        let pool = deadpool_postgres::Pool::builder(mgr).max_size(8).build()?;
        Ok(Self {
            pool,
            manual: AtomicBool::new(false),
            tx_conn: tokio::sync::Mutex::new(None),
        })
    }

    fn semantic_of(t: &Type) -> SemanticType {
        match *t {
            Type::BOOL => SemanticType::Bool,
            Type::INT2 | Type::INT4 | Type::INT8 | Type::FLOAT4 | Type::FLOAT8 | Type::OID => {
                SemanticType::Number
            }
            Type::NUMERIC => SemanticType::Decimal,
            Type::VARCHAR | Type::BPCHAR | Type::NAME | Type::CHAR | Type::UNKNOWN => {
                SemanticType::Text
            }
            // TEXT 为无长度限制长文本——与 MySQL TEXT→LongText 语义对齐（实锤：原与
            // VARCHAR 同归 Text，长文本列丢失 LongText 渲染路由）
            Type::TEXT => SemanticType::LongText,
            Type::JSON | Type::JSONB => SemanticType::Json,
            Type::BYTEA => SemanticType::Binary,
            Type::TIMESTAMP | Type::TIMESTAMPTZ => SemanticType::DateTime,
            Type::DATE => SemanticType::Date,
            Type::TIME => SemanticType::Time,
            Type::UUID => SemanticType::Uuid,
            _ => SemanticType::Unknown,
        }
    }

    fn convert_cell(row: &Row, i: usize, ty: &Type) -> CellValue {
        let semantic = Self::semantic_of(ty);
        let opt_cell: Result<Option<CellValue>, _> = match *ty {
            Type::BOOL => row.try_get::<_, Option<bool>>(i).map(|v| v.map(CellValue::Bool)),
            Type::INT2 => row.try_get::<_, Option<i16>>(i).map(|v| v.map(|x| CellValue::Int(x as i64))),
            Type::INT4 => row.try_get::<_, Option<i32>>(i).map(|v| v.map(|x| CellValue::Int(x as i64))),
            Type::INT8 => row.try_get::<_, Option<i64>>(i).map(|v| v.map(CellValue::Int)),
            Type::FLOAT4 => row
                .try_get::<_, Option<f32>>(i)
                .map(|v| v.map(|x| CellValue::Float(x as f64))),
            Type::FLOAT8 => row.try_get::<_, Option<f64>>(i).map(|v| v.map(CellValue::Float)),
            Type::OID => row
                .try_get::<_, Option<u32>>(i)
                .map(|v| v.map(|x| CellValue::UInt(x as u64))),
            Type::VARCHAR | Type::TEXT | Type::BPCHAR | Type::NAME | Type::CHAR | Type::UNKNOWN => {
                row.try_get::<_, Option<String>>(i).map(|v| v.map(CellValue::Text))
            }
            Type::TIMESTAMP => row
                .try_get::<_, Option<NaiveDateTime>>(i)
                .map(|v| v.map(|x| CellValue::Text(x.format("%Y-%m-%d %H:%M:%S%.f").to_string()))),
            Type::TIMESTAMPTZ => row
                .try_get::<_, Option<DateTime<Utc>>>(i)
                .map(|v| v.map(|x| CellValue::Text(x.format("%Y-%m-%d %H:%M:%S%.f %Z").to_string()))),
            Type::DATE => row
                .try_get::<_, Option<NaiveDate>>(i)
                .map(|v| v.map(|x| CellValue::Text(x.format("%Y-%m-%d").to_string()))),
            Type::TIME => row
                .try_get::<_, Option<NaiveTime>>(i)
                .map(|v| v.map(|x| CellValue::Text(x.format("%H:%M:%S%.f").to_string()))),
            Type::UUID => row
                .try_get::<_, Option<uuid::Uuid>>(i)
                .map(|v| v.map(|x| CellValue::Text(x.to_string()))),
            Type::JSON | Type::JSONB => row
                .try_get::<_, Option<serde_json::Value>>(i)
                .map(|v| v.map(|x| CellValue::Text(x.to_string()))),
            Type::BYTEA => row.try_get::<_, Option<Vec<u8>>>(i).map(|v| {
                v.map(|b| {
                    use base64::Engine;
                    CellValue::Binary(base64::engine::general_purpose::STANDARD.encode(b))
                })
            }),
            Type::NUMERIC => row
                .try_get::<_, Option<PgNumeric>>(i)
                .map(|v| v.map(|n| CellValue::Text(numeric_to_string(n.0).unwrap_or_default()))),
            _ if matches!(ty.kind(), tokio_postgres::types::Kind::Array(_)) => row
                .try_get::<_, Option<PgArrayText>>(i)
                .map(|v| v.map(|a| CellValue::Text(a.0))),
            _ => row.try_get::<_, Option<String>>(i).map(|v| v.map(CellValue::Text)),
        };
        let _ = semantic;
        match opt_cell {
            Ok(Some(c)) => c,
            Ok(None) => CellValue::Null,
            Err(_) => CellValue::Text("<无法显示的值>".into()),
        }
    }
}

/// NUMERIC 二进制格式解码（零依赖）：wire format = i16 ndigits / weight / sign /
/// dscale + base-10000 位组。tokio-postgres/gaussdb 默认无 NUMERIC FromSql（f32/f64
/// 均不支持），缺此解码 NUMERIC 列在结果集中显示为 `<无法显示的值>` 占位符。
/// 返回十进制文本（保精度，与前端按 semantic 格式化的约定一致）。
pub(crate) fn numeric_to_string(buf: &[u8]) -> Option<String> {
    if buf.len() < 8 {
        return None;
    }
    let ndigits = i16::from_be_bytes([buf[0], buf[1]]) as usize;
    let weight = i16::from_be_bytes([buf[2], buf[3]]) as i32;
    let sign = i16::from_be_bytes([buf[4], buf[5]]);
    let dscale = i16::from_be_bytes([buf[6], buf[7]]) as usize;
    if sign == -0x4000i16 {
        return Some("NaN".into());
    }
    let mut digits = Vec::with_capacity(ndigits);
    for i in 0..ndigits {
        let o = 8 + i * 2;
        if o + 2 > buf.len() {
            return None;
        }
        digits.push(i16::from_be_bytes([buf[o], buf[o + 1]]));
    }
    let mut s = String::new();
    if sign == 0x4000 {
        s.push('-');
    }
    if weight < 0 {
        s.push('0');
    } else {
        for i in 0..=(weight as usize) {
            let d = if i < digits.len() { digits[i] } else { 0 };
            if i == 0 {
                s.push_str(&d.to_string());
            } else {
                s.push_str(&format!("{d:04}"));
            }
        }
    }
    if dscale > 0 {
        s.push('.');
        let mut frac = String::new();
        let mut idx = weight + 1;
        while idx < digits.len() as i32 {
            frac.push_str(&format!("{:04}", digits[idx as usize]));
            idx += 1;
        }
        frac.truncate(dscale);
        while frac.len() < dscale {
            frac.push('0');
        }
        s.push_str(&frac);
    }
    Some(s)
}

/// NUMERIC 原始字节的 FromSql 包装（postgres-types 默认不支持 NUMERIC 读取）
struct PgNumeric<'a>(&'a [u8]);

impl<'a> tokio_postgres::types::FromSql<'a> for PgNumeric<'a> {
    fn from_sql(
        _: &tokio_postgres::types::Type,
        raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(PgNumeric(raw))
    }
    fn accepts(ty: &tokio_postgres::types::Type) -> bool {
        *ty == tokio_postgres::types::Type::NUMERIC
    }
}

/// PG 数组二进制解码（零依赖，1 维数组文本化）：wire format =
/// [ndim][flags][elem_oid] + [dim][lbound]×ndim + 元素 [len][data]。
/// tokio-postgres/gaussdb 默认无数组 FromSql——缺失时数组列显示为占位符。
/// 元素按 oid 渲染文本（int4/int8/float8/bool/text 系），其余标 `<bin:N>`。
pub(crate) fn array_to_string(buf: &[u8]) -> Option<String> {
    if buf.len() < 12 {
        return None;
    }
    let ndim = i32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let elem_oid = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
    if ndim != 1 {
        return None; // 多维数组暂按不支持处理
    }
    let n = i32::from_be_bytes([buf[12], buf[13], buf[14], buf[15]]) as usize;
    // 12 字节头 + dim(4) + lbound(4) = 元素区起点 20
    let mut off = 20usize;
    // (文本, 是否需要引号)——NULL 元素裸渲染，字符串 "NULL" 则加引号区分
    let mut parts: Vec<(String, bool)> = Vec::with_capacity(n);
    for _ in 0..n {
        if off + 4 > buf.len() {
            return None;
        }
        let len = i32::from_be_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]);
        off += 4;
        if len < 0 {
            parts.push(("NULL".to_string(), false));
            continue;
        }
        let len = len as usize;
        if off + len > buf.len() {
            return None;
        }
        let raw = &buf[off..off + len];
        off += len;
        let text = match elem_oid {
            23 => i32::from_be_bytes(raw.try_into().ok()?).to_string(),
            20 => i64::from_be_bytes(raw.try_into().ok()?).to_string(),
            701 => f64::from_be_bytes(raw.try_into().ok()?).to_string(),
            16 => (raw[0] != 0).to_string(),
            25 | 1043 | 18 | 1042 => String::from_utf8_lossy(raw).into_owned(),
            _ => format!("<bin:{len}>"),
        };
        parts.push((text, true));
    }
    let need_quote =
        |s: &str| s.is_empty() || s == "NULL" || s.chars().any(|c| "{}\",".contains(c) || c == '\\' || c.is_whitespace());
    let inner = parts
        .iter()
        .map(|(s, q)| if *q && need_quote(s) { format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")) } else { s.clone() })
        .collect::<Vec<_>>()
        .join(",");
    Some(format!("{{{inner}}}"))
}

/// 数组原始字节的 FromSql 包装（解码在 from_sql 内即时完成）
struct PgArrayText(String);

impl<'a> tokio_postgres::types::FromSql<'a> for PgArrayText {
    fn from_sql(
        _: &tokio_postgres::types::Type,
        raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(PgArrayText(array_to_string(raw).unwrap_or_default()))
    }
    fn accepts(ty: &tokio_postgres::types::Type) -> bool {
        matches!(ty.kind(), tokio_postgres::types::Kind::Array(_))
    }
}

#[async_trait]
impl DatabaseDriver for PostgresDriver {
    async fn server_info(&self) -> DbResult<ServerInfo> {
        let client = self.pool.get().await?;
        let row = client
            .query_one("SELECT version()", &[])
            .await?;
        Ok(ServerInfo {
            kind_label: "PostgreSQL".into(),
            server_version: row.get::<_, String>(0),
        })
    }

    /// PG 的可浏览实体是 schema（同连接内跨 schema 查询无需重连；
/// `database` 参数在本驱动中即 schema 上下文，见文件头说明）
async fn list_databases(&self) -> DbResult<Vec<String>> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                &templates::resolve("databases.postgresql", builtin_sql::DATABASES),
                &[],
            )
            .await?;
        Ok(rows.iter().map(|r| r.get(0)).collect())
    }

    async fn list_tables(&self, database: Option<&str>) -> DbResult<Vec<TableInfo>> {
        let schema = database.filter(|s| !s.is_empty()).unwrap_or("public");
        let client = self.pool.get().await?;
        let rows = client
            .query(
                &templates::resolve("tables.postgresql", builtin_sql::TABLES),
                &[&schema],
            )
            .await?;
        Ok(rows
            .iter()
            .map(|r| TableInfo {
                name: r.get(0),
                table_type: r.get(1),
                comment: r.try_get::<_, Option<String>>(2).unwrap_or(None),
            })
            .collect())
    }

    async fn describe_table(
        &self,
        database: Option<&str>,
        table: &str,
    ) -> DbResult<Vec<ColumnMeta>> {
        let schema = database.filter(|s| !s.is_empty()).unwrap_or("public");
        let client = self.pool.get().await?;
        let rows = client
            .query(
                &templates::resolve("columns.postgresql", builtin_sql::COLUMNS),
                &[&schema, &table],
            )
            .await?;
        Ok(rows
            .iter()
            .map(|r| ColumnMeta {
                name: r.get(0),
                data_type: r.get(1),
                nullable: r.get::<_, String>(2).eq_ignore_ascii_case("YES"),
                default_value: r.get(3),
                comment: r.try_get::<_, Option<String>>(4).unwrap_or(None),
                key: None,
            })
            .collect())
    }

    async fn execute(
        &self,
        database: Option<&str>,
        sql: &str,
        max_rows: u32,
    ) -> DbResult<QueryResult> {
        // 手动提交：专用连接承载事务（惰性 BEGIN）
        if self.manual.load(Ordering::Relaxed) {
            let mut guard = self.tx_conn.lock().await;
            if guard.is_none() {
                *guard = Some(PgTx {
                    conn: self.pool.get().await?,
                    in_tx: false,
                });
            }
            let tx = guard.as_mut().expect("tx connection just ensured");
            if !tx.in_tx {
                tx.conn.batch_execute("BEGIN").await?;
                tx.in_tx = true;
            }
            return execute_on(&tx.conn, database, sql, max_rows).await;
        }
        let client = self.pool.get().await?;
        execute_on(&client, database, sql, max_rows).await
    }

    async fn set_autocommit(&self, enabled: bool) -> DbResult<()> {
        let mut guard = self.tx_conn.lock().await;
        if enabled {
            if let Some(tx) = guard.take()
                && tx.in_tx {
                    let _ = tx.conn.batch_execute("COMMIT").await;
                }
            self.manual.store(false, Ordering::Relaxed);
        } else {
            if guard.is_none() {
                *guard = Some(PgTx {
                    conn: self.pool.get().await?,
                    in_tx: false,
                });
            }
            self.manual.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    async fn commit(&self) -> DbResult<()> {
        let mut guard = self.tx_conn.lock().await;
        if let Some(tx) = guard.as_mut()
            && tx.in_tx {
                tx.conn.batch_execute("COMMIT").await?;
                tx.in_tx = false;
            }
        Ok(())
    }

    async fn rollback(&self) -> DbResult<()> {
        let mut guard = self.tx_conn.lock().await;
        if let Some(tx) = guard.as_mut()
            && tx.in_tx {
                tx.conn.batch_execute("ROLLBACK").await?;
                tx.in_tx = false;
            }
        Ok(())
    }

    fn dialect(&self) -> Arc<dyn SqlDialect> {
        Arc::new(PostgresDialect::default())
    }

    async fn run_catalog(
        &self,
        obj: DbObjectType,
        q: &CatalogQuery,
        schema: Option<&str>,
    ) -> DbResult<Vec<ObjectInfo>> {
        let client = self.pool.get().await?;
        let rows = if q.has_param {
            let s = schema
                .filter(|s| !s.is_empty())
                .unwrap_or("public")
                .to_string();
            client.query(q.sql.as_str(), &[&s]).await?
        } else {
            client.query(q.sql.as_str(), &[]).await?
        };
        Ok(rows
            .iter()
            .map(|r| ObjectInfo {
                name: r.get(q.name_index),
                object_type: obj,
                comment: q
                    .comment_index
                    .and_then(|i| r.try_get::<_, Option<String>>(i).unwrap_or(None))
                    .filter(|c| !c.is_empty()),
                status: q
                    .status_index
                    .and_then(|i| r.try_get::<_, Option<String>>(i).unwrap_or(None))
                    .filter(|s| !s.is_empty()),
            })
            .collect())
    }

    async fn run_catalog_count(&self, q: &CatalogQuery, schema: Option<&str>) -> DbResult<u32> {
        let client = self.pool.get().await?;
        let row = if q.has_param {
            let s = schema
                .filter(|s| !s.is_empty())
                .unwrap_or("public")
                .to_string();
            client.query_one(q.sql.as_str(), &[&s]).await?
        } else {
            client.query_one(q.sql.as_str(), &[]).await?
        };
        Ok(row.get::<_, i64>(0).clamp(0, u32::MAX as i64) as u32)
    }

    async fn describe_routine(
        &self,
        database: Option<&str>,
        name: &str,
        routine_type: DbObjectType,
    ) -> DbResult<RoutineInfo> {
        let schema = database
            .filter(|s| !s.is_empty())
            .unwrap_or("public")
            .to_string();
        let client = self.pool.get().await?;
        let row = client
            .query_opt(
                &templates::resolve("routines.postgresql", builtin_sql::ROUTINES),
                &[&schema, &name],
            )
            .await?;
        let Some(row) = row else {
            return Err(AppError::General(format!("未找到例程 {name}")));
        };
        let prokind: String = row.get(0);
        let args: String = row.try_get(1).unwrap_or_default();
        let result: String = row.try_get(2).unwrap_or_default();
        let is_func = routine_type == DbObjectType::Function || prokind == "f";
        Ok(RoutineInfo {
            name: name.to_string(),
            routine_type: if is_func { "FUNCTION" } else { "PROCEDURE" }.into(),
            return_type: if is_func && !result.is_empty() {
                Some(result)
            } else {
                None
            },
            params: parse_pg_args(&args),
        })
    }

    async fn query_text(&self, sql: &str, params: &[String], value_index: usize) -> DbResult<String> {
        use tokio_postgres::types::ToSql;
        let client = self.pool.get().await?;
        let binds: Vec<&(dyn ToSql + Sync)> = params.iter().map(|s| s as &(dyn ToSql + Sync)).collect();
        let row = client.query_opt(sql, &binds).await?;
        Ok(row
            .and_then(|r| r.try_get::<_, Option<String>>(value_index).unwrap_or(None))
            .unwrap_or_default())
    }

    async fn list_indexes(&self, database: Option<&str>, table: &str) -> DbResult<Vec<IndexInfo>> {
        let schema = database
            .filter(|s| !s.is_empty())
            .unwrap_or("public")
            .to_string();
        let client = self.pool.get().await?;
        let rows = client
            .query(
                &templates::resolve("indexes.postgresql", builtin_sql::INDEXES),
                &[&schema, &table],
            )
            .await?;
        Ok(rows
            .iter()
            .map(|r| {
                let def: String = r.try_get(3).unwrap_or_default();
                IndexInfo {
                    name: r.get(0),
                    columns: parse_index_columns(&def),
                    unique: r.get(1),
                    primary: r.get(2),
                }
            })
            .collect())
    }
}

/// 从 pg_get_indexdef 输出中提取索引列（首个括号组，顶层逗号分割）
pub(crate) fn parse_index_columns(def: &str) -> Vec<String> {
    let Some(start) = def.find('(') else {
        return vec![];
    };
    let mut depth = 0i32;
    let mut end = start;
    for (i, ch) in def.char_indices().skip(start) {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    if end <= start {
        return vec![];
    }
    let inner = &def[start + 1..end];
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut d = 0i32;
    for ch in inner.chars() {
        match ch {
            '(' => {
                d += 1;
                cur.push(ch);
            }
            ')' => {
                d -= 1;
                cur.push(ch);
            }
            ',' if d == 0 => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out.into_iter()
        .map(|c| c.trim_matches('"').to_string())
        .collect()
}

/// 在指定连接上执行（自动提交 / 手动事务共用）
async fn execute_on(
    client: &Client,
    database: Option<&str>,
    sql: &str,
    max_rows: u32,
) -> DbResult<QueryResult> {
    let start = std::time::Instant::now();
    if let Some(schema) = database.filter(|s| !s.is_empty()) {
        // 兼容传入已带引号的名字；双引号成对转义，避免特殊字符改写语句
        let raw = schema.trim().trim_matches('"');
        let quoted = format!("\"{}\"", raw.replace('"', "\"\""));
        client
            .batch_execute(&format!("SET search_path TO {quoted}"))
            .await?;
    }

    // query_raw 惰性拉取，避免大结果集撑爆内存
    let stream = client.query_raw(sql, std::iter::empty::<i32>()).await?;
    let mut columns: Vec<ColumnInfo> = Vec::new();
    let mut rows_out: Vec<Vec<CellValue>> = Vec::new();
    let mut truncated = false;
    let mut first = true;

    let mut stream = std::pin::pin!(stream);
    while let Some(row) = stream.as_mut().try_next().await? {
        if first {
            let cols = row.columns();
            columns = cols
                .iter()
                .map(|c| ColumnInfo {
                    name: c.name().to_string(),
                    type_label: c.type_().to_string(),
                    semantic: PostgresDriver::semantic_of(c.type_()),
                })
                .collect();
            first = false;
        }
        if (rows_out.len() as u32) >= max_rows {
            truncated = true;
            break;
        }
        let cols = row.columns();
        rows_out.push(
            (0..cols.len())
                .map(|i| PostgresDriver::convert_cell(&row, i, cols[i].type_()))
                .collect(),
        );
    }

    // 无结果集 => DML
    let affected_rows = if columns.is_empty() {
        stream.rows_affected()
    } else {
        None
    };

    Ok(QueryResult {
        columns,
        rows: rows_out,
        affected_rows,
        duration_ms: start.elapsed().as_millis() as u64,
        truncated,
    })
}

/// 解析 pg_get_function_arguments 输出（如 `a integer, b text DEFAULT 'x'`）
pub(crate) fn parse_pg_args(s: &str) -> Vec<RoutineParam> {
    split_top_level(s)
        .into_iter()
        .filter(|p| !p.trim().is_empty())
        .map(|p| {
            let mut t = p.trim().to_string();
            let mut mode = "IN".to_string();
            let upper = t.to_uppercase();
            for (prefix, m) in [
                ("INOUT ", "INOUT"),
                ("IN OUT ", "INOUT"),
                ("OUT ", "OUT"),
                ("VARIADIC ", "IN"),
            ] {
                if upper.starts_with(prefix) {
                    t = t[prefix.len()..].to_string();
                    mode = m.into();
                    break;
                }
            }
            // 去除 DEFAULT 子句
            if let Some(i) = t.to_uppercase().find(" DEFAULT ") {
                t.truncate(i);
            }
            let t = t.trim();
            let tokens: Vec<&str> = t.split_whitespace().collect();
            if tokens.len() >= 2 {
                RoutineParam {
                    name: Some(tokens[0].to_string()),
                    mode,
                    data_type: tokens[1..].join(" "),
                }
            } else {
                RoutineParam {
                    name: None,
                    mode,
                    data_type: t.to_string(),
                }
            }
        })
        .collect()
}

/// 按顶层（括号深度 0）逗号切分
fn split_top_level(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for ch in s.chars() {
        match ch {
            '(' => {
                depth += 1;
                cur.push(ch);
            }
            ')' => {
                depth -= 1;
                cur.push(ch);
            }
            ',' if depth == 0 => {
                out.push(cur.clone());
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

/// 驱动层内置 SQL（模板注册表：方法与设置页规约共用，避免两处维护）。
/// 参数占位符 `$1..`（覆盖时不得增删占位符）。
pub(crate) mod builtin_sql {
    pub const DATABASES: &str = "SELECT nspname FROM pg_namespace \
         WHERE nspname NOT LIKE 'pg_%' AND nspname <> 'information_schema' \
         ORDER BY nspname";
    pub const TABLES: &str = "SELECT tablename, 'TABLE', obj_description(('\"' || schemaname || '\".\"' || tablename || '\"')::regclass, 'pg_class') \
         FROM pg_tables WHERE schemaname = $1 \
         UNION ALL \
         SELECT viewname, 'VIEW', NULL FROM pg_views WHERE schemaname = $1 \
         ORDER BY 1";
    pub const COLUMNS: &str = "SELECT column_name, data_type, is_nullable, column_default, \
         col_description(('\"' || table_schema || '\".\"' || table_name || '\"')::regclass, ordinal_position) \
         FROM information_schema.columns \
         WHERE table_schema = $1 AND table_name = $2 \
         ORDER BY ordinal_position";
    pub const INDEXES: &str = "SELECT i.relname, ix.indisunique, ix.indisprimary, pg_get_indexdef(ix.indexrelid) \
         FROM pg_index ix \
         JOIN pg_class i ON i.oid = ix.indexrelid \
         JOIN pg_class t ON t.oid = ix.indrelid \
         JOIN pg_namespace n ON n.oid = t.relnamespace \
         WHERE n.nspname = $1 AND t.relname = $2 \
         ORDER BY ix.indisprimary DESC, i.relname";
    pub const ROUTINES: &str = "SELECT p.prokind::text, pg_get_function_arguments(p.oid), \
         pg_get_function_result(p.oid) \
         FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace \
         WHERE n.nspname = $1 AND p.proname = $2 LIMIT 1";
}

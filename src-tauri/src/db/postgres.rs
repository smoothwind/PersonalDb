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
            Type::VARCHAR | Type::TEXT | Type::BPCHAR | Type::NAME | Type::CHAR | Type::UNKNOWN => {
                SemanticType::Text
            }
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
                "SELECT nspname FROM pg_namespace \
                 WHERE nspname NOT LIKE 'pg_%' AND nspname <> 'information_schema' \
                 ORDER BY nspname",
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
                "SELECT tablename, 'TABLE', obj_description(('\"' || schemaname || '\".\"' || tablename || '\"')::regclass, 'pg_class') \
                 FROM pg_tables WHERE schemaname = $1 \
                 UNION ALL \
                 SELECT viewname, 'VIEW', NULL FROM pg_views WHERE schemaname = $1 \
                 ORDER BY 1",
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
                "SELECT column_name, data_type, is_nullable, column_default, \
                 col_description(('\"' || table_schema || '\".\"' || table_name || '\"')::regclass, ordinal_position) \
                 FROM information_schema.columns \
                 WHERE table_schema = $1 AND table_name = $2 \
                 ORDER BY ordinal_position",
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
        Arc::new(PostgresDialect)
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
                "SELECT p.prokind::text, pg_get_function_arguments(p.oid), \
                 pg_get_function_result(p.oid) \
                 FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace \
                 WHERE n.nspname = $1 AND p.proname = $2 LIMIT 1",
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
                "SELECT i.relname, ix.indisunique, ix.indisprimary, pg_get_indexdef(ix.indexrelid) \
                 FROM pg_index ix \
                 JOIN pg_class i ON i.oid = ix.indexrelid \
                 JOIN pg_class t ON t.oid = ix.indrelid \
                 JOIN pg_namespace n ON n.oid = t.relnamespace \
                 WHERE n.nspname = $1 AND t.relname = $2 \
                 ORDER BY ix.indisprimary DESC, i.relname",
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
fn parse_index_columns(def: &str) -> Vec<String> {
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
fn parse_pg_args(s: &str) -> Vec<RoutineParam> {
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

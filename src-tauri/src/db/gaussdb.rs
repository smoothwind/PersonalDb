//! GaussDB / openGauss 驱动（官方 tokio-gaussdb，支持 sha256 认证）。
//!
//! 系统目录与 PostgreSQL 高度兼容，方言复用 `PostgresDialect::gauss()`。
//! `database` 参数与 PG 驱动一致：既作为连接库名（缺省 postgres），
//! 也作为执行时 `SET search_path` 的 schema 上下文。

use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use futures_util::TryStreamExt;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_gaussdb::types::Type;
use tokio_gaussdb::{Client, Config as GaussConfig, NoTls, Row};

use crate::error::{AppError, DbResult};

use super::dialect::{PostgresDialect, SqlDialect};
use super::driver::DatabaseDriver;
use super::objects::{CatalogQuery, DbObjectType, ObjectInfo};
use super::postgres::{parse_index_columns, parse_pg_args};
use super::types::{
    CellValue, ColumnInfo, ColumnMeta, ConnectionConfig, IndexInfo, QueryResult, RoutineInfo,
    SemanticType, ServerInfo, TableInfo,
};

const MAX_CONNS: usize = 8;

pub struct GaussDriver {
    pool: Arc<GaussPool>,
    /// 手动提交模式
    manual: AtomicBool,
    /// 手动模式下的专用连接（承载未提交事务）
    tx_conn: tokio::sync::Mutex<Option<GaussTx>>,
}

struct GaussPool {
    config: GaussConfig,
    idle: Mutex<Vec<Client>>,
    permits: Arc<Semaphore>,
}

impl GaussPool {
    fn new(config: GaussConfig) -> Self {
        Self {
            config,
            idle: Mutex::new(Vec::new()),
            permits: Arc::new(Semaphore::new(MAX_CONNS)),
        }
    }

    async fn acquire(self: &Arc<Self>) -> DbResult<GaussConn> {
        let permit = self
            .permits
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| AppError::General("GaussDB 连接池已关闭".into()))?;
        // 跳过已断开的空闲连接（锁只在取连接期间持有）
        loop {
            let idle = self
                .idle
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .pop();
            match idle {
                Some(client) if !client.is_closed() => {
                    return Ok(GaussConn {
                        pool: Arc::clone(self),
                        client: Some(client),
                        _permit: permit,
                    });
                }
                Some(_) => continue,
                None => break,
            }
        }
        let client = self.connect().await?;
        Ok(GaussConn {
            pool: Arc::clone(self),
            client: Some(client),
            _permit: permit,
        })
    }

    async fn connect(&self) -> DbResult<Client> {
        let (client, connection) = self.config.connect(NoTls).await?;
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                log::warn!("GaussDB 连接中断: {e}");
            }
        });
        Ok(client)
    }

    fn put(&self, client: Client) {
        self.idle
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(client);
    }
}

/// 池化连接守卫：Drop 时归还连接并释放并发名额
struct GaussConn {
    pool: Arc<GaussPool>,
    client: Option<Client>,
    _permit: OwnedSemaphorePermit,
}

impl GaussConn {
    fn client(&self) -> &Client {
        self.client.as_ref().expect("GaussDB 连接已归还")
    }
}

impl Deref for GaussConn {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        self.client()
    }
}

impl Drop for GaussConn {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            if !client.is_closed() {
                self.pool.put(client);
            }
        }
    }
}

/// 手动事务下的专用连接与事务状态
struct GaussTx {
    conn: GaussConn,
    in_tx: bool,
}

impl GaussDriver {
    pub fn new(cfg: &ConnectionConfig) -> DbResult<Self> {
        let mut gc = GaussConfig::new();
        gc.host(&cfg.host)
            .port(cfg.effective_port())
            .user(&cfg.username)
            .password(&cfg.password);
        let dbname = cfg
            .database
            .as_deref()
            .filter(|d| !d.is_empty())
            .unwrap_or("postgres");
        gc.dbname(dbname);
        if let Some(timeout) = cfg
            .params
            .get("connect_timeout")
            .and_then(|v| v.parse::<u64>().ok())
        {
            gc.connect_timeout(std::time::Duration::from_secs(timeout));
        }
        Ok(Self {
            pool: Arc::new(GaussPool::new(gc)),
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
        let opt_cell: Result<Option<CellValue>, _> = match *ty {
            Type::BOOL => row
                .try_get::<_, Option<bool>>(i)
                .map(|v| v.map(CellValue::Bool)),
            Type::INT2 => row
                .try_get::<_, Option<i16>>(i)
                .map(|v| v.map(|x| CellValue::Int(x as i64))),
            Type::INT4 => row
                .try_get::<_, Option<i32>>(i)
                .map(|v| v.map(|x| CellValue::Int(x as i64))),
            Type::INT8 => row
                .try_get::<_, Option<i64>>(i)
                .map(|v| v.map(CellValue::Int)),
            Type::FLOAT4 => row
                .try_get::<_, Option<f32>>(i)
                .map(|v| v.map(|x| CellValue::Float(x as f64))),
            Type::FLOAT8 => row
                .try_get::<_, Option<f64>>(i)
                .map(|v| v.map(CellValue::Float)),
            Type::OID => row
                .try_get::<_, Option<u32>>(i)
                .map(|v| v.map(|x| CellValue::UInt(x as u64))),
            Type::VARCHAR | Type::TEXT | Type::BPCHAR | Type::NAME | Type::CHAR | Type::UNKNOWN => {
                row.try_get::<_, Option<String>>(i)
                    .map(|v| v.map(CellValue::Text))
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
            _ => row
                .try_get::<_, Option<String>>(i)
                .map(|v| v.map(CellValue::Text)),
        };
        match opt_cell {
            Ok(Some(c)) => c,
            Ok(None) => CellValue::Null,
            Err(_) => CellValue::Text("<无法显示的值>".into()),
        }
    }
}

#[async_trait]
impl DatabaseDriver for GaussDriver {
    async fn server_info(&self) -> DbResult<ServerInfo> {
        let conn = self.pool.acquire().await?;
        let row = conn.query_one("SELECT version()", &[]).await?;
        Ok(ServerInfo {
            kind_label: "GaussDB".into(),
            server_version: row.get::<_, String>(0),
        })
    }

    async fn list_databases(&self) -> DbResult<Vec<String>> {
        let conn = self.pool.acquire().await?;
        let rows = conn
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
        let conn = self.pool.acquire().await?;
        let rows = conn
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
        let conn = self.pool.acquire().await?;
        let rows = conn
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
        if self.manual.load(Ordering::Relaxed) {
            let mut guard = self.tx_conn.lock().await;
            if guard.is_none() {
                *guard = Some(GaussTx {
                    conn: self.pool.acquire().await?,
                    in_tx: false,
                });
            }
            let tx = guard.as_mut().expect("tx connection just ensured");
            if !tx.in_tx {
                tx.conn.batch_execute("BEGIN").await?;
                tx.in_tx = true;
            }
            return execute_on(tx.conn.client(), database, sql, max_rows).await;
        }
        let conn = self.pool.acquire().await?;
        execute_on(conn.client(), database, sql, max_rows).await
    }

    async fn set_autocommit(&self, enabled: bool) -> DbResult<()> {
        let mut guard = self.tx_conn.lock().await;
        if enabled {
            if let Some(tx) = guard.take()
                && tx.in_tx
            {
                let _ = tx.conn.batch_execute("COMMIT").await;
            }
            self.manual.store(false, Ordering::Relaxed);
        } else {
            if guard.is_none() {
                *guard = Some(GaussTx {
                    conn: self.pool.acquire().await?,
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
            && tx.in_tx
        {
            tx.conn.batch_execute("COMMIT").await?;
            tx.in_tx = false;
        }
        Ok(())
    }

    async fn rollback(&self) -> DbResult<()> {
        let mut guard = self.tx_conn.lock().await;
        if let Some(tx) = guard.as_mut()
            && tx.in_tx
        {
            tx.conn.batch_execute("ROLLBACK").await?;
            tx.in_tx = false;
        }
        Ok(())
    }

    fn dialect(&self) -> Arc<dyn SqlDialect> {
        Arc::new(PostgresDialect::gauss())
    }

    async fn run_catalog(
        &self,
        obj: DbObjectType,
        q: &CatalogQuery,
        schema: Option<&str>,
    ) -> DbResult<Vec<ObjectInfo>> {
        let conn = self.pool.acquire().await?;
        let rows = if q.has_param {
            let s = schema
                .filter(|s| !s.is_empty())
                .unwrap_or("public")
                .to_string();
            conn.query(q.sql.as_str(), &[&s]).await?
        } else {
            conn.query(q.sql.as_str(), &[]).await?
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
        let conn = self.pool.acquire().await?;
        let row = if q.has_param {
            let s = schema
                .filter(|s| !s.is_empty())
                .unwrap_or("public")
                .to_string();
            conn.query_one(q.sql.as_str(), &[&s]).await?
        } else {
            conn.query_one(q.sql.as_str(), &[]).await?
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
        let conn = self.pool.acquire().await?;
        let row = conn
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
        use tokio_gaussdb::types::ToSql;
        let conn = self.pool.acquire().await?;
        let binds: Vec<&(dyn ToSql + Sync)> =
            params.iter().map(|s| s as &(dyn ToSql + Sync)).collect();
        let row = conn.query_opt(sql, &binds).await?;
        Ok(row
            .and_then(|r| r.try_get::<_, Option<String>>(value_index).unwrap_or(None))
            .unwrap_or_default())
    }

    async fn list_indexes(&self, database: Option<&str>, table: &str) -> DbResult<Vec<IndexInfo>> {
        let schema = database
            .filter(|s| !s.is_empty())
            .unwrap_or("public")
            .to_string();
        let conn = self.pool.acquire().await?;
        let rows = conn
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

async fn execute_on(
    client: &Client,
    database: Option<&str>,
    sql: &str,
    max_rows: u32,
) -> DbResult<QueryResult> {
    let start = std::time::Instant::now();
    if let Some(schema) = database.filter(|s| !s.is_empty()) {
        let raw = schema.trim().trim_matches('"');
        let quoted = format!("\"{}\"", raw.replace('"', "\"\""));
        client
            .batch_execute(&format!("SET search_path TO {quoted}"))
            .await?;
    }

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
                    semantic: GaussDriver::semantic_of(c.type_()),
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
                .map(|i| GaussDriver::convert_cell(&row, i, cols[i].type_()))
                .collect(),
        );
    }

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

impl std::fmt::Debug for GaussDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GaussDriver").finish_non_exhaustive()
    }
}

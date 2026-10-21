//! SQLite 驱动（rusqlite bundled，嵌入式文件数据库）。
//!
//! 连接 = 数据库文件路径（`params.file`，兼容 `database` 字段）。SQLite 同步 API，
//! 全部经 `spawn_blocking` 包装避免阻塞 runtime（与 Oracle 同步驱动同约定）；
//! 手动事务由专用连接承载（与 PostgreSQL 驱动同模式，BEGIN/COMMIT 由 tx_open 跟踪）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};

use crate::error::{AppError, DbResult};

use super::dialect::{SqlDialect, SqliteDialect};
use super::driver::DatabaseDriver;
use super::objects::{aggregate_index_rows, CatalogQuery, DbObjectType, ObjectInfo};
use super::types::{
    CellValue, ColumnInfo, ColumnMeta, ConnectionConfig, IndexInfo, QueryResult, RoutineInfo,
    SemanticType, ServerInfo, TableInfo,
};

pub struct SqliteDriver {
    conn: Arc<Mutex<Connection>>,
    /// 手动事务专用连接（惰性打开同一文件）
    tx_conn: Arc<Mutex<Option<Connection>>>,
    /// 手动提交模式开关（Arc 化便于移入 spawn_blocking 闭包）
    manual: Arc<AtomicBool>,
    /// 专用连接上是否已 BEGIN
    tx_open: Arc<AtomicBool>,
}

impl SqliteDriver {
    pub fn new(cfg: &ConnectionConfig) -> DbResult<Self> {
        let file = file_of(cfg)
            .ok_or_else(|| AppError::General("请在连接配置中选择 SQLite 数据库文件".into()))?;
        let conn = open_sqlite(&file)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            tx_conn: Arc::new(Mutex::new(None)),
            manual: Arc::new(AtomicBool::new(false)),
            tx_open: Arc::new(AtomicBool::new(false)),
        })
    }

    /// decltype（如 INTEGER/TEXT/DECIMAL(10,2)）→ 语义类型；SQLite 动态类型，值按实际存储类转换
    fn semantic_of_decl(decl: Option<&str>) -> SemanticType {
        let Some(d) = decl else {
            return SemanticType::Unknown;
        };
        let u = d.to_ascii_uppercase();
        if u.contains("INT") {
            SemanticType::Number
        } else if u.contains("BOOL") {
            SemanticType::Bool
        } else if u.contains("DEC") || u.contains("NUM") {
            SemanticType::Decimal
        } else if u.contains("REAL") || u.contains("FLOA") || u.contains("DOUB") {
            SemanticType::Number
        } else if u.contains("JSON") {
            SemanticType::Json
        } else if u.contains("BLOB") || u.contains("BINARY") {
            SemanticType::Binary
        } else if u.contains("TIMESTAMP") || u.contains("DATETIME") {
            SemanticType::DateTime
        } else if u.contains("DATE") {
            SemanticType::Date
        } else if u.contains("TIME") {
            SemanticType::Time
        } else if u.contains("CHAR") || u.contains("TEXT") || u.contains("CLOB") {
            SemanticType::Text
        } else {
            SemanticType::Unknown
        }
    }

    fn convert_cell(vr: ValueRef<'_>) -> CellValue {
        match vr {
            ValueRef::Null => CellValue::Null,
            ValueRef::Integer(i) => CellValue::Int(i),
            ValueRef::Real(f) => CellValue::Float(f),
            ValueRef::Text(b) => CellValue::Text(String::from_utf8_lossy(b).into_owned()),
            ValueRef::Blob(b) => {
                CellValue::Binary(base64::engine::general_purpose::STANDARD.encode(b))
            }
        }
    }
}

fn file_of(cfg: &ConnectionConfig) -> Option<String> {
    let f = cfg
        .params
        .get("file")
        .map(|s| s.as_str())
        .filter(|f| !f.trim().is_empty())
        .or_else(|| cfg.database.as_deref().filter(|d| !d.trim().is_empty()));
    f.map(|s| s.trim().to_string())
}

fn open_sqlite(file: &str) -> DbResult<Connection> {
    // 不带 SQLITE_OPEN_CREATE：文件不存在时报错而非静默建空库
    let conn = Connection::open_with_flags(
        file,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| AppError::General(format!("无法打开数据库文件: {e}")))?;
    // 主连接 + 手动事务专用连接并发访问时的写锁等待
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|e| AppError::General(format!("busy_timeout 失败: {e}")))?;
    Ok(conn)
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 同步任务包装：join 失败（panic）统一转 General 错误
async fn blocking<T, F>(f: F) -> DbResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> DbResult<T> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::General(format!("后台任务失败: {e}")))?
}

fn sql_err(e: impl std::fmt::Display) -> AppError {
    AppError::General(format!("SQL 错误: {e}"))
}

/// 在指定连接上执行（自动提交 / 手动事务共用）
fn execute_on(conn: &Connection, sql: &str, max_rows: u32) -> DbResult<QueryResult> {
    let start = std::time::Instant::now();
    let mut stmt = conn.prepare(sql).map_err(sql_err)?;
    let col_count = stmt.column_count();
    if col_count == 0 {
        let affected = stmt.execute([]).map_err(sql_err)? as u64;
        return Ok(QueryResult {
            columns: Vec::new(),
            rows: Vec::new(),
            affected_rows: Some(affected),
            duration_ms: start.elapsed().as_millis() as u64,
            truncated: false,
        });
    }
    let columns: Vec<ColumnInfo> = stmt
        .columns()
        .iter()
        .map(|c| {
            let decl = c.decl_type();
            ColumnInfo {
                name: c.name().to_string(),
                type_label: decl.unwrap_or("ANY").to_string(),
                semantic: SqliteDriver::semantic_of_decl(decl),
            }
        })
        .collect();
    let mut rows_out: Vec<Vec<CellValue>> = Vec::new();
    let mut truncated = false;
    let mut rows = stmt.query([]).map_err(sql_err)?;
    while let Some(row) = rows.next().map_err(sql_err)? {
        if (rows_out.len() as u32) >= max_rows {
            truncated = true;
            break;
        }
        rows_out.push(
            (0..col_count)
                .map(|i| {
                    SqliteDriver::convert_cell(row.get_ref(i).unwrap_or(ValueRef::Null))
                })
                .collect(),
        );
    }
    Ok(QueryResult {
        columns,
        rows: rows_out,
        affected_rows: None,
        duration_ms: start.elapsed().as_millis() as u64,
        truncated,
    })
}

#[async_trait]
impl DatabaseDriver for SqliteDriver {
    async fn server_info(&self) -> DbResult<ServerInfo> {
        let conn = self.conn.clone();
        let version = blocking(move || {
            lock(&conn)
                .query_row("SELECT sqlite_version()", [], |r| r.get::<_, String>(0))
                .map_err(sql_err)
        })
        .await?;
        Ok(ServerInfo {
            kind_label: "SQLite".into(),
            server_version: version,
        })
    }

    /// 单文件库仅 main；ATTACH 的库不在此管理（重连即失）
    async fn list_databases(&self) -> DbResult<Vec<String>> {
        Ok(vec!["main".into()])
    }

    async fn list_tables(&self, _database: Option<&str>) -> DbResult<Vec<TableInfo>> {
        let conn = self.conn.clone();
        blocking(move || {
            let conn = lock(&conn);
            let mut stmt = conn
                .prepare(
                    "SELECT name, CASE type WHEN 'table' THEN 'TABLE' ELSE 'VIEW' END \
                     FROM sqlite_master \
                     WHERE type IN ('table','view') AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\' \
                     ORDER BY 1",
                )
                .map_err(sql_err)?;
            let rows = stmt
                .query_map([], |r| {
                    Ok(TableInfo {
                        name: r.get::<_, String>(0)?,
                        table_type: r.get::<_, String>(1)?,
                        comment: None,
                    })
                })
                .map_err(sql_err)?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r.map_err(|e| AppError::General(format!("行读取失败: {e}")))?);
            }
            Ok(out)
        })
        .await
    }

    async fn describe_table(
        &self,
        _database: Option<&str>,
        table: &str,
    ) -> DbResult<Vec<ColumnMeta>> {
        let conn = self.conn.clone();
        let table = table.to_string();
        blocking(move || {
            let conn = lock(&conn);
            let mut stmt = conn
                .prepare(
                    "SELECT name, type, \"notnull\", dflt_value, pk \
                     FROM pragma_table_info(?) ORDER BY cid",
                )
                .map_err(sql_err)?;
            let rows = stmt
                .query_map([&table], |r| {
                    let pk: i64 = r.get(4)?;
                    Ok(ColumnMeta {
                        name: r.get::<_, String>(0)?,
                        data_type: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                        nullable: r.get::<_, i64>(2)? == 0,
                        default_value: r.get::<_, Option<String>>(3)?,
                        comment: None,
                        key: if pk > 0 { Some("PRI".into()) } else { None },
                    })
                })
                .map_err(sql_err)?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r.map_err(|e| AppError::General(format!("行读取失败: {e}")))?);
            }
            Ok(out)
        })
        .await
    }

    async fn describe_routine(
        &self,
        _database: Option<&str>,
        name: &str,
        _routine_type: DbObjectType,
    ) -> DbResult<RoutineInfo> {
        Err(AppError::General(format!(
            "SQLite 无存储过程/函数（对象 {name} 不适用）"
        )))
    }

    async fn list_indexes(&self, _database: Option<&str>, table: &str) -> DbResult<Vec<IndexInfo>> {
        let conn = self.conn.clone();
        let table = table.to_string();
        blocking(move || {
            let conn = lock(&conn);
            let mut stmt = conn
                .prepare("SELECT name, \"unique\", origin FROM pragma_index_list(?) ORDER BY seq")
                .map_err(sql_err)?;
            let metas: Vec<(String, bool, bool)> = stmt
                .query_map([&table], |r| {
                    let origin: String = r.get(2)?;
                    Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? != 0, origin == "pk"))
                })
                .map_err(sql_err)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| AppError::General(format!("行读取失败: {e}")))?;
            // (索引序号, 列名, 索引名) 展开后按相邻同名归并
            let mut idx_cols: Vec<(usize, String, String)> = Vec::new();
            for (i, (name, _, _)) in metas.iter().enumerate() {
                let mut cs = conn
                    .prepare("SELECT name FROM pragma_index_info(?) ORDER BY seqno")
                    .map_err(sql_err)?;
                let rows = cs.query_map([name], |r| r.get::<_, Option<String>>(0)).map_err(sql_err)?;
                for r in rows {
                    let col = r
                        .map_err(|e| AppError::General(format!("行读取失败: {e}")))?
                        .unwrap_or_default();
                    if !col.is_empty() {
                        idx_cols.push((i, col, name.clone()));
                    }
                }
            }
            Ok(aggregate_index_rows(
                idx_cols,
                |(i, _, name)| (name.clone(), metas[*i].1, metas[*i].2),
                |(_, col, _)| col.clone(),
            ))
        })
        .await
    }

    async fn set_autocommit(&self, enabled: bool) -> DbResult<()> {
        let tx = self.tx_conn.clone();
        let manual = self.manual.clone();
        let tx_open = self.tx_open.clone();
        blocking(move || {
            let mut guard = lock(&tx);
            if enabled {
                // 切回自动提交：提交遗留事务并释放专用连接
                if let Some(c) = guard.as_ref() {
                    let _ = c.execute_batch("COMMIT");
                }
                *guard = None;
                tx_open.store(false, Ordering::Relaxed);
                manual.store(false, Ordering::Relaxed);
            } else {
                manual.store(true, Ordering::Relaxed);
            }
            Ok(())
        })
        .await
    }

    async fn commit(&self) -> DbResult<()> {
        let tx = self.tx_conn.clone();
        let tx_open = self.tx_open.clone();
        blocking(move || {
            if !tx_open.load(Ordering::Relaxed) {
                return Ok(());
            }
            let guard = lock(&tx);
            if let Some(c) = guard.as_ref() {
                c.execute_batch("COMMIT")
                    .map_err(|e| AppError::General(format!("提交失败: {e}")))?;
            }
            tx_open.store(false, Ordering::Relaxed);
            Ok(())
        })
        .await
    }

    async fn rollback(&self) -> DbResult<()> {
        let tx = self.tx_conn.clone();
        let tx_open = self.tx_open.clone();
        blocking(move || {
            if !tx_open.load(Ordering::Relaxed) {
                return Ok(());
            }
            let guard = lock(&tx);
            if let Some(c) = guard.as_ref() {
                c.execute_batch("ROLLBACK")
                    .map_err(|e| AppError::General(format!("回滚失败: {e}")))?;
            }
            tx_open.store(false, Ordering::Relaxed);
            Ok(())
        })
        .await
    }

    async fn execute(
        &self,
        _database: Option<&str>,
        sql: &str,
        max_rows: u32,
    ) -> DbResult<QueryResult> {
        let conn = self.conn.clone();
        let tx = self.tx_conn.clone();
        let tx_open = self.tx_open.clone();
        let manual = self.manual.load(Ordering::Relaxed);
        let sql = sql.to_string();
        blocking(move || {
            if manual {
                // 手动提交：专用连接承载事务（惰性打开 + 惰性 BEGIN）
                let mut guard = lock(&tx);
                if guard.is_none() {
                    let path = lock(&conn)
                        .path()
                        .map(|p| p.to_string())
                        .unwrap_or_default();
                    if path.is_empty() {
                        return Err(AppError::General("无法确定数据库文件路径".into()));
                    }
                    *guard = Some(open_sqlite(&path)?);
                    tx_open.store(false, Ordering::Relaxed);
                }
                let c = guard.as_ref().expect("tx connection just ensured");
                if !tx_open.load(Ordering::Relaxed) {
                    c.execute_batch("BEGIN").map_err(sql_err)?;
                    tx_open.store(true, Ordering::Relaxed);
                }
                return execute_on(c, &sql, max_rows);
            }
            let conn = lock(&conn);
            execute_on(&conn, &sql, max_rows)
        })
        .await
    }

    fn dialect(&self) -> Arc<dyn SqlDialect> {
        Arc::new(SqliteDialect)
    }

    async fn run_catalog(
        &self,
        obj: DbObjectType,
        q: &CatalogQuery,
        _schema: Option<&str>,
    ) -> DbResult<Vec<ObjectInfo>> {
        let conn = self.conn.clone();
        let sql = q.sql.clone();
        let name_index = q.name_index;
        let comment_index = q.comment_index;
        blocking(move || {
            let conn = lock(&conn);
            let mut stmt = conn.prepare(&sql).map_err(sql_err)?;
            let rows = stmt
                .query_map([], |r| {
                    let name: String = r.get(name_index)?;
                    let comment: Option<String> =
                        comment_index.and_then(|i| r.get::<_, Option<String>>(i).unwrap_or(None));
                    Ok(ObjectInfo {
                        name,
                        object_type: obj,
                        comment: comment.filter(|c| !c.is_empty()),
                        status: None,
                    })
                })
                .map_err(sql_err)?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r.map_err(|e| AppError::General(format!("行读取失败: {e}")))?);
            }
            Ok(out)
        })
        .await
    }

    async fn run_catalog_count(&self, q: &CatalogQuery, _schema: Option<&str>) -> DbResult<u32> {
        let conn = self.conn.clone();
        let sql = q.sql.clone();
        blocking(move || {
            let n: i64 = lock(&conn)
                .query_row(&sql, [], |r| r.get(0))
                .map_err(sql_err)?;
            Ok(n.clamp(0, u32::MAX as i64) as u32)
        })
        .await
    }

    async fn query_text(&self, sql: &str, params: &[String], value_index: usize) -> DbResult<String> {
        let conn = self.conn.clone();
        let sql = sql.to_string();
        let params: Vec<String> = params.to_vec();
        blocking(move || {
            let conn = lock(&conn);
            let mut stmt = conn.prepare(&sql).map_err(sql_err)?;
            let binds: Vec<&str> = params.iter().map(|s| s.as_str()).collect();
            let mut rows = stmt
                .query(rusqlite::params_from_iter(binds.iter()))
                .map_err(sql_err)?;
            if let Some(row) = rows.next().map_err(sql_err)? {
                return Ok(row
                    .get::<_, Option<String>>(value_index)
                    .unwrap_or(None)
                    .unwrap_or_default());
            }
            Ok(String::new())
        })
        .await
    }
}

impl std::fmt::Debug for SqliteDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteDriver").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::types::DbKind;

    fn test_cfg(file: &std::path::Path) -> ConnectionConfig {
        ConnectionConfig {
            id: "t".into(),
            name: "t".into(),
            kind: DbKind::Sqlite,
            host: String::new(),
            port: None,
            username: String::new(),
            password: String::new(),
            password_ref: None,
            database: None,
            params: [("file".to_string(), file.to_string_lossy().into_owned())]
                .into_iter()
                .collect(),
            group_id: None,
        }
    }

    #[tokio::test]
    async fn sqlite_driver_smoke() {
        // 独立临时库文件（先建表，驱动不自动 CREATE）
        let dir = std::env::temp_dir();
        let file = dir.join(format!("pdb_sqlite_smoke_{}.db", uuid::Uuid::new_v4()));
        {
            let raw = Connection::open(&file).unwrap();
            raw.execute_batch(
                "CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT NOT NULL, score REAL, blob BLOB);\
                 CREATE INDEX idx_name ON t(name);\
                 CREATE TABLE u(code TEXT PRIMARY KEY, v INTEGER);",
            )
            .unwrap();
        }
        let cfg = test_cfg(&file);
        let d = SqliteDriver::new(&cfg).unwrap();

        // 版本 / 库列表
        assert!(d.server_info().await.unwrap().server_version.contains('.'));
        assert_eq!(d.list_databases().await.unwrap(), vec!["main".to_string()]);

        // 表列表与结构
        let tables = d.list_tables(None).await.unwrap();
        assert_eq!(tables.len(), 2);
        assert!(tables.iter().any(|t| t.name == "t" && t.table_type == "TABLE"));
        let cols = d.describe_table(None, "t").await.unwrap();
        assert_eq!(cols[0].name, "id");
        assert_eq!(cols[0].key.as_deref(), Some("PRI"));
        assert!(!cols[1].nullable); // name TEXT NOT NULL
        assert!(cols[2].nullable); // score REAL

        // 插入（受影响行数）+ 查询（值/语义）
        let ins = d.execute(None, "INSERT INTO t VALUES (1, 'a', 1.5, X'00FF')", 10).await.unwrap();
        assert_eq!(ins.affected_rows, Some(1));
        let sel = d.execute(None, "SELECT id, name, score, blob FROM t", 10).await.unwrap();
        assert_eq!(sel.columns.len(), 4);
        assert_eq!(sel.columns[1].semantic, SemanticType::Text);
        assert_eq!(sel.rows[0][1], CellValue::Text("a".into()));
        assert!(matches!(sel.rows[0][3], CellValue::Binary(_)));

        // 索引：显式索引 + TEXT 主键的 autoindex（origin=pk；INTEGER PK 为 rowid 别名不建索引）
        let idx = d.list_indexes(None, "t").await.unwrap();
        assert!(idx.iter().any(|i| i.name == "idx_name" && i.columns == vec!["name"]));
        let idx_u = d.list_indexes(None, "u").await.unwrap();
        assert!(idx_u.iter().any(|i| i.primary && i.columns == vec!["code"]));

        // 手动事务：插入 → 回滚 → 行数不变
        d.set_autocommit(false).await.unwrap();
        d.execute(None, "INSERT INTO t VALUES (2, 'b', NULL, NULL)", 10).await.unwrap();
        d.rollback().await.unwrap();
        let cnt = d.execute(None, "SELECT COUNT(*) FROM t", 10).await.unwrap();
        assert_eq!(cnt.rows[0][0], CellValue::Int(1));
        d.set_autocommit(true).await.unwrap();

        // 对象目录（表计数含 t/u，索引 1 且排除 sqlite_autoindex）
        let dia = d.dialect();
        let q = dia.catalog_count_query(DbObjectType::Table, None).unwrap();
        assert_eq!(d.run_catalog_count(&q, None).await.unwrap(), 2);
        let qi = dia.catalog_query(DbObjectType::Index, None).unwrap();
        let objs = d.run_catalog(DbObjectType::Index, &qi, None).await.unwrap();
        assert_eq!(objs.len(), 1);
        assert_eq!(objs[0].name, "idx_name");

        // 文件缺失：不静默建库
        let missing = dir.join(format!("pdb_missing_{}.db", uuid::Uuid::new_v4()));
        assert!(SqliteDriver::new(&test_cfg(&missing)).is_err());

        drop(d);
        let _ = std::fs::remove_file(&file);
    }
}

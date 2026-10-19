//! MySQL / OceanBase(MySQL 兼容模式) 驱动。
//!
//! OceanBase 使用 MySQL 协议，用户名按 OB 规范填写即可：
//! - 直连模式：`user@tenant`（如 `root@sys`）
//! - proxy 模式：`user@tenant#cluster`

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use mysql_async::prelude::*;
use mysql_async::{Conn, OptsBuilder, Row, Value};

use crate::error::{AppError, DbResult};

use super::dialect::{MySqlFamilyDialect, SqlDialect};
use super::driver::DatabaseDriver;
use super::objects::{CatalogQuery, DbObjectType, ObjectInfo};
use super::types::{
    CellValue, ColumnInfo, ColumnMeta, ConnectionConfig, DbKind, IndexInfo, QueryResult,
    RoutineInfo, RoutineParam, SemanticType, ServerInfo, TableInfo,
};

pub struct MySqlDriver {
    pool: mysql_async::Pool,
    /// 连接目标类型（MySQL / OceanBase MySQL 模式）
    kind: DbKind,
    /// 连接串中的默认库，未显式指定时执行时使用
    default_db: Option<String>,
    /// 手动提交模式（关闭自动提交）
    manual: AtomicBool,
    /// 手动模式下的专用连接（承载未提交事务）
    tx_conn: tokio::sync::Mutex<Option<Conn>>,
}

impl MySqlDriver {
    pub fn new(cfg: &ConnectionConfig) -> DbResult<Self> {
        let mut builder = OptsBuilder::default()
            .ip_or_hostname(cfg.host.clone())
            .tcp_port(cfg.effective_port())
            .user(Some(cfg.username.clone()))
            .pass(Some(cfg.password.clone()))
            .prefer_socket(false);
        // OceanBase（OBProxy/CE）的 TLS 协商常见失败：mysql_async 默认会在服务器
        // 宣告 TLS 能力时尝试升级，这里对 OB 显式禁用 SSL 以保证可连接
        if cfg.kind == DbKind::OceanBase {
            builder = builder.ssl_opts(None);
        }
        let default_db = cfg.database.clone().filter(|d| !d.is_empty());
        if let Some(db) = default_db.clone() {
            builder = builder.db_name(Some(db));
        }
        Ok(Self {
            pool: mysql_async::Pool::new(builder),
            default_db,
            kind: cfg.kind,
            manual: AtomicBool::new(false),
            tx_conn: tokio::sync::Mutex::new(None),
        })
    }

    /// 在指定连接上执行（自动提交 / 手动事务共用）
    async fn execute_on(
        conn: &mut Conn,
        database: Option<&str>,
        sql: &str,
        max_rows: u32,
    ) -> DbResult<QueryResult> {
        let start = std::time::Instant::now();
        if let Some(db) = database.filter(|d| !d.is_empty()) {
            // 库名经方言转义（反引号成对转义），避免特殊字符逃逸
            let d = MySqlFamilyDialect::mysql();
            conn.query_drop(format!("USE {}", d.quote_ident(db))).await?;
        }

        let mut result = conn.query_iter(sql).await?;
        let columns: Vec<ColumnInfo> = result
            .columns_ref()
            .iter()
            .map(|c| {
                let t = c.column_type();
                ColumnInfo {
                    name: c.name_str().into_owned(),
                    type_label: Self::type_label(t),
                    semantic: Self::semantic_of(t, c.character_set()),
                }
            })
            .collect();
        let num_cols = columns.len();

        let mut rows: Vec<Vec<CellValue>> = Vec::new();
        let mut truncated = false;
        // 循环不变量：列语义在迭代前一次性收集
        let sems: Vec<SemanticType> = columns.iter().map(|c| c.semantic).collect();
        while let Some(row) = result.next().await? {
            if (rows.len() as u32) >= max_rows {
                truncated = true;
                break;
            }
            rows.push(
                (0..num_cols)
                    .map(|i| Self::to_cell(&row, i, sems[i]))
                    .collect(),
            );
        }

        // 无结果集 => DML/DDL，取影响行数
        let affected_rows = if columns.is_empty() {
            Some(result.affected_rows())
        } else {
            None
        };

        Ok(QueryResult {
            columns,
            rows,
            affected_rows,
            duration_ms: start.elapsed().as_millis() as u64,
            truncated,
        })
    }

    /// 解析目录查询的 schema 参数（未指定时回退连接默认库）
    fn catalog_schema(&self, schema: Option<&str>) -> DbResult<String> {
        schema
            .filter(|d| !d.is_empty())
            .map(str::to_string)
            .or_else(|| self.default_db.clone())
            .ok_or_else(|| AppError::General("未指定数据库，请在连接配置或顶部下拉中选择".into()))
    }

    fn to_cell(row: &Row, i: usize, semantic: SemanticType) -> CellValue {
        match row.as_ref(i) {
            None | Some(Value::NULL) => CellValue::Null,
            Some(Value::Bytes(b)) => {
                if semantic == SemanticType::Binary {
                    use base64::Engine;
                    CellValue::Binary(base64::engine::general_purpose::STANDARD.encode(b))
                } else {
                    CellValue::Text(String::from_utf8_lossy(b).into_owned())
                }
            }
            Some(Value::Int(v)) => CellValue::Int(*v),
            Some(Value::UInt(v)) => CellValue::UInt(*v),
            Some(Value::Float(v)) => CellValue::Float(*v as f64),
            Some(Value::Double(v)) => CellValue::Float(*v),
            // Date / Time 及其他类型统一转为 SQL 字面量文本
            Some(other) => CellValue::Text(other.as_sql(true)),
        }
    }

    fn type_label(t: mysql_async::consts::ColumnType) -> String {
        use mysql_async::consts::ColumnType::*;
        match t {
            MYSQL_TYPE_DECIMAL | MYSQL_TYPE_NEWDECIMAL => "DECIMAL",
            MYSQL_TYPE_TINY => "TINYINT",
            MYSQL_TYPE_SHORT => "SMALLINT",
            MYSQL_TYPE_LONG => "INT",
            MYSQL_TYPE_INT24 => "MEDIUMINT",
            MYSQL_TYPE_LONGLONG => "BIGINT",
            MYSQL_TYPE_FLOAT => "FLOAT",
            MYSQL_TYPE_DOUBLE => "DOUBLE",
            MYSQL_TYPE_NULL => "NULL",
            MYSQL_TYPE_TIMESTAMP => "TIMESTAMP",
            MYSQL_TYPE_DATE | MYSQL_TYPE_NEWDATE => "DATE",
            MYSQL_TYPE_TIME => "TIME",
            MYSQL_TYPE_DATETIME => "DATETIME",
            MYSQL_TYPE_YEAR => "YEAR",
            MYSQL_TYPE_VARCHAR | MYSQL_TYPE_VAR_STRING => "VARCHAR",
            MYSQL_TYPE_BIT => "BIT",
            MYSQL_TYPE_JSON => "JSON",
            MYSQL_TYPE_ENUM => "ENUM",
            MYSQL_TYPE_SET => "SET",
            MYSQL_TYPE_TINY_BLOB => "TINYBLOB",
            MYSQL_TYPE_MEDIUM_BLOB => "MEDIUMBLOB",
            MYSQL_TYPE_LONG_BLOB => "LONGBLOB",
            MYSQL_TYPE_BLOB => "BLOB",
            MYSQL_TYPE_STRING => "STRING",
            MYSQL_TYPE_GEOMETRY => "GEOMETRY",
            other => return format!("{other:?}"),
        }
        .to_string()
    }

    /// charset 63 = binary 字符集：区分 TEXT 与 BLOB（二者共用 BLOB 类型码）
    fn semantic_of(t: mysql_async::consts::ColumnType, charset: u16) -> SemanticType {
        use mysql_async::consts::ColumnType::*;
        match t {
            MYSQL_TYPE_DECIMAL | MYSQL_TYPE_NEWDECIMAL => SemanticType::Decimal,
            MYSQL_TYPE_TINY
            | MYSQL_TYPE_SHORT
            | MYSQL_TYPE_LONG
            | MYSQL_TYPE_INT24
            | MYSQL_TYPE_LONGLONG
            | MYSQL_TYPE_FLOAT
            | MYSQL_TYPE_DOUBLE
            | MYSQL_TYPE_YEAR => SemanticType::Number,
            MYSQL_TYPE_DATE | MYSQL_TYPE_NEWDATE => SemanticType::Date,
            MYSQL_TYPE_TIME => SemanticType::Time,
            MYSQL_TYPE_DATETIME | MYSQL_TYPE_TIMESTAMP => SemanticType::DateTime,
            MYSQL_TYPE_JSON => SemanticType::Json,
            MYSQL_TYPE_TINY_BLOB | MYSQL_TYPE_MEDIUM_BLOB | MYSQL_TYPE_LONG_BLOB
            | MYSQL_TYPE_BLOB => {
                if charset == 63 {
                    SemanticType::Binary
                } else {
                    SemanticType::LongText
                }
            }
            MYSQL_TYPE_BIT | MYSQL_TYPE_GEOMETRY => SemanticType::Binary,
            MYSQL_TYPE_ENUM | MYSQL_TYPE_SET | MYSQL_TYPE_VARCHAR | MYSQL_TYPE_VAR_STRING
            | MYSQL_TYPE_STRING | MYSQL_TYPE_NULL => SemanticType::Text,
            _ => SemanticType::Unknown,
        }
    }
}

#[async_trait]
impl DatabaseDriver for MySqlDriver {
    async fn server_info(&self) -> DbResult<ServerInfo> {
        let mut conn = self.pool.get_conn().await?;
        let version: String = conn
            .query_first("SELECT VERSION()")
            .await?
            .ok_or_else(|| AppError::Db("无法获取服务器版本".into()))?;
        Ok(ServerInfo {
            kind_label: "MySQL/OceanBase".into(),
            server_version: version,
        })
    }

    async fn list_databases(&self) -> DbResult<Vec<String>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<mysql_async::Row> =
            conn.query("SHOW DATABASES").await?;
        Ok(rows
            .iter()
            .filter_map(|r| r.get::<String, _>(0))
            .collect())
    }

    async fn list_tables(&self, database: Option<&str>) -> DbResult<Vec<TableInfo>> {
        let db = database
            .filter(|d| !d.is_empty())
            .map(str::to_string)
            .or_else(|| self.default_db.clone())
            .ok_or_else(|| AppError::General("未指定数据库，请在连接配置或顶部下拉中选择".into()))?;
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<mysql_async::Row> = conn
            .exec(
                "SELECT table_name, table_type, table_comment \
                 FROM information_schema.tables \
                 WHERE table_schema = ? ORDER BY table_name",
                (db,),
            )
            .await?;
        Ok(rows
            .iter()
            .map(|r| TableInfo {
                name: r.get::<String, _>(0).unwrap_or_default(),
                // BASE TABLE -> TABLE，展示更简洁
                table_type: r
                    .get::<String, _>(1)
                    .unwrap_or_else(|| "TABLE".into())
                    .replace("BASE TABLE", "TABLE"),
                comment: r.get::<Option<String>, _>(2).flatten().filter(|c| !c.is_empty()),
            })
            .collect())
    }

    async fn describe_table(
        &self,
        database: Option<&str>,
        table: &str,
    ) -> DbResult<Vec<ColumnMeta>> {
        let db = database
            .filter(|d| !d.is_empty())
            .map(str::to_string)
            .or_else(|| self.default_db.clone())
            .ok_or_else(|| AppError::General("未指定数据库，请在连接配置或顶部下拉中选择".into()))?;
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<mysql_async::Row> = conn
            .exec(
                "SELECT column_name, column_type, is_nullable, column_default, column_comment, column_key \
                 FROM information_schema.columns \
                 WHERE table_schema = ? AND table_name = ? \
                 ORDER BY ordinal_position",
                (db, table),
            )
            .await?;
        Ok(rows
            .iter()
            .map(|r| ColumnMeta {
                name: r.get::<String, _>(0).unwrap_or_default(),
                data_type: r.get::<String, _>(1).unwrap_or_default(),
                nullable: r
                    .get::<String, _>(2)
                    .map(|v| v.eq_ignore_ascii_case("YES"))
                    .unwrap_or(false),
                default_value: r.get::<Option<String>, _>(3).flatten(),
                comment: r.get::<Option<String>, _>(4).flatten().filter(|c| !c.is_empty()),
                key: r.get::<Option<String>, _>(5).flatten().filter(|k| !k.is_empty()),
            })
            .collect())
    }

    async fn execute(
        &self,
        database: Option<&str>,
        sql: &str,
        max_rows: u32,
    ) -> DbResult<QueryResult> {
        // 手动提交：使用专用连接承载未提交事务
        if self.manual.load(Ordering::Relaxed) {
            let mut guard = self.tx_conn.lock().await;
            if guard.is_none() {
                *guard = Some(self.pool.get_conn().await?);
            }
            let conn = guard.as_mut().expect("tx connection just ensured");
            return Self::execute_on(conn, database, sql, max_rows).await;
        }
        let mut conn = self.pool.get_conn().await?;
        Self::execute_on(&mut conn, database, sql, max_rows).await
    }

    async fn set_autocommit(&self, enabled: bool) -> DbResult<()> {
        let mut guard = self.tx_conn.lock().await;
        if enabled {
            // 结束手动事务：提交未决变更并释放专用连接（池回收时重置会话）
            if let Some(mut conn) = guard.take() {
                let _ = conn.query_drop("COMMIT").await;
            }
            self.manual.store(false, Ordering::Relaxed);
        } else {
            if guard.is_none() {
                let mut conn = self.pool.get_conn().await?;
                conn.query_drop("SET autocommit = 0").await?;
                *guard = Some(conn);
            }
            self.manual.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    async fn commit(&self) -> DbResult<()> {
        let mut guard = self.tx_conn.lock().await;
        if let Some(conn) = guard.as_mut() {
            conn.query_drop("COMMIT").await?;
        }
        Ok(())
    }

    async fn rollback(&self) -> DbResult<()> {
        let mut guard = self.tx_conn.lock().await;
        if let Some(conn) = guard.as_mut() {
            conn.query_drop("ROLLBACK").await?;
        }
        Ok(())
    }

    fn dialect(&self) -> Arc<dyn SqlDialect> {
        Arc::new(MySqlFamilyDialect {
            ob: self.kind == DbKind::OceanBase,
        })
    }

    async fn run_catalog(
        &self,
        obj: DbObjectType,
        q: &CatalogQuery,
        schema: Option<&str>,
    ) -> DbResult<Vec<ObjectInfo>> {
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<Row> = if q.has_param {
            let db = self.catalog_schema(schema)?;
            conn.exec(&*q.sql, (db,)).await?
        } else {
            conn.query(&*q.sql).await?
        };
        Ok(rows
            .iter()
            .map(|r| ObjectInfo {
                name: r.get::<String, _>(q.name_index).unwrap_or_default(),
                object_type: obj,
                comment: q
                    .comment_index
                    .and_then(|i| r.get::<Option<String>, _>(i).flatten())
                    .filter(|c| !c.is_empty()),
                status: q
                    .status_index
                    .and_then(|i| r.get::<Option<String>, _>(i).flatten())
                    .filter(|s| !s.is_empty()),
            })
            .collect())
    }

    async fn run_catalog_count(&self, q: &CatalogQuery, schema: Option<&str>) -> DbResult<u32> {
        let mut conn = self.pool.get_conn().await?;
        let count: u32 = if q.has_param {
            let db = self.catalog_schema(schema)?;
            conn.exec_first(&*q.sql, (db,)).await?.unwrap_or(0)
        } else {
            conn.query_first(&*q.sql).await?.unwrap_or(0)
        };
        Ok(count)
    }

    async fn describe_routine(
        &self,
        database: Option<&str>,
        name: &str,
        routine_type: DbObjectType,
    ) -> DbResult<RoutineInfo> {
        let db = self.catalog_schema(database)?;
        let is_func = routine_type == DbObjectType::Function;
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<Row> = conn
            .exec(
                "SELECT parameter_mode, parameter_name, dtd_identifier \
                 FROM information_schema.parameters \
                 WHERE specific_schema = ? AND specific_name = ? \
                 ORDER BY ordinal_position",
                (db.clone(), name),
            )
            .await?;
        let params = rows
            .iter()
            .map(|r| RoutineParam {
                mode: r
                    .get::<Option<String>, _>(0)
                    .flatten()
                    .unwrap_or_else(|| "IN".into()),
                name: r.get::<Option<String>, _>(1).flatten(),
                data_type: r.get::<Option<String>, _>(2).flatten().unwrap_or_default(),
            })
            .collect();
        let return_type = if is_func {
            conn.exec_first::<String, _, _>(
                "SELECT COALESCE(dtd_identifier, data_type) FROM information_schema.routines \
                 WHERE routine_schema = ? AND specific_name = ? AND routine_type = 'FUNCTION'",
                (db, name),
            )
            .await?
        } else {
            None
        };
        Ok(RoutineInfo {
            name: name.to_string(),
            routine_type: if is_func { "FUNCTION" } else { "PROCEDURE" }.into(),
            return_type,
            params,
        })
    }

    async fn list_indexes(&self, database: Option<&str>, table: &str) -> DbResult<Vec<IndexInfo>> {
        let db = self.catalog_schema(database)?;
        let mut conn = self.pool.get_conn().await?;
        let rows: Vec<Row> = conn
            .exec(
                "SELECT index_name, non_unique, column_name, index_name = 'PRIMARY' \
                 FROM information_schema.statistics \
                 WHERE table_schema = ? AND table_name = ? \
                 ORDER BY index_name, seq_in_index",
                (db, table),
            )
            .await?;
        // 按索引名聚合列（保持查询顺序）
        let mut out: Vec<IndexInfo> = Vec::new();
        for r in &rows {
            let name: String = r.get(0).unwrap_or_default();
            let non_unique: i64 = r.get(1).unwrap_or(1);
            let column: String = r.get(2).unwrap_or_default();
            let primary: i64 = r.get(3).unwrap_or(0);
            match out.last_mut() {
                Some(last) if last.name == name => last.columns.push(column),
                _ => out.push(IndexInfo {
                    name,
                    columns: vec![column],
                    unique: non_unique == 0,
                    primary: primary != 0,
                }),
            }
        }
        Ok(out)
    }

    async fn query_text(&self, sql: &str, params: &[String], value_index: usize) -> DbResult<String> {
        let mut conn = self.pool.get_conn().await?;
        let row: Option<Row> = if params.is_empty() {
            conn.query_first(sql).await?
        } else {
            let binds: Vec<&str> = params.iter().map(|s| s.as_str()).collect();
            conn.exec_first(sql, binds).await?
        };
        Ok(row
            .and_then(|r| r.get::<Option<String>, _>(value_index).flatten())
            .unwrap_or_default())
    }
}

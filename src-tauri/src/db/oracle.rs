//! Oracle 驱动（oracle crate 0.6，需编译时启用 `oracle` 特性）。
//!
//! 依赖 Oracle Instant Client（含 SDK 头文件）：
//! - Windows: 设置 OCI_LIB_DIR 与 PATH，或 OCI_LIB64
//! - Linux/macOS: 设置 OCI_LIB_DIR
//!
//! 连接串格式：`//host:port/service_name`

use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime};

use crate::error::{AppError, DbResult};

use super::dialect::{OracleDialect, SqlDialect};
use super::driver::DatabaseDriver;
use super::objects::{
    aggregate_index_rows, parse_routine_arguments, CatalogQuery, DbObjectType, ObjectInfo,
};
use super::types::{
    CellValue, ColumnInfo, ColumnMeta, ConnectionConfig, DbKind, IndexInfo, QueryResult, RoutineInfo,
    SemanticType, ServerInfo, TableInfo,
};

use oracle::pool::{Pool, PoolBuilder};
use oracle::sql_type::OracleType;
use oracle::SqlValue;

pub struct OracleDriver {
    pool: Pool,
    /// OceanBase Oracle 模式（会话/锁走 OB 兼容视图）
    ob: bool,
}

impl OracleDriver {
    pub fn new(cfg: &ConnectionConfig) -> DbResult<Self> {
        let service = cfg
            .database
            .as_deref()
            .filter(|d| !d.is_empty())
            .or_else(|| cfg.params.get("service_name").map(|s| s.as_str()))
            .ok_or_else(|| AppError::General("Oracle 连接需要指定 database/service_name".into()))?;
        let conn_string = format!("//{}:{}/{}", cfg.host, cfg.effective_port(), service);
        let pool: Pool = PoolBuilder::new(&cfg.username, &cfg.password, &conn_string)
            .max_connections(4)
            .build()?;
        Ok(Self { pool, ob: cfg.kind == DbKind::OceanBaseOracle })
    }

    fn semantic_of(t: &OracleType) -> SemanticType {
        match t {
            OracleType::Number(_, _)
            | OracleType::Float(_)
            | OracleType::BinaryFloat
            | OracleType::BinaryDouble
            | OracleType::Int64
            | OracleType::UInt64 => SemanticType::Number,
            OracleType::CLOB | OracleType::NCLOB | OracleType::Long => SemanticType::LongText,
            OracleType::BLOB | OracleType::BFILE | OracleType::Raw(_) | OracleType::LongRaw => {
                SemanticType::Binary
            }
            OracleType::Date
            | OracleType::Timestamp(_)
            | OracleType::TimestampTZ(_)
            | OracleType::TimestampLTZ(_) => SemanticType::DateTime,
            OracleType::Json => SemanticType::Json,
            OracleType::Varchar2(_)
            | OracleType::NVarchar2(_)
            | OracleType::Char(_)
            | OracleType::NChar(_) => SemanticType::Text,
            _ => SemanticType::Unknown,
        }
    }

    fn cell_from_sql_value(sv: &SqlValue, semantic: SemanticType) -> CellValue {
        if sv.is_null().unwrap_or(true) {
            return CellValue::Null;
        }
        // 二进制列按 base64 传输（前端 HexViewer/图片预览解码）
        if semantic == SemanticType::Binary {
            if let Ok(bytes) = sv.get::<Vec<u8>>() {
                use base64::Engine;
                return CellValue::Binary(base64::engine::general_purpose::STANDARD.encode(bytes));
            }
            return text_or_null(sv);
        }
        let Ok(vt) = sv.oracle_type() else {
            return CellValue::Null;
        };
        match vt {
            OracleType::Number(_, _)
            | OracleType::Float(_)
            | OracleType::BinaryFloat
            | OracleType::BinaryDouble
            | OracleType::Int64 => {
                if let Ok(i) = sv.get::<i64>() {
                    CellValue::Int(i)
                } else if let Ok(f) = sv.get::<f64>() {
                    CellValue::Float(f)
                } else if let Ok(s) = sv.get::<String>() {
                    // 大数保留字符串精度
                    CellValue::Text(s)
                } else {
                    CellValue::Null
                }
            }
            OracleType::Date | OracleType::Timestamp(_) => sv
                .get::<NaiveDateTime>()
                .map(|dt| CellValue::Text(dt.format("%Y-%m-%d %H:%M:%S%.f").to_string()))
                .unwrap_or_else(|_| text_or_null(sv)),
            OracleType::TimestampTZ(_) | OracleType::TimestampLTZ(_) => sv
                .get::<DateTime<chrono::FixedOffset>>()
                .map(|dt| CellValue::Text(dt.format("%Y-%m-%d %H:%M:%S%.f %:z").to_string()))
                .unwrap_or_else(|_| text_or_null(sv)),
            _ => text_or_null(sv),
        }
    }

    fn blocking_execute(
        pool: &Pool,
        schema: Option<&str>,
        sql: &str,
        max_rows: u32,
    ) -> DbResult<QueryResult> {
        let start = std::time::Instant::now();
        let conn = pool.get()?;
        let mut stmt = conn.statement(sql).build()?;
        // Oracle 中 database 参数为 owner/schema；跨 schema 查询需在 SQL 中显式限定
        let _ = schema;

        if stmt.is_query() {
            let mut result_set = stmt.query(&[])?;
            let columns: Vec<ColumnInfo> = result_set
                .column_info()
                .iter()
                .map(|ci| {
                    let ot = ci.oracle_type();
                    ColumnInfo {
                        name: ci.name().to_string(),
                        type_label: ot.to_string(),
                        semantic: Self::semantic_of(ot),
                    }
                })
                .collect();
            let num_cols = columns.len();
            let sems: Vec<SemanticType> = columns.iter().map(|c| c.semantic).collect();

            let mut rows: Vec<Vec<CellValue>> = Vec::new();
            let mut truncated = false;
            for row_result in result_set.by_ref() {
                if (rows.len() as u32) >= max_rows {
                    truncated = true;
                    break;
                }
                let row = row_result?;
                let values = row.sql_values();
                rows.push(
                    (0..num_cols)
                        .filter_map(|i| values.get(i).map(|sv| (i, sv)))
                        .map(|(i, sv)| Self::cell_from_sql_value(sv, sems[i]))
                        .collect(),
                );
            }
            Ok(QueryResult {
                columns,
                rows,
                affected_rows: None,
                duration_ms: start.elapsed().as_millis() as u64,
                truncated,
            })
        } else {
            stmt.execute(&[])?;
            let affected = stmt.row_count()?;
            // Oracle 默认不自动提交，DML/DDL 执行后显式提交（修复变更不生效问题）
            conn.commit()?;
            Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                affected_rows: Some(affected),
                duration_ms: start.elapsed().as_millis() as u64,
                truncated: false,
            })
        }
    }

    /// 单值/目录类查询的行数保护上限（与 bridge_driver 侧 10_000 对齐）
    const MAX_QUERY_ROWS: usize = 10_000;

    fn blocking_query_strings(
        pool: &Pool,
        sql: &str,
        params: &[String],
    ) -> DbResult<Vec<Vec<String>>> {
        let conn = pool.get()?;
        let mut stmt = conn.statement(sql).build()?;
        let result_set = if params.is_empty() {
            stmt.query(&[])?
        } else {
            let bind: Vec<&dyn oracle::sql_type::ToSql> =
                params.iter().map(|p| p as &dyn oracle::sql_type::ToSql).collect();
            stmt.query(&bind)?
        };
        let num_cols = result_set.column_info().len();
        let mut out = Vec::new();
        for row_result in result_set {
            if out.len() >= Self::MAX_QUERY_ROWS {
                break;
            }
            let row = row_result?;
            let values = row.sql_values();
            let mut line = Vec::with_capacity(num_cols);
            for i in 0..num_cols {
                let v: Option<String> = values
                    .get(i)
                    .and_then(|sv| sv.get::<Option<String>>().ok())
                    .flatten();
                line.push(v.unwrap_or_default());
            }
            out.push(line);
        }
        Ok(out)
    }

    /// 目录查询的绑定参数（有占位符时必须有 schema/owner）
    fn catalog_params(q: &CatalogQuery, schema: Option<&str>) -> DbResult<Vec<String>> {
        if !q.has_param {
            return Ok(vec![]);
        }
        Ok(vec![schema
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .ok_or_else(|| AppError::General("未指定 schema/owner".into()))?])
    }
}

fn text_or_null(sv: &SqlValue) -> CellValue {
    match sv.get::<String>() {
        Ok(s) => CellValue::Text(s),
        Err(_) => CellValue::Text("<无法显示的值>".into()),
    }
}

#[async_trait]
impl DatabaseDriver for OracleDriver {
    async fn ping(&self) -> DbResult<()> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            Self::blocking_query_strings(&pool, "SELECT 1 FROM DUAL", &[]).map(|_| ())
        })
        .await
        .map_err(|e| AppError::Db(e.to_string()))?
    }

    async fn server_info(&self) -> DbResult<ServerInfo> {
        let pool = self.pool.clone();
        let rows = tokio::task::spawn_blocking(move || {
            Self::blocking_query_strings(&pool, "SELECT banner FROM v$version WHERE ROWNUM = 1", &[])
        })
        .await
        .map_err(|e| AppError::Db(e.to_string()))??;
        Ok(ServerInfo {
            kind_label: "Oracle".into(),
            server_version: rows.first().and_then(|r| r.first().cloned()).unwrap_or_default(),
        })
    }

    async fn list_databases(&self) -> DbResult<Vec<String>> {
        let pool = self.pool.clone();
        let rows = tokio::task::spawn_blocking(move || {
            Self::blocking_query_strings(
                &pool,
                "SELECT username FROM all_users ORDER BY username",
                &[],
            )
        })
        .await
        .map_err(|e| AppError::Db(e.to_string()))??;
        Ok(rows.into_iter().flatten().collect())
    }

    async fn list_tables(&self, database: Option<&str>) -> DbResult<Vec<TableInfo>> {
        let pool = self.pool.clone();
        let schema = database.filter(|s| !s.is_empty()).map(str::to_string);
        let rows = tokio::task::spawn_blocking(move || {
            match schema.as_deref() {
                Some(owner) => Self::blocking_query_strings(
                    &pool,
                    "SELECT table_name, table_type, comments FROM all_tab_comments WHERE owner = :1 ORDER BY table_name",
                    &[owner.to_string()],
                ),
                None => Self::blocking_query_strings(
                    &pool,
                    "SELECT table_name, table_type, comments FROM user_tab_comments ORDER BY table_name",
                    &[],
                ),
            }
        })
        .await
        .map_err(|e| AppError::Db(e.to_string()))??;
        Ok(rows
            .into_iter()
            .map(|r| TableInfo {
                name: r.first().cloned().unwrap_or_default(),
                table_type: r.get(1).cloned().unwrap_or_else(|| "TABLE".into()),
                comment: r.get(2).cloned().filter(|c| !c.is_empty()),
            })
            .collect())
    }

    async fn describe_table(
        &self,
        database: Option<&str>,
        table: &str,
    ) -> DbResult<Vec<ColumnMeta>> {
        let pool = self.pool.clone();
        let schema = database.filter(|s| !s.is_empty()).map(str::to_string);
        let table = table.to_string();
        let rows = tokio::task::spawn_blocking(move || {
            match schema.as_deref() {
                Some(owner) => Self::blocking_query_strings(
                    &pool,
                    "SELECT column_name, data_type, nullable, data_default \
                     FROM all_tab_columns WHERE owner = :1 AND table_name = :2 \
                     ORDER BY column_id",
                    &[owner.to_string(), table.clone()],
                ),
                None => Self::blocking_query_strings(
                    &pool,
                    "SELECT column_name, data_type, nullable, data_default \
                     FROM user_tab_columns WHERE table_name = :1 \
                     ORDER BY column_id",
                    std::slice::from_ref(&table),
                ),
            }
        })
        .await
        .map_err(|e| AppError::Db(e.to_string()))??;
        Ok(rows
            .into_iter()
            .map(|r| ColumnMeta {
                name: r.first().cloned().unwrap_or_default(),
                data_type: r.get(1).cloned().unwrap_or_default(),
                nullable: r.get(2).map(|s| s == "Y").unwrap_or(false),
                default_value: r.get(3).cloned().filter(|d| !d.is_empty()),
                comment: None,
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
        let pool = self.pool.clone();
        let schema = database.filter(|s| !s.is_empty()).map(str::to_string);
        let sql = sql.to_string();
        let result = tokio::task::spawn_blocking(move || {
            Self::blocking_execute(&pool, schema.as_deref(), &sql, max_rows)
        })
        .await
        .map_err(|e| AppError::Db(e.to_string()))??;
        Ok(result)
    }

    fn dialect(&self) -> std::sync::Arc<dyn SqlDialect> {
        std::sync::Arc::new(if self.ob {
            OracleDialect::oceanbase()
        } else {
            OracleDialect::oracle()
        })
    }

    async fn run_catalog(
        &self,
        obj: DbObjectType,
        q: &CatalogQuery,
        schema: Option<&str>,
    ) -> DbResult<Vec<ObjectInfo>> {
        let pool = self.pool.clone();
        let sql = q.sql.clone();
        let params = Self::catalog_params(q, schema)?;
        let name_idx = q.name_index;
        let comment_idx = q.comment_index;
        let status_idx = q.status_index;
        let rows = tokio::task::spawn_blocking(move || {
            Self::blocking_query_strings(&pool, &sql, &params)
        })
        .await
        .map_err(|e| AppError::Db(e.to_string()))??;
        Ok(rows
            .into_iter()
            .map(|r| ObjectInfo {
                name: r.get(name_idx).cloned().unwrap_or_default(),
                object_type: obj,
                comment: comment_idx
                    .and_then(|i| r.get(i).cloned())
                    .filter(|c| !c.is_empty()),
                status: status_idx
                    .and_then(|i| r.get(i).cloned())
                    .filter(|s| !s.is_empty()),
            })
            .collect())
    }

    async fn run_catalog_count(&self, q: &CatalogQuery, schema: Option<&str>) -> DbResult<u32> {
        let pool = self.pool.clone();
        let sql = q.sql.clone();
        let params = Self::catalog_params(q, schema)?;
        let rows = tokio::task::spawn_blocking(move || {
            Self::blocking_query_strings(&pool, &sql, &params)
        })
        .await
        .map_err(|e| AppError::Db(e.to_string()))??;
        let raw = rows
            .first()
            .and_then(|r| r.first())
            .cloned()
            .unwrap_or_else(|| "0".into());
        Ok(raw.trim().parse().unwrap_or(0))
    }

    async fn query_text(&self, sql: &str, params: &[String], value_index: usize) -> DbResult<String> {
        let pool = self.pool.clone();
        let sql = sql.to_string();
        let params = params.to_vec();
        let rows = tokio::task::spawn_blocking(move || {
            Self::blocking_query_strings(&pool, &sql, &params)
        })
        .await
        .map_err(|e| AppError::Db(e.to_string()))??;
        Ok(rows
            .first()
            .and_then(|r| r.get(value_index))
            .cloned()
            .unwrap_or_default())
    }

    /// Oracle 暂不支持手动事务（OCI 连接非 Send，无法跨调用持有专用连接），
    /// 所有语句执行后自动提交（见 blocking_execute）。
    async fn set_autocommit(&self, enabled: bool) -> DbResult<()> {
        if enabled {
            Ok(())
        } else {
            Err(AppError::General(
                "Oracle 暂不支持手动事务（始终自动提交）".into(),
            ))
        }
    }

    async fn commit(&self) -> DbResult<()> {
        Err(AppError::General("Oracle 暂不支持手动事务".into()))
    }

    async fn rollback(&self) -> DbResult<()> {
        Err(AppError::General("Oracle 暂不支持手动事务".into()))
    }

    async fn describe_routine(
        &self,
        database: Option<&str>,
        name: &str,
        routine_type: DbObjectType,
    ) -> DbResult<RoutineInfo> {
        let pool = self.pool.clone();
        let schema = database.filter(|s| !s.is_empty()).map(str::to_string);
        let name = name.to_string();
        let name_in = name.clone();
        let rows = tokio::task::spawn_blocking(move || match schema.as_deref() {
            Some(owner) => Self::blocking_query_strings(
                &pool,
                "SELECT argument_name, in_out, data_type, position FROM all_arguments \
                 WHERE owner = :1 AND object_name = :2 AND package_name IS NULL ORDER BY position",
                &[owner.to_string(), name_in.clone()],
            ),
            None => Self::blocking_query_strings(
                &pool,
                "SELECT argument_name, in_out, data_type, position FROM user_arguments \
                 WHERE object_name = :1 AND package_name IS NULL ORDER BY position",
                std::slice::from_ref(&name_in),
            ),
        })
        .await
        .map_err(|e| AppError::Db(e.to_string()))??;

        let (params, return_type) = parse_routine_arguments(rows.iter(), |r, i| {
            r.get(i).cloned().filter(|s| !s.is_empty())
        });
        Ok(RoutineInfo {
            name,
            routine_type: if routine_type == DbObjectType::Function {
                "FUNCTION"
            } else {
                "PROCEDURE"
            }
            .into(),
            return_type,
            params,
        })
    }

    async fn list_indexes(&self, database: Option<&str>, table: &str) -> DbResult<Vec<IndexInfo>> {
        let pool = self.pool.clone();
        let schema = database.filter(|s| !s.is_empty()).map(str::to_string);
        let table = table.to_string();
        let table_in = table.clone();
        let rows = tokio::task::spawn_blocking(move || match schema.as_deref() {
            Some(owner) => Self::blocking_query_strings(
                &pool,
                "SELECT i.index_name, i.uniqueness, c.column_name, c.column_position, \
                        (SELECT COUNT(*) FROM all_constraints k \
                         WHERE k.owner = i.owner AND k.index_name = i.index_name \
                           AND k.constraint_type = 'P') \
                 FROM all_ind_columns c \
                 JOIN all_indexes i ON i.index_name = c.index_name AND i.owner = c.index_owner \
                 WHERE i.table_owner = :1 AND i.table_name = :2 \
                 ORDER BY i.index_name, c.column_position",
                &[owner.to_string(), table_in.clone()],
            ),
            None => Self::blocking_query_strings(
                &pool,
                "SELECT i.index_name, i.uniqueness, c.column_name, c.column_position, \
                        (SELECT COUNT(*) FROM user_constraints k \
                         WHERE k.index_name = i.index_name AND k.constraint_type = 'P') \
                 FROM user_ind_columns c \
                 JOIN user_indexes i ON i.index_name = c.index_name \
                 WHERE i.table_name = :1 \
                 ORDER BY i.index_name, c.column_position",
                std::slice::from_ref(&table_in),
            ),
        })
        .await
        .map_err(|e| AppError::Db(e.to_string()))??;

        Ok(aggregate_index_rows(
            rows.iter(),
            |r| {
                let name = r.first().cloned().unwrap_or_default();
                let unique = r
                    .get(1)
                    .map(|u| u.eq_ignore_ascii_case("UNIQUE"))
                    .unwrap_or(false);
                let primary = r
                    .get(4)
                    .and_then(|c| c.trim().parse::<i32>().ok())
                    .unwrap_or(0)
                    > 0;
                (name, unique, primary)
            },
            |r| r.get(2).cloned().unwrap_or_default(),
        ))
    }
}


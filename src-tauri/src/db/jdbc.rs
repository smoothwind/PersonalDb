//! 通用 JDBC 连接：经 jdbc-bridge（Java）以任意 JDBC 驱动连接，
//! 对象目录走 JDBC `DatabaseMetaData`（方言为保守 ANSI 的 `GenericDialect`）。

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::error::{AppError, DbResult};

use super::bridge::{self, Bridge, BridgeKind};
use super::bridge_driver::parse_query_result;
use super::dialect::{self, SqlDialect};
use super::driver::DatabaseDriver;
use super::jdbc_driver::{self, JdbcDriver};
use super::objects::{aggregate_index_rows, CatalogQuery, DbObjectType, ObjectInfo};
use super::types::{
    cell_to_text, CellValue, ColumnMeta, ConnectionConfig, IndexInfo, QueryResult, RoutineInfo,
    ServerInfo, TableInfo,
};

pub struct GenericJdbcDriver {
    cfg: ConnectionConfig,
    driver: JdbcDriver,
    bridge: tokio::sync::Mutex<Option<Bridge>>,
    /// 会话内 schema 切换跟踪（None = 连接默认 schema）
    current_schema: tokio::sync::Mutex<Option<String>>,
    dialect: Arc<dyn SqlDialect>,
}

fn cell_text(row: &[CellValue], idx: usize) -> Option<String> {
    row.get(idx).and_then(cell_to_text)
}

impl GenericJdbcDriver {
    pub fn new(cfg: &ConnectionConfig) -> DbResult<Self> {
        let driver_id = cfg
            .params
            .get("jdbcDriverId")
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AppError::General("JDBC 连接需要选择驱动（驱动管理）".into()))?;
        let driver = jdbc_driver::find_driver(driver_id)?
            .ok_or_else(|| AppError::General(format!("未找到 JDBC 驱动：{driver_id}")))?;
        Ok(Self {
            cfg: cfg.clone(),
            driver,
            bridge: tokio::sync::Mutex::new(None),
            current_schema: tokio::sync::Mutex::new(None),
            dialect: dialect::build(cfg.kind)?,
        })
    }

    fn url(&self) -> String {
        if let Some(u) = self.cfg.params.get("jdbcUrl").filter(|s| !s.is_empty()) {
            return u.clone();
        }
        let port = self
            .cfg
            .port
            .or(self.driver.default_port)
            .unwrap_or_default();
        jdbc_driver::build_url(
            &self.driver,
            &self.cfg.host,
            port,
            self.cfg.database.as_deref().unwrap_or(""),
        )
    }

    async fn with_bridge<T>(&self, f: impl AsyncFnOnce(&mut Bridge) -> DbResult<T>) -> DbResult<T> {
        bridge::with_ensured(
            &self.bridge,
            async || {
                let mut b = bridge::spawn(BridgeKind::ObJdbc, &self.driver.jars).await?;
                b.call(json!({
                    "cmd": "connect",
                    "url": self.url(),
                    "driverClass": self.driver.driver_class,
                    "user": self.cfg.username,
                    "password": self.cfg.password,
                }))
                .await?;
                Ok(b)
            },
            f,
        )
        .await
    }

    async fn bridge_call(&self, req: Value) -> DbResult<Value> {
        self.with_bridge(async move |b| b.call(req).await).await
    }

    async fn query(&self, sql: &str, params: Vec<String>, max_rows: u32) -> DbResult<QueryResult> {
        let data = self
            .bridge_call(json!({
                "cmd": "query", "sql": sql, "params": params, "maxRows": max_rows
            }))
            .await?;
        parse_query_result(&data)
    }

    async fn meta(&self, req: Value) -> DbResult<QueryResult> {
        let data = self.bridge_call(req).await?;
        parse_query_result(&data)
    }

    async fn meta_tables(&self, database: Option<&str>) -> DbResult<QueryResult> {
        let schema = match database.filter(|s| !s.is_empty()) {
            Some(s) => Value::from(s),
            None => Value::Null,
        };
        self.meta(json!({ "cmd": "metaTables", "schema": schema }))
            .await
    }

    /// 会话内 schema 切换语句（按 driver_class 嗅探方言；不认识的驱动返回 None）。
    /// Some(schema) 切换 / None 复位为连接默认。
    fn schema_statement(&self, target: Option<&str>) -> Option<String> {
        let dc = self.driver.driver_class.to_lowercase();
        if dc.contains("oracle") {
            Some(match target {
                Some(s) => format!(
                    "ALTER SESSION SET CURRENT_SCHEMA = \"{}\"",
                    s.replace('"', "\"\"")
                ),
                None => format!(
                    "ALTER SESSION SET CURRENT_SCHEMA = \"{}\"",
                    self.cfg.username.replace('"', "\"\"")
                ),
            })
        } else if dc.contains("mysql") || dc.contains("mariadb") || dc.contains("oceanbase") {
            Some(match target {
                Some(s) => format!("USE `{}`", s.replace('`', "``")),
                None => format!("USE `{}`", self.cfg.username.replace('`', "``")),
            })
        } else if dc.contains("postgresql") || dc.contains("pgjdbc") {
            Some(match target {
                Some(s) => format!("SET search_path TO \"{}\"", s.replace('"', "\"\"")),
                None => "SET search_path TO \"$user\", public".into(),
            })
        } else {
            None
        }
    }

    /// 会话内切换 schema（best-effort：失败仅告警不中断用户查询；
    /// 无论成败都记录尝试值，避免每次查询重试风暴）
    async fn set_current_schema(&self, database: Option<&str>) {
        let target = database
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let mut last = self.current_schema.lock().await;
        if last.as_deref() == target.as_deref() {
            return;
        }
        *last = target;
        let stmt = match last.as_deref() {
            Some(t) => self.schema_statement(Some(t)),
            None if !self.cfg.username.is_empty() => self.schema_statement(None),
            None => None,
        };
        if let Some(stmt) = stmt {
            if let Err(e) = self.query(&stmt, vec![], 1).await {
                log::warn!("JDBC 会话内切换 schema 失败：{e}");
            }
        }
    }
}

#[async_trait]
impl DatabaseDriver for GenericJdbcDriver {
    async fn ping(&self) -> DbResult<()> {
        // 真实探活：先 SELECT 1，Oracle 类后端回退 DUAL 形式；
        // 绕过 execute 避免触发 schema 切换、保持会话内 schema 状态
        if self.query("SELECT 1", vec![], 1).await.is_ok() {
            return Ok(());
        }
        self.query("SELECT 1 FROM DUAL", vec![], 1).await.map(|_| ())
    }

    async fn server_info(&self) -> DbResult<ServerInfo> {
        let data = self.bridge_call(json!({ "cmd": "serverVersion" })).await?;
        Ok(ServerInfo {
            kind_label: "JDBC".into(),
            server_version: data
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or("JDBC")
                .to_string(),
        })
    }

    async fn list_databases(&self) -> DbResult<Vec<String>> {
        let r = self.meta(json!({ "cmd": "metaSchemas" })).await?;
        Ok(r.rows
            .iter()
            .filter_map(|row| cell_text(row, 0))
            .filter(|s| !s.is_empty())
            .collect())
    }

    async fn list_tables(&self, database: Option<&str>) -> DbResult<Vec<TableInfo>> {
        let r = self.meta_tables(database).await?;
        Ok(r.rows
            .iter()
            .map(|row| TableInfo {
                name: cell_text(row, 2).unwrap_or_default(),
                table_type: cell_text(row, 3).unwrap_or_else(|| "TABLE".into()),
                comment: cell_text(row, 4).filter(|s| !s.is_empty()),
            })
            .collect())
    }

    async fn describe_table(
        &self,
        database: Option<&str>,
        table: &str,
    ) -> DbResult<Vec<ColumnMeta>> {
        let schema = match database.filter(|s| !s.is_empty()) {
            Some(s) => Value::from(s),
            None => Value::Null,
        };
        let r = self
            .meta(json!({ "cmd": "metaColumns", "schema": schema, "table": table }))
            .await?;
        Ok(r.rows
            .iter()
            .map(|row| ColumnMeta {
                name: cell_text(row, 0).unwrap_or_default(),
                data_type: cell_text(row, 1).unwrap_or_default(),
                nullable: cell_text(row, 3).map(|n| n != "0").unwrap_or(true),
                default_value: cell_text(row, 4).filter(|s| !s.is_empty()),
                comment: cell_text(row, 5).filter(|s| !s.is_empty()),
                key: None,
            })
            .collect())
    }

    async fn describe_routine(
        &self,
        _database: Option<&str>,
        name: &str,
        routine_type: DbObjectType,
    ) -> DbResult<RoutineInfo> {
        Ok(RoutineInfo {
            name: name.to_string(),
            routine_type: if routine_type == DbObjectType::Function {
                "FUNCTION"
            } else {
                "PROCEDURE"
            }
            .into(),
            return_type: None,
            params: vec![],
        })
    }

    async fn list_indexes(&self, database: Option<&str>, table: &str) -> DbResult<Vec<IndexInfo>> {
        let schema = match database.filter(|s| !s.is_empty()) {
            Some(s) => Value::from(s),
            None => Value::Null,
        };
        let r = self
            .meta(json!({ "cmd": "metaIndexes", "schema": schema, "table": table }))
            .await?;
        // rows: [INDEX_NAME, NON_UNIQUE, COLUMN_NAME, ORDINAL_POSITION, TYPE]
        Ok(aggregate_index_rows(
            r.rows
                .iter()
                .filter(|row| !cell_text(row, 0).unwrap_or_default().is_empty()),
            |row| {
                let name = cell_text(row, 0).unwrap_or_default();
                let unique = cell_text(row, 1)
                    .map(|n| n == "false" || n == "0")
                    .unwrap_or(false);
                (name, unique, false)
            },
            |row| cell_text(row, 2).unwrap_or_default(),
        ))
    }

    async fn execute(
        &self,
        database: Option<&str>,
        sql: &str,
        max_rows: u32,
    ) -> DbResult<QueryResult> {
        // 用户的 SQL 需要跟随 UI 选中的库，切换为会话内可见（best-effort）
        self.set_current_schema(database).await;
        self.query(sql, vec![], max_rows).await
    }

    fn dialect(&self) -> Arc<dyn SqlDialect> {
        self.dialect.clone()
    }

    async fn run_catalog(
        &self,
        obj: DbObjectType,
        _q: &CatalogQuery,
        schema: Option<&str>,
    ) -> DbResult<Vec<ObjectInfo>> {
        let want_view = obj == DbObjectType::View;
        let want_table = obj == DbObjectType::Table;
        if !want_view && !want_table {
            return Ok(vec![]);
        }
        let r = self.meta_tables(schema).await?;
        Ok(r.rows
            .iter()
            .filter_map(|row| {
                let ty = cell_text(row, 3).unwrap_or_default();
                let is_view = ty.to_uppercase().contains("VIEW");
                if (want_view && is_view) || (want_table && !is_view) {
                    Some(ObjectInfo {
                        name: cell_text(row, 2).unwrap_or_default(),
                        object_type: obj,
                        comment: cell_text(row, 4).filter(|s| !s.is_empty()),
                        status: None,
                    })
                } else {
                    None
                }
            })
            .collect())
    }

    async fn run_catalog_count(&self, q: &CatalogQuery, schema: Option<&str>) -> DbResult<u32> {
        let want_view = q.sql.eq_ignore_ascii_case("VIEW");
        let r = self.meta_tables(schema).await?;
        let count = r
            .rows
            .iter()
            .filter(|row| {
                let ty = cell_text(row, 3).unwrap_or_default();
                ty.to_uppercase().contains("VIEW") == want_view
            })
            .count();
        Ok(count as u32)
    }

    async fn query_text(&self, sql: &str, params: &[String], value_index: usize) -> DbResult<String> {
        let r = self.query(sql, params.to_vec(), 1).await?;
        Ok(r.rows
            .first()
            .and_then(|row| cell_text(row, value_index))
            .unwrap_or_default())
    }

    async fn set_autocommit(&self, enabled: bool) -> DbResult<()> {
        self.bridge_call(json!({ "cmd": "autocommit", "autoCommit": enabled }))
            .await
            .map(|_| ())
    }

    async fn commit(&self) -> DbResult<()> {
        self.bridge_call(json!({ "cmd": "commit" })).await.map(|_| ())
    }

    async fn rollback(&self) -> DbResult<()> {
        self.bridge_call(json!({ "cmd": "rollback" })).await.map(|_| ())
    }
}

impl Drop for GenericJdbcDriver {
    fn drop(&mut self) {
        bridge::shutdown_on_drop(&self.bridge);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::pool::create_driver;
    use std::collections::HashMap;

    /// 通用 JDBC 连接真机验证（需 OB_TEST_PASSWORD）
    #[tokio::test]
    #[ignore]
    async fn jdbc_live_connect() {
        let Ok(password) = std::env::var("OB_TEST_PASSWORD") else {
            eprintln!("skip: 未设置 OB_TEST_PASSWORD");
            return;
        };
        let host = std::env::var("OB_TEST_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let mut params = HashMap::new();
        params.insert("jdbcDriverId".to_string(), "builtin-oceanbase".to_string());
        let cfg = ConnectionConfig {
            id: "test".into(),
            name: "test".into(),
            kind: crate::db::types::DbKind::Jdbc,
            host,
            port: Some(2881),
            username: "myuser@mytenant".into(),
            password,
            password_ref: None,
            database: None,
            params,
            group_id: None,
        };
        let driver = create_driver(&cfg).expect("create driver");
        let info = driver.server_info().await.expect("server_info");
        eprintln!("server: {}", info.server_version);
        let schemas = driver.list_databases().await.expect("list_databases");
        eprintln!("schemas: {:?}", &schemas.iter().take(5).collect::<Vec<_>>());
        let tables = driver.list_tables(Some("CS_PLATFORM")).await.expect("tables");
        eprintln!("tables(CS_PLATFORM): {} 个", tables.len());
        assert!(!schemas.is_empty());
    }
}

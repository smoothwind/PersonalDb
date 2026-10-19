//! Oracle / OceanBase-Oracle 的**免 OCI** 传输实现：经 sidecar（oracle-bridge / ob-jdbc-bridge）执行。
//!
//! 方言（SQL 片段/能力/对象目录）完全复用 `dialect` 层的 Oracle 方言，
//! 本模块只负责：桥接进程与连接的惰性建立、SQL 执行与结果映射、事务转发。

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::error::{AppError, DbResult};

use super::bridge::{self, Bridge, BridgeKind};
use super::dialect::{self, SqlDialect};
use super::driver::DatabaseDriver;
use super::objects::{
    aggregate_index_rows, parse_routine_arguments, CatalogQuery, DbObjectType, ObjectInfo,
};
use super::types::{
    cell_to_text, CellValue, ColumnInfo, ColumnMeta, ConnectionConfig, IndexInfo, QueryResult,
    RoutineInfo, SemanticType, ServerInfo, TableInfo,
};

pub struct BridgeDriver {
    cfg: ConnectionConfig,
    kind: BridgeKind,
    /// 惰性建立的桥接进程（内含数据库连接）
    bridge: tokio::sync::Mutex<Option<Bridge>>,
    /// 会话内 CURRENT_SCHEMA 跟踪（None = 连接默认 schema，避免重复下发 ALTER）
    current_schema: tokio::sync::Mutex<Option<String>>,
    dialect: Arc<dyn SqlDialect>,
}

impl BridgeDriver {
    pub fn new(cfg: &ConnectionConfig, kind: BridgeKind) -> DbResult<Self> {
        Ok(Self {
            cfg: cfg.clone(),
            kind,
            bridge: tokio::sync::Mutex::new(None),
            current_schema: tokio::sync::Mutex::new(None),
            dialect: dialect::build(cfg.kind)?,
        })
    }

    /// 连接串中的 service/schema：
    /// - Oracle：必填 service_name（database 或 params.service_name）
    /// - OceanBase Oracle：schema（Oracle 模式 schema = 用户名的默认值，可留空）
    fn service(&self) -> DbResult<String> {
        match self.kind {
            BridgeKind::OracleGo => self
                .cfg
                .database
                .clone()
                .filter(|d| !d.is_empty())
                .or_else(|| self.cfg.params.get("service_name").cloned())
                .ok_or_else(|| {
                    AppError::General("Oracle 连接需要指定 database/service_name".into())
                }),
            BridgeKind::ObJdbc => Ok(self.cfg.database.clone().unwrap_or_default()),
        }
    }

    /// 确保桥接进程与数据库连接就绪
    async fn with_bridge<T>(&self, f: impl AsyncFnOnce(&mut Bridge) -> DbResult<T>) -> DbResult<T> {
        bridge::with_ensured(
            &self.bridge,
            async || {
                let mut b = bridge::spawn(self.kind, &[]).await?;
                b.call(json!({
                    "cmd": "connect",
                    "host": self.cfg.host,
                    "port": self.cfg.effective_port(),
                    "service": self.service()?,
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

    /// 执行查询并映射为 QueryResult
    async fn query(&self, sql: &str, params: Vec<String>, max_rows: u32) -> DbResult<QueryResult> {
        let data = self
            .with_bridge(async |b| {
                b.call(json!({
                    "cmd": "query",
                    "sql": sql,
                    "params": params,
                    "maxRows": max_rows,
                }))
                .await
            })
            .await?;
        parse_query_result(&data)
    }

    /// 查询返回的行（字符串矩阵），供元数据命令使用
    async fn query_rows(&self, sql: &str, params: Vec<String>) -> DbResult<Vec<Vec<Option<String>>>> {
        let result = self.query(sql, params, 10_000).await?;
        Ok(result
            .rows
            .into_iter()
            .map(|row| row.into_iter().map(|c| cell_to_text(&c)).collect())
            .collect())
    }

    /// 会话内切换 CURRENT_SCHEMA（跟踪上次值避免重复下发；None 复位为用户默认 schema）
    async fn set_current_schema(&self, database: Option<&str>) -> DbResult<()> {
        let target = database
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let mut last = self.current_schema.lock().await;
        if last.as_deref() == target.as_deref() {
            return Ok(());
        }
        if let Some(s) = &target {
            let ident = self.dialect.quote_ident(s);
            self.query(
                &format!("ALTER SESSION SET CURRENT_SCHEMA = {ident}"),
                vec![],
                1,
            )
            .await?;
            *last = Some(s.clone());
            return Ok(());
        }
        // 复位为用户默认 schema（OB Oracle 用户名形如 user@tenant，取 user 部分）
        *last = None;
        let user = self
            .cfg
            .username
            .split('@')
            .next()
            .unwrap_or_default()
            .trim();
        if !user.is_empty() {
            let ident = self.dialect.quote_ident(user);
            self.query(
                &format!("ALTER SESSION SET CURRENT_SCHEMA = {ident}"),
                vec![],
                1,
            )
            .await?;
        }
        Ok(())
    }
}

pub(crate) fn json_cell(v: &Value, semantic: SemanticType) -> CellValue {
    match v {
        Value::Null => CellValue::Null,
        Value::Bool(b) => CellValue::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                CellValue::Int(i)
            } else if let Some(u) = n.as_u64() {
                CellValue::UInt(u)
            } else {
                CellValue::Float(n.as_f64().unwrap_or_default())
            }
        }
        Value::String(s) => {
            if semantic == SemanticType::Binary {
                CellValue::Binary(s.clone())
            } else {
                CellValue::Text(s.clone())
            }
        }
        other => CellValue::Text(other.to_string()),
    }
}

pub(crate) fn parse_query_result(data: &Value) -> DbResult<QueryResult> {
    let columns: Vec<ColumnInfo> = data
        .get("columns")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|c| ColumnInfo {
                    name: c.get("name").and_then(Value::as_str).unwrap_or_default().to_string(),
                    type_label: c
                        .get("typeLabel")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    semantic: SemanticType::from_label(
                        c.get("semantic").and_then(Value::as_str).unwrap_or("unknown"),
                        SemanticType::Unknown,
                    ),
                })
                .collect()
        })
        .unwrap_or_default();

    let rows: Vec<Vec<CellValue>> = data
        .get("rows")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|row| {
                    row.as_array()
                        .map(|cells| {
                            cells
                                .iter()
                                .enumerate()
                                .map(|(i, v)| {
                                    let semantic = columns
                                        .get(i)
                                        .map(|c| c.semantic)
                                        .unwrap_or(SemanticType::Unknown);
                                    json_cell(v, semantic)
                                })
                                .collect()
                        })
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(QueryResult {
        columns,
        rows,
        affected_rows: data.get("affectedRows").and_then(Value::as_u64),
        duration_ms: 0,
        truncated: data.get("truncated").and_then(Value::as_bool).unwrap_or(false),
    })
}

#[async_trait]
impl DatabaseDriver for BridgeDriver {
    async fn ping(&self) -> DbResult<()> {
        // 真实探活（sidecar 的 ping 命令仅协议回应，不校验 DB 连接）；
        // 绕过 execute 避免触发 schema 切换、保持会话内 CURRENT_SCHEMA 状态
        self.query("SELECT 1 FROM DUAL", vec![], 1).await.map(|_| ())
    }

    async fn server_info(&self) -> DbResult<ServerInfo> {
        let data = self
            .with_bridge(async |b| b.call(json!({ "cmd": "serverVersion" })).await)
            .await?;
        Ok(ServerInfo {
            kind_label: self.cfg.kind.display_name().to_string(),
            server_version: data
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or("Oracle")
                .to_string(),
        })
    }

    async fn list_databases(&self) -> DbResult<Vec<String>> {
        let rows = self
            .query_rows("SELECT username FROM all_users ORDER BY username", vec![])
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|mut r| r.drain(..).next().flatten())
            .collect())
    }

    async fn list_tables(&self, database: Option<&str>) -> DbResult<Vec<TableInfo>> {
        let objects = self
            .run_catalog(
                DbObjectType::Table,
                &self
                    .dialect
                    .catalog_query(DbObjectType::Table, database)
                    .ok_or_else(|| AppError::General("不支持对象类型 表".into()))?,
                database,
            )
            .await?;
        Ok(objects
            .into_iter()
            .map(|o| TableInfo {
                name: o.name,
                table_type: "TABLE".into(),
                comment: o.comment,
            })
            .collect())
    }

    async fn describe_table(
        &self,
        database: Option<&str>,
        table: &str,
    ) -> DbResult<Vec<ColumnMeta>> {
        let rows = match database.filter(|s| !s.is_empty()) {
            Some(owner) => {
                self.query_rows(
                    "SELECT column_name, data_type, nullable, data_default FROM all_tab_columns \
                     WHERE owner = :1 AND table_name = :2 ORDER BY column_id",
                    vec![owner.to_string(), table.to_string()],
                )
                .await?
            }
            None => {
                self.query_rows(
                    "SELECT column_name, data_type, nullable, data_default FROM user_tab_columns \
                     WHERE table_name = :1 ORDER BY column_id",
                    vec![table.to_string()],
                )
                .await?
            }
        };
        Ok(rows
            .into_iter()
            .map(|r| ColumnMeta {
                name: r.first().cloned().flatten().unwrap_or_default(),
                data_type: r.get(1).cloned().flatten().unwrap_or_default(),
                nullable: r.get(2).cloned().flatten().map(|s| s == "Y").unwrap_or(false),
                default_value: r.get(3).cloned().flatten().filter(|d| !d.is_empty()),
                comment: None,
                key: None,
            })
            .collect())
    }

    async fn describe_routine(
        &self,
        database: Option<&str>,
        name: &str,
        routine_type: DbObjectType,
    ) -> DbResult<RoutineInfo> {
        let rows = match database.filter(|s| !s.is_empty()) {
            Some(owner) => {
                self.query_rows(
                    "SELECT argument_name, in_out, data_type, position FROM all_arguments \
                     WHERE owner = :1 AND object_name = :2 AND package_name IS NULL ORDER BY position",
                    vec![owner.to_string(), name.to_string()],
                )
                .await?
            }
            None => {
                self.query_rows(
                    "SELECT argument_name, in_out, data_type, position FROM user_arguments \
                     WHERE object_name = :1 AND package_name IS NULL ORDER BY position",
                    vec![name.to_string()],
                )
                .await?
            }
        };
        let (params, return_type) = parse_routine_arguments(rows.iter(), |r, i| {
            r.get(i).cloned().flatten().filter(|s| !s.is_empty())
        });
        Ok(RoutineInfo {
            name: name.to_string(),
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
        let rows = match database.filter(|s| !s.is_empty()) {
            Some(owner) => {
                self.query_rows(
                    "SELECT i.index_name, i.uniqueness, c.column_name, \
                       (SELECT COUNT(*) FROM all_constraints k WHERE k.owner = i.owner \
                          AND k.index_name = i.index_name AND k.constraint_type = 'P') \
                     FROM all_ind_columns c \
                     JOIN all_indexes i ON i.index_name = c.index_name AND i.owner = c.index_owner \
                     WHERE i.table_owner = :1 AND i.table_name = :2 \
                     ORDER BY i.index_name, c.column_position",
                    vec![owner.to_string(), table.to_string()],
                )
                .await?
            }
            None => {
                self.query_rows(
                    "SELECT i.index_name, i.uniqueness, c.column_name, \
                       (SELECT COUNT(*) FROM user_constraints k WHERE k.index_name = i.index_name \
                          AND k.constraint_type = 'P') \
                     FROM user_ind_columns c \
                     JOIN user_indexes i ON i.index_name = c.index_name \
                     WHERE i.table_name = :1 \
                     ORDER BY i.index_name, c.column_position",
                    vec![table.to_string()],
                )
                .await?
            }
        };
        Ok(aggregate_index_rows(
            rows.iter(),
            |r| {
                let name = r.first().cloned().flatten().unwrap_or_default();
                let unique = r
                    .get(1)
                    .cloned()
                    .flatten()
                    .map(|u| u.eq_ignore_ascii_case("UNIQUE"))
                    .unwrap_or(false);
                let primary = r
                    .get(3)
                    .cloned()
                    .flatten()
                    .and_then(|c| c.trim().parse::<i32>().ok())
                    .unwrap_or(0)
                    > 0;
                (name, unique, primary)
            },
            |r| r.get(2).cloned().flatten().unwrap_or_default(),
        ))
    }

    async fn execute(
        &self,
        database: Option<&str>,
        sql: &str,
        max_rows: u32,
    ) -> DbResult<QueryResult> {
        // 用户的 SQL 需要跟随 UI 选中的库（schema），切换为会话内可见
        self.set_current_schema(database).await?;
        self.query(sql, vec![], max_rows).await
    }

    fn dialect(&self) -> Arc<dyn SqlDialect> {
        self.dialect.clone()
    }

    async fn run_catalog(
        &self,
        obj: DbObjectType,
        q: &CatalogQuery,
        schema: Option<&str>,
    ) -> DbResult<Vec<ObjectInfo>> {
        let params = if q.has_param {
            vec![schema
                .filter(|s| !s.is_empty())
                .ok_or_else(|| AppError::General("未指定 schema/owner".into()))?
                .to_string()]
        } else {
            vec![]
        };
        let result = self.query(&q.sql, params, 10_000).await?;
        Ok(result
            .rows
            .into_iter()
            .map(|row| {
                let cell = |i: usize| -> Option<String> { row.get(i).and_then(cell_to_text) };
                ObjectInfo {
                    name: cell(q.name_index).unwrap_or_default(),
                    object_type: obj,
                    comment: q.comment_index.and_then(cell).filter(|c| !c.is_empty()),
                    status: q.status_index.and_then(cell).filter(|s| !s.is_empty()),
                }
            })
            .collect())
    }

    async fn run_catalog_count(&self, q: &CatalogQuery, schema: Option<&str>) -> DbResult<u32> {
        let params = if q.has_param {
            vec![schema
                .filter(|s| !s.is_empty())
                .ok_or_else(|| AppError::General("未指定 schema/owner".into()))?
                .to_string()]
        } else {
            vec![]
        };
        let rows = self.query_rows(&q.sql, params).await?;
        Ok(rows
            .first()
            .and_then(|r| r.first())
            .cloned()
            .flatten()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0))
    }

    async fn query_text(&self, sql: &str, params: &[String], value_index: usize) -> DbResult<String> {
        let rows = self.query_rows(sql, params.to_vec()).await?;
        Ok(rows
            .first()
            .and_then(|r| r.get(value_index))
            .cloned()
            .flatten()
            .unwrap_or_default())
    }

    async fn set_autocommit(&self, enabled: bool) -> DbResult<()> {
        self.with_bridge(async |b| {
            b.call(json!({ "cmd": "autocommit", "autoCommit": enabled }))
                .await
                .map(|_| ())
        })
        .await
    }

    async fn commit(&self) -> DbResult<()> {
        self.with_bridge(async |b| b.call(json!({ "cmd": "commit" })).await.map(|_| ()))
            .await
    }

    async fn rollback(&self) -> DbResult<()> {
        self.with_bridge(async |b| b.call(json!({ "cmd": "rollback" })).await.map(|_| ()))
            .await
    }
}

impl Drop for BridgeDriver {
    fn drop(&mut self) {
        bridge::shutdown_on_drop(&self.bridge);
    }
}

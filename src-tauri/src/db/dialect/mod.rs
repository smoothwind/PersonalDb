//! SQL 方言层：与数据库分叉的 SQL 片段集中于此。
//!
//! 新增数据库时实现 `SqlDialect` 并在 `dialect::build` 注册，
//! 连接传输层（transport）协议兼容时可直接复用。

use std::sync::Arc;

use base64::Engine;

use crate::db::objects::{CatalogQuery, DbObjectType};
use crate::db::bridge::BridgeKind;
use crate::db::types::{CellValue, ColumnInfo, ColumnMeta, DbKind, SemanticType};
use crate::error::DbResult;
use crate::db::plan::PlanNode;

/// 方言能力声明（前端据此渲染菜单）
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub explain: bool,
    pub export_ddl: bool,
    pub export_insert: bool,
    /// 是否支持手动事务（自动/手动提交切换）
    pub transactions: bool,
    /// 是否支持服务器会话管理（查看/终止）
    pub sessions: bool,
    /// sql-formatter 语言标识
    pub format_language: &'static str,
    /// 对象浏览器支持的类型
    pub object_types: Vec<DbObjectType>,
}

/// 对象/表 DDL 获取查询
#[derive(Debug, Clone)]
pub struct DdlQuery {
    pub sql: String,
    pub params: Vec<String>,
    /// 取结果行的第几列作为 DDL 文本
    pub value_index: usize,
}

/// 表 DDL 获取策略
#[derive(Debug, Clone)]
pub enum DdlStrategy {
    /// 单条查询取文本（MySQL SHOW CREATE / Oracle DBMS_METADATA）
    Query(DdlQuery),
    /// 用列元数据程序化生成（PostgreSQL 简化版 DDL）
    FromColumns,
}

/// base64 编码的二进制转为大写 HEX（INSERT 字面量用）
pub(crate) fn base64_to_hex(b64: &str) -> String {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .unwrap_or_default();
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02X}"));
    }
    s
}

/// 列定义（表结构编辑用）
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnChange {
    pub name: String,
    pub data_type: String,
    #[serde(default)]
    pub nullable: bool,
    #[serde(default, rename = "defaultValue")]
    pub default_value: Option<String>,
    #[serde(default)]
    pub comment: Option<String>,
}

/// 表结构变更（前端收集 → 方言生成 DDL）
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TableChange {
    AddColumn { column: ColumnChange },
    DropColumn { name: String },
    ModifyColumn { column: ColumnChange },
    AddIndex {
        name: String,
        columns: Vec<String>,
        #[serde(default)]
        unique: bool,
    },
    DropIndex { name: String },
}

pub trait SqlDialect: Send + Sync {
    fn capabilities(&self) -> Capabilities;

    /// 标识符引用：`x` / "x"
    fn quote_ident(&self, ident: &str) -> String;

    /// 字符串字面量转义（含引号策略）
    fn quote_string(&self, s: &str) -> String;

    /// 值 → SQL 字面量（导出 INSERT 用；日期/二进制/布尔按方言函数包装）
    fn literal(&self, v: &CellValue, semantic: SemanticType) -> String;

    /// 值 → SQL 字面量（**会话内执行**的 DML 生成用，默认与 literal 一致）。
    ///
    /// MySQL 系覆盖：服务器默认模式启用反斜杠转义，字符串字面量须转义反斜杠；
    /// 导出文件路径走 `literal` + `NO_BACKSLASH_ESCAPES` 前导，两者语义不同。
    fn dml_literal(&self, v: &CellValue, semantic: SemanticType) -> String {
        self.literal(v, semantic)
    }

    /// 生成一条多值 INSERT 语句（调用方负责分批）
    fn insert_statement(
        &self,
        schema: Option<&str>,
        table: &str,
        cols: &[ColumnInfo],
        rows: &[Vec<CellValue>],
    ) -> String;

    /// 导出 SQL 文件头（会话变量等，保证可重放）
    fn export_preamble(&self) -> &'static str;

    /// 表 DDL 获取策略
    fn table_ddl_strategy(&self, schema: Option<&str>, table: &str) -> DdlStrategy;

    /// 程序化生成表 DDL（FromColumns 策略；简化版，不含外键/索引）
    fn build_ddl_from_columns(
        &self,
        schema: Option<&str>,
        table: &str,
        cols: &[ColumnMeta],
    ) -> String;

    /// 非表对象定义查询（视图/函数/存储过程/触发器/索引/序列），None 不支持
    fn object_ddl_query(
        &self,
        obj: DbObjectType,
        schema: Option<&str>,
        name: &str,
    ) -> Option<DdlQuery>;

    /// 生成"前 N 行"预览语句
    fn preview_rows_sql(&self, schema: Option<&str>, table: &str, limit: u32) -> String;

    /// 对象目录查询（按类型 / schema），None 表示不支持该类型。
    /// schema 为 None 时方言应返回当前用户作用域的查询（如 Oracle USER_ 视图）。
    fn catalog_query(&self, obj: DbObjectType, schema: Option<&str>) -> Option<CatalogQuery>;

    /// 对象数量统计查询（文件夹徽标用）
    fn catalog_count_query(&self, obj: DbObjectType, schema: Option<&str>) -> Option<CatalogQuery>;

    /// 元数据快照 SQL（列级平铺，供 SQL 补全）。None = 不支持（走 JDBC 元数据回退）。
    /// 结果列固定顺序：TABLE_NAME, TABLE_KIND, COLUMN_NAME, COLUMN_TYPE, TABLE_COMMENT, COLUMN_COMMENT
    fn schema_snapshot_sql(&self, _schema: Option<&str>) -> Option<String> {
        None
    }

    /// 判定表是否为分区表（返回单值计数 SQL）；None = 不支持
    fn table_partitioned_query(&self, _schema: Option<&str>, _table: &str) -> Option<String> {
        None
    }

    /// 重新编译对象（Oracle 视图/过程/函数/触发器/索引）；None 表示不支持
    fn recompile_sql(&self, obj: DbObjectType, schema: Option<&str>, name: &str) -> Option<String> {
        let _ = (obj, schema, name);
        None
    }

    /// 服务器会话列表查询。结果列固定顺序：
    /// id, user, host, program, state, secs, extra, sqlText, blocking（阻塞者 id，空串=无）
    /// None = 不支持（capabilities.sessions=false）
    fn server_sessions_sql(&self) -> Option<String> {
        None
    }

    /// 会话查询降级版（如无 v$sql / processlist info 权限时）；列序同上，sqlText 可为空
    fn server_sessions_fallback_sql(&self) -> Option<String> {
        None
    }

    /// 锁/事务视图查询（动态列，前端直接渲染）。None = 不支持。
    fn server_locks_sql(&self) -> Option<String> {
        None
    }

    /// 终止/取消会话语句。id 为会话列表首列原值；
    /// mode: "cancel"（取消当前查询）/ "kill"（断开会话）。None = 不支持或 id 非法。
    fn kill_session_sql(&self, id: &str, mode: &str) -> Option<String> {
        let _ = (id, mode);
        None
    }

    /// 包装语句为执行计划查询（capabilities.explain=false 时调用会返回错误）
    fn explain_sql(&self, sql: &str) -> DbResult<String>;

    /// 解析执行计划输出（JSON 文本）
    fn parse_explain(&self, json_text: &str) -> DbResult<PlanNode>;

    /// 表结构变更 → DDL（返回语句列表，PG 修改列可能多条）
    fn table_change_ddl(
        &self,
        schema: Option<&str>,
        table: &str,
        change: &TableChange,
    ) -> Vec<String> {
        let ident = match schema.filter(|s| !s.is_empty()) {
            Some(s) => format!("{}.{}", self.quote_ident(s), self.quote_ident(table)),
            None => self.quote_ident(table),
        };
        match change {
            TableChange::AddColumn { column } => vec![format!(
                "ALTER TABLE {ident} ADD COLUMN {}",
                col_def(self, column)
            )],
            TableChange::DropColumn { name } => vec![format!(
                "ALTER TABLE {ident} DROP COLUMN {}",
                self.quote_ident(name)
            )],
            TableChange::ModifyColumn { column } => vec![format!(
                "ALTER TABLE {ident} MODIFY COLUMN {}",
                col_def(self, column)
            )],
            TableChange::AddIndex {
                name,
                columns,
                unique,
            } => {
                let cols = columns
                    .iter()
                    .map(|c| self.quote_ident(c))
                    .collect::<Vec<_>>()
                    .join(", ");
                vec![format!(
                    "CREATE {}INDEX {} ON {ident} ({cols})",
                    if *unique { "UNIQUE " } else { "" },
                    self.quote_ident(name)
                )]
            }
            TableChange::DropIndex { name } => vec![format!(
                "DROP INDEX {} ON {ident}",
                self.quote_ident(name)
            )],
        }
    }
}

/// 列定义片段：`"name" type NOT NULL DEFAULT x COMMENT 'c'`（各方言按需覆盖整体生成）
pub(crate) fn col_def<D: SqlDialect + ?Sized>(dialect: &D, col: &ColumnChange) -> String {
    let mut s = format!("{} {}", dialect.quote_ident(&col.name), col.data_type);
    if !col.nullable {
        s.push_str(" NOT NULL");
    }
    if let Some(d) = col.default_value.as_deref().filter(|d| !d.is_empty()) {
        s.push_str(&format!(" DEFAULT {d}"));
    }
    if let Some(c) = col.comment.as_deref().filter(|c| !c.is_empty()) {
        s.push_str(&format!(" COMMENT {}", dialect.quote_string(c)));
    }
    s
}

mod generic;
mod mysql_family;
#[cfg(feature = "oracle")]
mod oracle;
mod postgresql;
mod sqlite;

pub use generic::GenericDialect;
pub use mysql_family::MySqlFamilyDialect;
#[cfg(feature = "oracle")]
pub use oracle::OracleDialect;
pub use postgresql::PostgresDialect;
pub use sqlite::SqliteDialect;

/// 方言工厂（注册表：新增数据库在此追加一条）
pub fn build(kind: DbKind) -> DbResult<Arc<dyn SqlDialect>> {
    match kind {
        DbKind::MySql => Ok(Arc::new(MySqlFamilyDialect::mysql())),
        DbKind::OceanBase => Ok(Arc::new(MySqlFamilyDialect::oceanbase())),
        DbKind::PostgreSql => Ok(Arc::new(PostgresDialect::default())),
        DbKind::GaussDb => Ok(Arc::new(PostgresDialect::gauss())),
        DbKind::Sqlite => Ok(Arc::new(SqliteDialect)),
        DbKind::Jdbc => Ok(Arc::new(GenericDialect)),
        // OceanBase Oracle 兼容模式与 Oracle 共享方言
        DbKind::Oracle | DbKind::OceanBaseOracle => {
            #[cfg(feature = "oracle")]
            {
                Ok(Arc::new(if kind == DbKind::OceanBaseOracle {
                    OracleDialect::oceanbase()
                } else {
                    OracleDialect::oracle()
                }))
            }
            #[cfg(not(feature = "oracle"))]
            {
                Err(crate::error::AppError::UnsupportedKind(if kind == DbKind::OceanBaseOracle {
                    "OceanBase Oracle 模式需编译时启用 --features oracle（依赖 OCI 客户端库）".into()
                } else {
                    "Oracle 支持需编译时启用 --features oracle".into()
                }))
            }
        }
    }
}

/// 编译期不受 feature 影响的方言可用性判断（registry 展示用）
pub fn available(kind: DbKind) -> bool {
    match kind {
        // OCI 直连特性 或 对应内置桥接可用
        DbKind::Oracle => cfg!(feature = "oracle") || crate::db::bridge::available(BridgeKind::OracleGo),
        DbKind::OceanBaseOracle => {
            cfg!(feature = "oracle") || crate::db::bridge::available(BridgeKind::ObJdbc)
        }
        DbKind::Jdbc => crate::db::bridge::available(BridgeKind::ObJdbc),
        _ => true,
    }
}

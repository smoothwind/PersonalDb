use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::DbResult;

/// 支持的数据库类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DbKind {
    /// MySQL 5.7 / 8.x
    MySql,
    /// PostgreSQL 12+
    PostgreSql,
    /// OceanBase（MySQL 兼容模式，默认端口 2881）
    OceanBase,
    /// OceanBase（Oracle 兼容模式，默认端口 2881；走 OCI 协议）
    #[serde(rename = "oceanbase-oracle")]
    OceanBaseOracle,
    /// Oracle 11g+（需编译时启用 oracle 特性）
    Oracle,
    /// SQLite 3（嵌入式文件数据库，连接 = 数据库文件路径）
    Sqlite,
    /// 通用 JDBC（任意驱动/驱动类/URL，经驱动管理配置）
    Jdbc,
}

impl DbKind {
    pub fn default_port(self) -> u16 {
        match self {
            DbKind::MySql => 3306,
            DbKind::PostgreSql => 5432,
            // OB MySQL / Oracle 模式默认端口均为 2881（OBProxy）
            DbKind::OceanBase | DbKind::OceanBaseOracle => 2881,
            DbKind::Oracle => 1521,
            // 通用 JDBC 端口由驱动配置决定；SQLite 为嵌入式无端口
            DbKind::Jdbc | DbKind::Sqlite => 0,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            DbKind::MySql => "MySQL",
            DbKind::PostgreSql => "PostgreSQL",
            DbKind::OceanBase => "OceanBase",
            DbKind::OceanBaseOracle => "OceanBase (Oracle)",
            DbKind::Oracle => "Oracle",
            DbKind::Sqlite => "SQLite",
            DbKind::Jdbc => "JDBC",
        }
    }
}

/// 连接配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionConfig {
    pub id: String,
    pub name: String,
    pub kind: DbKind,
    pub host: String,
    pub port: Option<u16>,
    pub username: String,
    pub password: String,
    /// 凭据引用：密码存于系统凭据管理器时指向其条目（连接 id）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password_ref: Option<String>,
    /// 默认连接的数据库 / 服务名（Oracle 为 service_name）
    #[serde(default)]
    pub database: Option<String>,
    /// 额外参数（如 Oracle 的 service_name、SSL 模式等）
    #[serde(default)]
    pub params: HashMap<String, String>,
    /// 所属连接文件夹（None 为未分组）
    #[serde(default)]
    pub group_id: Option<String>,
}

/// 连接文件夹（支持多级嵌套）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionGroup {
    pub id: String,
    pub name: String,
    /// 上级文件夹（None = 根级）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
}

impl ConnectionConfig {
    pub fn effective_port(&self) -> u16 {
        self.port.unwrap_or_else(|| self.kind.default_port())
    }
}

/// 服务器版本信息
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub kind_label: String,
    pub server_version: String,
}

/// 会话信息（连接成功后返回给前端）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub session_id: String,
    pub config: ConnectionConfig,
    pub server_info: ServerInfo,
}

/// 结果列描述
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub name: String,
    pub type_label: String,
    /// 语义类型（前端据此路由查看组件与对齐方式）
    pub semantic: SemanticType,
}

/// 列的语义类型：按数据库原生类型推断，前端展示路由依据
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SemanticType {
    Text,
    LongText,
    Json,
    Binary,
    Date,
    DateTime,
    Time,
    Number,
    Decimal,
    Bool,
    Uuid,
    Unknown,
}

impl SemanticType {
    /// 语义标签（前端/桥接）→ 枚举；未识别值回退 fallback
    pub fn from_label(s: &str, fallback: SemanticType) -> Self {
        match s {
            "text" => SemanticType::Text,
            "longtext" => SemanticType::LongText,
            "json" => SemanticType::Json,
            "binary" => SemanticType::Binary,
            "date" => SemanticType::Date,
            "datetime" => SemanticType::DateTime,
            "time" => SemanticType::Time,
            "number" => SemanticType::Number,
            "decimal" => SemanticType::Decimal,
            "bool" => SemanticType::Bool,
            "uuid" => SemanticType::Uuid,
            _ => fallback,
        }
    }
}

/// 单元格值：直接映射为 JSON 标量（Binary 为 base64 字符串）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CellValue {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    Text(String),
    Binary(String),
}

/// 目录/元数据场景的单元格文本化（数字转十进制文本，布尔转 1/0，Null → None）
pub fn cell_to_text(v: &CellValue) -> Option<String> {
    match v {
        CellValue::Null => None,
        CellValue::Text(s) | CellValue::Binary(s) => Some(s.clone()),
        CellValue::Int(i) => Some(i.to_string()),
        CellValue::UInt(u) => Some(u.to_string()),
        CellValue::Float(f) => Some(f.to_string()),
        CellValue::Bool(b) => Some(if *b { "1".into() } else { "0".into() }),
    }
}

/// SQL 执行结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Vec<CellValue>>,
    /// DML 语句的影响行数
    pub affected_rows: Option<u64>,
    pub duration_ms: u64,
    /// 因 max_rows 截断
    pub truncated: bool,
}

/// 表 / 视图信息
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableInfo {
    pub name: String,
    /// TABLE / VIEW
    pub table_type: String,
    #[serde(default)]
    pub comment: Option<String>,
}

/// 列元数据
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMeta {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    #[serde(default)]
    pub default_value: Option<String>,
    #[serde(default)]
    pub comment: Option<String>,
    /// PRI / UNI / MUL（MySQL），其他数据库为空
    #[serde(default)]
    pub key: Option<String>,
}

/// 例程（存储过程 / 函数）参数
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutineParam {
    /// 参数名（Oracle 返回值参数可为空）
    #[serde(default)]
    pub name: Option<String>,
    /// IN / OUT / INOUT / RETURN
    pub mode: String,
    pub data_type: String,
}

/// 例程（存储过程 / 函数）元数据
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutineInfo {
    pub name: String,
    /// PROCEDURE | FUNCTION
    pub routine_type: String,
    /// 函数返回类型（过程为 None）
    #[serde(default)]
    pub return_type: Option<String>,
    pub params: Vec<RoutineParam>,
}

/// 索引元数据
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub primary: bool,
}

pub fn new_session_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[allow(dead_code)]
pub fn ensure_non_empty(v: &str, field: &str) -> DbResult<()> {
    if v.trim().is_empty() {
        Err(crate::error::AppError::General(format!("{field} 不能为空")))
    } else {
        Ok(())
    }
}

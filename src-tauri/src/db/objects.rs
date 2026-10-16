//! 数据库对象类型与元数据结构（P1 对象浏览器）。

use serde::{Deserialize, Serialize};

/// 浏览器中的对象类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DbObjectType {
    Table,
    View,
    Index,
    Sequence,
    Function,
    Procedure,
    Trigger,
}

impl DbObjectType {
    pub fn display_name(self) -> &'static str {
        match self {
            DbObjectType::Table => "表",
            DbObjectType::View => "视图",
            DbObjectType::Index => "索引",
            DbObjectType::Sequence => "序列",
            DbObjectType::Function => "函数",
            DbObjectType::Procedure => "存储过程",
            DbObjectType::Trigger => "触发器",
        }
    }
}

/// 对象实例信息
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectInfo {
    pub name: String,
    pub object_type: DbObjectType,
    #[serde(default)]
    pub comment: Option<String>,
    /// 对象状态（Oracle：VALID / INVALID / UNUSABLE；其他数据库为 None）
    #[serde(default)]
    pub status: Option<String>,
}

/// 对象类型及其数量（库/文件夹层级展示）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectFolder {
    pub object_type: DbObjectType,
    pub count: u32,
}

/// 元数据快照（SQL 补全用）：表/视图 + 列 + 注释
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaSnapshot {
    pub tables: Vec<TableMeta>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableMeta {
    pub name: String,
    /// TABLE / VIEW
    pub kind: String,
    #[serde(default)]
    pub comment: Option<String>,
    pub columns: Vec<ColumnSnap>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnSnap {
    pub name: String,
    #[serde(default)]
    pub data_type: String,
    #[serde(default)]
    pub comment: Option<String>,
}

/// 方言生成的对象目录查询：
/// `has_param` 为 true 时 SQL 含一个 schema/owner 参数占位符（风格由方言决定）。
#[derive(Debug, Clone)]
pub struct CatalogQuery {
    pub sql: String,
    pub has_param: bool,
    /// 名称所在列索引
    pub name_index: usize,
    /// 备注所在列索引（None 表示查询不含备注列）
    pub comment_index: Option<usize>,
    /// 状态所在列索引（None 表示该方言/对象无状态概念）
    pub status_index: Option<usize>,
}

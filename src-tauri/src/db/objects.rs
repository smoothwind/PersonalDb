//! 数据库对象类型与元数据结构（P1 对象浏览器）。

use serde::{Deserialize, Serialize};

use super::types::{IndexInfo, RoutineParam};

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

/// 将按索引名有序的行聚合为 IndexInfo（相邻同名归并）。
/// meta 返回 (索引名, 是否唯一, 是否主键)，column 返回列名。
pub fn aggregate_index_rows<T>(
    rows: impl IntoIterator<Item = T>,
    mut meta: impl FnMut(&T) -> (String, bool, bool),
    mut column: impl FnMut(&T) -> String,
) -> Vec<IndexInfo> {
    let mut out: Vec<IndexInfo> = Vec::new();
    for r in rows {
        let (name, unique, primary) = meta(&r);
        let col = column(&r);
        match out.last_mut() {
            Some(last) if last.name == name => last.columns.push(col),
            _ => out.push(IndexInfo { name, columns: vec![col], unique, primary }),
        }
    }
    out
}

/// Oracle 系 arguments 行 → (参数列表, 函数返回类型)。
/// 列序：0=argument_name 1=in_out 2=data_type 3=position；
/// 无名参数且 position<=0 视为函数返回值（存储过程无返回值行，return_type 为 None）。
pub fn parse_routine_arguments<T>(
    rows: impl IntoIterator<Item = T>,
    mut col: impl FnMut(&T, usize) -> Option<String>,
) -> (Vec<RoutineParam>, Option<String>) {
    let mut params: Vec<RoutineParam> = Vec::new();
    let mut return_type: Option<String> = None;
    for r in rows {
        let arg_name = col(&r, 0);
        if arg_name.is_none() {
            // 无名参数：position 0 为函数返回值（负数防御无效行）
            let position = col(&r, 3)
                .and_then(|p| p.trim().parse::<i32>().ok())
                .unwrap_or(-1);
            if position <= 0 {
                return_type = col(&r, 2);
            }
        } else {
            params.push(RoutineParam {
                name: arg_name,
                mode: col(&r, 1).filter(|m| !m.is_empty()).unwrap_or_else(|| "IN".into()),
                data_type: col(&r, 2).unwrap_or_default(),
            });
        }
    }
    (params, return_type)
}

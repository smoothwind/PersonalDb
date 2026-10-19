//! 结果集编辑 DML 生成：由前端收集的增/改/删 + 定位列，按方言生成 SQL 语句。
//!
//! 定位（WHERE）策略由前端决定并传入：主键列 / ROWID 列 / 全列匹配（高危）。

use serde::Deserialize;

use crate::error::{AppError, DbResult};

use super::dialect::build;
use super::types::{CellValue, DbKind, SemanticType};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DmlColumn {
    pub name: String,
    #[serde(default)]
    pub semantic: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DmlUpdate {
    /// 原行（与 columns 对齐）
    pub old: Vec<CellValue>,
    /// 新行（与 columns 对齐）
    pub new: Vec<CellValue>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DmlRequest {
    pub kind: DbKind,
    pub schema: Option<String>,
    pub table: String,
    pub columns: Vec<DmlColumn>,
    /// 定位列下标（主键 / ROWID / 全列）
    pub where_indices: Vec<usize>,
    #[serde(default)]
    pub inserts: Vec<Vec<CellValue>>,
    #[serde(default)]
    pub updates: Vec<DmlUpdate>,
    #[serde(default)]
    pub deletes: Vec<Vec<CellValue>>,
}

fn parse_sem(s: &str) -> SemanticType {
    match s {
        "date" => SemanticType::Date,
        "datetime" => SemanticType::DateTime,
        "time" => SemanticType::Time,
        "binary" => SemanticType::Binary,
        "json" => SemanticType::Json,
        "longtext" => SemanticType::LongText,
        "number" => SemanticType::Number,
        "decimal" => SemanticType::Decimal,
        "bool" => SemanticType::Bool,
        "uuid" => SemanticType::Uuid,
        _ => SemanticType::Text,
    }
}

/// 生成 DML 语句列表（INSERT / UPDATE / DELETE）
pub fn build_dml(req: &DmlRequest) -> DbResult<Vec<String>> {
    let dialect = build(req.kind)?;
    let ident = |n: &str| dialect.quote_ident(n);
    let table = match req.schema.as_deref().filter(|s| !s.is_empty()) {
        Some(s) => format!("{}.{}", ident(s), ident(&req.table)),
        None => ident(&req.table),
    };
    let sem = |i: usize| -> SemanticType {
        req.columns
            .get(i)
            .and_then(|c| c.semantic.as_deref())
            .map(parse_sem)
            .unwrap_or(SemanticType::Text)
    };
    let lit = |v: &CellValue, i: usize| dialect.dml_literal(v, sem(i));
    let ncol = req.columns.len();

    let mut out: Vec<String> = Vec::new();

    // INSERT
    let col_list = req
        .columns
        .iter()
        .map(|c| ident(&c.name))
        .collect::<Vec<_>>()
        .join(", ");
    for row in &req.inserts {
        if row.len() != ncol {
            return Err(AppError::General("新增行数据与列数量不一致".into()));
        }
        let vals = row
            .iter()
            .enumerate()
            .map(|(i, v)| lit(v, i))
            .collect::<Vec<_>>()
            .join(", ");
        out.push(format!(
            "INSERT INTO {table} ({col_list}) VALUES ({vals});"
        ));
    }

    // UPDATE
    for u in &req.updates {
        if u.old.len() != ncol || u.new.len() != ncol {
            return Err(AppError::General("更新行数据与列数量不一致".into()));
        }
        let changed: Vec<usize> = (0..ncol).filter(|&i| u.old[i] != u.new[i]).collect();
        if changed.is_empty() {
            continue;
        }
        let set = changed
            .iter()
            .map(|&i| format!("{} = {}", ident(&req.columns[i].name), lit(&u.new[i], i)))
            .collect::<Vec<_>>()
            .join(", ");
        let where_sql = where_clause(req, &u.old, &ident, &lit)?;
        out.push(format!("UPDATE {table} SET {set} WHERE {where_sql};"));
    }

    // DELETE
    for row in &req.deletes {
        if row.len() != ncol {
            return Err(AppError::General("删除行数据与列数量不一致".into()));
        }
        let where_sql = where_clause(req, row, &ident, &lit)?;
        out.push(format!("DELETE FROM {table} WHERE {where_sql};"));
    }

    Ok(out)
}

fn where_clause(
    req: &DmlRequest,
    row: &[CellValue],
    ident: &dyn Fn(&str) -> String,
    lit: &dyn Fn(&CellValue, usize) -> String,
) -> DbResult<String> {
    if req.where_indices.is_empty() {
        return Err(AppError::General(
            "缺少定位列（主键 / ROWID / 全列匹配）".into(),
        ));
    }
    Ok(req
        .where_indices
        .iter()
        .map(|&i| {
            let name = ident(&req.columns[i].name);
            match row.get(i) {
                None | Some(CellValue::Null) => format!("{name} IS NULL"),
                Some(v) => format!("{name} = {}", lit(v, i)),
            }
        })
        .collect::<Vec<_>>()
        .join(" AND "))
}

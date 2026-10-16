//! 通用 JDBC 方言：面向未知数据库的保守 ANSI 实现。
//!
//! 对象目录不经 SQL（`catalog_query` 仅作占位），由传输层走 JDBC `DatabaseMetaData`。

use crate::db::objects::{CatalogQuery, DbObjectType};
use crate::db::plan::PlanNode;
use crate::db::types::{CellValue, ColumnInfo, ColumnMeta, SemanticType};
use crate::error::{AppError, DbResult};

use super::{base64_to_hex, Capabilities, DdlQuery, DdlStrategy, SqlDialect};

pub struct GenericDialect;

impl SqlDialect for GenericDialect {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            explain: false,
            export_ddl: false,
            export_insert: true,
            transactions: true,
            sessions: false,
            format_language: "sql",
            object_types: vec![DbObjectType::Table, DbObjectType::View],
        }
    }

    fn quote_ident(&self, ident: &str) -> String {
        format!("\"{}\"", ident.replace('"', "\"\""))
    }

    fn quote_string(&self, s: &str) -> String {
        format!("'{}'", s.replace('\'', "''"))
    }

    fn literal(&self, v: &CellValue, semantic: SemanticType) -> String {
        match v {
            CellValue::Null => "NULL".into(),
            CellValue::Bool(b) => if *b { "TRUE" } else { "FALSE" }.into(),
            CellValue::Int(i) => i.to_string(),
            CellValue::UInt(u) => u.to_string(),
            CellValue::Float(f) => f.to_string(),
            CellValue::Binary(b64) => format!("X'{}'", base64_to_hex(b64)),
            CellValue::Text(s) => match semantic {
                SemanticType::Binary => "NULL".into(),
                _ => self.quote_string(s),
            },
        }
    }

    fn insert_statement(
        &self,
        schema: Option<&str>,
        table: &str,
        cols: &[ColumnInfo],
        rows: &[Vec<CellValue>],
    ) -> String {
        let ident = match schema.filter(|s| !s.is_empty()) {
            Some(s) => format!("{}.{}", self.quote_ident(s), self.quote_ident(table)),
            None => self.quote_ident(table),
        };
        let col_list = cols
            .iter()
            .map(|c| self.quote_ident(&c.name))
            .collect::<Vec<_>>()
            .join(", ");
        let values = rows
            .iter()
            .map(|row| {
                let cells = row
                    .iter()
                    .zip(cols.iter())
                    .map(|(v, c)| self.literal(v, c.semantic))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({cells})")
            })
            .collect::<Vec<_>>()
            .join(",\n  ");
        format!("INSERT INTO {ident} ({col_list}) VALUES\n  {values};")
    }

    fn export_preamble(&self) -> &'static str {
        ""
    }

    fn table_ddl_strategy(&self, _schema: Option<&str>, _table: &str) -> DdlStrategy {
        DdlStrategy::FromColumns
    }

    fn build_ddl_from_columns(
        &self,
        schema: Option<&str>,
        table: &str,
        cols: &[ColumnMeta],
    ) -> String {
        let ident = match schema.filter(|s| !s.is_empty()) {
            Some(s) => format!("{}.{}", self.quote_ident(s), self.quote_ident(table)),
            None => self.quote_ident(table),
        };
        let lines: Vec<String> = cols
            .iter()
            .map(|c| {
                let mut line = format!("  {} {}", self.quote_ident(&c.name), c.data_type);
                if !c.nullable {
                    line.push_str(" NOT NULL");
                }
                if let Some(def) = c.default_value.as_deref().filter(|d| !d.is_empty()) {
                    line.push_str(&format!(" DEFAULT {def}"));
                }
                line
            })
            .collect();
        format!("CREATE TABLE {ident} (\n{}\n);", lines.join(",\n"))
    }

    fn object_ddl_query(
        &self,
        _obj: DbObjectType,
        _schema: Option<&str>,
        _name: &str,
    ) -> Option<DdlQuery> {
        None
    }

    fn preview_rows_sql(&self, schema: Option<&str>, table: &str, _limit: u32) -> String {
        // 通用场景不做 LIMIT（交由传输层 max_rows 截断）
        let ident = match schema.filter(|s| !s.is_empty()) {
            Some(s) => format!("{}.{}", self.quote_ident(s), self.quote_ident(table)),
            None => self.quote_ident(table),
        };
        format!("SELECT * FROM {ident}")
    }

    fn catalog_query(&self, obj: DbObjectType, _schema: Option<&str>) -> Option<CatalogQuery> {
        // sql 作为类型标记（"TABLE"/"VIEW"），实际由 JDBC DatabaseMetaData 提供
        let marker = match obj {
            DbObjectType::Table => "TABLE",
            DbObjectType::View => "VIEW",
            _ => return None,
        };
        Some(CatalogQuery {
            sql: marker.into(),
            has_param: false,
            name_index: 2,
            comment_index: Some(4),
            status_index: None,
        })
    }

    fn catalog_count_query(&self, obj: DbObjectType, _schema: Option<&str>) -> Option<CatalogQuery> {
        self.catalog_query(obj, None)
    }

    fn explain_sql(&self, _sql: &str) -> DbResult<String> {
        Err(AppError::General("通用 JDBC 连接不支持执行计划".into()))
    }

    fn parse_explain(&self, _json_text: &str) -> DbResult<PlanNode> {
        Err(AppError::General("通用 JDBC 连接不支持执行计划".into()))
    }
}

//! SQLite 方言。标识符 `"x"`；无 schema 概念（attached 库不在此管理）。
//!
//! 目录查询基于 `sqlite_master` + PRAGMA 表值函数（bundled SQLite ≥ 3.35）。

use crate::db::objects::{CatalogQuery, DbObjectType};
use crate::db::types::{CellValue, ColumnInfo, ColumnMeta, SemanticType};

use super::{base64_to_hex, Capabilities, DdlQuery, DdlStrategy, SqlDialect, TableChange};

pub struct SqliteDialect;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_and_literals() {
        let d = SqliteDialect;
        assert_eq!(d.quote_ident("user"), "\"user\"");
        assert_eq!(d.quote_ident("we\"ird"), "\"we\"\"ird\"");
        assert_eq!(d.quote_string("it's"), "'it''s'");
        assert_eq!(d.literal(&CellValue::Bool(true), SemanticType::Bool), "1");
        assert_eq!(
            d.literal(&CellValue::Binary("AAEC".into()), SemanticType::Binary),
            "X'000102'"
        );
        assert_eq!(d.literal(&CellValue::Null, SemanticType::Text), "NULL");
    }

    #[test]
    fn insert_and_preview() {
        let d = SqliteDialect;
        let cols = vec![ColumnInfo {
            name: "id".into(),
            type_label: "INTEGER".into(),
            semantic: SemanticType::Number,
        }];
        let sql = d.insert_statement(None, "t", &cols, &[vec![CellValue::Int(1)]]);
        assert_eq!(sql, "INSERT INTO \"t\" (\"id\") VALUES\n  (1);");
        assert_eq!(d.preview_rows_sql(None, "t", 100), "SELECT * FROM \"t\" LIMIT 100");
    }

    #[test]
    fn catalog_excludes_internal() {
        let d = SqliteDialect;
        let q = d.catalog_query(DbObjectType::Table, None).unwrap();
        assert!(q.sql.contains("sqlite_master"));
        assert!(!q.has_param);
        // 内部对象（sqlite_sequence / sqlite_autoindex_*）经 ESCAPE 过滤
        assert!(q.sql.contains("sqlite\\_%"));
    }
}

impl SqliteDialect {
    /// `sqlite_master.type` 过滤值（对象类型 → master 类型）
    fn master_type(obj: DbObjectType) -> Option<&'static str> {
        match obj {
            DbObjectType::Table => Some("'table'"),
            DbObjectType::View => Some("'view'"),
            DbObjectType::Index => Some("'index'"),
            DbObjectType::Trigger => Some("'trigger'"),
            _ => None,
        }
    }
}

impl SqlDialect for SqliteDialect {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            explain: true, // EXPLAIN QUERY PLAN（行式，驱动侧结构化）
            export_ddl: true,
            export_insert: true,
            transactions: true,
            sessions: false, // 嵌入式引擎，无服务器会话概念
            format_language: "sql",
            object_types: vec![
                DbObjectType::Table,
                DbObjectType::View,
                DbObjectType::Index,
                DbObjectType::Trigger,
            ],
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
            // SQLite 布尔即整数 1/0
            CellValue::Bool(b) => if *b { "1" } else { "0" }.into(),
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
        _schema: Option<&str>,
        table: &str,
        cols: &[ColumnInfo],
        rows: &[Vec<CellValue>],
    ) -> String {
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
        format!("INSERT INTO {} ({col_list}) VALUES\n  {values};", self.quote_ident(table))
    }

    fn export_preamble(&self) -> &'static str {
        "PRAGMA foreign_keys=OFF;"
    }

    /// SQLite 的 `sqlite_master.sql` 即原始建表语句（含主键/索引内联）
    fn table_ddl_strategy(&self, _schema: Option<&str>, table: &str) -> DdlStrategy {
        DdlStrategy::Query(DdlQuery {
            sql: "SELECT COALESCE(sql, '') FROM sqlite_master \
                  WHERE type = 'table' AND name = ?"
                .into(),
            params: vec![table.to_string()],
            value_index: 0,
        })
    }

    /// 兜底 DDL（sqlite_master 无记录时用列元数据程序化生成）
    fn build_ddl_from_columns(
        &self,
        _schema: Option<&str>,
        table: &str,
        cols: &[ColumnMeta],
    ) -> String {
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
                if c.key.as_deref() == Some("PRI") {
                    line.push_str(" PRIMARY KEY");
                }
                line
            })
            .collect();
        format!(
            "CREATE TABLE {} (\n{}\n);",
            self.quote_ident(table),
            lines.join(",\n")
        )
    }

    fn object_ddl_query(
        &self,
        obj: DbObjectType,
        _schema: Option<&str>,
        name: &str,
    ) -> Option<DdlQuery> {
        let t = Self::master_type(obj)?;
        Some(DdlQuery {
            sql: format!(
                "SELECT COALESCE(sql, '') FROM sqlite_master WHERE type = {t} AND name = ?"
            ),
            params: vec![name.to_string()],
            value_index: 0,
        })
    }

    fn preview_rows_sql(&self, _schema: Option<&str>, table: &str, limit: u32) -> String {
        format!("SELECT * FROM {} LIMIT {limit}", self.quote_ident(table))
    }

    fn catalog_query(&self, obj: DbObjectType, _schema: Option<&str>) -> Option<CatalogQuery> {
        let t = Self::master_type(obj)?;
        // sqlite_autoindex_* 为约束自动索引；sqlite_sequence 为 WITHOUT ROWID 辅助表
        Some(CatalogQuery {
            sql: format!(
                "SELECT name, NULL FROM sqlite_master \
                 WHERE type = {t} AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\' ORDER BY 1"
            ),
            has_param: false,
            name_index: 0,
            comment_index: None,
            status_index: None,
        })
    }

    fn catalog_count_query(&self, obj: DbObjectType, _schema: Option<&str>) -> Option<CatalogQuery> {
        let t = Self::master_type(obj)?;
        Some(CatalogQuery {
            sql: format!(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = {t} AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\'"
            ),
            has_param: false,
            name_index: 0,
            comment_index: None,
            status_index: None,
        })
    }

    /// 补全快照：sqlite_master × pragma_table_info（表值函数，bundled ≥ 3.16 可用）
    fn schema_snapshot_sql(&self, _schema: Option<&str>) -> Option<String> {
        Some(
            "SELECT m.name, UPPER(m.type), c.name, c.type, \
                    NULL, NULL \
             FROM sqlite_master m \
             JOIN pragma_table_info(m.name) c \
             WHERE m.type IN ('table', 'view') \
               AND m.name NOT LIKE 'sqlite\\_%' ESCAPE '\\' \
             ORDER BY m.name, c.cid"
                .into(),
        )
    }

    fn explain_sql(&self, sql: &str) -> crate::error::DbResult<String> {
        let trimmed = sql.trim().trim_end_matches(';').to_string();
        Ok(format!("EXPLAIN QUERY PLAN {trimmed}"))
    }

    /// SQLite 计划为行式输出（多行多列），不走 JSON 文本解析；
    /// 由 `db_explain` 命令特判后调用 `plan::sqlite_plan_from_rows`。
    fn parse_explain(&self, _json_text: &str) -> crate::error::DbResult<crate::db::plan::PlanNode> {
        Err(crate::error::AppError::General(
            "SQLite 执行计划经行式结果构建，不应到达 JSON 解析".into(),
        ))
    }

    /// SQLite 差异：改列走「建新表-拷数-改名-重建索引」成本高，仅放行加列/删列/索引（现代 SQLite 均原生支持）
    fn table_change_ddl(
        &self,
        _schema: Option<&str>,
        table: &str,
        change: &TableChange,
    ) -> Vec<String> {
        let ident = self.quote_ident(table);
        match change {
            TableChange::AddColumn { column } => {
                // SQLite ADD COLUMN 不支持 COMMENT 子句；注释需走表重建，此处省略
                let mut s = format!(
                    "ALTER TABLE {ident} ADD COLUMN {} {}",
                    self.quote_ident(&column.name),
                    column.data_type
                );
                if !column.nullable {
                    s.push_str(" NOT NULL");
                }
                if let Some(d) = column.default_value.as_deref().filter(|d| !d.is_empty()) {
                    s.push_str(&format!(" DEFAULT {d}"));
                }
                vec![s]
            }
            TableChange::DropColumn { name } => vec![format!(
                "ALTER TABLE {ident} DROP COLUMN {}",
                self.quote_ident(name)
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
            // SQLite 的 DROP INDEX 不带 ON 子句
            TableChange::DropIndex { name } => {
                vec![format!("DROP INDEX {}", self.quote_ident(name))]
            }
            // 改列类型需整表重建，不做有损自动生成
            TableChange::ModifyColumn { .. } => vec![],
        }
    }
}

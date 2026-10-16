//! PostgreSQL 方言。参数占位符 `$1`（schema，默认 public）。

use crate::db::objects::{CatalogQuery, DbObjectType};
use crate::db::types::{CellValue, ColumnInfo, ColumnMeta, SemanticType};

use super::{base64_to_hex, Capabilities, ColumnChange, DdlQuery, DdlStrategy, SqlDialect, TableChange};

pub struct PostgresDialect;

#[cfg(test)]
mod kill_tests {
    use super::*;

    #[test]
    fn kill_sql_injection_guard() {
        let d = PostgresDialect;
        assert_eq!(
            d.kill_session_sql("99", "cancel"),
            Some("SELECT pg_cancel_backend(99)".into())
        );
        assert_eq!(
            d.kill_session_sql("99", "kill"),
            Some("SELECT pg_terminate_backend(99)".into())
        );
        assert_eq!(d.kill_session_sql("99;DROP TABLE t", "kill"), None);
        assert_eq!(d.kill_session_sql("-1", "kill"), None);
        assert_eq!(d.kill_session_sql("abc", "kill"), None);
    }
}

impl PostgresDialect {
    fn object_types() -> Vec<DbObjectType> {
        vec![
            DbObjectType::Table,
            DbObjectType::View,
            DbObjectType::Index,
            DbObjectType::Sequence,
            DbObjectType::Function,
            DbObjectType::Procedure,
            DbObjectType::Trigger,
        ]
    }
}

impl SqlDialect for PostgresDialect {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            explain: true,    // EXPLAIN (FORMAT JSON)
            export_ddl: true, // information_schema 自拼
            export_insert: true,
            transactions: true,
            sessions: true,
            format_language: "postgresql",
            object_types: Self::object_types(),
        }
    }

    fn server_sessions_sql(&self) -> Option<String> {
        Some(
            "SELECT pid::text AS id, usename AS \"user\", COALESCE(client_addr::text,'-') AS host, \
             COALESCE(application_name,'-') AS program, COALESCE(state,'-') AS state, \
             COALESCE(EXTRACT(EPOCH FROM (now() - query_start))::bigint::text,'0') AS secs, \
             COALESCE(wait_event_type || '/' || wait_event, '-') AS extra, left(query, 800) AS sql_text, \
             COALESCE(array_to_string(pg_blocking_pids(pid), ','), '') AS blocking \
             FROM pg_stat_activity WHERE pid <> pg_backend_pid() ORDER BY query_start NULLS LAST"
                .into(),
        )
    }

    fn server_locks_sql(&self) -> Option<String> {
        Some(
            "SELECT l.pid::text AS id, COALESCE(a.usename, '') AS \"user\", l.locktype, l.mode, \
             CASE WHEN l.granted THEN 'YES' ELSE 'NO' END AS granted, \
             COALESCE(l.relation::regclass::text, '') AS target, \
             COALESCE(a.state, '') AS state, COALESCE(a.query, '') AS sql_text \
             FROM pg_locks l LEFT JOIN pg_stat_activity a ON a.pid = l.pid \
             WHERE l.pid <> pg_backend_pid() ORDER BY l.granted, l.pid"
                .into(),
        )
    }

    fn kill_session_sql(&self, id: &str, mode: &str) -> Option<String> {
        // pid 为无符号；负数/非数字一律拒绝
        let pid: u64 = id.trim().parse().ok()?;
        Some(if mode == "cancel" {
            format!("SELECT pg_cancel_backend({pid})")
        } else {
            format!("SELECT pg_terminate_backend({pid})")
        })
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
            CellValue::Binary(b64) => format!("decode('{}', 'hex')", base64_to_hex(b64)),
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

    /// 简化版 DDL：列定义 + NOT NULL + DEFAULT + 注释；不含主键/外键/索引
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
        let mut lines: Vec<String> = cols
            .iter()
            .map(|c| {
                let mut line = format!("  {}", self.quote_ident(&c.name));
                line.push_str(&format!(" {}", c.data_type));
                if !c.nullable {
                    line.push_str(" NOT NULL");
                }
                if let Some(def) = c.default_value.as_deref().filter(|d| !d.is_empty()) {
                    line.push_str(&format!(" DEFAULT {def}"));
                }
                line
            })
            .collect();
        let mut out = format!("CREATE TABLE {ident} (\n{}\n);", lines.join(",\n"));
        for c in cols.iter().filter(|c| !c.comment.as_deref().unwrap_or("").is_empty()) {
            lines.clear();
            out.push_str(&format!(
                "\nCOMMENT ON COLUMN {ident}.{} IS {};",
                self.quote_ident(&c.name),
                self.quote_string(c.comment.as_deref().unwrap_or(""))
            ));
        }
        out
    }

    fn object_ddl_query(
        &self,
        obj: DbObjectType,
        schema: Option<&str>,
        name: &str,
    ) -> Option<DdlQuery> {
        let s = schema.filter(|s| !s.is_empty()).unwrap_or("public").to_string();
        let q = |sql: String| {
            Some(DdlQuery {
                sql,
                params: vec![s.clone(), name.to_string()],
                value_index: 0,
            })
        };
        match obj {
            DbObjectType::View => q(format!(
                "SELECT pg_get_viewdef('\"{}\".\"{}\"'::regclass, true)",
                s.replace('"', ""), name.replace('"', "")
            )),
            DbObjectType::Function | DbObjectType::Procedure => q(
                "SELECT pg_get_functiondef(p.oid) FROM pg_proc p \
                 JOIN pg_namespace n ON n.oid = p.pronamespace \
                 WHERE n.nspname = $1 AND p.proname = $2"
                    .into(),
            ),
            DbObjectType::Trigger => q(
                "SELECT pg_get_triggerdef(t.oid) FROM pg_trigger t \
                 JOIN pg_class c ON c.oid = t.tgrelid \
                 JOIN pg_namespace n ON n.oid = c.relnamespace \
                 WHERE n.nspname = $1 AND t.tgname = $2"
                    .into(),
            ),
            DbObjectType::Index => {
                q(format!(
                    "SELECT indexdef FROM pg_indexes WHERE schemaname = '{}' AND indexname = '{}'",
                    s.replace('\'', "''"),
                    name.replace('\'', "''")
                ))
            }
            _ => None,
        }
    }

    fn preview_rows_sql(&self, schema: Option<&str>, table: &str, limit: u32) -> String {
        let ident = match schema.filter(|s| !s.is_empty()) {
            Some(s) => format!("{}.{}", self.quote_ident(s), self.quote_ident(table)),
            None => self.quote_ident(table),
        };
        format!("SELECT * FROM {ident} LIMIT {limit}")
    }

    fn catalog_query(&self, obj: DbObjectType, _schema: Option<&str>) -> Option<CatalogQuery> {
        let sql: &str = match obj {
            DbObjectType::Table => {
                "SELECT c.relname, obj_description(c.oid, 'pg_class') \
                 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
                 WHERE c.relkind IN ('r', 'p') AND n.nspname = $1 ORDER BY 1"
            }
            DbObjectType::View => {
                "SELECT c.relname, obj_description(c.oid, 'pg_class') \
                 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
                 WHERE c.relkind IN ('v', 'm') AND n.nspname = $1 ORDER BY 1"
            }
            DbObjectType::Index => {
                "SELECT indexname, indexdef FROM pg_indexes WHERE schemaname = $1 ORDER BY 1"
            }
            DbObjectType::Sequence => {
                "SELECT sequencename, NULL::text FROM pg_sequences WHERE schemaname = $1 ORDER BY 1"
            }
            DbObjectType::Function => {
                "SELECT p.proname, NULL::text FROM pg_proc p \
                 JOIN pg_namespace n ON n.oid = p.pronamespace \
                 WHERE n.nspname = $1 AND p.prokind = 'f' ORDER BY 1"
            }
            DbObjectType::Procedure => {
                "SELECT p.proname, NULL::text FROM pg_proc p \
                 JOIN pg_namespace n ON n.oid = p.pronamespace \
                 WHERE n.nspname = $1 AND p.prokind = 'p' ORDER BY 1"
            }
            DbObjectType::Trigger => {
                "SELECT t.tgname, NULL::text FROM pg_trigger t \
                 JOIN pg_class c ON c.oid = t.tgrelid \
                 JOIN pg_namespace n ON n.oid = c.relnamespace \
                 WHERE n.nspname = $1 AND NOT t.tgisinternal ORDER BY 1"
            }
        };
        Some(CatalogQuery {
            sql: sql.to_string(),
            has_param: true,
            name_index: 0,
            comment_index: Some(1),
            status_index: None,
        })
    }

    fn catalog_count_query(&self, obj: DbObjectType, _schema: Option<&str>) -> Option<CatalogQuery> {
        let sql: &str = match obj {
            DbObjectType::Table => {
                "SELECT COUNT(*) FROM pg_class c \
                 JOIN pg_namespace n ON n.oid = c.relnamespace \
                 WHERE c.relkind IN ('r', 'p') AND n.nspname = $1"
            }
            DbObjectType::View => {
                "SELECT COUNT(*) FROM pg_class c \
                 JOIN pg_namespace n ON n.oid = c.relnamespace \
                 WHERE c.relkind IN ('v', 'm') AND n.nspname = $1"
            }
            DbObjectType::Index => "SELECT COUNT(*) FROM pg_indexes WHERE schemaname = $1",
            DbObjectType::Sequence => "SELECT COUNT(*) FROM pg_sequences WHERE schemaname = $1",
            DbObjectType::Function => {
                "SELECT COUNT(*) FROM pg_proc p \
                 JOIN pg_namespace n ON n.oid = p.pronamespace \
                 WHERE n.nspname = $1 AND p.prokind = 'f'"
            }
            DbObjectType::Procedure => {
                "SELECT COUNT(*) FROM pg_proc p \
                 JOIN pg_namespace n ON n.oid = p.pronamespace \
                 WHERE n.nspname = $1 AND p.prokind = 'p'"
            }
            DbObjectType::Trigger => {
                "SELECT COUNT(*) FROM pg_trigger t \
                 JOIN pg_class c ON c.oid = t.tgrelid \
                 JOIN pg_namespace n ON n.oid = c.relnamespace \
                 WHERE n.nspname = $1 AND NOT t.tgisinternal"
            }
        };
        Some(CatalogQuery {
            sql: sql.to_string(),
            has_param: true,
            name_index: 0,
            comment_index: None,
            status_index: None,
        })
    }

    fn table_partitioned_query(&self, schema: Option<&str>, table: &str) -> Option<String> {
        let schema_expr = match schema.filter(|s| !s.is_empty()) {
            Some(s) => self.quote_string(s),
            None => "current_schema()".to_string(),
        };
        Some(format!(
            "SELECT COUNT(*) FROM pg_catalog.pg_partitioned_table pt \
             JOIN pg_catalog.pg_class c ON c.oid = pt.partrelid \
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             WHERE c.relname = {} AND n.nspname = {schema_expr}",
            self.quote_string(table)
        ))
    }

    fn schema_snapshot_sql(&self, schema: Option<&str>) -> Option<String> {
        let cond = match schema.filter(|s| !s.is_empty()) {
            Some(s) => format!("c.table_schema = {}", self.quote_string(s)),
            None => "c.table_schema = current_schema()".to_string(),
        };
        Some(format!(
            "SELECT c.table_name, \
               CASE WHEN t.table_type = 'VIEW' THEN 'VIEW' ELSE 'TABLE' END, \
               c.column_name, c.data_type, \
               pg_catalog.obj_description(pc.oid, 'pg_class'), \
               pg_catalog.col_description(pc.oid, c.ordinal_position), \
               c.ordinal_position \
             FROM information_schema.columns c \
             JOIN information_schema.tables t \
               ON t.table_schema = c.table_schema AND t.table_name = c.table_name \
             JOIN pg_catalog.pg_class pc ON pc.relname = c.table_name \
             JOIN pg_catalog.pg_namespace pn ON pn.oid = pc.relnamespace AND pn.nspname = c.table_schema \
             WHERE {cond} ORDER BY c.table_name, c.ordinal_position"
        ))
    }

    fn explain_sql(&self, sql: &str) -> crate::error::DbResult<String> {
        let trimmed = sql.trim().trim_end_matches(';').to_string();
        Ok(format!("EXPLAIN (FORMAT JSON) {trimmed}"))
    }

    fn parse_explain(&self, json_text: &str) -> crate::error::DbResult<crate::db::plan::PlanNode> {
        crate::db::plan::parse_pg(json_text)
    }

    /// PG 差异：修改列拆分为多条 ALTER、注释与索引删除需 schema 限定
    fn table_change_ddl(
        &self,
        schema: Option<&str>,
        table: &str,
        change: &TableChange,
    ) -> Vec<String> {
        let sp = schema.filter(|s| !s.is_empty()).unwrap_or("public");
        let ident = format!("{}.{}", self.quote_ident(sp), self.quote_ident(table));
        match change {
            TableChange::AddColumn { column } => {
                let mut stmts = vec![format!(
                    "ALTER TABLE {ident} ADD COLUMN {}",
                    pg_col_def(self, column)
                )];
                if let Some(c) = column.comment.as_deref().filter(|c| !c.is_empty()) {
                    stmts.push(format!(
                        "COMMENT ON COLUMN {ident}.{} IS {}",
                        self.quote_ident(&column.name),
                        self.quote_string(c)
                    ));
                }
                stmts
            }
            TableChange::ModifyColumn { column } => {
                let qname = self.quote_ident(&column.name);
                let mut stmts = vec![format!(
                    "ALTER TABLE {ident} ALTER COLUMN {qname} TYPE {}",
                    column.data_type
                )];
                stmts.push(format!(
                    "ALTER TABLE {ident} ALTER COLUMN {qname} {} NOT NULL",
                    if column.nullable { "DROP" } else { "SET" }
                ));
                match column.default_value.as_deref().filter(|d| !d.is_empty()) {
                    Some(d) => stmts.push(format!(
                        "ALTER TABLE {ident} ALTER COLUMN {qname} SET DEFAULT {d}"
                    )),
                    None => stmts.push(format!(
                        "ALTER TABLE {ident} ALTER COLUMN {qname} DROP DEFAULT"
                    )),
                }
                if let Some(c) = column.comment.as_deref().filter(|c| !c.is_empty()) {
                    stmts.push(format!(
                        "COMMENT ON COLUMN {ident}.{qname} IS {}",
                        self.quote_string(c)
                    ));
                }
                stmts
            }
            TableChange::DropIndex { name } => vec![format!(
                "DROP INDEX {}.{}",
                self.quote_ident(sp),
                self.quote_ident(name)
            )],
            // 其余走默认实现
            _ => super::SqlDialect::table_change_ddl(self, schema, table, change),
        }
    }
}

/// PG 列定义（注释单独语句，不内联）
fn pg_col_def(dialect: &PostgresDialect, col: &ColumnChange) -> String {
    let mut s = format!("{} {}", dialect.quote_ident(&col.name), col.data_type);
    if !col.nullable {
        s.push_str(" NOT NULL");
    }
    if let Some(d) = col.default_value.as_deref().filter(|d| !d.is_empty()) {
        s.push_str(&format!(" DEFAULT {d}"));
    }
    s
}

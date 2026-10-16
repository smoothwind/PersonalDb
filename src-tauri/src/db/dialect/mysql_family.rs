//! MySQL / OceanBase(MySQL 模式) 家族方言。
//! 元数据来自 information_schema，参数占位符为 `?`（一个 schema 参数）。

use crate::db::objects::{CatalogQuery, DbObjectType};
use crate::db::plan;
use crate::db::types::{CellValue, ColumnInfo, ColumnMeta, SemanticType};

use super::{base64_to_hex, Capabilities, DdlQuery, DdlStrategy, SqlDialect};

fn qualified_ident(dialect: &MySqlFamilyDialect, schema: Option<&str>, table: &str) -> String {
    match schema.filter(|s| !s.is_empty()) {
        Some(s) => format!("{}.{}", dialect.quote_ident(s), dialect.quote_ident(table)),
        None => dialect.quote_ident(table),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kill_sql_injection_guard() {
        let d = MySqlFamilyDialect::mysql();
        assert_eq!(d.kill_session_sql("12", "kill"), Some("KILL 12".into()));
        assert_eq!(d.kill_session_sql("12", "cancel"), Some("KILL QUERY 12".into()));
        assert_eq!(d.kill_session_sql("12;DROP TABLE t", "kill"), None);
        assert_eq!(d.kill_session_sql("-1", "kill"), None);
        assert_eq!(d.kill_session_sql("1.5", "kill"), None);
        assert_eq!(d.kill_session_sql("abc", "kill"), None);
        assert_eq!(d.kill_session_sql("", "kill"), None);
        // OB MySQL：锁视图声明不支持
        let ob = MySqlFamilyDialect::oceanbase();
        assert_eq!(ob.server_locks_sql(), None);
        assert!(d.server_locks_sql().is_some());
    }
}

pub struct MySqlFamilyDialect {
    /// OceanBase MySQL 模式：无 performance_schema.data_locks，锁视图不可用
    pub ob: bool,
}

impl MySqlFamilyDialect {
    pub fn mysql() -> Self {
        Self { ob: false }
    }

    pub fn oceanbase() -> Self {
        Self { ob: true }
    }

    fn object_types() -> Vec<DbObjectType> {
        // MySQL 生态无独立序列对象（AUTO_INCREMENT 实现）
        vec![
            DbObjectType::Table,
            DbObjectType::View,
            DbObjectType::Index,
            DbObjectType::Function,
            DbObjectType::Procedure,
            DbObjectType::Trigger,
        ]
    }
}

impl SqlDialect for MySqlFamilyDialect {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            explain: true,
            export_ddl: true, // SHOW CREATE TABLE
            export_insert: true,
            transactions: true,
            sessions: true,
            format_language: "mysql",
            object_types: Self::object_types(),
        }
    }

    fn server_locks_sql(&self) -> Option<String> {
        // OceanBase MySQL 模式无 performance_schema.data_locks（直接声明不支持，
        // 避免下发必败查询）；MySQL 8+ 需要 performance_schema 开启且有权访问。
        // 注意 data_locks 列为 OBJECT_SCHEMA/OBJECT_NAME（无 TABLE_NAME）
        if self.ob {
            return None;
        }
        Some(
            "SELECT t.PROCESSLIST_ID AS id, COALESCE(t.PROCESSLIST_USER, '') AS `user`, \
             d.LOCK_TYPE, d.LOCK_MODE, \
             COALESCE(CONCAT_WS('.', d.OBJECT_SCHEMA, d.OBJECT_NAME), '') AS target, \
             COALESCE(d.INDEX_NAME, '') AS index_name, d.LOCK_STATUS, \
             COALESCE(t.PROCESSLIST_INFO, '') AS sql_text \
             FROM performance_schema.data_locks d \
             LEFT JOIN performance_schema.threads t ON t.THREAD_ID = d.THREAD_ID"
                .into(),
        )
    }

    fn server_sessions_sql(&self) -> Option<String> {
        Some(
            "SELECT CAST(Id AS CHAR) AS id, User, Host, '' AS program, Command AS state, \
             CAST(Time AS CHAR) AS secs, COALESCE(State,'') AS extra, COALESCE(Info,'') AS sql_text \
             FROM information_schema.processlist WHERE Id <> CONNECTION_ID() ORDER BY Time DESC"
                .into(),
        )
    }

    fn kill_session_sql(&self, id: &str, mode: &str) -> Option<String> {
        let id = id.trim();
        id.parse::<u64>().ok()?;
        Some(if mode == "cancel" { format!("KILL QUERY {id}") } else { format!("KILL {id}") })
    }

    fn quote_ident(&self, ident: &str) -> String {
        format!("`{}`", ident.replace('`', "``"))
    }

    fn quote_string(&self, s: &str) -> String {
        // 导出文件头已设置 NO_BACKSLASH_ESCAPES，反斜杠为字面量，仅转义单引号
        format!("'{}'", s.replace('\'', "''"))
    }

    fn literal(&self, v: &CellValue, semantic: SemanticType) -> String {
        match v {
            CellValue::Null => "NULL".into(),
            CellValue::Bool(b) => if *b { "b'1'" } else { "b'0'" }.into(),
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
        let ident = qualified_ident(self, schema, table);
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
        "SET NAMES utf8mb4;\nSET NO_BACKSLASH_ESCAPES = 1;\n\n"
    }

    fn table_ddl_strategy(&self, schema: Option<&str>, table: &str) -> DdlStrategy {
        DdlStrategy::Query(DdlQuery {
            sql: format!("SHOW CREATE TABLE {}", qualified_ident(self, schema, table)),
            params: vec![],
            value_index: 1,
        })
    }

    fn build_ddl_from_columns(
        &self,
        _schema: Option<&str>,
        _table: &str,
        _cols: &[ColumnMeta],
    ) -> String {
        // MySQL 家族走 SHOW CREATE TABLE（Query 策略），不使用此方法
        String::new()
    }

    fn object_ddl_query(
        &self,
        obj: DbObjectType,
        schema: Option<&str>,
        name: &str,
    ) -> Option<DdlQuery> {
        let ident = qualified_ident(self, schema, name);
        let (sql, idx) = match obj {
            DbObjectType::View => (format!("SHOW CREATE VIEW {ident}"), 1),
            DbObjectType::Function => (format!("SHOW CREATE FUNCTION {ident}"), 2),
            DbObjectType::Procedure => (format!("SHOW CREATE PROCEDURE {ident}"), 2),
            DbObjectType::Trigger => (format!("SHOW CREATE TRIGGER {ident}"), 2),
            _ => return None,
        };
        Some(DdlQuery {
            sql,
            params: vec![],
            value_index: idx,
        })
    }

    fn preview_rows_sql(&self, schema: Option<&str>, table: &str, limit: u32) -> String {
        let ident = match schema.filter(|s| !s.is_empty()) {
            Some(s) => format!("{}.{}", self.quote_ident(s), self.quote_ident(table)),
            None => self.quote_ident(table),
        };
        format!("SELECT * FROM {ident} LIMIT {limit}")
    }

    fn catalog_query(&self, obj: DbObjectType, _schema: Option<&str>) -> Option<CatalogQuery> {
        let (sql, with_comment): (&str, bool) = match obj {
            DbObjectType::Table => (
                "SELECT table_name, table_comment FROM information_schema.tables \
                 WHERE table_schema = ? AND table_type = 'BASE TABLE' ORDER BY table_name",
                true,
            ),
            DbObjectType::View => (
                "SELECT table_name, table_comment FROM information_schema.tables \
                 WHERE table_schema = ? AND table_type = 'VIEW' ORDER BY table_name",
                true,
            ),
            DbObjectType::Index => (
                "SELECT DISTINCT index_name FROM information_schema.statistics \
                 WHERE table_schema = ? AND index_name != 'PRIMARY' ORDER BY index_name",
                false,
            ),
            DbObjectType::Function => (
                "SELECT routine_name, routine_comment FROM information_schema.routines \
                 WHERE routine_schema = ? AND routine_type = 'FUNCTION' ORDER BY routine_name",
                true,
            ),
            DbObjectType::Procedure => (
                "SELECT routine_name, routine_comment FROM information_schema.routines \
                 WHERE routine_schema = ? AND routine_type = 'PROCEDURE' ORDER BY routine_name",
                true,
            ),
            DbObjectType::Trigger => (
                "SELECT trigger_name FROM information_schema.triggers \
                 WHERE trigger_schema = ? ORDER BY trigger_name",
                false,
            ),
            DbObjectType::Sequence => return None,
        };
        Some(CatalogQuery {
            sql: sql.to_string(),
            has_param: true,
            name_index: 0,
            comment_index: if with_comment { Some(1) } else { None },
            status_index: None,
        })
    }

    fn catalog_count_query(&self, obj: DbObjectType, _schema: Option<&str>) -> Option<CatalogQuery> {
        let sql: &str = match obj {
            DbObjectType::Table => {
                "SELECT COUNT(*) FROM information_schema.tables \
                 WHERE table_schema = ? AND table_type = 'BASE TABLE'"
            }
            DbObjectType::View => {
                "SELECT COUNT(*) FROM information_schema.tables \
                 WHERE table_schema = ? AND table_type = 'VIEW'"
            }
            DbObjectType::Index => {
                "SELECT COUNT(DISTINCT index_name) FROM information_schema.statistics \
                 WHERE table_schema = ? AND index_name != 'PRIMARY'"
            }
            DbObjectType::Function => {
                "SELECT COUNT(*) FROM information_schema.routines \
                 WHERE routine_schema = ? AND routine_type = 'FUNCTION'"
            }
            DbObjectType::Procedure => {
                "SELECT COUNT(*) FROM information_schema.routines \
                 WHERE routine_schema = ? AND routine_type = 'PROCEDURE'"
            }
            DbObjectType::Trigger => {
                "SELECT COUNT(*) FROM information_schema.triggers WHERE trigger_schema = ?"
            }
            DbObjectType::Sequence => return None,
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
        let cond = match schema.filter(|s| !s.is_empty()) {
            Some(s) => format!("TABLE_SCHEMA = {}", self.quote_string(s)),
            None => "TABLE_SCHEMA = DATABASE()".to_string(),
        };
        Some(format!(
            "SELECT COUNT(*) FROM information_schema.PARTITIONS \
             WHERE {cond} AND TABLE_NAME = {} AND PARTITION_NAME IS NOT NULL",
            self.quote_string(table)
        ))
    }

    fn schema_snapshot_sql(&self, schema: Option<&str>) -> Option<String> {
        let cond = match schema.filter(|s| !s.is_empty()) {
            Some(s) => format!("t.TABLE_SCHEMA = {}", self.quote_string(s)),
            None => "t.TABLE_SCHEMA = DATABASE()".to_string(),
        };
        Some(format!(
            "SELECT t.TABLE_NAME, t.TABLE_TYPE, c.COLUMN_NAME, c.COLUMN_TYPE, \
             t.TABLE_COMMENT, c.COLUMN_COMMENT \
             FROM information_schema.TABLES t \
             JOIN information_schema.COLUMNS c \
               ON c.TABLE_SCHEMA = t.TABLE_SCHEMA AND c.TABLE_NAME = t.TABLE_NAME \
             WHERE {cond} ORDER BY t.TABLE_NAME, c.ORDINAL_POSITION"
        ))
    }

    fn explain_sql(&self, sql: &str) -> crate::error::DbResult<String> {
        let trimmed = sql.trim().trim_end_matches(';').to_string();
        Ok(format!("EXPLAIN FORMAT=JSON {trimmed}"))
    }

    fn parse_explain(&self, json_text: &str) -> crate::error::DbResult<crate::db::plan::PlanNode> {
        plan::parse_mysql(json_text)
    }
}

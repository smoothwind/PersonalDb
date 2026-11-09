//! MySQL / OceanBase(MySQL 模式) 家族方言。
//! 元数据来自 information_schema，参数占位符为 `?`（一个 schema 参数）。

use crate::db::objects::{CatalogQuery, DbObjectType};
use crate::db::plan;
use crate::db::templates;
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

    #[test]
    fn dml_literal_escapes_backslash() {
        let d = MySqlFamilyDialect::mysql();
        let path = r"C:\new".to_string();
        // 导出语义（文件头 NO_BACKSLASH_ESCAPES）：反斜杠字面量
        assert_eq!(d.quote_string(&path), r"'C:\new'");
        // DML 语义（会话默认模式）：反斜杠转义，避免 \n 被解析为换行
        assert_eq!(
            d.dml_literal(&CellValue::Text(path), SemanticType::Text),
            r"'C:\\new'"
        );
        // 两条路径都成对转义单引号
        assert_eq!(d.quote_string("a'b"), "'a''b'");
        assert_eq!(
            d.dml_literal(&CellValue::Text("a'b".into()), SemanticType::Text),
            "'a''b'"
        );
    }
}

pub struct MySqlFamilyDialect {
    /// OceanBase MySQL 模式：无 performance_schema.data_locks，锁视图不可用
    pub ob: bool,
}

impl MySqlFamilyDialect {
    /// 模板 key 用数据库种类（方言共用，按模式分流）
    fn template_kind(&self) -> &'static str {
        if self.ob {
            "oceanbase"
        } else {
            "mysql"
        }
    }

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

    /// 字符串引用（escape_backslash = 服务器按反斜杠转义解析的会话语义）
    fn quote_string_impl(&self, s: &str, escape_backslash: bool) -> String {
        let inner = if escape_backslash {
            s.replace('\\', "\\\\").replace('\'', "''")
        } else {
            s.replace('\'', "''")
        };
        format!("'{inner}'")
    }

    fn literal_impl(&self, v: &CellValue, semantic: SemanticType, escape_backslash: bool) -> String {
        match v {
            CellValue::Null => "NULL".into(),
            CellValue::Bool(b) => if *b { "b'1'" } else { "b'0'" }.into(),
            CellValue::Int(i) => i.to_string(),
            CellValue::UInt(u) => u.to_string(),
            CellValue::Float(f) => f.to_string(),
            CellValue::Binary(b64) => format!("X'{}'", base64_to_hex(b64)),
            CellValue::Text(s) => match semantic {
                SemanticType::Binary => "NULL".into(),
                _ => self.quote_string_impl(s, escape_backslash),
            },
        }
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
        templates::resolve_opt("locks.mysql", Some(BUILTIN_LOCKS.to_string()))
    }

    fn server_sessions_sql(&self) -> Option<String> {
        templates::resolve_opt(
            &format!("sessions.{}", self.template_kind()),
            Some(BUILTIN_SESSIONS.to_string()),
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
        self.quote_string_impl(s, false)
    }

    fn literal(&self, v: &CellValue, semantic: SemanticType) -> String {
        self.literal_impl(v, semantic, false)
    }

    /// DML 生成（会话内执行）：服务器默认模式无 NO_BACKSLASH_ESCAPES，
    /// 含反斜杠的字符串值须转义，否则 UPDATE/DELETE 定位错位甚至语法错误
    fn dml_literal(&self, v: &CellValue, semantic: SemanticType) -> String {
        self.literal_impl(v, semantic, true)
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
        // NO_BACKSLASH_ESCAPES 是 sql_mode 值而非独立变量——直接 SET 会报
        // ERROR 1193（Unknown system variable），重放导出文件首行即失败。
        // 合并进现有 sql_mode（保留其他模式），MySQL/OB 通用。
        "SET NAMES utf8mb4;\nSET SESSION sql_mode = CONCAT(@@sql_mode, ',NO_BACKSLASH_ESCAPES');\n\n"
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
        let (tpl, idx) = match obj {
            DbObjectType::View => ("SHOW CREATE VIEW {table}", 1),
            DbObjectType::Function => ("SHOW CREATE FUNCTION {table}", 2),
            DbObjectType::Procedure => ("SHOW CREATE PROCEDURE {table}", 2),
            DbObjectType::Trigger => ("SHOW CREATE TRIGGER {table}", 2),
            _ => return None,
        };
        let key = format!("objectDdl.{}.{}", self.template_kind(), obj.key());
        Some(DdlQuery {
            sql: templates::substitute(&templates::resolve(&key, tpl), &[("table", &ident)]),
            params: vec![],
            value_index: idx,
        })
    }

    fn preview_rows_sql(&self, schema: Option<&str>, table: &str, limit: u32) -> String {
        let ident = match schema.filter(|s| !s.is_empty()) {
            Some(s) => format!("{}.{}", self.quote_ident(s), self.quote_ident(table)),
            None => self.quote_ident(table),
        };
        let tpl = templates::resolve(
            &format!("previewRows.{}", self.template_kind()),
            "SELECT * FROM {table} LIMIT {limit}",
        );
        templates::substitute(&tpl, &[("table", &ident), ("limit", &limit.to_string())])
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
            sql: templates::resolve(
                &format!("catalog.{}.{}", self.template_kind(), obj.key()),
                sql,
            ),
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
            sql: templates::resolve(
                &format!("catalogCount.{}.{}", self.template_kind(), obj.key()),
                sql,
            ),
            has_param: true,
            name_index: 0,
            comment_index: None,
            status_index: None,
        })
    }

    fn table_partitioned_query(&self, schema: Option<&str>, table: &str) -> Option<String> {
        let schema_expr = match schema.filter(|s| !s.is_empty()) {
            Some(s) => self.quote_string(s),
            None => "DATABASE()".to_string(),
        };
        let tpl = templates::resolve(
            &format!("partitioned.{}", self.template_kind()),
            "SELECT COUNT(*) FROM information_schema.PARTITIONS \
             WHERE TABLE_SCHEMA = {schema} AND TABLE_NAME = {table} \
               AND PARTITION_NAME IS NOT NULL",
        );
        Some(templates::substitute(
            &tpl,
            &[("schema", &schema_expr), ("table", &self.quote_string(table))],
        ))
    }

    fn schema_snapshot_sql(&self, schema: Option<&str>) -> Option<String> {
        let schema_expr = match schema.filter(|s| !s.is_empty()) {
            Some(s) => self.quote_string(s),
            None => "DATABASE()".to_string(),
        };
        let tpl = templates::resolve(
            &format!("snapshot.{}", self.template_kind()),
            "SELECT t.TABLE_NAME, t.TABLE_TYPE, c.COLUMN_NAME, c.COLUMN_TYPE, \
             t.TABLE_COMMENT, c.COLUMN_COMMENT \
             FROM information_schema.TABLES t \
             JOIN information_schema.COLUMNS c \
               ON c.TABLE_SCHEMA = t.TABLE_SCHEMA AND c.TABLE_NAME = t.TABLE_NAME \
             WHERE t.TABLE_SCHEMA = {schema} ORDER BY t.TABLE_NAME, c.ORDINAL_POSITION",
        );
        Some(templates::substitute(&tpl, &[("schema", &schema_expr)]))
    }

    fn explain_sql(&self, sql: &str) -> crate::error::DbResult<String> {
        let trimmed = sql.trim().trim_end_matches(';').to_string();
        let tpl = templates::resolve(
            &format!("explain.{}", self.template_kind()),
            "EXPLAIN FORMAT=JSON {sql}",
        );
        Ok(templates::substitute(&tpl, &[("sql", &trimmed)]))
    }

    fn parse_explain(&self, json_text: &str) -> crate::error::DbResult<crate::db::plan::PlanNode> {
        plan::parse_mysql(json_text)
    }
}

/// 会话/锁视图内置 SQL（模板注册表规约采集与 resolve 兜底共用）
const BUILTIN_LOCKS: &str = "SELECT t.PROCESSLIST_ID AS id, COALESCE(t.PROCESSLIST_USER, '') AS `user`, \
     d.LOCK_TYPE, d.LOCK_MODE, \
     COALESCE(CONCAT_WS('.', d.OBJECT_SCHEMA, d.OBJECT_NAME), '') AS target, \
     COALESCE(d.INDEX_NAME, '') AS index_name, d.LOCK_STATUS, \
     COALESCE(t.PROCESSLIST_INFO, '') AS sql_text \
     FROM performance_schema.data_locks d \
     LEFT JOIN performance_schema.threads t ON t.THREAD_ID = d.THREAD_ID";

const BUILTIN_SESSIONS: &str = "SELECT CAST(Id AS CHAR) AS id, User, Host, '' AS program, Command AS state, \
     CAST(Time AS CHAR) AS secs, COALESCE(State,'') AS extra, COALESCE(Info,'') AS sql_text \
     FROM information_schema.processlist WHERE Id <> CONNECTION_ID() ORDER BY Time DESC";

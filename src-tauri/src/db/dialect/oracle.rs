//! Oracle 方言（feature-gated）。有 owner 时用 ALL_ 视图（绑定 :1），
//! 无 owner（当前用户）时用 USER_ 视图（无参数）。

use crate::db::objects::{CatalogQuery, DbObjectType};
use crate::db::templates;
use crate::db::types::{CellValue, ColumnInfo, ColumnMeta, SemanticType};

use super::{base64_to_hex, Capabilities, ColumnChange, DdlQuery, DdlStrategy, SqlDialect, TableChange};

pub struct OracleDialect {
    /// OceanBase Oracle 模式：使用 OB 兼容视图（V$OB_PROCESSLIST / V$OB_LOCKS），
    /// 标准 Oracle 的 V$SESSION / V$LOCK 在该模式下不存在
    pub ob: bool,
}

impl OracleDialect {
    pub fn oracle() -> Self {
        Self { ob: false }
    }

    pub fn oceanbase() -> Self {
        Self { ob: true }
    }

    /// 模板 key 用数据库种类（方言共用，按模式分流）
    fn template_kind(&self) -> &'static str {
        if self.ob {
            "oceanbase-oracle"
        } else {
            "oracle"
        }
    }
}

#[cfg(test)]
mod kill_tests {
    use super::*;

    #[test]
    fn kill_sql_injection_guard() {
        let d = OracleDialect::oracle();
        assert_eq!(
            d.kill_session_sql("3,45", "kill"),
            Some("ALTER SYSTEM KILL SESSION '3,45' IMMEDIATE".into())
        );
        assert_eq!(
            d.kill_session_sql("3,45", "cancel"),
            Some("ALTER SYSTEM DISCONNECT SESSION '3,45' POST_TRANSACTION".into())
        );
        // 缺 serial / 非数字 / 负数 / 多段 → 一律拒绝
        assert_eq!(d.kill_session_sql("3", "kill"), None);
        assert_eq!(d.kill_session_sql("3,45,7", "kill"), None);
        assert_eq!(d.kill_session_sql("a,b", "kill"), None);
        assert_eq!(d.kill_session_sql("3,-1", "kill"), None);
        assert_eq!(d.kill_session_sql("", "kill"), None);
    }

    #[test]
    fn ob_kill_and_views() {
        let ob = OracleDialect::oceanbase();
        // OB Oracle：按 ID 终止；无 cancel 能力
        assert_eq!(
            ob.kill_session_sql("3221701774", "kill"),
            Some("ALTER SYSTEM KILL SESSION '3221701774'".into())
        );
        assert_eq!(ob.kill_session_sql("3221701774", "cancel"), None);
        assert_eq!(ob.kill_session_sql("1; DROP TABLE t", "kill"), None);
        // OB 走 OB 兼容视图；不再使用 V$SESSION
        let sessions = ob.server_sessions_sql().expect("ob sessions sql");
        assert!(sessions.contains("V$OB_PROCESSLIST"));
        assert!(ob.server_sessions_fallback_sql().is_none());
        let locks = ob.server_locks_sql().expect("ob locks sql");
        assert!(locks.contains("V$OB_LOCKS"));
        // 标准 Oracle 保持原视图
        let ora = OracleDialect::oracle();
        assert!(ora.server_sessions_sql().unwrap().contains("v$session"));
    }
}

fn qualified_ident(dialect: &OracleDialect, schema: Option<&str>, table: &str) -> String {
    match schema.filter(|s| !s.is_empty()) {
        Some(s) => format!("{}.{}", dialect.quote_ident(s), dialect.quote_ident(table)),
        None => dialect.quote_ident(table),
    }
}

struct CatalogSpec {
    sql: &'static str,
    with_comment: bool,
    with_status: bool,
}

/// 指定 owner 的目录查询（对象状态来自 all_objects/all_indexes）
fn all_query(obj: DbObjectType) -> Option<CatalogSpec> {
    let spec = match obj {
        DbObjectType::Table => CatalogSpec {
            sql: "SELECT o.object_name, c.comments, o.status FROM all_objects o \
                  LEFT JOIN all_tab_comments c ON c.owner = o.owner \
                    AND c.table_name = o.object_name AND c.table_type = 'TABLE' \
                  WHERE o.owner = :1 AND o.object_type = 'TABLE' ORDER BY o.object_name",
            with_comment: true,
            with_status: true,
        },
        DbObjectType::View => CatalogSpec {
            sql: "SELECT o.object_name, c.comments, o.status FROM all_objects o \
                  LEFT JOIN all_tab_comments c ON c.owner = o.owner \
                    AND c.table_name = o.object_name AND c.table_type = 'VIEW' \
                  WHERE o.owner = :1 AND o.object_type = 'VIEW' ORDER BY o.object_name",
            with_comment: true,
            with_status: true,
        },
        DbObjectType::Index => CatalogSpec {
            sql: "SELECT index_name, status FROM all_indexes WHERE owner = :1 ORDER BY index_name",
            with_comment: false,
            with_status: true,
        },
        DbObjectType::Sequence => CatalogSpec {
            sql: "SELECT sequence_name FROM all_sequences WHERE sequence_owner = :1 ORDER BY sequence_name",
            with_comment: false,
            with_status: false,
        },
        DbObjectType::Function => CatalogSpec {
            sql: "SELECT object_name, status FROM all_objects \
                  WHERE owner = :1 AND object_type = 'FUNCTION' ORDER BY object_name",
            with_comment: false,
            with_status: true,
        },
        DbObjectType::Procedure => CatalogSpec {
            sql: "SELECT object_name, status FROM all_objects \
                  WHERE owner = :1 AND object_type = 'PROCEDURE' ORDER BY object_name",
            with_comment: false,
            with_status: true,
        },
        DbObjectType::Trigger => CatalogSpec {
            sql: "SELECT object_name, status FROM all_objects \
                  WHERE owner = :1 AND object_type = 'TRIGGER' ORDER BY object_name",
            with_comment: false,
            with_status: true,
        },
    };
    Some(spec)
}

/// 当前用户的目录查询
fn user_query(obj: DbObjectType) -> Option<CatalogSpec> {
    let spec = match obj {
        DbObjectType::Table => CatalogSpec {
            sql: "SELECT o.object_name, c.comments, o.status FROM user_objects o \
                  LEFT JOIN user_tab_comments c ON c.table_name = o.object_name \
                    AND c.table_type = 'TABLE' \
                  WHERE o.object_type = 'TABLE' ORDER BY o.object_name",
            with_comment: true,
            with_status: true,
        },
        DbObjectType::View => CatalogSpec {
            sql: "SELECT o.object_name, c.comments, o.status FROM user_objects o \
                  LEFT JOIN user_tab_comments c ON c.table_name = o.object_name \
                    AND c.table_type = 'VIEW' \
                  WHERE o.object_type = 'VIEW' ORDER BY o.object_name",
            with_comment: true,
            with_status: true,
        },
        DbObjectType::Index => CatalogSpec {
            sql: "SELECT index_name, status FROM user_indexes ORDER BY index_name",
            with_comment: false,
            with_status: true,
        },
        DbObjectType::Sequence => CatalogSpec {
            sql: "SELECT sequence_name FROM user_sequences ORDER BY sequence_name",
            with_comment: false,
            with_status: false,
        },
        DbObjectType::Function => CatalogSpec {
            sql: "SELECT object_name, status FROM user_objects \
                  WHERE object_type = 'FUNCTION' ORDER BY object_name",
            with_comment: false,
            with_status: true,
        },
        DbObjectType::Procedure => CatalogSpec {
            sql: "SELECT object_name, status FROM user_objects \
                  WHERE object_type = 'PROCEDURE' ORDER BY object_name",
            with_comment: false,
            with_status: true,
        },
        DbObjectType::Trigger => CatalogSpec {
            sql: "SELECT object_name, status FROM user_objects \
                  WHERE object_type = 'TRIGGER' ORDER BY object_name",
            with_comment: false,
            with_status: true,
        },
    };
    Some(spec)
}

impl SqlDialect for OracleDialect {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            explain: false, // P4: DBMS_XPLAN 文本解析
            export_ddl: true,
            export_insert: true,
            // Oracle OCI 连接非 Send，暂不支持专用连接承载手动事务（始终自动提交）
            transactions: false,
            sessions: true,
            format_language: "plsql",
            object_types: vec![
                DbObjectType::Table,
                DbObjectType::View,
                DbObjectType::Index,
                DbObjectType::Sequence,
                DbObjectType::Function,
                DbObjectType::Procedure,
                DbObjectType::Trigger,
            ],
        }
    }

    fn server_sessions_sql(&self) -> Option<String> {
        // OceanBase Oracle 模式：无 V$SESSION，使用 OB 兼容视图 V$OB_PROCESSLIST
        if self.ob {
            return templates::resolve_opt(
                "sessions.oceanbase-oracle",
                Some(BUILTIN_SESSIONS_OB.to_string()),
            );
        }
        templates::resolve_opt("sessions.oracle", Some(BUILTIN_SESSIONS_ORACLE.to_string()))
    }

    fn server_sessions_fallback_sql(&self) -> Option<String> {
        // OB 的主查询即兼容视图，无需降级；标准 Oracle 无 v$sql 权限时退化为仅 v$session
        if self.ob {
            return None;
        }
        templates::resolve_opt(
            "sessionsFallback.oracle",
            Some(BUILTIN_SESSIONS_FALLBACK.to_string()),
        )
    }

    fn server_locks_sql(&self) -> Option<String> {
        if self.ob {
            // OB 锁视图：LMODE/REQUEST 为字符串（'X'/'NONE' 等），不做数值 DECODE；
            // 无会话 id（终止按钮随之禁用）
            return templates::resolve_opt(
                "locks.oceanbase-oracle",
                Some(BUILTIN_LOCKS_OB.to_string()),
            );
        }
        templates::resolve_opt("locks.oracle", Some(BUILTIN_LOCKS_ORACLE.to_string()))
    }

    fn kill_session_sql(&self, id: &str, mode: &str) -> Option<String> {
        if self.ob {
            // OB Oracle：ALTER SYSTEM KILL SESSION '<ID>'；无 cancel/断开模式区分
            let id = id.trim();
            id.parse::<u64>().ok()?;
            if mode == "cancel" {
                return None;
            }
            return Some(format!("ALTER SYSTEM KILL SESSION '{id}'"));
        }
        // id 形如 "sid,serial#"，两段都必须是纯数字（防注入）
        let parts: Vec<&str> = id.trim().split(',').collect();
        if parts.len() != 2 || parts[0].parse::<u64>().is_err() || parts[1].parse::<u64>().is_err()
        {
            return None;
        }
        Some(if mode == "cancel" {
            format!("ALTER SYSTEM DISCONNECT SESSION '{id}' POST_TRANSACTION")
        } else {
            format!("ALTER SYSTEM KILL SESSION '{id}' IMMEDIATE")
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
            CellValue::Bool(b) => if *b { "1" } else { "0" }.into(),
            CellValue::Int(i) => i.to_string(),
            CellValue::UInt(u) => u.to_string(),
            CellValue::Float(f) => f.to_string(),
            CellValue::Binary(b64) => format!("HEXTORAW('{}')", base64_to_hex(b64)),
            CellValue::Text(s) => match semantic {
                SemanticType::Binary => "NULL".into(),
                SemanticType::Date => format!("TO_DATE({}, 'YYYY-MM-DD')", self.quote_string(s)),
                SemanticType::DateTime => {
                    if s.contains('.') {
                        format!(
                            "TO_TIMESTAMP({}, 'YYYY-MM-DD HH24:MI:SS.FF')",
                            self.quote_string(s)
                        )
                    } else {
                        format!(
                            "TO_TIMESTAMP({}, 'YYYY-MM-DD HH24:MI:SS')",
                            self.quote_string(s)
                        )
                    }
                }
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
        "SET DEFINE OFF;\n\n"
    }

    fn ddl_comment_append(&self) -> bool {
        // GET_DDL 默认不导出注释（COMMENT transform OFF），由调用方拼接 COMMENT ON 语句
        true
    }

    fn table_ddl_strategy(&self, schema: Option<&str>, table: &str) -> DdlStrategy {
        match schema.filter(|s| !s.is_empty()) {
            Some(owner) => DdlStrategy::Query(DdlQuery {
                // 占位符按出现顺序绑定：:1 = 表名，:2 = owner
                sql: "SELECT DBMS_METADATA.GET_DDL('TABLE', :1, :2) FROM DUAL".into(),
                params: vec![table.to_string(), owner.to_string()],
                value_index: 0,
            }),
            None => DdlStrategy::Query(DdlQuery {
                sql: "SELECT DBMS_METADATA.GET_DDL('TABLE', :1) FROM DUAL".into(),
                params: vec![table.to_string()],
                value_index: 0,
            }),
        }
    }

    fn build_ddl_from_columns(
        &self,
        _schema: Option<&str>,
        _table: &str,
        _cols: &[ColumnMeta],
    ) -> String {
        // Oracle 走 DBMS_METADATA（Query 策略），不使用此方法
        String::new()
    }

    fn object_ddl_query(
        &self,
        obj: DbObjectType,
        schema: Option<&str>,
        name: &str,
    ) -> Option<DdlQuery> {
        let db_type = match obj {
            DbObjectType::View => "VIEW",
            DbObjectType::Function => "FUNCTION",
            DbObjectType::Procedure => "PROCEDURE",
            DbObjectType::Trigger => "TRIGGER",
            DbObjectType::Index => "INDEX",
            DbObjectType::Sequence => "SEQUENCE",
            _ => return None,
        };
        let (sql, params) = match schema.filter(|s| !s.is_empty()) {
            Some(owner) => (
                "SELECT DBMS_METADATA.GET_DDL(:1, :2, :3) FROM DUAL".to_string(),
                vec![db_type.to_string(), name.to_string(), owner.to_string()],
            ),
            None => (
                "SELECT DBMS_METADATA.GET_DDL(:1, :2) FROM DUAL".to_string(),
                vec![db_type.to_string(), name.to_string()],
            ),
        };
        Some(DdlQuery {
            sql: templates::resolve(
                &format!("objectDdl.{}.{}", self.template_kind(), obj.key()),
                &sql,
            ),
            params,
            value_index: 0,
        })
    }

    fn preview_rows_sql(&self, schema: Option<&str>, table: &str, limit: u32) -> String {
        let ident = match schema.filter(|s| !s.is_empty()) {
            Some(s) => format!("{}.{}", self.quote_ident(s), self.quote_ident(table)),
            None => self.quote_ident(table),
        };
        let tpl = templates::resolve(
            &format!("previewRows.{}", self.template_kind()),
            "SELECT * FROM {table} WHERE ROWNUM <= {limit}",
        );
        templates::substitute(&tpl, &[("table", &ident), ("limit", &limit.to_string())])
    }

    fn catalog_query(&self, obj: DbObjectType, schema: Option<&str>) -> Option<CatalogQuery> {
        let has_param = schema.filter(|s| !s.is_empty()).is_some();
        let spec = if has_param {
            all_query(obj)?
        } else {
            user_query(obj)?
        };
        // 目录模板 key：有 owner 参数走 ALL_ 变体；当前用户走 `.user` 变体
        let key = if has_param {
            format!("catalog.{}.{}", self.template_kind(), obj.key())
        } else {
            format!("catalog.{}.{}.user", self.template_kind(), obj.key())
        };
        let comment_index = if spec.with_comment { Some(1) } else { None };
        let status_index = if spec.with_status {
            Some(if spec.with_comment { 2 } else { 1 })
        } else {
            None
        };
        Some(CatalogQuery {
            sql: templates::resolve(&key, spec.sql),
            has_param,
            name_index: 0,
            comment_index,
            status_index,
        })
    }

    fn catalog_count_query(&self, obj: DbObjectType, schema: Option<&str>) -> Option<CatalogQuery> {
        // 由目录 SQL 派生 COUNT(*)：覆盖 `catalog.*` 后计数随之生效，避免重复维护
        let base = self.catalog_query(obj, schema)?.sql;
        let count_sql = match base.split_once(" FROM ") {
            Some((_, rest)) => format!("SELECT COUNT(*) FROM {rest}"),
            None => return None,
        };
        Some(CatalogQuery {
            sql: count_sql,
            has_param: schema.filter(|s| !s.is_empty()).is_some(),
            name_index: 0,
            comment_index: None,
            status_index: None,
        })
    }

    /// 重新编译（Oracle：视图/函数/存储过程/触发器/索引）
    fn recompile_sql(
        &self,
        obj: DbObjectType,
        schema: Option<&str>,
        name: &str,
    ) -> Option<String> {
        let ident = qualified_ident(self, schema, name);
        match obj {
            DbObjectType::View => Some(format!("ALTER VIEW {ident} COMPILE")),
            DbObjectType::Function => Some(format!("ALTER FUNCTION {ident} COMPILE")),
            DbObjectType::Procedure => Some(format!("ALTER PROCEDURE {ident} COMPILE")),
            DbObjectType::Trigger => Some(format!("ALTER TRIGGER {ident} COMPILE")),
            DbObjectType::Index => Some(format!("ALTER INDEX {ident} REBUILD")),
            _ => None,
        }
    }

    fn table_partitioned_query(&self, schema: Option<&str>, table: &str) -> Option<String> {
        let owner = match schema.filter(|s| !s.is_empty()) {
            Some(s) => self.quote_string(s),
            None => "USER".to_string(),
        };
        let tpl = templates::resolve(
            &format!("partitioned.{}", self.template_kind()),
            "SELECT COUNT(*) FROM all_part_tables \
             WHERE owner = {schema} AND table_name = {table}",
        );
        Some(templates::substitute(
            &tpl,
            &[("schema", &owner), ("table", &self.quote_string(table))],
        ))
    }

    fn schema_snapshot_sql(&self, schema: Option<&str>) -> Option<String> {
        let owner = match schema.filter(|s| !s.is_empty()) {
            Some(s) => self.quote_string(s),
            None => "USER".to_string(),
        };
        let tpl = templates::resolve(
            &format!("snapshot.{}", self.template_kind()),
            "SELECT c.table_name, NVL(o.object_type, 'TABLE'), c.column_name, c.data_type, \
               tc.comments, cc.comments, c.column_id \
             FROM all_tab_columns c \
             LEFT JOIN all_col_comments cc \
               ON cc.owner = c.owner AND cc.table_name = c.table_name AND cc.column_name = c.column_name \
             LEFT JOIN all_tab_comments tc ON tc.owner = c.owner AND tc.table_name = c.table_name \
             LEFT JOIN all_objects o ON o.owner = c.owner AND o.object_name = c.table_name \
               AND o.object_type IN ('TABLE', 'VIEW') \
             WHERE c.owner = {schema} ORDER BY c.table_name, c.column_id",
        );
        Some(templates::substitute(&tpl, &[("schema", &owner)]))
    }

    fn explain_sql(&self, _sql: &str) -> crate::error::DbResult<String> {
        Err(crate::error::AppError::General(
            "Oracle 执行计划解析将在后续版本提供（capabilities.explain = false）".into(),
        ))
    }

    fn parse_explain(&self, _json_text: &str) -> crate::error::DbResult<crate::db::plan::PlanNode> {
        Err(crate::error::AppError::General(
            "Oracle 执行计划解析将在后续版本提供".into(),
        ))
    }

    /// Oracle 差异：ADD/MODIFY 使用括号语法、注释单独语句、索引删除需 owner 限定
    fn table_change_ddl(
        &self,
        schema: Option<&str>,
        table: &str,
        change: &TableChange,
    ) -> Vec<String> {
        let owner = schema.filter(|s| !s.is_empty());
        let ident = qualified_ident(self, schema, table);
        match change {
            TableChange::AddColumn { column } => {
                let mut stmts = vec![format!(
                    "ALTER TABLE {ident} ADD ({})",
                    oracle_col_def(self, column)
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
                let mut stmts = vec![format!(
                    "ALTER TABLE {ident} MODIFY ({})",
                    oracle_col_def(self, column)
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
            TableChange::DropIndex { name } => match owner {
                Some(o) => vec![format!(
                    "DROP INDEX {}.{}",
                    self.quote_ident(o),
                    self.quote_ident(name)
                )],
                None => vec![format!("DROP INDEX {}", self.quote_ident(name))],
            },
            // 其余走默认实现
            _ => super::SqlDialect::table_change_ddl(self, schema, table, change),
        }
    }
}

/// Oracle 列定义（括号语法内使用，注释单独语句）
fn oracle_col_def(dialect: &OracleDialect, col: &ColumnChange) -> String {
    let mut s = format!("{} {}", dialect.quote_ident(&col.name), col.data_type);
    if let Some(d) = col.default_value.as_deref().filter(|d| !d.is_empty()) {
        s.push_str(&format!(" DEFAULT {d}"));
    }
    if !col.nullable {
        s.push_str(" NOT NULL");
    }
    s
}

/// 会话/锁视图内置 SQL（模板注册表规约采集与 resolve 兜底共用）
const BUILTIN_SESSIONS_OB: &str = "SELECT TO_CHAR(ID) AS id, USER AS \"user\", \
     NVL(USER_HOST, NVL(SVR_IP, '-')) AS host, NVL(MODULE, '-') AS program, \
     NVL(COMMAND, '-') AS state, NVL(TO_CHAR(TIME), '0') AS secs, \
     NVL(STATE, '-') || DECODE(DB, NULL, '', ' · ' || DB) AS extra, \
     NVL(INFO, ' ') AS sql_text, '' AS blocking \
     FROM V$OB_PROCESSLIST ORDER BY TIME DESC";

const BUILTIN_SESSIONS_ORACLE: &str = "SELECT s.sid || ',' || s.serial# AS id, NVL(s.username, ' ') AS \"user\", \
     NVL(s.machine, '-') AS host, NVL(s.program, '-') AS program, \
     NVL(s.status, '-') AS state, NVL(TO_CHAR(s.seconds_in_wait), '0') AS secs, \
     NVL(s.event, '-') AS extra, \
     NVL((SELECT q.sql_text FROM v$sql q WHERE q.address = s.sql_address \
       AND q.hash_value = s.sql_hash_value AND ROWNUM = 1), ' ') AS sql_text, \
     NVL(TO_CHAR(s.blocking_session), '') AS blocking \
     FROM v$session s WHERE s.sid <> USERENV('SID') ORDER BY s.sid";

const BUILTIN_SESSIONS_FALLBACK: &str = "SELECT s.sid || ',' || s.serial# AS id, NVL(s.username, ' ') AS \"user\", \
     NVL(s.machine, '-') AS host, NVL(s.program, '-') AS program, \
     NVL(s.status, '-') AS state, NVL(TO_CHAR(s.seconds_in_wait), '0') AS secs, \
     NVL(s.event, '-') AS extra, ' ' AS sql_text, \
     NVL(TO_CHAR(s.blocking_session), '') AS blocking \
     FROM v$session s WHERE s.sid <> USERENV('SID') ORDER BY s.sid";

const BUILTIN_LOCKS_OB: &str = "SELECT SVR_IP || ':' || TO_CHAR(SVR_PORT) AS instance, '' AS id, \
     TO_CHAR(TRANS_ID) AS trans_id, TYPE, ID1, ID2, \
     LMODE AS held_mode, REQUEST AS request_mode, CTIME, BLOCK AS blocking \
     FROM V$OB_LOCKS ORDER BY TRANS_ID";

const BUILTIN_LOCKS_ORACLE: &str = "SELECT s.sid || ',' || s.serial# AS id, NVL(s.username, ' ') AS \"user\", \
     l.TYPE AS lock_type, DECODE(l.LMODE, 0, 'None', 1, 'Null', 2, 'Row-S', 3, 'Row-X', \
     4, 'Share', 5, 'S/Row-X', 6, 'Exclusive', TO_CHAR(l.LMODE)) AS held_mode, \
     DECODE(l.REQUEST, 0, '', TO_CHAR(l.REQUEST)) AS request_mode, \
     l.ID1, l.ID2, NVL(s.machine, '-') AS host \
     FROM v$lock l JOIN v$session s ON s.sid = l.sid \
     WHERE l.TYPE IN ('TX', 'TM') ORDER BY l.TYPE, l.ID1, l.sid";

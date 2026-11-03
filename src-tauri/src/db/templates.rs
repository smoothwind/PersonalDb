//! 查询 SQL 模板注册表：内置默认 + 用户覆盖（存储类别 sqltemplates / `sqltemplates.json`）。
//!
//! 生效顺序：**用户覆盖（非空）→ 内置默认**。前端分页/预览与后端方言/驱动层查询 SQL
//! 同源登记：`spec()` 列出全部条目（设置页编辑），`resolved()` 返回全量生效值。
//!
//! - `resolve(key, built_in)`：覆盖（非空白）优先，否则内置；`bypass` 期间恒返回内置
//!   （设置页规约 harvest 用，取纯内置而非覆盖值）。
//! - `substitute`：`{name}` 占位符替换（引用后的标识符/字面量/数字在调用点代入）。
//!
//! 安全红线（**不提供模板覆盖**）：`kill_session_sql`（id 纯数字校验防注入）、
//! 标识符/字符串引用、INSERT 字面量生成、`recompile_sql`。
//!
//! 分页别名约定：**必须字母开头**（Oracle 系未引用标识符不允许 `_` 开头，
//! 历史 `_pdb_rn` 导致 ORA-00911，故为 `pdb_rn` / `pdb_p`）。

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use serde::Serialize;

use crate::db::dialect::{MySqlFamilyDialect, PostgresDialect, SqlDialect, SqliteDialect};
use crate::error::{AppError, DbResult};
use crate::storage::{self, StorageCategory};

#[cfg(feature = "oracle")]
use crate::db::dialect::OracleDialect;

/// 所有数据库种类 key（与 `DbKind::key()` 一致；oracle 系受 feature 门控）
pub(crate) const KINDS: &[&str] = &[
    "mysql",
    "oceanbase",
    "postgresql",
    "gaussdb",
    "oracle",
    "oceanbase-oracle",
    "sqlite",
    "jdbc",
];

fn kind_available(kind: &str) -> bool {
    !matches!(kind, "oracle" | "oceanbase-oracle") || cfg!(feature = "oracle")
}

// ---------------------------------------------------------------------------
// 覆盖存取
// ---------------------------------------------------------------------------

static STORE: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();
static LOADED: OnceLock<()> = OnceLock::new();

thread_local! {
    /// 规约 harvest 标记：true 时 resolve 恒返回内置默认
    static BYPASS: Cell<bool> = const { Cell::new(false) };
}

fn store() -> &'static RwLock<HashMap<String, String>> {
    let lock = STORE.get_or_init(|| RwLock::new(HashMap::new()));
    if LOADED.get().is_none() {
        let map = load_file().unwrap_or_default();
        if let Ok(mut guard) = lock.write() {
            *guard = map;
        }
        let _ = LOADED.set(());
    }
    lock
}

fn sql_templates_file() -> DbResult<std::path::PathBuf> {
    storage::file_path(StorageCategory::SqlTemplates, "sqltemplates.json")
}

fn load_file() -> DbResult<HashMap<String, String>> {
    let path = sql_templates_file()?;
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let text = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&text).unwrap_or_default())
}

/// 在 `f` 执行期间忽略覆盖（设置页规约 harvest：取纯内置）
pub fn bypass<T>(f: impl FnOnce() -> T) -> T {
    BYPASS.with(|b| b.set(true));
    let out = f();
    BYPASS.with(|b| b.set(false));
    out
}

/// 解析模板：用户覆盖（非空白）优先，否则内置默认；bypass 期间恒返回内置
pub fn resolve(key: &str, built_in: &str) -> String {
    if BYPASS.with(Cell::get) {
        return built_in.to_string();
    }
    let override_value = store()
        .read()
        .ok()
        .and_then(|g| g.get(key).filter(|s| !s.trim().is_empty()).cloned());
    override_value.unwrap_or_else(|| built_in.to_string())
}

/// `Option<String>` 版 resolve（None 透传：该能力本就不支持）
pub fn resolve_opt(key: &str, built_in: Option<String>) -> Option<String> {
    built_in.map(|b| resolve(key, &b))
}

/// `{name}` 占位符替换；未提供的占位符原样保留
pub fn substitute(tpl: &str, vars: &[(&str, &str)]) -> String {
    let mut out = tpl.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

/// 当前覆盖快照
pub fn overrides_snapshot() -> HashMap<String, String> {
    store().read().map(|g| g.clone()).unwrap_or_default()
}

/// 保存覆盖：剪裁空白值（视为未覆盖）→ 写文件 → 原子替换内存
pub fn save(map: HashMap<String, String>) -> DbResult<()> {
    let cleaned: HashMap<String, String> = map
        .into_iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .collect();
    let path = sql_templates_file()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::General(format!("创建 SQL 模板目录失败: {e}")))?;
    }
    let text = serde_json::to_string_pretty(&cleaned)
        .map_err(|e| AppError::General(format!("SQL 模板序列化失败: {e}")))?;
    std::fs::write(&path, text)
        .map_err(|e| AppError::General(format!("写入 SQL 模板文件失败: {e}")))?;
    if let Ok(mut guard) = store().write() {
        *guard = cleaned;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 前端内置模板（分页 / 预览；与 src/utils/sqlTemplates.ts 的离线兜底保持一致）
// ---------------------------------------------------------------------------

/// 分页包装（LIMIT 系：MySQL/PG/GaussDB/SQLite/JDBC/OB MySQL）
pub const PAGED_LIMIT: &str = "SELECT * FROM ({sql}) pdb_p LIMIT {limit} OFFSET {offset}";
/// 分页包装（Oracle 系 ROWNUM；别名 `pdb_rn` 必须字母开头）
pub const PAGED_ORACLE: &str =
    "SELECT * FROM (SELECT t.*, ROWNUM pdb_rn FROM ({sql}) t WHERE ROWNUM <= {end}) \
     WHERE pdb_rn > {offset}";

/// 前端分页内置模板
pub fn builtin_paged(kind: &str) -> &'static str {
    if matches!(kind, "oracle" | "oceanbase-oracle") {
        PAGED_ORACLE
    } else {
        PAGED_LIMIT
    }
}

/// 前端双击表预览内置模板（占位符 {database}/{table}/{limit}）
pub fn builtin_preview(kind: &str) -> &'static str {
    match kind {
        "mysql" | "oceanbase" => "SELECT * FROM `{database}`.`{table}` LIMIT {limit}",
        "postgresql" | "gaussdb" => "SELECT * FROM \"{database}\".\"{table}\" LIMIT {limit}",
        "oracle" | "oceanbase-oracle" => "SELECT * FROM \"{table}\" WHERE ROWNUM <= {limit}",
        // SQLite 无库概念；通用 JDBC 保守不加 LIMIT（传输层 max_rows 截断）
        "sqlite" | "jdbc" => "SELECT * FROM \"{table}\" LIMIT {limit}",
        _ => "SELECT * FROM \"{table}\" LIMIT {limit}",
    }
}

// ---------------------------------------------------------------------------
// 规约条目（设置页）
// ---------------------------------------------------------------------------

/// 模板规约条目（设置页「SQL 模板」全量列表）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSpecEntry {
    /// 模板 key（如 `paged.oracle` / `catalog.mysql.tables`）
    pub key: String,
    /// 分组 key（前端 i18n：`tplCat.<category>`）
    pub category: String,
    /// 数据库种类 key（"" = 通用默认条目）
    pub dialect: String,
    /// 占位符名（`{sql}` 等，不含花括号）
    pub placeholders: Vec<String>,
    /// 内置默认（bypass 采集）
    pub builtin: String,
    /// 用户覆盖（None = 未覆盖，生效值即内置）
    pub override_value: Option<String>,
}

fn entry(
    key: String,
    category: &str,
    dialect: &str,
    placeholders: &[&str],
    builtin: String,
    overrides: &HashMap<String, String>,
) -> TemplateSpecEntry {
    TemplateSpecEntry {
        override_value: overrides.get(&key).filter(|s| !s.trim().is_empty()).cloned(),
        key,
        category: category.to_string(),
        dialect: dialect.to_string(),
        placeholders: placeholders.iter().map(|s| s.to_string()).collect(),
        builtin,
    }
}

/// 方言实例（按 key 构造；oracle 系 feature 门控）
fn dialect_for(kind: &str) -> Option<Box<dyn SqlDialect>> {
    match kind {
        "mysql" => Some(Box::new(MySqlFamilyDialect::mysql())),
        "oceanbase" => Some(Box::new(MySqlFamilyDialect::oceanbase())),
        "postgresql" => Some(Box::new(PostgresDialect::default())),
        "gaussdb" => Some(Box::new(PostgresDialect::gauss())),
        "sqlite" => Some(Box::new(SqliteDialect)),
        #[cfg(feature = "oracle")]
        "oracle" => Some(Box::new(OracleDialect::oracle())),
        #[cfg(feature = "oracle")]
        "oceanbase-oracle" => Some(Box::new(OracleDialect::oceanbase())),
        _ => None,
    }
}

/// 全量规约条目（含 paged/preview 前端条目 + 方言层 + 驱动层）
pub fn spec() -> Vec<TemplateSpecEntry> {
    let overrides = overrides_snapshot();
    let mut out: Vec<TemplateSpecEntry> = Vec::new();

    // 前端：分页包装 + 双击预览（通用默认 + 每种类）
    out.push(entry(
        "paged.default".into(),
        "paged",
        "",
        &["sql", "offset", "limit", "end"],
        PAGED_LIMIT.to_string(),
        &overrides,
    ));
    out.push(entry(
        "preview.default".into(),
        "preview",
        "",
        &["database", "table", "limit"],
        builtin_preview("").to_string(),
        &overrides,
    ));

    // 驱动层内置（常量直采：方法需连接，不可调用采集）
    out.extend(driver_builtin_specs(&overrides));

    for kind in KINDS.iter().filter(|k| kind_available(k)) {
        let kind = *kind;

        out.push(entry(
            format!("paged.{kind}"),
            "paged",
            kind,
            &["sql", "offset", "limit", "end"],
            builtin_paged(kind).to_string(),
            &overrides,
        ));
        out.push(entry(
            format!("preview.{kind}"),
            "preview",
            kind,
            &["database", "table", "limit"],
            builtin_preview(kind).to_string(),
            &overrides,
        ));

        // JDBC 方言对象目录走 DatabaseMetaData（无 SQL），仅登记 previewRows
        if kind == "jdbc" {
            out.push(entry(
                "previewRows.jdbc".into(),
                "previewRows",
                kind,
                &["table"],
                preview_rows_builtin(kind).to_string(),
                &overrides,
            ));
            continue;
        }
        let Some(d) = dialect_for(kind) else { continue };
        let kind_dyn: &dyn SqlDialect = d.as_ref();

        out.extend(dialect_specs(kind_dyn, kind, &overrides));
    }
    out
}

/// 单个方言的模板条目采集（previewRows / catalog / objectDdl / 会话 / 锁 / 计划 …）
fn dialect_specs(
    d: &dyn SqlDialect,
    kind: &str,
    overrides: &HashMap<String, String>,
) -> Vec<TemplateSpecEntry> {
    let mut out = Vec::new();
    let objs = d.capabilities().object_types;

    // 预览前 N 行（{table} = 已引用限定标识符，{limit} = 数字）
    let preview_tpl = preview_rows_builtin(kind);
    out.push(entry(
        format!("previewRows.{kind}"),
        "previewRows",
        kind,
        &["table", "limit"],
        preview_tpl.to_string(),
        overrides,
    ));

    // 执行计划（{sql}）
    if let Some(tpl) = explain_builtin(kind) {
        out.push(entry(
            format!("explain.{kind}"),
            "explain",
            kind,
            &["sql"],
            tpl.to_string(),
            overrides,
        ));
    }

    // 会话 / 会话降级 / 锁（无占位符，覆盖即整条 SQL）
    bypass(|| {
        if let Some(sql) = d.server_sessions_sql() {
            out.push(entry(
                format!("sessions.{kind}"),
                "sessions",
                kind,
                &[],
                sql,
                overrides,
            ));
        }
        if let Some(sql) = d.server_sessions_fallback_sql() {
            out.push(entry(
                format!("sessionsFallback.{kind}"),
                "sessions",
                kind,
                &[],
                sql,
                overrides,
            ));
        }
        if let Some(sql) = d.server_locks_sql() {
            out.push(entry(
                format!("locks.{kind}"),
                "locks",
                kind,
                &[],
                sql,
                overrides,
            ));
        }
    });

    // 元数据快照（{schema} = 引用字符串字面量；无 schema 时代入当前库表达式；SQLite 无占位符）
    if let Some((tpl, phs)) = snapshot_builtin(kind) {
        out.push(entry(
            format!("snapshot.{kind}"),
            "snapshot",
            kind,
            phs,
            tpl.to_string(),
            overrides,
        ));
    }
    // 分区判定（{schema}/{table} = 引用字符串字面量）
    if let Some(tpl) = partitioned_builtin(kind) {
        out.push(entry(
            format!("partitioned.{kind}"),
            "partitioned",
            kind,
            &["schema", "table"],
            tpl.to_string(),
            overrides,
        ));
    }

    for obj in &objs {
        let obj_key = obj.key();

        // 目录查询：参数化变体为主 key；无参数变体与主变体内置不同时（Oracle 系
        // ALL_ / USER_ 视图）追加 `.user` 变体 key
        let (all_sql, user_sql) = bypass(|| {
            (
                d.catalog_query(*obj, Some("x")).map(|q| q.sql),
                d.catalog_query(*obj, None).map(|q| q.sql),
            )
        });
        let user_differs = all_sql.as_deref() != user_sql.as_deref();
        if let Some(sql) = all_sql {
            out.push(entry(
                format!("catalog.{kind}.{obj_key}"),
                "catalog",
                kind,
                &[],
                sql,
                overrides,
            ));
        }
        if let Some(sql) = user_sql {
            if user_differs {
                out.push(entry(
                    format!("catalog.{kind}.{obj_key}.user"),
                    "catalog",
                    kind,
                    &[],
                    sql,
                    overrides,
                ));
            }
        }

        // 目录计数（Oracle 系由目录 SQL 派生 COUNT，无独立 key）
        if !matches!(kind, "oracle" | "oceanbase-oracle") {
            if let Some(q) = bypass(|| d.catalog_count_query(*obj, Some("x"))) {
                out.push(entry(
                    format!("catalogCount.{kind}.{obj_key}"),
                    "catalogCount",
                    kind,
                    &[],
                    q.sql,
                    overrides,
                ));
            }
        }

        // 对象定义查询（MySQL 经 {table} 代入；SQLite 经 {type}；PG/Oracle 为绑定参数版）
        if let Some(q) = bypass(|| d.object_ddl_query(*obj, Some("s"), "x")) {
            let placeholders: &[&str] = match kind {
                "mysql" | "oceanbase" => &["table"],
                "sqlite" => &["type"],
                _ => &[],
            };
            out.push(entry(
                format!("objectDdl.{kind}.{obj_key}"),
                "objectDdl",
                kind,
                placeholders,
                q.sql,
                overrides,
            ));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// 含占位符的方言内置模板（方法运行时 substitute；常量供本模块规约采集）
// ---------------------------------------------------------------------------

fn preview_rows_builtin(kind: &str) -> &'static str {
    match kind {
        "oracle" | "oceanbase-oracle" => "SELECT * FROM {table} WHERE ROWNUM <= {limit}",
        "sqlite" | "jdbc" => "SELECT * FROM {table}",
        _ => "SELECT * FROM {table} LIMIT {limit}",
    }
}

fn explain_builtin(kind: &str) -> Option<&'static str> {
    match kind {
        "mysql" | "oceanbase" => Some("EXPLAIN FORMAT=JSON {sql}"),
        "postgresql" | "gaussdb" => Some("EXPLAIN (FORMAT JSON) {sql}"),
        "sqlite" => Some("EXPLAIN QUERY PLAN {sql}"),
        _ => None,
    }
}

fn snapshot_builtin(kind: &str) -> Option<(&'static str, &'static [&'static str])> {
    match kind {
        "mysql" | "oceanbase" => Some((
            "SELECT t.TABLE_NAME, t.TABLE_TYPE, c.COLUMN_NAME, c.COLUMN_TYPE, \
             t.TABLE_COMMENT, c.COLUMN_COMMENT \
             FROM information_schema.TABLES t \
             JOIN information_schema.COLUMNS c \
               ON c.TABLE_SCHEMA = t.TABLE_SCHEMA AND c.TABLE_NAME = t.TABLE_NAME \
             WHERE t.TABLE_SCHEMA = {schema} ORDER BY t.TABLE_NAME, c.ORDINAL_POSITION",
            &["schema"],
        )),
        "postgresql" | "gaussdb" => Some((
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
             WHERE c.table_schema = {schema} ORDER BY c.table_name, c.ordinal_position",
            &["schema"],
        )),
        "oracle" | "oceanbase-oracle" => Some((
            "SELECT c.table_name, NVL(o.object_type, 'TABLE'), c.column_name, c.data_type, \
               tc.comments, cc.comments, c.column_id \
             FROM all_tab_columns c \
             LEFT JOIN all_col_comments cc \
               ON cc.owner = c.owner AND cc.table_name = c.table_name AND cc.column_name = c.column_name \
             LEFT JOIN all_tab_comments tc ON tc.owner = c.owner AND tc.table_name = c.table_name \
             LEFT JOIN all_objects o ON o.owner = c.owner AND o.object_name = c.table_name \
               AND o.object_type IN ('TABLE', 'VIEW') \
             WHERE c.owner = {schema} ORDER BY c.table_name, c.column_id",
            &["schema"],
        )),
        // SQLite 无 schema 概念（静态查询）
        "sqlite" => Some((
            "SELECT m.name, UPPER(m.type), c.name, c.type, \
                    NULL, NULL \
             FROM sqlite_master m \
             JOIN pragma_table_info(m.name) c \
             WHERE m.type IN ('table', 'view') \
               AND m.name NOT LIKE 'sqlite\\_%' ESCAPE '\\' \
             ORDER BY m.name, c.cid",
            &[],
        )),
        _ => None,
    }
}

fn partitioned_builtin(kind: &str) -> Option<&'static str> {
    match kind {
        "mysql" | "oceanbase" => Some(
            "SELECT COUNT(*) FROM information_schema.PARTITIONS \
             WHERE TABLE_SCHEMA = {schema} AND TABLE_NAME = {table} \
               AND PARTITION_NAME IS NOT NULL",
        ),
        // GaussDB 无 pg_partitioned_table（分区信息在 pg_partition），不提供
        "postgresql" => Some(
            "SELECT COUNT(*) FROM pg_catalog.pg_partitioned_table pt \
             JOIN pg_catalog.pg_class c ON c.oid = pt.partrelid \
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             WHERE c.relname = {table} AND n.nspname = {schema}",
        ),
        "oracle" | "oceanbase-oracle" => Some(
            "SELECT COUNT(*) FROM all_part_tables \
             WHERE owner = {schema} AND table_name = {table}",
        ),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// 驱动层内置规约（常量见各驱动文件 builtin_sql 模块）
// ---------------------------------------------------------------------------

#[cfg(feature = "oracle")]
fn oracle_driver_specs(out: &mut Vec<TemplateSpecEntry>, overrides: &HashMap<String, String>) {
    use crate::db::oracle::builtin_sql as b;
    for kind in ["oracle", "oceanbase-oracle"] {
        out.push(entry(
            format!("databases.{kind}"),
            "driver",
            kind,
            &[],
            b::DATABASES.to_string(),
            overrides,
        ));
        out.push(entry(
            format!("tables.{kind}"),
            "driver",
            kind,
            &[],
            b::TABLES.to_string(),
            overrides,
        ));
        out.push(entry(
            format!("tables.{kind}.user"),
            "driver",
            kind,
            &[],
            b::TABLES_USER.to_string(),
            overrides,
        ));
        out.push(entry(
            format!("columns.{kind}"),
            "driver",
            kind,
            &[],
            b::COLUMNS.to_string(),
            overrides,
        ));
        out.push(entry(
            format!("columns.{kind}.user"),
            "driver",
            kind,
            &[],
            b::COLUMNS_USER.to_string(),
            overrides,
        ));
        out.push(entry(
            format!("indexes.{kind}"),
            "driver",
            kind,
            &[],
            b::INDEXES.to_string(),
            overrides,
        ));
        out.push(entry(
            format!("indexes.{kind}.user"),
            "driver",
            kind,
            &[],
            b::INDEXES_USER.to_string(),
            overrides,
        ));
        out.push(entry(
            format!("routines.{kind}"),
            "driver",
            kind,
            &[],
            b::ROUTINES.to_string(),
            overrides,
        ));
        out.push(entry(
            format!("routines.{kind}.user"),
            "driver",
            kind,
            &[],
            b::ROUTINES_USER.to_string(),
            overrides,
        ));
    }
}

#[cfg(not(feature = "oracle"))]
fn oracle_driver_specs(_out: &mut Vec<TemplateSpecEntry>, _overrides: &HashMap<String, String>) {}

/// 驱动层条目（databases / tables / columns / indexes / routines）
fn driver_builtin_specs(overrides: &HashMap<String, String>) -> Vec<TemplateSpecEntry> {
    let mut out = Vec::new();

    // MySQL / OceanBase（MySQL 模式）
    {
        use crate::db::mysql::builtin_sql as b;
        for kind in ["mysql", "oceanbase"] {
            out.push(entry(
                format!("databases.{kind}"),
                "driver",
                kind,
                &[],
                b::DATABASES.to_string(),
                overrides,
            ));
            out.push(entry(
                format!("tables.{kind}"),
                "driver",
                kind,
                &[],
                b::TABLES.to_string(),
                overrides,
            ));
            out.push(entry(
                format!("columns.{kind}"),
                "driver",
                kind,
                &[],
                b::COLUMNS.to_string(),
                overrides,
            ));
            out.push(entry(
                format!("indexes.{kind}"),
                "driver",
                kind,
                &[],
                b::INDEXES.to_string(),
                overrides,
            ));
            out.push(entry(
                format!("routines.{kind}"),
                "driver",
                kind,
                &[],
                b::ROUTINES.to_string(),
                overrides,
            ));
            out.push(entry(
                format!("routines.{kind}.return"),
                "driver",
                kind,
                &[],
                b::ROUTINE_RETURN.to_string(),
                overrides,
            ));
        }
    }

    // PostgreSQL
    {
        use crate::db::postgres::builtin_sql as b;
        out.push(entry(
            "databases.postgresql".into(),
            "driver",
            "postgresql",
            &[],
            b::DATABASES.to_string(),
            overrides,
        ));
        out.push(entry(
            "tables.postgresql".into(),
            "driver",
            "postgresql",
            &[],
            b::TABLES.to_string(),
            overrides,
        ));
        out.push(entry(
            "columns.postgresql".into(),
            "driver",
            "postgresql",
            &[],
            b::COLUMNS.to_string(),
            overrides,
        ));
        out.push(entry(
            "indexes.postgresql".into(),
            "driver",
            "postgresql",
            &[],
            b::INDEXES.to_string(),
            overrides,
        ));
        out.push(entry(
            "routines.postgresql".into(),
            "driver",
            "postgresql",
            &[],
            b::ROUTINES.to_string(),
            overrides,
        ));
    }

    // GaussDB
    {
        use crate::db::gaussdb::builtin_sql as b;
        out.push(entry(
            "databases.gaussdb".into(),
            "driver",
            "gaussdb",
            &[],
            b::DATABASES.to_string(),
            overrides,
        ));
        out.push(entry(
            "tables.gaussdb".into(),
            "driver",
            "gaussdb",
            &[],
            b::TABLES.to_string(),
            overrides,
        ));
        out.push(entry(
            "columns.gaussdb".into(),
            "driver",
            "gaussdb",
            &[],
            b::COLUMNS.to_string(),
            overrides,
        ));
        out.push(entry(
            "indexes.gaussdb".into(),
            "driver",
            "gaussdb",
            &[],
            b::INDEXES.to_string(),
            overrides,
        ));
        out.push(entry(
            "routines.gaussdb".into(),
            "driver",
            "gaussdb",
            &[],
            b::ROUTINES.to_string(),
            overrides,
        ));
    }

    // SQLite（databases 固定 ["main"] 无 SQL；无例程）
    {
        use crate::db::sqlite::builtin_sql as b;
        out.push(entry(
            "tables.sqlite".into(),
            "driver",
            "sqlite",
            &[],
            b::TABLES.to_string(),
            overrides,
        ));
        out.push(entry(
            "columns.sqlite".into(),
            "driver",
            "sqlite",
            &[],
            b::COLUMNS.to_string(),
            overrides,
        ));
        out.push(entry(
            "indexes.sqlite".into(),
            "driver",
            "sqlite",
            &[],
            b::INDEXES.to_string(),
            overrides,
        ));
        out.push(entry(
            "indexes.sqlite.info".into(),
            "driver",
            "sqlite",
            &[],
            b::INDEX_INFO.to_string(),
            overrides,
        ));
    }

    // Oracle / OceanBase Oracle（OCI 传输路径；feature 门控）
    oracle_driver_specs(&mut out, overrides);
    out
}

// ---------------------------------------------------------------------------
// 全量生效值（前端运行时使用；含 paged/preview）
// ---------------------------------------------------------------------------

/// 全量生效模板（key → 生效 SQL；覆盖优先，否则内置）
pub fn resolved() -> HashMap<String, String> {
    spec()
        .into_iter()
        .map(|e| {
            let effective = e
                .override_value
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(e.builtin);
            (e.key, effective)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_paged_alias_is_letter_first() {
        // 回归：别名必须字母开头（`_pdb_rn` 触发 ORA-00911）
        let tpl = builtin_paged("oracle");
        assert!(tpl.contains("ROWNUM"), "oracle 分页必须走 ROWNUM");
        assert!(tpl.contains("pdb_rn"));
        assert!(!tpl.contains("_pdb"), "禁止下划线开头别名");
        assert!(!tpl.contains("LIMIT"), "oracle 分页不得混入 LIMIT 语法");

        let tpl = builtin_paged("oceanbase-oracle");
        assert!(tpl.contains("ROWNUM"));
        assert!(!tpl.contains("_pdb"));

        // MySQL 系与 Oracle 系彻底隔离
        let mysql = builtin_paged("mysql");
        assert!(mysql.contains("LIMIT {limit} OFFSET {offset}"));
        assert!(!mysql.contains("ROWNUM"), "LIMIT 系分页不得混入 ROWNUM");
    }

    #[test]
    fn substitute_replaces_known_and_keeps_unknown() {
        let out = substitute("SELECT * FROM {table} LIMIT {limit} -- {unknown}", &[
            ("table", "\"t\""),
            ("limit", "10"),
        ]);
        assert_eq!(out, "SELECT * FROM \"t\" LIMIT 10 -- {unknown}");
    }

    #[test]
    fn paged_default_differs_from_oracle() {
        assert_ne!(builtin_paged("mysql"), builtin_paged("oracle"));
        assert_eq!(builtin_paged("mysql"), builtin_paged("postgresql"));
    }

    #[test]
    fn spec_and_resolved_expose_frontend_entries() {
        let entries = spec();
        let oracle = entries
            .iter()
            .find(|e| e.key == "paged.oracle")
            .expect("spec 必含 paged.oracle");
        assert_eq!(oracle.category, "paged");
        assert!(oracle.builtin.contains("ROWNUM"));
        assert!(oracle.placeholders.contains(&"sql".to_string()));
        assert!(entries.iter().any(|e| e.key == "paged.default"));
        assert!(entries.iter().any(|e| e.key == "preview.oracle"));
        assert!(entries.iter().any(|e| e.key == "previewRows.jdbc"));

        let resolved = resolved();
        let tpl = resolved
            .get("paged.oracle")
            .expect("resolved 必含 paged.oracle");
        assert!(tpl.contains("ROWNUM") && tpl.contains("pdb_rn"));
        assert!(!tpl.contains("_pdb"));
    }
}

//! 数据库类型注册表：新增数据库在此登记（kind → 元数据 + 能力）。

use serde::Serialize;

use crate::db::dialect;
use crate::db::objects::DbObjectType;
use crate::db::types::DbKind;

/// 前端展示用的类型信息（能力由方言推导）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KindInfo {
    pub kind: DbKind,
    pub label: &'static str,
    pub default_port: u16,
    /// 传输协议标识（协议兼容的数据库共享 transport）
    pub transport: &'static str,
    /// 该版本编译产物中是否可用（如 Oracle 依赖 feature）
    pub available: bool,
    pub experimental: bool,
    /// 用户名占位提示（OceanBase: user@tenant）
    pub username_hint: Option<&'static str>,
    pub format_language: &'static str,
    pub explain: bool,
    pub export_ddl: bool,
    pub export_insert: bool,
    /// 是否支持手动事务（自动/手动提交）
    pub transactions: bool,
    /// 是否支持服务器会话管理
    pub sessions: bool,
    pub object_types: Vec<DbObjectType>,
}

fn transport_of(kind: DbKind) -> (&'static str, bool, Option<&'static str>) {
    match kind {
        DbKind::MySql => ("mysql", false, None),
        DbKind::OceanBase => ("mysql", false, Some("user@tenant")),
        // OB Oracle 模式走 OCI 协议（需 OB 客户端库 / Instant Client），标记实验性
        DbKind::OceanBaseOracle => ("oracle", true, Some("user@tenant")),
        DbKind::PostgreSql => ("postgres", false, None),
        // GaussDB / openGauss 走官方 tokio-gaussdb（原生协议，支持 sha256 认证）
        DbKind::GaussDb => ("gaussdb", false, None),
        DbKind::Oracle => ("oracle", false, None),
        // SQLite：嵌入式文件数据库，文件路径存于 params.file
        DbKind::Sqlite => ("sqlite", false, None),
        // 通用 JDBC：驱动/URL 由驱动管理配置
        DbKind::Jdbc => ("jdbc", false, None),
    }
}

pub fn kind_info(kind: DbKind) -> KindInfo {
    let (transport, experimental, username_hint) = transport_of(kind);
    let cap = dialect::build(kind).ok().map(|d| d.capabilities());
    KindInfo {
        kind,
        label: kind.display_name(),
        default_port: kind.default_port(),
        transport,
        available: dialect::available(kind),
        experimental,
        username_hint,
        format_language: cap.as_ref().map(|c| c.format_language).unwrap_or("sql"),
        explain: cap.as_ref().map(|c| c.explain).unwrap_or(false),
        export_ddl: cap.as_ref().map(|c| c.export_ddl).unwrap_or(false),
        export_insert: cap.as_ref().map(|c| c.export_insert).unwrap_or(false),
        transactions: cap.as_ref().map(|c| c.transactions).unwrap_or(false),
        sessions: cap.as_ref().map(|c| c.sessions).unwrap_or(false),
        object_types: cap.map(|c| c.object_types).unwrap_or_default(),
    }
}

pub fn all_kinds() -> Vec<KindInfo> {
    [
        DbKind::MySql,
        DbKind::OceanBase,
        DbKind::OceanBaseOracle,
        DbKind::PostgreSql,
        DbKind::GaussDb,
        DbKind::Oracle,
        DbKind::Sqlite,
        DbKind::Jdbc,
    ]
    .into_iter()
    .map(kind_info)
    .collect()
}

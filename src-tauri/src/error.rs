use serde::Serialize;

pub type DbResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Db(String),
    #[error("连接会话不存在: {0}")]
    SessionNotFound(String),
    #[error("不支持的数据库类型: {0}")]
    #[allow(dead_code)] // 仅在未启用 oracle 特性时由 factory 分支构造
    UnsupportedKind(String),
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    General(String),
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl Drop for AppError {
    /// 任何未被妥善处理的错误在析构时统一落盘日志（零侵入捕获关键报错）。
    /// 注意：日志中不得包含密码等敏感信息。
    fn drop(&mut self) {
        let msg = self.to_string();
        // 可预期的服务端策略拒绝 / 配置问题（非程序缺陷）降为 WARN，避免日志噪音
        const EXPECTED_REJECTIONS: &[&str] = &[
            // OceanBase Oracle 模式租户不支持 MySQL 协议
            "Oracle tenant for current client driver is not supported",
            // 常见认证/对象不存在等配置或 SQL 类服务端拒绝
            "Access denied for user",
            "Unknown database",
            "password authentication failed",
            "does not exist",
        ];
        if EXPECTED_REJECTIONS.iter().any(|k| msg.contains(k)) {
            log::warn!(target: "app_error", "{msg}");
        } else {
            log::error!(target: "app_error", "{msg}");
        }
    }
}

impl From<mysql_async::Error> for AppError {
    fn from(e: mysql_async::Error) -> Self {
        AppError::Db(e.to_string())
    }
}

impl From<tokio_postgres::Error> for AppError {
    fn from(e: tokio_postgres::Error) -> Self {
        AppError::Db(e.to_string())
    }
}

impl From<tokio_gaussdb::Error> for AppError {
    fn from(e: tokio_gaussdb::Error) -> Self {
        AppError::Db(e.to_string())
    }
}

impl From<deadpool_postgres::PoolError> for AppError {
    fn from(e: deadpool_postgres::PoolError) -> Self {
        AppError::Db(format!("连接池错误: {e}"))
    }
}

impl From<deadpool_postgres::CreatePoolError> for AppError {
    fn from(e: deadpool_postgres::CreatePoolError) -> Self {
        AppError::Db(format!("创建连接池失败: {e}"))
    }
}

impl From<deadpool_postgres::BuildError> for AppError {
    fn from(e: deadpool_postgres::BuildError) -> Self {
        AppError::Db(format!("创建连接池失败: {e}"))
    }
}

#[cfg(feature = "oracle")]
impl From<oracle::Error> for AppError {
    fn from(e: oracle::Error) -> Self {
        AppError::Db(e.to_string())
    }
}

impl From<rust_xlsxwriter::XlsxError> for AppError {
    fn from(e: rust_xlsxwriter::XlsxError) -> Self {
        AppError::General(format!("Excel 导出错误: {e}"))
    }
}

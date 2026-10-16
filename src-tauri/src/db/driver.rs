use std::sync::Arc;

use async_trait::async_trait;

use crate::error::DbResult;

use super::dialect::SqlDialect;
use super::objects::{CatalogQuery, DbObjectType, ObjectInfo};
use super::types::{ColumnMeta, IndexInfo, QueryResult, RoutineInfo, ServerInfo, TableInfo};

/// 数据库驱动抽象：所有数据库实现统一接口，便于扩展新数据库类型。
///
/// `database` 参数语义：
/// - MySQL / OceanBase：schema（USE db）
/// - PostgreSQL：schema（SET search_path）
/// - Oracle：owner/schema（SQL 过滤），传 None 时使用当前用户
#[async_trait]
pub trait DatabaseDriver: Send + Sync {
    /// 探测服务器版本
    async fn server_info(&self) -> DbResult<ServerInfo>;

    /// 列出可用的数据库（Oracle 返回 schema/用户列表）
    async fn list_databases(&self) -> DbResult<Vec<String>>;

    /// 列出指定库 / schema 下的表与视图
    async fn list_tables(&self, database: Option<&str>) -> DbResult<Vec<TableInfo>>;

    /// 查看表结构
    async fn describe_table(
        &self,
        database: Option<&str>,
        table: &str,
    ) -> DbResult<Vec<ColumnMeta>>;

    /// 例程（存储过程 / 函数）元数据：参数列表与返回类型
    async fn describe_routine(
        &self,
        database: Option<&str>,
        name: &str,
        routine_type: DbObjectType,
    ) -> DbResult<RoutineInfo>;

    /// 索引列表（表结构查看用）
    async fn list_indexes(&self, database: Option<&str>, table: &str) -> DbResult<Vec<IndexInfo>>;

    /// 切换自动提交模式（关闭时进入手动事务，由专用连接承载）
    async fn set_autocommit(&self, enabled: bool) -> DbResult<()>;

    /// 提交当前手动事务
    async fn commit(&self) -> DbResult<()>;

    /// 回滚当前手动事务
    async fn rollback(&self) -> DbResult<()>;

    /// 执行 SQL（查询返回结果集，DML 返回影响行数）
    async fn execute(
        &self,
        database: Option<&str>,
        sql: &str,
        max_rows: u32,
    ) -> DbResult<QueryResult>;

    /// 保活探活（默认执行最小查询；桥接/OCI 驱动覆盖为专用 ping 命令）
    async fn ping(&self) -> DbResult<()> {
        self.execute(None, "SELECT 1", 1).await.map(|_| ())
    }

    /// 方言实例（SQL 片段与能力声明的来源）
    fn dialect(&self) -> Arc<dyn SqlDialect>;

    /// 执行方言生成的对象目录查询
    async fn run_catalog(
        &self,
        obj: DbObjectType,
        q: &CatalogQuery,
        schema: Option<&str>,
    ) -> DbResult<Vec<ObjectInfo>>;

    /// 执行方言生成的目录计数查询
    async fn run_catalog_count(&self, q: &CatalogQuery, schema: Option<&str>)
        -> DbResult<u32>;

    /// 执行单值文本查询（DDL 获取等），返回首行指定列；无结果返回空串
    async fn query_text(&self, sql: &str, params: &[String], value_index: usize)
        -> DbResult<String>;
}

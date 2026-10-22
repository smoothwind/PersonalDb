# PersonalDb 进阶功能设计方案（v2 — 面向多数据库扩展）

> 范围：品牌图标 / 方言 SQL 美化 / 执行计划 / 记录视图 / 类型化字段查看 / 多格式导出 / 对象浏览器分组
> v2 变更：引入 **Transport × Dialect 正交架构 + 注册表 + 能力声明**，保证未来以最小成本接入新数据库。
>
> 已确认决策：行列转换 = 记录视图；导出 = SQL(DDL+INSERT) + CSV + Excel

## 设计原则

1. **可扩展性第一**：新增数据库 = 注册一个条目 + 实现一个方言，**不修改任何现有代码**（开闭原则）。
2. **方言差异收敛**：所有按数据库分叉的逻辑（DDL/INSERT/执行计划/对象目录/分页）归入方言层，与连接传输层解耦。
3. **UI 由能力驱动**：前端菜单按后端下发的 capabilities 渲染，禁止 `switch(kind)` 硬编码功能开关。
4. **大结果集/大值保护**：`max_rows` 惰性拉取 + 单值尺寸阈值。

---

## 核心架构：Transport × Dialect 正交模型

### 关键洞察：协议兼容是常态

大量数据库复用现有协议，**连接驱动可复用，只有方言不同**：

| 数据库 | 传输协议（Transport） | SQL 方言（Dialect） |
| --- | --- | --- |
| MySQL | mysql_async | mysql |
| OceanBase | mysql_async | oceanbase（≈mysql + 租户规则） |
| TiDB（规划） | mysql_async | mysql（微调） |
| StarRocks/Doris（规划） | mysql_async | mysql/分析型微调 |
| PostgreSQL | tokio-postgres | postgresql |
| GaussDB / openGauss（已支持） | tokio-gaussdb（官方，支持 sha256） | postgresql（微调，见 SPEC-CONN-5） |
| KingbaseES（规划） | tokio-postgres | postgresql（微调） |
| Oracle | oracle (ODPI-C) | oracle |
| SQLite（规划） | rusqlite | sqlite |
| SQL Server（规划） | tiberius（纯 Rust） | tsql |
| 任意 ODBC 数据库（兜底） | odbc-api | 由用户选择已知方言 |

### Rust 目录结构演进

```
src-tauri/src/db/
├── registry.rs          # ★ 注册表：DbKind → (TransportFactory, Dialect) + 能力集
├── transport/           # 原 mysql.rs/postgres.rs/oracle.rs 改名：只管连接与执行
│   ├── mysql_async.rs
│   ├── postgres.rs
│   ├── oracle.rs
│   └── odbc.rs          # （远期）通用 ODBC 兜底通道
├── dialect/
│   ├── mod.rs           # SqlDialect trait + Capabilities
│   ├── mysql.rs
│   ├── oceanbase.rs     # 覆盖 mysql 方言的少量行为
│   ├── postgresql.rs
│   └── oracle.rs
├── driver.rs            # DatabaseDriver trait（对外统一接口，不变）
├── pool.rs              # 会话管理（通过 registry 创建 driver）
├── export.rs            # 只依赖 SqlDialect，无 kind 判断
├── plan.rs
└── objects.rs
```

### 注册表与工厂

```rust
// registry.rs
pub struct DbKindMeta {
    pub kind: DbKind,
    pub label: &'static str,
    pub default_port: u16,
    pub transport: TransportKind,      // MySql | Postgres | Oracle | Odbc
    pub dialect: DialectKind,
    pub experimental: bool,
}

pub static REGISTRY: &[DbKindMeta] = &[
    /* mysql, oceanbase, postgresql, oracle, ... 未来只在此追加 */
];

pub fn create_driver(cfg: &ConnectionConfig) -> DbResult<Arc<dyn DatabaseDriver>> {
    let meta = lookup(cfg.kind)?;
    let transport = transport::build(meta.transport, cfg)?;      // 协议复用
    let dialect = dialect::build(meta.dialect);                  // 方言独立
    Ok(Arc::new(GenericDriver::new(transport, dialect)))
}
```

`GenericDriver` 通用编排层：实现 `DatabaseDriver`，把每个 trait 方法拆解为「transport 原语 + dialect SQL 模板」，驱动实现从"每个数据库一套完整代码"变为"方言定义 SQL 片段，编排层复用"。

### SqlDialect trait（方言契约）

```rust
pub trait SqlDialect: Send + Sync {
    // —— 基础语法 ——
    fn capabilities(&self) -> Capabilities;              // 能力声明
    fn quote_ident(&self, ident: &str) -> String;        // `x` / "x" / "X"
    fn quote_string(&self, s: &str) -> String;           // 字面量转义（含反斜杠策略）
    fn literal(&self, v: &CellValue, sem: SemanticType) -> String;
                                                         // 日期 TO_DATE / 二进制 X'' / bool ...
    fn preview_rows_sql(&self, schema: Option<&str>, table: &str, limit: u32) -> String;
                                                         // LIMIT / FETCH FIRST / ROWNUM
    fn format_language(&self) -> &'static str;           // sql-formatter 语言

    // —— 执行计划 ——
    fn explain_sql(&self, sql: &str) -> String;
    fn parse_explain(&self, raw: &JsonValue) -> DbResult<PlanNode>;

    // —— 导出 ——
    fn table_ddl_query(&self, schema: Option<&str>, table: &str) -> DdlQuery;
                                                         // SHOW CREATE / information_schema / DBMS_METADATA
    fn insert_statement(&self, schema: Option<&str>, table: &str,
                        cols: &[ColumnInfo], batch: &[Vec<CellValue>]) -> String;
    fn export_preamble(&self) -> &'static str;           // 如 MySQL SET NO_BACKSLASH_ESCAPES=1

    // —— 对象目录 ——
    fn object_catalog(&self) -> &'static ObjectCatalog;  // 各对象类型的元数据 SQL 模板
}

pub struct Capabilities {
    pub explain: bool, pub export_ddl: bool, pub export_insert: bool,
    pub object_types: &'static [DbObjectType], pub record_view: bool, ...
}
```

### 能力集下发前端

新增命令 `db_registry() -> Vec<KindInfo>`（label/默认端口/能力集/方言格式化语言），前端启动时拉取：

- 连接表单的数据库类型下拉**动态生成**（新增数据库前端零改动）
- 右键菜单/按钮按 capabilities 显隐（无 explain 能力的库不显示"执行计划"）

### 前端注册表

```ts
// types/dbRegistry.ts —— 仅剩「展示层」信息，能力来自后端
export interface DbKindMeta {
  kind: DbKind
  logo: string                     // assets/logos 路径
  usernamePlaceholder?: string     // OB: user@tenant
  experimental?: boolean
}
```

`buildSelectSql` 等前端硬编码全部移除，改用后端 `db_preview_rows_sql(session_id, db, table)` 命令生成预览语句。

---

## 七项需求落点（v2 调整）

| 需求 | 落点 | 与 v1 差异 |
| --- | --- | --- |
| 1. 品牌图标 | 前端 `DbLogo.vue` + 注册表 logo 配置 | 图标映射从 switch 改为注册表查找 |
| 2. SQL 美化 | sql-formatter，语言取自 `db_registry` 返回的 `formatLanguage` | 方言映射不再前端硬编码 |
| 3. 执行计划 | `GenericDriver.explain` = dialect.explain_sql + transport 执行 + dialect.parse_explain | MySQL/PG 完整树；ODBC/未知方言 capabilities.explain=false 自动隐藏 |
| 4. 记录视图 | 纯前端，能力 `record_view`（恒 true） | 无差异 |
| 5. 类型化查看 | SemanticType 由各 transport 推断 + dialect 可覆盖（如 Oracle NUMBER→Decimal） | 推断规则与 transport 绑定，方言不重复实现 |
| 6. 导出 | export.rs 纯粹调用 `dialect.literal/insert_statement/table_ddl_query` | 从"每个驱动实现导出"改为"方言声明 SQL 片段，导出引擎统一" |
| 7. 对象分组 | objects.rs 读 `dialect.object_catalog()` 模板执行 | 元数据 SQL 集中在各方言文件，一目了然 |

## 候选数据库路线图（按接入成本）

| 成本 | 数据库 | 说明 |
| --- | --- | --- |
| ≈0 天 | TiDB / StarRocks / Doris | mysql 方言 + 默认端口差异，仅注册表加条目 |
| ≈0 天 | KingbaseES | postgresql 方言微调（GaussDB 已支持，见 SPEC-CONN-5） |
| 1~2 天 | SQLite | rusqlite 传输 + sqlite 方言（无服务端/无库列表概念） |
| 2~3 天 | SQL Server | tiberius 纯 Rust + tsql 方言（TOP/OFFSET-FETCH/方括号） |
| 3~5 天 | 达梦 DM | ODBC 通道 + oracle 方言微调；或官方驱动 |
| 兜底 | ODBC 通用连接 | 用户填连接串+选方言，覆盖长尾（Access/Excel/未知库） |

## 分期计划（v2）

| 阶段 | 内容 | 预估 |
| --- | --- | --- |
| P0 注册表重构 | transport/dialect 拆分、registry.rs、capabilities 下发、前端动态化（**后续一切的地基**） | 2 天 |
| P1 对象浏览器 | 品牌图标 + 树分组（基于 dialect.object_catalog） | 2~3 天 |
| P2 结果体验 | SemanticType + 查看组件 + 记录视图 | 2~3 天 |
| P3 导出 | export.rs（方言驱动）+ dialog/fs 插件 + ExportDialog | 3 天 |
| P4 高级 | 美化 + 执行计划 | 2 天 |

## 新增依赖

- npm：`sql-formatter`
- cargo：`rust_xlsxwriter`（P3）
- Tauri 插件：`tauri-plugin-dialog`、`tauri-plugin-fs`（P3）

## 风险与限制

1. **P0 重构风险**：transport/dialect 拆分涉及现有 4 个驱动改造，需在 `cargo check` 双配置 + 真实连接冒烟后合并。
2. PG 自拼 DDL 为简化版；Oracle 计划文本解析脆弱；超大 BLOB（>10MB）拒绝预览——同 v1。
3. ODBC 通道（远期）：Windows 现成，但类型系统弱、游标差异大，仅作为兜底而非主路径。
4. MySQL 反斜杠转义：导出文件头写 `SET NO_BACKSLASH_ESCAPES=1;` 保证可重放（由 dialect.export_preamble 声明）。

---

## 后续演进（v2 之后落地）

### 1. 内置桥接驱动（免 OCI）
真实 Oracle 与 OceanBase Oracle 通过 **sidecar + NDJSON over stdio** 桥接（`db/bridge.rs` + `db/bridge_driver.rs`）：

- **oracle-bridge**（Go + go-ora）：真实 Oracle 的纯 Go 协议实现，运行时无需 Instant Client；`build.rs` 调 Go 工具链构建。
- **jdbc-bridge**（Java + OceanBase Connector/J）：OceanBase Oracle 模式。**OceanBase 的 Oracle 模式本质是 MySQL 协议 + OB 扩展能力位**（`OB_CLIENT_SUPPORT_ORACLE_MODE`），标准 Oracle TNS 客户端（含 go-ora）无法连接，故用官方 JDBC 驱动；`build.rs` 调 Maven 打包 fat jar。
- 选择规则（`pool.rs#create_driver`）：`params.driver == "oci"` → `oracle.rs`（OCI）；否则内置桥接可用则用之；缺失回退 OCI。
- 占位符：Java 桥把 Oracle 风格 `:N` 转 JDBC `?` 时**按序号重排参数**；OCI 路径 SQL 一律按 `:1, :2, …` 升序书写（OCI 按出现顺序绑定）。

### 2. 通用 JDBC 连接（驱动管理）
- `DbKind::Jdbc` + `db/jdbc.rs`（传输）+ `dialect/generic.rs`（保守 ANSI 方言）
- 驱动配置 `db/jdbc_driver.rs` → `drivers.json`（名称/驱动类/URL 模板/端口/本地 jar），**仅本地来源**
- 对象树走 JDBC `DatabaseMetaData`（`metaTables/metaColumns/metaIndexes/...`）

### 3. 结果集与编辑器能力
- **分页续拉**：首页 `pageSize`（默认 10），滚动触底 `tabs.loadMore` 以 SQL 包装分页（Oracle ROWNUM / 其余 LIMIT OFFSET），总量上限 `maxTotalRows`。
- **结果集编辑**：单表 SELECT 判定 + 主键/ROWID/全列定位；`db_build_dml` 生成语句 → 预览确认 → 事务提交。
- **导出**：`export_result_rows`（Excel/SQL INSERT/CSV/TXT），SQL 选项含去引号/大小写/日期显式转换/合并行数/**值转换规则**（TO_CLOB、NVL、COALESCE 模板）。
- **自动补全**：`db_schema_snapshot`（全方言；MySQL/PG/Oracle 走系统视图，JDBC 走元数据），按列类型绘制矢量图标、内联展示注释。
- **美化器**：自研 `utils/plsqlBeautifier.ts`，对齐 PL/SQL Developer Beautifier 规则（大小写/元素格式/对齐/逗号/AND-OR/JOIN-ON/块缩进等）。

### 4. 界面布局
- 页签栏位置：顶部 / 左侧 / 右侧（竖向支持拖拽排序）；左侧可与连接树**互斥切换**或**上下分栏**。
- 分屏：左右 / 上下，可拖拽分隔条，分屏时仅显示相关页签。
- 通用面板拖拽、全局字体/行高、事务工具栏、驱动管理、关于窗体等。

### 5. 打包
- `npm run bundle`（MSI）/ `npm run bundle:nsis`；`build.rs` 自动构建 Go/Java sidecar 到 `src-tauri/binaries/` 并经 `bundle.resources` 打包。

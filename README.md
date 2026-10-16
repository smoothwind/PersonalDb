# PersonalDb

基于 **Tauri 2 + WebView2 + Vue 3** 的多数据库客户端工具（Windows 桌面应用）。

## 功能特性

- **多数据库**：MySQL / OceanBase（MySQL、Oracle 模式）/ PostgreSQL / Oracle / 任意 JDBC
  - Oracle 免装 OCI（内置 Go go-ora）；OceanBase Oracle 走内置 JDBC（OceanBase Connector/J）
- **密码安全**：连接密码存于 **Windows 凭据管理器**（keyring），配置文件仅存引用；启动自动迁移明文
- **配置互通**：导出/导入自身配置（可选 AES-GCM 口令加密）；**从 DBeaver / Navicat(.ncx) / PL-SQL Developer 导入连接**（密码解密 best-effort）
- **会话管理**：活动会话查看（自动刷新/筛选/**阻塞链**/**锁·事务视图**/摘要统计），取消查询 / 终止会话（Oracle 双模式、批量终止、Kill 跟踪）、会话详情、**CSV 导出**、>300 行虚拟化、列序持久化（MySQL / PostgreSQL / Oracle / OB）
- **漫游指导**：driver.js 功能引导（总览/查询/会话），首次启动自动播放
- **连接管理**：分组文件夹、搜索、拖拽归类、快速重连（状态栏一键重连全部）、连接离线去重提醒
- **对象浏览器**：库/表/视图/索引/序列/函数/存储过程/触发器；对象状态角标（如 Oracle INVALID）、分区表标识
- **对象查看 / 编辑**：表结构与源码、对象定义、例程编辑（均为 Tab）；**默认只读，点「编辑」才可改**
- **SQL 编辑器**：CodeMirror 6、方言高亮、**自动补全**（同 schema 表/列 + 注释、按列类型图标）、**自定义美化器**（对齐 PL/SQL Developer Beautifier 规则）、快捷指令、执行范围（全部/选中/当前行）、保存/加载 SQL 文件、**Ctrl+F 搜索面板**、**Ctrl+滚轮字号缩放**
- **结果集**：分页续拉、**大结果自动虚拟滚动（>600 行）**、**多语句多结果集切换**、筛选（关键字/**列值筛选**）/排序/全局查找、**统计行**、**列宽持久化**、显示列（**拖拽排序**/移首尾/还原）、**粘贴建行**、URL 链接化、长文/JSON 悬浮卡、行高固定（tooltip 展示长文本）、多选复制、导出（Excel/SQL INSERT/CSV/TXT/**JSON** + 值转换规则）、结果编辑（增删改 + 预览 + 撤销/重做 + 高危确认 + 事务提交）、记录视图、单元格详情
- **查询历史**：SQLite 本地持久化（500 条），搜索、**仅当前连接**、单击回填、**双击立即执行**
- **命令面板**：Ctrl+K 全局搜索命令/连接/最近 SQL
- **事务**：自动 / 手动提交（主窗体工具栏；OB Oracle 非 Send 连接自动提交）
- **布局与外观**：页签栏位置（顶部/左侧/右侧，竖向可拖拽排序）、**分屏（左右/上下）**、面板拖拽、可调字体与行高、主题（浅/深）、国际化（中/英）
- **驱动管理**：JDBC 驱动 CRUD（驱动类 / URL 模板 / 本地 jar / 查找类）
- **日志**：文件日志自动轮转 + 内置日志查看器

## 支持的数据库

| 数据库                   | 协议 / 驱动                                                          | 默认端口   | 说明                                                                                    |
| ------------------------ | -------------------------------------------------------------------- | ---------- | --------------------------------------------------------------------------------------- |
| MySQL 5.7 / 8.x          | `mysql_async`（Rust 原生）                                         | 3306       | 开箱即用                                                                                |
| OceanBase（MySQL 模式）  | `mysql_async`                                                      | 2881       | 用户名按 OB 规范：`user@tenant[#cluster]`                                             |
| OceanBase（Oracle 模式） | 内置 JDBC 驱动（OceanBase Connector/J）/ `oracle` crate 0.6（OCI） | 2881       | **实验性**；默认内置驱动免装 OCI（需 JRE 21+），用户名 `用户@租户`，Schema 留空 |
| PostgreSQL 12+           | `tokio-postgres` + deadpool                                        | 5432       | 开箱即用                                                                                |
| Oracle 11g+              | 内置 Go 驱动（go-ora）/ `oracle` crate 0.6（ODPI-C）               | 1521       | 默认内置驱动**免安装 OCI**；可选 OCI 方式                                         |
| JDBC（任意库）           | 通用 JDBC 桥（Java）+ 用户驱动 jar                                   | 由驱动决定 | **驱动管理**（工具 → 驱动管理）：自定义驱动类/URL 模板/本地 jar                  |

## 开发环境要求

- Node.js 20+
- Rust 1.85+（edition 2024）
- **Go 1.21+**（构建内置 Oracle 驱动 `oracle-bridge`；缺失则跳过并回退 OCI）
- **Java 21（JRE/JDK）+ Maven**（构建 OceanBase Oracle 驱动 `jdbc-bridge`；缺失则跳过并回退 OCI）
- Windows：MSVC 工具链 + WebView2（Win10/11 自带）
- Windows 资源编译器 `rc.exe`：项目已在 `src-tauri/.cargo/config.toml` 中指定 Windows SDK 路径，若 SDK 版本不同请自行调整

## 常用命令

```bash
npm install          # 安装前端依赖
npm run tauri dev    # 开发调试（热更新）
npm run build        # 类型检查 + 前端打包
npm run bundle       # 打包安装包（MSI，输出 target/release/bundle/msi）
npm run bundle:nsis  # 打包 NSIS 安装器（需 NSIS 工具缓存）
```

> 打包首次会下载 WiX/NSIS 工具；若 GitHub 受限，可将工具预置到 `%LOCALAPPDATA%\tauri\WixTools314`（MSI）等缓存目录后再执行。

### Oracle / OceanBase Oracle 支持

**默认走内置驱动，免安装 OCI**，运行时经 NDJSON over stdio 桥接执行：

- **Oracle**：`oracle-bridge`（`src-tauri/oracle-bridge/`，纯 Go 的 [go-ora](https://github.com/sijms/go-ora)）→ 无需 Oracle Instant Client
- **OceanBase Oracle 模式**：`jdbc-bridge`（`src-tauri/jdbc-bridge/`，Java + OceanBase Connector/J）→ 需 **JRE 21+**（经 JAVA_HOME/PATH 定位 java）
  - OceanBase 的 Oracle 模式实际是 **MySQL 协议 + OB 扩展能力位**，标准 Oracle TNS 客户端（含 go-ora）无法连接，故使用官方 JDBC 驱动
  - 连接要点：用户名填 **`用户@租户`**（如 `myuser@mytenant`），**Schema 留空**（Oracle 模式 schema 默认 = 用户名）

连接表单中的「连接方式」可选：

- **内置驱动**（默认）：开箱即用，无需任何客户端库
- **OCI 客户端**：使用 `oracle` crate 的 ODPI-C（运行时动态加载 OCI），需准备客户端库之一并加入 `PATH`（可选 `OCI_LIB_DIR`）
  - Oracle：[Oracle Instant Client](https://www.oracle.com/database/technologies/instant-client.html)
  - OceanBase Oracle 模式：OB 的 `libobclient`（随 OB 客户端工具提供）

未检测到 Go / Maven 时 `build.rs` 会跳过对应内置驱动（仅告警），运行时自动回退 OCI；也可排除 OCI 支持以减小体积：

```bash
cargo check --no-default-features   # src-tauri 目录（仍可使用内置驱动）
```

### 通用 JDBC（驱动管理）

支持以任意 JDBC 驱动连接数据库（参考 DBeaver/Navicat）：

- 入口：**工具 → 驱动管理**。可新建/复制/编辑/删除驱动：名称、**驱动类**、**URL 模板**（如 `jdbc:mysql://{host}:{port}/{database}`）、默认端口、**驱动 jar/目录**（本地添加）、**查找类**（扫描 jar 列出驱动类）
- 驱动来源：仅本地 jar 文件/目录（由数据库厂商提供，不联网下载）
- 连接：连接表单选择「JDBC」类型并选目标驱动；对象树经 JDBC `DatabaseMetaData` 浏览（通用 ANSI 方言）
- 内置预设：OceanBase（驱动随程序打包，免配置）

## 项目结构

```
src/                        # Vue 3 前端
├── api/db.ts               # Tauri invoke 封装
├── stores/                 # connections / tabs / history(SQLite 后端化) / preferences / workspaces…
├── types/db.ts             # 与 Rust 端对齐的类型定义
├── utils/                  # icons(自绘 SVG) / objectIcons / plsqlBeautifier / clipboard / notify…
├── tours/                  # driver.js 漫游指导脚本
└── components/             # QueryTabPane / SessionPane / TableInspectorPane / CommandPalette / HistoryPanel…

src-tauri/src/              # Rust 后端
├── db/
│   ├── driver.rs           # DatabaseDriver trait（统一抽象）
│   ├── dialect/            # SQL 方言层（mysql_family / postgresql / oracle / generic）
│   ├── mysql.rs            # MySQL / OceanBase 实现
│   ├── postgres.rs         # PostgreSQL 实现
│   ├── oracle.rs           # Oracle OCI 实现（feature-gated）
│   ├── bridge.rs           # 桥接进程管理与 NDJSON 通信（oracle-bridge / jdbc-bridge）
│   ├── bridge_driver.rs    # 内置桥接驱动（go-ora / OceanBase Connector/J）
│   ├── jdbc.rs / jdbc_driver.rs  # 通用 JDBC 连接 + 驱动管理
│   ├── dml.rs / export.rs  # 结果集编辑 DML / 导出引擎
│   ├── pool.rs             # 驱动工厂 + 会话管理器 + 保活
│   ├── registry.rs         # 数据库类型注册表（能力下发）
│   └── types.rs            # 连接配置 / 结果集等通用类型
├── secrets.rs              # 密码存 Windows 凭据管理器（keyring）
├── vault.rs                # 配置导出加密（AES-256-GCM + PBKDF2）
├── import/                 # 三方导入（DBeaver / Navicat / PLSQL Dev）
├── history.rs              # 查询历史（SQLite）
├── commands.rs             # Tauri 命令层（前端 invoke 入口）
└── error.rs                # 统一错误类型

src-tauri/oracle-bridge/    # Go sidecar（go-ora）：免 OCI 的 Oracle 协议实现
src-tauri/jdbc-bridge/      # Java sidecar（OceanBase Connector/J）：OceanBase Oracle 模式（需 JRE 21+）
```

## 扩展新数据库

1. 在 `src-tauri/src/db/types.rs` 的 `DbKind` 增加变体与默认端口
2. 实现 `db/dialect/<name>.rs`（`SqlDialect`）并在 `dialect::build` 注册
3. 在 `db/registry.rs` 登记 `KindInfo`（能力下发前端）
4. 新建 `db/<name>.rs` 实现 `DatabaseDriver`（或复用既有传输）
5. 在 `db/pool.rs` 的 `create_driver` 注册分支
6. 前端 `src/types/db.ts` 的 `DbKind` / `DB_KIND_LABELS` / `DB_DEFAULT_PORTS` 同步增加
7. 图标映射 `src/utils/objectIcons.ts` 与 `DbLogo.vue` 同步

## 数据存储位置

所有数据文件**不使用 %APPDATA%**，按类别管理（`src-tauri/src/storage.rs`），可在「首选项 → 存储」中修改并自动迁移：

| 类别      | 默认位置                         | 文件                             |
| --------- | -------------------------------- | -------------------------------- |
| 连接配置  | `{安装目录}\data\connections\` | connections.json、groups.json    |
| 快捷指令  | `{安装目录}\data\snippets\`    | snippets.json                    |
| 工作区    | `{安装目录}\data\workspaces\`  | workspaces.json                  |
| JDBC 驱动 | `{安装目录}\data\drivers\`     | drivers.json                     |
| 日志      | `{安装目录}\data\logs\`        | personaldb.log（512KB 自动轮转） |

- **开发模式**（debug 构建）：运行时文件统一放**项目根 `.devdata\`**（已加入 .gitignore）
- 存储设置本体存于基础目录 `settings.json`；安装目录不可写（如 Program Files）时自动回退 `%LOCALAPPDATA%` 并记录日志
- 首版遗留的 `%APPDATA%` 数据在启动时**一次性自动迁移**至新位置
- 日志目录变更需**重启应用**生效

## 安全说明

- 连接配置（含密码）以明文 JSON 存储于上述存储目录，后续可升级为系统凭据保护（Windows Credential Manager）；**日志不会打印密码**
- Oracle 特性未启用时，Oracle 连接会返回明确的错误提示

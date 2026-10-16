# AGENTS.md

PersonalDb — 基于 Tauri 2 + WebView2 + Vue 3 的多数据库客户端桌面工具（Windows）。

## 常用命令

```bash
# 前端（根目录）
npm run build            # vue-tsc 类型检查 + vite 打包（提交前必跑）
npm test                 # Vitest 单测（jsdom，含 About 对话框/连接分组回归）
npm run dev              # 仅前端 Vite 开发服务器
npm run tauri dev        # 完整桌面应用开发调试
npm run bundle:all       # 打包 MSI + NSIS（CI bundle 作业用；本地需 Go + Maven + JRE 21）

# Rust（src-tauri 目录）
cargo check                        # 后端类型检查（默认已含 oracle 特性，提交前必跑）
cargo check --no-default-features  # 排除 oracle 的路径检查（改动 cfg 分支时跑）
cargo fix --lib --allow-dirty      # 自动修复 warning
```

构建必须同时通过：`npm run build` 与 `cargo check`（默认 + `--no-default-features` 两种配置均需零 error 零 warning）。CI（`.github/workflows/ci.yml`）为 `check` + `bundle` 两个作业：push（main/master、`v*` 标签）与 PR 均触发；`bundle` 在 windows-latest 上经 Go/Maven 产出 MSI + NSIS 并上传 Artifact，`v*` 标签附到草稿 Release。

## 架构

- `src/` Vue 3 前端（单窗口布局，无路由）：`App.vue`（**菜单栏（最顶）→ 工具栏 → 连接树 → 查询 Tabs → 状态栏**，主题切换 + 首选项）、`api/db.ts`（invoke 封装）、`stores/`（connections 多会话 Map / tabs 查询 Tab / snippets 快捷指令 / preferences 偏好）、`types/db.ts`（与 Rust serde camelCase 对齐的类型）、`utils/objectIcons.ts`（**对象类型→图标统一映射**）、`components/`（ConnectionTree 连接树（对象分组/搜索/右键菜单）、QueryTabPane 查询面板（可编辑 SQL + 结果集增强）、**TableInspectorPane 表查看面板（列/索引/源码 Tab，支持编辑与变更预览执行）**、ObjectInspector 只读对象查看器（非表对象定义）、RoutineEditor 例程编辑器（存储过程/函数）、CodeViewer 只读代码视图、SqlEditor 可编辑 CM6 编辑器、SnippetSettings、ConnectionForm、DbLogo、WelcomePane）
- **Tab 类型约定**：`QueryTab.type` 为 `'query'`（查询编辑器）/ `'table'`（表查看面板）/ `'object'`（对象定义查看）/ `'routine'`（例程编辑器）/ `'session'`（数据库会话管理）；App.vue 按 type 分派 `QueryTabPane` / `TableInspectorPane` / `ObjectInspector` / `RoutineEditor` / `SessionPane`；**对象查看与例程编辑一律以 Tab 形式，禁止抽屉组件**；面板经 `openTableInspector/openObjectInspector/openRoutineEditor/openSessionManager`（按连接+库+对象去重）
- 交互约定：连接树单击表仅选中，双击表/视图打开 SQL 编辑器并生成预览 SQL（不自动执行）；**双击连接节点=连接并加载库列表；树展开不作为连接触发器**（未连接节点展开为空，防断开后刷新/启动恢复隐式重连），右键未连接节点首项为「连接」；右键节点管理（新建查询/编辑/断开/删除/导出/新建表/查看结构/查看源码/编辑例程/重新编译）；**表的结构与源码走「表查看面板」Tab，非表对象定义走「对象查看面板」Tab，例程走「例程编辑」Tab（均为 Tab，禁用抽屉）**；查询 Tab 只承担可编辑 SQL（选中执行、快捷指令展开、方言格式化、执行计划）；**查询页签双击标题可重命名**
- 事务约定：**事务管理位于主窗体工具栏**（App.vue，紧凑：分段控件 + 提交/回滚图标按钮），作用于当前激活查询 Tab 所属连接，按 `capabilities.transactions` 显隐；手动模式由**会话专用连接**承载事务（MySQL/PG 支持；Oracle 非 Send 连接不支持 → 始终自动提交，且 DML 后须显式 `conn.commit()`，否则变更不生效）
- 查询面板工具栏：按钮一律**图标化（circle + tooltip）**并保持紧凑，不再放置事务控件
- 结果集约定：斑马纹与行高三档（偏好）、列拖拽排序（sortablejs，固定列时禁用）、固定列/筛选（列设置弹层）、全局查找、列排序；列级设置存 `colSettings`（format/fixed/filter）；**分页续拉**（`preferences.result.pageSize` 默认 10 / `maxTotalRows` 默认 10000，滚动触底 `tabs.loadMore` 走 SQL 包装分页）；**多选 + 复制（单元格/整行/本列/选区）+ 导出**（Excel/SQL INSERT/CSV/TXT，`export_result_rows`，SQL 选项与值转换规则默认取首选项）；列显隐经回显列弹层（`hiddenProps`，**暂存后确认再重绘**）；**结果集编辑**（单表 SELECT；主键/ROWID/全列定位；`db_build_dml` 预览 + 事务提交）
- 图标约定：树节点与 Tab 页签图标一律取自 `utils/objectIcons.ts`（禁止在组件内散落图标判断）；`QueryTab.icon` 记录页签图标 key；菜单栏各项图标取自 `App.vue#MENU_ICONS` 映射表（**同一动作在工具栏/菜单/命令面板必须同图标**，见 SPEC-UI-8/SPEC-UI-29）；**自绘 SVG 图标库在 `utils/icons.ts`（obj-*/act-*/st-* 三类，24×24 线性 currentColor）+ `components/AppIcon.vue` 渲染**，`objectIcon()` 返回 AppIcon 代理组件（1em，兼容 el-icon）；`objectIcon(key)` 解析优先级 = `APP_ICONS` 直连 key（`obj-`/`act-`/`st-`，如 `objectIcon('act_run')`）> 对象类型别名（`table` → `obj-*`）> 兜底 `obj_query`（**禁止让未登记 key 静默回退，历史上导致动作图标全渲染成查询图标**）
- **图标使用红线（强制，见 SPEC-UI-8）**：① **禁止不同语义复用同一图标**——一个图标全局只表达一种语义，同语义必须同图标（如最小化在主窗与悬浮窗都用 `Minus`；隐藏面板在面板头/悬浮窗/日志面板都用 `act_hide`）；② **禁止可能造成用户困惑的图标**，尤其禁止与主窗标题栏「最小化 `Minus` / 最大化 `FullScreen` / 关闭 `Close`」混淆——面板级动作用自绘 `act_float` / `act_hide` / `act_dock`；③ 新增图标前先查 `APP_ICONS`，同语义必须复用；语义→图标唯一映射表见 `docs/specifications.md` SPEC-UI-8
- 密码凭据约定：**明文密码不落盘**——密码存系统凭据管理器（`secrets.rs`，keyring/Windows Credential Manager），`ConnectionConfig.passwordRef` 存引用、`password` 恒空；连接/测试连接经 `commands::resolve_secret` 自动取回，改密自动回写，复制连接凭据随复制（`secret_copy`），删除连接清理凭据；启动 `migrate_connection_file` 自动迁移历史明文（失败回退明文 + WARN）；`connections.ts#upsert` 是保存侧唯一 choke point（新密码→`secret_store`→清空明文）
- 配置互通约定：导出/导入走「工具 → 配置导入 / 导出」（`ConfigTransferDialog.vue` 三 Tab）；**含密码必须 AES-256-GCM + PBKDF2 口令加密**（`vault.rs`，信封 format=personaldb-vault）；三方导入（`src-tauri/src/import/`：DBeaver JSON / Navicat ncx+Blowfish / PLSQL Dev INI+注册表）密码 best-effort，失败置空提示
- 连接错误约定：连接失败一律经 `utils/connError.ts#describeConnError` 输出**一行可操作摘要**（toast / 抛出），原始文本走 `connErrorDetail` 进日志详情（`摘要\n原始`）；规则表覆盖 Windows 套接字码（10060/10061/10065/11001）、Oracle ORA 码（12541/12514/12505/12154/12170/01017/28000）、驱动文案与 OB 租户/驱动缺失，未命中则剥离包装前缀（`数据库错误:`/`Input/output error:`/`Error:`）后原样；**连接入口禁止直接 `String(e)`**（见 SPEC-CONN-1）
- 会话管理约定：能力 `capabilities.sessions`；方言 `server_sessions_sql`（固定列序 id,user,host,program,state,secs,extra,sqlText,blocking）+ `kill_session_sql(id, cancel|kill)`（**id 严格数字校验防注入**）+ `server_locks_sql`（动态列）；**OceanBase Oracle 走 OB 兼容视图 `V$OB_PROCESSLIST`/`V$OB_LOCKS`（`OracleDialect{ob}` 分支，终止按 ID），标准 Oracle 走 v$session/v$lock**；UI 为 Tab（`SessionPane.vue`：锁视图/详情/批量终止/CSV/虚拟化>300 行）；入口=连接右键+主工具栏（能力显隐）
- Splash 约定：主窗 `visible:false`，`public/splash.html` 静态小窗先显示；前端挂载完成 `invoke('app_ready')` → 后端关 splash 并 show 主窗（`commands.rs#app_ready`）
- 漫游指导约定：driver.js；锚点元素加 `data-tour="<key>"`，tour 定义在 `src/tours/index.ts`（隐藏锚点自动剔除，文案全 i18n）；入口=帮助菜单；首次启动自动播放（localStorage `personaldb.tourDone`）
- 性能约定：**重型面板一律 `defineAsyncComponent`**（QueryTabPane/TableInspectorPane/ObjectInspector/RoutineEditor/SessionPane/各对话框）；分包经 `vite.config.ts#rolldownOptions.output.advancedChunks`（Vite 8/rolldown，非 rollup manualChunks）
- 保活约定：`ConnectionManager::spawn_keepalive` 每 55s 对全部会话 `driver.ping()`（默认 `SELECT 1`；桥接走 ping 命令、Oracle `SELECT 1 FROM DUAL`），失败仅 WARN 不踢会话
- 查询历史约定：`stores/history.ts`（localStorage `personaldb.history`，上限 500 条、SQL 截断 2000 字符）；`tabs.runTab` 成败均入栈；UI 为查询工具栏 `HistoryPanel.vue`（点击回填 `tab.sql`）
- 命令面板约定：Ctrl+K（`onGlobalKeydown` 前置拦截，编辑器聚焦时也生效）；`CommandPalette.vue` 接收 `commands`（应用命令/连接/最近 SQL 注入），Esc 关闭
- 设计令牌约定：全局间距/圆角/高度变量在 `styles/tokens.css`（`--pd-*`），新样式优先取令牌；`.conn-dot`（页签连接色点，`utils/color.ts#connColor` 按 id 哈希取色）为共享原子类；**功能带/区间边界**视觉见 SPEC-UI-25（`--pd-band-bg`/`--pd-band-border`，带=chrome、内容=画布）
- 工作台布局约定：四区停靠 left/right/bottom/center（`stores/layout.ts`，**center 常驻**）+ 悬浮窗层叠（FloatLayer）；**主窗左侧常驻导航条 `components/workbench/PanelRail.vue` 恒定吸附窗口最左**（`--pd-rail-w`，Activity Bar 模式：面板显隐/唤回入口 + 日志未读徽标，**可见=主色+淡底+左缘指示条 / 隐藏=灰阶压暗**，SPEC-UI-26），拖拽左缘热区从 `--pd-rail-w` 起算；面板头 `PanelHeader` 拖拽重排、双击 停靠⇄悬浮（**落点仅响应面板拖拽**，否则外来 HTML5 拖拽会卡住遮罩）；**功能区仅顶部页签栏**（`TabArea.vue`，竖排页签栏已废弃），面板级操作统一走 `App.vue#togglePanel`
- 空状态约定：统一用 `components/EmptyState.vue`（icon=APP_ICONS key + title/hint/action 插槽），禁止散落 el-empty/自绘占位
- 右键菜单约定：**唯一实现 `components/ContextMenu.vue`**（Teleport + 视口收边 + 全局收起/Esc 集中实现），`:items="CtxMenuItem[]"` + `@select(key)`；**禁止新增自绘右键菜单或自行注册收起监听**（SPEC-UI-31）
- 图标尺寸/焦点环约定：尺寸走 `styles/tokens.css` 的 `--pd-icon-sm/md/lg`（13/15/17，新代码禁止散落 `:size` 数字）；自绘 button 必须带 `:focus-visible` 焦点环（SPEC-UI-30）
- i18n 校验约定：`npm run check:i18n`（`scripts/check-i18n.mts`，node strip-types 递归对比 zh/en 叶子 key，缺失即非零退出；**需 Node 22.6+**，Node 20 报 `bad option: --experimental-strip-types`）；CI（`.github/workflows/ci.yml`）强制执行
- 虚拟滚动约定：结果集 >600 行且非编辑态走 `ResultVirtualGrid.vue`（el-table-v2，排序+右键复制），否则完整 el-table；行对象按 `c{count}_{name}` prop 键，虚拟模式需转矩阵
- 多结果集约定：非 Oracle 连接 `splitStatements` 分割后逐条执行（`runMultiStatements`），`tab.multiResults/activeResultIndex` + segmented 切换；Oracle 含 PL/SQL 块跳过分割
- 编辑撤销约定：`editUndoStack/editRedoStack` 全量快照（上限 50），五类变更入口均需 `pushEditUndo()`；Ctrl+Z/Ctrl+Y 经 `onEditKeydown`（编辑态且非单元格输入中）
- 历史存储约定：SQLite（`history.rs`，存储类别 History/`history.db`，上限 500，WAL）；`stores/history.ts` 后端化（旧 localStorage 首次迁移），`push` 乐观插入
- 更新约定：updater/process 插件已接入但**未启用端点**（启用步骤见 SPEC-DEV-2）；签名密钥 `%USERPROFILE%\.personaldb\tauri-updater.key`（口令 personaldb），启用前打包不受影响
- 日志约定：后端 `tauri-plugin-log`（stdout + `logs/personaldb.log` 自动轮转 + WebView 控制台；debug 构建 Debug 级、release Info 级）；`AppError` 实现 Drop 日志（未处理错误零侵入落盘；**可预期的服务端拒绝/配置类错误降级为 WARN**，如 OB Oracle 租户 1235、认证失败、对象不存在）；panic hook 记录崩溃。前端 `utils/logger.ts`（attachConsole 转发 + 全局 error/unhandledrejection/Vue errorHandler），关键业务点（连接/SQL 执行/导出失败）调用 `logger`。**日志禁止打印密码**；SQL 仅记录前 200 字符且失败时才落盘完整前缀
- 快捷键约定：动作定义与组合工具在 `utils/shortcuts.ts`（global/editor 两种 scope）；绑定存 `preferences.shortcuts`（可覆盖默认、实时生效）；global 动作由 App 的 keydown 监听（输入框/CM6 聚焦时不拦截），editor 动作由 SqlEditor 的 keymap compartment 动态生成
- **Windows 拖拽约定（重要）**：主窗 `tauri.conf.json#windows[main].dragDropEnabled=false`——Tauri 原生拖放会**吞掉 WebView2 的 HTML5 dragover/drop**（表现为「禁止光标」且 `allow-drop` 永不触发）；禁用后 HTML5 拖拽可用，**文件拖入一律走 HTML5**（`text/uri-list` 解析本地路径，参见 `DriverManager#onFileDrop`），不再使用 `getCurrentWebview().onDragDropEvent`
- 连接分组约定：树根级 = 文件夹（`stores/connections.groups`，**`ConnectionGroup.parentId` 支持多级嵌套**，null=根级）+ 未分组连接；`ConnectionConfig.groupId` 标识归属（可指向任意层级文件夹）；支持**拖拽连接/文件夹至任意层级文件夹**（el-tree draggable，`allow-drag` **仅放行连接/文件夹**（库/对象/表返回 false → 禁止光标，杜绝「看似移动实则不落库」），`allow-drop` **恒真（纯谓词）**（返回 false 会显示禁止光标且悬浮期副作用会中断拖拽），归属/层级校验全部在 `node-drop`（`moveToGroup`/`moveGroup` 含**循环嵌套防护**，非法落点 `ElMessage` 提示并恢复）后按需重建树）；`isLeaf` = 表/视图 + 其余对象（`table`/`object`），避免叶对象渲染无用展开箭头；**删除文件夹提供三种策略**（`removeGroup(id, 'promote'|'flatten'|'purge')`：仅删该节点内容上移 / 删本级+子文件夹连接上移 / 连同连接与凭据一并删除）；`loadNode` 按 `nodeType` 分层判断（勿依赖 level 数字），组子级 = `groupChildren(parentId)` + 组内连接
- 存储约定：**禁止新增 %APPDATA% 存储路径**——数据文件一律经 `src-tauri/src/storage.rs`（类别：connections/snippets/logs/workspaces）；解析优先级：settings.json 覆盖 > dev `.devdata` > 安装目录\data（不可写回退 LOCALAPPDATA）；文件路径必须走 `storage::file_path`；新增持久化数据时同步扩展 `StorageCategory::file_names` 与「存储」设置分类；日志 target 用 `TargetKind::Folder`（路径变更重启生效）
- 工作区存储约定：工作区**索引**（列表+currentId）存 `workspaces.json`（Workspaces 类别）；每个工作区创建时**必须指定磁盘存储位置**（`Workspace.dir`，目录选择对话框，取消则不创建），空值=默认类别目录（向后兼容旧数据）
- 工作区约定：`stores/workspaces.ts` 管理工作区（页签快照 + exportDir），持久化 `workspaces.json`；页签变化经 `tabs.exportSnapshot` 防抖快照；启动恢复上次工作区；导出默认目录取当前工作区 exportDir
- 对象状态约定：`ObjectInfo.status` 由方言目录查询提供（仅 Oracle 有编译状态），`CatalogQuery.status_index` 声明状态列；树节点异常状态（INVALID/UNUSABLE）显示角标；重编译经 `dialect.recompile_sql`（Oracle 实现）
- 交互约定：连接树单击表即查询前 100 行（预览 SQL 由后端方言生成），右键节点管理（新建查询/编辑/断开/删除/导出/查看定义）；查询 Tab 绑定连接+库上下文，支持选中执行；SQL 编辑器支持快捷指令展开（空格/Tab 触发，`${1:占位符}` Tab 跳转）、方言格式化（Ctrl+Shift+F，sql-formatter）、执行计划（capabilities.explain 驱动）；结果区支持表格/记录视图切换，单元格双击打开语义化详情（JSON 树/HEX/图片预览/日期多格式）
- `src-tauri/src/db/` 数据库层（Transport × Dialect 架构）：
  - `driver.rs` 定义 `DatabaseDriver` trait（连接/执行/元数据 + `dialect()`/`run_catalog()`/`query_text()`）
  - `dialect/` SQL 方言层：`mod.rs` 定义 `SqlDialect` trait（能力声明、标识符/字面量引用、INSERT 生成、DDL 策略、对象定义查询、预览 SQL、对象目录），`mysql_family.rs`（MySQL+OceanBase 共用）、`postgresql.rs`、`oracle.rs`（feature-gated）；**新增数据库 = 实现方言 + dialect::build 注册 + registry 登记 + create_driver 分支**
  - `registry.rs` 数据库类型注册表（`KindInfo` 能力下发前端，`db_registry` 命令）
  - `catalog.rs` + `objects.rs` 对象浏览器（按类型分组：表/视图/索引/序列/函数/存储过程/触发器）
  - `export.rs` 导出引擎（SQL=DDL+方言 INSERT / CSV / Excel，文件路径由前端 save 对话框提供）
  - `plan.rs` 执行计划（PlanNode + MySQL/PG JSON 计划解析器；Oracle capability 关闭）
  - `mysql.rs`/`postgres.rs`/`oracle.rs` 连接传输实现（连接池 + 结果集拉取 + semantic 推断 + 二进制 base64）
  - `bridge.rs` + `bridge_driver.rs` **内置桥接驱动**（免 OCI，NDJSON over stdio）：真实 Oracle 走 oracle-bridge（Go + go-ora），OceanBase Oracle 走 jdbc-bridge（Java + OceanBase Connector/J——OceanBase 的 Oracle 模式本质是 **MySQL 协议 + OB 扩展能力位** `OB_CLIENT_SUPPORT_ORACLE_MODE`，标准 Oracle TNS 客户端无法连接）；`pool.rs` 默认优先内置桥接，`params.driver == "oci"` 或桥接缺失时回退 `oracle.rs`
  - `pool.rs` 驱动工厂 `create_driver` + 会话管理器
  - `jdbc_driver.rs` + 存储类别 `drivers`（`drivers.json`）：**JDBC 驱动管理**（名称/驱动类/URL 模板/默认端口/本地 jar 列表，内置 OceanBase 预设）；`jdbc.rs` **通用 JDBC 连接类型**（`DbKind::Jdbc`，对象树走 JDBC `DatabaseMetaData`，方言为保守 ANSI 的 `dialect/generic.rs`）；`bridge.rs` 泛化支持 `-cp` 附加用户驱动 jar
- `src-tauri/src/commands.rs` 全部 Tauri 命令；`error.rs` 统一错误（实现 Serialize，含 XlsxError 转换）

## 约定与注意事项

- **功能规约（必读）**：`docs/specifications.md` 为长期维护的功能规约（UI/编辑器/结果集/浏览器/首选项/国际化），修改相关功能前先阅读对应条目，实现变更后同步修订
- **国际化（必须遵守）**：所有 UI 文案必须走 vue-i18n，语言包在 `src/i18n/locales/zh-CN.ts` 与 `en.ts` **双端同步维护**，新增任何界面文字需同时补充两种语言；组件内用 `useI18n().t()`，非组件（store/工具）用 `i18n.global.t`；内置数据（如 snippetDefaults 的 description）存 i18n key 而非中文原文；**文案中的 `@` 必须写成 `{'@'}`**（否则被 vue-i18n 当作 linked message 语法导致模板编译崩溃，如 OB 用户名 `user{'@'}tenant`）
- **用户偏好**：语言/主题/编辑器样式/源码查看器/数据格式/结果集展示/界面字体/导出默认项/SQL 美化等统一放 `stores/preferences.ts`（localStorage 持久化：`personaldb.locale`/`personaldb.theme`/`personaldb.editor`/`personaldb.viewer`/`personaldb.dataformat`/`personaldb.result`/`personaldb.ui`/`personaldb.export`/`personaldb.format`），**改动即时生效**（组件通过响应式偏好热更新）；首选项对话框（两栏式）内提供源码查看器与数据格式实时预览；Element Plus 组件文案由 App.vue 的 `el-config-provider` 动态绑定
- **SQL 编辑器**：使用 CodeMirror 6（`components/SqlEditor.vue`），方言高亮按 DbKind 映射（mysql/oceanbase→MySQL、postgresql→PostgreSQL、oracle→PLSQL）；大小写转换与压缩 SQL 必须跳过字符串字面值（`convertOutsideLiterals` / `utils/sqlFormat.ts#compressSql`）；**格式化方案可配置**（`preferences.formatPrefs`，`formatSql(kind, sql, opts)`）；**自动补全**来源于 `db_schema_snapshot`（同 schema 表/列 + comments）与当前 SQL 词（CM6 `autocompletion` 语言数据源）
- **菜单命令**：跨组件命令经 `utils/commands.ts` 的 `app-command` 事件分发，命令需携带目标 tabId，QueryTabPane 按 tabId 过滤响应
- **执行状态**：查询执行使用 `runSeq` 请求序列号，过期响应必须丢弃；停止按钮通过自增 runSeq 作废挂起请求（不得直接依赖 Promise 返回复位状态）
- 数据库类型枚举 `DbKind` 在 `src-tauri/src/db/types.rs` 与 `src/types/db.ts` 双端维护，新增数据库需同步修改并实现 trait 后在 `pool.rs` 的 `create_driver` 注册
- Oracle **默认走内置 go-ora 驱动**（`oracle-bridge`，纯 Go，免装 OCI）；OceanBase Oracle **默认走内置 JDBC 驱动**（`jdbc-bridge`，OceanBase Connector/J，**需 JRE 21+**，经 JAVA_HOME/PATH 定位 java）。构建时 `build.rs` 分别调 Go 工具链编译 `src-tauri/oracle-bridge/`、调 Maven 打包 `src-tauri/jdbc-bridge/` 到 `src-tauri/binaries/`，经 `tauri.conf.json#bundle.resources` 打包；**缺 Go/Maven 则跳过并告警，运行时回退 OCI**。连接方式由 `params.driver` 选择（缺省内置 / `oci` 走 `oracle.rs`）。产物不入库（`.gitignore` 忽略 `binaries/*.exe`、`jdbc-bridge/target/`）。OCI 路径（`oracle.rs`）依赖 OCI 客户端库，**默认特性已启用**（ODPI-C 运行时动态加载，编译期不依赖 OCI）；所有 OCI 相关代码位于 `#[cfg(feature = "oracle")]`，`--no-default-features` 可排除（此时仍可用内置驱动）
- **OceanBase Oracle 连接要点**：用户名须为 `用户@租户`（如 `myuser@mytenant`）；**Schema（库名）必须留空**（Oracle 模式 schema 默认 = 用户名，填租户名会报 `Unknown database`）；端口直连 OBServer 2881 或 OBProxy 2883 均可
- Rust 侧禁止阻塞 runtime：Oracle 同步驱动必须用 `tokio::task::spawn_blocking` 包装
- 结果集必须支持 `max_rows` 截断（MySQL 用 `QueryResult::next`、PG 用 `TryStreamExt::try_next` 惰性拉取）
- 前后端字段命名：Rust 结构体一律 `#[serde(rename_all = "camelCase")]`，TS 端不使用 snake_case
- Windows 下 tauri-winres 需要 rc.exe，路径配置在 `src-tauri/.cargo/config.toml`（Windows SDK 版本变化时需更新）
- 导出方言细节：MySQL 文件头写 `SET NO_BACKSLASH_ESCAPES=1`（literal 不转义反斜杠）；Oracle 日期用 `TO_TIMESTAMP`（含/不含毫秒两种格式）；二进制统一 base64 传输、导出时转 HEX（X''/decode/HEXTORAW）
- 方言连接注意：PG 未填 database 时**必须默认连 `postgres` 库**（tokio-postgres 默认以用户名作为库名，同名库不存在会连接失败）；PG 的 `list_databases` 返回 **schema 列表**（`database` 参数在 PG 驱动中即 schema 上下文，同连接跨 schema 无需重连，勿改回 pg_database）；OceanBase 需显式 `ssl_opts(None)` 禁用 TLS（mysql_async 默认 PREFERRED，OBProxy/CE 的 TLS 协商常见失败）；OceanBase **Oracle 模式租户**不支持 MySQL 协议（服务端报 1235），须使用独立的 `DbKind::OceanBaseOracle`（复用 oracle 传输 + Oracle 方言，需 `--features oracle`；用户填租户名作 service_name）—— 错误经 `utils/connError.ts#describeConnError` 附加处理建议
- 不要用 PowerShell 的 Set-Content 重写源码文件（会破坏 UTF-8 中文注释），改文件一律用编辑工具
- `fallible-iterator` 必须锁 0.2（与 tokio-postgres 一致）；oracle crate 0.6 为重构版 API（`PoolBuilder::max_connections`、`Connection::statement().build()`）
- 连接密码目前明文存于存储目录（`storage.rs` 解析，默认 `安装目录\data\connections\connections.json`），改动相关逻辑时注意不要把密码写入日志

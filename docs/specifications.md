# PersonalDb 功能规约（UI / 编辑器 / 结果集 / 浏览器）

> 本文件为长期维护的功能规约，与 `AGENTS.md` 配合。新增或修改相关功能前先阅读对应条目；实现变更后同步修订本文。
> 编号规则：`SPEC-<域>-<序号>`，域：UI / EDITOR / RESULT / TREE / PREFS / I18N。

## SPEC-UI-1 结果集紧凑布局

- **需求**：结果集默认紧凑展示，减小行高与单元格内边距，提升信息密度。
- **验收**：
  - 首选项「结果集 → 紧凑布局」默认开启；
  - 紧凑模式下表格行高 22px、单元格纵向留白 ≤2px、字号 12px；
  - 关闭后恢复 28px 常规行高。
- **落点**：`stores/preferences.ts#result.compact`、`components/QueryTabPane.vue`（`.result-table.is-compact`）。

## SPEC-RESULT-1 结果集格式化展示（双层设置）

- **需求**：结果集支持按值类型格式化展示；提供**全局默认**（首选项）与**列级覆盖**（表头）两级设置。
- **规则**（列级优先于全局）：
  | 语义类型               | 可选格式                     |
  | ---------------------- | ---------------------------- |
  | json                   | 原始值 / 格式化（缩进）      |
  | date / datetime / time | 原始值 / 本地格式 / ISO 8601 |
  | number / decimal       | 原始值 / 千分位              |
  | text / longtext        | 截断（默认）/ 完整显示       |
- **验收**：表头设置图标可打开列格式菜单；列级选择立即生效且仅影响该列；全局开关影响所有同类列（未被列级覆盖的）。
- **落点**：`components/cell-viewers/CellDisplay.vue#format`、`QueryTabPane.vue#colFormats/effectiveFormat`、`stores/preferences.ts#result`。

## SPEC-RESULT-2 文本字段查看器

- **需求**：文本类型字段的详情查看器支持：
  1. **十六进制查看**（按 UTF-8 编码转 hex dump）；
  2. **不可见字符显示/隐藏**（CR/LF/TAB 及控制符符号化，隐藏时还原原始文本）。
- **验收**：文本 Tab 提供「文本 / 十六进制」切换；文本模式下可切换不可见字符与自动换行；HEX 视图与二进制查看器复用同一组件。
- **落点**：`components/cell-viewers/CellDetailDrawer.vue`（`textView/showInvisible/wrapText`）、`HexViewer.vue`。

## SPEC-EDITOR-1 SQL 编辑器

- **需求**：
  1. 按当前数据库类型加载**不同语法高亮**（MySQL / PostgreSQL / PLSQL）；
  2. 支持常见编辑操作（撤销重做、搜索、括号匹配、自动缩进、行号、自动换行可配置）；
  3. 支持**大小写转换**，且**字符串字面值不得被转换**；
  4. 支持**压缩 SQL**（去注释与多余空白、折叠为单行，字面值保持原样）；
  5. 编辑器右侧提供**代码缩略图**（minimap，VSCode 式缩放文本 + 视口指示），可经首选项关闭。
- **验收**：
  - 编辑器为 CodeMirror 6；切换 Tab 到不同方言连接时高亮方案随之切换；
  - `Ctrl+Shift+F` 格式化、`Ctrl+Enter` 执行、`Ctrl+U / Ctrl+Shift+U` 大写/小写、`Ctrl+Alt+K` 压缩；
  - 大小写转换与压缩均跳过 `'...'`、`"..."`、`` `...` `` 及其 `''`/`""` 转义；
  - 无选区时转换光标所在标识符；压缩默认快捷键可在快捷键设置中修改；
  - 查找/替换、转到行、折叠等 **CodeMirror 内置面板文案走 i18n**（`EditorState.phrases`，`cm.*` 键，语言切换实时生效，无需重建编辑器）；
  - 缩略图经 `@replit/codemirror-minimap`（`displayText: 'characters'`），偏好 `editor.minimap`（默认开）热切换；随 SqlEditor 生效于查询/例程/表查看等全部宿主。
- **落点**：`components/SqlEditor.vue`（`dialectOf`、`convertOutsideLiterals`、`compressCode`、`phrasesExt`、`minimapExt`、keymap）、`utils/sqlFormat.ts#formatSql/compressSql`、`stores/preferences.ts#editor.minimap`。

## SPEC-EDITOR-2 快捷指令（扩充既有）

- **需求**：编辑器内输入缩写 + 空格/Tab 展开模板，占位符 Tab 跳转；可配置。
- **验收**：行为与 `SnippetSettings` 一致；展开逻辑与 textarea 版保持等价（命中缩写才拦截空格）。
- **落点**：`components/SqlEditor.vue#tryExpand/jumpPlaceholder`、`stores/snippets.ts`。

## SPEC-TREE-1 对象浏览器搜索过滤

- **需求**：连接树支持按关键字搜索/过滤节点（库、对象名、备注）。
- **验收**：搜索框输入即时过滤已加载节点；清空恢复；备注命中亦可。
- **落点**：`components/ConnectionTree.vue`（`filterText/filterNode`）。

## SPEC-TREE-2 对象浏览器右键菜单

- **需求**：按节点类型提供上下文操作：
  | 节点                            | 菜单项                                                                  |
  | ------------------------------- | ----------------------------------------------------------------------- |
  | 连接                            | 新建查询、编辑、测试、断开、删除                                        |
  | 库 / 表文件夹                   | 新建查询、**新建表（模板）**                                      |
  | 表                              | 查询数据、查看结构、**查看源码（DDL）**、**复制 DDL**、导出 |
  | 视图/函数/过程/触发器/索引/序列 | 查看定义（例程为编辑例程）、重新编译（Oracle）                          |
- **验收**：新建表生成按方言的 CREATE TABLE 模板到新 Tab；复制 DDL 写入剪贴板并提示；**双击表/视图与右键「查询数据」行为一致——打开 SQL 编辑器并生成前 100 行预览 SQL（不自动执行）**；**查看数据结构与源码在专用面板 Tab（表查看面板 / 对象查看面板 / 例程编辑，见 SPEC-UI-6/7）中展示，不占用 SQL 编辑器**；**菜单为 fixed 定位，必须全局收起——窗口失焦（blur）、Esc、任意位置点击/他处右键、滚动、窗口缩放均关闭（capture 监听，卸载时移除），杜绝「焦点离开后菜单仍滞留」**；页签右键菜单（TabArea）同规约。
- **落点**：`ConnectionTree.vue`、`components/workbench/TabArea.vue`、`stores/tabs.ts#openCreateTableTemplate`、Rust `commands::db_fetch_table_ddl`。

## SPEC-UI-6 表查看面板与对象查看面板（均为 Tab）

- **需求**：**表**的查看结构与源码、**非表对象**（视图/触发器/序列/索引等）的定义查看，均采用 **Tab 页签组件**，**禁止使用抽屉组件**。
- **表查看面板（Tab，`type: 'table'`）验收**：
  - 三个内部页签：**列**（可编辑字段：名称/类型/可空/默认值/备注，可新增/删除列，主键/键标记，字段过滤）、**索引**（索引名/列/唯一/主键，可新增/删除索引）、**源码**（只读代码视图 + 复制 + 在新查询中打开）；
  - 「生成变更 SQL」→ 汇总待应用变更 → 方言层生成 DDL → **预览对话框** → 确认后按序执行并重新加载；
  - 去重：同连接同表已打开则激活既有页签。
- **对象查看面板（Tab，`type: 'object'`）验收**：只读展示对象定义（CodeViewer）+ 刷新 + 在新查询中打开；按连接+库+对象名去重。
- **落点**：`components/TableInspectorPane.vue`、`components/ObjectInspector.vue`、`stores/tabs.ts#openTableInspector/openObjectInspector`。

## SPEC-UI-7 例程编辑器（Tab）

- **需求**：存储过程与函数有**专门的编辑与查看组件**（区别于只读对象查看面板），以 **Tab 形式**（`type: 'routine'`）展示，可查看定义与参数，也可直接编辑并保存。
- **验收**：

  - 右键函数/过程节点「编辑例程」打开**例程编辑 Tab**；
  - 头部显示名称、方言、类型（FUNCTION/PROCEDURE）与函数返回类型；
  - **参数面板**（可折叠）：序号/名称/模式（IN/OUT/INOUT）/数据类型，无参数时提示；
  - 源码区为**可编辑**编辑器（CM6，含方言高亮/快捷指令/快捷键）；操作：刷新、复制、在新查询中打开、**保存**（执行源码并重新加载）；
  - 按连接+库+例程名去重；元数据来自 `db_describe_routine`。
- **落点**：`components/RoutineEditor.vue`（Tab 面板）、`stores/tabs.ts#openRoutineEditor`、Rust `db_describe_routine`。
- **需求**：结果集表格支持斑马纹、行高设定、列拖拽排序、固定列、筛选、查找、排序。
- **验收**：

  - **斑马纹**：首选项开关（默认开）；
  - **行高**：首选项三档（紧凑 22px / 标准 28px / 宽松 36px）；
  - **列拖拽排序**：拖拽表头调整列顺序（使用 sortablejs；存在固定列时禁用以免错位）；
  - **固定列**：列设置弹层选择「不固定 / 左 / 右」；
  - **筛选**：列设置弹层输入包含值，按列过滤（多列叠加，显示生效列数）；
  - **查找**：结果区查找框，跨列包含匹配并显示匹配行数；
  - **排序**：所有列可点击表头排序（前端排序）；
  - **列头布局**：双行（列名 + 类型小字），列设置图标 hover 显示（已设置时常显），排序箭头独立。
- **落点**：`components/QueryTabPane.vue`（`colSettings`/`colOrder`/`searchText`/`resultRows`/`initSortable`/列头布局）、`stores/preferences.ts#result`（rowHeight/zebra）。

## SPEC-UI-8 图标体系

- **需求**：不同组件类型使用**不同的默认图标**；查询 Tab 页签也显示对应的类型图标。
- **图标使用红线（强制）**：
  1. **禁止不同语义复用同一图标**——一个图标在所有位置只表达一种语义；同语义必须同图标（如「最小化」在主窗标题栏与悬浮窗均为 `Minus`；「隐藏面板」在面板头 / 悬浮窗 / 日志面板均为 `act_hide`）；
  2. **禁止使用可能造成用户困惑的图标**——尤其禁止与主窗标题栏的「最小化 `Minus` / 最大化 `FullScreen` / 关闭 `Close`」混淆；面板级动作一律用自绘 `act_float / act_hide / act_dock`；
  3. 新增图标前先查 `utils/icons.ts#APP_ICONS`，已有同语义图标必须复用；确需新增时同步登记语义（注释）。
- **语义 → 图标映射（唯一）**：
  | 语义 | 图标 | 语义 | 图标 |
  | --- | --- | --- | --- |
  | 执行/运行（全部·选中·当前行） | `act_run` | 停止 | `act_stop` |
  | 事务模式 | `act_tx` | 提交 | `act_commit` |
  | 回滚 | `act_rollback` | 撤销 / 重做 | `act_undo` / `act_redo` |
  | 刷新·重新加载 | `act_refresh` | 重新编译对象 | EP `Refresh` |
  | 重连 | `act_reconnect` | 恢复默认/重置 | EP `RefreshLeft` |
  | 保存 | `act_save` | 打开文件·目录 | EP `FolderOpened` |
  | 导出 | `act_export` / EP `Download`（导出类） | 导入 | `act_import` |
  | 复制（DDL/源码/单元格/DML） | EP `CopyDocument` | 删除·清空（破坏性） | EP `Delete` |
  | 新建/添加 | EP `Plus` | 编辑·重命名 | EP `EditPen` |
  | 关闭（窗口·页签族） | EP `Close` | 隐藏面板 | `act_hide` |
  | 悬浮 | `act_float` | 回收停靠 | `act_dock` |
  | 最小化（窗口/面板） | EP `Minus` | 最大化（窗口） | EP `FullScreen` |
  | 分屏 | `act_split` | 表结构·列族 | EP `Grid` |
  | 记录上/下一条 | `act_prev` / `act_next` | 下拉/上拉指示 | EP `ArrowDown` / `ArrowUp` |
  | 展开/折叠 | EP `Expand` / `Fold` | 格式化 SQL | EP `Brush` |
  | 快捷指令 | EP `MagicStick` | 检出/预览变更 | EP `View` |
  | 分屏（左右） | `act_split` | 分屏（上下） | `act_split_stack` |
  | 还原单屏（退出分屏） | `act_unsplit` | 页签列表（标签管理） | EP `List` |
  | 表格视图（结果集网格） | `act_view_grid` | 记录视图 | EP `Postcard` |
  | 主题切换（明暗） | `act_theme` | 检查更新 | `act_update` |
  | 驱动管理（连接器） | `act_driver` | 关于 / 信息 | `act_info` |
  | 首选项 / 设置 | `act_settings` | 日志（日志面板 / 查看日志） | `act_monitor` |
  | 文件夹容器（分组·工作区） | EP `Folder` | 查询数据 | EP `Tickets` |
  | 连接（连接管理器面板） | `obj_connection` | 功能区（工作区面板） | `act_workspace` |
  | 日志（日志面板） | `act_monitor` | 面板条 · 面板开关 | 复用各面板图标（`obj_connection` / `act_workspace` / `act_monitor`） |
- **验收**：
  - 统一映射 `utils/objectIcons.ts`：表/视图/索引/序列/函数/存储过程/触发器/库/文件夹/查询各有专属图标；
  - **`objectIcon(key)` 解析优先级**：`APP_ICONS` 直连 key（`obj-` / `act-` / `st-` 系列，如 `objectIcon('act_run')`）> 对象类型别名（`table` / `session` → `obj-*`）> 兜底 `obj_query`；**禁止把未登记 key 静默回退为查询图标**（历史上导致工作台动作图标全渲染成查询图标的缺陷）；
  - 连接树节点与 Tab 页签共用该映射（同类对象图标一致）；
  - Tab 图标由来源决定：单击表（或视图）打开查询 → 表/视图图标；新建表模板 → 表图标；例程「在新查询中打开」→ 函数/过程图标。
- **落点**：`utils/objectIcons.ts`、`utils/icons.ts`、`components/AppIcon.vue`、`ConnectionTree.vue#nodeIconKey`、`App.vue`（`.tab-label`）、`stores/tabs.ts#QueryTab.icon`。

## SPEC-UI-2 执行状态与停止

- **需求**：执行按钮不得长期停留在 loading；提供停止能力。
- **验收**：运行时按钮变为「停止」（危险态），点击立即复位并作废本次结果；过期响应不得覆盖新结果（请求序列号机制）；新建 Tab 初始非 loading。
- **落点**：`stores/tabs.ts#runTab/cancelRun/runSeq`、`QueryTabPane.vue`。

## SPEC-UI-3 记录视图取值

- **需求**：单行记录查看器必须正确渲染数据（含重名列）。
- **验收**：记录视图按列渲染 prop 取值（而非原始列名），重名列不丢值。
- **落点**：`components/cell-viewers/RecordView.vue`（`RecordColumn.prop`）。

## SPEC-UI-4 工具栏与主题切换

- **需求**：工具栏按钮在深浅主题下样式正确，text 型按钮不得继承描边按钮样式。
- **验收**：主题/设置按钮无边框、hover 仅浅底；切换主题后工具栏、侧栏、面板、状态栏、菜单栏颜色一致跟随。
- **落点**：`App.vue`（`.app-header` 的 `:not(.is-text)` 选择器、CSS 变量双主题）。

## SPEC-PREFS-1 首选项

- **需求**：首选项为**两栏式**（左分类导航 + 右内容），承载更细的组件样式配置；**支持实时预览**。
- **验收**：
  - 分类：外观（主题/语言）、编辑器（字号/缩进宽度/行号/自动换行/关键字大写）、**源码查看器（行号/代码块折叠 + 内嵌实时预览）**、结果集（行高/斑马纹/行数上限/JSON 格式化）、**数据格式（日期格式/千分位/布尔样式/NULL 文本/截断长度 + 预览表格）**、**存储（各类别目录/修改/重置/打开）**、快捷键、关于；
  - 预览区：查看器分类内置 CodeViewer 示例；数据格式分类内置示例表格（日期/数值/布尔/长文本/NULL 实时渲染）；
  - 所有偏好持久化（localStorage）且**改动即时生效**（各组件通过响应式偏好热更新，无需重开）。
- **落点**：`stores/preferences.ts`（editor/viewer/result/dataFormat/shortcuts 五组）、`components/SettingsDialog.vue`、`components/SqlEditor.vue`、`components/CodeViewer.vue`、`components/cell-viewers/CellDisplay.vue`、`stores/tabs.ts`。

## SPEC-RESULT-5 数据格式展示设置

- **需求**：单元格值的展示格式提供**全局数据格式设置**（列级设置仍优先，见 SPEC-RESULT-1）。
- **验收**：
  - **日期时间**：原始值 / 本地格式 / ISO 8601（取代原布尔式"日期本地化"开关）；
  - **数值千分位**：开关；
  - **布尔样式**：true/false 或 YES/NO；
  - **NULL 显示文本**：自定义（默认 NULL）；
  - **长文本截断长度**：可配（默认 300 字符）；
  - 首选项面板提供示例表格实时预览；设置即时生效于结果表格、记录视图与详情抽屉。
- **落点**：`stores/preferences.ts#dataFormat`、`components/cell-viewers/CellDisplay.vue`、`components/cell-viewers/CellDetailDrawer.vue`、`QueryTabPane.vue#effectiveFormat`、`SettingsDialog.vue`（数据格式分类）。

## SPEC-UI-10 源码查看器增强

- **需求**：源码查看器（CodeViewer）支持**展示/隐藏行号**、**收起/展开代码块**、**格式化代码**。
- **验收**：
  - 行号：开关受首选项「源码查看器 → 显示行号」控制（查看器工具栏同步，改动实时生效）；
  - 代码块折叠：基于括号的通用折叠服务（`(`/`{` 折叠至匹配闭合符，扫描上限 500 行），gutter 提供折叠标记，工具栏提供「全部折叠/全部展开」；开关受首选项「代码块折叠」控制；
  - 格式化：按当前方言（sql-formatter）格式化显示内容（查看辅助，不回写源数据）；
  - 复制：复制当前显示内容（含格式化结果）。
- **落点**：`components/CodeViewer.vue`（`gutterExt`/`bracketFold`/`foldAll`/`unfoldAll`/`formatCode`）、`stores/preferences.ts#viewer`。

## SPEC-UI-5 菜单栏与工具栏

- **需求**：窗口顶部自上而下为**菜单栏 → 工具栏**；菜单栏提供 文件/编辑/查看/工具/帮助。
- **验收**：
  - 菜单栏位于窗口绝对顶部（高度 30px，跟随主题的面板底色），工具栏在其下方（高度 42px，品牌深色）；
  - 编辑/查看/工具分组命令作用于**当前激活查询 Tab**；跨组件通过 `app-command` 事件分发，命令携带目标 tabId，非目标 Tab 不响应。
- **落点**：`App.vue`（`el-menu` 在 `el-header` 之前、`onMenuSelect`）、`utils/commands.ts`、`QueryTabPane.vue#handleAppCommand`。

## SPEC-I18N-1 国际化（强制）

- **需求**：所有 UI 文案走 vue-i18n；新增功能必须同时补充 `zh-CN` 与 `en`。
- **验收**：组件内用 `useI18n().t()`；非组件（store/工具）用 `i18n.global.t`；内置数据存 i18n key。
- **落点**：`src/i18n/locales/*.ts`、`AGENTS.md`。

## SPEC-UI-11 树交互与页签命名

- **需求**：连接树单击仅选中节点；**双击表/视图打开 SQL 编辑器并生成预览 SQL（不自动执行）**；查询页签支持手动重命名。
- **验收**：
  - 单击：仅高亮选中，无任何执行；
  - 双击表/视图：打开查询 Tab，编辑器内生成方言预览 SQL，**不执行**（等待用户手动执行）；
  - 页签重命名：双击页签标题进入内联编辑（回车/失焦保存，Esc 取消）；图标保持不变。
- **落点**：`ConnectionTree.vue#onNodeClick/onNodeDblClick`、`stores/tabs.ts#openTableQuery`、`App.vue`（`startRename/commitRename`）。

## SPEC-TX-1 DML 事务支持（自动提交 / 手动提交）

- **需求**：支持切换事务模式：**自动提交**与**手动提交**；手动模式下 DML 需显式提交或回滚。
- **验收**：
  - 事务管理位于**主窗体工具栏**（紧凑布局）：事务图标 + 「自动提交 / 手动提交」分段控件 + 「提交 / 回滚」图标按钮（仅手动模式显示）；
  - 作用于**当前激活查询 Tab 所属连接**，按 `capabilities.transactions` 显隐；
  - 手动模式：会话持有**专用连接**承载事务（MySQL `SET autocommit=0`；PG 惰性 `BEGIN`），所有执行走该连接，DML 不自动提交；
  - 提交/回滚作用于当前手动事务；切回自动提交时提交未决事务并释放专用连接；
  - **Oracle 始终自动提交**（capabilities.transactions=false，DML/DDL 执行后显式 `commit`）。
- **落点**：`db/{mysql,postgres,oracle}.rs`（`set_autocommit/commit/rollback` + 专用连接）、`commands::db_set_autocommit/db_commit/db_rollback`、`App.vue`（工具栏事务区）、`stores/connections.ts#txModeOf`。

## SPEC-UI-13 查询面板工具栏

- **需求**：查询面板工具栏**图标化、紧凑**。
- **验收**：
  - 按钮改为图标按钮（circle）并配 tooltip：执行 / 执行选中 / 停止（运行时）/ 快捷指令 / 格式化 / 执行计划；
  - 库选择下拉收窄；仅保留数据库类型小字提示，长说明文案移除（信息在状态栏与 tooltip）；
  - 事务控件不再出现在查询面板（移至主工具栏，见 SPEC-TX-1）。
- **落点**：`components/QueryTabPane.vue`（`.query-toolbar`）。

## SPEC-UI-14 自定义标题栏与树自动加载与页签菜单

- **需求**：
  1. **自定义标题栏**跟随主题（隐藏系统装饰），含窗口控制；
  2. 对象浏览器**打开窗体时自动加载**已保存配置；
  3. 查询页签支持**右键上下文菜单**（重命名/关闭/关闭其他/关闭右侧/关闭全部）。
- **验收**：
  - `decorations: false` + 自绘标题栏（32px：品牌 + 拖拽区 + 最小化/最大化/关闭），深浅主题联动；拖拽区可移动窗口，双击切换最大化；
  - 树在配置就绪后自动构建（store 加载幂等 + 树挂载后重建），无需手动刷新；
  - 页签右键菜单项按预期关闭对应页签并保持激活态有效；
  - 「查询数据」打开的页签执行受超时保护（默认 60s，可在首选项「查询执行」中调整，0 = 不限制），超时自动复位按钮并提示；错误条提供「调整超时」按钮直达该设置（杜绝永久 loading，长查询/报表可自行放宽）。
- **落点**：`src-tauri/tauri.conf.json`（decorations）、`capabilities/default.json`（window 权限）、`App.vue`（titlebar/tabMenu + open-settings 命令）、`stores/connections.ts#load`（幂等）、`stores/tabs.ts#runTab`（超时）、`stores/preferences.ts#query`、`components/SettingsDialog.vue`（查询执行分类）。

## SPEC-EDITOR-3 执行范围（全部 / 选中 / 当前行）

- **需求**：SQL 编辑器支持**执行全部、执行选中、执行当前行**。
- **验收**：
  - 工具栏三个图标按钮（▶ 全部 / 选中 / 当前行）+ 快捷键：`Ctrl+Enter` / `Ctrl+Shift+Enter` / `Ctrl+Alt+Enter`（均可在快捷键设置中改绑）；
  - 选中为空时回退执行全部；当前行取光标所在行文本；
  - 三种范围共用执行序列号与停止机制。
- **落点**：`components/SqlEditor.vue#getCurrentLine/customKeymap`、`QueryTabPane.vue#run`、`utils/shortcuts.ts`。

## SPEC-UI-12 菜单栏样式与快捷键绑定

- **需求**：菜单栏样式精致化；提供**可配置的快捷键绑定**。
- **验收**：
  - 菜单栏：项 hover 主色、下拉面板圆角阴影加宽、菜单项右侧显示快捷键提示（灰色等宽字体）；
  - 快捷键：动作表（`utils/shortcuts.ts`）区分 **global**（新建连接/新建查询/切换主题/首选项）与 **editor**（执行/格式化/大写/小写）；首选项「快捷键」分类支持**点击输入框录制新组合**（Esc 取消）与单项/全部重置；绑定持久化（`personaldb.shortcuts`）并**实时生效**（全局监听 + CM6 keymap compartment 重配）；
  - 输入框/编辑器聚焦时全局快捷键不拦截。
- **落点**：`utils/shortcuts.ts`（matchesShortcut/eventToCombo/toCodeMirrorKey）、`stores/preferences.ts#shortcuts`、`App.vue`（onGlobalKeydown）、`components/SqlEditor.vue#customKeymap`、`SettingsDialog.vue`（快捷键分类）。

## SPEC-TREE-3 连接文件夹管理

- **需求**：连接支持创建文件夹分组管理。
- **验收**：
  - 树根级 = 文件夹节点 + 未分组连接；文件夹可展开/收起；
  - 工具栏「新建文件夹」；文件夹右键：新建连接 / 重命名 / 删除（删除后连接移出分组）、连接右键「移动到文件夹」（对话框选择）；
  - **拖拽移动**：连接与文件夹均可拖拽（`allow-drag` 仅放行 `conn`/`group`，库/对象/表显示禁止光标）；连接可拖入文件夹内部（inner → 归入该文件夹），拖到根级空白或连接同级（prev/next）→ 移出分组（组内顺序暂不支持，落点为「移出/同组」语义）；文件夹支持 inner（嵌套为子文件夹）与 prev/next（同级排序，含循环嵌套防护）；非法落点 `ElMessage` 提示并回弹；落点提供虚线高亮与插入指示线；
    - 拖入**已展开**文件夹：el-tree 原生移动，不重建树（保留展开态）；
    - 拖入**未展开**文件夹（懒加载 resolve 会覆盖拖入结果）或拖到根级：重建树并恢复连接/文件夹层展开状态，确保界面与归属一致；
  - 连接表单提供「所属文件夹」选择；
  - 持久化：连接配置 `groupId`（connections.json）+ 文件夹列表（groups.json，命令 `group_save/group_load`）。
- **落点**：Rust `ConnectionConfig.group_id`/`ConnectionGroup`/`commands::group_save/group_load`；`stores/connections.ts`（groups/createGroup/renameGroup/removeGroup/moveToGroup）；`ConnectionTree.vue#loadNode`（按 nodeType 分层）；`ConnectionForm.vue`。

## SPEC-TREE-4 选中与活动高亮

- **需求**：高亮显示当前选中的连接、文件夹等节点，以及当前已连接的连接。
- **验收**：
  - 当前选中节点：主色浅底 + 主色加粗文字（覆盖 el-tree 默认高亮）；
  - 已连接的连接节点：左侧绿色侧条 + 绿色加粗名称；
  - hover 提供浅底反馈。
- **落点**：`ConnectionTree.vue`（`.is-current` 与 `.is-active-conn` 样式）。

## SPEC-STORAGE-1 存储目录体系

- **需求**：所有数据文件**不得存放于 %APPDATA%**；安装时默认存于安装目录；支持在首选项按类别修改与迁移；开发模式运行时文件置于项目目录。
- **验收**：
  - 类别：连接配置（connections.json/groups.json）、快捷指令（snippets.json）、日志（personaldb.log）；
  - 解析优先级：**settings.json 覆盖 > dev `.devdata` > 安装目录 `data`**（安装目录不可写时回退 %LOCALAPPDATA% 并记日志）；
  - 开发模式（debug 构建）以 `CARGO_MANIFEST_DIR` 父目录（项目根）的 `.devdata` 为运行时目录（已加入 .gitignore）；
  - 首选项「存储」分类：显示各类别当前目录与是否自定义，支持修改目录（**自动迁移现有文件**）/ 重置默认（迁回）/ 在资源管理器中打开；日志类别路径变更**重启生效**；
  - 首版遗留 `%APPDATA%` 数据在启动时一次性自动迁移；
  - 任何新持久化数据必须走 `storage::file_path`，禁止直接使用 AppData 路径。
- **落点**：`src-tauri/src/storage.rs`、`commands::storage_info/set_dir/reset_dir/open_dir`、`lib.rs`（日志 `TargetKind::Folder` + 启动迁移）、`components/SettingsDialog.vue`（存储分类）、`api/db.ts`。

## SPEC-TREE-5 对象浏览器展开 / 收起

- **需求**：对象浏览器支持展开与收起。
- **验收**：
  - 工具栏「展开已加载 / 全部收起」：仅作用于**已加载节点**（不触发新的懒加载）；
  - 节点右键「展开该节点 / 折叠该节点」（含子级）；
  - 收起后重新展开不重复发起元数据查询（el-tree 懒加载缓存）。
- **落点**：`components/ConnectionTree.vue`（`expandLoaded/collapseAll/expandNode/collapseNode`）。

## SPEC-TREE-6 对象状态（编译错误等）

- **需求**：除表外对象需展示状态（正常 / 编译错误等）。
- **验收**：
  - 能力驱动：**Oracle** 对象携带 `status`（VALID/INVALID/UNUSABLE，来自 `all_objects`/`all_indexes`），MySQL/PG 无此概念（不显示）；
  - 树节点在类型标签旁显示异常状态角标：`INVALID` 红色「编译错误」、`UNUSABLE` 橙色「不可用」，正常（VALID）不显示；
  - Oracle 对象右键提供「重新编译」（视图/过程/函数/触发器 `ALTER ... COMPILE`，索引 `ALTER INDEX ... REBUILD`）；其他数据库不显示该项。
- **落点**：`db/objects.rs`（`ObjectInfo.status`、`CatalogQuery.status_index`）、`db/dialect/oracle.rs`（目录查询携带 status + `recompile_sql`）、三驱动 `run_catalog` 状态映射、`commands::db_recompile_object`、`ConnectionTree.vue`（角标/菜单）。

## SPEC-UI-15 浮层主题化

- **需求**：系统 tooltip 等浮层配色须与主题一致。
- **验收**：`src/styles/overrides.css` 统一覆盖 `.el-popper.is-dark`（背景/文字/边框/阴影/箭头）走 Element Plus 主题变量，深浅主题自动联动。
- **落点**：`src/styles/overrides.css`（main.ts 全局引入）。

## SPEC-WORKSPACE-1 工作区

- **需求**：新增工作区概念，管理**页签集合**与**文件保存位置**。
- **验收**：
  - 模型：`Workspace { id, name, exportDir?, tabs: TabSnapshot[] }`，持久化于存储体系 `workspaces.json`（`currentId + list`）；
  - 工具栏「工作区」下拉：显示当前工作区、切换、新建、重命名、删除、设置/打开文件保存位置；当前项高亮；
  - **页签自动快照**：tab 变化（增删/标题/SQL）防抖 500ms 写入当前工作区（不含结果集）；应用启动**恢复上次工作区的页签**；
  - 切换工作区：保存当前 → 载入目标工作区页签；
  - **导出默认目录**：ExportDialog 的保存对话框默认路径取当前工作区 `exportDir`；
  - 至少保留一个工作区（删至 1 个时禁用删除）。
- **落点**：`src-tauri/src/commands.rs`（`workspace_save/load`）、`stores/workspaces.ts`、`stores/tabs.ts`（`exportSnapshot/importSnapshot`）、`App.vue`（下拉 + 启动恢复 + 快照）、`ExportDialog.vue`。

## SPEC-LOG-2 日志查看窗体

- **需求**：应用内提供日志查看窗体（无需离开程序查看日志文件）。
- **验收**：
  - 入口：菜单「工具 → 查看日志」；
  - 内容：读取 logs 目录最新日志文件末尾 N 行（默认 1000，可调 100~5000；兼容轮转文件）；
  - 能力：级别过滤（ERROR/WARN/INFO/DEBUG）、关键字过滤、**自动刷新**（2s，可开关）、行数统计、复制、打开日志目录、清空（二次确认）；
  - 行级着色：ERROR 红 / WARN 橙 / DEBUG 灰。
- **落点**：Rust `commands::log_read/log_clear`、`components/LogViewer.vue`、`App.vue`（工具菜单入口）。

## SPEC-LOG-3 日志面板（活动日志，底部吸附）

- **需求**：应用内实时活动日志窗体，记录 SQL 执行成败、连接断开/恢复等运行事件，普通用户无需打开日志文件即可了解发生了什么。
- **验收**：
  - 位置：**默认吸附于主区下边框**（状态栏上方），顶部边缘可拖拽调高（120~480px，持久化 `personaldb.ui#logPanelHeight`）；显隐持久化 `personaldb.ui#logPanelVisible`（默认开）；
  - 入口：「查看 → 日志面板」（Ctrl+L，可改键）、状态栏「日志」切换（含**未读错误徽标**：面板关闭期间每条 error 计数，打开清零）；
  - 内容：SQL 执行（成功 info：SQL 摘要/行数/耗时；失败 error：附完整错误详情）、多语句（部分失败 warn）、分页续拉失败、连接打开/断开/丢失/重连失败（连接事件与 SQL 失败去重防刷屏：`markLost` 重复标记静默）；
  - 交互：级别过滤（全部/错误/警告/信息）、自动滚动（上翻则暂停贴底）、点击行展开/收起完整详情、清空、收起面板；
  - 存储：**会话内存环形缓冲，上限 1000 条，不落盘**（SQL 历史持久化仍由查询历史 SQLite 负责）。
- **落点**：`stores/logs.ts`（`useLogStore`：entries/unreadErrors/push/markOpened/clear、`sqlTitle` 摘要）、`components/LogPanel.vue`、数据源接线 `stores/tabs.ts`（runTab 成败/多语句/重连/分页）、`stores/connections.ts`（open/close/markLost/reconnect）、`QueryTabPane.vue`（结果集编辑应用失败）、`App.vue`（挂载/菜单/状态栏/快捷键）、`utils/shortcuts.ts`（toggleLogPanel）。

## SPEC-UI-23 工作台可停靠布局（拖拽重排 / 悬浮层叠）

- **需求**：连接管理器、功能区页签容器、日志查看器三大面板可拖拽调整布局；小组件可悬浮为自由窗口并层叠展示。
- **验收**：
  - 停靠区模型：`left / right / bottom / center` 四区；**center 常驻**（不允许为空，中心面板不可隐藏/悬浮，其余面板可与中心交换位置）；默认 `conn=左 / tabs=中 / log=下`；
  - 拖拽重排（业界通用交互）：面板头（PanelHeader，抓取点 + 悬浮/隐藏按钮）`draggable`，**拖拽悬停时在真实布局上预览落点**——目标区显示蓝色虚线遮罩与操作提示（空区=「停靠×侧」、占用区=「与「面板名」交换」），拖动源半透明 ghost；面板隐藏的空区在**窗口边缘显示插入热区**（左/右/下）；松落执行移动/交换，Esc 或拖回原处取消（dragend 清理）；
  - **落点仅响应面板拖拽**：停靠区/热区的 `dragover`/`drop` 必须先判 `layout.dragging` 才 `preventDefault` 并置 `dropZone`（遮罩 `v-if` 也带该条件）。否则 **sortablejs 列排序等原生 HTML5 拖拽**（结果集/表列头）冒泡到停靠区会误置 `dropZone`，因其从未置位 `dragging`、watch 不会清零，导致「与「功能区」交换」遮罩**永久卡住**；
  - **双击面板头 = 停靠 ⇄ 悬浮**（中心区面板除外）；
  - **中心区标题栏按需渲染**：`layout.centerPanel === 'tabs'` 时功能区自带页签栏作头部；**换入其它面板必须补渲染 `PanelHeader`**，否则该面板无法再次拖拽/还原；中心区的 `PanelHeader` 保持自然高度（禁止被 `flex: 1` 拉伸）、内容区占满剩余空间；
  - 区间分割线拖拽调尺寸（left 200~560 / right 140~420 / bottom 120~480，持久化）；
  - **窄容器自适应**：停靠区宽度可小至 140px，面板头必须允许换行（`flex-wrap` + `row-gap`）、文本节点 `min-width: 0` + 省略号，禁止出现控制项被裁切/溢出（`overflow: hidden` 的停靠区会直接切掉超宽内容）；
  - **重置布局**：查看菜单「重置布局」+ 全局快捷键 `Ctrl+Alt+R` → `layout.resetLayout()` 并提示「已还原默认布局」，用于拖乱后一键恢复默认（连接左 / 功能区中 / 日志下 + 默认尺寸，清空悬浮窗与最小化态）；
  - 悬浮窗（FloatWindow）：标题栏拖拽移动、右/下/角缩放、点击置焦（z 序递增）、级联偏移开新窗；`回收停靠`（act_dock 图标）回最近区、`最小化`（Minus，与主窗标题栏最小化同语义同图标）收进**右下角层叠条**（chip 点击还原）、`隐藏面板`（act_hide 图标）；
  - 图标约定：面板动作一律用自绘 `act_float / act_hide / act_dock`（utils/icons.ts），与主窗标题栏的最小化/最大化/关闭图标明确区分；
  - 持久化：`personaldb.layout`（zones/floats/sizes，防抖落盘，启动恢复）；`resetLayout` 可重置；
  - 容器化约定：面板容器统一由 `PanelHeader`（标题 + 拖拽手柄 + 悬浮/隐藏）承载标题栏；**面板内容组件在停靠/悬浮时一律以 `embedded` 渲染，禁止重复自带头部标题**（如 `ConnectionTree` 内部工具栏仅保留动作按钮并右对齐、`LogPanel` 隐藏自带头部标题与收起按钮但保留过滤/清空/自动滚动）；
  - 附带重构：功能区容器抽离为 `workbench/TabArea.vue`（分屏/右键菜单/分屏选择自包含；**仅顶部页签栏**——原 `tabBarPosition` 竖排页签栏（左/右 rail）已废弃：它渲染在 TabArea 内部、结构上无法吸附主窗左边且样式简陋，面板级切换职责由停靠区 + 主窗左侧面板条承担）；原「左侧连接+页签组合侧栏」（leftPanelMode tabs/stack）由工作台停靠取代并从首选项移除；
  - 入口：查看菜单「连接管理器 / 日志面板」显隐切换、查看菜单「重置布局」（`Ctrl+Alt+R`）、状态栏「日志」切换（含未读错误徽标）、Ctrl+L、主窗左侧面板条（SPEC-UI-26）。
- **落点**：`stores/layout.ts`、`components/workbench/{TabArea,PanelHeader,PanelBody,FloatWindow,FloatLayer,PanelRail}.vue`、`App.vue`（工作台装配）、`utils/shortcuts.ts`（resetLayout）、`stores/preferences.ts`（移除 logPanelVisible/logPanelHeight/tabBarPosition/tabBarWidth）、`tests/layout.spec.ts`。

## SPEC-UI-24 面板工具栏三段式（会话 / 表结构 / 日志等）

- **需求**：面板内部工具栏按职责分区（业界对齐 DBeaver Session Manager / SSMS Activity Monitor），功能区划分有条理、学习成本低、各面板观感一致且与其它组件易区分。
- **验收**：
  - 三段式（左→右）：**段1 视图切换**（segmented，最左 = 面板身份）→ `.pt-sep` → **段2 监视/过滤**（刷新、自动刷新「标签+下拉+上次刷新」一组；搜索、状态过滤、下拉、复选框）→ `.pt-spacer` → **段3 动作/状态**（导出、批量危险动作靠右；统计 chips 与连接身份 chip 最右）；
  - 共享原子类在 `styles/tokens.css`：`.pt-sep`（1px 竖分隔线）/`.pt-spacer`（弹性空间），面板 scoped 样式直接复用；
  - **危险批量动作必须与过滤组明确隔开**（分隔线 + `danger plain` 文字按钮 + 未选中禁用，DBeaver「选中动作贴近表格」同型）；
  - 图标按钮一律 circle + tooltip（同 SPEC-UI-8）；过滤类控件保持 12px small 档；
  - 落点：`SessionPane.vue`（会话/锁视图切换）、`TableInspectorPane.vue`（pane-tools：过滤/显示｜结构编辑｜预览）、`LogPanel.vue`（log-head：级别｜自动滚动/清空）。

## SPEC-UI-25 组件边界系统（内部功能带 / 外部区间线）

- **需求**：各组件内部与外部边界明显清晰——面板之间、功能带与内容区之间层次分明。
- **验收**：
  - **功能带统一视觉**（面板头 PanelHeader、面板工具栏、页签栏、日志头、结果集 meta 行）：浅填充底 `var(--pd-band-bg)` + 1px 下边线 `var(--pd-band-border)`（tokens.css 主题感知令牌），内容区保持主背景 → 「带=chrome，内容=画布」的层次语言（VSCode/SSMS 同型）；
  - **外部区间线**：左/右停靠区朝向主区一侧 1px 边线（`.wb-dock.is-left` border-right / `.is-right` border-left），下停靠区保留 border-top；拖拽分隔线（透明 4/5px）叠加于边线之上，悬停高亮不变；
  - 已有边界的组件（TabArea `.tabbar`、状态栏、主菜单）保持原样，不重复叠加；
  - 落点：`styles/tokens.css`（`--pd-band-bg`/`--pd-band-border`）、`App.vue`、`workbench/PanelHeader.vue`、`SessionPane.vue`、`QueryTabPane.vue`（result-meta）、`TableInspectorPane.vue`（inspector-toolbar/pane-tools）、`LogPanel.vue`（log-head）。

## SPEC-UI-26 主窗左侧导航条（Activity Bar 模式）

- **需求**：面板级操作（显示/隐藏/唤回）此前入口分散（菜单 + 状态栏 + 快捷键），可发现性低；且面板隐藏后缺少稳定的「唤回」位置。参照 VSCode Activity Bar / JetBrains 工具窗条（业界标准做法：导航条不是窗体容器，而是面板的**常驻开关 + 唤回器**）。
- **验收**：
  - **位置**：`workbench/PanelRail.vue`（导航条）为 `app-body` 的第一个子元素 → **恒定吸附主窗最左边**（x=0，宽 `--pd-rail-w`，左停靠区在其右侧）；不随面板停靠/悬浮变化，不参与拖拽重排（属窗口 chrome）；
  - **按钮**：每个可停靠面板一枚图标按钮（连接 `obj_connection` / 功能区 `act_workspace` / 日志 `act_monitor`），尺寸 36×36（图标 19px，导航条宽 44px，视觉重量对齐 VSCode）；
  - **状态区分（必须有明显区分度）**：**可见 = 主色图标 + 淡主色底（`--el-color-primary-light-9`）+ 贴窗口左缘的 2px 指示条**；**隐藏 = 灰阶（`--el-text-color-placeholder`）且图标压暗至 45%**（无底色、无指示条）；**悬停 = 恢复满不透明 + `--el-fill-color` 底 + 主色文本**；日志按钮附加未读错误数徽标（同状态栏语义，`logs.unreadErrors`）；
  - **点击语义**：停靠→隐藏 / 隐藏→唤回（回 `lastZone`）/ 悬浮→置焦 / 悬浮最小化→还原；**中心常驻面板无动作**（tooltip 追加 `workbench.pinned`「常驻主区」）；tooltip = 面板名（+ 快捷键 / 常驻主区）；
  - **图标唯一性**：严格遵循 SPEC-UI-8，功能区使用为此新增的 `act_workspace`（窗口 + 顶部页签带），不得复用 `obj_query` 等查询语义图标；
  - **拖拽热区避让**：面板拖拽时的左缘插入热区 `.edge-hot.edge-left` 起点为 `var(--pd-rail-w)`，不得覆盖导航条；
  - 落点：`components/workbench/PanelRail.vue`、`App.vue`（装配 + `togglePanel` + `.edge-left`）、`utils/icons.ts`（`act_workspace`）、`styles/tokens.css`（`--pd-rail-w`）。

## SPEC-UI-27 页签栏动作区（标签管理）图标化

- **需求**：页签栏右侧「标签管理」动作区（页签列表 + 分屏控制）原为**文字按钮**（左右 / 上下 / 单屏 + 蓝色实心 active），与全应用「图标按钮 + tooltip」规范不一致、占地大且观感笨重。
- **验收**：
  - **一律图标化 + tooltip**（禁止文字按钮）：页签列表 = EP `List` + 数量（11px 次级色），左右分屏 = `act_split`，上下分屏 = `act_split_stack`，退出分屏 = `act_unsplit`；
  - **状态语言与导航条（SPEC-UI-26）一致**：active = 主色图标 + `--el-color-primary-light-9` 淡底；hover = `--el-fill-color` 底 + 主文本色；中立态 = 次级文本色；按钮尺寸 24px（分屏段内 26px 定宽）；
  - **分屏段为分段控件形态**：段内 1px 分隔（`--el-border-color-lighter`）、整体圆角容器；当前分屏方向高亮；
  - **退出分屏按钮仅在分屏时出现**（默认态隐藏而非禁用——灰方块易被误读为空控件）；
  - 页签右键菜单的分屏项图标与方向一一对应（左右 = `act_split`，上下 = `act_split_stack`，退出 = `act_unsplit`）；
  - 页签列表弹层：过滤输入 + 选项（连接色点 / 类型图标 / 标题 / 连接名 / 关闭），当前页签淡主色高亮；选项行 `padding 5px 6px`、圆角 `--pd-radius`，列表体左右负边距对齐。
- **落点**：`components/TabBarActions.vue`、`components/workbench/TabArea.vue`（右键菜单图标）、`utils/icons.ts`（新增 `act_split_stack` / `act_unsplit`）、`i18n`（移除弃用的 `split*Short` 文案）。

## SPEC-UI-28 事务管理区（主窗体工具栏）

- **需求**：事务管理位于主窗体工具栏、作用于当前激活查询 Tab 所属连接。原实现为「文字下拉（⚡ 自动 ▾）+ 仅手动模式出现的提交/回滚」：观感笨重、切模式需两次点击、切手动时工具栏宽度跳动。
- **验收**：
  - **结构**：`[act_tx 图标（组身份 + 提示）] [分段：自动 | 手动] ␟ [提交] [回滚]`，段间用共享原子类 `.pt-sep`；
  - **模式用分段控件**（`el-segmented` small，选中项 = 主色 + `--el-color-primary-light-9`）：持久显示当前模式、一次点击切换（原下拉需两次）；忙碌时（txBusy）禁用；
  - **提交 / 回滚一律为图标按钮**（`act_commit` / `act_rollback`，26×24）**且两种模式下均常驻**：自动模式禁用（opacity 0.38）——布局**零跳动**（禁止用 v-if 让按钮在切模式时出现/消失）；禁用态的 tooltip 由外层 `.tx-slot` 承载（禁用元素不触发 tooltip）；
  - **语义配色**：提交 hover = 主色 + 淡主色底（确认）；回滚 hover = 危险色 + 淡危险底（破坏性）；
  - 提示 `txTip`：手动模式显示「事务 · 手动模式下 DML 不自动提交，请显式提交或回滚」；
  - 能力驱动（`capabilities.transactions`）显隐；无激活页签/未连接时隐藏。
- **落点**：`App.vue`（事务区模板 + `.tx-lead/.tx-mode/.tx-acts/.tx-slot/.tx-act` 样式 + `txManual/txModeOptions/txTip`）。

## SPEC-UI-29 菜单栏图标化与工具栏布局

- **需求**：菜单栏各下拉项原为纯文字（无图标，扫描性差）；工具栏控件间距松散（相邻按钮默认 12px 外边距 + 容器 10px gap 混用）、状态文字为裸文本、右侧图标按钮（主题/首选项/会话）尺寸不一。
- **验收**：
  - **菜单栏项一律带图标**（禁止纯文字项）：图标来自 `App.vue#MENU_ICONS` 映射表（唯一来源，禁止在模板内散落图标判断），语义严格遵循 SPEC-UI-8；同一动作在工具栏 / 菜单 / 命令面板使用**同一图标**（如 主题切换 = `act_theme` 三处一致，替换原工具栏 EP Sunny/Moon 与命令面板 `act_refresh`）；
  - **菜单项布局（紧凑）**：下拉面板 `min-width: 176px`、内边距 4px；项高 26px、内边距 `0 8px`、圆角 4px、字号 12px；图标槽 15px 定宽 + 8px 间隔、图标 14px；文本 `flex: 1`、快捷键右对齐（`menu-shortcut`）；hover/active 时图标转主色；子菜单标题同规格（箭头 `margin-left: auto`）；
  - **实现约束（易回归，务必遵守）**：`el-menu--popup` 由 EP **teleport 到 body**（不在 `.app-menu` 内），故弹层样式**必须写在全局 `styles/overrides.css`**（App.vue 的 scoped `:deep` 选择器对弹层无效）；项高/内边距需 **3 类优先级**（`.el-menu--popup.el-menu .el-menu-item`）才能压过 EP 水平菜单的 `.el-menu--horizontal .el-menu--popup .el-menu-item` 规则（否则退回 36px 高）；
  - **分组分隔**：同一下拉内按语义分组，组间 `.menu-sep`（1px、`--el-border-color-lighter`、`margin: 4px 6px`）——查看 3 组 / 编辑 2 组 / 工具 2 组 / 帮助 2 组；
  - **工具栏布局**：统一 `gap: 6px` 节奏并移除 EP 相邻按钮默认外边距；竖向分隔线统一 `height: 16px`；
  - **连接状态 chip**：裸文本改为胶囊（22px 高、`12px` 字、状态点 6px）：已连接 = 成功色淡底 + 实心点，未连接 = 中性 fill 底 + 半透明点；
  - **右侧图标按钮统一**（主题 / 首选项 / 会话）：`.hdr-btn` 26×24、透明底、hover = `--app-header-hover-bg` + 主色，与标题栏窗口按钮同型；禁止再用带 padding 的 `el-button.is-text` 混排。
- **落点**：`App.vue`（菜单模板 + `MENU_ICONS` + 工具栏模板 + `.app-menu/.conn-state/.hdr-btn/.header-divider` 样式）、`styles/overrides.css`（`el-menu--popup` 紧凑菜单 + `.menu-icon/.menu-sep/.menu-shortcut`）、`utils/icons.ts`（新增 `act_theme`/`act_driver`/`act_update`/`act_view_grid`/`act_info`）。

## SPEC-UI-30 空状态 / 图标尺寸令牌 / 键盘焦点环

- **需求**：空状态实现不一（散落 `el-empty` 违反约定）；图标尺寸 6 档散落数字（12/13/14/15/16/19）导致视觉不齐；自绘原生 `button`（导航条/页签动作/工具栏/面板头/悬浮窗）无键盘焦点样式，键盘用户无法定位。
- **验收**：
  - **空状态**：一律用 `components/EmptyState.vue`（icon=APP_ICONS key + title/hint + `#action` 插槽），禁止散落 `el-empty`；连接树空态（无连接 → 新建连接按钮）与例程参数空态均已接入；
  - **图标尺寸令牌**（`styles/tokens.css`）：`--pd-icon-sm: 13px`（工具带/小按钮）/ `--pd-icon-md: 15px`（面板头与动作按钮）/ `--pd-icon-lg: 17px`（导航条/主工具按钮）；组件内用 CSS（`font-size: var(--pd-icon-md)`）指定，**新代码禁止再散落 `:size="13"` 之类的数字**；
  - **焦点环**：自绘按钮统一 `:focus-visible { outline: 2px solid var(--el-color-primary); outline-offset: -2px }`（EP `el-button` 保留其自身焦点样式）；覆盖 `.rail-btn / .tba-btn / .hdr-btn / .tb-btn / .tx-act / .ph-btn / .fw-btn / .chip`。
- **落点**：`components/EmptyState.vue`（复用）、`ConnectionTree.vue`、`RoutineEditor.vue`、`styles/tokens.css`、`workbench/{PanelRail,TabBarActions,PanelHeader,FloatWindow,FloatLayer}.vue`、`App.vue`。

## SPEC-UI-31 右键菜单统一（ContextMenu）

- **需求**：全应用原有多套各自实现的右键菜单（连接树 `.ctx-menu`、页签 `.tab-ctx-menu`、结果单元格 `.cell-menu`、会话表头 `.hdr-menu`、表查看表头 `.insp-hdr-menu`），样式与收起行为各写一遍、易漏（历史上多次出现「菜单不收起/永久滞留」缺陷）。
- **验收**：
  - **唯一实现** `components/ContextMenu.vue`：`Teleport` 到 body + 视口内收边（右/下溢出自动回退）+ **全局收起集中实现**（窗口失焦/缩放、他处左键或右键 capture、滚动、Esc）——调用点**禁止**再自行注册收起监听；
  - **API**：`:visible`（v-model）、`:x/:y`、`:items="CtxMenuItem[]"`，`@select="key"`；项模型 `{ key, label, icon?, danger?, disabled?, hint?, title?, sep? }`，`icon` 支持 `APP_ICONS` key 或 EP 组件；
  - **视觉统一**：`min-width 168px`、项 `13px` 字 / `padding 5px 10px` / hover 主色淡底、危险项危险色淡底、禁用项 45% 并禁点、`.pd-ctx-sep` 分组线、`hint` 右对齐等宽字体；
  - **迁移范围**：连接树节点（按 `nodeType` 动态成组，含「已连接则不显示连接项」等条件项）、页签（重命名/分屏/关闭族）、结果单元格（按选区/视图/编辑态成组）、会话表头与表查看表头（移动到最前/最后/重置列，二者同构）；
  - 选择即执行并自动收起；`defineExpose`/旧 handler 保留但仅由 `onMenuSelect(key)` 派发（避免行为漂移）。
- **落点**：`components/ContextMenu.vue`（新增）、`ConnectionTree.vue`、`workbench/TabArea.vue`、`QueryTabPane.vue`、`SessionPane.vue`、`TableInspectorPane.vue`。

## SPEC-UI-32 对话框规范（标题图标 / 宽度档位 / 浮层主题）

- **需求**：各对话框原为纯文字标题、宽度取值零散（400/420/460/480/520/540/560/620/640/660/720/760/920），视觉不统一。
- **验收**：
  - **标题一律带图标**：`<template #header><span class="dlg-title"><el-icon>…</el-icon>{{ title }}</span></template>`（`.dlg-title` 为 tokens.css 共享原子类：flex + 8px 间隔 + 15px 半粗 + 主色图标）；图标取 SPEC-UI-8 体系（连接 `obj_connection` / 首选项与快捷指令 `act_settings` / 驱动 `act_driver` / 配置互通与导出 `act_export` / 日志查看 `act_monitor` / 关于 `act_info` …）；
  - **宽度四档**：S=420（关于、小确认）/ M=560（连接表单、导出、详情）/ L=720（首选项、配置互通、快捷指令、预览类）/ XL=920（驱动管理、日志查看器）；
  - 首选项左侧分类图标**全部使用自绘体系**（外观 `act_theme` / 编辑器 `act_code` / 源码查看器 `act_code_view` / 结果集 `act_view_grid` / 数据格式 `act_format` / 存储 `obj_folder` / 快捷键 `act_keyboard` / 关于 `act_info`）；
  - 工具栏下拉（工作区）各项同样带图标（列表项 `act_workspace`、新建 `Plus`、重命名 `EditPen`、导出目录 `act_export`、更改位置 `FolderAdd`、打开位置 `FolderOpened`）；
  - **driver.js 漫游弹窗改用主题令牌**（`styles/overrides.css`：`.driver-popover*` 背景/文字/按钮/箭头均取 `--el-*`），深浅色一致，禁止保留 driver 默认浅色样式。
- **落点**：`styles/tokens.css`（`.dlg-title`）、`styles/overrides.css`（driver.js）、`components/{ConnectionForm,SettingsDialog,DriverManager,ConfigTransferDialog,AboutDialog,LogViewer,ExportDialog,SnippetSettings}.vue`、`App.vue`（工作区下拉）、`utils/icons.ts`（`act_code`/`act_code_view`/`act_keyboard`）。

## SPEC-UI-33 无障碍基线（图标按钮 / 键盘可达）

- **需求**：图标按钮普遍缺少可访问名称（`aria-label` 全仓 0 处），纯 el-tooltip 包裹的按钮对读屏不可用；状态栏日志入口仅支持鼠标点击。
- **验收**：
  - **图标按钮必须有可访问名称**：优先 `aria-label`（与 tooltip 文案一致）——导航条三枚、页签动作区三枚、工具栏主题/首选项/会话、事务区提交/回滚、查询工具栏全部圆形按钮（执行/执行选中/执行当前行/停止/快捷指令/格式化/压缩/执行计划/保存脚本/打开脚本/重连）、日志面板清空/收起、历史入口；其余依赖 EP `title` 的图标按钮视为已有名称（`title` 参与可访问名计算）；
  - **浮层样式需注意加载顺序**：第三方 CSS（如 driver.css）在本模块之后加载时，同优先级会覆盖，主题化选择器需提升优先级（如 `.driver-popover.driver-popover …`）——driver.js 弹窗在深浅色下均须与整体配色一致（背景/文字/按钮/箭头取 `--el-*`）；
  - **自绘可点元素键盘可达**：`role="button"` + `tabindex="0"` + `Enter`/`Space` 触发（如状态栏日志入口），且带 `:focus-visible` 焦点环；
  - 焦点环规范见 SPEC-UI-30（2px 主色内描边）；
  - 深色主题逐面板核查对比度（连接树/查询结果/设置/日志面板），除 Element Plus 主色按钮自身的固有比值外不得出现 <3:1 的正文/图标对比。
- **落点**：`components/workbench/{PanelRail,TabBarActions}.vue`、`App.vue`（hdr-btn/tx-act/状态栏）、`components/QueryTabPane.vue`（查询工具栏）。

## SPEC-CONN-1 连接错误友好提示

- **需求**：连接失败必须给出**可操作的一行摘要**（原始报错链 `数据库错误: Input/output error: ... (os error 10061)` 对用户无意义），同时保留原始文本供排查。
- **验收**：
  - 双层输出：`describeConnError(err)`（toast / 抛出给调用方的一行摘要）与 `connErrorDetail(err)`（原始文本，写入日志详情）；
  - **命中即摘要**（顺序敏感，先具体后泛化，规则表见 `utils/connError.ts#RULES`）：
    - Windows 套接字码：`10061` 拒绝 / `10060` 超时 / `10065`·`10051` 主机不可达 / `11001` DNS 解析失败；
    - Oracle ORA 码：`ORA-12541` 无监听（拒绝）/ `12514`·`12505`·`12154` 服务未注册 / `12170` 超时 / `01017` 认证失败 / `28000` 账号锁定；
    - 驱动/服务端文案：`Access denied for user`·`password authentication failed`·`authentication failed` / `Unknown database`·`does not exist` / TLS 握手失败；
    - OceanBase Oracle 租户（服务端 `1235` / `Oracle tenant ... not supported`）→ 引导改用「OceanBase (Oracle)」类型；内置驱动缺失（`oracle-bridge`）→ 引导改用 OCI 或重新构建；
  - **未命中即清理**：折叠空白为单行、剥离多层包装前缀（`数据库错误:` / `Input/output error:` / `Error:`）、超长截断 200 字符；
  - **幂等**：已友好化的文本再次传入（调用方二次包装）不再叠加；
  - **入口全覆盖**：连接树「连接 / 测试连接」、连接表单「测试连接」、命令面板连接项、快速新建查询、会话恢复（`stores/connections.ts#openConnection`）一律走 `describeConnError`，其余入口不得直接 `String(e)`；
  - **文案不重复前缀**：toast 模板 `common.connectionFailed` 已含「连接失败：」，摘要文本自身不得再以「连接失败 / 连接被拒绝：」开头；
  - 日志：连接失败 detail = `友好摘要 + 换行 + 原始文本`（便于日志面板直读与复制排查）。
- **落点**：`utils/connError.ts`、`stores/connections.ts#openConnection`、`components/ConnectionForm.vue`、`components/ConnectionTree.vue`、`App.vue`（快速新建查询 / 命令面板）、`tests/conn-error.spec.ts`。

## SPEC-CONN-2 Oracle 连接方式（内置驱动 / OCI）

- **需求**：默认连接 Oracle / OceanBase Oracle **无需安装 OCI 客户端**；保留 OCI 方式供特殊环境使用。
- **机制**：两类内置 sidecar，均经 NDJSON over stdio 桥接执行（一进程一连接）：
  - **Oracle** → `oracle-bridge`（Go + go-ora，纯 Oracle 协议），`build.rs` 调 Go 工具链编译；
  - **OceanBase Oracle** → `ob-jdbc-bridge`（Java + OceanBase Connector/J），`build.rs` 调 Maven 打包 fat jar（**需 JRE 21+**）。
  - OceanBase 的 Oracle 模式本质是 **MySQL 协议 + OB 扩展能力位**（capability bit 27 `OB_CLIENT_SUPPORT_ORACLE_MODE`，服务端 `obmp_connect.cpp` 校验），标准 Oracle TNS 客户端（go-ora）无法连接 → 必须用 OB 官方驱动。缺 Go/Maven 则跳过并告警。
- **选择规则**（`db/pool.rs#create_driver`）：`params.driver == "oci"` → OCI 驱动；否则对应内置桥接可用 → 桥接驱动；桥接缺失 → 回退 OCI 驱动。
- **会话内 schema 切换**：桥接驱动 `execute` 前按 UI 选中库执行 `ALTER SESSION SET CURRENT_SCHEMA`（标识符经方言 `quote_ident` 转义；库切换跟踪 `current_schema`，`None` 复位为用户默认 schema，兼容 OB `user@tenant` 用户名取 user 部分）；切换失败 fail-hard；`ping` 直连 `query` 绕过切换逻辑（不打断会话内状态）。OCI 驱动连接池跨连接持有，**不做** schema 切换（沿用 owner 过滤语义）；元数据查询走 `user_*/all_*` + owner 绑定，不依赖 CURRENT_SCHEMA。
- **连接表单**：Oracle / OceanBase Oracle 类型展示「连接方式」下拉（内置驱动 / OCI 客户端），写入 `params.driver`；内置驱动不可用时该选项禁用并提示。OceanBase Oracle 的用户名提示 `用户@租户`，库名字段为 **Schema（留空默认 = 用户名）**。
- **验收**：未安装任何 OCI 库时可连接 Oracle / OceanBase Oracle；`cargo test --lib` 的 `db::bridge::tests` 验证两类 sidecar ping 往返；`--no-default-features` 下仍可用内置驱动。
- **落点**：`src-tauri/build.rs`、`src-tauri/oracle-bridge/`、`src-tauri/ob-jdbc-bridge/`、`db/bridge.rs`、`db/bridge_driver.rs`、`db/pool.rs`、`tauri.conf.json#bundle.resources`、`commands.rs#bridge_status`、`ConnectionForm.vue`（文案 `form.oracleDriver` 等）。

## SPEC-CONN-3 通用 JDBC 与驱动管理

- **需求**：支持以任意 JDBC 驱动连接数据库；驱动的 jar / 驱动类 / URL 模板可配置，并支持新增/复制/编辑/删除（参考 DBeaver/Navicat）。
- **驱动模型**（`db/jdbc_driver.rs`，持久化 `drivers.json`）：`{ id, name, driverClass, urlTemplate, defaultPort, jars[], builtin }`；内置 OceanBase 预设（`builtin=true` 只读、可复制）。驱动来源**仅本地 jar/目录**。
- **驱动管理**（工具 → 驱动管理，`DriverManager.vue`）：列表 + 新建/复制/编辑/删除；表单含驱动类、URL 模板、默认端口、本地 jar/目录列表；**查找类**（`jdbc_driver_scan_classes` 经 ServiceLoader 扫描）。
- **连接类型**：`DbKind::Jdbc`；`db/jdbc.rs#GenericJdbcDriver` 经 jdbc-bridge（`java -cp "<jdbc-bridge.jar>;<jars>" personaldb.JdbcBridge`）连接；URL 取参数 `jdbcUrl` 或由驱动 `urlTemplate` 结合 host/port/database 生成；对象树走 JDBC `DatabaseMetaData`（`metaTables/metaColumns/metaIndexes/...`），方言为 `dialect/generic.rs`（保守 ANSI，`catalog_query` 的 `sql` 仅作类型标记）。
- **会话内 schema 切换**：`execute` 前 best-effort 切换——按 `driver_class` 小写嗅探方言（oracle → `ALTER SESSION SET CURRENT_SCHEMA`；mysql/mariadb/oceanbase → `USE`；postgresql/pgjdbc → `SET search_path`；未知驱动跳过），失败仅 WARN 不中断用户查询；无论成败记录尝试值（避免逐查询重试），`None` 复位为连接默认；`ping` 直连 `query` 绕过切换。
- **验收**：驱动管理可增删改并扫描驱动类；JDBC 连接表单可选驱动；真机连接 OceanBase（`builtin-oceanbase`）可列出 schema/表。
- **落点**：`src-tauri/jdbc-bridge/`、`db/jdbc_driver.rs`、`db/jdbc.rs`、`db/dialect/generic.rs`、`db/bridge.rs`、`db/pool.rs`、`storage.rs#StorageCategory::Drivers`、`commands.rs#jdbc_driver_*`、`DriverManager.vue`、`ConnectionForm.vue`、`types/db.ts`。

## SPEC-CONN-4 SQLite 嵌入式连接

- **需求**：支持 SQLite 3 单文件数据库（嵌入式，无服务端、无账号体系）；连接 = 本地数据库文件路径。
- **选择规则**：`DbKind::Sqlite`；驱动为 **rusqlite（bundled，含 `column_decltype`）**，零外部依赖；`pool.rs#create_driver` 直接分支，无回退。
- **连接模型**：
  - 文件路径存 `params.file`（兼容 `database` 字段回退），ConnectionForm 提供文件选择对话框（`.db/.sqlite/.sqlite3/.db3`），未选文件保存时拦截；
  - 打开标志 **不带 `SQLITE_OPEN_CREATE`**——文件不存在直接报错，杜绝静默建空库；`busy_timeout` 5s；
  - `list_databases` 固定 `["main"]`（ATTACH 库不管理）；主机/端口/账号/密码 UI 全部隐藏，无凭据存储。
- **能力**（`dialect/sqlite.rs`）：explain ✓（`EXPLAIN QUERY PLAN` 行式输出，`db_explain` 特判经 `plan::sqlite_plan_from_rows` 构建树）、DDL ✓（`sqlite_master.sql` 原文）、INSERT ✓（二进制 `X'HEX'`）、手动事务 ✓（专用连接 + `tx_open` 跟踪，与 PG 同模式）、会话管理 ✗（嵌入式无服务器会话）；对象类型：表/视图/索引/触发器（无序列/函数/存储过程）。
- **实现约束**：rusqlite 同步 API 一律 `spawn_blocking` 包装；内部对象（`sqlite_sequence`/`sqlite_autoindex_*`）在目录/快照中统一 `NOT LIKE 'sqlite\_%' ESCAPE '\'` 过滤；改列类型不自动生成 DDL（需整表重建，避免有损变更）；导出文件头 `PRAGMA foreign_keys=OFF`。
- **落点**：`db/sqlite.rs`（传输）、`db/dialect/sqlite.rs`、`db/plan.rs#sqlite_plan_from_rows`、`db/registry.rs`、`pool.rs`、`commands.rs#db_explain`、`ConnectionForm.vue`（文件选择）、`DbLogo.vue`、`types/db.ts`、`utils/connError.ts`（文件缺失/被锁/非数据库三规则）。

## SPEC-RESULT-6 结果集导出（选中行 / 过滤结果）

- **需求**：将结果集**选中行**（无选中则为当前过滤后全部行）导出为 **Excel / SQL INSERT / CSV / TXT**；列范围默认「可见列」，可选择具体列；所有可配置项须在**首选项**可改。
- **行/列范围**：有勾选行 → 仅选中行；否则当前过滤/搜索后的行。列 → 默认全部**展示列**（列显隐生效），SQL 导出对话框可再勾选。
- **默认文件名与目录**：`export_{表名}_{yyyyMMddHHmmss}.{ext}`；目录默认**当前工作空间 `exportDir`**（未设置则弹系统保存框）。表名取 `tab.table`，无则 `query_result`。
- **编码**：UTF-8；CSV 恒定写 BOM（Excel 友好）；SQL/TXT 的 BOM 可选（`bom`）。
- **SQL INSERT 选项**（首选项默认 + 对话框本次覆盖）：
  - 去掉标识符引号（`unquoteIdent`）；标识符大小写原样/大写/小写（`identCase`）
  - 日期/时间**显式转换**（`dateConvert`，默认开）：按值的显示形态推断格式（`20200101`→`TO_DATE('20200101','YYYYMMDD')`、含毫秒→`TO_TIMESTAMP(…FF)`；MySQL `STR_TO_DATE`、PG `TO_DATE/TO_TIMESTAMP`、未知库回退引号串）
  - **每条 INSERT 合并行数**（`batchSize`，默认 10，1..1000；**1 = 每行单独一条 INSERT**）
  - **值转换规则**（`valueRules`）：按列语义将非空值包裹为模板函数，`{v}`=值字面量；内置预设 `TO_CLOB`(longtext)、`NVL`/`COALESCE`(text)；NULL 不套规则
- **落点**：后端 `db/export.rs#export_rows` + `RowsExportRequest`（`columns` 带语义、`sqlOptions`）与命令 `commands.rs#export_result_rows`；前端 `utils/clipboard.ts`、`QueryTabPane.vue`（导出下拉 + SQL 选项对话框 + 列筛选）、`ValueRuleEditor.vue`、`stores/preferences.ts#exportPrefs`、`SettingsDialog.vue`（结果集导出小节）。
- **验收**：选行导出四格式；`batchSize=1` 每行一条 INSERT；关闭/开启日期转换观察 `TO_DATE`；新增 `longtext→TO_CLOB({v})` 规则后 CLOB 列被包裹；勾选列后导出仅含勾选列。

## SPEC-RESULT-7 结果集分页续拉（滚动加载）

- **需求**：结果集行数过多时，筛选/渲染卡顿；改为分页拉取 + 滚动续拉。
- **默认**：单页 `result.pageSize=10`（1..100）、总量上限 `result.maxTotalRows=10000`（100..10000），均可在**首选项 → 结果集**修改。
- **行为**：执行查询仅拉首页；表格滚动至距底 60px 触发 `tabs.loadMore`：按已有行数包装分页 SQL（Oracle/OceanBase Oracle 用 `ROWNUM` 三层包装，MySQL/PG/JDBC 用 `LIMIT/OFFSET`），新行**追加**；达上限或返回不足一页即停止。底部提示「加载中… / 向下滚动加载更多」。续拉遇连接类错误复用一次性重连。
- **说明**：续拉基于**SQL 包装分页**（非服务端游标），无 `ORDER BY` 时页边界顺序不保证；分页状态（`baseSql/hasMore/loadingMore`）**不入工作区快照**。
- **落点**：`stores/preferences.ts#result`、`stores/tabs.ts#runTab/loadMore/wrapPaged`、`QueryTabPane.vue`（滚动绑定 + 提示）、`SettingsDialog.vue`。
- **备注**：虚拟滚动表格（`el-table-v2`）未采用——会牺牲列拖拽/表头列设置等既有能力；以「行数受控 + 滚动续拉」替代。

## SPEC-UI-16 界面可调项（布局 / 字体 / 结果交互）

- **可拖拽布局**：连接树↔主区（左右，`personaldb.sidebarWidth`）、编辑器↔结果区（上下，`personaldb.editorHeight`），localStorage 持久化。
- **全局 UI 字体**：首选项→外观（字体族/字号/行高），作用于根元素与 Element Plus 变量，持久化 `personaldb.ui`。
- **记录视图**（`cell-viewers/RecordView.vue`）：属性表布局（字段名 | 值），工具栏含行号跳转与复制整行，逐字段复制；非卡片。
- **表格复制**：结果表格右键（复制单元格/整行/本列/选中行）+ `Ctrl+C`（有选中行复制选区，否则当前单元格）。
- **页签连接信息**：页签显示 `连接名@库`。
- **右键菜单**：全局禁用 WebView 默认右键菜单（`main.ts`），组件使用自定义菜单。
- **连接拖拽**：连接树支持拖拽连接入文件夹 / 移出到根级；`utils/` 参见 `ConnectionTree.vue#allowDrop`。

## SPEC-RESULT-8 结果集编辑（增删改 / 预览 / 事务提交）

- **需求**：在结果集上增（可复制）、删、改；变更须**用户批准**后执行；右键可跳转记录查看器。
- **启用**：仅**单表 SELECT**（`FROM` 单表，含 JOIN/逗号则禁用）；编辑开关（结果工具栏 ✏️）。
- **定位策略**（WHERE）：主键 → ROWID 列（结果含 ROWID 时）→ 否则**全列匹配**并在预览中**高危告知**。
- **交互**：双击单元格内联编辑；右键「新增行/复制行/标记删除/撤销该行修改」；行底色 新增(绿)/已改(黄)/已删(红删除线)；右键「在记录视图中查看」跳转并定位。
- **提交**：`db_build_dml`（`db/dml.rs`，复用方言 `quote_ident`/`literal`）生成 `INSERT/UPDATE/DELETE` → 预览对话框（含**复制 SQL** 按钮 + 高危警告）→ 关闭自动提交 → 逐条执行 → **全部成功才提交**，失败回滚并恢复自动提交 → 刷新结果。
- **落点**：`db/dml.rs#build_dml`、`commands.rs#db_build_dml`、`QueryTabPane.vue`、`RecordView.vue`（`startIndex`）。

## SPEC-EDITOR-4 SQL 美化方案（可配置）

- **需求**：格式化方案可在首选项自定义。
- **选项**（`preferences.formatPrefs`，持久化 `personaldb.format`）：关键字大小写（大写/小写/保持原样）、缩进风格（空格/制表符）、缩进宽度、语句间空行、表达式换行宽度、**方言覆盖**（自动/MySQL/PostgreSQL/PLSQL/ANSI）。
- **落点**：`utils/sqlFormat.ts#formatSql(kind, sql, opts)`；`SettingsDialog.vue`（SQL 美化小节）；调用点 `QueryTabPane.vue#doFormat`、`CodeViewer.vue`。

## SPEC-EDITOR-5 SQL 自动补全

- **需求**：补全来源于**当前 SQL** 与**同 schema 元数据**（表/列），表名/字段名展示 **comments**。
- **元数据**：命令 `db_schema_snapshot(sessionId, database)`（`db/catalog.rs#schema_snapshot`）——优先方言快照 SQL（`SqlDialect#schema_snapshot_sql`：MySQL/OB `information_schema`、PG `information_schema`+`pg_description`、Oracle/OB Oracle `all_tab_columns`+`all_col_comments`），否则 JDBC 元数据回退（最多 500 表）。
- **编辑器**：`SqlEditor.vue` 使用 CM6 `autocompletion` + 自定义 `completionSource`（语言数据注入，保留关键字补全）：表（type=class/interface，detail=TABLE/VIEW，info=comment）、列（detail=表·类型，info=comment）；表数 > 300 时仅补全表名与文档词，避免选项爆炸；按 (连接,库) 惰性加载并缓存。
- **落点**：`db/dialect/{mysql_family,postgresql,oracle}.rs`、`db/catalog.rs`、`commands.rs#db_schema_snapshot`、`SqlEditor.vue`、`api/db.ts#dbSchemaSnapshot`。

## SPEC-EDITOR-6 PL/SQL Beautifier（自研引擎）

- **研究**：PL/SQL Developer 14 Beautifier 规则文件（`.br`/`.brs`）：KeywordCase/IdentifierCase（0 不变/1 大写/2 小写/3 首字母大写）、ItemList.Format（0 同行/1 尽量同行/2 每项一行/3 自适应）、Align、CommaAfter、AtLeftMargin、RightMargin、Indent、UseTab/TabSize、EmptyLines、SplitAndOr、AndOrAfterExpression、WhereAndOrUnderWhere、JoinSplitBeforeOn、OnOneLineIfPossible。
- **实现**：自研引擎 `utils/plsqlBeautifier.ts`（词法分析 + 子句布局），弥补 sql-formatter 能力不足：
  - 词法：保留字符串/引号标识/注释，关键字与标识符各自 4 档大小写
  - 布局：SELECT 元素（Format/Align/CommaAfter，多行时对齐到 `SELECT ` 列）、FROM + JOIN（`LEFT JOIN` 同行、`ON` 可换行）、WHERE（AND/OR 换行、是否置表达式后、缩进）、GROUP BY/ORDER BY 元素列表、`;` 结尾
  - `OnOneLineIfPossible` 且长度 ≤ `RightMargin` 时合并为一行；语句间空行按 `EmptyLines`
  - 集合运算 `UNION/UNION ALL/MINUS/INTERSECT` 后 `SELECT` 另起一行
  - 控制结构：`DECLARE/BEGIN/END` 单独成行、`FOR/WHILE` 断行；`ThenOnNewLine`（THEN/ELSE/ELSIF/IF 换行）、`LoopOnNewLine`（LOOP 换行），`END IF`/`END LOOP` 保持一行
  - 对齐：`AlignDeclarationGroups`（变量名与类型列对齐）、`AlignAssignmentGroups`（`:=` 列对齐）
- **偏好/预设**：`preferences.formatPrefs`（`personaldb.format`），首选项 → 编辑器 → SQL 美化；预设 `PL/SQL Developer 风格` / `默认`（含 `thenOnNewLine`/`loopOnNewLine`/`alignDeclarationGroups`/`alignAssignmentGroups`）。
- **落点**：`utils/plsqlBeautifier.ts`、`utils/sqlFormat.ts#formatSql`、`stores/preferences.ts#FormatPrefs`、`SettingsDialog.vue`。
- **说明**：面向 DML 与常见语句；复杂控制结构（IF/LOOP 等）按通用换行，未逐条对齐。

## SPEC-UI-17 查询面板：快速重连与 SQL 脚本存取

- **快速重连**：查询工具栏在连接离线时显示「重新连接」按钮（`connStore.isConnected` 判定），点击 `tabs.reconnect` 关闭旧会话并重开（成功后刷新库列表），免去进连接管理器。
- **SQL 脚本存取**：工具栏「保存 SQL 至文件」（系统保存框 → `save_text_file`）与「从文件打开 SQL」（`read_text_file`）；**加载后弹窗提示选择连接**（连接下拉），确认后重绑 `connectionId/connectionName/kind`、重置 `database` 与结果。两命令**拒绝触碰存储目录内敏感数据文件**（connections/groups/snippets/workspaces/drivers/history，`is_storage_guarded_file` 规范化路径比对）——前端若被注入，任意文件读写不得波及凭据与配置。
- **落点**：`commands.rs#save_text_file/read_text_file`、`stores/tabs.ts#reconnect`、`QueryTabPane.vue`。

## SPEC-CONN-4 密码安全存储（系统凭据管理器）

- **模型**：`ConnectionConfig.passwordRef` 指向凭据条目（连接 id）；配置文件中 `password` 恒为空，明文只存于 Windows 凭据管理器（`secrets.rs`，keyring）。连接/测试连接时后端 `resolve_secret` 自动取回；用户改密自动回写；**连接复制凭据随复制**（`secret_copy`），删除连接清理凭据。
- **迁移**：启动时 `migrate_connection_file` 扫描 connections.json 明文密码 → 逐条入凭据管理器 → 改引用；单条失败保留明文并 WARN（回退路径）。
- **禁令**：日志禁打印密码（含迁移日志）；导出含密码必须走加密（见 SPEC-TRANSFER-1）。

## SPEC-TRANSFER-1 配置导出 / 导入

- **导出**：菜单「工具 → 配置导入 / 导出 → 配置导出」；类别勾选（连接+分组 / 快捷指令 / 首选项 / JDBC 驱动定义）；**含密码时 AES-256-GCM + PBKDF2-SHA256(310k) 口令加密**（`vault.rs`，信封 format=personaldb-vault），密码经 `secret_get` 取回后随包加密，不落明文。
- **导入**：选文件 → 识别加密信封 → 口令解密 → 按 id **跳过/覆盖**合并（连接经 `upsert` 自动把明文密码转入凭据管理器）；首选项写回 localStorage 后提示重启完全生效。
- **落点**：`ConfigTransferDialog.vue`、`stores/connections.ts#mergeGroups/upsert`。

## SPEC-TRANSFER-2 三方配置导入

- **来源**：DBeaver（data-sources.json）、Navicat（.ncx 导出文件，经典 Blowfish 解密）、PL/SQL Developer（`alias = user/password@db` INI + 注册表 Preferences 扫描）。
- **流程**：选文件扫描 → 归一化草稿列表（名称/类型/主机/用户/密码有无）→ 勾选导入 → 走 `connStore.upsert`（密码自动入凭据管理器）。
- **密码 best-effort**：解不开置空并提示补填，不阻断导入；DBeaver 密码（credentials.json 绑定本地安全存储）不支持离线解密。

## SPEC-SESSION-1 数据库会话管理

- **能力**：`capabilities.sessions`（registry 下发；MySQL/OB/PG/Oracle/OB Oracle 支持，通用 JDBC 关闭）。
- **方言**：`SqlDialect#server_sessions_sql`（固定列序 id,user,host,program,state,secs,extra,sqlText；MySQL processlist / PG pg_stat_activity / Oracle v$session+v$sql）与 `kill_session_sql(id, cancel|kill)`（KILL [QUERY] / pg_cancel|terminate_backend / KILL|DISCONNECT SESSION；**id 严格数字校验防注入**）。
- **UI**：Tab 形式（`type='session'`，禁抽屉）；顶部「会话 / 锁·事务」二视图；自动刷新（关/5/10/30s，**请求未返回或窗口失焦自动跳过/暂停**）、关键字筛选 + **状态 chip（全部/活动/空闲/被阻塞）** + 用户下拉、摘要 chips（总数/活动/阻塞）、**耗时热力着色**（≥60s 橙、≥300s 红）、**SQL 悬浮卡**（hover 弹出 pre + 复制；sqlText 列禁用原生 `show-overflow-tooltip`，避免与悬浮卡双弹窗）、`row-key=id` 差分刷新、行内图标操作（取消/终止）、**阻塞链**（第 9 列 `blocking`：Oracle `blocking_session`、PG `pg_blocking_pids`；被阻塞行红色底、阻塞源头行 ⚡ 标）、**批量终止**、**双击行开详情**（全字段 + 完整 SQL + 复制）、**Oracle 终止模式选择**（IMMEDIATE 确认按钮 / POST_TRANSACTION 取消按钮 / 关闭放弃）、**Kill 后跟踪**（行置灰 + 「已请求终止」标记，会话消失自动清除）、**导出 CSV**（BOM + 引号转义，经 save/save_text_file）、**>300 行自动虚拟化**（复用 ResultVirtualGrid，该模式仅排序/复制，禁用勾选/详情/导出并提示）。
- **锁/事务视图**：方言 `server_locks_sql`（动态列直渲染；Oracle v$lock+v$session TX/TM、PG pg_locks+pg_stat_activity、MySQL 8 data_locks+threads——目标列用 `OBJECT_SCHEMA.OBJECT_NAME` 拼接，持有线程经 `THREAD_ID` 关联；**OceanBase Oracle 走 `V$OB_LOCKS`（LMODE/REQUEST 为字符串 `'RX'/'NONE'` 不做数值 DECODE，无会话 id → 行级终止禁用）**）；**OceanBase MySQL 模式无 data_locks，方言直接声明不支持**；权限缺失/5.7 等查询失败显示错误于空态；动态列客户端排序；>300 行虚拟化；入口 `db_server_locks`。
- **列操作**：会话表支持**表头拖拽排序**（sortablejs）+ **表头右键菜单**（移到最前/最后、还原列序）；列序为组件内状态（不持久化）。
- **落点**：`SessionPane.vue`、`commands.rs#db_server_sessions/db_kill_session/db_server_locks`。

## SPEC-UI-18 Splash 启动窗体

- 主窗 `visible:false`；`public/splash.html` 独立小窗（品牌+进度动画，纯静态）先行显示；前端挂载完成 `invoke('app_ready')` → 后端关 splash、show/focus 主窗。
- **落点**：`tauri.conf.json#windows`、`commands.rs#app_ready`、`App.vue#onMounted`。

## SPEC-UI-19 漫游指导

- driver.js；锚点 `data-tour="<key>"`（tree/query-toolbar/result/new-conn/workspaces/tabs/settings/session-toolbar/history）；`src/tours/index.ts` 定义总览/查询/会话三套（隐藏锚点自动剔除）。
- 入口：帮助菜单子菜单；**首次启动自动播放**（localStorage `personaldb.tourDone`）；文案全部 i18n。

## SPEC-UI-20 命令面板（Ctrl+K）

- `CommandPalette.vue`（Teleport 遮罩 + 搜索 + ↑↓/Enter/Esc）；条目由 App 注入：应用命令（新建/设置/驱动/导入导出/日志/主题/重连全部/漫游/关于/会话）+ 全部连接（点击即连）+ 最近 20 条历史 SQL（回填当前 Tab）。
- 快捷键 `Ctrl+K` 在 `onGlobalKeydown` **前置拦截**（编辑器聚焦时也生效，优先于 shortcut 体系）。

## SPEC-EDITOR-7 查询历史

- `stores/history.ts`：localStorage `personaldb.history`，上限 500 条、单条 SQL 截断 2000 字符；`runTab` 成功/失败均记录（含耗时/连接/库/ok 标记）。
- UI：查询工具栏时钟按钮 → `HistoryPanel.vue` 弹层（搜索/清空/点击回填 `tab.sql`）。

## SPEC-CONN-5 连接保活

- `ConnectionManager::spawn_keepalive`（lib.rs setup 启动）：55s 周期对全部会话 `driver.ping()`；默认实现 `SELECT 1`，Bridge/JDBC 走 sidecar `ping` 命令，Oracle 为 `SELECT 1 FROM DUAL`（spawn_blocking）；失败仅 WARN（不主动断开会话，前端仍按执行失败标记丢失）。

## SPEC-UI-21 设计令牌与视觉统一

- **令牌**：`styles/tokens.css`（`--pd-space-1..4` / `--pd-radius[-lg]` / `--pd-titlebar-h` / `--pd-menu-h` / `--pd-header-h`）；新样式优先取令牌。
- **连接色点**：`utils/color.ts#connColor`（id 哈希→HSL 均布色相）；两个页签面（顶部 el-tabs 标签 / `TabBarActions` 列表）在图标前渲染 `.conn-dot`。
- **空状态**：`EmptyState.vue`（图标=APP_ICONS、title/hint、action 插槽）；查询结果「无结果」与「占位」两态已接入（占位态含引导 hint）。

## SPEC-DEV-1 CI 与 i18n 校验

- `.github/workflows/ci.yml`（windows-latest）：`npm ci` → `check:i18n` → `npm run build` → `cargo check`（默认 + `--no-default-features`）→ `cargo test --lib`；Go/Java 21 工具链就绪使 sidecar 可构建。
- `npm run check:i18n`：`scripts/check-i18n.mts`（node `--experimental-strip-types`）递归对比 zh-CN/en 叶子 key，缺失即退出码 1。

## SPEC-EDITOR-8 大结果虚拟滚动

- 结果集 **>600 行且非编辑态** 自动切换 `ResultVirtualGrid.vue`（el-table-v2）：行虚拟 + 列头点击排序 + 右键复制单元格；列宽按语义类型分配。
- 编辑模式 / ≤600 行保持完整 el-table 路径（选择/筛选/固定列/内联编辑等全功能）。

## SPEC-RESULT-9 多结果集

- 非 Oracle 类连接按语句分割（`utils/sqlSplit.ts`，字符串/注释感知；**Oracle PL/SQL 块含分号，跳过分割**）；`runMultiStatements` 顺序执行，任一失败即停（部分成功仍展示已得结果并以消息提示失败）。
- UI：`result-meta` 顶部 `el-segmented`（结果 1..N）切换，`tab.result` 始终指向当前项；复制/导出/编辑作用于当前结果；多语句模式禁用续拉。

## SPEC-RESULT-10 网格编辑撤销 / 重做

- 编辑模式全量快照栈（上限 50）：覆盖 单元格提交/新增/复制/标记删除/行级还原 五类变更；`Ctrl+Z` / `Ctrl+Y`（或 Ctrl+Shift+Z）+ 工具栏按钮；进入编辑或退出时清栈。逻辑抽为 `composables/useEditUndo.ts`（栈与快捷键判定，`EditRow` 类型随迁）。

## SPEC-EDITOR-9 查询历史（SQLite 持久化）

- 后端 `history.rs`：`{存储目录}/history/history.db`（WAL，上限 500 条，按 at 修剪）；命令 `history_add/history_list(keyword,limit)/history_clear`；文件库失败自动回退**内存库**（启动恒可管理 State）。
- 前端 `stores/history.ts` 改为后端存储：首次使用把旧 localStorage `personaldb.history` **一次性迁入**后删除；`push` 乐观插入 + 后台落库；面板搜索走 SQL LIKE；**「仅当前连接」过滤**；**单击回填、双击回填并立即执行**。
- 存储类别 `History`（`storage.catHistory`，可在「存储」设置迁移）。

## SPEC-DEV-2 更新提示（查 GitHub Releases，无签名 / 无自动检测）

- **不做自动检测**：应用内不发起启动/定时检查（历史实现曾用 `tauri-plugin-updater` 在打包版启动 5s 后静默检测，该链路已整体移除），只在用户显式触发时查询。
- **检测入口**：帮助菜单「检查更新」与首选项「关于 → 更新提示 → 检查更新」→ `stores/update.ts#runManualCheck()`。
  - 请求 `https://api.github.com/repos/{GITHUB_REPO}/releases/latest`（`Accept: application/vnd.github+json`）；`GITHUB_REPO` 常量在 `stores/update.ts`（默认 `smoothwind/PersonalDb`）。该接口只返回**最近一次已发布**的 Release——草稿与预发布版本不计入。
  - 当前版本取 `getVersion()`（`@tauri-apps/api/app`，即 `tauri.conf.json` 的 version）；比较逻辑在 `utils/version.ts#compareVersions`（忽略 `v` 前缀、缺失段按 0、预发布低于同号正式版），单测 `tests/update-version.spec.ts`。
  - 命中更高版本 → `outcome='update'`，记录 `version`（已去 `v` 前缀）与 `releaseUrl`（Release 的 `html_url`，缺失时回退发布列表页）。
  - 404（仓库私有 / 尚无 Release）→ `update.noRelease`；其余非 2xx → `HTTP {status} {statusText}`；网络异常取原文；一律 WARN 落日志，`outcome='error'`。
- **非模态提示**：状态栏常驻「有新版本 v{version}」（`hasUpdate` = 有新版且未被跳过；点击 `openRelease()` → `opener.openUrl(releaseLink)` 用浏览器打开发布页）+ 帮助菜单「检查更新」红点（`.menu-dot`）；可「不再提示此版本」（状态栏 × → `preferences.skipVersion` → `personaldb.update.skippedVersion`）。另给一次性反馈：已最新 info / 失败 error（带摘要）/ 有新版本 success。
- **无应用内下载安装**：不需要签名密钥、`.sig`、`latest.json`；`tauri-plugin-updater`、`tauri-plugin-process`（含 `updater:default`、`process:allow-restart` 权限）、`plugins.updater` 配置与 `tauri.updater.conf.json` 均已删除。若要恢复「应用内安装」，需重新引入插件 + 签名密钥 + latest.json 链路。
- **发布链路（GitHub）**：版本号在所有位置同步递增（`tauri.conf.json` / `package.json` / `package-lock.json`（2 处）/ `Cargo.toml` / `Cargo.lock`，以及界面硬编码的 `App.vue` 状态栏、`AboutDialog.vue`、`SettingsDialog.vue`）→ 打 `v*` 标签推送 → CI `bundle` 作业经 `tauri-apps/tauri-action@v1` 构建 MSI + NSIS 并建**草稿** Release（不需要任何 Secret）；确认产物后手动发布，旧版用户下次手动检查即可看到提示。标签含 `-`（beta / rc，如 `v0.1.0-beta.1`）时 `prerelease: contains(github.ref_name, '-')` 会标记为 pre-release，`/releases/latest` 不计入，故预发布不会推送给正式用户。
- 前提：仓库可被匿名读取 `api.github.com`（公开仓库）；换仓库需改 `GITHUB_REPO` 常量并同步 README。

## SPEC-SESSION-3 OceanBase Oracle 会话/锁（OB 兼容视图）

- 实测（OB 4.2.1.6）：`V$SESSION` / `GV$OB_SESSIONS` / `V$OB_SESSIONS` / `SYS.GV$OB_SESSIONS` 均不存在（ORA-00942）；可用视图为 **`V$OB_PROCESSLIST`**（会话：ID/USER/USER_HOST/MODULE/COMMAND/TIME/STATE/DB/INFO）与 **`V$OB_LOCKS`**（锁：TYPE/ID1/ID2/LMODE/REQUEST/CTIME/BLOCK，LMODE/REQUEST 为字符串如 `RX`/`NONE`）。
- 方言 `OracleDialect { ob: bool }`（`dialect::build` 按 DbKind 选择；OCI 驱动与桥接驱动均经同一构造）：OB 分支会话 SQL 用 `TO_CHAR(ID)`（`CAST(ID AS CHAR)` 触发 OB ORA-00600）、不引用 v$session；锁 SQL 直取 LMODE/REQUEST（不做数值 DECODE，否则 ORA-01722）；**终止 = `ALTER SYSTEM KILL SESSION '<ID>'`（已实测自终止成功；无 cancel 语义 → cancel 返回 None）**；`server_sessions_fallback_sql` 对 OB 返回 None。标准 Oracle 分支保持 v$session/v$lock + `sid,serial#`。
- 桥接 `ping` 改为真实探活（Oracle 桥 / OB JDBC 桥统一 `SELECT 1 FROM DUAL`；sidecar 的 `ping` 命令仅协议回应，无法发现服务端断连）；通用 JDBC 桥先 `SELECT 1` 再回退 DUAL。
- 锁视图行级终止在无 `id` 列时禁用（OB 锁视图无会话 id）。

## SPEC-SESSION-2 会话查询降级

- 方言 `server_sessions_fallback_sql`（Oracle 实现 = 仅 v$session，sqlText 置空）；`db_server_sessions` 主查询失败且存在降级 SQL 时 **WARN + 自动重试**（OB 租户无 v$sql 权限场景）。

## SPEC-RESULT-11 结果集组件增强（性能 / 功能 / UI）

- **性能**：全局查找 **200ms 防抖**（`searchApplied`）；`.result-meta` 可换行防溢出。（列虚拟化/矩阵化/Worker 过滤为远期，见 plan-roadmap）
- **值筛选（Excel 式）**：列设置弹层新增「值筛选」——distinct 值复选（cap 200，结果变更失效缓存，NULL 记为 `NULL`），多列叠加过滤；`activeFilterCount` 一并计入。
- **统计行**：数值列 count/Σ/x̄（min/max 进 tooltip），对过滤后行计算，工具栏开关（Σ 图标）。
- **列宽持久化**：`header-dragend` 记忆列宽，localStorage `personaldb.colWidths`，键 = `连接:库:表/SQL哈希`，防抖 300ms 落盘。
- **粘贴建行**：编辑模式监听 `paste`——TSV 多行 → 批量新增（按列序映射，数值/布尔语义转换），一次性入撤销栈。
- **单元格**：NULL 灰斜体（既有）；**http(s) 值渲染为链接**（`opener.openUrl`，失败回退 window.open）；**长文本/JSON >80 字符悬浮卡**（虚拟锚点 popover + 复制）；**紧凑模式行高固定不换行**（nowrap+ellipsis，长文本走 `show-overflow-tooltip` / 悬浮卡；记录视图保持换行）。
- **导出范围/格式**：选中行或过滤后全量；Excel / SQL INSERT / CSV / TXT / **JSON**（对象数组，数值/布尔保持类型，二进制 base64）。
- **快捷键**：`Ctrl+F` 聚焦结果查找框（编辑器聚焦时除外）；`Ctrl+A` 全选结果行（非编辑态）。
- **列操作补齐**：列设置弹层新增**移到最前/最后**；显示列弹层新增**还原默认**（清空列序/列宽/显隐，含持久化清理）与**列表项拖拽排序**（搜索时禁用）；表头拖拽既有。
- **虚拟模式补齐**：双击整行复制（TSV）。
- **UI**：错误条带「重试」（复用 `runTab` baseSql）与关闭；**编辑态常驻提示条**（表名 + 定位策略：主键/ROWID 或 全列匹配-高危红条 + 退出按钮）；记录视图新增**字段搜索**。
- **落点**：`QueryTabPane.vue`、`ResultVirtualGrid.vue`、`CellDisplay.vue`、`RecordView.vue`。

## SPEC-UI-22 页签连接信息与状态标识

- **两个页签面统一**（顶部 el-tabs 标签 / `TabBarActions` 列表）：
  - **连接信息**：列表项为双行（标题 + `连接名@库` 副行），tooltip 显示 `标题 · 连接名@库 · 状态`；
  - **状态三态**：已连接（默认）/ 已断开（`lostIds`，标题与连接名红色、色点红圈）/ 未连接（无会话，整体 55% 透明度、色点 35%）。
- 状态判定 `connStore.isConnected / lostIds`，文案 `conn.stateConnected/Lost/Off`。

## SPEC-RESULT-12 统一数据管线（矩阵 + 索引 + Worker）

- **管线**：`filteredIdx: number[]` 为过滤唯一事实源；过滤核心 `workers/filterCore.ts` 纯函数（全局查找 + 列包含 + 值集合精确，NULL→`NULL` 文本化），主线程同步与 Worker **共用同一实现**（语义一致）。≥3000 行走 Worker（token latest-wins；Worker 失败自动回退同步）；编辑态强制同步。
- **消费**：`resultRows` = 按需物化对象视图（el-table 渲染/复制/导出/统计触达才构建；**虚拟路径模板不触达 → 零对象分配**）；虚拟网格行 = `filteredIdx.map(i => matrix[i])` **零拷贝引用切片**，列携带 `srcIndex` 直读源矩阵（排序/取值/NULL 灰斜体一致）。
- **明确不做**：导出 Worker（后端为瓶颈）；列虚拟化（el-table-v2 不支持）；双渲染器完全统一（数据管线已统一，特性差异见 SPEC-RESULT-11）。
- **落点**：管线整体抽为 `composables/useResultPipeline.ts`（QueryTabPane 仅消费 `matrix/propIndex/filteredIdx/valueFilters/distinctCache/resultRows`；Worker 生命周期随 composable 卸载自清理）。
- **反应式不变量**：分页续拉是**原地 `result.rows.push`**（不换 `result` 对象），故过滤/去重 watch 必须显式跟踪 `result.rows.length`，否则续拉行不参与过滤与渲染；Worker 收到 `postMessage` 前必须 `toRaw`（reactive proxy 无法结构化克隆，抛 `DataCloneError`，须 catch 后同步回退）。

## 备注

- 编辑器依赖：CodeMirror 6（`codemirror`、`@codemirror/lang-sql`、`@codemirror/theme-one-dark`）。
- 结果格式化与「紧凑布局」均为纯前端；DDL 相关依赖后端方言层（`db/dialect`）。

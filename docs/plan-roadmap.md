# PersonalDb 任务规划（Roadmap）

> 状态标记：`[ ]` 未开始 / `[~]` 进行中 / `[x]` 完成。
> 依赖关系：**④（密码安全）是 ②/③ 的地基**；**①（图标）是 ⑦（漫游锚点）的前提**；⑧ 为独立快赢项。

## 阶段总览

| 阶段 | 任务 | 主题 | 预估 | 依赖 | 状态 |
| --- | --- | --- | --- | --- | --- |
| P1 基础 | ④ | 密码安全存储（Windows 凭据管理器） | 1~2 天 | 无 | `[x]` |
| P1 基础 | ⑧ | Splash 启动窗体 | 0.5 天 | 无 | `[x]` |
| P1 基础 | ① | 全系统 SVG 图标体系 | 2~3 天 | 无 | `[x]` |
| P2 数据互通 | ③ | 自身配置导出/导入 | 1.5 天 | ④ | `[x]` |
| P2 数据互通 | ② | 三方配置导入（DBeaver/Navicat/PLSQL Dev） | 3~4 天 | ④ | `[x]` |
| P3 体验 | ⑥ | 数据库会话管理 | 2~3 天 | 无 | `[x]` |
| P3 体验 | ⑦ | 功能漫游指导 | 2~3 天 | ① | `[x]` |
| 持续 | ⑨ | 可改进方向归纳 | 0.5 天 + 持续 | — | `[~]` |

**执行结果（全部落地）**：

- **④**：`src-tauri/src/secrets.rs`（keyring + Windows 凭据管理器）；`ConnectionConfig.passwordRef` 引用模型；启动自动迁移明文；连接/测试连接自动解析与回写；连接复制凭据随复制；删除连接清理凭据；凭据不可用回退明文 + WARN。
- **⑧**：`public/splash.html`（品牌 + 进度条，纯静态零依赖）+ `app_ready` 命令（关 splash / show 主窗）；主窗 `visible:false`。
- **①**：`src/utils/icons.ts`（40+ 自绘 24×24 线性 SVG，obj-*/act-*/st-* 三类）+ `AppIcon.vue`；`objectIcons.ts` 改为图标 key 映射（树/页签/标题全部切换）；动作按钮沿用 Element Plus SVG 保持一致。
- **③**：`vault.rs`（AES-256-GCM + PBKDF2-SHA256 310k 次）；「工具 → 配置导入 / 导出」三 Tab 对话框：导出（连接/分组/指令/首选项/驱动 + 口令加密）、导入（跳过/覆盖合并 + 首选项 localStorage 回写）。
- **②**：`src-tauri/src/import/`（DBeaver data-sources.json、Navicat .ncx + 经典 Blowfish 解密 + 自实现 SHA1、PL/SQL Dev INI + 注册表扫描）；密码 best-effort，失败置空提示。
- **⑥**：能力 `capabilities.sessions`；方言 `server_sessions_sql` / `kill_session_sql`（MySQL processlist+KILL、PG pg_stat_activity+pg_terminate/cancel_backend、Oracle v$session+KILL/DISCONNECT SESSION，均防注入校验）；`SessionPane.vue` Tab（自动刷新/筛选/取消/终止二次确认）；连接右键 + 主工具栏入口。
- **⑦**：driver.js + `src/tours/index.ts`（总览/查询/会话三套引导，`data-tour` 锚点，i18n 全覆盖）；帮助菜单入口 + 首次启动自动引导。

### 按价值排序的体验改进（第二批，已落地）

1. **代码分割**：Vite 8（rolldown）`advancedChunks` 分包 + 重型面板 `defineAsyncComponent`；主包 **1787KB → 97.5KB**（gzip 27KB），CodeMirror/Element Plus/工具库独立分包按需加载。
2. **连接保活**：55s 周期 ping 全部会话（驱动各自最轻实现），根治 MySQL `wait_timeout` 静默断连。
3. **查询历史**：本地 500 条（localStorage），查询工具栏时钟面板，点击回填 SQL。
4. **Ctrl+K 命令面板**：应用命令 / 连接 / 最近 SQL 统一搜索入口。
5. **会话查询降级**：OB 无 `v$sql` 权限自动回退仅 `v$session`。
6. **Splash 超时兜底**：8s 未收到 `app_ready` 强制显示主窗 + WARN。

### 仍未做（后续候选）

- 结果集虚拟滚动（el-table-v2 迁移，改动大）
- 查询历史 → SQLite 持久化（跨设备）；多结果集；网格撤销重做
- tauri-updater 应用内自动更新 + 代码签名（现为「手动查询 GitHub Releases 提示」，见 SPEC-DEV-2）；NSIS 中文安装器

### 第三批（已落地）

1. **UI 设计令牌**：`styles/tokens.css`（`--pd-*` 间距/圆角/高度变量）。
2. **连接色点页签**：`connColor(id)` 哈希取色，三个页签面统一渲染 `.conn-dot`。
3. **空状态组件**：`EmptyState.vue` 统一（结果占位/无结果已接入，占位态带引导文案）。
4. **CI**：`.github/workflows/ci.yml`（i18n 校验 → 前端构建 → 双配置 cargo check → 测试）。
5. **i18n 校验脚本**：`npm run check:i18n`，当前 **708 键 zh/en 完全同步**。

### 第四批（已落地，全部候选清零）

1. **结果集虚拟滚动**：>600 行自动切换 `ResultVirtualGrid.vue`（el-table-v2，行虚拟/列排序/右键复制），≤600 行或编辑态保持全功能 el-table。
2. **历史 SQLite 化**：`history.rs`（WAL，500 条上限，类别 History）；旧 localStorage 一次性迁移；面板搜索走 SQL；文件库失败回退内存库。
3. **多结果集**：语句分割（字符串/注释感知，Oracle 跳过）逐条执行；segmented「结果 N」切换；部分成功保留已得结果。
4. **网格撤销/重做**：快照栈 50，覆盖五类编辑变更，Ctrl+Z/Y + 工具栏按钮。
5. **更新提示**：`stores/update.ts` 手动「检查更新」→ 查 GitHub `releases/latest`（`utils/version.ts#compareVersions` 与当前版本比较），命中新版本只在**状态栏/帮助菜单**给非模态提示（可「不再提示此版本」）；**无自动检测、不下载安装、不需要签名密钥**；发布链路（tauri-action 草稿 Release）见 SPEC-DEV-2。

### 第五批：会话管理深度增强（已落地）

1. **阻塞链分析**：第 9 列 blocking（Oracle blocking_session / PG pg_blocking_pids），被阻塞红底 + 阻塞源 ⚡ 标 + 阻塞计数 chip。
2. **锁/事务视图**：`server_locks_sql`（Oracle v$lock / PG pg_locks / MySQL 8 data_locks，动态列直渲染）。
3. **会话详情面板**：双击行 → 全字段 descriptions + 完整 SQL + 复制。
4. **Oracle 终止模式选择**：IMMEDIATE（确认）/ POST_TRANSACTION（取消）/ 关闭放弃。
5. **Kill 后跟踪**：行置灰 + 「已请求终止」标记，刷新后会话消失自动清除。
6. **导出 CSV**：BOM + 引号转义，走系统保存框。
7. **>300 行虚拟化**：复用 ResultVirtualGrid（该模式禁勾选/详情/导出并提示）。
8. 体验补齐：刷新节流 + 失焦暂停、状态 chip 快捷筛选、SQL 悬浮卡、耗时热力、row-key 差分、操作列图标化、EmptyState 空态。

### 第六批：结果集组件增强（已落地）

性能：查找防抖 200ms、meta 工具栏换行；功能：**列值筛选（Excel 式）**、**数值列统计行**、**列宽持久化**、**剪贴板 TSV 粘贴建行**、URL 链接化、长文/JSON 悬浮卡、Ctrl+F 聚焦查找、虚拟模式双击复制整行、记录视图字段搜索；UI：错误条带重试、**编辑态常驻策略提示条（高危红色）**。
仍未做（工程量大，远期）：列虚拟化、行数据矩阵化、Worker 过滤/导出、双路径完全统一。

### 第七批（已落地）

1. **列操作体验**：显示列弹层支持**拖拽排序**（与隐藏勾选一并确认提交，搜索时禁用拖拽）；结果集列设置弹层「移到最前/最后」；「还原默认」清空列序/列宽/显隐；会话表拖拽 + 表头右键移动/还原 + **列序按连接类型持久化**；列筛选输入改为**回车/失焦提交**（根治逐键重渲染）。
2. **显示列弹层性能**：状态隔离到 ColVisPopover 子组件 + 原生复选框（勾选过程零整表重渲染，宽表不再巨卡）；值筛选同（ColValueFilter）。
3. **页签连接标识**：竖排页签双行化（标题+连接名@库）；三个页签面统一连接状态三态（已连接/已断开红/未连接灰）。
4. **修复**：树展开隐式重连（展开不再作为连接触发器，双击/右键「连接」显式建立）；MySQL 锁查询列名（OBJECT_SCHEMA.OBJECT_NAME + THREAD_ID 关联）；OB MySQL 锁视图声明不支持。
5. **导出 JSON**（对象数组、类型保真）；历史「仅当前连接」过滤 + **双击回填并执行**；错误条按错误类型智能切换 重连/重试；锁视图排序/空态/虚拟化。

### 第八批（已落地）

1. **安全**：高危（全列匹配）DML 预览需**输入表名**方可执行；清空历史二次确认。
2. **效率**：结果表 **Ctrl+A 全选**；锁视图 CSV 导出（与会话共用入口）；连接丢失通知**可点击直接重连**（ElNotification，`reconnectOne`）。

### 第九批（已落地）

1. **结果集行高固定**：紧凑模式下单元格不换行（nowrap+ellipsis），长文本经 `show-overflow-tooltip` / 悬浮卡查看（记录视图仍可换行）。
2. **SqlEditor**：Ctrl+F 搜索面板（CM6 searchKeymap 原生支持确认）；**Ctrl+滚轮字号缩放**（10–24px，写回首选项、fontComp 热更新、全局生效）。
3. **表查看面板（列 Tab）全量列对齐**：表头拖拽、移到最前/最后（表头右键）、显示列（复用 ColVisPopover）、列宽持久化、还原默认——按 `连接:库:表` 键持久化于 `personaldb.inspectorCols:*`。
4. **README 功能清单同步**至第九批。

### 第十批（已落地）

1. **树搜索增强**：搜索时自动展开**已加载**的父链（懒加载未加载节点保持折叠，不触发隐式连接）。
2. **导出体验**：导出成功通知**点击打开文件所在目录**（`revealItemInDir`；结果集各格式 + 会话/锁 CSV）。
3. **历史面板**：条目**右键仅复制 SQL**（不回填、不关闭）。
4. **启动性能观测**：`main.ts` 首帧耗时日志（`[perf] first frame in Xms`）。

### 第十一批（已落地）

1. **表查看面板数据预览**：结构 Tab 工具栏「查询数据」→ 直开预览页签（前 100 行 SQL，不自动执行，与树双击行为一致）。
2. **驱动管理 jar 拖拽导入**：拖入 `.jar` 即追加（Tauri drag-drop 事件取真实路径，去重 + 成功提示）。
3. **多结果集标签带行数**：segmented 显示 `结果 N (行数)`。

### 第十二批（已落地）

1. **列宽双击自适应**：结果集 + 会话表表头双击（400ms 两次同列合成），canvas 按表头+前 200 行测宽（60–600 clamp），写回持久化（结果集 `personaldb.colWidths`、会话 `sessionCols:{kind}.widths`）。
2. **会话列宽持久化**：`sessionCols` 文件升级为 `{order,widths}`（兼容旧数组格式）；还原默认一并清宽。
3. **会话连接信息头**：工具栏 chip `user@host:port · 版本`（悬停全量）。

### 第十三批（已落地）

1. **锁视图行级终止**：锁行「终止」按钮（id 与会话同语义：sid,serial / pid / processlist id），确认后 kill 并刷新。
2. **仅看当前用户**：会话工具栏开关（按连接用户名过滤），与状态 chip/用户下拉/关键字叠加。
3. **README 项目结构树更新**：前端（stores/utils/tours/components）与后端（dialect/jdbc/dml/export/secrets/vault/import/history）全量对齐实际模块。

### 第十四批（已落地，含分批提交 160581a/4663c64/eb80b17/9317fb3）

1. **多语句失败定位**：提示「第 i/total 条执行失败」，tab.error 与日志均带序号。
2. **Ctrl+S 保存 SQL 脚本**（单元格输入中不拦截，避免误触）。

### 第十六·五批（已落地，修复 OB Oracle 会话/锁）

1. **OB Oracle 会话/锁改用 OB 兼容视图**：实测 `V$SESSION`/`GV$OB_SESSIONS` 不存在 → 会话走 `V$OB_PROCESSLIST`（`TO_CHAR(ID)`；`CAST(ID AS CHAR)` 触发 ORA-00600）、锁走 `V$OB_LOCKS`（LMODE/REQUEST 为字符串，去 DECODE 避免 ORA-01722）；查看 `OracleDialect{ob:bool}` 分支，OCI 与桥接驱动统一。终止 = `ALTER SYSTEM KILL SESSION '<ID>'`（实测自终止成功；cancel→None）；锁行无 id 时禁用终止。
2. **桥接 ping 真实探活**：Oracle 桥/OB JDBC 桥 `SELECT 1 FROM DUAL`，通用 JDBC 先 `SELECT 1` 再回退 DUAL（原 sidecar ping 命令仅协议回应，保活形同虚设）。
3. **clippy 清零**（23 处 lint）；**Element Plus 按需加载评估后回退**（强类型暴露数十处模板签名，收益/风险不匹配，结论记录）。

### 第十九批（已落地）

1. **CSV 公式注入防护（OWASP）**：文本值以 `= + @ \t \r` 或「-非数字」开头时前缀 `'`；数值类型放行保证可计算；单测覆盖（**25 通过**）；SQL 导出文件头注释此前已有。

### 第二十批（已落地）

1. **Excel 公式注入核查**：文本经 `write_string` 写入（rust_xlsxwriter `write()` 的公式自动探测不涉及），安全。
2. **续拉兜底与上限提示**：滚动提示旁新增「立即加载」按钮；达到 `maxTotalRows` 上限（truncated 且不再续拉）时显示上限说明文案。

### 第二十一批（已落地）

1. **多语句超时保护**：`runMultiStatements` 每条语句套用与单语句一致的 60s 超时（此前挂起语句会阻塞整批且无兜底）。
2. **深色主题硬编码色审查**：全量扫描组件/样式——品牌色（DbLogo/WelcomePane）、HexViewer 棋盘格、阴影 rgba 均为双主题合理设计；EP 变量已全面接管，无需修改。
3. 编辑器空态占位符确认已存在（`cmPlaceholder`）。

### 第二十二批（已落地：管线重构核心）

1. **统一数据管线（矩阵 + 索引 + Worker）**：`filteredIdx` 成为过滤唯一事实源——`filterCore.ts` 纯函数供主线程同步与 Worker（≥3000 行，token latest-wins、失败回退同步）共用；`resultRows` 降级为按需物化对象视图（虚拟路径模板不触达 → **零对象分配**）；vgrid 列携带 `srcIndex` 直读源矩阵（**零拷贝行** + 排序/取值一致）+ NULL 灰斜体渲染；distinctOf/上限判定全部走矩阵/索引。
2. **Worker 过滤落地**；**导出 Worker 评估为低价值**（后端导出为瓶颈，前端映射开销小）——不做。
3. **行数据矩阵化**：以「大数据路径全矩阵、小路径对象按需」形态落地（避免 2500 行热文件全量改写）。
4. **列虚拟化**：el-table-v2 不支持（列冻结为独立窗格、其余全渲染）——需自研 canvas 网格，保持远期；双路径完全统一同步远期（数据管线已统一，渲染器特性差异见 SPEC-RESULT-11）。

### 第二十三批（已落地）

1. **文件夹拖拽排序**：树文件夹节点可拖拽调整根级顺序（扁平模型，仅组间 prev/next；不允许嵌套或落到连接节点），`connections.moveGroupOrder` 重排并持久化 `groups.json`，落位后重建树保留展开状态 + 提示「文件夹顺序已更新」。

### 第二十四批（已落地：多级文件夹）

1. **多级文件夹**：`ConnectionGroup.parentId`（Rust 结构同步，null 省略存储，向后兼容旧 groups.json）；树按 `groupChildren(parentId)` 递归加载（组子级 = 子文件夹 + 组内连接）。
2. **拖拽全层级**：连接可拖入任意层级文件夹（文件夹任意位置=移入、连接同级=保持同组、根级空白=移出）；文件夹可拖拽嵌套（inner）/同级排序（prev/next）/落到连接=进入其所属文件夹；`moveGroup` 含**循环嵌套防护**（拒自身/后代，违规提示）。
3. **删除策略选择**：右键删除文件夹弹出策略对话框（含影响统计：文件夹数/连接数）——仅删该节点内容上移（promote）/ 删本级与子文件夹、连接上移（flatten）/ **连同其中连接与凭据一并删除**（purge，二次确认）。
4. 配套：右键「新建子文件夹」；「移动到文件夹」下拉改为**深度优先缩进**展示全部层级。

### 第二十五批（修复：拖拽不生效）

1. **根因**：`allow-drop` 在拖拽悬浮阶段对懒加载文件夹执行 `expand()`（+`pendingExpand`）——节点被重建导致 el-tree **拖拽中断、drop 不触发**；且用户现网 `tauri:dev` 早于改动启动（HMR 未覆盖 store/worker 新模块），表现为「完全不能拖」。
2. **修复**：`allowDrop` 改为**纯谓词**（无副作用，删除 pendingExpand 机制）；`onNodeDrop` 统一 `refreshPreservingExpanded(extraKeys)`——store 为唯一事实源，重建后恢复展开并**附加展开目标文件夹**以便看到移动结果。AGENTS 已固化「allow-drop 必须为纯谓词」。

### 第二十六批（修复：「禁止」光标）

1. **现象**：拖拽连接时所有落点显示「禁止」光标 → el-tree 在 `allow-drop` 返回 false 时即显示 no-drop。
2. **修复**：`allow-drop` **一律放行（恒真）**，归属/层级校验全部后移到 `node-drop`；非法落点（库/对象/表或无法识别的节点）在 drop 中忽略并恢复树；「根级空白」严格以 `drop.level === 0` 判定，防御 data 缺失被误判为移出分组。AGENTS 同步。

### 第二十七批（修复根因：Windows WebView2 原生拖放吞掉 HTML5 拖拽）

1. **证据链**：EP 源码 `tree-node` 的 `handleDragOver` 会调用 `allow-drop`；实测日志仅有 `allowDrag`/`dragstart`，**`allowDrop` 零调用** → `dragover/drop` 未投递到页面 → 无 `preventDefault` → 浏览器显示「禁止」光标。
2. **根因**：Tauri 默认注册原生拖放（`dragDropEnabled: true`），在 Windows/WebView2 上**拦截并吞掉 HTML5 的 dragover/drop**（dragstart 仍触发，故迷惑性强）。
3. **修复**：主窗 `dragDropEnabled: false` → HTML5 拖拽恢复；驱动管理 jar 拖入改走 **HTML5（`text/uri-list` 解析本地路径）**，保留功能（无路径时提示改用「添加文件」）；移除临时诊断日志，保留 `allowDrag/allowDrop` 恒真 + `node-drop` 校验设计。AGENTS 增加「Windows 拖拽约定」。

### 第二十八批（已落地：工作区指定磁盘存储位置）

1. `Workspace.dir`（创建时**必须**经目录选择对话框指定；取消则不创建）：工作区数据目录不再隐含固定于默认 Workspaces 类别；空值=默认类别目录（兼容旧数据）。
2. 菜单「新建工作区」流程：输入名称 → **选择存储目录**（`openDialog({directory:true})`，含名称占位提示）→ 创建并切换。

### 第二十九批（已落地：更改工作区存储位置 + 导出默认取工作区目录）

1. 工作区下拉菜单新增「**更改存储位置**」：目录选择 → 后端 `workspace_set_dir`（迁移旧目录文件到新目录、目标已存在不覆盖）→ 更新索引 `dir`。
2. 「导出保存位置」默认值改为 `exportDir ?? dir`（工作区存储目录）——`defaultExportPath`/`saveSqlFile` 统一回退链。

### 第十五批（已落地）

1. **连接表单端口校验**（1–65535 整数，blur 触发）。
2. **结果集列头悬停**显示完整列名 + 类型。
3. **导出列全选**确认已存在（setAllExportCols）；新增 Rust 测试 `cell_to_json_types` / `csv_line_escapes`（**20 通过**）。

### 第十六批（已落地）

1. **i18n 占位符一致性检查**：`check:i18n` 扩展——逐 key 对比 zh/en 的 `{name}` 插值集合，不一致即 CI 拦截（当前零差异）。
2. **kill SQL 防注入回归测试**：三方言注入用例（分号/负数/非数字/缺段）落为单测；**测试抓出并修复 PG `i64` 放行负 pid 漏洞（改 u64）**——**23 通过**。

### 落地过程中的新改进点（并入 ⑨）

- 动作按钮仍用 Element Plus 图标（`:icon` 组件协议所限）：如需全自绘，须为 el-button 封装 AppIcon 按钮组件。
- DBeaver 密码绑定本地安全存储（credentials.json + master key），离线解密不可行；后续可研究 dbeaver 加密密钥导出路径。
- OB Oracle 租户对 `v$sql` 权限差异可能导致会话 SQL 列为空/报错：可增加「无 v$sql 回退查询」（仅 v$session）。
- splash → 主窗时序依赖前端 `app_ready`；后续可加超时兜底（>8s 强制 show 主窗 + WARN）。
- 导入覆盖模式不迁移旧凭据引用（旧 id 凭据残留）：可在覆盖时同步 `secret_delete(旧id)`。

**建议执行顺序**：④ → ⑧ → ① → ③ → ② → ⑥ → ⑦（⑨ 随各任务收尾更新）。

---

## ① 全系统 SVG 图标体系 `[ ]`

**目标**：统一全系统图标，替代散落在组件内的图标判断与 Element Plus 混用。

- **方案**：单色线性图标（`currentColor`，自动适配明暗主题）+ 少量多色品牌 Logo（数据库厂商）；沿用 CM6 补全已验证的 CSS mask 单色方案；统一组件 `<AppIcon name="..."/>` + `src/assets/icons/`。
- **步骤**：
  1. 盘点现有图标点：连接树 / 页签 / 工具栏 / 菜单 / 状态栏 / `utils/objectIcons.ts` / `DbLogo.vue` / CM6 补全图标。
  2. 制定命名规范：`obj-*`（对象）、`act-*`（动作）、`db-*`（品牌）、`st-*`（状态）。
  3. 图标来源：基于 Tabler / Lucide 裁剪 + 自绘补齐（禁与现有视觉冲突）。
  4. 替换与清理：组件内禁止内联图标判断，一律走 `objectIcons.ts` 映射（既有规约）。
  5. 明暗主题 + 高分屏验证。
- **验收**：全应用无散落 SVG/图标分支；`objectIcons.ts` 为唯一对象图标来源。

## ② 导入 DBeaver / Navicat / PL-SQL Developer 配置 `[ ]`

**目标**：向导式导入三方工具的连接配置。

- **前置研究（实现前先做原型验证，不闭门造车）**：
  - **DBeaver**：`%APPDATA%\DBeaverData\workspace6\General\.dbeaver\data-sources.json`（驱动/主机/库）；密码在 `credentials.json`（本地 AES，需 dbeaver 加密密钥方案）。
  - **Navicat**：注册表 / `%APPDATA%\PremiumSoft\`（11 与 12+ 存储结构不同）；密码为公开逆向的 **Navicat Crypto（Blowfish 变体）** 算法，采用开源实现移植。
  - **PL/SQL Developer**：注册表 `HKCU\Software\Allround Automations\PL/SQL Developer\Preferences`（及 INI 偏好文件）；历史加密方案有公开实现。
- **实现**：
  - `src-tauri/src/import/`：每来源一个解析器 → 归一化 `ConnectionConfig` 草稿（DbKind 映射：oracle/mysql/postgresql/未知→JDBC 草稿）。
  - UI 向导：选择来源 → 自动扫描（含多版本路径探测）→ 列表勾选预览 → 写入 connections/groups。
  - 密码解密失败：置空 + 明确提示手填（不阻断导入）。
- **测试**：准备三工具真实样例（多版本）做解析单元测试（脱敏 fixture）。
- **风险**：版本差异大；Navicat 12+ 可选口令加密（用户提供口令才可解）。

## ③ 自身配置导出 `[ ]`

**目标**：PersonalDb 配置的导出/导入（备份与迁移）。

- **范围**：连接 + 分组 + 快捷指令 + 首选项 + JDBC 驱动定义（不含驱动 jar 本体）。
- **格式**：带 `schemaVersion` 的 JSON 包；选项：全量 / 勾选类别、**是否含密码**。
- **密码策略**：含密码时必须 **AES-GCM + 用户口令**（PBKDF2/Argon2 派生密钥），不落明文；不含密码则该字段置空。
- **导入**：按 id/名称冲突合并（跳过 / 覆盖 / 另存），复用 ② 的向导骨架。
- **入口**：菜单「工具 → 导出配置 / 导入配置」。

## ④ 密码安全存储 `[ ]`（业界标准方案）

**目标**：密码迁移出明文 JSON，采用 OS 级凭据存储。

- **方案**：`keyring` crate → **Windows 凭据管理器**（VS Code / DBeaver 新版同路数；跨平台自动映射 macOS Keychain / Linux Secret Service）。
- **迁移流程**：启动时检测 connections.json 明文密码 → 逐条写入凭据管理器 → JSON 中改存引用 `credential:<id>` → 清除明文字段 → 记录迁移日志（不含密码）。
- **回退**：keyring 不可用（异常环境）→ 保留明文并 WARN + 首选项提示。
- **联动**：③ 导出、② 导入统一经此层取/存密码；日志规约不变（**禁打印密码**）。
- **存储分类**：凭据不属于 `storage.rs` 文件类别（OS 管理），需在「存储」设置中做说明文案。

## ⑥ 数据库会话管理 `[ ]`

**目标**：查看与管理当前数据库实例/租户的活动会话（类 PLSQL Developer → Sessions / Navicat → 工具-会话）。

- **能力开关**：`capabilities.sessions`（registry 下发；JDBC 通用类型默认关闭）。
- **方言层**（`SqlDialect` 新增约定）：
  - `sessions_query`：会话列表 SQL；
  - `kill_session_statement(kind, id)`：终止语句生成。
- **各库实现**：
  - **Oracle / OB Oracle**：`v$session`（OB 为租户内 `v$session`/`gv$`，按权限降级）关联 `v$sql` 取当前 SQL；`ALTER SYSTEM KILL SESSION 'sid,serial#' [IMMEDIATE]`。
  - **MySQL / OB MySQL**：`information_schema.processlist`；`KILL [QUERY] <id>`。
  - **PostgreSQL**：`pg_stat_activity`；`pg_cancel_backend(pid)`（取消）/ `pg_terminate_backend(pid)`（终止）。
- **UI**：会话管理以 **Tab 形式**（`QueryTab.type = 'session'`，遵循"禁抽屉"约定）；入口：工具栏图标 / 连接右键「会话管理」（按连接打开，去重）。
  - 列：会话 ID、用户、主机、程序、状态、当前 SQL、开始时间、耗时等（按方言裁剪）。
  - 功能：手动刷新 + 定时自动刷新（可关）、按用户/状态/关键字筛选、「取消查询」与「终止会话」分级操作（**二次确认**、危险操作红色标识）、查看会话详情。
- **权限处理**：无 `v$session`/`processlist` 权限时明确报错并 WARN 降级（遵循日志降级规约）。
- **约定**：执行终止语句走会话专用命令（`db_session_kill`），不污染查询 Tab；事务规约不适用。

## ⑦ 功能漫游指导 `[ ]`

**目标**：为全部功能区提供可回放的漫游指导（onboarding tour）。

- **方案**：`driver.js`（Apache-2.0、~5KB、零依赖、现代 API）。
- **设计**：
  - 锚点：稳定 UI 元素加 `data-tour="<step-id>"`（依赖 ① 图标与布局稳定后实施）。
  - 脚本：`src/tours/` 按功能区拆分（连接、查询、结果集、表查看、会话、设置、驱动管理），文案全部 i18n。
  - 入口：菜单「帮助 → 漫游指导」子菜单（按功能区选择）；首次启动自动引导 + 「跳过/不再显示」（首选项持久化）。
- **验收**：任一功能区可在无人工提示下完成一轮引导；窗口尺寸变化时浮层自适应。

## ⑧ Splash 启动窗体 `[ ]`

**目标**：启动即反馈，掩盖前端/后端初始化窗口期。

- **方案**（Tauri 2 官方 splashscreen 模式）：
  - `splashscreen.html` 独立小窗（品牌 Logo + 进度动画 + 版本号），`visible: true` 立即显示；
  - 主窗 `visible: false`，后端初始化完成 emit `app-ready` → 关闭 splash、显示主窗；
  - 超时兜底（如 8s 强制切主窗并 WARN）。
- **注意**：单实例插件与窗口事件时序；splash 不参与页签/工作区恢复逻辑。

## ⑨ 可改进方向归纳 `[ ]`（随任务持续维护）

- **性能**：结果集迁移 `el-table-v2` 虚拟滚动；前端 bundle（1.79MB）代码分割与重依赖懒加载。
- **质量**：美化器 / 导出 / 三方解密模块单元测试；引入 WebDriver e2e 冒烟。
- **安全**：凭据管理器（即④）；SQL 历史/日志敏感信息审计。
- **功能**：查询历史面板、多结果集展示、网格撤销/重做、ODBC 兜底通道、驱动热加载、会话管理扩展（锁等待/顶会话图表）。
- **分发**：NSIS + 代码签名 + tauri-updater 自动更新。
- **可达性 / 国际化**：键盘导航与焦点管理、i18n 完整性检查脚本。

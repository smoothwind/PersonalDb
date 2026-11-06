# PersonalDb 数据库场景测试方案

> 长期维护文档，与 `specifications.md` / `AGENTS.md` 配套。功能变更后同步修订对应用例。
> 用例编号 `TC-<域>-<序号>`；markdown checkbox 便于手动执行打勾；每条用例标注**适用库 / 前置条件 / 关联 SPEC**。
> 适用库缩写：MY = MySQL · OBM = OceanBase(MySQL 模式) · OBO = OceanBase(Oracle 模式) · PG = PostgreSQL · GA = GaussDB/openGauss · ORA = Oracle · LIT = SQLite · JD = JDBC(通用)
> 流程约束：执行中发现的缺陷按 **SPEC-DEV-3** 先提案后修复。

---

## 1. 测试环境矩阵

| 库 | 推荐版本 | 传输路径 | 连接参数要点 | 典型坑位 |
|---|---|---|---|---|
| MY | MySQL 8.0+ | mysql_async 原生 | 默认端口 3306 | 反斜杠转义（会话 vs 导出语义不同） |
| OBM | OceanBase 4.x CE | mysql 协议（TLS 已禁用） | 用户名须 `用户@租户`；直连 OBServer 2881 / OBProxy 2883 | 租户名误填 schema 报 Unknown database |
| OBO | OceanBase 4.x | **jdbc-bridge**（OceanBase Connector/J，需 JRE 8+） | 用户名 `用户@租户`；service_name 填**租户名**；**Schema 留空** | Oracle 模式租户不支持 MySQL 协议（服务端 1235） |
| PG | PostgreSQL 14+ | tokio-postgres | 未填 database 默认连 `postgres` 库 | list_databases 返回 **schema 列表**（非 pg_database） |
| GA | GaussDB 3.x / openGauss 5.x | tokio-gaussdb（sha256 认证） | 同 PG 语义 | 无 pg_sequences / pg_partitioned_table / wait_event / pg_blocking_pids |
| ORA | Oracle 11g ~ 19c/23ai | 默认 oracle-bridge（go-ora 免 OCI）；`driver=oci` 回退 ODPI-C | service_name / SID；**手动事务不支持**（能力位隐藏，DML 自动提交） | 非 Send 连接；DML 后须显式 commit 才对其他会话可见 |
| LIT | SQLite 3.x | rusqlite bundled | 连接=**文件路径**（.db/.sqlite/.sqlite3/.db3） | 不带 CREATE 标志（不静默建空库）；单库 main |
| JD | 按驱动 | jdbc-bridge + `drivers.json` | 驱动类/URL 模板/本地 jar（内置 OceanBase 预设） | generic 保守 ANSI 方言；对象树走 DatabaseMetaData |

环境准备建议：每个库各建一张含**全类型列**的样表（数值/字符/日期/二进制/CLOB·TEXT/JSON 视版本）+ 一组带注释的表 + 一组含 IN/OUT 参数的存储过程与函数（参数覆盖 NUMBER/VARCHAR2/DATE/无参/多参）。

> **本机容器方案（可选）**：WSL2 + Docker 拉起服务端库。已验证覆盖：MY、PG、GA、ORA、**OBM**（SQLite 无需容器，JDBC 复用任意容器）。**OBO 不可本地容器化**——Oracle 兼容模式是 OceanBase 企业版特性，社区版 `CREATE TENANT ... ob_compatibility_mode='oracle'` 报 1235，OBO 用例仍依赖企业实例（见第 4.2 节）。入口：`ops/dbtest/docker-compose.yml`（profiles: `light`/`ob`/`oracle`/`gauss`/`all`）+ `ob-init.sh`（内核参数/等待/资源腾挪一键化，含企业版建租户路径）。经验参数：WSL `.wslconfig` 建议 `memory=16GB` + `vmIdleTimeout=-1`（防空闲关机杀容器）；OB 容器需 `ulimits.nofile ≥ 20000`；**注意 docker logs 跨重启累积，"boot success!" 是无条件打印，真实就绪判据 = root@sys 可连接**。

---

## 2. 能力差异矩阵（功能显隐依据）

来源 `dialect capabilities()` → `db_registry` 下发前端；用例按此裁剪显隐断言。

| kind | 执行计划 | 导出 DDL | 导出 INSERT | 手动事务 | 会话管理 | 格式化方言 |
|---|---|---|---|---|---|---|
| MY / OBM | ✓（JSON 树） | ✓（SHOW CREATE） | ✓ | ✓ | ✓ | mysql |
| PG / GA | ✓（FORMAT JSON） | ✓（information_schema 自拼） | ✓ | ✓ | ✓ | postgresql |
| ORA / OBO | ✗（P4: DBMS_XPLAN） | ✓（DBMS_METADATA） | ✓ | ✗（隐藏） | ✓ | plsql |
| LIT | ✓（EXPLAIN QUERY PLAN 行式树特判） | ✓ | ✓ | ✓ | ✗（嵌入式无会话） | sql |
| JD | ✗ | ✗ | ✓ | ✓ | ✗ | sql |

对象类型超集：表 / 视图 / 索引 / 序列 / 函数 / 存储过程 / 触发器（各库实际下发按方言裁剪；LIT 仅表/视图/索引；JD 走 DatabaseMetaData）。树节点异常状态角标仅 ORA 系（INVALID/UNUSABLE）。

---

## 3. 功能域用例清单

### 3.1 连接管理（TC-CONN）

- [ ] **TC-CONN-001 八库新建连接**（全库）
  前置：环境矩阵中 8 个实例可用。步骤：按第 1 节参数逐一新建 → 测试连接 → 保存。预期：全部成功；树根级出现对应节点（未分组）。
- [ ] **TC-CONN-002 连接失败错误摘要**（全库）
  步骤：分别用错误端口 / 错误密码 / 错误 service_name / OB 错租户连接。预期：toast 呈现**一行可操作摘要**（10061 / ORA-01017 / ORA-12154 / OB 租户文案），不出现裸 `String(e)` 原文（原文进日志详情）。（SPEC-CONN-1）
- [ ] **TC-CONN-003 双击连接 & 树展开不隐式重连**
  步骤：①双击已断开连接节点 → 连接并加载库列表；②断开后展开节点 → 空且不发起连接；③重启应用（启动恢复）→ 不隐式重连。
- [ ] **TC-CONN-004 断开 / 重连**
  步骤：连接中右键断开 → 节点灰、页签连接点变灰 → 再连接 → 库列表恢复；查询页签断开态执行 → 提示连接丢失并可重连。
- [ ] **TC-CONN-005 密码凭据链路**（MY/ORA/OBM 代表）
  步骤：①保存连接后检查存储目录 connections.json → password 恒空、passwordRef 引用；②改密重连成功；③复制连接 → 凭据随复制（secret_copy）；④删除连接 → 凭据清理；⑤连接失败 → 日志/错误中无密码明文。
- [ ] **TC-CONN-006 连接分组**（UI 层，库无关）
  步骤：①新建根级文件夹 → 拖入连接；②嵌套子文件夹 → 拖动连接跨层级；③拖文件夹入自身后代 → 禁止并提示（循环防护）；④删除文件夹三策略（上移 / 平铺 / 连同连接与凭据删除）逐一验证。
- [ ] **TC-CONN-007 三方导入**（有 DBeaver / Navicat / PLSQL Dev 环境时）
  步骤：三类工具导出文件分别导入。预期：连接参数正确解析；密码 best-effort（失败置空并提示）。
- [ ] **TC-CONN-008 配置导出 / 导入**（库无关）
  步骤：①含密码导出 → 强制 AES-256-GCM 口令加密（信封 personaldb-vault）；②口令导入 → 凭据恢复可用；③错口令 → 明确报错。
- [ ] **TC-CONN-009 keepalive**（MY/ORA/OBO 任一）
  步骤：连接后闲置 >60s（服务端 wait_timeout 放宽）→ 再执行查询。预期：55s ping 保活，会话不失效；服务端不可达时仅 WARN 不踢会话。
- [ ] **TC-CONN-010 Splash 启动**（库无关）
  步骤：冷启动。预期：静态小窗先显 → 前端就绪 `app_ready` 后主窗显示，无白屏闪烁。
- [ ] **TC-CONN-011 托盘与关闭拦截**（库无关）
  步骤：①开「关闭到托盘」偏好 → 关窗 → 主窗隐藏、托盘在；②托盘左键显隐切换；③托盘右键「退出」→ 真退出（不经关闭拦截）；④文案随语言热切换。

### 3.2 对象树（TC-TREE）

- [ ] **TC-TREE-001 库列表加载**（全库）
  预期：MY/OBM=数据库；PG/GA=**schema 列表**；ORA/OBO=用户 schema；LIT=固定 `main`；JD=目录策略。
- [ ] **TC-TREE-002 对象分组加载**（全库）
  预期：按方言能力分组（表/视图/索引/序列/函数/存储过程/触发器）；LIT 内部对象（sqlite_%）过滤；JD 走元数据。
- [ ] **TC-TREE-003 搜索过滤**：关键词过滤节点名；清空恢复。
- [ ] **TC-TREE-004 单击 / 双击语义**：单击表仅选中；双击表/视图 → 打开 SQL 编辑器并生成前 100 行预览 SQL，**不自动执行**。
- [ ] **TC-TREE-005 右键菜单全项**（表/视图/例程/非表对象四类）
  预期：表 = 查询数据/生成 SQL▸/查看结构/查看源码/导出；**视图 INSERT 置灰**；例程 = **执行**（首项）/编辑例程/(ORA 系)重新编译；非表对象 = 查看定义；未连接连接节点首项 = 连接。
- [ ] **TC-TREE-006 对象状态角标**（ORA/OBO）
  步骤：手工置一个过程 INVALID（如改其依赖表列）→ 刷新。预期：树节点 INVALID 角标；右键重新编译 → 成功后角标消失。
- [ ] **TC-TREE-007 拖拽规则**：仅连接/文件夹可拖起；库/对象节点拖起显示禁止光标；非法落点不落库。
- [ ] **TC-TREE-008 叶节点无展开箭头**：表/视图/其余对象 isLeaf，无无用箭头。

### 3.3 查询执行（TC-QRY）

- [ ] **TC-QRY-001 单语句执行**（全库）
  步骤：SELECT 执行。预期：结果网格呈现、页签连接色点、runSeq 正常、耗时显示。
- [ ] **TC-QRY-002 选中执行**：编辑器内选中一段中的一部分 → 仅选中部分执行。
- [ ] **TC-QRY-003 多语句逐条执行 + 逐条日志**（全库；SPEC-LOG-3）
  步骤：≥3 条语句（混合 SELECT/DML）选中执行。预期：①多结果集页签；②日志面板出现 N 条「第 i/N 条 · 摘要」独立记录（各带独立耗时/行数）；③无聚合条目；④查询历史保留整段 SQL。
- [ ] **TC-QRY-004 多语句部分失败中止**
  步骤：第 2 条改为非法 SQL 重跑。预期：第 2 条日志 error 级含完整错误详情；第 1 条 info 在案；toast 提示失败位置（multiFailAt）；后续不再执行；结果区显示已成功部分。
- [ ] **TC-QRY-005 Oracle 系 PL 块整段执行**（ORA/OBO）
  步骤：①含 `BEGIN..END` 匿名块的多语句文本；②`DECLARE` 块；③`CREATE PROCEDURE` 脚本；④字符串/注释里含 `BEGIN..END` 字样的普通多语句。预期：①②③整段发送不拆分；④按分号拆分逐条（字符串/注释不误判）。
- [ ] **TC-QRY-006 OB Oracle 多语句回归**（OBO）
  步骤：普通多语句（无 PL 块）执行。预期：客户端拆分逐条，不出现 ORA-00600 [5031]。
- [ ] **TC-QRY-007 停止按钮**：构造慢查询（大表笛卡尔//pg_sleep）→ 执行中点停止 → 界面即时复位，挂起请求作废（runSeq 自增）。
- [ ] **TC-QRY-008 执行计划**（MY/OBM/PG/GA/LIT 有入口；ORA/OBO/JD 无）
  步骤：对同一条 SELECT 分别在 4 个库取计划。预期：MySQL/PG 解析为结构化树；SQLite 行式输出构建树（EXPLAIN QUERY PLAN 特判）；Oracle 无入口不报错。
- [ ] **TC-QRY-009 查询历史**：成败均入栈（SQLite history.db）；点击回填 tab.sql；上限 500；截断 2000 字符。
- [ ] **TC-QRY-010 快捷指令展开**：输入缩写 + 空格/Tab → 模板展开；`${1:占位符}` Tab 跳转。
- [ ] **TC-QRY-011 编辑器辅助**：方言高亮随库切换（mysql/plsql/postgresql/sql）；Ctrl+Shift+F 格式化；大小写转换与压缩跳过字符串字面值；自动补全（表/列/注释）。
- [ ] **TC-QRY-012 页签管理**：双击标题重命名；同连接空 Tab 复用；上限 50 提示。

### 3.4 结果集（TC-RST）

- [ ] **TC-RST-001 基本呈现**：斑马纹偏好；行高三档（22/28/36）。
- [ ] **TC-RST-002 分页续拉**（全库）：`pageSize` 默认 10 → 滚动触底续拉 → `maxTotalRows`（默认 10000）截断提示。
- [ ] **TC-RST-003 OBO ROWNUM 分页续拉**（OBO 专项）：>1000 行表触底续拉 → 物化层静默丢弃 `pdb_rn` 列、数据不错位（P2 瑕疵回归：不恶化）。
- [ ] **TC-RST-004 列排序与拖拽列序**：表头排序；拖拽调整列序；存在固定列时禁拖。
- [ ] **TC-RST-005 筛选与固定列**：列设置弹层开筛选 → 过滤生效；固定左/右列；清空恢复。
- [ ] **TC-RST-006 列显隐**：回显列弹层勾选 → **暂存后确认再重绘**（取消不生效）；持久化 colSettings。
- [ ] **TC-RST-007 格式化双层**（SPEC-RESULT-1）：全局默认（json 缩进/日期三格式/千分位/长文本截断）+ 列级覆盖优先；列级仅影响该列。
- [ ] **TC-RST-008 单元格语义详情**：双击 JSON→树、BLOB→HEX、图片→预览、日期→多格式。
- [ ] **TC-RST-009 多选复制**：拖选/Ctrl 选区 → 复制单元格/整行/本列/选区；粘贴到 Excel 行列对齐。
- [ ] **TC-RST-010 导出**（全库抽 MY/ORA/LIT）：Excel/SQL INSERT/CSV/TXT 四格式；导出默认项取首选项；工作区 exportDir 默认路径；失败入日志。
- [ ] **TC-RST-011 虚拟滚动**：>600 行结果 → ResultVirtualGrid（排序/右键复制正常）；进入编辑态退化为完整表格。
- [ ] **TC-RST-012 多结果集切换**：多 SELECT 语句 → 结果页签横切，各页独立排序/复制。
- [ ] **TC-RST-013 特殊值呈现**：NULL 显示为占位符；二进制 base64 传输无乱码；超长文本截断点击展开。

### 3.5 结果集编辑（TC-EDIT）

- [ ] **TC-EDIT-001 编辑态进入**（MY/ORA/PG 任一）：单表 SELECT → 编辑开关可用；JOIN/聚合 SQL → 不可编辑提示。
- [ ] **TC-EDIT-002 定位策略**：有主键表走主键；ORA 无主键走 ROWID；均无走全列匹配。
- [ ] **TC-EDIT-003 改值 → 预览 → 应用**（SPEC-UI 相关）
  步骤：改单元格 → 生成变更预览（db_build_dml）→ 应用。预期：逐条执行 → **全部成功才提交，失败回滚**；结果刷新。
- [ ] **TC-EDIT-004 编辑撤销/重做**：五类变更入口（改值/增删行等）均可 Ctrl+Z 撤销、Ctrl+Y 重做；单元格输入中不拦截。
- [ ] **TC-EDIT-005 空值/类型边界**：NULL 化、日期格式、数字精度修改各一轮。

### 3.6 表查看面板（TC-TIP）

- [ ] **TC-TIP-001 列页签**（全库）：列名/类型/可空/默认值/**注释**呈现。
- [ ] **TC-TIP-002 索引页签**：索引列/唯一性；LIT 内部索引过滤。
- [ ] **TC-TIP-003 源码页签 + DDL 注释附加**（ORA/OBO 回归）
  预期：源码 = `CREATE TABLE ...` 尾部自动附 `COMMENT ON TABLE` + 逐列 `COMMENT ON COLUMN`（有注释才附加；无注释表零变化）。MY 内联注释（SHOW CREATE）；PG 自拼 DDL。
- [ ] **TC-TIP-004 结构编辑与变更执行**：改列名/类型/默认值/备注、新增列（绿色标记）→ 生成变更 SQL → 应用 → 刷新生效。
- [ ] **TC-TIP-005 去重与命名**：同表重复打开激活既有页签；双击标题重命名。

### 3.7 对象查看面板（TC-OIP）

- [ ] **TC-OIP-001 非表对象定义**：ORA 序列/触发器/包、PG 视图定义等 → 只读 CodeViewer（Tab 形式，无抽屉）。
- [ ] **TC-OIP-002 DDL 清洗三开关**：源码查看器「去存储 / 去 schema / 去引号」与设置页「SQL 生成」双向同步、改动即持久化、重开面板回显一致。

### 3.8 例程编辑与执行（TC-RTE，SPEC-ROUTINE-1）

- [ ] **TC-RTE-001 例程编辑 Tab**：树右键「编辑例程」→ Tab 打开（去重）；头部名称/方言/类型/函数返回类型。
- [ ] **TC-RTE-002 参数面板**：序号/名称/模式（IN/OUT/INOUT）/数据类型；无参提示。
- [ ] **TC-RTE-003 源码保存**（ORA/OBO/MY）：编辑 → 保存（CREATE OR REPLACE）→ 重新加载；失败报错不覆盖原源码。
- [ ] **TC-RTE-004 执行入口**：树右键例程节点**首项「执行」**（act_run 图标）+ 例程编辑工具栏「执行」两处均可；SQLite 无例程入口。
- [ ] **TC-RTE-005 参数对话框**：值留空/NULL 勾选 → `NULL`；纯数字 → 数值内联；原样勾选 → 表达式原文内联（如 `TO_DATE('2024-01-01','YYYY-MM-DD')`）；其余按方言字符串转义。
- [ ] **TC-RTE-006 方言调用矩阵**：

  | 场景 | MY/OBM | ORA/OBO | PG/GA |
  |---|---|---|---|
  | 函数执行 | `SELECT db.f(..)` | `SELECT s.f(..) FROM DUAL` | `SELECT f(..)` |
  | 过程（IN） | `CALL db.p(..)` | 匿名块 `BEGIN s.p(..); END;` | `CALL p(..)` |
  | 过程（OUT） | `CALL @pdb_a*` + SELECT 回显 | DECLARE 变量块 | OUT 位 NULL 占位 |

- [ ] **TC-RTE-007 MySQL OUT 回显**（MY）：带 1 IN + 2 OUT 的过程 → 执行 → 第二个结果集回显两列输出值。
- [ ] **TC-RTE-008 OBO OUT 过程执行**（OBO 回归 ORA-00600 -5544）
  预期：DECLARE 生成 `pdb_a{n} VARCHAR2(32767)` 等归一化类型 → 执行成功（副作用生效）；toast 提示 OUT 值不回显；日志标题显示块体内调用行而非「DECLARE」。
- [ ] **TC-RTE-009 裸类型归一化**（ORA/OBO）：OUT 参数类型为裸 `VARCHAR2`/`RAW`/`CHAR` → 声明补 `(32767)`/`(2000)`；带长度/NUMBER/CLOB 原样。
- [ ] **TC-RTE-010 PG CALL**（PG≥11）：无 OUT 过程 CALL 成功；OUT 占位 + toast 提示。
- [ ] **TC-RTE-011 自动执行与结果承载**：确认后查询页签 force 新建并自动 runTab；多结果集/导出/虚拟滚动在结果区可用。

### 3.9 SQL 生成与清洗（TC-GEN）

- [ ] **TC-GEN-001 子菜单 8 项**：生成 SELECT / INSERT / **CREATE 语句** / 4 个列名预设；视图 INSERT 置灰；**全部复制到剪贴板 + toast，不开页签**。
- [ ] **TC-GEN-002 五偏好即时生效**（`personaldb.sqlgen`）：去限定符（仅 SELECT/INSERT）/ 标识符大小写（含 CREATE 文本）/ 去存储 / 去 schema / 去引号 —— 逐项开关后重新生成对比；设置面板与源码查看器联动。
- [ ] **TC-GEN-003 大小写边界**：CREATE 中字符串字面值不受大小写转换影响（convertOutsideLiterals）。
- [ ] **TC-GEN-004 清洗组合**（ORA DDL 抽查）：schema+引号+存储三开关组合结果与单测语义一致；`-- pdb-ignore` 不受影响。

### 3.10 模板系统（TC-TMP，SPEC-TEMPLATE-1）

- [ ] **TC-TMP-001 三级解析**：方言覆盖 `<key>.<kind>` → 通用默认 `<key>.default` → 内置兜底（分别在设置页配置两级后观察查询行为）。
- [ ] **TC-TMP-002 设置页**：8 功能组折叠；按种类/覆盖状态筛选；usage 说明。
- [ ] **TC-TMP-003 安全红线**：`kill_session_sql` / 标识符引用 / INSERT 字面量 / `recompile_sql` 无覆盖入口；覆盖模板占位符不可增删（缺 `{sql}` 回退内置）。
- [ ] **TC-TMP-004 分页别名规约**：Oracle 系模板禁含 LIMIT、LIMIT 系禁 ROWNUM；别名字母开头（pdb_rn/pdb_p）——故意配置违规模板观察回退/报错路径。

### 3.11 危险操作确认（TC-SAFE，SPEC-SAFE-1）

- [ ] **TC-SAFE-001 检测矩阵**：DROP/DELETE 无 WHERE/TRUNCATE/ALTER 等逐类触发；SELECT 不触发。
- [ ] **TC-SAFE-002 四档位**：每类风险改档（不提示/弹窗确认/输对象名/阻止）→ 行为逐档验证；档位持久化（`personaldb.danger`）。
- [ ] **TC-SAFE-003 输名确认**：danger 未降级时须输入首个对象名；错误名拒绝；取消按钮默认焦点。
- [ ] **TC-SAFE-004 事务降级**：手动事务模式下 danger → 普通认可弹窗。
- [ ] **TC-SAFE-005 逃生门**：SQL 尾加 `-- pdb-ignore` → 不拦截。

### 3.12 会话管理（TC-SES）

- [ ] **TC-SES-001 入口显隐**：工具栏+树右键按 capabilities.sessions（MY/OBM/PG/GA/ORA/OBO 有；LIT/JD 无）。
- [ ] **TC-SES-002 会话列表**：固定列序 id,user,host,program,state,secs,extra,sqlText,blocking。
- [ ] **TC-SES-003 OBO 兼容视图**（OBO）：走 `V$OB_PROCESSLIST`/`V$OB_LOCKS`；标准 ORA 走 v$session/v$lock。
- [ ] **TC-SES-004 终止会话**：kill 单个 + 批量终止；id 严格数字校验（注入尝试报错不执行）；ORA 系按 ID 终止。
- [ ] **TC-SES-005 锁视图**：构造行锁竞争 → 锁视图呈现阻塞关系；CSV 导出；>300 行虚拟化。

### 3.13 事务（TC-TXN）

- [ ] **TC-TXN-001 事务区显隐**：MY/OBM/PG/GA/LIT 显示；ORA/OBO/JD 按 capabilities（ORA 系隐藏）。
- [ ] **TC-TXN-002 手动模式隔离**（MY/PG）：切手动 → 连接 A 更新未提交 → 连接 B 不可见 → 提交后可见；回滚后数据还原。手动模式走**会话专用连接**。
- [ ] **TC-TXN-003 ORA 自动提交语义**：ORA 下执行 UPDATE（事务区隐藏）→ 新开会话可见（自动提交已生效）——防止误以为需 commit。
- [ ] **TC-TXN-004 结果集编辑事务边界**：多行变更中一条失败 → 全部回滚、自动提交恢复。

### 3.14 导出（TC-EXP）

- [ ] **TC-EXP-001 全量导出**：含 DDL + 数据（INSERT）→ 执行导入方验证可重放（DDL 注释语句随导出，ORA 系含 COMMENT ON）。
- [ ] **TC-EXP-002 方言字面量细节**：ORA 日期 `TO_TIMESTAMP`（含/不含毫秒）；MySQL 文件头 `SET NO_BACKSLASH_ESCAPES=1` 且 literal 不转义反斜杠；二进制统一 HEX（`X''`/decode/HEXTORAW）。
- [ ] **TC-EXP-003 流式与失败**：大表（>10 万行）导出不爆内存；中途失败（磁盘满/权限）→ 报错入日志、无半截文件残留。

### 3.15 日志（TC-LOG，SPEC-LOG-2/3）

- [ ] **TC-LOG-001 面板行为**：入口（Ctrl+L/状态栏/查看菜单）；高度与显隐持久化；级别过滤；自动滚动（上翻暂停贴底）；点击行展开详情；清空。
- [ ] **TC-LOG-002 未读徽标**：面板关闭期间制造错误 → 状态栏徽标计数 → 打开清零。
- [ ] **TC-LOG-003 匿名块摘要回归**：执行 DECLARE 块 → 标题为块体内调用行（非「DECLARE」）。
- [ ] **TC-LOG-004 日志文件**：`logs/personaldb.log` 轮转；「工具→查看日志」窗体（过滤/自动刷新/打开目录）；debug 构建分级。
- [ ] **TC-LOG-005 敏感红线**：密码永不落日志；SQL 仅前 200 字符且失败才落盘完整前缀；连接目标脱敏（JDBC URL 授权段打码）。

### 3.16 工作台 / 首选项 / 国际化 / 更新（TC-WSB）

- [ ] **TC-WSB-001 停靠布局**：四区拖拽重排（预览遮罩/热区）；**落点仅响应面板拖拽**（结果集列拖拽不触发遮罩卡死——SPEC-UI-23 回归）；窄容器（140px）不裁切。
- [ ] **TC-WSB-002 悬浮窗**：双击面板头 停靠⇄悬浮；缩放/置顶/回收停靠/最小化层叠条；隐藏面板从面板条唤回。
- [ ] **TC-WSB-003 重置布局**：查看菜单 + `Ctrl+Alt+R` → 恢复默认。
- [ ] **TC-WSB-004 命令面板**：`Ctrl+K` 全局生效（编辑器聚焦也生效）；应用命令/连接/最近 SQL 注入；Esc 关闭；同一动作同图标（SPEC-UI-8）。
- [ ] **TC-WSB-005 快捷键**：global/editor 双 scope；自定义绑定实时生效；输入框聚焦不拦截 global。
- [ ] **TC-WSB-006 漫游**：帮助菜单触发；隐藏锚点自动剔除；首次启动自动播放（tourDone）。
- [ ] **TC-WSB-007 首选项即时生效**：语言（EP 文案联动 el-config-provider）/主题/字体族与滑杆（实时预览）/行高/数据格式/结果集档位。
- [ ] **TC-WSB-008 i18n 完整性**：`npm run check:i18n` 绿；界面语言切换全量覆盖；含 `@` 文案（OB `user@tenant`）不崩（`{'@'}` 转义）。
- [ ] **TC-WSB-009 更新检测**（SPEC-DEV-2）：检查更新 → 命中显示状态栏入口 + 菜单红点；「不再提示此版本」；无 Release 404 文案；预发布（标签含 `-`）不入正式检测。

---

## 4. 方言专项差异用例

### 4.1 Oracle（ORA）

- [ ] **TC-ORA-001 DDL 注释附加三处统一**：生成 CREATE（树右键）/ 查看源码（表查看面板）/ 导出 SQL → 尾部均附 `COMMENT ON TABLE/COLUMN`；无注释表零变化；权限受限视图失败 best-effort 降级原样。
- [ ] **TC-ORA-002 桥接回退**：临时移除 `binaries/oracle-bridge*` → 连接自动回退 OCI 路径（或明确报错提示）；恢复后重新走桥接。
- [ ] **TC-ORA-003 手动事务隐藏语义**：事务区不显示；UPDATE 后**不 commit** → 其他会话**可见**（自动提交）——确认 UI 不会让用户误以为需要手动提交。
- [ ] **TC-ORA-004 ROWID 编辑**：无主键表结果集编辑 → ROWID 定位 UPDATE 成功。
- [ ] **TC-ORA-005 会话终止**：v$session 选中会话 kill → 服务端会话消失；DBA 权限受限时的错误降级 WARN。

### 4.2 OceanBase Oracle 模式（OBO）

- [ ] **TC-OBO-001 连接参数矩阵**：①用户名不带租户 → 报错可读；②Schema 填租户名 → `Unknown database` 类错误摘要（不裸抛）；③Schema 留空 + service_name=租户 → 成功。
- [ ] **TC-OBO-002 多语句回归**：普通多语句逐条执行无 ORA-00600 [5031]；PL 块整段执行。
- [ ] **TC-OBO-003 OUT 过程**：裸 VARCHAR2 OUT 参数执行成功（-5544 回归）；日志标题为调用行。
- [ ] **TC-OBO-004 分页续拉**：千行级表滚动加载 → 数据完整不错位（pdb_rn 被丢弃属已知 P2）。
- [ ] **TC-OBO-005 注释链路**：describe 的列注释 → 表查看面板注释列 + DDL COMMENT ON 附加均正确（all_col_comments 联查）。

### 4.3 MySQL / OB MySQL（MY / OBM）

- [ ] **TC-MY-001 SHOW CREATE 内联注释**：带表/列注释的表 → 源码页签注释以内联 `COMMENT '...'` 呈现（无 COMMENT ON 追加）。
- [ ] **TC-MY-002 反斜杠语义**：含 `\` 的字符串插入（会话执行走 dml_literal 转义）与导出（文件头 NO_BACKSLASH_ESCAPES，literal 不转义）分别回放验证一致。
- [ ] **TC-MY-003 CALL OUT 回显**：见 TC-RTE-007。
- [ ] **TC-MY-004 OBM 租户语义**：`用户@租户` 连接；Oracle 模式租户误连 MySQL 协议端口 → 服务端 1235 错误降级为可读 WARN。

### 4.4 PostgreSQL / GaussDB（PG / GA）

- [ ] **TC-PG-001 schema 语义**：切换库（实为 schema）不重连即生效；跨 schema 同名表互不干扰。
- [ ] **TC-PG-002 默认库**：database 留空 → 自动连 `postgres`，不出现「以用户名为库名」失败。
- [ ] **TC-PG-003 计划解析**：EXPLAIN (FORMAT JSON) → 结构化树（cost/rows）。
- [ ] **TC-GA-001 序列与分区差异**：无 pg_sequences → 序列列表走 pg_class.relkind='S'；无 pg_partitioned_table 不报错。
- [ ] **TC-GA-002 会话降级**：会话面板无 wait_event/pg_blocking_pids 列（字段缺失不崩）；sha256 认证连接成功。

### 4.5 SQLite（LIT）

- [ ] **TC-LIT-001 文件连接**：选择 .db 文件连接成功；**不存在的路径不静默建库**（报错）。
- [ ] **TC-LIT-002 单库与内部对象**：库列表固定 main；sqlite_% 对象在目录/索引/快照中被 ESCAPE 过滤。
- [ ] **TC-LIT-003 计划特判**：EXPLAIN QUERY PLAN 行式输出 → 树形呈现。
- [ ] **TC-LIT-004 能力裁剪**：无例程执行入口 / 无会话面板 / 事务可用（BEGIN..COMMIT 走手动模式验证）。

### 4.6 JDBC 通用（JD）

- [ ] **TC-JD-001 驱动管理**：新增驱动配置（名称/驱动类/URL 模板/jar）→ 连接成功；缺 jar → 明确报错；OB 预设开箱可用。
- [ ] **TC-JD-002 generic 方言**：对象树走 DatabaseMetaData；SQL 生成/INSERT 可用；无 DDL 导出/无计划入口。

---

## 5. 发版回归基线（Release 前 ~35 分钟必跑）

1. 八库连接冒烟（TC-CONN-001）+ 断开重连（TC-CONN-004）
2. 各库单语句查询 + 多语句逐条日志（TC-QRY-001/003）
3. OBO 三回归：多语句 5031 / OUT 过程 -5544 / 分页续拉（TC-OBO-002/003/004）
4. ORA DDL 注释附加三处（TC-ORA-001）+ 例程执行入口（TC-RTE-004/008）
5. 结果集核心：分页/排序/筛选/复制/导出 Excel+INSERT（TC-RST-002/004/005/009/010）
6. 危险操作档位抽查（TC-SAFE-002）+ 逃生门（TC-SAFE-005）
7. 表查看面板结构/源码/编辑变更（TC-TIP-001/003/004）
8. SQL 生成五偏好（TC-GEN-002）
9. 会话面板 MY + ORA 各一轮（TC-SES-002/004）
10. i18n 校验 + `npm test` + `npm run build` + `cargo check`（默认 + no-default）四绿

---

## 6. 自动化现状与补强建议

### 6.1 既有单测覆盖图（tests/*.spec.ts，Vitest/jsdom）

| spec | 覆盖 |
|---|---|
| sqlGen / ddl-clean | 生成 SQL、五偏好清洗、组合语义 |
| routine-exec | 例程调用 SQL 矩阵、字面量规则、裸类型归一化 |
| sql-split | 分割与 hasPlSqlBlock（字符串/注释不误判） |
| sql-templates | 模板三级解析与占位符 |
| risky-sql | 危险操作检测矩阵 |
| logs / logs-title | 日志 store、匿名块摘要、逐条标题 |
| result-pipeline / filter-core | 结果管线（矩阵+Worker+物化）、过滤核心 |
| edit-undo | 五类变更撤销栈 |
| conn-error | 错误摘要规则表（套接字码/ORA 码/驱动文案） |
| connections | 连接分组/存储 choke point |
| history-panel / preferences-query / layout / context-menu / i18n-keys / about / update-version | 历史面板 / 查询偏好 / 布局 / 右键菜单模型 / i18n key 对齐 / About / 版本比较 |

Rust 侧：`cargo check`（默认 + `--no-default-features`）零警告；CI 强制 check:i18n。

### 6.2 补强建议（纯前端单测，按收益排序）

1. **routineExec 字符串转义边界**：多行字符串/引号混用/超长输入（补 routine-exec.spec）
2. **sqlTitle 块体回退分支**：DECLARE 无 BEGIN / 嵌套块标记（补 logs-title.spec）
3. **会话 kill id 校验纯函数**：非数字/负数/超长（若无独立函数则先抽函数再测）
4. **capability 裁剪矩阵快照**：mock db_registry 输出断言各库显隐集合
5. E2E 不引入（Tauri 桌面 E2E 成本高；第 5 节手动基线兜底）

---

## 7. 已知限制与边缘场景登记（P2，测试时不算缺陷）

| 项 | 说明 | 登记 |
|---|---|---|
| OBO OUT 不回显 | 桥接协议无 CallableStatement 绑定；副作用生效 + toast 提示 | SPEC-ROUTINE-1 |
| OBO 分页 pdb_rn | ROWNUM 包装模板使续拉页多一列，物化层静默丢弃（不错位） | SPEC-ROUTINE-1 相关 |
| CALL 不触发 danger | 危险检测按语句关键字，过程内部风险不感知 | SPEC-ROUTINE-1 |
| ORA 无执行计划 | P4: DBMS_XPLAN 文本解析 | capabilities |
| ORA 手动事务 | 非 Send 连接不支持；事务区按能力隐藏 | SPEC-TX |
| OBO experimental | transport 标记实验性（oracle 协议路径） | registry |
| logTitle count 参数 | `sqlTitle(sql, count)` 聚合前缀保留但主路径已逐条化 | SPEC-LOG-3 |

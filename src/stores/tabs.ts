import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { dbExecute, dbPreviewRowsSql } from '../api/db'
import { i18n } from '../i18n'
import { useConnectionStore } from './connections'
import { useHistoryStore } from './history'
import { usePreferencesStore } from './preferences'
import { useLogStore, sqlTitle } from './logs'
import { logger } from '../utils/logger'
import { splitStatements } from '../utils/sqlSplit'
import type { DbKind, DbObjectType, QueryResult, TabSnapshot } from '../types/db'

const tt = i18n.global.t

export interface QueryTab {
  id: string
  title: string
  /** 页签类型：query=查询编辑器，table=表查看面板，object=对象定义查看，routine=例程编辑器，session=会话管理 */
  type: 'query' | 'table' | 'object' | 'routine' | 'session'
  connectionId: string
  connectionName: string
  kind: DbKind
  /** 当前库 / schema 上下文 */
  database: string
  sql: string
  result: QueryResult | null
  error: string
  /** 错误类别：timeout = 执行超时（错误条据此提供「调整超时」入口） */
  errorKind?: 'timeout'
  running: boolean
  /** 执行序列号：作废过期请求（停止按钮/防结果错位） */
  runSeq: number
  /** 页签图标 key（objectIcons 映射：query/table/view/function/...） */
  icon: string
  /** 表查看面板：目标表名 */
  table?: string
  /** 对象/例程面板：目标对象名（去重用） */
  objectName?: string
  /** 对象/例程面板：对象类型 */
  objType?: DbObjectType
  /** 分页续拉：末次执行的 SQL（滚动加载基准） */
  baseSql?: string
  /** 分页续拉：后端仍有更多行 */
  hasMore?: boolean
  /** 分页续拉：加载中 */
  loadingMore?: boolean
  /** 多结果集（多语句执行；result 始终指向 activeResultIndex 对应项） */
  multiResults?: QueryResult[]
  activeResultIndex?: number
}

let tabSeq = 0

function buildSelectSql(kind: DbKind, database: string, table: string): string {
  switch (kind) {
    case 'mysql':
    case 'oceanbase':
      return `SELECT * FROM \`${database}\`.\`${table}\` LIMIT 100;`
    case 'postgresql':
      return `SELECT * FROM "${database}"."${table}" LIMIT 100;`
    case 'sqlite':
      return `SELECT * FROM "${table}" LIMIT 100;`
    case 'oracle':
    case 'oceanbase-oracle':
      return `SELECT * FROM "${table}" WHERE ROWNUM <= 100`
    case 'jdbc':
      return `SELECT * FROM \"${table}\"`
  }
}

/** 执行超时错误（错误条据此提供「调整超时」直达设置入口） */
export class ExecTimeoutError extends Error {}

/** 连接类错误（区别于 SQL 语法/约束错误）：命中则重连后重试一次；供错误条等处复用 */
export function looksLikeConnError(msg: string): boolean {
  const m = msg.toLowerCase()
  if (m.includes('timeout') || m.includes('超时')) return false
  return (
    m.includes('session') ||
    m.includes('not connected') ||
    m.includes('connection') ||
    m.includes('连接') ||
    m.includes('closed') ||
    m.includes('broken') ||
    m.includes('pipe') ||
    m.includes('disconnect') ||
    m.includes('断开')
  )
}

export const useTabsStore = defineStore('tabs', () => {
  const tabs = ref<QueryTab[]>([])
  const activeTabId = ref<string>('')

  const activeTab = computed(() => tabs.value.find((t) => t.id === activeTabId.value) ?? null)

  const connStore = useConnectionStore()
  const prefsStore = usePreferencesStore()
  const historyStore = useHistoryStore()
  const logStore = useLogStore()

  function openTab(input: {
    connectionId: string
    connectionName: string
    kind: DbKind
    database?: string
    title?: string
    sql?: string
    /** 页签图标 key（默认 query） */
    icon?: string
    /** 页签类型（默认 query） */
    type?: 'query' | 'table' | 'object' | 'routine' | 'session'
    /** 表查看面板：目标表名 */
    table?: string
    /** 对象/例程面板：目标对象名 */
    objectName?: string
    /** 对象/例程面板：对象类型 */
    objType?: DbObjectType
    /** 强制新建（跳过“同连接空 Tab 复用”） */
    force?: boolean
  }): QueryTab {
    // 复用同连接的空白查询 Tab，避免 Tab 泛滥（force 时跳过；仅 query 类型，防止误激活空表/对象/会话面板）
    if (!input.force && !input.sql && !input.title && (input.type ?? 'query') === 'query') {
      const reusable = tabs.value.find(
        (t) => t.connectionId === input.connectionId && t.type === 'query' && !t.sql.trim(),
      )
      if (reusable) {
        if (input.database) reusable.database = input.database
        activeTabId.value = reusable.id
        return reusable
      }
    }
    // 全局页签数量上限
    const limit = prefsStore.ui.maxTabs > 0 ? prefsStore.ui.maxTabs : 50
    if (tabs.value.length >= limit) {
      ElMessage.warning(tt('common.tabLimitReached', { n: limit }))
      return activeTab.value ?? tabs.value[0]
    }
    tabSeq += 1
    const tab: QueryTab = {
      id: crypto.randomUUID(),
      title: input.title ?? tt('query.tabTitle', { n: tabSeq }),
      type: input.type ?? 'query',
      connectionId: input.connectionId,
      connectionName: input.connectionName,
      kind: input.kind,
      database: input.database ?? '',
      sql: input.sql ?? '',
      result: null,
      error: '',
      running: false,
      runSeq: 0,
      icon: input.icon ?? 'query',
      table: input.table,
      objectName: input.objectName,
      objType: input.objType,
    }
    tabs.value.push(tab)
    activeTabId.value = tab.id
    return tab
  }

  function closeTab(id: string): void {
    const idx = tabs.value.findIndex((t) => t.id === id)
    if (idx < 0) return
    tabs.value.splice(idx, 1)
    if (activeTabId.value === id) {
      activeTabId.value = tabs.value[Math.min(idx, tabs.value.length - 1)]?.id ?? ''
    }
  }

  function closeTabsOfConnection(connectionId: string): void {
    for (const t of tabs.value.filter((t) => t.connectionId === connectionId)) {
      closeTab(t.id)
    }
  }

  /** 关闭除指定页签外的所有页签 */
  function closeOthers(id: string): void {
    tabs.value = tabs.value.filter((t) => t.id === id)
    activeTabId.value = id
  }

  /** 关闭指定页签及其右侧的所有页签 */
  function closeLeft(id: string): void {
    const idx = tabs.value.findIndex((t) => t.id === id)
    if (idx <= 0) return
    tabs.value = tabs.value.slice(idx)
    if (!tabs.value.some((t) => t.id === activeTabId.value)) {
      activeTabId.value = id
    }
  }

  function closeRight(id: string): void {
    const idx = tabs.value.findIndex((t) => t.id === id)
    if (idx < 0) return
    tabs.value = tabs.value.slice(0, idx + 1)
    if (!tabs.value.some((t) => t.id === activeTabId.value)) {
      activeTabId.value = id
    }
  }

  /** 关闭全部页签 */
  function closeAll(): void {
    tabs.value = []
    activeTabId.value = ''
  }

  /** 导出页签快照（工作区持久化，不含结果集） */
  function exportSnapshot(): TabSnapshot[] {
    return tabs.value.map((t) => ({
      type: t.type,
      title: t.title,
      connectionId: t.connectionId,
      connectionName: t.connectionName,
      kind: t.kind,
      database: t.database,
      sql: t.sql,
      icon: t.icon,
      table: t.table,
      objectName: t.objectName,
      objType: t.objType,
    }))
  }

  /** 从快照恢复页签（工作区切换/启动恢复） */
  function importSnapshot(list: TabSnapshot[]): void {
    tabs.value = list.map((s) => ({
      id: crypto.randomUUID(),
      type: s.type,
      title: s.title,
      connectionId: s.connectionId,
      connectionName: s.connectionName,
      kind: s.kind,
      database: s.database,
      sql: s.sql,
      result: null,
      error: '',
      running: false,
      runSeq: 0,
      icon: s.icon,
      table: s.table,
      objectName: s.objectName,
      objType: s.objType,
    }))
    activeTabId.value = tabs.value[0]?.id ?? ''
  }

  /** 单语句执行 + 超时保护（秒，0 = 不限制；超时后不悬挂定时器） */
  function withExecTimeout<T>(p: Promise<T>): Promise<T> {
    const sec = Number(prefsStore.query.execTimeoutSec)
    if (!Number.isFinite(sec) || sec <= 0) return p
    let timer: number | undefined
    return Promise.race([
      p,
      new Promise<never>((_, reject) => {
        timer = window.setTimeout(
          () => reject(new ExecTimeoutError(tt('query.execTimeout', { sec }))),
          sec * 1000,
        )
      }),
    ]).finally(() => {
      if (timer !== undefined) window.clearTimeout(timer)
    })
  }

  /** 多语句执行：逐条顺序执行，收集成功结果集；任一失败即停（部分成功仍展示已得结果） */
  async function runMultiStatements(
    tab: QueryTab,
    sessionId: string,
    stmts: string[],
    pageSize: number,
    seq: number,
    startedAt: number,
  ): Promise<void> {
    const results: QueryResult[] = []
    let firstErr = ''
    let timedOut = false
    let failedAt = 0
    for (let i = 0; i < stmts.length; i++) {
      if (seq !== tab.runSeq) return
      try {
        // 与单语句路径一致的超时保护，防止挂起语句阻塞整批
        results.push(
          await withExecTimeout(
            dbExecute(sessionId, tab.database || null, stmts[i], pageSize),
          ),
        )
      } catch (e) {
        firstErr = String(e)
        timedOut = e instanceof ExecTimeoutError
        failedAt = i + 1
        break
      }
    }
    if (seq !== tab.runSeq) return
    const sqlText = stmts.join(';\n')
    if (results.length === 0) {
      tab.error = firstErr
      tab.errorKind = timedOut ? 'timeout' : undefined
      if (looksLikeConnError(firstErr)) connStore.markLost(tab.connectionId)
      logStore.push({
        level: 'error',
        kind: 'sql',
        title: sqlTitle(sqlText),
        detail: firstErr,
        conn: tab.connectionName,
        db: tab.database,
        durationMs: Date.now() - startedAt,
      })
      historyStore.push({
        connectionName: tab.connectionName,
        kind: tab.kind,
        database: tab.database,
        sql: sqlText,
        durationMs: Date.now() - startedAt,
        ok: false,
      })
      ElMessage.error(tt('query.multiFailAt', { i: failedAt, total: stmts.length, msg: firstErr }))
      logger.error(`[sql] multi failed at #${failedAt}: ${firstErr} | stmts=${stmts.length}`)
      return
    }
    tab.multiResults = results
    tab.activeResultIndex = 0
    tab.result = results[0]
    tab.baseSql = sqlText
    tab.hasMore = false
    if (firstErr) ElMessage.error(tt('query.multiFailAt', { i: failedAt, total: stmts.length, msg: firstErr }))
    logStore.push({
      level: firstErr ? 'warn' : 'info',
      kind: 'sql',
      title: sqlTitle(sqlText),
      detail: firstErr || undefined,
      conn: tab.connectionName,
      db: tab.database,
      durationMs: Date.now() - startedAt,
      rows: results.reduce((n, r) => n + r.rows.length, 0),
    })
    historyStore.push({
      connectionName: tab.connectionName,
      kind: tab.kind,
      database: tab.database,
      sql: sqlText,
      durationMs: Date.now() - startedAt,
      ok: true,
    })
    logger.info(`[sql] multi ok results=${results.length}/${stmts.length}`)
  }

  async function runTab(tab: QueryTab, sqlOverride?: string): Promise<void> {
    const session = connStore.sessionOf(tab.connectionId)
    if (!session) {
      tab.error = tt('query.connectionLost')
      logStore.push({
        level: 'warn',
        kind: 'conn',
        title: tt('query.connectionLost'),
        conn: tab.connectionName,
        db: tab.database,
      })
      return
    }
  const sqlText = sqlOverride ?? tab.sql
  const trimmed = sqlText.trim()
  if (!trimmed) return
  const startedAt = Date.now()
    // 请求序列：新执行/手动停止会使旧请求作废，保证按钮不会永久卡在 loading
    tab.runSeq += 1
    const seq = tab.runSeq
    tab.running = true
    tab.error = ''
    tab.errorKind = undefined
    logger.debug(
      `[sql] session=${session.sessionId} db=${tab.database || '-'}: ${sqlText.slice(0, 200)}`,
    )
    // 首页仅拉取一页（滚动触底后由 loadMore 续拉）
    const pageSize = Math.max(1, Math.min(prefsStore.result.pageSize, prefsStore.result.maxTotalRows))
    // 多语句：非 Oracle 类连接按语句分割（Oracle PL/SQL 块内含分号，跳过分割）
    const stmts =
      tab.kind === 'oracle' || tab.kind === 'oceanbase-oracle'
        ? [sqlText]
        : splitStatements(sqlText)
    if (stmts.length > 1) {
      try {
        await runMultiStatements(tab, session.sessionId, stmts, pageSize, seq, startedAt)
      } finally {
        if (seq === tab.runSeq) tab.running = false
      }
      return
    }
    try {
      let result
      try {
        result = await withExecTimeout(
          dbExecute(session.sessionId, tab.database || null, sqlText, pageSize),
        )
      } catch (firstErr) {
        if (seq !== tab.runSeq) throw firstErr
        // 连接类失败：立即重连一次后重试（不重试 SQL 错误/超时）
        if (!looksLikeConnError(String(firstErr))) throw firstErr
        logger.warn(`[sql] 连接类错误，重连后重试一次: ${String(firstErr)}`)
        const ok = await reconnectTab(tab)
        const retrySession = ok ? connStore.sessionOf(tab.connectionId) : undefined
        if (!retrySession) throw firstErr
        result = await withExecTimeout(
          dbExecute(retrySession.sessionId, tab.database || null, sqlText, pageSize),
        )
      }
      if (seq === tab.runSeq) {
        tab.result = result
        tab.multiResults = undefined
        tab.activeResultIndex = undefined
        tab.baseSql = sqlText
        tab.hasMore = Boolean(result.truncated) && result.rows.length < prefsStore.result.maxTotalRows
        connStore.markAlive(tab.connectionId)
        logStore.push({
          level: 'info',
          kind: 'sql',
          title: sqlTitle(sqlText),
          conn: tab.connectionName,
          db: tab.database,
          durationMs: result.durationMs,
          rows: result.rows.length,
        })
        historyStore.push({
          connectionName: tab.connectionName,
          kind: tab.kind,
          database: tab.database,
          sql: sqlText,
          durationMs: result.durationMs,
          ok: true,
        })
        logger.info(
          `[sql] ok rows=${result.rows.length} affected=${result.affectedRows ?? '-'} ${result.durationMs}ms`,
        )
      }
    } catch (e) {
      if (seq === tab.runSeq) {
        tab.error = String(e)
        tab.errorKind = e instanceof ExecTimeoutError ? 'timeout' : undefined
        if (looksLikeConnError(String(e))) connStore.markLost(tab.connectionId)
        logStore.push({
          level: 'error',
          kind: 'sql',
          title: sqlTitle(sqlText),
          detail: String(e),
          conn: tab.connectionName,
          db: tab.database,
          durationMs: Date.now() - startedAt,
        })
        historyStore.push({
          connectionName: tab.connectionName,
          kind: tab.kind,
          database: tab.database,
          sql: sqlText,
          durationMs: Date.now() - startedAt,
          ok: false,
        })
        logger.error(`[sql] failed: ${String(e)} | sql: ${sqlText.slice(0, 200)}`)
      }
    } finally {
      if (seq === tab.runSeq) tab.running = false
    }
  }

  /** 停止当前执行：作废挂起请求并立即复位按钮状态 */
  function cancelRun(tab: QueryTab): void {
    tab.runSeq += 1
    tab.running = false
  }

  /** 拖拽排序：将页签从 fromId 移动到 toId 之前/之后位置 */
  function moveTab(fromId: string, toIndex: number): void {
    const from = tabs.value.findIndex((t) => t.id === fromId)
    if (from < 0) return
    const [moved] = tabs.value.splice(from, 1)
    const idx = Math.max(0, Math.min(tabs.value.length, toIndex > from ? toIndex - 1 : toIndex))
    tabs.value.splice(idx, 0, moved)
  }

  /** 重连指定连接（关闭旧会话 → 重新打开），用于连接类失败后的一次性恢复 */
  async function reconnectTab(tab: QueryTab): Promise<boolean> {
    try {
      const cfg = connStore.saved.find((c) => c.id === tab.connectionId)
      if (!cfg) return false
      await connStore.closeConnection(tab.connectionId)
      await connStore.openConnection(cfg)
      connStore.markAlive(tab.connectionId)
      return true
    } catch (e) {
      logger.error(`[sql] reconnect failed: ${String(e)}`)
      logStore.push({
        level: 'error',
        kind: 'conn',
        title: tt('logs.reconnectFailed', { name: tab.connectionName }),
        detail: String(e),
        conn: tab.connectionName,
        db: tab.database,
      })
      return false
    }
  }

  /** 任意 SQL 包装为分页查询（与后端方言分页策略对应：Oracle ROWNUM / 其余 LIMIT OFFSET） */
  function wrapPaged(kind: DbKind, sql: string, offset: number, limit: number): string {
    const inner = sql.trim().replace(/;+\s*$/, '')
    if (kind === 'oracle' || kind === 'oceanbase-oracle') {
      return `SELECT * FROM (SELECT t.*, ROWNUM _pdb_rn FROM (${inner}) t WHERE ROWNUM <= ${offset + limit}) WHERE _pdb_rn > ${offset}`
    }
    return `SELECT * FROM (${inner}) _pdb_p LIMIT ${limit} OFFSET ${offset}`
  }

  /** 滚动触底：按页续拉结果集 */
  async function loadMore(tab: QueryTab): Promise<void> {
    if (!tab.hasMore || tab.loadingMore || tab.running) return
    const session = connStore.sessionOf(tab.connectionId)
    const base = tab.baseSql
    if (!session || !base || !tab.result) return
    const prefs = prefsStore.result
    const total = tab.result.rows.length
    if (total >= prefs.maxTotalRows) {
      tab.hasMore = false
      return
    }
    const pageSize = Math.max(1, Math.min(prefs.pageSize, prefs.maxTotalRows - total))
    tab.loadingMore = true
    try {
      const sql = wrapPaged(tab.kind, base, total, pageSize)
      let rows
      try {
        rows = await withExecTimeout(
          dbExecute(session.sessionId, tab.database || null, sql, pageSize),
        )
      } catch (err) {
        if (!looksLikeConnError(String(err))) throw err
        const ok = await reconnectTab(tab)
        const s2 = ok ? connStore.sessionOf(tab.connectionId) : undefined
        if (!s2) throw err
        rows = await withExecTimeout(
          dbExecute(s2.sessionId, tab.database || null, sql, pageSize),
        )
      }
      tab.result.rows.push(...rows.rows)
      const newTotal = tab.result.rows.length
      tab.hasMore = rows.rows.length >= pageSize && newTotal < prefs.maxTotalRows
      logger.debug(`[sql] loadMore +${rows.rows.length} total=${newTotal}`)
    } catch (e) {
      tab.hasMore = false
      logger.error(`[sql] loadMore failed: ${String(e)}`)
      logStore.push({
        level: 'error',
        kind: 'sql',
        title: tt('logs.pagedFailed'),
        detail: String(e),
        conn: tab.connectionName,
        db: tab.database,
      })
    } finally {
      tab.loadingMore = false
    }
  }

  /** 双击表/视图：打开 SQL 编辑器并生成预览 SQL（不执行，等待手动执行） */
  async function openTableQuery(input: {
    connectionId: string
    connectionName: string
    kind: DbKind
    database: string
    table: string
    /** 视图/表区分（页签图标） */
    typeLabel?: string
  }): Promise<QueryTab> {
    let sql = buildSelectSql(input.kind, input.database, input.table)
    const session = connStore.sessionOf(input.connectionId)
    if (session) {
      sql = await dbPreviewRowsSql(
        session.sessionId,
        input.database || null,
        input.table,
        100,
      ).catch(() => sql)
    }
    return openTab({
      connectionId: input.connectionId,
      connectionName: input.connectionName,
      kind: input.kind,
      database: input.database,
      title: input.table,
      sql,
      icon: input.typeLabel === 'VIEW' ? 'view' : 'table',
    })
  }

  /** 打开表查看面板（列/索引/源码三合一，去重：同表已打开则激活） */
  function openTableInspector(input: {
    connectionId: string
    connectionName: string
    kind: DbKind
    database: string
    table: string
  }): QueryTab {
    const existing = tabs.value.find(
      (t) =>
        t.type === 'table' &&
        t.connectionId === input.connectionId &&
        t.database === input.database &&
        t.table === input.table,
    )
    if (existing) {
      activeTabId.value = existing.id
      return existing
    }
    return openTab({
      connectionId: input.connectionId,
      connectionName: input.connectionName,
      kind: input.kind,
      database: input.database,
      title: input.table,
      icon: 'table',
      type: 'table',
      table: input.table,
    })
  }

  /** 打开非表对象定义查看 Tab（type=object，按连接+库+对象名去重） */
  function openObjectInspector(input: {
    connectionId: string
    connectionName: string
    kind: DbKind
    database: string
    name: string
    objType: DbObjectType
  }): QueryTab {
    const existing = tabs.value.find(
      (t) =>
        t.type === 'object' &&
        t.connectionId === input.connectionId &&
        t.database === input.database &&
        t.objectName === input.name,
    )
    if (existing) {
      activeTabId.value = existing.id
      return existing
    }
    return openTab({
      connectionId: input.connectionId,
      connectionName: input.connectionName,
      kind: input.kind,
      database: input.database,
      title: input.name,
      icon: input.objType,
      type: 'object',
      objectName: input.name,
      objType: input.objType,
    })
  }

  /** 打开例程编辑 Tab（type=routine，按连接+库+对象名去重） */
  function openRoutineEditor(input: {
    connectionId: string
    connectionName: string
    kind: DbKind
    database: string
    name: string
    routineType: 'FUNCTION' | 'PROCEDURE'
  }): QueryTab {
    const existing = tabs.value.find(
      (t) =>
        t.type === 'routine' &&
        t.connectionId === input.connectionId &&
        t.database === input.database &&
        t.objectName === input.name,
    )
    if (existing) {
      activeTabId.value = existing.id
      return existing
    }
    const objType: DbObjectType = input.routineType === 'FUNCTION' ? 'function' : 'procedure'
    return openTab({
      connectionId: input.connectionId,
      connectionName: input.connectionName,
      kind: input.kind,
      database: input.database,
      title: input.name,
      icon: objType,
      type: 'routine',
      objectName: input.name,
      objType,
    })
  }

  /** 会话管理 Tab（type=session，按连接去重） */
  function openSessionManager(input: {
    connectionId: string
    connectionName: string
    kind: DbKind
    database: string
  }): QueryTab {
    const existing = tabs.value.find(
      (t) => t.type === 'session' && t.connectionId === input.connectionId,
    )
    if (existing) {
      activeTabId.value = existing.id
      return existing
    }
    return openTab({
      connectionId: input.connectionId,
      connectionName: input.connectionName,
      kind: input.kind,
      database: input.database,
      title: tt('session.tabTitle'),
      icon: 'session',
      type: 'session',
      force: true,
    })
  }

  /** 新建表模板（按方言生成骨架 SQL 到新 Tab） */
  function openCreateTableTemplate(input: {
    connectionId: string
    connectionName: string
    kind: DbKind
    database: string
  }): void {
    const ident =
      input.kind === 'mysql' || input.kind === 'oceanbase'
        ? `\`${input.database}\`.\`new_table\``
        : `"${input.database}"."new_table"`
    const sql =
      input.kind === 'oracle'
        ? `CREATE TABLE "${input.database}"."NEW_TABLE" (\n  ID NUMBER(18) NOT NULL,\n  NAME VARCHAR2(200),\n  CREATED_AT TIMESTAMP DEFAULT SYSTIMESTAMP,\n  CONSTRAINT PK_NEW_TABLE PRIMARY KEY (ID)\n)`
        : `CREATE TABLE ${ident} (\n  id BIGINT NOT NULL,\n  name VARCHAR(200),\n  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,\n  PRIMARY KEY (id)\n);`
    const tab = openTab({
      connectionId: input.connectionId,
      connectionName: input.connectionName,
      kind: input.kind,
      database: input.database,
      title: tt('query.tabCreateTable'),
      icon: 'table',
    })
    tab.sql = sql
    tab.result = null
    tab.error = ''
  }

  return {
    tabs,
    activeTabId,
    activeTab,
    openTab,
    closeTab,
    closeTabsOfConnection,
    runTab,
    cancelRun,
    loadMore,
    moveTab,
    reconnect: reconnectTab,
    openTableQuery,
    openCreateTableTemplate,
    openTableInspector,
    openObjectInspector,
    openRoutineEditor,
    openSessionManager,
    closeOthers,
    closeLeft,
    closeRight,
    closeAll,
    exportSnapshot,
    importSnapshot,
  }
})

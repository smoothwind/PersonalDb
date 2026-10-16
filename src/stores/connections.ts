import { defineStore } from 'pinia'
import { computed, reactive, ref } from 'vue'
import {
  configLoad,
  configSave,
  dbConnect,
  dbDisconnect,
  dbListDatabases,
  groupLoad,
  groupSave,
  secretCopy,
  secretDelete,
  secretStore,
} from '../api/db'
import { logger } from '../utils/logger'
import { describeConnError, connErrorDetail } from '../utils/connError'
import { notifyOnce } from '../utils/notify'
import { i18n } from '../i18n'
import { useLogStore } from './logs'
import type { ConnectionConfig, ConnectionGroup, SessionInfo } from '../types/db'

/// 连接运行时状态
export const useConnectionStore = defineStore('connections', () => {
  const saved = ref<ConnectionConfig[]>([])
  const logStore = useLogStore()
  /** 连接文件夹（分组管理） */
  const groups = ref<ConnectionGroup[]>([])
  /** 已加载标志（避免重复拉取） */
  let loaded = false
  /** connectionId -> SessionInfo（支持多连接并行） */
  const sessions = reactive(new Map<string, SessionInfo>())
  const connectingIds = reactive(new Set<string>())
  /** connectionId -> 库列表缓存 */
  const databaseLists = reactive(new Map<string, string[]>())
  /** connectionId -> 事务模式（auto | manual） */
  const txModes = reactive(new Map<string, 'auto' | 'manual'>())

  const connectedCount = computed(() => sessions.size)

  /** 已断开（连接丢失）的连接 id 集合 */
  const lostIds = reactive(new Set<string>())

  /** 标记连接丢失（去重提示，避免多 Tab 重复提醒；点击通知可重连） */
  function markLost(id: string): void {
    // 已标记过则静默（避免连续失败时通知与日志刷屏）
    if (lostIds.has(id)) return
    lostIds.add(id)
    const name = saved.value.find((c) => c.id === id)?.name ?? id
    logStore.push({ level: 'warn', kind: 'conn', title: i18n.global.t('logs.lost', { name }), conn: name })
    notifyOnce(`conn-lost:${id}`, i18n.global.t('conn.lost', { name }), {
      onClick: () => {
        void reconnectOne(id)
      },
    })
  }

  /** 重连单个丢失的连接 */
  async function reconnectOne(id: string): Promise<void> {
    const cfg = saved.value.find((c) => c.id === id)
    if (!cfg) {
      lostIds.delete(id)
      return
    }
    try {
      await closeConnection(id)
      await openConnection(cfg)
      lostIds.delete(id)
    } catch (e) {
      logger.error(`[conn] reconnect failed (${id}): ${String(e)}`)
      logStore.push({
        level: 'error',
        kind: 'conn',
        title: i18n.global.t('logs.reconnectFailed', { name: cfg.name }),
        detail: String(e),
        conn: cfg.name,
      })
    }
  }

  /** 标记连接恢复 */
  function markAlive(id: string): void {
    lostIds.delete(id)
  }

  /** 重连所有已断开连接 */
  async function reconnectLost(): Promise<void> {
    for (const id of [...lostIds]) {
      const cfg = saved.value.find((c) => c.id === id)
      if (!cfg) {
        lostIds.delete(id)
        continue
      }
      try {
        await closeConnection(id)
        await openConnection(cfg)
        lostIds.delete(id)
      } catch (e) {
        logger.error(`[conn] reconnect lost failed (${id}): ${String(e)}`)
        logStore.push({
          level: 'error',
          kind: 'conn',
          title: i18n.global.t('logs.reconnectFailed', { name: cfg.name }),
          detail: String(e),
          conn: cfg.name,
        })
      }
    }
  }

  function txModeOf(id: string): 'auto' | 'manual' {
    return txModes.get(id) ?? 'auto'
  }

  function setTxMode(id: string, mode: 'auto' | 'manual'): void {
    txModes.set(id, mode)
  }

  async function load(): Promise<void> {
    if (loaded) return
    saved.value = await configLoad()
    groups.value = await groupLoad().catch(() => [])
    loaded = true
  }

  async function persist(): Promise<void> {
    await configSave(saved.value)
  }

  async function upsert(cfg: ConnectionConfig): Promise<void> {
    let next = { ...cfg }
    // 密码安全策略：明文密码一律转入系统凭据管理器，配置文件仅存引用
    if (next.password) {
      try {
        await secretStore(next.id, next.password)
        next = { ...next, password: '', passwordRef: next.id }
      } catch (e) {
        // 凭据不可用时回退明文（保持原行为），并记录告警
        logger.warn(`[conn] credential store failed, fallback plaintext: ${String(e)}`)
        next = { ...next, passwordRef: null }
      }
    } else if (next.passwordRef && next.passwordRef !== next.id) {
      // 复制连接：凭据随新连接复制一份
      try {
        await secretCopy(next.passwordRef, next.id)
        next = { ...next, passwordRef: next.id }
      } catch {
        next = { ...next, passwordRef: null }
      }
    }
    const idx = saved.value.findIndex((c) => c.id === next.id)
    if (idx >= 0) {
      saved.value[idx] = next
      // 配置变化后旧会话作废，下次打开使用新配置
      await closeConnection(next.id)
    } else {
      saved.value.push(next)
    }
    await persist()
  }

  async function remove(id: string): Promise<void> {
    await closeConnection(id)
    saved.value = saved.value.filter((c) => c.id !== id)
    await persist()
    // 凭据清理（失败不影响主流程）
    secretDelete(id).catch(() => {})
  }

  function configOf(id: string): ConnectionConfig | undefined {
    return saved.value.find((c) => c.id === id)
  }

  /** 新建文件夹（parentId 为 null 时建在根级） */
  async function createGroup(name: string, parentId: string | null = null): Promise<ConnectionGroup> {
    const g: ConnectionGroup = { id: crypto.randomUUID(), name, parentId }
    groups.value = [...groups.value, g]
    await groupSave(groups.value)
    return g
  }

  async function renameGroup(id: string, name: string): Promise<void> {
    groups.value = groups.value.map((g) => (g.id === id ? { ...g, name } : g))
    await groupSave(groups.value)
  }

  /** 某父级下的子文件夹（按数组顺序） */
  function groupChildren(parentId: string | null): ConnectionGroup[] {
    return groups.value.filter((g) => (g.parentId ?? null) === parentId)
  }

  /** 含自身的全部后代文件夹 id */
  function descendantGroupIds(id: string): string[] {
    const out: string[] = []
    const stack = [id]
    while (stack.length > 0) {
      const cur = stack.pop()!
      for (const g of groups.value) {
        if ((g.parentId ?? null) === cur) {
          out.push(g.id)
          stack.push(g.id)
        }
      }
    }
    return out
  }

  /** 删除文件夹策略统计（供删除对话框展示） */
  function groupDeleteStats(id: string): { folders: number; connections: number } {
    const ids = new Set([id, ...descendantGroupIds(id)])
    return {
      folders: ids.size,
      connections: saved.value.filter((c) => c.groupId && ids.has(c.groupId)).length,
    }
  }

  type GroupDeleteStrategy = 'promote' | 'flatten' | 'purge'

  /**
   * 删除文件夹（多级策略）：
   * - promote：仅删该节点，直接子级（子文件夹/连接）上移到其父级
   * - flatten：删除该节点与全部子文件夹，所有连接上移到其父级
   * - purge：删除该节点与全部子文件夹，且其中连接一并删除（含凭据清理）
   */
  async function removeGroup(id: string, strategy: GroupDeleteStrategy = 'promote'): Promise<void> {
    const target = groups.value.find((g) => g.id === id)
    if (!target) return
    const parentId = target.parentId ?? null
    const descendants = descendantGroupIds(id)

    if (strategy === 'promote') {
      groups.value = groups.value
        .filter((g) => g.id !== id)
        .map((g) => ((g.parentId ?? null) === id ? { ...g, parentId } : g))
      saved.value = saved.value.map((c) => (c.groupId === id ? { ...c, groupId: parentId } : c))
    } else if (strategy === 'flatten') {
      const ids = new Set([id, ...descendants])
      groups.value = groups.value
        .filter((g) => !ids.has(g.id))
        .map((g) => (ids.has(g.parentId ?? '') ? { ...g, parentId } : g))
      saved.value = saved.value.map((c) =>
        c.groupId && ids.has(c.groupId) ? { ...c, groupId: parentId } : c,
      )
    } else {
      const ids = new Set([id, ...descendants])
      groups.value = groups.value.filter((g) => !ids.has(g.id))
      const doomed = saved.value.filter((c) => c.groupId && ids.has(c.groupId))
      for (const c of doomed) {
        await closeConnection(c.id)
        secretDelete(c.id).catch(() => {})
      }
      saved.value = saved.value.filter((c) => !c.groupId || !ids.has(c.groupId))
    }
    await Promise.all([groupSave(groups.value), persist()])
  }

  /**
   * 文件夹移动/排序：
   * - position='inner'：移动为 target 的子文件夹（target 为 null 时移回根级末尾）
   * - position='before'/'after'：与 target 同级插入（父级随 target）
   * 返回 false 表示被拒绝（循环嵌套）。
   */
  async function moveGroup(
    dragId: string,
    targetId: string | null,
    position: 'inner' | 'before' | 'after',
  ): Promise<boolean> {
    if (dragId === targetId) return false
    const list = [...groups.value]
    const from = list.findIndex((g) => g.id === dragId)
    if (from < 0) return false

    let newParent: string | null
    if (position === 'inner') {
      newParent = targetId
    } else {
      const t = list.find((g) => g.id === targetId)
      if (!t) return false
      newParent = t.parentId ?? null
    }
    // 循环嵌套防护：不得移入自身或后代
    if (newParent && (newParent === dragId || descendantGroupIds(dragId).includes(newParent))) {
      return false
    }

    const [moved] = list.splice(from, 1)
    const updated: ConnectionGroup = { ...moved, parentId: newParent }
    if (position === 'inner') {
      // 追加为该父级的最后一个子项：放在数组中最后一个同父/后代元素的后面
      let insertAt = list.length
      if (targetId) {
        const ids = new Set([targetId, ...descendantGroupIds(targetId)])
        for (let i = list.length - 1; i >= 0; i--) {
          if (ids.has(list[i].id)) {
            insertAt = i + 1
            break
          }
        }
      }
      list.splice(insertAt, 0, updated)
    } else {
      const to = list.findIndex((g) => g.id === targetId)
      list.splice(position === 'before' ? to : to + 1, 0, updated)
    }
    groups.value = list
    await groupSave(groups.value)
    return true
  }

  /** 移动连接到文件夹（groupId 为 null 表示移出分组） */
  async function moveToGroup(connectionId: string, groupId: string | null): Promise<void> {
    saved.value = saved.value.map((c) => (c.id === connectionId ? { ...c, groupId } : c))
    await persist()
  }

  /** 批量合并分组（配置导入用；按 id 去重） */
  async function mergeGroups(incoming: ConnectionGroup[]): Promise<void> {
    const map = new Map(groups.value.map((g) => [g.id, g]))
    for (const g of incoming) {
      map.set(g.id, map.get(g.id) ?? g)
    }
    groups.value = [...map.values()]
    await groupSave(groups.value)
  }

  function sessionOf(id: string): SessionInfo | undefined {
    return sessions.get(id)
  }

  function isConnected(id: string): boolean {
    return sessions.has(id)
  }

  function isConnecting(id: string): boolean {
    return connectingIds.has(id)
  }

  /** 打开连接（幂等：已连接直接复用会话） */
  async function openConnection(cfg: ConnectionConfig): Promise<SessionInfo> {
    const existing = sessions.get(cfg.id)
    if (existing) return existing
    connectingIds.add(cfg.id)
    logger.info(
      `[conn] connecting: ${cfg.kind} ${cfg.username}@${cfg.host}:${cfg.port ?? ''} db=${cfg.database ?? '-'}`,
    )
    try {
      const session = await dbConnect(cfg)
      sessions.set(cfg.id, session)
      lostIds.delete(cfg.id)
      logger.info(`[conn] connected: ${session.serverInfo.serverVersion}`)
      logStore.push({
        level: 'info',
        kind: 'conn',
        title: i18n.global.t('logs.connected', { name: cfg.name }),
        conn: cfg.name,
        db: cfg.database || undefined,
      })
      return session
    } catch (e) {
      logger.error(`[conn] connect failed: ${String(e)}`)
      markLost(cfg.id)
      const brief = describeConnError(e)
      const raw = connErrorDetail(e)
      logStore.push({
        level: 'error',
        kind: 'conn',
        title: i18n.global.t('logs.connectFailed', { name: cfg.name }),
        detail: brief === raw ? raw : `${brief}\n${raw}`,
        conn: cfg.name,
        db: cfg.database || undefined,
      })
      throw new Error(brief)
    } finally {
      connectingIds.delete(cfg.id)
    }
  }

  async function closeConnection(id: string): Promise<void> {
    const session = sessions.get(id)
    if (!session) return
    sessions.delete(id)
    databaseLists.delete(id)
    txModes.delete(id)
    const name = saved.value.find((c) => c.id === id)?.name ?? id
    logStore.push({ level: 'info', kind: 'conn', title: i18n.global.t('logs.disconnected', { name }), conn: name })
    try {
      await dbDisconnect(session.sessionId)
    } catch {
      // 会话可能已失效
    }
  }

  async function listDatabases(id: string, force = false): Promise<string[]> {
    if (!force && databaseLists.has(id)) {
      return databaseLists.get(id) ?? []
    }
    const session = sessions.get(id)
    if (!session) return []
    const dbs = await dbListDatabases(session.sessionId)
    databaseLists.set(id, dbs)
    return dbs
  }

  return {
    saved,
    groups,
    sessions,
    connectingIds,
    databaseLists,
    txModes,
    connectedCount,
    load,
    persist,
    upsert,
    remove,
    configOf,
    createGroup,
    renameGroup,
    removeGroup,
    groupChildren,
    descendantGroupIds,
    groupDeleteStats,
    mergeGroups,
    moveGroup,
    moveToGroup,
    sessionOf,
    isConnected,
    isConnecting,
    lostIds,
    markAlive,
    markLost,
    reconnectOne,
    reconnectLost,
    openConnection,
    closeConnection,
    listDatabases,
    txModeOf,
    setTxMode,
  }
})

import { defineStore } from 'pinia'
import { ref } from 'vue'
import { historyAdd, historyClear, historyList } from '../api/db'

export interface HistoryEntry {
  id: string
  connectionName: string
  kind: string
  database: string
  sql: string
  durationMs: number
  ok: boolean
  /** epoch ms */
  at: number
}

const LEGACY_KEY = 'personaldb.history'
const SQL_MAX = 2000

/** 查询历史（SQLite 持久化，上限 500 条；SQL 截断 2000 字符） */
export const useHistoryStore = defineStore('history', () => {
  const list = ref<HistoryEntry[]>([])
  let loaded = false

  function toEntry(raw: Record<string, unknown>): HistoryEntry {
    return {
      id: String(raw.id ?? crypto.randomUUID()),
      connectionName: String(raw.connectionName ?? ''),
      kind: String(raw.kind ?? ''),
      database: String(raw.database ?? ''),
      sql: String(raw.sql ?? '').slice(0, SQL_MAX),
      durationMs: Number(raw.durationMs ?? 0),
      ok: Boolean(raw.ok),
      at: Number(raw.at ?? Date.now()),
    }
  }

  /** 首次使用时加载；同时把旧 localStorage 数据一次性迁入 SQLite */
  async function ensureLoaded(): Promise<void> {
    if (loaded) return
    loaded = true
    try {
      const legacy = localStorage.getItem(LEGACY_KEY)
      if (legacy) {
        localStorage.removeItem(LEGACY_KEY)
        const parsed = JSON.parse(legacy) as Record<string, unknown>[]
        if (Array.isArray(parsed)) {
          for (const raw of parsed.slice(0, 500)) {
            historyAdd(toEntry(raw)).catch(() => {})
          }
        }
        // 给后台写入留一点时间后再首次拉取
        await new Promise((r) => setTimeout(r, 150))
      }
      list.value = (await historyList('', 500)) as HistoryEntry[]
    } catch {
      list.value = list.value ?? []
    }
  }

  async function search(keyword: string): Promise<void> {
    try {
      list.value = (await historyList(keyword.trim(), 200)) as HistoryEntry[]
    } catch {
      /* 保持现列表 */
    }
  }

  function push(entry: {
    connectionName: string
    kind: string
    database: string
    sql: string
    durationMs: number
    ok: boolean
  }): void {
    const item: HistoryEntry = {
      id: crypto.randomUUID(),
      ...entry,
      sql: entry.sql.slice(0, SQL_MAX),
      at: Date.now(),
    }
    // 乐观插入 + 后台落库
    list.value = [item, ...list.value].slice(0, 500)
    historyAdd(item).catch(() => {})
  }

  async function clear(): Promise<void> {
    list.value = []
    await historyClear().catch(() => {})
  }

  return { list, ensureLoaded, search, push, clear }
})

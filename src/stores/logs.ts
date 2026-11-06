import { defineStore } from 'pinia'
import { ref } from 'vue'
import { i18n } from '../i18n'

export type LogLevel = 'error' | 'warn' | 'info'
export type LogKind = 'sql' | 'conn'

/** 日志面板条目（会话内环形缓冲，不落盘；SQL 历史的持久化由 history store 负责） */
export interface LogEntry {
  id: string
  /** epoch ms */
  at: number
  level: LogLevel
  kind: LogKind
  /** 一行摘要（SQL 首行截断 / 事件文案） */
  title: string
  /** 完整错误信息或完整 SQL（点击行展开查看） */
  detail?: string
  /** 连接名 */
  conn?: string
  /** 库 / schema */
  db?: string
  durationMs?: number
  rows?: number
}

const MAX_ENTRIES = 1000
const TITLE_MAX = 160

/** SQL 摘要：取首个非空行并截断；多语句时加「N 条语句」前缀（i18n） */
export function sqlTitle(sql: string, count?: number): string {
  const line =
    sql
      .split('\n')
      .map((s) => s.trim())
      .find(Boolean) ?? ''
  const base = line.length > TITLE_MAX ? line.slice(0, TITLE_MAX) + '…' : line
  return count && count > 1
    ? i18n.global.t('logs.multiPrefix', { n: count, summary: base })
    : base
}

/** 多语句逐条日志标题：第 {i}/{n} 条 · 摘要（SPEC-LOG-3：每条语句必须独立入栈） */
export function sqlStmtTitle(sql: string, index: number, total: number): string {
  const base = sqlTitle(sql)
  return i18n.global.t('logs.stmtPrefix', { i: index, n: total, summary: base })
}

/** 日志面板数据源：SQL 执行成败 / 连接事件统一入栈 */
export const useLogStore = defineStore('logs', () => {
  const entries = ref<LogEntry[]>([])
  /** 面板关闭期间新增的错误数（打开面板时清零，状态栏徽标用） */
  const unreadErrors = ref(0)

  /** 追加一条日志（底部追加，超上限丢弃最旧；就地变更避免全量拷贝） */
  function push(e: Omit<LogEntry, 'id' | 'at'>): void {
    entries.value.push({ id: crypto.randomUUID(), at: Date.now(), ...e })
    if (entries.value.length > MAX_ENTRIES) {
      entries.value.splice(0, entries.value.length - MAX_ENTRIES)
    }
    if (e.level === 'error') unreadErrors.value++
  }

  /** 打开面板：清除未读错误计数 */
  function markOpened(): void {
    unreadErrors.value = 0
  }

  function clear(): void {
    entries.value = []
    unreadErrors.value = 0
  }

  return { entries, unreadErrors, push, markOpened, clear }
})

import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { DbKind } from '../types/db'
import type { RiskItem, RiskLevel, RiskType } from '../utils/riskySql'

/**
 * 危险操作确认服务（SPEC-SAFE-1）：全局唯一确认弹窗的状态载体。
 *
 * tabs.runTab 在拦截点调用 request() 挂起待确认上下文并 await Promise；
 * DangerConfirmDialog（App.vue 全局挂载）渲染 pending 并经 settle() 结算。
 * 「不再提示」豁免：会话级（内存 Set，重启失效）/ 持久级（localStorage，可清除）。
 */

const EXEMPT_KEY = 'personaldb.dangerExempt'

export interface DangerConfirmContext {
  level: RiskLevel
  type: RiskType
  connectionName: string
  database: string | null
  kind: DbKind
  /** 风险语句清单（仅含命中项） */
  items: RiskItem[]
  /** 是否需要输入对象名确认（danger 且未被事务降级） */
  needTypeConfirm: boolean
  /** 输入匹配目标（首个涉及对象名） */
  confirmTarget: string
  /** 当前连接是否手动事务模式（可回滚提示） */
  txnManual: boolean
  /** 豁免键（connectionId:type 粒度） */
  exemptKey: string
}

function loadPersistent(): string[] {
  try {
    const raw = localStorage.getItem(EXEMPT_KEY)
    const list = raw ? (JSON.parse(raw) as unknown) : []
    return Array.isArray(list) ? list.filter((x): x is string => typeof x === 'string') : []
  } catch {
    return []
  }
}

export const useDangerStore = defineStore('danger', () => {
  const pending = ref<DangerConfirmContext | null>(null)
  const sessionExempt = new Set<string>()
  let resolver: ((ok: boolean) => void) | null = null

  /** 挂起确认并等待用户结算（resolve ok=true 才继续执行） */
  function request(ctx: DangerConfirmContext): Promise<{ ok: boolean }> {
    pending.value = ctx
    return new Promise((resolve) => {
      resolver = (ok: boolean) => {
        pending.value = null
        resolver = null
        resolve({ ok })
      }
    })
  }

  /** 由确认弹窗结算 */
  function settle(ok: boolean): void {
    resolver?.(ok)
  }

  function isExempted(key: string, scope: 'session' | 'persistent'): boolean {
    if (sessionExempt.has(key)) return true
    if (scope === 'persistent') return loadPersistent().includes(key)
    return false
  }

  function addExemption(key: string, scope: 'session' | 'persistent'): void {
    sessionExempt.add(key)
    if (scope === 'persistent') {
      const list = loadPersistent()
      if (!list.includes(key)) {
        list.push(key)
        localStorage.setItem(EXEMPT_KEY, JSON.stringify(list))
      }
    }
  }

  function persistentList(): string[] {
    return loadPersistent()
  }

  function clearPersistent(): void {
    localStorage.removeItem(EXEMPT_KEY)
  }

  return { pending, request, settle, isExempted, addExemption, persistentList, clearPersistent }
})

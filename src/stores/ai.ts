/**
 * AI 能力 store：提供方配置（密钥经 keyring，明文不落盘）+ 流式运行态 + 侧栏开关。
 *
 * 约定对齐：
 * - 密钥走 secretStore(`ai:<id>`, key)——与连接密码同一 keyring 体系，配置 JSON 只存 passwordRef；
 * - 流式事件按 reqId 过期丢弃（runSeq 同款语义）；停止 = aiCancel 作废，不依赖 Promise 复位；
 * - 配置持久化 localStorage `personaldb.ai`（不含密钥），改动即时生效。
 */

import { computed, reactive, ref } from 'vue'
import { defineStore } from 'pinia'
import { i18n } from '../i18n'
import { aiCancel, aiChatStream, aiTestConnection, type AiCallCfg } from '../api/ai'
import { secretDelete, secretStore } from '../api/db'
import type { AiMsg, AiPrefs, AiProviderCfg } from '../types/ai'

const AI_KEY = 'personaldb.ai'

const t = i18n.global.t

function defaultPrefs(): AiPrefs {
  return { providers: [], activeId: '', contextEnabled: true, insightEnabled: false }
}

function loadPrefs(): AiPrefs {
  try {
    const raw = localStorage.getItem(AI_KEY)
    if (!raw) return defaultPrefs()
    return { ...defaultPrefs(), ...(JSON.parse(raw) as Partial<AiPrefs>) }
  } catch {
    return defaultPrefs()
  }
}

/** 提供方 → 后端调用配置（keyRef 即 keyring 引用，明文密钥不过 IPC） */
function callCfgOf(p: AiProviderCfg): AiCallCfg {
  return {
    baseUrl: p.baseURL,
    model: p.model,
    keyRef: p.passwordRef ?? null,
    temperature: p.temperature ?? null,
    maxTokens: p.maxTokens ?? null,
    timeoutSecs: p.timeoutSecs ?? null,
  }
}

export const useAiStore = defineStore('ai', () => {
  const prefs = reactive<AiPrefs>(loadPrefs())

  function persist(): void {
    localStorage.setItem(AI_KEY, JSON.stringify({ ...prefs }))
  }

  const active = computed<AiProviderCfg | null>(
    () => prefs.providers.find((p) => p.id === prefs.activeId) ?? null,
  )

  // ================= 提供方 CRUD =================

  /**
   * 新增/更新提供方。apiKey 语义：
   * - 非空：写入 keyring（`ai:<id>`）并置 passwordRef；
   * - 空串且原来有引用：清除凭据并转匿名端点；
   * - undefined：不动凭据（仅改其他字段）。
   */
  async function upsertProvider(p: AiProviderCfg, apiKey?: string): Promise<void> {
    if (apiKey !== undefined) {
      const ref = `ai:${p.id}`
      if (apiKey) {
        await secretStore(ref, apiKey)
        p.passwordRef = ref
      } else {
        await secretDelete(ref).catch(() => undefined)
        p.passwordRef = null
      }
    }
    const idx = prefs.providers.findIndex((x) => x.id === p.id)
    if (idx >= 0) prefs.providers[idx] = p
    else prefs.providers.push(p)
    if (!prefs.activeId) prefs.activeId = p.id
    persist()
  }

  async function removeProvider(id: string): Promise<void> {
    await secretDelete(`ai:${id}`).catch(() => undefined)
    prefs.providers = prefs.providers.filter((p) => p.id !== id)
    if (prefs.activeId === id) prefs.activeId = prefs.providers[0]?.id ?? ''
    persist()
  }

  function setActive(id: string): void {
    prefs.activeId = id
    persist()
  }

  function updateFlags(patch: Partial<Pick<AiPrefs, 'contextEnabled' | 'insightEnabled'>>): void {
    Object.assign(prefs, patch)
    persist()
  }

  /** 设置页「测试连接」：错误向上抛（调用方 toast） */
  async function testConnection(p: AiProviderCfg): Promise<string> {
    return aiTestConnection(callCfgOf(p))
  }

  // ================= 流式运行态 =================

  const running = ref(false)
  /** 当前请求 id（过期事件守卫 + 取消句柄） */
  let currentReqId = ''

  /**
   * 发起流式请求。onToken 逐 token 回调；结束/出错后 running 复位。
   * 未配置提供方时抛错（文案 i18n）。
   */
  async function stream(
    messages: AiMsg[],
    onToken: (delta: string) => void,
    onDone: (finish: string | null) => void,
    onError: (message: string) => void,
  ): Promise<void> {
    const p = active.value
    if (!p) throw new Error(t('ai.noProvider'))
    const reqId = `ai-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`
    currentReqId = reqId
    running.value = true
    try {
      await aiChatStream(reqId, callCfgOf(p), messages, (e) => {
        if (reqId !== currentReqId) return // 过期响应丢弃（停止/新请求后）
        if (e.type === 'token') onToken(e.delta)
        else if (e.type === 'done') {
          running.value = false
          onDone(e.finish)
        } else {
          running.value = false
          onError(e.message)
        }
      })
    } catch (e) {
      if (reqId === currentReqId) {
        running.value = false
        onError(e instanceof Error ? e.message : String(e))
      }
    } finally {
      if (reqId === currentReqId) running.value = false
    }
  }

  /** 停止：作废挂起流（事件守卫同时生效，迟到的 token 不再上屏） */
  function cancel(): void {
    const id = currentReqId
    currentReqId = ''
    running.value = false
    if (id) void aiCancel(id)
  }

  // ================= 侧栏开关（按查询页签） =================

  const sidebarOpen = reactive<Record<string, boolean>>({})

  function toggleSidebar(tabId: string): void {
    sidebarOpen[tabId] = !sidebarOpen[tabId]
  }

  function setSidebar(tabId: string, open: boolean): void {
    sidebarOpen[tabId] = open
  }

  function sidebarOf(tabId: string): boolean {
    return !!sidebarOpen[tabId]
  }

  // ================= 多轮对话（ChatPane；localStorage 持久化） =================

  const CHAT_KEY = 'personaldb.ai.chat'
  const CHAT_MAX_MSGS = 200

  function loadChat(): AiMsg[] {
    try {
      const raw = localStorage.getItem(CHAT_KEY)
      return raw ? (JSON.parse(raw) as AiMsg[]) : []
    } catch {
      return []
    }
  }

  const chatMsgs = reactive<AiMsg[]>(loadChat())

  function persistChat(): void {
    localStorage.setItem(CHAT_KEY, JSON.stringify(chatMsgs.slice(-CHAT_MAX_MSGS)))
  }

  function pushChat(msg: AiMsg): void {
    chatMsgs.push(msg)
    if (chatMsgs.length > CHAT_MAX_MSGS) chatMsgs.splice(0, chatMsgs.length - CHAT_MAX_MSGS)
    persistChat()
  }

  function clearChat(): void {
    chatMsgs.splice(0, chatMsgs.length)
    persistChat()
  }

  return {
    prefs,
    active,
    running,
    sidebarOpen,
    upsertProvider,
    removeProvider,
    setActive,
    updateFlags,
    testConnection,
    stream,
    cancel,
    toggleSidebar,
    setSidebar,
    sidebarOf,
    chatMsgs,
    pushChat,
    clearChat,
  }
})

/** AI 能力 Tauri 命令封装（与 src-tauri/src/ai 模块对齐） */

import { invoke, Channel } from '@tauri-apps/api/core'
import type { AiMsg, AiStreamEvent } from '../types/ai'

/** 与后端 AiCallConfig（serde camelCase）对齐 */
export interface AiCallCfg {
  baseUrl: string
  model: string
  keyRef?: string | null
  temperature?: number | null
  maxTokens?: number | null
  timeoutSecs?: number | null
}

/**
 * 流式对话：立即返回，事件经 Channel 推送（token/done/error）。
 * 前端以 reqId 作过期守卫（runSeq 同款语义），配合 aiCancel 作废挂起流。
 */
export function aiChatStream(
  reqId: string,
  cfg: AiCallCfg,
  messages: AiMsg[],
  onEvent: (e: AiStreamEvent) => void,
): Promise<void> {
  const channel = new Channel<AiStreamEvent>()
  channel.onmessage = onEvent
  return invoke('ai_chat_stream', { reqId, cfg, messages, onEvent: channel })
}

export const aiCancel = (reqId: string) => invoke<boolean>('ai_cancel', { reqId })

/** 设置页「测试连接」：返回模型回显摘要 */
export const aiTestConnection = (cfg: AiCallCfg) => invoke<string>('ai_test_connection', { cfg })

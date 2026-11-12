/** AI 能力类型（与 Rust src/ai 模块 serde camelCase 对齐） */

/** 单个 AI 提供方配置（key 明文永不在前端持久化——经 secretStore 入 keyring） */
export interface AiProviderCfg {
  id: string
  name: string
  baseURL: string
  model: string
  /** keyring 引用（`ai:<id>`）；null = 匿名端点（Ollama 本地等） */
  passwordRef?: string | null
  temperature?: number | null
  maxTokens?: number | null
  /** 整体超时秒数（含流式读取） */
  timeoutSecs?: number | null
}

/** AI 首选项（localStorage `personaldb.ai`，不含密钥） */
export interface AiPrefs {
  providers: AiProviderCfg[]
  activeId: string
  /** NL2SQL 注入表结构上下文（隐私开关，默认开） */
  contextEnabled: boolean
  /** 数据洞察采样外发（隐私红线，默认关） */
  insightEnabled: boolean
}

/** 后端流式事件（tag = type） */
export type AiStreamEvent =
  | { type: 'token'; delta: string }
  | { type: 'done'; finish: string | null }
  | { type: 'error'; message: string }

/** 侧栏功能模式 */
export type AiMode = 'nl2sql' | 'explain' | 'optimize' | 'diagnose' | 'insight' | 'chat'

/** 对话消息（前端组装；role: system/user/assistant） */
export interface AiMsg {
  role: string
  content: string
}

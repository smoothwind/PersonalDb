/**
 * AI 提供方内置预设（OpenAI 兼容端点）。
 * 文案存 i18n key（内置数据不落中文原文——i18n 约定）；
 * key 仍由用户填写，预设只预填 baseURL 与默认模型。
 */

import type { AiProviderCfg } from '../types/ai'

export interface AiPreset {
  key: string
  /** i18n key：aiPresets.<key> */
  labelKey: string
  baseURL: string
  defaultModel: string
  /** 免密端点（本地推理服务） */
  anonymous?: boolean
}

export const AI_PRESETS: AiPreset[] = [
  {
    key: 'deepseek',
    labelKey: 'aiPresets.deepseek',
    baseURL: 'https://api.deepseek.com/v1',
    defaultModel: 'deepseek-chat',
  },
  {
    key: 'moonshot',
    labelKey: 'aiPresets.moonshot',
    baseURL: 'https://api.moonshot.cn/v1',
    defaultModel: 'moonshot-v1-8k',
  },
  {
    key: 'qwen',
    labelKey: 'aiPresets.qwen',
    baseURL: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
    defaultModel: 'qwen-plus',
  },
  {
    key: 'glm',
    labelKey: 'aiPresets.glm',
    baseURL: 'https://open.bigmodel.cn/api/paas/v4',
    defaultModel: 'glm-4-flash',
  },
  {
    key: 'ollama',
    labelKey: 'aiPresets.ollama',
    baseURL: 'http://127.0.0.1:11434/v1',
    defaultModel: 'qwen2.5-coder:7b',
    anonymous: true,
  },
]

/** 由预设新建提供方（keyring 引用按 `ai:<id>` 预留） */
export function providerFromPreset(preset: AiPreset): AiProviderCfg {
  const id = `ai-${preset.key}-${Date.now().toString(36)}`
  return {
    id,
    name: preset.key,
    baseURL: preset.baseURL,
    model: preset.defaultModel,
    passwordRef: preset.anonymous ? null : `ai:${id}`,
    temperature: null,
    maxTokens: null,
    timeoutSecs: null,
  }
}

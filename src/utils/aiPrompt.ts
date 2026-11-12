/**
 * AI 请求消息组装（哑管道后端的另一半：Prompt 面在这里）。
 * 系统提示词走 i18n（随语言热切换）；schema 上下文压缩有硬上限，
 * 超限截断打标——防止大库把请求撑爆。
 */

import { i18n } from '../i18n'
import type { AiMsg } from '../types/ai'
import type { DbKind, SchemaSnapshot } from '../types/db'

const t = i18n.global.t

/** schema 上下文硬上限（字符）——超过截断打标 */
const SCHEMA_MAX_CHARS = 8000

export const AI_MODES = ['nl2sql', 'explain', 'optimize', 'diagnose', 'insight', 'chat'] as const

function dialectOf(kind: DbKind): string {
  return t(`dbKind.${kind}`)
}

/**
 * 压缩 schema 快照为提示用文本。tables 为 null = 全部（仍受 maxChars 截断）。
 * 形如：`TABLE user (用户表)\n  id BIGINT 主键\n  name VARCHAR 姓名`
 */
export function compressSchema(
  snap: SchemaSnapshot,
  tables: string[] | null,
  maxChars = SCHEMA_MAX_CHARS,
): string {
  const lines: string[] = []
  const wanted = tables === null ? null : new Set(tables)
  for (const tb of snap.tables) {
    if (wanted && !wanted.has(tb.name)) continue
    const label = tb.kind === 'VIEW' ? 'VIEW' : 'TABLE'
    let head = `${label} ${tb.name}`
    if (tb.comment) head += ` (${tb.comment})`
    lines.push(head)
    for (const col of tb.columns) {
      let cl = `  ${col.name} ${col.dataType}`
      if (col.comment) cl += ` ${col.comment}`
      lines.push(cl)
    }
  }
  let text = lines.join('\n')
  if (text.length > maxChars) {
    text = text.slice(0, maxChars) + `\n…[${t('ai.schemaTruncated')}]`
  }
  return text
}

/** NL2SQL：需求 → 可执行 SQL（只输出 SQL 本身，禁解释禁围栏） */
export function buildNl2sql(kind: DbKind, schemaCtx: string, requirement: string): AiMsg[] {
  const dialect = dialectOf(kind)
  const sys = [t('ai.promptNl2sql', { dialect })]
  if (schemaCtx) sys.push(t('ai.promptSchemaHeader'), schemaCtx)
  return [
    { role: 'system', content: sys.join('\n') },
    { role: 'user', content: requirement },
  ]
}

/** 解释选中 SQL：逐子句语义 + 风险 */
export function buildExplain(kind: DbKind, sql: string): AiMsg[] {
  const dialect = dialectOf(kind)
  return [
    { role: 'system', content: t('ai.promptExplain', { dialect }) },
    { role: 'user', content: sql },
  ]
}

/** 优化建议：索引/改写/计划关注点 */
export function buildOptimize(kind: DbKind, sql: string): AiMsg[] {
  const dialect = dialectOf(kind)
  return [
    { role: 'system', content: t('ai.promptOptimize', { dialect }) },
    { role: 'user', content: sql },
  ]
}

/** 错误诊断：报错摘要 + 原始文本前 500 字符 */
export function buildDiagnose(kind: DbKind, summary: string, detail: string): AiMsg[] {
  const dialect = dialectOf(kind)
  const clipped = detail.length > 500 ? detail.slice(0, 500) + '…' : detail
  return [
    { role: 'system', content: t('ai.promptDiagnose', { dialect }) },
    { role: 'user', content: `${summary}\n\n${clipped}` },
  ]
}

/** 数据洞察：采样行 JSON（隐私红线：调用方须确认 insightEnabled） */
export function buildInsight(kind: DbKind, columns: string[], sampleRows: unknown[]): AiMsg[] {
  const dialect = dialectOf(kind)
  const payload = JSON.stringify({ columns, rows: sampleRows })
  return [
    { role: 'system', content: t('ai.promptInsight', { dialect }) },
    { role: 'user', content: payload },
  ]
}

/** 通用对话（ChatPane 多轮） */
export function buildChat(kind: DbKind, history: AiMsg[], input: string): AiMsg[] {
  const dialect = dialectOf(kind)
  return [
    { role: 'system', content: t('ai.promptChat', { dialect }) },
    ...history,
    { role: 'user', content: input },
  ]
}

/** 产出后处理：剥掉 markdown 代码围栏（模型不听话时兜底） */
export function stripSqlFences(text: string): string {
  const m = text.match(/```(?:sql|\w+)?\s*\n([\s\S]*?)```/i)
  const inner = m ? m[1] : text
  return inner.trim()
}

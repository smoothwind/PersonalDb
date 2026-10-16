import { defineStore } from 'pinia'
import { ref } from 'vue'
import { snippetLoad, snippetSave } from '../api/db'
import { defaultSnippets } from '../utils/snippetDefaults'
import type { DbKind, SqlSnippet } from '../types/db'

export interface SnippetPlaceholder {
  start: number
  end: number
  text: string
}

export interface SnippetExpansion {
  /** 展开后的完整编辑器文本 */
  text: string
  /** 展开文本在新文本中的区间 [abbrevStart, insertedEnd) */
  abbrevStart: number
  insertedEnd: number
  /** 占位符在展开后文本中的区间（按序号排列） */
  placeholders: SnippetPlaceholder[]
}

/** 占位符语法：${1:默认文本} */
const PLACEHOLDER_RE = /\$\{(\d+):([^}]*)\}/g

/** 从模板解析出占位符与展开文本 */
function parseTemplate(template: string): {
  text: string
  placeholders: Array<{ order: number; start: number; end: number; text: string }>
} {
  const placeholders: Array<{ order: number; start: number; end: number; text: string }> = []
  let text = ''
  let last = 0
  for (const m of template.matchAll(PLACEHOLDER_RE)) {
    const idx = m.index ?? 0
    text += template.slice(last, idx)
    const start = text.length
    const value = m[2]
    text += value
    placeholders.push({ order: Number(m[1]), start, end: text.length, text: value })
    last = idx + m[0].length
  }
  text += template.slice(last)
  placeholders.sort((a, b) => a.order - b.order)
  return { text, placeholders }
}

export const useSnippetStore = defineStore('snippets', () => {
  /** 自定义条目（覆盖同名内置） */
  const custom = ref<SqlSnippet[]>([])

  /** 合并内置与自定义（自定义同名覆盖），每个 Tab 按其方言取一次 */
  function mergedFor(kind: DbKind): Map<string, SqlSnippet> {
    const map = new Map<string, SqlSnippet>()
    for (const s of defaultSnippets(kind)) map.set(s.abbrev.toLowerCase(), s)
    for (const s of custom.value) map.set(s.abbrev.toLowerCase(), s)
    return map
  }

  async function load(): Promise<void> {
    custom.value = await snippetLoad().catch(() => [])
  }

  async function persist(list: SqlSnippet[]): Promise<void> {
    custom.value = list
    await snippetSave(list)
  }

  /**
   * 尝试在光标处展开快捷指令：匹配光标左侧连续单词，命中返回展开结果。
   */
  function expandAt(
    text: string,
    cursor: number,
    snippets: Map<string, SqlSnippet>,
  ): SnippetExpansion | null {
    if (cursor <= 0 || cursor > text.length) return null
    let start = cursor
    while (start > 0 && /[A-Za-z0-9_]/.test(text[start - 1])) start -= 1
    if (start === cursor) return null
    const word = text.slice(start, cursor)
    const hit = snippets.get(word.toLowerCase())
    if (!hit) return null
    const parsed = parseTemplate(hit.template)
    const newText = text.slice(0, start) + parsed.text + text.slice(cursor)
    return {
      text: newText,
      abbrevStart: start,
      insertedEnd: start + parsed.text.length,
      placeholders: parsed.placeholders.map((p) => ({
        start: p.start + start,
        end: p.end + start,
        text: p.text,
      })),
    }
  }

  return { custom, mergedFor, load, persist, expandAt }
})

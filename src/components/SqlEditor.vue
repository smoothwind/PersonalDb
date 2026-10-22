<template>
  <div
    ref="hostEl"
    class="sql-editor-host"
    :class="{ 'is-fill': fill }"
    :style="{ '--editor-max-height': maxHeight }"
    @wheel.ctrl="onWheelZoom"
  />
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Compartment, EditorSelection, EditorState, type Extension } from '@codemirror/state'
import {
  EditorView,
  drawSelection,
  dropCursor,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
  placeholder as cmPlaceholder,
  rectangularSelection,
} from '@codemirror/view'
import { defaultKeymap, history, historyKeymap, indentMore } from '@codemirror/commands'
import { highlightSelectionMatches, searchKeymap } from '@codemirror/search'
import { showMinimap } from '@replit/codemirror-minimap'
import {
  bracketMatching,
  defaultHighlightStyle,
  foldGutter,
  indentOnInput,
  indentUnit,
  syntaxHighlighting,
} from '@codemirror/language'
import { closeBrackets, closeBracketsKeymap, completionKeymap, autocompletion } from '@codemirror/autocomplete'
import type { Completion, CompletionContext, CompletionResult } from '@codemirror/autocomplete'
import { MySQL, PLSQL, PostgreSQL, StandardSQL, sql } from '@codemirror/lang-sql'
import { oneDark } from '@codemirror/theme-one-dark'
import { useI18n } from 'vue-i18n'
import { usePreferencesStore } from '../stores/preferences'
import { useSnippetStore, type SnippetPlaceholder } from '../stores/snippets'
import { useConnectionStore } from '../stores/connections'
import { toCodeMirrorKey } from '../utils/shortcuts'
import { compressSql } from '../utils/sqlFormat'
import { convertOutsideLiterals } from '../utils/sqlCase'
import { dbSchemaSnapshot } from '../api/db'
import { logger } from '../utils/logger'
import type { DbKind, SchemaSnapshot, SqlSnippet, TableMeta } from '../types/db'

const props = withDefaults(
  defineProps<{
    modelValue: string
    kind: DbKind
    /** 当前方言的快捷指令表（展开用） */
    snippets: Map<string, SqlSnippet>
    /** 最大高度（CSS 值），默认 32vh */
    maxHeight?: string
    /** 撑满父容器（抽屉/分栏场景） */
    fill?: boolean
    /** 自动补全元数据来源：连接 + 库 */
    connectionId?: string
    database?: string
    /** 只读（默认可编辑） */
    readonly?: boolean
  }>(),
  { maxHeight: '32vh', fill: false, connectionId: '', database: '', readonly: false },
)

const emit = defineEmits<{
  (e: 'update:modelValue', v: string): void
  (e: 'run', mode: 'all' | 'selection' | 'line' | 'auto'): void
  (e: 'format'): void
}>()

const { t, locale } = useI18n()
const prefs = usePreferencesStore()
const snippetStore = useSnippetStore()
const connStore = useConnectionStore()

const hostEl = ref<HTMLDivElement>()
let view: EditorView | null = null

/** 动态配置隔间（主题/方言/换行/字号/行号/缩进可热切换） */
const dialectComp = new Compartment()
const themeComp = new Compartment()
const wrapComp = new Compartment()
const fontComp = new Compartment()
const gutterComp = new Compartment()
const indentComp = new Compartment()
/** 自定义快捷键（设置面板可改，动态重配） */
const keymapComp = new Compartment()
/** 只读开关 */
const readonlyComp = new Compartment()
/** CodeMirror 内置面板文案（查找/替换/转到行/折叠）随语言热切换 */
const phraseComp = new Compartment()
/** 代码缩略图开关 */
const minimapComp = new Compartment()

function readonlyExt(ro: boolean): Extension {
  return ro ? [EditorState.readOnly.of(true), EditorView.editable.of(false)] : []
}

/** CM6 内置 UI 短语本地化（键为 CM 源文案，逐字匹配） */
function phrasesExt(): Extension {
  return EditorState.phrases.of({
    Find: t('cm.find'),
    Replace: t('cm.replace'),
    next: t('cm.next'),
    previous: t('cm.previous'),
    all: t('cm.all'),
    'Replace all': t('cm.replaceAll'),
    close: t('cm.close'),
    'current match': t('cm.currentMatch'),
    'replaced $ matches': t('cm.replacedMatches'),
    'replaced match on line $': t('cm.replacedMatchOnLine'),
    'on line': t('cm.onLine'),
    'Go to line': t('cm.gotoLine'),
    go: t('cm.go'),
    'Folded lines': t('cm.foldedLines'),
    'Unfolded line': t('cm.unfoldedLine'),
  })
}

/** 缩略图（VSCode 式缩放文本；短文档时 minimap 反而碍事 → 由偏好控制） */
function minimapExt(): Extension {
  if (!prefs.editor.minimap) return []
  return showMinimap.compute(['doc'], () => ({
    create: () => {
      const dom = document.createElement('div')
      dom.className = 'cm-minimap'
      return { dom }
    },
    displayText: 'characters',
    showOverlay: 'always',
  }))
}

/** 展开的占位符状态 */
let activePlaceholders: SnippetPlaceholder[] = []
let placeholderIdx = -1

function dialectOf(kind: DbKind) {
  switch (kind) {
    case 'postgresql':
      return PostgreSQL
    case 'sqlite':
      return StandardSQL
    case 'oracle':
    case 'oceanbase-oracle':
      return PLSQL
    default:
      return MySQL
  }
}

/** 方言扩展（关键字大小写由偏好控制） */
function dialectExt(kind = props.kind) {
  return sql({ dialect: dialectOf(kind), upperCaseKeywords: prefs.editor.upperCaseKeywords })
}

function themeOf(dark: boolean) {
  return dark
    ? oneDark
    : [syntaxHighlighting(defaultHighlightStyle, { fallback: true })]
}

function fontTheme(size: number) {
  return EditorView.theme({
    '&': { fontSize: `${size}px` },
    '.cm-content': {
      fontFamily: "Consolas, 'Cascadia Mono', 'Courier New', monospace",
      padding: '8px 0',
    },
    '.cm-scroller': { overflow: 'auto' },
  })
}

/** Ctrl+滚轮缩放字号（10–24px，写回首选项持久化并全局生效） */
function onWheelZoom(ev: WheelEvent): void {
  ev.preventDefault()
  const cur = prefs.editor.fontSize
  const next = Math.min(24, Math.max(10, cur + (ev.deltaY < 0 ? 1 : -1)))
  if (next !== cur) prefs.updateEditor({ fontSize: next })
}

function gutterExt(show: boolean) {
  return show ? [lineNumbers(), highlightActiveLineGutter()] : []
}

/** 快捷指令展开：光标前缀命中缩写则替换并选中第一个占位符 */
function tryExpand(v: EditorView): boolean {
  const sel = v.state.selection.main
  if (!sel.empty) return false
  const text = v.state.doc.toString()
  const cursor = sel.head
  const exp = snippetStore.expandAt(text, cursor, props.snippets)
  if (!exp) return false
  const inserted = exp.text.slice(exp.abbrevStart, exp.insertedEnd)
  const placeholders = exp.placeholders
  v.dispatch({
    changes: { from: exp.abbrevStart, to: cursor, insert: inserted },
    selection: placeholders.length
      ? EditorSelection.range(placeholders[0].start, placeholders[0].end)
      : EditorSelection.cursor(exp.insertedEnd),
    scrollIntoView: true,
  })
  activePlaceholders = placeholders
  placeholderIdx = placeholders.length > 0 ? 0 : -1
  return true
}

function jumpPlaceholder(v: EditorView): boolean {
  if (placeholderIdx < 0 || activePlaceholders.length === 0) return false
  const sel = v.state.selection.main
  const cur = activePlaceholders[placeholderIdx]
  if (sel.head < cur.start || sel.head > cur.end) {
    clearPlaceholders()
    return false
  }
  const next = placeholderIdx + 1
  if (next >= activePlaceholders.length) {
    const last = activePlaceholders[activePlaceholders.length - 1]
    v.dispatch({ selection: EditorSelection.cursor(last.end), scrollIntoView: true })
    clearPlaceholders()
    return true
  }
  placeholderIdx = next
  const p = activePlaceholders[next]
  v.dispatch({ selection: EditorSelection.range(p.start, p.end), scrollIntoView: true })
  return true
}

function clearPlaceholders(): void {
  activePlaceholders = []
  placeholderIdx = -1
}

function transformCase(v: EditorView, mode: 'upper' | 'lower'): boolean {
  const ranges = v.state.selection.ranges.map((r) => {
    if (r.empty) {
      const word = v.state.wordAt(r.head)
      return word ?? r
    }
    return r
  })
  if (ranges.every((r) => r.empty)) return false
  v.dispatch(
    v.state.changeByRange((range) => {
      const matched = ranges.find((r) => r.from === range.from && r.to === range.to) ?? range
      const text = v.state.doc.sliceString(matched.from, matched.to)
      const converted = convertOutsideLiterals(text, mode)
      return {
        changes: { from: matched.from, to: matched.to, insert: converted },
        range: EditorSelection.range(matched.from, matched.from + converted.length),
      }
    }),
  )
  return true
}

/** 按方言压缩 SQL（去注释/多余空白，字面值保护） */
function compressCode(v: EditorView): boolean {
  const cur = v.state.doc.toString()
  if (!cur.trim()) return true
  const compressed = compressSql(cur)
  v.dispatch({ changes: { from: 0, to: cur.length, insert: compressed } })
  return true
}

/** 自定义快捷键（组合来自偏好设置，可热更新） */
function customKeymap() {
  const k = (action: string, fallback: string) =>
    toCodeMirrorKey(prefs.shortcutOf(action) || fallback)
  return keymap.of([
    { key: k('run', 'Ctrl+Enter'), run: () => { emit('run', 'auto'); return true } },
    { key: k('runSelection', 'Ctrl+Shift+Enter'), run: () => { emit('run', 'selection'); return true } },
    { key: k('runLine', 'Ctrl+Alt+Enter'), run: () => { emit('run', 'line'); return true } },
    { key: k('format', 'Ctrl+Shift+F'), run: () => { emit('format'); return true } },
    { key: k('compress', 'Ctrl+Alt+K'), run: (v) => compressCode(v) },
    { key: k('upper', 'Ctrl+U'), run: (v) => transformCase(v, 'upper') },
    { key: k('lower', 'Ctrl+Shift+U'), run: (v) => transformCase(v, 'lower') },
    {
      key: 'Tab',
      run: (v) => {
        if (jumpPlaceholder(v)) return true
        if (tryExpand(v)) return true
        return indentMore(v)
      },
    },
    {
      key: 'Space',
      run: (v) => tryExpand(v), // 命中缩写才拦截，否则默认插入空格
    },
    {
      key: 'Escape',
      run: () => {
        clearPlaceholders()
        return false
      },
    },
  ])
}

/** ===== 自动补全：来源于同 schema 元数据（表/列 + 注释）与当前 SQL ===== */
let snapshot: SchemaSnapshot | null = null
let snapKey = ''
let pendingKey = ''
let snapTimer: number | undefined

/** 依据当前 SQL 与工具栏库，解析需要抓取快照的 schema 集合 */
function schemasOf(docText: string): string[] {
  const set = new Set<string>()
  if (props.database) set.add(props.database)
  const re = /\b(?:from|join)\s+([`"[\].\w$]+)/gi
  for (const m of docText.matchAll(re)) {
    const raw = m[1].replace(/[`"[\]]/g, '')
    const parts = raw.split('.')
    if (parts.length >= 2) set.add(parts[parts.length - 2])
  }
  return [...set].sort()
}

/** 抓取快照：合并各 schema 的表（失败可重试）。注意：需用「会话 ID」而非连接 ID */
async function loadSnapshot(docText = props.modelValue): Promise<void> {
  if (!props.connectionId) return
  const sessionId = connStore.sessionOf(props.connectionId)?.sessionId
  if (!sessionId) return // 会话未建立：稍后（聚焦/输入/连接就绪）再试
  const schemas = schemasOf(docText)
  const key = `${sessionId}::${schemas.join(',') || '(default)'}`
  if (key === snapKey || key === pendingKey) return
  pendingKey = key
  try {
    const merged: TableMeta[] = []
    const seen = new Set<string>()
    const targets = schemas.length > 0 ? schemas : ['']
    for (const s of targets) {
      const sn = await dbSchemaSnapshot(sessionId, s || null)
      for (const t of sn.tables) {
        const k = t.name.toLowerCase()
        if (!seen.has(k)) {
          seen.add(k)
          merged.push(t)
        }
      }
    }
    if (pendingKey === key) {
      snapshot = { tables: merged }
      snapKey = key
    }
  } catch (e) {
    // 失败（如会话未建立）不置 snapKey，便于后续重试
    logger.warn(`[sql] schema snapshot failed: ${String(e)}`)
  } finally {
    if (pendingKey === key) pendingKey = ''
  }
}

/** 文档变化时的去抖重载 */
function scheduleSnapshot(docText: string): void {
  window.clearTimeout(snapTimer)
  snapTimer = window.setTimeout(() => void loadSnapshot(docText), 400)
}

/** 依据列类型名推断补全图标类别 */
function colIconType(dataType: string): string {
  const s = (dataType || '').toUpperCase()
  if (/JSON/.test(s)) return 'col-json'
  if (/(BOOL)/.test(s)) return 'col-bool'
  if (/(BLOB|BINARY|RAW|BFILE|BYTEA)/.test(s)) return 'col-bin'
  if (/(DATE|TIME)/.test(s)) return 'col-date'
  if (/(NUMBER|INT|DECIMAL|NUMERIC|FLOAT|DOUBLE|REAL|LONG|MONEY)/.test(s)) return 'col-num'
  return 'col-text'
}

const completionSource = (ctx: CompletionContext): CompletionResult | null => {
  const word = ctx.matchBefore(/[\w$.\u4e00-\u9fa5]*/)
  if (!word) return null
  // 关键：from 定位到最后一个「.」之后，避免用「别名.」去过滤候选（否则全部被滤掉）
  const dot = word.text.lastIndexOf('.')
  const from = word.from + (dot >= 0 ? dot + 1 : 0)
  // 点号后置空（如 `a.`）也应给出候选；仅完全无词且非显式触发时返回 null
  if (from === word.to && dot < 0 && !ctx.explicit) return null
  const prefix = word.text.slice(dot + 1).toLowerCase()
  // 点前的限定符（别名或表名），用于限定列范围
  const qualifier =
    dot >= 0 ? (word.text.slice(0, dot).toLowerCase().split('.').pop() ?? '') : ''

  const text = ctx.state.doc.toString()
  const baseOf = (n: string): string => (n.split('.').pop() ?? n).toLowerCase()
  // 解析 FROM/JOIN 引用表与别名
  const aliasMap = new Map<string, string>()
  const relevant = new Set<string>()
  const STOP = new Set([
    'on', 'where', 'left', 'right', 'inner', 'outer', 'full', 'join', 'cross',
    'natural', 'group', 'order', 'union', 'having', 'and', 'or', 'as',
  ])
  for (const m of text.matchAll(/\b(?:from|join)\s+([`"[\].\w$]+)(?:\s+(?:as\s+)?([\w$]+))?/gi)) {
    const tbl = baseOf(m[1].replace(/[`"[\]]/g, ''))
    relevant.add(tbl)
    const alias = m[2]?.toLowerCase()
    if (alias && !STOP.has(alias)) aliasMap.set(alias, tbl)
  }

  const options: Completion[] = []
  const seen = new Set<string>()
  const push = (o: Completion): void => {
    const k = o.label.toLowerCase()
    if (seen.has(k)) return
    seen.add(k)
    options.push(o)
  }

  const tables = snapshot?.tables ?? []
  // 计算需要补全列的表：
  // - 有「别名./表.」限定 → 仅该表
  // - 无限定且表数 ≤ 300 → 全部表
  // - 无限定且表数 > 300 → 仅当前 SQL 引用的表（FROM/JOIN）
  let columnTables: TableMeta[]
  if (qualifier) {
    const target = aliasMap.get(qualifier) ?? qualifier
    columnTables = tables.filter((t) => baseOf(t.name) === target)
  } else if (tables.length <= 300) {
    columnTables = tables
  } else {
    columnTables = tables.filter((t) => relevant.has(baseOf(t.name)))
  }

  // 关键字（有列限定符时不提示）
  if (!qualifier) for (const kw of SQL_KEYWORDS) push({ label: kw, type: 'keyword' })
  // 表名（有列限定符时不提示表名）
  if (!qualifier) {
    for (const t of tables) {
      push({
        label: t.name,
        type: t.kind === 'VIEW' ? 'interface' : 'class',
        detail: t.comment ? `${t.kind} · ${t.comment}` : t.kind,
      })
    }
  }
  // 列（内联展示类型与注释；图标按数据类型细化）
  for (const t of columnTables) {
    for (const c of t.columns) {
      const meta = qualifier ? c.dataType : `${t.name} · ${c.dataType}`
      push({
        label: c.name,
        type: colIconType(c.dataType),
        detail: c.comment ? `${meta} · ${c.comment}` : meta,
      })
    }
  }
  // 当前 SQL 中的标识符
  for (const m of text.matchAll(/[A-Za-z_][A-Za-z0-9_$]{1,}/g)) {
    const w = m[0]
    if (w.toLowerCase() !== prefix) push({ label: w, type: 'text' })
  }
  if (options.length === 0) return null
  return { from, options, validFor: /^[\w$]*$/ }
}

const SQL_KEYWORDS = [
  'SELECT', 'FROM', 'WHERE', 'INSERT', 'INTO', 'VALUES', 'UPDATE', 'SET', 'DELETE',
  'LEFT', 'RIGHT', 'INNER', 'OUTER', 'JOIN', 'ON', 'GROUP', 'BY', 'ORDER', 'HAVING',
  'UNION', 'ALL', 'DISTINCT', 'AS', 'AND', 'OR', 'NOT', 'NULL', 'IS', 'IN', 'LIKE',
  'BETWEEN', 'EXISTS', 'CASE', 'WHEN', 'THEN', 'ELSE', 'END', 'LIMIT', 'OFFSET',
  'CREATE', 'ALTER', 'DROP', 'TABLE', 'VIEW', 'INDEX', 'PRIMARY', 'KEY', 'FOREIGN',
  'REFERENCES', 'CONSTRAINT', 'DEFAULT', 'ASC', 'DESC', 'WITH', 'OVER', 'PARTITION',
]

function buildExtensions() {  return [
    phraseComp.of(phrasesExt()),
    minimapComp.of(minimapExt()),
    gutterComp.of(gutterExt(prefs.editor.lineNumbers)),
    history(),
    drawSelection(),
    dropCursor(),
    highlightSpecialChars(),
    rectangularSelection(),
    highlightActiveLine(),
    highlightSelectionMatches(),
    bracketMatching(),
    indentOnInput(),
    closeBrackets(),
    foldGutter(),
    dialectComp.of(dialectExt()),
    themeComp.of(themeOf(prefs.theme === 'dark')),
    fontComp.of(fontTheme(prefs.editor.fontSize)),
    wrapComp.of(prefs.editor.wrap ? EditorView.lineWrapping : []),
    indentComp.of(indentUnit.of(' '.repeat(prefs.editor.tabSize || 2))),
    cmPlaceholder(t('query.placeholder')),
    EditorState.allowMultipleSelections.of(true),
    readonlyComp.of(readonlyExt(props.readonly)),
    // SQL 自动补全（关键字 + 元数据/文档标识符，含注释）
    autocompletion({ activateOnTyping: true }),
    EditorState.languageData.of(() => [{ autocomplete: completionSource }]),
    EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        emit('update:modelValue', update.state.doc.toString())
        // 文档变化：去抖重载快照（可能新增 schema/表引用）
        scheduleSnapshot(update.state.doc.toString())
      }
    }),
    // 聚焦时也尝试加载
    EditorView.domEventHandlers({
      focus: () => {
        if (!snapshot) void loadSnapshot()
      },
    }),
    // 自定义快捷键（置于默认键位之前）
    keymapComp.of(customKeymap()),
    keymap.of([...closeBracketsKeymap, ...completionKeymap, ...defaultKeymap, ...historyKeymap, ...searchKeymap]),
  ]
}

onMounted(() => {
  if (!hostEl.value) return
  view = new EditorView({
    state: EditorState.create({
      doc: props.modelValue,
      extensions: buildExtensions(),
    }),
    parent: hostEl.value,
  })
  void loadSnapshot(props.modelValue)
})

// 只读开关
watch(
  () => props.readonly,
  (ro) => {
    view?.dispatch({ effects: readonlyComp.reconfigure(readonlyExt(ro)) })
  },
)

// 连接/库变化 → 重新加载补全元数据
watch(
  () => [props.connectionId, props.database],
  () => {
    snapKey = ''
    void loadSnapshot(props.modelValue)
  },
)

// 会话建立（连接成功）后 → 重新加载补全元数据
watch(
  () => connStore.sessionOf(props.connectionId)?.sessionId,
  (sid) => {
    if (sid) {
      snapKey = ''
      pendingKey = ''
      void loadSnapshot(props.modelValue)
    }
  },
)

onBeforeUnmount(() => {
  view?.destroy()
  view = null
})

// 外部文本变化（格式化/导入）→ 同步到编辑器
watch(
  () => props.modelValue,
  (val) => {
    if (!view) return
    const cur = view.state.doc.toString()
    if (val !== cur) {
      view.dispatch({ changes: { from: 0, to: cur.length, insert: val } })
    }
  },
)

// 方言 / 外观偏好热切换
watch(
  () => props.kind,
  (k) => {
    view?.dispatch({ effects: dialectComp.reconfigure(dialectExt(k)) })
  },
)
watch(
  () => prefs.editor.upperCaseKeywords,
  () => {
    view?.dispatch({ effects: dialectComp.reconfigure(dialectExt()) })
  },
)
watch(
  () => prefs.editor.tabSize,
  (size) => {
    view?.dispatch({ effects: indentComp.reconfigure(indentUnit.of(' '.repeat(size || 2))) })
  },
)
watch(
  () => prefs.editor.fontSize,
  (size) => {
    view?.dispatch({ effects: fontComp.reconfigure(fontTheme(size)) })
  },
)
// 快捷键绑定变更实时生效
watch(
  () => prefs.shortcuts,
  () => {
    view?.dispatch({ effects: keymapComp.reconfigure(customKeymap()) })
  },
  { deep: true },
)
watch(
  () => prefs.theme,
  (mode) => {
    view?.dispatch({ effects: themeComp.reconfigure(themeOf(mode === 'dark')) })
  },
)
watch(
  () => prefs.editor.wrap,
  (wrap) => {
    view?.dispatch({ effects: wrapComp.reconfigure(wrap ? EditorView.lineWrapping : []) })
  },
)
watch(
  () => prefs.editor.lineNumbers,
  (show) => {
    view?.dispatch({ effects: gutterComp.reconfigure(gutterExt(show)) })
  },
)
// 语言切换 → 内置面板文案热切换
watch(locale, () => {
  view?.dispatch({ effects: phraseComp.reconfigure(phrasesExt()) })
})
// 缩略图开关实时生效
watch(
  () => prefs.editor.minimap,
  () => {
    view?.dispatch({ effects: minimapComp.reconfigure(minimapExt()) })
  },
)

defineExpose({
  /** 当前选中文本（无选区返回空串） */
  getSelection(): string {
    if (!view) return ''
    const { from, to } = view.state.selection.main
    return from === to ? '' : view.state.doc.sliceString(from, to)
  },
  /** 光标所在行文本（执行当前行用） */
  getCurrentLine(): string {
    if (!view) return ''
    const head = view.state.selection.main.head
    return view.state.doc.lineAt(head).text
  },
  focus(): void {
    view?.focus()
  },
  /** 供菜单/工具栏调用的大小写转换 */
  transformCase(mode: 'upper' | 'lower'): void {
    if (view) transformCase(view, mode)
  },
  /** 供菜单/工具栏调用的压缩 */
  compress(): void {
    if (view) compressCode(view)
  },
  /** 光标处插入文本（新建表模板等） */
  insertAtCursor(text: string): void {
    if (!view) return
    const pos = view.state.selection.main.head
    view.dispatch({
      changes: { from: pos, insert: text },
      selection: EditorSelection.cursor(pos + text.length),
      scrollIntoView: true,
    })
    view.focus()
  },
})
</script>

<style scoped>
.sql-editor-host {
  width: 100%;
  min-height: 120px;
  max-height: var(--editor-max-height, 32vh);
  overflow: hidden;
  border-bottom: 1px solid var(--el-border-color-lighter);
}

.sql-editor-host:not(.is-fill) :deep(.cm-editor) {
  min-height: 120px;
  max-height: var(--editor-max-height, 32vh);
}

.sql-editor-host.is-fill {
  height: 100%;
  max-height: none;
  border-bottom: none;
}

.sql-editor-host :deep(.cm-editor) {
  height: 100%;
}

.sql-editor-host :deep(.cm-editor.cm-focused) {
  outline: none;
}

/* 缩略图容器（内容由 codemirror-minimap 绘制） */
.sql-editor-host :deep(.cm-minimap) {
  width: 84px;
  height: 100%;
  background: var(--el-bg-color);
  border-left: 1px solid var(--el-border-color-lighter);
  opacity: 0.85;
}
</style>
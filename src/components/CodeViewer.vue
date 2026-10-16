<template>
  <div class="code-viewer">
    <div class="cv-toolbar">
      <el-checkbox
        :model-value="prefs.viewer.lineNumbers"
        size="small"
        @update:model-value="(v: string | number | boolean) => prefs.updateViewer({ lineNumbers: Boolean(v) })"
      >
        {{ t('codeViewer.lineNumbers') }}
      </el-checkbox>
      <el-button
        size="small"
        :icon="Fold"
        :disabled="!prefs.viewer.folding"
        @click="collapseAll"
      >
        {{ t('codeViewer.collapseAll') }}
      </el-button>
      <el-button size="small" :icon="Expand" :disabled="!prefs.viewer.folding" @click="expandAll">
        {{ t('codeViewer.expandAll') }}
      </el-button>
      <el-button size="small" :icon="Brush" @click="formatCode">
        {{ t('query.format') }}
      </el-button>
      <el-button size="small" @click="caseConvert('upper')">{{ t('codeViewer.upper') }}</el-button>
      <el-button size="small" @click="caseConvert('lower')">{{ t('codeViewer.lower') }}</el-button>
      <el-checkbox
        v-if="ddlOptions"
        v-model="stripStorage"
        size="small"
      >
        {{ t('inspector.stripStorage') }}
      </el-checkbox>
      <el-checkbox v-if="ddlOptions" v-model="stripQualifier" size="small">
        {{ t('inspector.stripQualifier') }}
      </el-checkbox>
      <el-button size="small" :icon="ScaleToOriginal" @click="compressCode">
        {{ t('query.compress') }}
      </el-button>
      <div class="cv-spacer" />
      <el-button size="small" :icon="CopyDocument" @click="copyText">
        {{ t('cell.copy') }}
      </el-button>
    </div>
    <div ref="hostEl" class="cv-host" />
  </div>
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import { Compartment, EditorState, type Extension } from '@codemirror/state'
import {
  EditorView,
  highlightSpecialChars,
  keymap,
  lineNumbers,
} from '@codemirror/view'
import { defaultKeymap } from '@codemirror/commands'
import { highlightSelectionMatches, searchKeymap } from '@codemirror/search'
import {
  bracketMatching,
  defaultHighlightStyle,
  foldAll,
  foldGutter,
  foldService,
  syntaxHighlighting,
  unfoldAll,
} from '@codemirror/language'
import { MySQL, PLSQL, PostgreSQL, sql } from '@codemirror/lang-sql'
import { oneDark } from '@codemirror/theme-one-dark'
import { Brush, CopyDocument, Expand, Fold, ScaleToOriginal } from '@element-plus/icons-vue'
import { usePreferencesStore } from '../stores/preferences'
import { compressSql, formatSql } from '../utils/sqlFormat'
import { convertOutsideLiterals } from '../utils/sqlCase'
import { cleanDdl } from '../utils/ddlClean'
import type { DbKind } from '../types/db'

/**
 * 只读 SQL 代码查看器（源码/DDL 展示专用）：
 * 语法高亮 + 行号（可切换）+ 括号代码块折叠 + 格式化；禁止编辑。
 */
const props = withDefaults(
  defineProps<{
    modelValue: string
    kind: DbKind
    /** 展示 DDL 清洗选项（去除存储相关 / 去除限定符） */
    ddlOptions?: boolean
  }>(),
  { ddlOptions: false },
)

const { t } = useI18n()
const prefs = usePreferencesStore()
const hostEl = ref<HTMLDivElement>()
let view: EditorView | null = null

/** DDL 清洗选项 */
const stripStorage = ref(false)
const stripQualifier = ref(false)

/** 实际展示内容（按清洗选项处理） */
const displayText = computed(() =>
  props.ddlOptions
    ? cleanDdl(props.modelValue, props.kind, {
        stripStorage: stripStorage.value,
        stripQualifier: stripQualifier.value,
      })
    : props.modelValue,
)

const dialectComp = new Compartment()
const themeComp = new Compartment()
const fontComp = new Compartment()
const gutterComp = new Compartment()

function dialectOf(kind: DbKind) {
  switch (kind) {
    case 'postgresql':
      return PostgreSQL
    case 'oracle':
    case 'oceanbase-oracle':
      return PLSQL
    default:
      return MySQL
  }
}

function themeOf(dark: boolean) {
  return dark ? oneDark : [syntaxHighlighting(defaultHighlightStyle, { fallback: true })]
}

function fontTheme(size: number) {
  return EditorView.theme({
    '&': { fontSize: `${size}px`, height: '100%' },
    '.cm-content': {
      fontFamily: "Consolas, 'Cascadia Mono', 'Courier New', monospace",
      padding: '8px 0',
    },
    '.cm-scroller': { overflow: 'auto' },
  })
}

/** 偏好驱动的行号 + 折叠扩展（设置面板改动实时生效） */
function gutterExt(showLineNumbers: boolean, folding: boolean) {
  const exts: Extension[] = []
  if (showLineNumbers) exts.push(lineNumbers())
  if (folding) exts.push(foldGutter())
  return exts
}

/**
 * 基于括号的通用折叠服务（SQL 无内置折叠定义）：
 * 行尾的 `(` 折叠至匹配 `)`；扫描上限 500 行防止性能问题。
 */
const bracketFold = foldService.of((state, lineStart, lineEnd) => {
  const doc = state.doc
  const line = doc.lineAt(lineStart)
  const text = line.text
  const scanTo = Math.min(lineEnd - line.from, text.length)
  let open = -1
  for (let i = scanTo - 1; i >= 0; i--) {
    const ch = text[i]
    if (ch === '(' || ch === '{') {
      open = i
      break
    }
    if (ch === ')' || ch === '}') return null
  }
  if (open < 0) return null
  const openPos = line.from + open
  const openChar = text[open]
  const closeChar = openChar === '(' ? ')' : '}'
  let depth = 1
  let pos = openPos + 1
  const limitLine = Math.min(doc.lines, line.number + 500)
  const limit = doc.line(limitLine).to
  while (pos < limit) {
    const ch = doc.sliceString(pos, pos + 1)
    if (ch === openChar) depth += 1
    else if (ch === closeChar) {
      depth -= 1
      if (depth === 0) break
    }
    pos += 1
  }
  if (depth !== 0 || pos <= openPos + 1) return null
  return { from: openPos + 1, to: pos }
})

function buildExtensions() {
  return [
    gutterComp.of(gutterExt(prefs.viewer.lineNumbers, prefs.viewer.folding)),
    highlightSpecialChars(),
    highlightSelectionMatches(),
    bracketMatching(),
    bracketFold,
    dialectComp.of(sql({ dialect: dialectOf(props.kind), upperCaseKeywords: false })),
    themeComp.of(themeOf(prefs.theme === 'dark')),
    fontComp.of(fontTheme(prefs.editor.fontSize)),
    EditorView.lineWrapping,
    // 只读：允许选择/复制/搜索/折叠，禁止修改
    EditorState.readOnly.of(true),
    EditorView.editable.of(false),
    keymap.of([...defaultKeymap, ...searchKeymap]),
  ]
}

onMounted(() => {
  if (!hostEl.value) return
  view = new EditorView({
    state: EditorState.create({ doc: displayText.value, extensions: buildExtensions() }),
    parent: hostEl.value,
  })
})

onBeforeUnmount(() => {
  view?.destroy()
  view = null
})

watch(displayText, (val) => {
  if (!view) return
  const cur = view.state.doc.toString()
  if (val !== cur) view.dispatch({ changes: { from: 0, to: cur.length, insert: val } })
})

// 源码查看器偏好（行号 / 折叠）由设置面板驱动，改动实时生效
watch(
  () => [prefs.viewer.lineNumbers, prefs.viewer.folding],
  ([lineNumbers, folding]) => {
    view?.dispatch({ effects: gutterComp.reconfigure(gutterExt(lineNumbers, folding)) })
  },
)

watch(
  () => props.kind,
  (k) => {
    view?.dispatch({
      effects: dialectComp.reconfigure(sql({ dialect: dialectOf(k), upperCaseKeywords: false })),
    })
  },
)

watch(
  () => prefs.theme,
  (mode) => {
    view?.dispatch({ effects: themeComp.reconfigure(themeOf(mode === 'dark')) })
  },
)

watch(
  () => prefs.editor.fontSize,
  (size) => {
    view?.dispatch({ effects: fontComp.reconfigure(fontTheme(size)) })
  },
)

/** 折叠 / 展开全部代码块 */
function collapseAll(): void {
  if (view) foldAll(view)
}

function expandAll(): void {
  if (view) unfoldAll(view)
}

/** 按方言格式化当前显示内容（查看辅助，不回写源数据） */
function formatCode(): void {
  if (!view) return
  const cur = view.state.doc.toString()
  try {
    const formatted = formatSql(props.kind, cur, prefs.formatPrefs)
    view.dispatch({ changes: { from: 0, to: cur.length, insert: formatted } })
  } catch (e) {
    ElMessage.error(t('export.failed', { msg: String(e) }))
  }
}

/** 压缩当前显示内容（去注释/多余空白，字面值保护） */
function compressCode(): void {
  if (!view) return
  const cur = view.state.doc.toString()
  view.dispatch({ changes: { from: 0, to: cur.length, insert: compressSql(cur) } })
}

/** 大小写转换（跳过字符串字面值，避免修改字面值） */
function caseConvert(mode: 'upper' | 'lower'): void {
  if (!view) return
  const cur = view.state.doc.toString()
  view.dispatch({ changes: { from: 0, to: cur.length, insert: convertOutsideLiterals(cur, mode) } })
}

async function copyText(): Promise<void> {
  const text = view?.state.doc.toString() ?? displayText.value
  await navigator.clipboard.writeText(text)
  ElMessage.success(t('cell.copied'))
}
</script>

<style scoped>
.code-viewer {
  height: 100%;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 4px;
  overflow: hidden;
}

.cv-toolbar {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  background: var(--el-bg-color);
  flex-shrink: 0;
}

.cv-toolbar :deep(.el-checkbox) {
  margin-right: 2px;
}

.cv-spacer {
  flex: 1;
}

.cv-host {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.cv-host :deep(.cm-editor) {
  height: 100%;
}

.cv-host :deep(.cm-editor.cm-focused) {
  outline: none;
}
</style>
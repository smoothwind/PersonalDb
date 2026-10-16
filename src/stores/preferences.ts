import { defineStore } from 'pinia'
import { reactive, ref } from 'vue'
import { initialLocale, setLocale, type Locale } from '../i18n'
import { normalizeCombo, shortcutDefault } from '../utils/shortcuts'

export type ThemeMode = 'light' | 'dark'

/** 编辑器外观偏好 */
export interface EditorPrefs {
  /** 字号 px */
  fontSize: number
  /** 自动换行 */
  wrap: boolean
  /** 显示行号 */
  lineNumbers: boolean
  /** 缩进宽度（空格数） */
  tabSize: number
  /** 关键字大写高亮 */
  upperCaseKeywords: boolean
}

/** 源码查看器偏好（只读代码视图） */
export interface ViewerPrefs {
  /** 显示行号 */
  lineNumbers: boolean
  /** 启用代码块折叠 */
  folding: boolean
}

/** 结果集展示偏好 */
export interface ResultPrefs {
  /** 行高密度：紧凑 / 标准 / 宽松 */
  rowHeight: 'compact' | 'normal' | 'loose'
  /** 斑马纹 */
  zebra: boolean
  /** JSON 列默认格式化展示 */
  formatJson: boolean
  /** 结果行数上限 */
  maxRows: number
  /** 单页行数（滚动加载分页大小，1..100） */
  pageSize: number
  /** 结果集总行数上限（100..10000） */
  maxTotalRows: number
}

/** 数据格式展示偏好（单元格值的呈现方式） */
export interface DataFormatPrefs {
  /** 日期时间显示格式：原始 / 本地 / ISO 8601 */
  dateStyle: 'original' | 'local' | 'iso'
  /** 数值千分位分隔 */
  thousand: boolean
  /** 布尔值样式：true/false 或 YES/NO */
  boolStyle: 'trueFalse' | 'yesNo'
  /** NULL 显示文本 */
  nullText: string
  /** 长文本单元格截断长度（字符） */
  truncateLength: number
}

/** 全局界面字体偏好 */
export interface UiPrefs {
  /** 字体族（空 = 系统默认） */
  fontFamily: string
  /** 基准字号 px */
  fontSize: number
  /** 行高倍数 */
  lineHeight: number
  /** 页签全局数量上限 */
  maxTabs: number
  /** 左侧模式下：连接树与页签 互斥 Tab / 上下分栏 */
  leftPanelMode: 'tabs' | 'stack'
}

/** 自动更新检测偏好（仅打包版生效，见 docs/specifications.md SPEC-DEV-2） */
export interface UpdatePrefs {
  /** 启动后自动静默检测更新 */
  autoCheck: boolean
  /** 已跳过的版本号（不再提示，空 = 不跳过） */
  skippedVersion: string
}

/** 兜底默认（供 loadPrefs 合并旧数据） */
export function defaultUiPrefs(): UiPrefs {
  return {
    fontFamily: '',
    fontSize: 14,
    lineHeight: 1.5,
    maxTabs: 50,
    leftPanelMode: 'tabs',
  }
}

/** 值转换规则：按列语义将值包裹为函数模板（{v} = 值的 SQL 字面量） */
export interface ValueRule {
  semantic: string
  template: string
}

/** 结果集导出偏好（SQL INSERT 默认选项） */
export interface ExportPrefs {  /** 去掉对象/字段名引号 */
  unquoteIdent: boolean
  /** 标识符大小写 */
  identCase: 'original' | 'upper' | 'lower'
  /** 日期/时间显式转换 */
  dateConvert: boolean
  /** UTF-8 BOM */
  bom: boolean
  /** 每条 INSERT 合并行数（1 = 每行单独一条） */
  insertBatchSize: number
  /** 值转换规则（TO_CLOB / NVL / COALESCE 等自定义） */
  valueRules: ValueRule[]
}

/** SQL 美化方案偏好（对齐 PL/SQL Developer Beautifier `.br` 规则） */
export interface FormatPrefs {
  /** 右边距（决定一行最大宽度） */
  rightMargin: number
  /** 缩进宽度 */
  indent: number
  /** 使用制表符 */
  useTab: boolean
  /** 制表符宽度 */
  tabSize: number
  /** 关键字的风格：0 不变 / 1 大写 / 2 小写 / 3 首字母大写 */
  keywordCase: 0 | 1 | 2 | 3
  /** 标识符的风格：0/1/2/3 */
  identifierCase: 0 | 1 | 2 | 3
  /** 空行处理：0 移除 / 1 合并为一行 / 2 保持 */
  emptyLines: 0 | 1 | 2
  /** 可能时放在一行 */
  onOneLineIfPossible: boolean
  /** 元素格式：0 同行 / 1 尽量同行 / 2 每项一行 / 3 自适应 */
  selectItemFormat: 0 | 1 | 2 | 3
  /** 元素对齐 */
  selectItemAlign: boolean
  /** 逗号在元素后（否则在元素前） */
  selectCommaAfter: boolean
  /** WHERE 中 AND/OR 另起一行 */
  whereSplitAndOr: boolean
  /** AND/OR 放在表达式之后 */
  andOrAfterExpression: boolean
  /** AND/OR 位于 WHERE 之下 */
  whereAndOrUnderWhere: boolean
  /** 连接时在 ON 之前换行 */
  joinSplitBeforeOn: boolean
  /** THEN/ELSE/ELSIF 换行 */
  thenOnNewLine: boolean
  /** LOOP 换行 */
  loopOnNewLine: boolean
  /** 对齐变量声明组 */
  alignDeclarationGroups: boolean
  /** 对齐赋值组 */
  alignAssignmentGroups: boolean
}

/** PL/SQL Developer 风格预设（参考 Beautifier 规则文件） */
export function plsqlDeveloperPreset(): FormatPrefs {
  return {
    rightMargin: 90,
    indent: 2,
    useTab: false,
    tabSize: 2,
    keywordCase: 1,
    identifierCase: 3,
    emptyLines: 1,
    onOneLineIfPossible: false,
    selectItemFormat: 1,
    selectItemAlign: true,
    selectCommaAfter: true,
    whereSplitAndOr: true,
    andOrAfterExpression: false,
    whereAndOrUnderWhere: false,
    joinSplitBeforeOn: true,
    thenOnNewLine: true,
    loopOnNewLine: true,
    alignDeclarationGroups: true,
    alignAssignmentGroups: true,
  }
}

/** 默认（紧凑 Fit） */
export function defaultFormatPrefs(): FormatPrefs {
  return {
    rightMargin: 120,
    indent: 2,
    useTab: false,
    tabSize: 2,
    keywordCase: 1,
    identifierCase: 0,
    emptyLines: 1,
    onOneLineIfPossible: false,
    selectItemFormat: 1,
    selectItemAlign: false,
    selectCommaAfter: true,
    whereSplitAndOr: true,
    andOrAfterExpression: false,
    whereAndOrUnderWhere: false,
    joinSplitBeforeOn: true,
    thenOnNewLine: false,
    loopOnNewLine: false,
    alignDeclarationGroups: false,
    alignAssignmentGroups: false,
  }
}

const THEME_KEY = 'personaldb.theme'
const EDITOR_KEY = 'personaldb.editor'
const RESULT_KEY = 'personaldb.result'
const VIEWER_KEY = 'personaldb.viewer'
const DATAFORMAT_KEY = 'personaldb.dataformat'
const UI_KEY = 'personaldb.ui'
const EXPORT_KEY = 'personaldb.export'
const FORMAT_KEY = 'personaldb.format'
const SHORTCUT_KEY = 'personaldb.shortcuts'
const UPDATE_KEY = 'personaldb.update'

function initialTheme(): ThemeMode {
  const saved = localStorage.getItem(THEME_KEY)
  return saved === 'dark' ? 'dark' : 'light'
}

function loadPrefs<T extends object>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key)
    return raw ? { ...fallback, ...(JSON.parse(raw) as Partial<T>) } : fallback
  } catch {
    return fallback
  }
}

/** 首选项：外观主题、界面语言、编辑器样式、结果集展示 */
export const usePreferencesStore = defineStore('preferences', () => {
  const theme = ref<ThemeMode>(initialTheme())
  const locale = ref<Locale>(initialLocale())

  const editor = reactive<EditorPrefs>(
    loadPrefs<EditorPrefs>(EDITOR_KEY, {
      fontSize: 14,
      wrap: false,
      lineNumbers: true,
      tabSize: 2,
      upperCaseKeywords: false,
    }),
  )
  const result = reactive<ResultPrefs>(
    loadPrefs<ResultPrefs>(RESULT_KEY, {
      rowHeight: 'compact',
      zebra: true,
      formatJson: true,
      maxRows: 1000,
      pageSize: 10,
      maxTotalRows: 10000,
    }),
  )
  const dataFormat = reactive<DataFormatPrefs>(
    loadPrefs<DataFormatPrefs>(DATAFORMAT_KEY, {
      dateStyle: 'original',
      thousand: false,
      boolStyle: 'trueFalse',
      nullText: 'NULL',
      truncateLength: 300,
    }),
  )
  const viewer = reactive<ViewerPrefs>(
    loadPrefs<ViewerPrefs>(VIEWER_KEY, { lineNumbers: true, folding: true }),
  )
  const ui = reactive<UiPrefs>(loadPrefs<UiPrefs>(UI_KEY, defaultUiPrefs()))
  const exportPrefs = reactive<ExportPrefs>(
    loadPrefs<ExportPrefs>(EXPORT_KEY, {
      unquoteIdent: false,
      identCase: 'original',
      dateConvert: true,
      bom: false,
      insertBatchSize: 10,
      valueRules: [],
    }),
  )
  const formatPrefs = reactive<FormatPrefs>(
    loadPrefs<FormatPrefs>(FORMAT_KEY, defaultFormatPrefs()),
  )
  const update = reactive<UpdatePrefs>(
    loadPrefs<UpdatePrefs>(UPDATE_KEY, { autoCheck: true, skippedVersion: '' }),
  )
  /** 快捷键覆盖（actionKey → 组合）；未覆盖项用默认值 */
  const shortcuts = ref<Record<string, string>>(
    loadPrefs<Record<string, string>>(SHORTCUT_KEY, {}),
  )

  function applyTheme(): void {
    document.documentElement.classList.toggle('dark', theme.value === 'dark')
  }

  function setTheme(mode: ThemeMode): void {
    theme.value = mode
    localStorage.setItem(THEME_KEY, mode)
    applyTheme()
  }

  function toggleTheme(): void {
    setTheme(theme.value === 'light' ? 'dark' : 'light')
  }

  function changeLocale(newLocale: Locale): void {
    locale.value = newLocale
    setLocale(newLocale)
  }

  function updateEditor(patch: Partial<EditorPrefs>): void {
    Object.assign(editor, patch)
    localStorage.setItem(EDITOR_KEY, JSON.stringify({ ...editor }))
  }

  function updateResult(patch: Partial<ResultPrefs>): void {
    Object.assign(result, patch)
    localStorage.setItem(RESULT_KEY, JSON.stringify({ ...result }))
  }

  function updateViewer(patch: Partial<ViewerPrefs>): void {
    Object.assign(viewer, patch)
    localStorage.setItem(VIEWER_KEY, JSON.stringify({ ...viewer }))
  }

  function updateDataFormat(patch: Partial<DataFormatPrefs>): void {
    Object.assign(dataFormat, patch)
    localStorage.setItem(DATAFORMAT_KEY, JSON.stringify({ ...dataFormat }))
  }

  /** 应用全局界面字体（字体族/字号/行高）到根元素与 Element Plus 变量 */
  function applyUi(): void {
    const root = document.documentElement
    const s = ui.fontSize
    if (ui.fontFamily.trim()) root.style.fontFamily = ui.fontFamily
    else root.style.removeProperty('font-family')
    root.style.fontSize = `${s}px`
    root.style.lineHeight = String(ui.lineHeight)
    root.style.setProperty('--app-line-height', String(ui.lineHeight))
    root.style.setProperty('--el-font-size-base', `${s}px`)
    root.style.setProperty('--el-font-size-small', `${Math.max(10, s - 2)}px`)
    root.style.setProperty('--el-font-size-large', `${s + 4}px`)
  }

  function updateUi(patch: Partial<UiPrefs>): void {
    Object.assign(ui, patch)
    localStorage.setItem(UI_KEY, JSON.stringify({ ...ui }))
    applyUi()
  }

  function updateExport(patch: Partial<ExportPrefs>): void {
    Object.assign(exportPrefs, patch)
    localStorage.setItem(EXPORT_KEY, JSON.stringify({ ...exportPrefs }))
  }

  function updateFormat(patch: Partial<FormatPrefs>): void {
    Object.assign(formatPrefs, patch)
    localStorage.setItem(FORMAT_KEY, JSON.stringify({ ...formatPrefs }))
  }

  function updateUpdatePrefs(patch: Partial<UpdatePrefs>): void {
    Object.assign(update, patch)
    localStorage.setItem(UPDATE_KEY, JSON.stringify({ ...update }))
  }

  /** 记录「不再提示该版本」 */
  function skipVersion(version: string): void {
    updateUpdatePrefs({ skippedVersion: version })
  }

  function clearSkippedVersion(): void {
    updateUpdatePrefs({ skippedVersion: '' })
  }

  /** 当前生效的快捷键组合 */
  function shortcutOf(actionKey: string): string {
    return shortcuts.value[actionKey] ?? shortcutDefault(actionKey)
  }

  function setShortcut(actionKey: string, combo: string): void {
    shortcuts.value = { ...shortcuts.value, [actionKey]: normalizeCombo(combo) }
    localStorage.setItem(SHORTCUT_KEY, JSON.stringify(shortcuts.value))
  }

  function resetShortcut(actionKey: string): void {
    const next = { ...shortcuts.value }
    delete next[actionKey]
    shortcuts.value = next
    localStorage.setItem(SHORTCUT_KEY, JSON.stringify(next))
  }

  function resetAllShortcuts(): void {
    shortcuts.value = {}
    localStorage.setItem(SHORTCUT_KEY, JSON.stringify({}))
  }

  // 启动时立即应用
  applyTheme()
  applyUi()

  return {
    theme,
    locale,
    editor,
    result,
    viewer,
    dataFormat,
    ui,
    exportPrefs,
    formatPrefs,
    update,
    shortcuts,
    setTheme,
    toggleTheme,
    changeLocale,
    updateEditor,
    updateResult,
    updateViewer,
    updateDataFormat,
    updateUi,
    updateExport,
    updateFormat,
    updateUpdatePrefs,
    skipVersion,
    clearSkippedVersion,
    shortcutOf,
    setShortcut,
    resetShortcut,
    resetAllShortcuts,
  }
})
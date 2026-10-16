/**
 * 快捷键体系：动作定义、组合匹配与格式化。
 * 组合格式：`Ctrl+Shift+F`（修饰键顺序 Ctrl → Shift → Alt → 主键）。
 */
export interface ShortcutAction {
  key: string
  /** 动作名称 i18n key */
  labelKey: string
  /** global：应用级（App 监听）；editor：编辑器内（CM6 keymap） */
  scope: 'global' | 'editor'
  default: string
}

export const SHORTCUT_ACTIONS: ShortcutAction[] = [
  { key: 'newConnection', labelKey: 'app.newConnection', scope: 'global', default: 'Ctrl+N' },
  { key: 'newQuery', labelKey: 'app.newQuery', scope: 'global', default: 'Ctrl+Shift+N' },
  { key: 'toggleTheme', labelKey: 'menu.toggleTheme', scope: 'global', default: 'Ctrl+Shift+T' },
  { key: 'openSettings', labelKey: 'settings.title', scope: 'global', default: 'Ctrl+,' },
  { key: 'toggleLogPanel', labelKey: 'logs.panel', scope: 'global', default: 'Ctrl+L' },
  { key: 'resetLayout', labelKey: 'menu.resetLayout', scope: 'global', default: 'Ctrl+Alt+R' },
  { key: 'run', labelKey: 'query.run', scope: 'editor', default: 'Ctrl+Enter' },
  { key: 'runSelection', labelKey: 'query.runSelected', scope: 'editor', default: 'Ctrl+Shift+Enter' },
  { key: 'runLine', labelKey: 'query.runLine', scope: 'editor', default: 'Ctrl+Alt+Enter' },
  { key: 'format', labelKey: 'query.format', scope: 'editor', default: 'Ctrl+Shift+F' },
  { key: 'compress', labelKey: 'query.compress', scope: 'editor', default: 'Ctrl+Alt+K' },
  { key: 'upper', labelKey: 'menu.upper', scope: 'editor', default: 'Ctrl+U' },
  { key: 'lower', labelKey: 'menu.lower', scope: 'editor', default: 'Ctrl+Shift+U' },
]

export function shortcutDefault(actionKey: string): string {
  return SHORTCUT_ACTIONS.find((a) => a.key === actionKey)?.default ?? ''
}

/** 规范化组合（修饰键顺序统一、主键大写） */
export function normalizeCombo(combo: string): string {
  const parts = combo
    .split('+')
    .map((p) => p.trim())
    .filter(Boolean)
  const mods: string[] = []
  let main = ''
  for (const p of parts) {
    const low = p.toLowerCase()
    if (low === 'ctrl' || low === 'control') mods.push('Ctrl')
    else if (low === 'shift') mods.push('Shift')
    else if (low === 'alt') mods.push('Alt')
    else main = p.length === 1 ? p.toUpperCase() : p
  }
  return [...mods, main].filter(Boolean).join('+')
}

/** 键盘事件是否匹配组合 */
export function matchesShortcut(e: KeyboardEvent, combo: string): boolean {
  if (!combo) return false
  const parts = combo.split('+').map((p) => p.trim().toLowerCase())
  const main = parts[parts.length - 1]
  const needCtrl = parts.includes('ctrl')
  const needShift = parts.includes('shift')
  const needAlt = parts.includes('alt')
  if (e.ctrlKey !== needCtrl || e.shiftKey !== needShift || e.altKey !== needAlt) return false
  return e.key.toLowerCase() === main
}

/** 由键盘事件生成组合（仅按下主键时返回，修饰键单独按下返回 null） */
export function eventToCombo(e: KeyboardEvent): string | null {
  if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return null
  const parts: string[] = []
  if (e.ctrlKey) parts.push('Ctrl')
  if (e.shiftKey) parts.push('Shift')
  if (e.altKey) parts.push('Alt')
  const main = e.key.length === 1 ? e.key.toUpperCase() : e.key
  parts.push(main)
  return parts.join('+')
}

/** 组合 → CodeMirror keymap 键名（如 `Mod-Shift-f`） */
export function toCodeMirrorKey(combo: string): string {
  return combo
    .split('+')
    .map((p) => {
      const s = p.trim()
      const low = s.toLowerCase()
      if (low === 'ctrl' || low === 'control') return 'Mod'
      if (low === 'shift') return 'Shift'
      if (low === 'alt') return 'Alt'
      return s.length === 1 ? s.toLowerCase() : s
    })
    .join('-')
}
/**
 * 字体设置共用常量与工具（设置页「字体设置栏」/ 对齐 UX-FRW SettingsShell 与 UxSelect/UxSlider 模式）。
 *
 * - 编辑器等宽字体栈：SQL 编辑器（SqlEditor）、源码查看器（CodeViewer）与设置预览共用；
 *   用户自定义字体族置于栈首，缺失时逐级回退到 Windows 内置等宽栈
 * - 字体选项表：设置栏下拉的常见字体预设（el-select allow-create 允许输入本机任意字体族）
 */

/** 编辑器默认等宽字体栈（未自定义时的兜底，与既有渲染一致） */
export const DEFAULT_EDITOR_FONT = "Consolas, 'Cascadia Mono', 'Courier New', monospace"

/** 自定义字体族 → 编辑器字体栈（空 = 默认栈；自定义置于栈首保证缺失时逐级回退） */
export function editorFontStack(family: string): string {
  const f = family.trim()
  return f ? `${f}, ${DEFAULT_EDITOR_FONT}` : DEFAULT_EDITOR_FONT
}

export interface FontOption {
  /** 展示名（含中文名） */
  label: string
  /** font-family 值 */
  value: string
}

/** 界面字体常见预设（Windows 内置优先 + 常见开源中文字体） */
export const UI_FONT_OPTIONS: FontOption[] = [
  { label: '微软雅黑 Microsoft YaHei', value: 'Microsoft YaHei' },
  { label: '等线 DengXian', value: 'DengXian' },
  { label: '黑体 SimHei', value: 'SimHei' },
  { label: '宋体 SimSun', value: 'SimSun' },
  { label: '楷体 KaiTi', value: 'KaiTi' },
  { label: '仿宋 FangSong', value: 'FangSong' },
  { label: '思源黑体 Source Han Sans SC', value: 'Source Han Sans SC' },
  { label: 'Noto Sans SC', value: 'Noto Sans SC' },
  { label: 'MiSans', value: 'MiSans' },
  { label: 'HarmonyOS Sans SC', value: 'HarmonyOS Sans SC' },
  { label: 'Segoe UI', value: 'Segoe UI' },
]

/** 编辑器等宽字体常见预设 */
export const MONO_FONT_OPTIONS: FontOption[] = [
  { label: 'Consolas（Windows 默认）', value: 'Consolas' },
  { label: 'Cascadia Mono（Windows 11）', value: 'Cascadia Mono' },
  { label: 'JetBrains Mono', value: 'JetBrains Mono' },
  { label: 'Source Code Pro', value: 'Source Code Pro' },
  { label: 'Fira Code', value: 'Fira Code' },
  { label: '等距更纱黑体 Sarasa Mono SC', value: 'Sarasa Mono SC' },
  { label: '新宋体 NSimSun', value: 'NSimSun' },
  { label: 'Courier New', value: 'Courier New' },
]

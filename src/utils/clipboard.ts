import { ElMessage } from 'element-plus'
import { i18n } from '../i18n'
import type { CellValue } from '../types/db'

/** 单元格值 → 纯文本（用于复制） */
export function cellToText(v: CellValue | undefined | null): string {
  if (v === null || v === undefined) return ''
  return String(v)
}

/** 复制文本到剪贴板（带提示） */
export async function copyText(text: string): Promise<void> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text)
    } else {
      const ta = document.createElement('textarea')
      ta.value = text
      ta.style.position = 'fixed'
      ta.style.opacity = '0'
      document.body.appendChild(ta)
      ta.select()
      document.execCommand('copy')
      document.body.removeChild(ta)
    }
    ElMessage.success(i18n.global.t('common.copied'))
  } catch {
    ElMessage.error(i18n.global.t('common.copyFailed'))
  }
}

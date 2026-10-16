import { ElNotification } from 'element-plus'
import { revealItemInDir } from '@tauri-apps/plugin-opener'

const lastAt: Record<string, number> = {}

/** 导出成功通知：点击打开文件所在目录 */
export function notifyFileSaved(path: string, message: string): void {
  ElNotification({
    title: message,
    message: path,
    type: 'success',
    duration: 6000,
    onClick: () => {
      revealItemInDir(path).catch(() => {})
    },
  })
}

/** 去重提示：同一 key 在 windowMs 内只提示一次（避免多 Tab 重复提醒）。
 *  传入 onClick 时使用可点击通知（如点击重连）。 */
export function notifyOnce(
  key: string,
  message: string,
  opts?: { windowMs?: number; onClick?: () => void },
): void {
  const windowMs = opts?.windowMs ?? 5000
  const now = Date.now()
  if (lastAt[key] && now - lastAt[key] < windowMs) return
  lastAt[key] = now
  if (opts?.onClick) {
    ElNotification({
      title: message,
      type: 'warning',
      duration: 8000,
      onClick: () => {
        opts.onClick?.()
      },
    })
  } else {
    ElNotification({ title: message, type: 'warning', duration: 5000 })
  }
}

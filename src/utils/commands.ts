/**
 * 跨组件命令分发（菜单栏 → 当前激活的查询 Tab）。
 * 通过 window 自定义事件，无需额外依赖。
 */
export interface AppCommandPayload {
  cmd: string
  /** 目标 Tab id（缺省表示广播） */
  tabId?: string
  /** 目标设置分类（open-settings 命令用） */
  section?: string
}

const EVENT_NAME = 'app-command'

export function dispatchCommand(cmd: string, tabId?: string, section?: string): void {
  window.dispatchEvent(
    new CustomEvent<AppCommandPayload>(EVENT_NAME, { detail: { cmd, tabId, section } }),
  )
}

export function onAppCommand(handler: (payload: AppCommandPayload) => void): () => void {
  const listener = (e: Event): void => {
    handler((e as CustomEvent<AppCommandPayload>).detail)
  }
  window.addEventListener(EVENT_NAME, listener)
  return () => window.removeEventListener(EVENT_NAME, listener)
}
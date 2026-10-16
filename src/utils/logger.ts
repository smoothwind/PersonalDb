/**
 * 前端日志封装：统一走 tauri-plugin-log（stdout / 日志文件 / WebView 控制台）。
 * - initLogging：转发 console.* 到日志文件，并挂接全局错误捕获
 * - logger.debug/info/warn/error：业务关键点调用（禁止打印密码等敏感信息）
 */
import {
  attachConsole,
  debug,
  error,
  info,
  warn,
} from '@tauri-apps/plugin-log'

export const logger = { debug, info, warn, error }

let initialized = false

export async function initLogging(): Promise<void> {
  if (initialized) return
  initialized = true
  try {
    // 浏览器 console.log/warn/error 同步转发到日志文件
    await attachConsole()
  } catch (e) {
    console.error('日志插件初始化失败', e)
  }

  // 全局运行时错误捕获
  window.addEventListener('error', (event) => {
    error(`window.error: ${event.message} @ ${event.filename}:${event.lineno}:${event.colno}`)
  })
  window.addEventListener('unhandledrejection', (event) => {
    error(`unhandledrejection: ${String(event.reason)}`)
  })
}

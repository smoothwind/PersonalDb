/**
 * 自动更新检测（tauri-plugin-updater）。
 *
 * 两种入口：
 * - 启动自动检测：`checkForUpdate(true)`，静默、失败仅记日志；命中新版本后由状态栏/帮助菜单
 *   给出**非模态**提示（不打断操作），点击才进入确认与下载。
 * - 手动检测：`runManualCheck()`，帮助菜单「检查更新」与首选项按钮共用，带对话框反馈。
 *
 * 未配置更新端点/签名公钥时插件会抛错 → 归类为「未启用」，静默检测不做任何打扰。
 * 启用步骤（公钥、端点、CI 签名）见 docs/specifications.md SPEC-DEV-2。
 */
import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { ElMessage, ElMessageBox } from 'element-plus'
import { check, type DownloadEvent, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { i18n } from '../i18n'
import { logger } from '../utils/logger'
import { usePreferencesStore } from './preferences'

/** 非组件环境取文案（约定：store/工具用 i18n.global.t） */
function t(key: string, named?: Record<string, unknown>): string {
  return i18n.global.t(key, named ?? {})
}

export type UpdatePhase = 'idle' | 'checking' | 'downloading' | 'installed'
/** update=有新版 / latest=已最新 / disabled=未启用 / error=检测失败 / busy=进行中 */
export type CheckOutcome = 'update' | 'latest' | 'disabled' | 'error' | 'busy'

/** 插件在端点/公钥缺失时抛出的错误文案 → 与网络失败区分，避免误导提示 */
function looksUnconfigured(message: string): boolean {
  const m = message.toLowerCase()
  return ['endpoint', 'pubkey', 'public key', 'not configured'].some((k) => m.includes(k))
}

export const useUpdateStore = defineStore('update', () => {
  const prefs = usePreferencesStore()

  const phase = ref<UpdatePhase>('idle')
  /** 可用新版本号（空 = 无） */
  const version = ref('')
  /** 新版本更新说明（来自 latest.json 的 notes） */
  const notes = ref('')
  /** 下载进度 0-100；-1 表示服务端未返回总大小 */
  const progress = ref(0)
  /** false = 端点/公钥未配置（启动检测静默跳过，不打扰用户） */
  const configured = ref(true)
  const lastError = ref('')

  /** 待安装的更新句柄：含内部资源，不参与响应式 */
  let pending: Update | null = null

  const busy = computed(() => phase.value === 'checking' || phase.value === 'downloading')
  /** 是否有值得提示的新版本（用户已跳过的版本不再打扰） */
  const hasUpdate = computed(
    () => !!version.value && prefs.update.skippedVersion !== version.value,
  )
  /** 下载进度文案：有总大小时显示百分比，否则只显示「正在下载」 */
  const progressLabel = computed(() =>
    progress.value >= 0 ? t('update.progress', { percent: progress.value }) : t('update.downloading'),
  )

  /** 检测更新；silent=true 用于启动自动检测（不弹窗、失败仅记日志） */
  async function checkForUpdate(silent = false): Promise<CheckOutcome> {
    if (busy.value) return 'busy'
    phase.value = 'checking'
    lastError.value = ''
    try {
      const found = await check()
      phase.value = 'idle'
      configured.value = true
      if (!found) {
        version.value = ''
        notes.value = ''
        if (!silent) logger.info('[updater] already up to date')
        return 'latest'
      }
      pending = found
      version.value = found.version
      notes.value = found.body ?? ''
      logger.info(`[updater] available: v${found.version} (current v${found.currentVersion})`)
      return 'update'
    } catch (e) {
      phase.value = 'idle'
      lastError.value = String(e)
      configured.value = !looksUnconfigured(lastError.value)
      logger.warn(`[updater] check failed: ${lastError.value}`)
      return configured.value ? 'error' : 'disabled'
    }
  }

  /** 手动检测（帮助菜单 / 首选项）：给出一次性反馈，命中新版本则进入确认流程 */
  async function runManualCheck(): Promise<void> {
    const outcome = await checkForUpdate(false)
    if (outcome === 'latest') ElMessage.info(t('update.latest'))
    else if (outcome === 'disabled') ElMessage.info(t('update.notConfigured'))
    else if (outcome === 'error') ElMessage.error(t('update.checkFailed', { msg: lastError.value }))
    else if (outcome === 'update') await promptInstall()
  }

  /** 下载并安装（进度写入 progress），返回是否安装完成 */
  async function downloadAndInstall(): Promise<boolean> {
    if (!pending) return false
    phase.value = 'downloading'
    progress.value = 0
    let total = 0
    let received = 0
    try {
      await pending.downloadAndInstall((ev: DownloadEvent) => {
        if (ev.event === 'Started') {
          total = ev.data.contentLength ?? 0
          progress.value = total > 0 ? 0 : -1
        } else if (ev.event === 'Progress') {
          received += ev.data.chunkLength
          progress.value = total > 0 ? Math.min(100, Math.round((received / total) * 100)) : -1
        } else {
          progress.value = 100
        }
      })
      phase.value = 'installed'
      logger.info(`[updater] v${version.value} installed, restart required`)
      // 已安装：清空「有新版本」状态，状态栏提示随之消失（重启后即为新版本）
      pending = null
      version.value = ''
      notes.value = ''
      progress.value = 0
      return true
    } catch (e) {
      phase.value = 'idle'
      lastError.value = String(e)
      logger.error(`[updater] download/install failed: ${lastError.value}`)
      ElMessage.error(t('update.downloadFailed', { msg: lastError.value }))
      return false
    }
  }

  /** 确认 → 下载安装 → 询问重启（非模态提示点击后也走这里） */
  async function promptInstall(): Promise<void> {
    if (!pending || busy.value) return
    const go = await ElMessageBox.confirm(
      t('update.availableDesc', {
        version: version.value,
        notes: notes.value.trim() || t('update.noNotes'),
      }),
      t('update.available'),
      {
        confirmButtonText: t('update.download'),
        cancelButtonText: t('common.cancel'),
        type: 'info',
      },
    )
      .then(() => true)
      .catch(() => false)
    if (!go) return
    if (!(await downloadAndInstall())) return
    const restart = await ElMessageBox.confirm(
      t('update.installedDesc'),
      t('update.installed'),
      {
        confirmButtonText: t('update.restart'),
        cancelButtonText: t('update.later'),
        type: 'success',
      },
    )
      .then(() => true)
      .catch(() => false)
    if (restart) await relaunch()
  }

  /** 不再提示当前版本（非模态提示上的关闭按钮） */
  function skipCurrentVersion(): void {
    if (version.value) prefs.skipVersion(version.value)
  }

  return {
    phase,
    version,
    notes,
    progress,
    configured,
    lastError,
    busy,
    hasUpdate,
    progressLabel,
    checkForUpdate,
    runManualCheck,
    downloadAndInstall,
    promptInstall,
    skipCurrentVersion,
  }
})

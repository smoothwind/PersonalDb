/**
 * 更新提示（查 GitHub Releases 元数据，不做签名校验与应用内安装）。
 *
 * **无自动检测**：只在用户手动触发时发起请求——
 * - 帮助菜单「检查更新」（`runManualCheck`）
 * - 首选项「关于 → 更新提示 → 检查更新」
 *
 * 请求 `repos/{owner}/{repo}/releases/latest`（**草稿与预发布不计入**），用
 * `utils/version.ts#compareVersions` 与当前应用版本（`getVersion()`）比较；命中更高版本后
 * 给出**非模态**提示：状态栏常驻入口（点击用浏览器打开发布页）+ 帮助菜单红点，
 * 可「不再提示此版本」（`personaldb.update`）。
 *
 * 仓库为私有或尚无 Release 时接口返回 404 → 按查询失败提示。
 * 规格见 docs/specifications.md SPEC-DEV-2。
 */
import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { ElMessage } from 'element-plus'
import { getVersion } from '@tauri-apps/api/app'
import { openUrl } from '@tauri-apps/plugin-opener'
import { i18n } from '../i18n'
import { logger } from '../utils/logger'
import { compareVersions } from '../utils/version'
import { usePreferencesStore } from './preferences'

/** 发布仓库（与 README/发布流程一致，换仓库时同步修改） */
export const GITHUB_REPO = 'smoothwind/PersonalDb'
/** 最近一次**已发布**的 Release 接口（草稿/预发布不返回） */
const LATEST_RELEASE_API = `https://api.github.com/repos/${GITHUB_REPO}/releases/latest`
/** 发布列表页（无具体 Release 时的兜底入口） */
const RELEASES_PAGE = `https://github.com/${GITHUB_REPO}/releases`

/** 检测状态：idle=未检测 / busy=查询中 / update=有新版 / latest=已最新 / error=查询失败 */
export type CheckOutcome = 'idle' | 'busy' | 'update' | 'latest' | 'error'

/** GitHub Release 中本功能用到的字段 */
interface ReleaseInfo {
  tag_name?: string
  html_url?: string
}

/** 非组件环境取文案（约定：store/工具用 i18n.global.t） */
function t(key: string, named?: Record<string, unknown>): string {
  return i18n.global.t(key, named ?? {})
}

export const useUpdateStore = defineStore('update', () => {
  const prefs = usePreferencesStore()

  /** 最近一次检测状态 */
  const outcome = ref<CheckOutcome>('idle')
  /** 查询到的可用新版本号（空 = 无） */
  const version = ref('')
  /** 该版本对应的发布页地址 */
  const releaseUrl = ref('')
  /** 最近一次查询失败原因（空 = 无） */
  const lastError = ref('')
  /** 当前应用版本（首次查询时读取，取自 tauri.conf.json） */
  const currentVersion = ref('')

  const busy = computed(() => outcome.value === 'busy')
  /** 是否有值得提示的新版本（用户已跳过的版本不再打扰） */
  const hasUpdate = computed(
    () => !!version.value && prefs.update.skippedVersion !== version.value,
  )
  /** 待打开的地址：有新版本 → 该版本 Release；否则 → 发布列表页 */
  const releaseLink = computed(() => releaseUrl.value || RELEASES_PAGE)

  /** 查 GitHub 是否有更高的已发布版本（只手动触发，失败不重试） */
  async function checkForUpdate(): Promise<CheckOutcome> {
    if (busy.value) return 'busy'
    outcome.value = 'busy'
    lastError.value = ''
    try {
      if (!currentVersion.value) currentVersion.value = await getVersion()
      const res = await fetch(LATEST_RELEASE_API, {
        headers: { Accept: 'application/vnd.github+json' },
      })
      if (res.status === 404) throw new Error(t('update.noRelease'))
      if (!res.ok) throw new Error(`HTTP ${res.status} ${res.statusText}`)
      const info = (await res.json()) as ReleaseInfo
      const tag = (info.tag_name ?? '').trim()
      if (!tag) throw new Error(t('update.noRelease'))
      if (compareVersions(tag, currentVersion.value) > 0) {
        version.value = tag.replace(/^v/i, '')
        releaseUrl.value = info.html_url ?? RELEASES_PAGE
        outcome.value = 'update'
        logger.info(`[update] available: ${tag} (current v${currentVersion.value})`)
      } else {
        version.value = ''
        releaseUrl.value = ''
        outcome.value = 'latest'
        logger.info(`[update] up to date: latest ${tag}, current v${currentVersion.value}`)
      }
      return outcome.value
    } catch (e) {
      version.value = ''
      releaseUrl.value = ''
      lastError.value = e instanceof Error ? e.message : String(e)
      outcome.value = 'error'
      logger.warn(`[update] check failed: ${lastError.value}`)
      return 'error'
    }
  }

  /** 手动检测（帮助菜单 / 首选项）：一次性反馈，命中新版本后由状态栏常驻提示 */
  async function runManualCheck(): Promise<void> {
    const result = await checkForUpdate()
    if (result === 'latest') ElMessage.info(t('update.latest'))
    else if (result === 'error') ElMessage.error(t('update.checkFailed', { msg: lastError.value }))
    else if (result === 'update') ElMessage.success(t('update.availableToast', { version: version.value }))
  }

  /** 在浏览器打开发布页（有新版本 → 该版本 Release；否则 → 发布列表） */
  async function openRelease(): Promise<void> {
    try {
      await openUrl(releaseLink.value)
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      logger.warn(`[update] open release page failed: ${msg}`)
      ElMessage.error(t('update.openFailed', { msg }))
    }
  }

  /** 不再提示当前版本（状态栏提示上的关闭按钮） */
  function skipCurrentVersion(): void {
    if (version.value) prefs.skipVersion(version.value)
  }

  return {
    outcome,
    version,
    releaseUrl,
    lastError,
    currentVersion,
    busy,
    hasUpdate,
    releaseLink,
    checkForUpdate,
    runManualCheck,
    openRelease,
    skipCurrentVersion,
  }
})

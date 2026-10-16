<script setup lang="ts">
/**
 * 检查更新（tauri-plugin-updater）。
 * 未配置更新端点/公钥时 check() 会报错 → 提示「未启用」，不影响其他功能。
 * 启用步骤见 docs/specifications.md SPEC-DEV-2。
 */
import { useI18n } from 'vue-i18n'
import { ElMessageBox } from 'element-plus'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { logger } from '../utils/logger'

const { t } = useI18n()

export interface UpdateResult {
  status: 'latest' | 'updated' | 'disabled' | 'error'
  message: string
  version?: string
}

async function checkForUpdate(): Promise<UpdateResult> {
  let update: Update | null = null
  try {
    update = await check()
  } catch (e) {
    logger.warn(`[updater] check failed: ${String(e)}`)
    return { status: 'disabled', message: t('update.notConfigured') }
  }
  if (!update) {
    return { status: 'latest', message: t('update.latest') }
  }
  const ok = await ElMessageBox.confirm(
    t('update.availableDesc', { version: update.version, notes: update.body ?? '' }),
    t('update.available'),
    { confirmButtonText: t('update.download'), cancelButtonText: t('common.cancel'), type: 'info' },
  ).then(() => true).catch(() => false)
  if (!ok) return { status: 'latest', message: t('update.latest') }
  try {
    await update.downloadAndInstall()
  } catch (e) {
    return { status: 'error', message: t('update.downloadFailed', { msg: String(e) }) }
  }
  const restart = await ElMessageBox.confirm(
    t('update.installedDesc'),
    t('update.installed'),
    { confirmButtonText: t('update.restart'), cancelButtonText: t('update.later'), type: 'success' },
  ).then(() => true).catch(() => false)
  if (restart) await relaunch()
  return { status: 'updated', message: t('update.installedDesc'), version: update.version }
}

defineExpose({ checkForUpdate })
</script>

<template>
  <span />
</template>

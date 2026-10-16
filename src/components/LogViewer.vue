<template>
  <el-dialog
    :model-value="modelValue"
    width="920px"
    top="5vh"
    destroy-on-close
    @update:model-value="emit('update:modelValue', $event)"
    @open="onOpen"
    @close="onClose"
  >
    <template #header>
      <span class="dlg-title">
        <el-icon><component :is="objectIcon('act_monitor')" /></el-icon>
        {{ t('logViewer.title') }}
      </span>
    </template>
    <div class="log-toolbar">
      <el-select v-model="level" size="small" style="width: 110px">
        <el-option value="" :label="t('logViewer.allLevels')" />
        <el-option value="ERROR" label="ERROR" />
        <el-option value="WARN" label="WARN" />
        <el-option value="INFO" label="INFO" />
        <el-option value="DEBUG" label="DEBUG" />
      </el-select>
      <el-input
        v-model="keyword"
        size="small"
        clearable
        :prefix-icon="Search"
        :placeholder="t('logViewer.filter')"
        style="width: 200px"
      />
      <span class="log-label">{{ t('logViewer.tail') }}</span>
      <el-input-number
        v-model="limit"
        size="small"
        :min="100"
        :max="5000"
        :step="100"
        controls-position="right"
        style="width: 120px"
        @change="load"
      />
      <el-checkbox v-model="autoRefresh" size="small">{{ t('logViewer.autoRefresh') }}</el-checkbox>
      <span class="log-count">{{ t('logViewer.lineCount', { n: filteredLines.length }) }}</span>
      <div class="log-spacer" />
      <el-button size="small" @click="load">
        <el-icon><component :is="objectIcon('act_refresh')" /></el-icon>
        {{ t('common.refresh') }}
      </el-button>
      <el-button size="small" :icon="CopyDocument" @click="copy">{{ t('cell.copy') }}</el-button>
      <el-button size="small" :icon="FolderOpened" @click="openDir">
        {{ t('logViewer.openDir') }}
      </el-button>
      <el-button size="small" type="danger" :icon="Delete" @click="clear">
        {{ t('logViewer.clear') }}
      </el-button>
    </div>

    <div class="log-body" v-loading="loading">
      <div v-if="filteredLines.length === 0" class="log-empty">{{ t('logViewer.empty') }}</div>
      <div v-for="(line, i) in filteredLines" :key="i" class="log-line" :class="lineClass(line)">
        {{ line }}
      </div>
    </div>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { CopyDocument, Delete, FolderOpened, Search } from '@element-plus/icons-vue'
import { objectIcon } from '../utils/objectIcons'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { logClear, logRead, storageOpenDir } from '../api/db'

defineProps<{ modelValue: boolean }>()

const emit = defineEmits<{ (e: 'update:modelValue', v: boolean): void }>()

const { t } = useI18n()

const loading = ref(false)
const raw = ref('')
const level = ref('')
const keyword = ref('')
const limit = ref(1000)
const autoRefresh = ref(false)
let timer: number | undefined

const filteredLines = computed(() => {
  let lines = raw.value.split('\n').filter((l) => l.length > 0)
  if (level.value) lines = lines.filter((l) => l.includes(`[${level.value}]`))
  const kw = keyword.value.trim().toLowerCase()
  if (kw) lines = lines.filter((l) => l.toLowerCase().includes(kw))
  return lines
})

function lineClass(line: string): string {
  if (line.includes('[ERROR]') || line.includes('PANIC')) return 'is-error'
  if (line.includes('[WARN]')) return 'is-warn'
  if (line.includes('[DEBUG]') || line.includes('[TRACE]')) return 'is-debug'
  return ''
}

async function load(): Promise<void> {
  loading.value = true
  try {
    raw.value = await logRead(limit.value)
  } catch (e) {
    ElMessage.error(String(e))
  } finally {
    loading.value = false
  }
}

function onOpen(): void {
  void load()
  startAuto()
}

function onClose(): void {
  stopAuto()
  raw.value = ''
}

function startAuto(): void {
  stopAuto()
  if (!autoRefresh.value) return
  timer = window.setInterval(() => void load(), 2000)
}

function stopAuto(): void {
  window.clearInterval(timer)
  timer = undefined
}

// 自动刷新开关变化时重启定时器
watch(autoRefresh, (on) => {
  if (on) startAuto()
  else stopAuto()
})

onBeforeUnmount(stopAuto)

async function copy(): Promise<void> {
  await navigator.clipboard.writeText(filteredLines.value.join('\n'))
  ElMessage.success(t('cell.copied'))
}

function openDir(): void {
  void storageOpenDir('logs')
    .then((path) => revealItemInDir(path))
    .catch((e) => ElMessage.error(String(e)))
}

async function clear(): Promise<void> {
  try {
    await ElMessageBox.confirm(t('logViewer.clearConfirm'), t('logViewer.clear'), {
      type: 'warning',
    })
  } catch {
    return
  }
  await logClear()
  await load()
  ElMessage.success(t('logViewer.cleared'))
}
</script>

<style scoped>
.log-toolbar {
.log-toolbar :deep(.el-icon) {
  font-size: var(--pd-icon-sm);
}

  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  flex-wrap: wrap;
}

.log-label {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.log-count {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.log-spacer {
  flex: 1;
}

.log-body {
  height: 62vh;
  overflow: auto;
  background: var(--el-fill-color-lighter);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 4px;
  padding: 8px 10px;
  font-family: Consolas, 'Cascadia Mono', monospace;
  font-size: 12px;
  line-height: 1.6;
}

.log-line {
  white-space: pre-wrap;
  word-break: break-all;
  color: var(--el-text-color-regular);
}

.log-line.is-error {
  color: var(--el-color-danger);
}

.log-line.is-warn {
  color: var(--el-color-warning);
}

.log-line.is-debug {
  color: var(--el-text-color-placeholder);
}

.log-empty {
  color: var(--el-text-color-placeholder);
  text-align: center;
  padding: 40px 0;
}
</style>

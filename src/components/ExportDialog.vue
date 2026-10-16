<template>
  <el-dialog
    :model-value="modelValue"
    width="560px"
    destroy-on-close
    @update:model-value="emit('update:modelValue', $event)"
  >
    <template #header>
      <span class="dlg-title">
        <el-icon><component :is="objectIcon('act_export')" /></el-icon>
        {{ t('export.title', { table }) }}
      </span>
    </template>
    <el-form label-width="90px" size="small">
      <el-form-item :label="t('export.format')">
        <el-radio-group v-model="format">
          <el-radio-button value="sql">{{ t('export.sql') }}</el-radio-button>
          <el-radio-button value="csv">{{ t('export.csv') }}</el-radio-button>
          <el-radio-button value="excel">{{ t('export.excel') }}</el-radio-button>
        </el-radio-group>
      </el-form-item>

      <template v-if="format === 'sql'">
        <el-form-item :label="t('export.content')">
          <el-checkbox v-model="includeDdl">{{ t('export.includeDdl') }}</el-checkbox>
          <el-checkbox v-model="includeData">{{ t('export.includeData') }}</el-checkbox>
        </el-form-item>
      </template>

      <el-form-item :label="t('export.range')">
        <el-input-number
          v-model="maxRows"
          :min="0"
          :step="10000"
          controls-position="right"
          style="width: 200px"
        />
        <span class="range-hint">{{ t('export.rangeHint') }}</span>
      </el-form-item>

      <el-form-item :label="t('export.where')">
        <el-input
          v-model="whereClause"
          type="textarea"
          :rows="2"
          :placeholder="t('export.wherePlaceholder')"
        />
      </el-form-item>

      <el-form-item :label="t('export.target')">
        <el-button @click="choosePath">{{ t('export.chooseFile') }}</el-button>
        <span class="path-text" :title="path">{{ path || t('export.notChosen') }}</span>
      </el-form-item>
    </el-form>

    <template #footer>
      <el-button @click="emit('update:modelValue', false)">{{ t('common.cancel') }}</el-button>
      <el-button type="primary" :loading="exporting" :disabled="!path" @click="doExport">
        {{ t('common.export') }}
      </el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { objectIcon } from '../utils/objectIcons'
import { ElMessage } from 'element-plus'
import { save } from '@tauri-apps/plugin-dialog'
import { dbExport, type ExportSummary } from '../api/db'
import { useConnectionStore } from '../stores/connections'
import { useWorkspaceStore } from '../stores/workspaces'
import type { DbKind } from '../types/db'

const props = defineProps<{
  modelValue: boolean
  connectionId: string
  kind: DbKind
  database: string
  table: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
}>()

const { t } = useI18n()
const connStore = useConnectionStore()
const workspaceStore = useWorkspaceStore()

const format = ref<'sql' | 'csv' | 'excel'>('sql')
const includeDdl = ref(true)
const includeData = ref(true)
const maxRows = ref(100000)
const whereClause = ref('')
const path = ref('')
const exporting = ref(false)
const summary = ref<ExportSummary | null>(null)

const defaultName = computed(() => {
  const ext = format.value === 'excel' ? 'xlsx' : format.value
  return `${props.table}.${ext}`
})

async function choosePath(): Promise<void> {
  const dir = workspaceStore.current?.exportDir
  const defaultPath = dir ? `${dir}\\${defaultName.value}` : defaultName.value
  const selected = await save({
    defaultPath,
    filters: [
      {
        name: format.value === 'excel' ? 'Excel' : format.value.toUpperCase(),
        extensions: [format.value === 'excel' ? 'xlsx' : format.value],
      },
    ],
  })
  if (selected) path.value = selected
}

async function doExport(): Promise<void> {
  const session = connStore.sessionOf(props.connectionId)
  if (!session) {
    ElMessage.error(t('export.connectionLost'))
    return
  }
  exporting.value = true
  try {
    summary.value = await dbExport({
      sessionId: session.sessionId,
      database: props.database || null,
      table: props.table,
      format: format.value,
      includeDdl: format.value === 'sql' && includeDdl.value,
      includeData: format.value === 'sql' ? includeData.value : true,
      whereClause: whereClause.value.trim() || null,
      maxRows: maxRows.value,
      path: path.value,
    })
    ElMessage.success(
      t('export.done', {
        rows: summary.value.rows,
        kb: (summary.value.bytes / 1024).toFixed(1),
        ms: summary.value.durationMs,
      }),
    )
    emit('update:modelValue', false)
  } catch (e) {
    ElMessage.error(t('export.failed', { msg: String(e) }))
  } finally {
    exporting.value = false
  }
}
</script>

<style scoped>
.range-hint {
  margin-left: 10px;
  color: var(--el-text-color-secondary);
  font-size: 12px;
}

.path-text {
  margin-left: 10px;
  color: var(--el-text-color-secondary);
  font-size: 12px;
  max-width: 280px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  display: inline-block;
  vertical-align: middle;
}
</style>

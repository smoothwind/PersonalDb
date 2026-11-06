<template>
  <el-dialog
    :model-value="modelValue"
    :title="t('routineExec.title')"
    width="720px"
    append-to-body
    @update:model-value="emit('update:modelValue', $event)"
    @open="resetArgs"
  >
    <div v-if="routine" class="rex-meta">
      <span class="rex-name">{{ routine.name }}</span>
      <el-tag size="small" effect="plain">{{ routine.routineType }}</el-tag>
      <span v-if="routine.returnType" class="rex-ret">{{ t('routine.returns') }}: <code>{{ routine.returnType }}</code></span>
    </div>

    <template v-if="routine && routine.params.length > 0">
      <el-table :data="rows" size="small" class="rex-table">
        <el-table-column :label="t('routine.paramName')" min-width="110">
          <template #default="{ row }">
            <code>{{ row.param.name || `#${row.index + 1}` }}</code>
          </template>
        </el-table-column>
        <el-table-column :label="t('routine.paramMode')" width="72">
          <template #default="{ row }">
            <el-tag size="small" :type="row.param.mode === 'IN' ? 'info' : 'warning'" effect="plain">
              {{ row.param.mode }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column :label="t('routine.paramType')" min-width="130">
          <template #default="{ row }">
            <code>{{ row.param.dataType }}</code>
          </template>
        </el-table-column>
        <el-table-column :label="t('routineExec.value')" min-width="150">
          <template #default="{ row }">
            <el-input
              v-model="row.arg.text"
              size="small"
              :placeholder="t('routineExec.valuePlaceholder')"
              :disabled="row.arg.isNull"
            />
          </template>
        </el-table-column>
        <el-table-column :label="t('routineExec.null')" width="56" align="center">
          <template #default="{ row }">
            <el-checkbox v-model="row.arg.isNull" size="small" />
          </template>
        </el-table-column>
        <el-table-column width="56" align="center">
          <template #header>
            <el-tooltip :content="t('routineExec.rawTip')" placement="top">
              <span class="rex-raw-h">{{ t('routineExec.raw') }}</span>
            </el-tooltip>
          </template>
          <template #default="{ row }">
            <el-checkbox v-model="row.arg.raw" size="small" :disabled="row.arg.isNull" />
          </template>
        </el-table-column>
      </el-table>
      <div class="rex-hint">{{ t('routineExec.valueHint') }}</div>
    </template>
    <EmptyState v-else-if="routine" icon="obj_column" :title="t('routine.noParams')" />

    <template #footer>
      <el-button size="small" @click="emit('update:modelValue', false)">{{ t('common.cancel') }}</el-button>
      <el-button size="small" type="primary" @click="onExecute">
        <el-icon><component :is="objectIcon('act_run')" /></el-icon>
        {{ t('routine.exec') }}
      </el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { reactive, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import EmptyState from './EmptyState.vue'
import { objectIcon } from '../utils/objectIcons'
import type { DbKind, RoutineInfo } from '../types/db'
import { buildRoutineCallSql, emptyArg, type RoutineArgInput, type RoutineCallSql } from '../utils/routineExec'

/**
 * 例程执行参数对话框（SPEC-ROUTINE-1）：收集参数 → 方言调用 SQL → 交父级开查询页签执行。
 * 「执行」即提交（生成 SQL 并关闭），父级负责打开查询页签并自动运行。
 */
const props = defineProps<{
  modelValue: boolean
  kind: DbKind
  routine: RoutineInfo | null
  /** 库 / schema（限定名前缀，可空） */
  schema?: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  (e: 'run', call: RoutineCallSql): void
}>()

const { t } = useI18n()

interface ExecRow {
  param: RoutineInfo['params'][number]
  index: number
  arg: RoutineArgInput
}
const rows = reactive<ExecRow[]>([])

function resetArgs(): void {
  rows.splice(
    0,
    rows.length,
    ...(props.routine?.params ?? []).map((param, index) => ({ param, index, arg: emptyArg() })),
  )
}

// 关闭时清空输入，避免下次打开残留上次值（@open 再兜底一次）
watch(
  () => props.modelValue,
  (v) => {
    if (!v) resetArgs()
  },
)

function onExecute(): void {
  if (!props.routine) return
  const call = buildRoutineCallSql(props.kind, props.routine, rows.map((r) => r.arg), props.schema || null)
  if (call.outNoteKey) ElMessage.warning(t(call.outNoteKey))
  emit('run', call)
  emit('update:modelValue', false)
}
</script>

<style scoped>
.rex-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}
.rex-name {
  font-weight: 600;
}
.rex-ret {
  color: var(--el-text-color-secondary);
  font-size: 12px;
}
.rex-table :deep(.cell) {
  padding: 0 6px;
}
.rex-raw-h {
  cursor: help;
}
.rex-hint {
  margin-top: 8px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
</style>

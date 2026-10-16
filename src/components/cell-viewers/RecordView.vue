<template>
  <div class="record-view">
    <div class="record-toolbar">
      <el-button-group>
        <el-button size="small" :disabled="current <= 0" @click="current -= 1">
          <el-icon><component :is="objectIcon('act_prev')" /></el-icon>
        </el-button>
        <el-button
          size="small"
          :disabled="current >= rows.length - 1"
          @click="current += 1"
        >
          <el-icon><component :is="objectIcon('act_next')" /></el-icon>
        </el-button>
      </el-button-group>
      <span class="record-pos">
        <input
          class="pos-input"
          type="number"
          :value="current + 1"
          min="1"
          :max="rows.length"
          @change="onPosChange"
        />
        <span class="pos-total">/ {{ rows.length }}</span>
      </span>
      <span class="record-dim">{{ t('record.fields', { n: shownColumns.length }) }}</span>
      <el-input
        v-model="fieldKw"
        size="small"
        clearable
        :prefix-icon="Search"
        :placeholder="t('record.filterFields')"
        class="field-filter"
      />
      <div class="spacer" />
      <el-button size="small" :icon="CopyDocument" @click="copyRecord">
        {{ t('record.copyRecord') }}
      </el-button>
    </div>

    <div class="record-table-wrap">
      <table class="field-table">
        <tbody>
          <tr v-for="col in shownColumns" :key="col.prop" class="field-row">
            <td class="f-name" :title="col.typeLabel">
              <span class="f-name-text">{{ col.name }}</span>
              <span v-if="col.typeLabel" class="f-type">{{ col.typeLabel }}</span>
            </td>
            <td
              class="f-value"
              :class="{ 'is-number': isNum(col), 'is-null': currentValues[col.prop] == null }"
              @dblclick="openDetail(col)"
            >
              <div class="f-value-inner">
                <CellDisplay
                  :value="currentValues[col.prop] ?? null"
                  :semantic="col.semantic"
                  :format="col.format"
                  :compact="false"
                />
                <el-button
                  class="f-copy"
                  link
                  size="small"
                  :icon="CopyDocument"
                  :title="t('common.copy')"
                  @click="copyField(col)"
                />
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { CopyDocument, Search } from '@element-plus/icons-vue'
import { objectIcon } from '../../utils/objectIcons'
import CellDisplay from './CellDisplay.vue'
import type { CellFormat } from './CellDisplay.vue'
import { cellToText, copyText } from '../../utils/clipboard'
import type { CellValue, SemanticType } from '../../types/db'

/** 列定义（含渲染 prop，来自结果表格的列计算结果） */
export interface RecordColumn {
  name: string
  /** 行数据中的取值键（重名列已加序号） */
  prop: string
  typeLabel?: string
  semantic?: SemanticType
  format?: CellFormat
}

const props = defineProps<{
  columns: RecordColumn[]
  /** 行数据已按 prop 键对象化 */
  rows: Array<Record<string, CellValue>>
  /** 初始定位行（0 基），用于从表格跳转 */
  startIndex?: number
}>()

const emit = defineEmits<{
  (e: 'detail', payload: { col: RecordColumn; value: CellValue }): void
}>()

const { t } = useI18n()
const current = ref(0)

/** 从表格跳转：定位到指定行 */
watch(
  () => props.startIndex,
  (v) => {
    if (typeof v === 'number' && v >= 0) {
      current.value = Math.min(Math.max(0, v), Math.max(0, props.rows.length - 1))
    }
  },
  { immediate: true },
)

const currentValues = computed(() => props.rows[current.value] ?? {})

/** 字段过滤（按列名包含匹配） */
const fieldKw = ref('')
const shownColumns = computed(() => {
  const k = fieldKw.value.trim().toLowerCase()
  if (!k) return props.columns
  return props.columns.filter((c) => c.name.toLowerCase().includes(k))
})

function isNum(col: RecordColumn): boolean {
  return col.semantic === 'number' || col.semantic === 'decimal'
}

function onPosChange(e: Event): void {
  const v = Number((e.target as HTMLInputElement).value)
  if (!Number.isFinite(v)) return
  current.value = Math.min(props.rows.length, Math.max(1, Math.round(v))) - 1
}

function openDetail(col: RecordColumn): void {
  emit('detail', { col, value: currentValues.value[col.prop] ?? null })
}

async function copyField(col: RecordColumn): Promise<void> {
  await copyText(cellToText(currentValues.value[col.prop]))
}

async function copyRecord(): Promise<void> {
  const row = currentValues.value
  const lines = props.columns.map((c) => `${c.name}\t${cellToText(row[c.prop])}`)
  await copyText(lines.join('\n'))
}
</script>

<style scoped>
.record-view {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 8px;
  overflow: hidden;
}

.record-toolbar {
.record-toolbar :deep(.el-icon) {
  font-size: var(--pd-icon-sm);
}

  display: flex;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
}

.record-pos {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  font-variant-numeric: tabular-nums;
}

.pos-input {
  width: 48px;
  height: 22px;
  text-align: center;
  font-size: 12px;
  color: var(--el-text-color-primary);
  background: var(--el-fill-color-blank);
  border: 1px solid var(--el-border-color);
  border-radius: 4px;
  outline: none;
  -moz-appearance: textfield;
}

.pos-input::-webkit-outer-spin-button,
.pos-input::-webkit-inner-spin-button {
  -webkit-appearance: none;
  margin: 0;
}

.pos-input:focus {
  border-color: var(--el-color-primary);
}

.record-dim {
  font-size: 12px;
  color: var(--el-text-color-placeholder);
}

.field-filter {
  width: 150px;
}

.spacer {
  flex: 1;
}

.record-table-wrap {
  flex: 1;
  min-height: 0;
  overflow: auto;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  background: var(--el-fill-color-blank);
}

.field-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}

.field-row {
  border-bottom: 1px solid var(--el-border-color-extra-light);
}

.field-row:last-child {
  border-bottom: none;
}

.field-row:hover {
  background: var(--el-fill-color-light);
}

.f-name {
  width: 220px;
  min-width: 160px;
  padding: 6px 10px;
  vertical-align: top;
  background: var(--el-fill-color-light);
  border-right: 1px solid var(--el-border-color-lighter);
  user-select: none;
}

.f-name-text {
  font-weight: 600;
  word-break: break-all;
}

.f-type {
  display: block;
  margin-top: 1px;
  font-weight: 400;
  font-size: 10px;
  color: var(--el-text-color-secondary);
}

.f-value {
  padding: 0;
  vertical-align: top;
}

.f-value-inner {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  padding: 6px 8px 6px 10px;
}

.f-value-inner > :first-child {
  flex: 1;
  min-width: 0;
  overflow-wrap: break-word;
  word-break: break-word;
  font-family: var(--app-mono-font, Consolas, 'Courier New', monospace);
}

.f-copy {
  flex-shrink: 0;
  opacity: 0;
  transition: opacity 0.15s;
}

.field-row:hover .f-copy {
  opacity: 1;
}

.f-value.is-number .f-value-inner > :first-child {
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.f-value.is-null .f-value-inner > :first-child {
  color: var(--el-text-color-placeholder);
  font-style: italic;
}
</style>

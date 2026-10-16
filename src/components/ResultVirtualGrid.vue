<template>
  <div class="rv-grid" @contextmenu.prevent>
    <el-auto-resizer>
      <template #default="{ width, height }">
        <el-table-v2
          :columns="vcols"
          :data="sortedRows"
          :width="width"
          :height="height"
          :row-height="28"
          :header-height="34"
          fixed
          class="rv-table"
        />
      </template>
    </el-auto-resizer>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, h, type FunctionalComponent } from 'vue'
import { Sort } from '@element-plus/icons-vue'
import type { SemanticType } from '../types/db'

export interface VGridColumn {
  name: string
  semantic: SemanticType
  /** 该列在源矩阵中的下标（零拷贝行取值）；缺省用列下标 */
  srcIndex?: number
}

const props = defineProps<{
  /** 行为源矩阵行的引用切片（非投影拷贝） */
  columns: VGridColumn[]
  rows: unknown[][]
}>()

const emit = defineEmits<{
  (e: 'cellMenu', payload: { rowIndex: number; colIndex: number; value: unknown; x: number; y: number }): void
  (e: 'rowCopy', rowIndex: number): void
}>()

const sortKey = ref('')
const sortOrder = ref<'asc' | 'desc'>('asc')

function widthFor(c: VGridColumn): number {
  switch (c.semantic) {
    case 'number':
    case 'decimal':
    case 'bool':
      return 110
    case 'date':
    case 'time':
      return 150
    case 'datetime':
      return 170
    case 'longtext':
    case 'json':
      return 300
    default:
      return 190
  }
}

function display(v: unknown): string {
  if (v === null || v === undefined) return ''
  if (typeof v === 'object') return JSON.stringify(v)
  return String(v)
}

const sortedRows = computed(() => {
  const rows = props.rows
  if (!sortKey.value) return rows
  const colIdx = Number(sortKey.value)
  const src = props.columns[colIdx]?.srcIndex ?? colIdx
  const dir = sortOrder.value === 'asc' ? 1 : -1
  return [...rows].sort((a, b) => {
    const av = a[src]
    const bv = b[src]
    if (av === null || av === undefined) return -dir
    if (bv === null || bv === undefined) return dir
    if (typeof av === 'number' && typeof bv === 'number') return (av - bv) * dir
    return String(av).localeCompare(String(bv)) * dir
  })
})

function toggleSort(idx: number): void {
  if (sortKey.value === String(idx)) {
    sortOrder.value = sortOrder.value === 'asc' ? 'desc' : 'asc'
  } else {
    sortKey.value = String(idx)
    sortOrder.value = 'asc'
  }
}

interface CellRenderPayload {
  cellData: unknown
  column: { key?: string }
  rowIndex: number
}

const Cell: FunctionalComponent<{
  text: string
  isNull: boolean
  rowIndex: number
  colIndex: number
  value: unknown
}> = (p, { emit: cellEmit }) =>
  h(
    'div',
    {
      class: ['rv-cell', { 'rv-null': p.isNull }],
      title: p.text,
      onContextmenu: (ev: MouseEvent) => {
        ev.preventDefault()
        cellEmit('menu', { rowIndex: p.rowIndex, colIndex: p.colIndex, value: p.value, x: ev.clientX, y: ev.clientY })
      },
      onDblclick: () => {
        cellEmit('rowcopy', p.rowIndex)
      },
    },
    p.text,
  )
Cell.emits = ['menu', 'rowcopy']
Cell.props = ['text', 'isNull', 'rowIndex', 'colIndex', 'value']

const vcols = computed(() =>
  props.columns.map((c, colIndex) => {
    // dataKey 指向源矩阵下标：行是零拷贝引用切片，取值须按 srcIndex
    const src = c.srcIndex ?? colIndex
    return {
      key: String(colIndex),
      dataKey: String(src),
      title: c.name,
      width: widthFor(c),
      cellRenderer: (p: CellRenderPayload) => {
        const v = p.cellData
        const isNull = v === null || v === undefined
        return h(Cell, {
          text: display(v),
          isNull,
          rowIndex: p.rowIndex,
          colIndex,
          value: v,
          onMenu: (payload: { rowIndex: number; colIndex: number; value: unknown; x: number; y: number }) =>
            emit('cellMenu', payload),
          onRowcopy: (rowIndex: number) => emit('rowCopy', rowIndex),
        })
      },
      headerCellRenderer: (p: { column: { title?: string; key?: string } }) =>
        h(
          'div',
          {
            class: ['rv-head', { 'is-sorted': sortKey.value === p.column.key }],
            onClick: () => toggleSort(Number(p.column.key)),
          },
          [
            h('span', { class: 'rv-head-text' }, p.column.title ?? ''),
            h(Sort, { class: 'rv-head-sort', style: { width: '12px', height: '12px' } }),
          ],
        ),
    }
  }),
)
</script>

<style scoped>
.rv-grid {
  height: 100%;
  min-height: 0;
}
.rv-cell {
  height: 100%;
  line-height: 28px;
  padding: 0 10px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  cursor: default;
}
.rv-cell.rv-null {
  color: var(--el-text-color-placeholder);
  font-style: italic;
  font-size: 11px;
}
:deep(.rv-head) {
  display: flex;
  align-items: center;
  gap: 4px;
  cursor: pointer;
  font-size: 12px;
  font-weight: 600;
}
:deep(.rv-head-sort) {
  opacity: 0.35;
}
:deep(.rv-head.is-sorted .rv-head-sort) {
  opacity: 1;
  color: var(--el-color-primary);
}
</style>

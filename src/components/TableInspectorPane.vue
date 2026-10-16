<template>
  <div class="table-inspector">
    <div class="inspector-toolbar">
      <!-- 段1：对象身份 -->
      <el-tag size="small" effect="dark">{{ t('dbKind.' + tab.kind) }}</el-tag>
      <span class="ctx">{{ tab.database }}.{{ tab.table }}</span>
      <el-tag v-if="partitioned" size="small" type="warning" effect="plain">
        {{ t('tableInspector.partitioned') }}
      </el-tag>
      <span class="pt-sep" />
      <!-- 段3：动作 + 状态 -->
      <el-button size="small" :loading="loading" @click="loadAll">
        <el-icon><component :is="objectIcon('act_refresh')" /></el-icon>
        {{ t('common.refresh') }}
      </el-button>
      <el-button
        size="small"
        :type="editMode ? 'warning' : 'default'"
        :icon="EditPen"
        @click="toggleEdit"
      >
        {{ editMode ? t('common.cancel') : t('common.edit') }}
      </el-button>
      <span class="pt-spacer" />
      <el-tag v-if="pendingChanges > 0" size="small" type="warning">
        {{ t('tableInspector.pending', { n: pendingChanges }) }}
      </el-tag>
      <el-button
        size="small"
        type="primary"
        :icon="View"
        :disabled="!editMode || pendingChanges === 0"
        @click="previewChanges"
      >
        {{ t('tableInspector.generate') }}
      </el-button>
    </div>

    <el-tabs v-model="activeTab" size="small" class="inspector-tabs">
      <!-- 列 -->
      <el-tab-pane name="columns">
        <template #label>
          <span class="pane-label">
            <el-icon><Tickets /></el-icon>{{ t('tableInspector.columns') }}
            <el-tag size="small" type="info">{{ columns.length }}</el-tag>
          </span>
        </template>
        <div class="pane-tools">
          <el-input
            v-model="columnFilter"
            size="small"
            clearable
            :prefix-icon="Search"
            :placeholder="t('inspector.filterColumns')"
            class="filter-input"
          />
          <ColVisPopover
            :cols="inspPopoverCols"
            :hidden="inspHidden"
            @apply="inspApplyHidden"
            @reorder="inspReorder"
            @reset="inspReset"
          />
          <span class="pt-sep" />
          <el-button size="small" :icon="Plus" :disabled="!editMode" @click="addColumn">
            {{ t('tableInspector.addColumn') }}
          </el-button>
          <span class="pt-spacer" />
          <el-button size="small" :disabled="!props.tab.table" @click="previewData">
            {{ t('tree.queryRows') }}
          </el-button>
        </div>
        <el-table
          ref="inspTableRef"
          :data="filteredColumns"
          size="small"
          border
          :row-class-name="columnRowClass"
          max-height="calc(100vh - 300px)"
          :row-style="{ height: '30px' }"
          @header-dragend="inspHeaderResize"
          @header-contextmenu="inspHeaderMenu"
        >
          <el-table-column type="index" width="44" />
          <el-table-column
            v-for="d in inspVisibleDefs"
            :key="d.prop"
            :prop="d.prop"
            :label="t(d.labelKey)"
            :min-width="inspWidths[d.prop] || d.min"
            :width="d.width ? inspWidths[d.prop] || d.width : undefined"
            :align="d.align"
            show-overflow-tooltip
          >
            <template #default="{ row }">
              <el-input v-if="d.prop === 'name'" v-model="row.name" size="small" :disabled="!editMode" />
              <el-input v-else-if="d.prop === 'dataType'" v-model="row.dataType" size="small" :disabled="!editMode" />
              <el-checkbox v-else-if="d.prop === 'nullable'" v-model="row.nullable" :disabled="!editMode" />
              <el-input v-else-if="d.prop === 'defaultValue'" v-model="row.defaultValue" size="small" :disabled="!editMode" />
              <el-input v-else-if="d.prop === 'comment'" v-model="row.comment" size="small" :disabled="!editMode" />
              <el-tag v-else-if="d.prop === 'key' && row.key" size="small" type="warning">{{ row.key }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column :label="t('snippets.actions')" width="70" align="center">
            <template #default="{ row, $index }">
              <el-button
                size="small"
                text
                type="danger"
                :icon="Delete"
                :disabled="!editMode"
                @click="removeColumn(row, $index)"
              />
            </template>
          </el-table-column>
        </el-table>

        <!-- 表头右键：移到最前/最后 / 还原 -->
        <ContextMenu
          v-model:visible="inspMenu.visible"
          :x="inspMenu.x"
          :y="inspMenu.y"
          :items="inspMenuItems"
          @select="onInspMenuSelect"
        />
        <div class="pane-hint">{{ t('tableInspector.editHint') }}</div>
      </el-tab-pane>

      <!-- 索引 -->
      <el-tab-pane name="indexes">
        <template #label>
          <span class="pane-label">
            <el-icon><Grid /></el-icon>{{ t('tableInspector.indexes') }}
            <el-tag size="small" type="info">{{ indexes.length + newIndexes.length }}</el-tag>
          </span>
        </template>
        <div class="pane-tools">
          <span class="pt-spacer" />
          <el-button size="small" :icon="Plus" :disabled="!editMode" @click="addIndex">
            {{ t('tableInspector.addIndex') }}
          </el-button>
        </div>
        <el-table
          :data="allIndexes"
          size="small"
          border
          max-height="calc(100vh - 300px)"
          :row-style="{ height: '30px' }"
        >
          <el-table-column prop="name" :label="t('tableInspector.indexName')" min-width="160">
            <template #default="{ row }">
              <el-input v-model="row.name" size="small" :disabled="row.primary || !editMode" />
            </template>
          </el-table-column>
          <el-table-column :label="t('tableInspector.indexColumns')" min-width="220">
            <template #default="{ row }">
              <el-select
                v-if="row.isNew"
                v-model="row.columns"
                multiple
                filterable
                allow-create
                default-first-option
                size="small"
                style="width: 100%"
                :disabled="!editMode"
                :placeholder="t('tableInspector.selectColumns')"
              >
                <el-option v-for="c in columns" :key="c.name" :label="c.name" :value="c.name" />
              </el-select>
              <code v-else>{{ row.columns.join(', ') }}</code>
            </template>
          </el-table-column>
          <el-table-column :label="t('tableInspector.unique')" width="80" align="center">
            <template #default="{ row }">
              <el-checkbox v-model="row.unique" :disabled="row.primary || !editMode" />
            </template>
          </el-table-column>
          <el-table-column :label="t('inspector.colKey')" width="80" align="center">
            <template #default="{ row }">
              <el-tag v-if="row.primary" size="small" type="warning">PRI</el-tag>
              <el-tag v-else-if="row.unique" size="small" type="info">UNI</el-tag>
            </template>
          </el-table-column>
          <el-table-column :label="t('snippets.actions')" width="70" align="center" fixed="right">
            <template #default="{ row, $index }">
              <el-button
                size="small"
                text
                type="danger"
                :icon="Delete"
                :disabled="row.primary || !editMode"
                @click="removeIndex(row, $index)"
              />
            </template>
          </el-table-column>
        </el-table>
      </el-tab-pane>

      <!-- 源码 -->
      <el-tab-pane name="source">
        <template #label>
          <span class="pane-label">
            <el-icon><Document /></el-icon>{{ t('inspector.source') }}
          </span>
        </template>
        <div class="pane-tools">
          <el-button size="small" type="primary" :icon="Document" @click="openSourceInQuery">
            {{ t('inspector.openInQuery') }}
          </el-button>
          <span class="pt-spacer" />
          <span v-if="source" class="hint">{{ t('cell.chars', { n: source.length }) }}</span>
        </div>
        <div class="source-wrap">
          <CodeViewer :model-value="source" :kind="tab.kind" :ddl-options="true" />
        </div>
      </el-tab-pane>
    </el-tabs>

    <!-- 变更预览 -->
    <el-dialog v-model="previewVisible" :title="t('tableInspector.previewTitle')" width="640px">
      <el-alert
        :title="t('tableInspector.previewWarn')"
        type="warning"
        show-icon
        :closable="false"
        class="preview-warn"
      />
      <pre class="preview-sql">{{ previewSql.join('\n\n') }}</pre>
      <template #footer>
        <el-button @click="previewVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button :loading="applying" type="danger" :icon="Select" @click="applyChanges">
          {{ t('tableInspector.apply') }}
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import {
  Delete,
  Document,
  EditPen,
  Grid,
  Plus,
  Search,
  Select,
  Tickets,
  View,
} from '@element-plus/icons-vue'
import { objectIcon } from '../utils/objectIcons'
import CodeViewer from './CodeViewer.vue'
import ColVisPopover from './ColVisPopover.vue'
import ContextMenu, { type CtxMenuItem } from './ContextMenu.vue'
import Sortable from 'sortablejs'
import {
  dbDescribeTable,
  dbExecute,
  dbFetchTableDdl,
  dbListIndexes,
  dbTableChangeSql,
  dbTablePartitioned,
} from '../api/db'
import { useConnectionStore } from '../stores/connections'
import { useTabsStore, type QueryTab } from '../stores/tabs'
import { onAppCommand } from '../utils/commands'
import type { ColumnMeta, IndexInfo, TableChange } from '../types/db'

/** 编辑态列（新增行 isNew 标记） */
interface EditableColumn extends ColumnMeta {
  isNew?: boolean
}

/** 编辑态索引 */
interface EditableIndex extends IndexInfo {
  isNew?: boolean
}

const props = defineProps<{ tab: QueryTab }>()

const { t } = useI18n()
const connStore = useConnectionStore()
const tabsStore = useTabsStore()

const activeTab = ref('columns')
const loading = ref(false)
/** 编辑状态：默认只读，点「编辑」后方可修改结构 */
const editMode = ref(false)
/** 是否分区表 */
const partitioned = ref(false)

function toggleEdit(): void {
  if (editMode.value) {
    editMode.value = false
    void loadAll() // 取消编辑：放弃改动并重新加载
  } else {
    editMode.value = true
  }
}
const columnFilter = ref('')
const columns = ref<EditableColumn[]>([])

/** 从结构 Tab 直开数据预览（前 100 行 SQL，不自动执行） */
function previewData(): void {
  const table = props.tab.table
  if (!table) return
  tabsStore.openTableQuery({
    connectionId: props.tab.connectionId,
    connectionName: props.tab.connectionName,
    kind: props.tab.kind,
    database: props.tab.database,
    table,
  })
}

// ================= 列表（schema 字段）列操作：拖拽 / 移首尾 / 显隐 / 列宽 / 还原 =================
interface InspDef {
  prop: string
  labelKey: string
  min: number
  width?: number
  align?: 'center'
}

const INSP_DEFS: InspDef[] = [
  { prop: 'name', labelKey: 'inspector.colName', min: 150 },
  { prop: 'dataType', labelKey: 'inspector.colType', min: 170 },
  { prop: 'nullable', labelKey: 'inspector.colNullable', min: 80, width: 80, align: 'center' },
  { prop: 'defaultValue', labelKey: 'inspector.colDefault', min: 140 },
  { prop: 'comment', labelKey: 'inspector.colComment', min: 140 },
  { prop: 'key', labelKey: 'inspector.colKey', min: 70, width: 70, align: 'center' },
]

const inspKey = computed(
  () => `personaldb.inspectorCols:${props.tab.connectionId}:${props.tab.database}:${props.tab.table ?? ''}`,
)

interface InspState {
  order: string[]
  hidden: Record<string, boolean>
  widths: Record<string, number>
}

function inspLoad(): InspState {
  const fallback: InspState = {
    order: INSP_DEFS.map((d) => d.prop),
    hidden: {},
    widths: {},
  }
  try {
    const raw = localStorage.getItem(inspKey.value)
    if (!raw) return fallback
    const parsed = JSON.parse(raw) as Partial<InspState>
    return {
      order: Array.isArray(parsed.order) ? parsed.order : fallback.order,
      hidden: parsed.hidden ?? {},
      widths: parsed.widths ?? {},
    }
  } catch {
    return fallback
  }
}

let inspState = inspLoad()
const inspOrder = ref<string[]>([...inspState.order])
const inspHidden = ref<Record<string, boolean>>({ ...inspState.hidden })
const inspWidths = ref<Record<string, number>>({ ...inspState.widths })

let inspSaveTimer: number | undefined
function inspPersist(): void {
  window.clearTimeout(inspSaveTimer)
  inspSaveTimer = window.setTimeout(() => {
    try {
      localStorage.setItem(
        inspKey.value,
        JSON.stringify({ order: inspOrder.value, hidden: inspHidden.value, widths: inspWidths.value }),
      )
    } catch {
      /* ignore */
    }
  }, 300)
}

const inspOrderedDefs = computed(() => {
  const map = new Map(INSP_DEFS.map((d) => [d.prop, d]))
  const out = inspOrder.value.map((p) => map.get(p)).filter(Boolean) as InspDef[]
  for (const d of INSP_DEFS) {
    if (!out.some((o) => o.prop === d.prop)) out.push(d)
  }
  return out
})

const inspVisibleDefs = computed(() => inspOrderedDefs.value.filter((d) => !inspHidden.value[d.prop]))

const inspPopoverCols = computed(() =>
  inspOrderedDefs.value.map((d) => ({ prop: d.prop, name: t(d.labelKey) })),
)

function inspApplyHidden(next: Record<string, boolean>): void {
  inspHidden.value = { ...next }
  inspPersist()
}

function inspReorder(order: string[]): void {
  inspOrder.value = order
  inspPersist()
  void Promise.resolve().then(refreshInspSortable)
}

function inspReset(): void {
  inspOrder.value = INSP_DEFS.map((d) => d.prop)
  inspHidden.value = {}
  inspWidths.value = {}
  inspPersist()
  void Promise.resolve().then(refreshInspSortable)
}

function inspMoveEdge(prop: string, edge: 'front' | 'back'): void {
  const order = [...inspOrder.value]
  const idx = order.indexOf(prop)
  if (idx < 0) return
  order.splice(idx, 1)
  if (edge === 'front') order.unshift(prop)
  else order.push(prop)
  inspOrder.value = order
  inspPersist()
  void Promise.resolve().then(refreshInspSortable)
}

function inspHeaderResize(prop: string | undefined, width: number): void {
  if (!prop) return
  inspWidths.value = { ...inspWidths.value, [prop]: Math.round(width) }
  inspPersist()
}

const inspMenu = ref<{ visible: boolean; x: number; y: number; prop: string }>({
  visible: false,
  x: 0,
  y: 0,
  prop: '',
})

function inspHeaderMenu(column: { property?: string }, ev: MouseEvent): void {
  if (!column.property) return
  inspMenu.value = { visible: true, x: ev.clientX, y: ev.clientY, prop: column.property }
}

/** 表头右键菜单（公共 ContextMenu 渲染） */
const inspMenuItems = computed<CtxMenuItem[]>(() => [
  { key: 'first', label: t('result.moveFirst') },
  { key: 'last', label: t('result.moveLast') },
  { key: 'sep', label: '', sep: true },
  { key: 'reset', label: t('result.resetCols') },
])

function onInspMenuSelect(key: string): void {
  if (key === 'first') inspMoveEdge(inspMenu.value.prop, 'front')
  else if (key === 'last') inspMoveEdge(inspMenu.value.prop, 'back')
  else if (key === 'reset') inspReset()
}

const inspTableRef = ref()
let inspSortable: Sortable | null = null

function refreshInspSortable(): void {
  inspSortable?.destroy()
  inspSortable = null
  const el = (inspTableRef.value?.$el as HTMLElement | undefined)?.querySelector(
    '.el-table__header-wrapper thead tr',
  )
  if (!el) return
  inspSortable = Sortable.create(el as HTMLElement, {
    animation: 150,
    filter: '.el-table-column--selection',
    onEnd: ({ oldIndex, newIndex }) => {
      if (oldIndex == null || newIndex == null || oldIndex === newIndex) return
      const from = oldIndex - 1
      const to = newIndex - 1
      if (from < 0 || to < 0) return
      const order = [...inspOrderedDefs.value.map((d) => d.prop)]
      const [moved] = order.splice(from, 1)
      order.splice(to, 0, moved)
      inspOrder.value = order
      inspPersist()
      void Promise.resolve().then(refreshInspSortable)
    },
  })
}const originalColumns = ref<ColumnMeta[]>([])
const indexes = ref<IndexInfo[]>([])
const newIndexes = ref<EditableIndex[]>([])
const removedColumns = ref<ColumnMeta[]>([])
const removedIndexes = ref<IndexInfo[]>([])
const source = ref('')

const previewVisible = ref(false)
const previewSql = ref<string[]>([])
const applying = ref(false)

const filteredColumns = computed(() => {
  const q = columnFilter.value.trim().toLowerCase()
  if (!q) return columns.value
  return columns.value.filter(
    (c) =>
      c.name.toLowerCase().includes(q) ||
      c.dataType.toLowerCase().includes(q) ||
      (c.comment ?? '').toLowerCase().includes(q),
  )
})

const allIndexes = computed(() => [...indexes.value, ...newIndexes.value])

/** 待应用变更数量 */
const pendingChanges = computed(() => collectChanges().length)

function sessionId(): string | null {
  return connStore.sessionOf(props.tab.connectionId)?.sessionId ?? null
}

async function loadAll(): Promise<void> {
  const sid = sessionId()
  if (!sid) {
    ElMessage.error(t('query.connectionLost'))
    return
  }
  loading.value = true
  try {
    const db = props.tab.database || null
    const table = props.tab.table ?? props.tab.title
    const [cols, idx, ddl, part] = await Promise.all([
      dbDescribeTable(sid, db, table),
      dbListIndexes(sid, db, table),
      dbFetchTableDdl(sid, db, table),
      dbTablePartitioned(sid, db, table).catch(() => false),
    ])
    originalColumns.value = cols
    columns.value = cols.map((c) => ({ ...c }))
    indexes.value = idx
    source.value = ddl
    partitioned.value = part
    editMode.value = false
    newIndexes.value = []
    removedColumns.value = []
    removedIndexes.value = []
  } catch (e) {
    ElMessage.error(String(e))
  } finally {
    loading.value = false
  }
}

onMounted(loadAll)

onMounted(() => {
  void Promise.resolve().then(refreshInspSortable)
})

let offCommand: (() => void) | null = null
onMounted(() => {
  offCommand = onAppCommand((p) => {
    if (p.tabId && p.tabId !== props.tab.id) return
    if (p.cmd === 'table-source') activeTab.value = 'source'
  })
})
onBeforeUnmount(() => {
  offCommand?.()
  inspSortable?.destroy()
})

function addColumn(): void {
  columns.value.push({
    name: '',
    dataType: '',
    nullable: true,
    defaultValue: '',
    comment: '',
    key: null,
    isNew: true,
  })
}

function removeColumn(row: EditableColumn, index: number): void {
  if (!row.isNew) removedColumns.value.push(row)
  columns.value.splice(index, 1)
}

function addIndex(): void {
  newIndexes.value.push({
    name: '',
    columns: [],
    unique: false,
    primary: false,
    isNew: true,
  })
}

function removeIndex(row: EditableIndex, index: number): void {
  if (row.isNew) {
    newIndexes.value.splice(index - indexes.value.length, 1)
  } else if (!row.primary) {
    removedIndexes.value.push(row)
    indexes.value = indexes.value.filter((i) => i.name !== row.name)
  }
  void index
}

function columnRowClass({ row }: { row: EditableColumn }): string {
  return row.isNew ? 'is-new-row' : ''
}

/** 对比基线收集变更 */
function collectChanges(): TableChange[] {
  const changes: TableChange[] = []
  const originalMap = new Map(originalColumns.value.map((c) => [c.name, c]))
  for (const col of columns.value) {
    if (col.isNew) {
      changes.push({
        kind: 'addColumn',
        column: {
          name: col.name,
          dataType: col.dataType,
          nullable: col.nullable,
          defaultValue: col.defaultValue ?? null,
          comment: col.comment ?? null,
        },
      })
      continue
    }
    const orig = originalMap.get(col.name)
    if (!orig) continue
    const changed =
      orig.dataType !== col.dataType ||
      orig.nullable !== col.nullable ||
      (orig.defaultValue ?? '') !== (col.defaultValue ?? '') ||
      (orig.comment ?? '') !== (col.comment ?? '')
    if (changed) {
      changes.push({
        kind: 'modifyColumn',
        column: {
          name: col.name,
          dataType: col.dataType,
          nullable: col.nullable,
          defaultValue: col.defaultValue ?? null,
          comment: col.comment ?? null,
        },
      })
    }
  }
  for (const col of removedColumns.value) {
    changes.push({ kind: 'dropColumn', name: col.name })
  }
  for (const idx of newIndexes.value) {
    if (idx.name.trim() && idx.columns.length > 0) {
      changes.push({
        kind: 'addIndex',
        name: idx.name.trim(),
        columns: idx.columns,
        unique: idx.unique,
      })
    }
  }
  for (const idx of removedIndexes.value) {
    changes.push({ kind: 'dropIndex', name: idx.name })
  }
  return changes
}

async function previewChanges(): Promise<void> {
  const sid = sessionId()
  if (!sid) return
  const changes = collectChanges()
  if (changes.length === 0) {
    ElMessage.info(t('tableInspector.noChanges'))
    return
  }
  try {
    previewSql.value = await dbTableChangeSql(
      sid,
      props.tab.database || null,
      props.tab.table ?? props.tab.title,
      changes,
    )
    previewVisible.value = true
  } catch (e) {
    ElMessage.error(String(e))
  }
}

async function applyChanges(): Promise<void> {
  const sid = sessionId()
  if (!sid) return
  applying.value = true
  try {
    for (const sql of previewSql.value) {
      await dbExecute(sid, props.tab.database || null, sql, 1)
    }
    ElMessage.success(t('tableInspector.applied'))
    previewVisible.value = false
    await loadAll()
  } catch (e) {
    ElMessage.error(t('tableInspector.applyFailed', { msg: String(e) }))
  } finally {
    applying.value = false
  }
}

function openSourceInQuery(): void {
  const tab = tabsStore.openTab({
    connectionId: props.tab.connectionId,
    connectionName: props.tab.connectionName,
    kind: props.tab.kind,
    database: props.tab.database,
    title: props.tab.table ?? props.tab.title,
    icon: 'table',
  })
  tab.sql = source.value
  tab.result = null
  tab.error = ''
}
</script>

<style scoped>

.table-inspector {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 0 10px;
  overflow: hidden;
}

.inspector-toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 8px;
    background: var(--pd-band-bg);
    border-bottom: var(--pd-band-border);
    }

    .ctx {
  font-family: Consolas, monospace;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.spacer {
  flex: 1;
}

.inspector-tabs {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

/* 页签头与面板内 small 控件同档：12px 字体、紧凑高度，避免头部比内容还重 */
.inspector-tabs :deep(.el-tabs__header) {
  margin: 0 0 8px;
}
.inspector-tabs :deep(.el-tabs__item) {
  font-size: 12px;
  padding: 0 12px;
}
.inspector-tabs :deep(.pane-label .el-tag) {
  height: 16px;
  padding: 0 4px;
  font-size: 10px;
  line-height: 14px;
}

.inspector-tabs :deep(.el-tabs__content) {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.inspector-tabs :deep(.el-tab-pane) {
  height: 100%;
}

.pane-label {
  display: inline-flex;
  align-items: center;
  gap: 5px;
}

.pane-tools {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 8px;
    background: var(--pd-band-bg);
    border-bottom: var(--pd-band-border);
    }

.filter-input {
  width: 240px;
}

.pane-hint {
  margin-top: 8px;
  font-size: 12px;
  color: var(--el-text-color-placeholder);
}

.hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.source-wrap {
  height: calc(100vh - 320px);
}

.preview-warn {
  margin-bottom: 10px;
}

.preview-sql {
  margin: 0;
  padding: 10px;
  background: var(--el-fill-color-light);
  border-radius: 4px;
  font-family: Consolas, monospace;
  font-size: 12px;
  line-height: 1.7;
  white-space: pre-wrap;
  word-break: break-all;
  max-height: 320px;
  overflow: auto;
}

:deep(.is-new-row) {
  background: var(--el-color-success-light-9);
}
</style>

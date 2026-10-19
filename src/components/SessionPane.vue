<template>
  <div class="session-pane">
    <!-- 未连接 -->
    <div v-if="!session" class="center-hint">
      <EmptyState icon="obj_session" :title="t('session.notConnected')">
        <template #action>
          <el-button type="primary" :loading="connecting" @click="connect">
            {{ t('session.connect') }}
          </el-button>
        </template>
      </EmptyState>
    </div>

    <!-- 不支持 -->
    <div v-else-if="capLoaded && !supported" class="center-hint">
      <EmptyState icon="obj_session" :title="t('session.unsupported')" />
    </div>

    <template v-else>
      <div class="toolbar" data-tour="session-toolbar">
        <!-- 段1：视图切换（面板身份，最左） -->
        <el-segmented
          :model-value="view"
          :options="viewOptions"
          size="small"
          @update:model-value="(v: number | string) => switchView(String(v))"
        />
        <span class="pt-sep" />
        <!-- 段2：监视（刷新 / 自动刷新设置） -->
        <el-button size="small" circle :loading="loading" :title="t('session.refresh')" @click="load">
          <el-icon><component :is="objectIcon('act_refresh')" /></el-icon>
        </el-button>
        <span class="lbl">{{ t('session.autoRefresh') }}</span>
        <el-select v-model="intervalSec" size="small" style="width: 76px">
          <el-option :value="0" :label="t('session.off')" />
          <el-option :value="5" label="5s" />
          <el-option :value="10" label="10s" />
          <el-option :value="30" label="30s" />
        </el-select>
        <span class="last-refresh">{{ lastRefreshText }}</span>
        <!-- 段2b：过滤（仅会话视图） -->
        <template v-if="view === 'sessions'">
          <span class="pt-sep" />
          <el-input v-model="kw" size="small" clearable :placeholder="t('session.filter')" class="filter" />
          <el-radio-group v-model="stateFilter" size="small">
            <el-radio-button value="all">{{ t('session.fAll') }}</el-radio-button>
            <el-radio-button value="active">{{ t('session.fActive') }}</el-radio-button>
            <el-radio-button value="idle">{{ t('session.fIdle') }}</el-radio-button>
            <el-radio-button value="blocked">{{ t('session.fBlocked') }}</el-radio-button>
          </el-radio-group>
          <el-select
            v-model="userFilter"
            size="small"
            clearable
            :placeholder="t('session.colUser')"
            class="user-filter"
          >
            <el-option v-for="u in userOptions" :key="u" :label="u" :value="u" />
          </el-select>
          <el-checkbox v-model="onlyMine" size="small">{{ t('session.onlyMine') }}</el-checkbox>
        </template>

        <span class="pt-spacer" />
        <!-- 段3：动作（危险批量动作靠右，与过滤组明确隔开） -->
        <template v-if="view === 'sessions'">
          <el-tooltip :content="useVirtualSessions ? t('session.virtualHint') : t('session.exportCsv')" placement="bottom">
            <span>
              <el-button
                size="small"
                circle
                :disabled="useVirtualSessions"
                :title="t('session.exportCsv')"
                @click="exportCsv"
              >
                <el-icon><component :is="objectIcon('act_export')" /></el-icon>
              </el-button>
            </span>
          </el-tooltip>
          <el-tooltip v-if="useVirtualSessions" :content="t('session.virtualHint')" placement="bottom">
            <el-button size="small" type="danger" plain disabled>
              {{ t('session.batchKill', { n: selected.length }) }}
            </el-button>
          </el-tooltip>
          <el-button
            v-else
            size="small"
            type="danger"
            plain
            :disabled="selected.length === 0"
            @click="batchKill"
          >
            {{ t('session.batchKill', { n: selected.length }) }}
          </el-button>
          <span class="pt-sep" />
        </template>
        <!-- 段3b：状态与环境（最右） -->
        <span v-if="view === 'sessions'" class="count chips">
          <el-tag size="small" type="info">{{ t('common.rows', { n: rows.length }) }}</el-tag>
          <el-tag size="small" type="warning">{{ t('session.chipActive', { n: statActive }) }}</el-tag>
          <el-tag v-if="statBlocked > 0" size="small" type="danger">
            {{ t('session.chipBlocked', { n: statBlocked }) }}
          </el-tag>
        </span>
        <span v-if="connHeader" class="conn-chip" :title="connHeader">{{ connHeader }}</span>
      </div>

      <!-- ============ 会话视图 ============ -->
      <template v-if="view === 'sessions'">
        <ResultVirtualGrid
          v-if="useVirtualSessions"
          :columns="virtualColumns"
          :rows="virtualRows"
          @cell-menu="onVirtualCellMenu"
        />
        <el-table
          v-else
          ref="sessTableRef"
          v-loading="loading"
          :data="filtered"
          size="small"
          height="100%"
          class="grid"
          row-key="id"
          :row-class-name="rowClass"
          :cell-style="cellStyle"
          @header-contextmenu="onHeaderMenu"
          @header-click="onSessionHeaderClick"
          @header-dragend="onSessionHeaderResize"
          @selection-change="(rs: SessionRow[]) => (selected = rs)"
          @row-dblclick="(row: SessionRow) => openDetail(row)"
        >
          <el-table-column type="selection" width="36" />
          <el-table-column
            v-for="k in orderedSessionCols"
            :key="k"
            :prop="k"
            :label="sessionColLabel(k)"
            :min-width="k === 'sqlText' ? 240 : 100"
            :width="sessionWidths[k] || undefined"
            :align="k === 'secs' ? 'right' : undefined"
            :sortable="k === 'secs'"
            show-overflow-tooltip
          >
            <template v-if="k === 'state'" #default="{ row }">
              <el-tag size="small" :type="stateTagType(row.state)">{{ row.state }}</el-tag>
              <el-tag v-if="killedIds.has(row.id)" size="small" type="info" class="killed-tag">
                {{ t('session.killedMark') }}
              </el-tag>
            </template>
            <template v-else-if="k === 'sqlText'" #default="{ row }">
              <span
                v-if="row.sqlText"
                class="sql-text"
                @mouseenter="onSqlHover($event, row)"
                @mouseleave="sqlHoverVisible = false"
              >{{ row.sqlText }}</span>
            </template>
          </el-table-column>
          <el-table-column :label="t('session.colActions')" width="86">
            <template #default="{ row }">
              <el-tooltip :content="t('session.cancel')" placement="top">
                <el-button size="small" circle text type="primary" @click="doCancel(row)">
                  <el-icon><CircleClose /></el-icon>
                </el-button>
              </el-tooltip>
              <el-tooltip :content="t('session.kill')" placement="top">
                <el-button size="small" circle text type="danger" @click="doKill(row)">
                  <el-icon><CloseBold /></el-icon>
                </el-button>
              </el-tooltip>
            </template>
          </el-table-column>
          <template #empty>{{ t('session.empty') }}</template>
        </el-table>

        <!-- 表头右键菜单：列移动 / 还原 -->
        <ContextMenu
          v-model:visible="colMenu.visible"
          :x="colMenu.x"
          :y="colMenu.y"
          :items="colMenuItems"
          @select="onColMenuSelect"
        />
      </template>

      <!-- ============ 锁视图（动态列） ============ -->
      <template v-else>
        <ResultVirtualGrid
          v-if="useVirtualLocks"
          :columns="lockVirtualColumns"
          :rows="lockVirtualRows"
          @cell-menu="onVirtualCellMenu"
        />
        <el-table
          v-else
          v-loading="loading"
          :data="lockRows"
          size="small"
          height="100%"
          class="grid"
        >
          <el-table-column
            v-for="c in lockColumns"
            :key="c"
            :prop="c"
            :label="c"
            min-width="110"
            sortable
            show-overflow-tooltip
          />
          <el-table-column :label="t('session.colActions')" width="70">
            <template #default="{ row }">
              <el-tooltip :content="t('session.kill')" placement="top">
                <el-button
                  size="small"
                  circle
                  text
                  type="danger"
                  :disabled="!row.id"
                  @click="killFromLock(row)"
                >
                  <el-icon><CloseBold /></el-icon>
                </el-button>
              </el-tooltip>
            </template>
          </el-table-column>
          <template #empty>
            <span v-if="lockError" class="lock-error">{{ lockError }}</span>
            <EmptyState v-else icon="obj_key" :title="t('session.lockEmpty')" />
          </template>
        </el-table>
      </template>

      <!-- SQL 悬浮卡 -->
      <el-popover
        :visible="sqlHoverVisible"
        :virtual-ref="sqlAnchor"
        virtual-triggering
        placement="top-start"
        :width="480"
        popper-class="sql-hover-popper"
        @hide="sqlHoverVisible = false"
      >
        <div class="sql-card">
          <div class="sql-card-head">
            <span class="sql-card-title">{{ t('session.colSql') }}</span>
            <el-button size="small" text type="primary" @click="copyHoverSql">
              {{ t('cell.copy') }}
            </el-button>
          </div>
          <pre class="sql-card-body">{{ hoverSql }}</pre>
        </div>
      </el-popover>

      <!-- 会话详情 -->
      <el-dialog v-model="detailVisible" :title="t('session.detail')" width="560px" append-to-body>
        <el-descriptions :column="1" border size="small">
          <el-descriptions-item v-for="d in detailRows" :key="d.k" :label="d.k">
            {{ d.v }}
          </el-descriptions-item>
        </el-descriptions>
        <template v-if="detailSql">
          <div class="detail-sql-head">
            <span class="sql-card-title">{{ t('session.colSql') }}</span>
            <el-button size="small" text type="primary" @click="copyDetailSql">
              {{ t('cell.copy') }}
            </el-button>
          </div>
          <pre class="sql-card-body detail-sql">{{ detailSql }}</pre>
        </template>
      </el-dialog>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, defineAsyncComponent, onActivated, onBeforeUnmount, onDeactivated, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { CircleClose, CloseBold } from '@element-plus/icons-vue'
import Sortable from 'sortablejs'
import { save as saveDialog } from '@tauri-apps/plugin-dialog'
import {
  dbKillSession,
  dbRegistry,
  dbServerLocks,
  dbServerSessions,
  saveTextFile,
} from '../api/db'
import { useConnectionStore } from '../stores/connections'
import { objectIcon } from '../utils/objectIcons'
import { copyText } from '../utils/clipboard'
import { notifyFileSaved } from '../utils/notify'
import EmptyState from './EmptyState.vue'
import ContextMenu, { type CtxMenuItem } from './ContextMenu.vue'
import type { QueryTab } from '../stores/tabs'

interface SessionRow {
  id: string
  user: string
  host: string
  program: string
  state: string
  secs: string
  extra: string
  sqlText: string
  blocking: string
}

const props = defineProps<{ tab: QueryTab }>()

const { t } = useI18n()
const connStore = useConnectionStore()
const ResultVirtualGrid = defineAsyncComponent(() => import('./ResultVirtualGrid.vue'))

const session = computed(() => connStore.sessionOf(props.tab.connectionId))
const connecting = ref(false)
const loading = ref(false)
const rows = ref<SessionRow[]>([])
const selected = ref<SessionRow[]>([])
const kw = ref('')
const stateFilter = ref<'all' | 'active' | 'idle' | 'blocked'>('all')
const userFilter = ref('')
const intervalSec = ref(10)
const supported = ref(false)
const capLoaded = ref(false)
const lastRefreshAt = ref(0)
const view = ref<'sessions' | 'locks'>('sessions')
const lockRows = ref<Record<string, string>[]>([])
const lockColumns = ref<string[]>([])
const lockError = ref('')
/** 已请求终止的会话 id（刷新后仍存在则保留标记，消失即清除） */
const killedIds = ref(new Set<string>())
const isOracleKind = computed(() => props.tab.kind === 'oracle' || props.tab.kind === 'oceanbase-oracle')

const viewOptions = computed(() => [
  { label: t('session.tabSessions'), value: 'sessions' },
  { label: t('session.tabLocks'), value: 'locks' },
])

function switchView(v: string): void {
  view.value = v === 'locks' ? 'locks' : 'sessions'
  if (view.value === 'locks') void load()
}

const userOptions = computed(() => [...new Set(rows.value.map((r) => r.user))].sort())
const onlyMine = ref(false)
const currentUser = computed(() => connStore.configOf(props.tab.connectionId)?.username ?? '')
const statActive = computed(() => rows.value.filter((r) => isActiveState(r.state)).length)
const statBlocked = computed(() => rows.value.filter((r) => r.blocking).length)

function isActiveState(state: string): boolean {
  const s = state.toUpperCase()
  return s.includes('ACTIVE') || s === 'RUNNING' || s === 'EXECUTING'
}

const filtered = computed(() => {
  const k = kw.value.trim().toLowerCase()
  return rows.value.filter((r) => {
    if (onlyMine.value && currentUser.value && r.user !== currentUser.value) return false
    if (userFilter.value && r.user !== userFilter.value) return false
    if (stateFilter.value === 'active' && !isActiveState(r.state)) return false
    if (stateFilter.value === 'idle' && isActiveState(r.state)) return false
    if (stateFilter.value === 'blocked' && !r.blocking) return false
    if (!k) return true
    return [r.id, r.user, r.host, r.program, r.state, r.extra, r.sqlText]
      .join('\n')
      .toLowerCase()
      .includes(k)
  })
})

const blockerIds = computed(() => {
  const set = new Set<string>()
  for (const r of rows.value) {
    for (const id of r.blocking.split(',')) {
      const v = id.trim()
      if (v) set.add(v)
    }
  }
  return set
})

// ---------- 会话表列序（拖拽 / 表头右键移动 / 还原；按连接类型持久化） ----------
const SESSION_COL_KEYS = ['id', 'user', 'host', 'program', 'state', 'secs', 'extra', 'sqlText'] as const
const SESSION_COLS_KEY = `personaldb.sessionCols:${props.tab.kind}`
const colOrderSessions = ref<string[]>(loadSessionCols())
/** 列宽（双击表头自适应 / 拖拽后记忆） */
const sessionWidths = ref<Record<string, number>>(loadSessionWidths())
const orderedSessionCols = computed(() => {
  const valid = colOrderSessions.value.filter((k) => (SESSION_COL_KEYS as readonly string[]).includes(k))
  for (const k of SESSION_COL_KEYS) {
    if (!valid.includes(k)) valid.push(k)
  }
  return valid
})

interface SessionColsFile {
  order?: string[]
  widths?: Record<string, number>
}

function loadSessionFile(): SessionColsFile {
  try {
    const raw = localStorage.getItem(SESSION_COLS_KEY)
    if (!raw) return {}
    const parsed = JSON.parse(raw) as SessionColsFile
    if (Array.isArray(parsed)) return { order: parsed } // 旧格式
    return parsed
  } catch {
    return {}
  }
}

function loadSessionCols(): string[] {
  const order = loadSessionFile().order
  return Array.isArray(order) && order.length > 0 ? order : [...SESSION_COL_KEYS]
}

function loadSessionWidths(): Record<string, number> {
  return loadSessionFile().widths ?? {}
}

function saveSessionCols(): void {
  try {
    localStorage.setItem(
      SESSION_COLS_KEY,
      JSON.stringify({ order: colOrderSessions.value, widths: sessionWidths.value }),
    )
  } catch {
    /* ignore */
  }
}

function onSessionHeaderResize(prop: string | undefined, width: number): void {
  if (!prop) return
  sessionWidths.value = { ...sessionWidths.value, [prop]: Math.round(width) }
  saveSessionCols()
}

/** 双击表头自适应（400ms 内两次同列点击合成） */
const lastSessHeaderClick = ref<{ prop: string; at: number }>({ prop: '', at: 0 })
let sessMeasureCtx: CanvasRenderingContext2D | null = null

function onSessionHeaderClick(column: { property?: string; label?: string }): void {
  const prop = column.property
  if (!prop) return
  const now = Date.now()
  if (lastSessHeaderClick.value.prop === prop && now - lastSessHeaderClick.value.at < 400) {
    lastSessHeaderClick.value = { prop: '', at: 0 }
    const label = column.label ?? prop
    if (!sessMeasureCtx) sessMeasureCtx = document.createElement('canvas').getContext('2d')
    const ctx = sessMeasureCtx
    const width = (s: string): number => {
      if (!ctx) return s.length * 7
      ctx.font = '12px Consolas, "Courier New", monospace'
      return ctx.measureText(s).width
    }
    let max = width(label) + 26
    for (const row of rows.value.slice(0, 200)) {
      const cells = row as unknown as Record<string, string>
      const s = String(cells[prop] ?? '')
      if (!s) continue
      const w = width(s) + 24
      if (w > max) max = w
    }
    sessionWidths.value = { ...sessionWidths.value, [prop]: Math.min(600, Math.max(60, Math.ceil(max))) }
    saveSessionCols()
    return
  }
  lastSessHeaderClick.value = { prop, at: now }
}

function sessionColLabel(k: string): string {
  const map: Record<string, string> = {
    id: t('session.colId'),
    user: t('session.colUser'),
    host: t('session.colHost'),
    program: t('session.colProgram'),
    state: t('session.colState'),
    secs: t('session.colSecs'),
    extra: t('session.colExtra'),
    sqlText: t('session.colSql'),
  }
  return map[k] ?? k
}

const colMenu = ref<{ visible: boolean; x: number; y: number; prop: string }>({
  visible: false,
  x: 0,
  y: 0,
  prop: '',
})

function onHeaderMenu(column: { property?: string }, ev: MouseEvent): void {
  if (!column.property) return
  colMenu.value = { visible: true, x: ev.clientX, y: ev.clientY, prop: column.property }
}

/** 表头右键菜单（公共 ContextMenu 渲染） */
const colMenuItems = computed<CtxMenuItem[]>(() => [
  { key: 'first', label: t('result.moveFirst') },
  { key: 'last', label: t('result.moveLast') },
  { key: 'sep', label: '', sep: true },
  { key: 'reset', label: t('result.resetCols') },
])

function onColMenuSelect(key: string): void {
  if (key === 'first') moveColEdge(colMenu.value.prop, 'front')
  else if (key === 'last') moveColEdge(colMenu.value.prop, 'back')
  else if (key === 'reset') resetColOrder()
}

function moveColEdge(prop: string, edge: 'front' | 'back'): void {
  const order = [...colOrderSessions.value]
  const idx = order.indexOf(prop)
  if (idx < 0) return
  order.splice(idx, 1)
  if (edge === 'front') order.unshift(prop)
  else order.push(prop)
  colOrderSessions.value = order
  saveSessionCols()
  refreshSortable()
}

function resetColOrder(): void {
  colOrderSessions.value = [...SESSION_COL_KEYS]
  sessionWidths.value = {}
  saveSessionCols()
  refreshSortable()
}

// ---------- 表头拖拽 ----------
const sessTableRef = ref()
let colSortable: Sortable | null = null

function refreshSortable(): void {
  colSortable?.destroy()
  colSortable = null
  const el = (sessTableRef.value?.$el as HTMLElement | undefined)?.querySelector(
    '.el-table__header-wrapper thead tr',
  )
  if (!el) return
  colSortable = Sortable.create(el as HTMLElement, {
    animation: 150,
    filter: '.el-table-column--selection',
    onEnd: ({ oldIndex, newIndex }) => {
      if (oldIndex == null || newIndex == null || oldIndex === newIndex) return
      // 索引偏移：首列为固定 selection 列
      const from = oldIndex - 1
      const to = newIndex - 1
      if (from < 0 || to < 0) return
      const order = [...orderedSessionCols.value]
      const [moved] = order.splice(from, 1)
      order.splice(to, 0, moved)
      colOrderSessions.value = order
      saveSessionCols()
      void Promise.resolve().then(refreshSortable)
    },
  })
}


onMounted(() => {
  void Promise.resolve().then(refreshSortable)
})

function rowClass({ row }: { row: SessionRow }): string {
  if (killedIds.value.has(row.id)) return 'row-killed'
  if (row.blocking) return 'row-blocked'
  if (blockerIds.value.has(row.id)) return 'row-blocker'
  return ''
}

function secLevel(secs: string): 0 | 1 | 2 {
  const n = Number(secs) || 0
  if (n >= 300) return 2
  if (n >= 60) return 1
  return 0
}

function cellStyle({ row, column }: { row: SessionRow; column: { property?: string } }): Record<string, string> {
  if (column.property !== 'secs') return {}
  const lv = secLevel(row.secs)
  if (lv === 2) return { color: 'var(--el-color-danger)', fontWeight: '600' }
  if (lv === 1) return { color: 'var(--el-color-warning)', fontWeight: '600' }
  return {}
}

const lastRefreshText = computed(() =>
  lastRefreshAt.value ? new Date(lastRefreshAt.value).toLocaleTimeString() : '',
)

/** 连接信息头：user@host:port · 版本 */
const connHeader = computed(() => {
  const s = session.value
  if (!s) return ''
  const cfg = s.config
  const ver = s.serverInfo.serverVersion.split('\n')[0]?.slice(0, 40) ?? ''
  return `${cfg.username}@${cfg.host}:${cfg.port ?? ''}${ver ? ' · ' + ver : ''}`
})

// ---------- 虚拟滚动（>300 行） ----------
const useVirtualSessions = computed(() => rows.value.length > 300)
const virtualColumns = computed(() => [
  { name: t('session.colId'), semantic: 'text' as const },
  { name: t('session.colUser'), semantic: 'text' as const },
  { name: t('session.colHost'), semantic: 'text' as const },
  { name: t('session.colProgram'), semantic: 'text' as const },
  { name: t('session.colState'), semantic: 'text' as const },
  { name: t('session.colSecs'), semantic: 'number' as const },
  { name: t('session.colExtra'), semantic: 'text' as const },
  { name: t('session.colSql'), semantic: 'longtext' as const },
])
const virtualRows = computed(() =>
  filtered.value.map((r) => [r.id, r.user, r.host, r.program, r.state, Number(r.secs) || 0, r.extra, r.sqlText]),
)
function onVirtualCellMenu(p: { value: unknown }): void {
  void copyText(p.value === null || p.value === undefined ? '' : String(p.value))
  ElMessage.success(t('cell.copied'))
}

async function ensureCap(): Promise<void> {
  try {
    const list = await dbRegistry()
    supported.value = list.find((k) => k.kind === props.tab.kind)?.sessions ?? false
  } catch {
    supported.value = false
  }
  capLoaded.value = true
}

// 锁视图 >300 行虚拟化（动态列统一按文本渲染）
const useVirtualLocks = computed(() => view.value === 'locks' && lockRows.value.length > 300)
const lockVirtualColumns = computed(() =>
  lockColumns.value.map((name) => ({ name, semantic: 'text' as const })),
)
const lockVirtualRows = computed(() =>
  lockRows.value.map((r) => lockColumns.value.map((c) => r[c] ?? '')),
)

async function load(): Promise<void> {
  const s = session.value
  if (!s) return
  loading.value = true
  try {
    if (view.value === 'locks') {
      const res = await dbServerLocks(s.sessionId)
      lockColumns.value = res.columns.map((c) => c.name)
      lockRows.value = res.rows.map((row) => {
        const cells = row as unknown as (string | null)[]
        const obj: Record<string, string> = {}
        lockColumns.value.forEach((name, i) => {
          obj[name] = String(cells[i] ?? '')
        })
        return obj
      })
      lockError.value = ''
    } else {
      const res = await dbServerSessions(s.sessionId)
      rows.value = res.rows.map((r) => {
        const cells = r as unknown as (string | null)[]
        return {
          id: String(cells[0] ?? ''),
          user: String(cells[1] ?? ''),
          host: String(cells[2] ?? ''),
          program: String(cells[3] ?? ''),
          state: String(cells[4] ?? ''),
          secs: String(cells[5] ?? '0'),
          extra: String(cells[6] ?? ''),
          sqlText: String(cells[7] ?? ''),
          blocking: String(cells[8] ?? ''),
        }
      })
      // Kill 跟踪：会话已消失则清除标记
      const alive = new Set(rows.value.map((r) => r.id))
      for (const id of [...killedIds.value]) {
        if (!alive.has(id)) killedIds.value.delete(id)
      }
    }
    lastRefreshAt.value = Date.now()
  } catch (e) {
    if (view.value === 'locks') lockError.value = String(e)
    else ElMessage.error(String(e))
  } finally {
    loading.value = false
  }
}

async function connect(): Promise<void> {
  const cfg = connStore.configOf(props.tab.connectionId)
  if (!cfg) return
  connecting.value = true
  try {
    await connStore.openConnection(cfg)
    await Promise.all([ensureCap(), load()])
  } catch (e) {
    ElMessage.error(String(e))
  } finally {
    connecting.value = false
  }
}

function stateTagType(state: string): 'success' | 'info' | 'warning' | 'danger' {
  const s = state.toUpperCase()
  if (s.includes('ACTIVE') || s === 'RUNNING' || s === 'EXECUTING') return 'warning'
  if (s.includes('KILL') || s.includes('ERROR') || s.includes('ABORT')) return 'danger'
  return 'info'
}

// ---------- SQL 悬浮卡 ----------
const sqlHoverVisible = ref(false)
const sqlAnchor = ref<HTMLElement | null>(null)
const hoverSql = ref('')

function onSqlHover(ev: MouseEvent, row: SessionRow): void {
  hoverSql.value = row.sqlText
  sqlAnchor.value = ev.currentTarget as HTMLElement
  sqlHoverVisible.value = true
}

function copyHoverSql(): void {
  void copyText(hoverSql.value)
  ElMessage.success(t('cell.copied'))
}

// ---------- 详情 ----------
const detailVisible = ref(false)
const detailRows = ref<{ k: string; v: string }[]>([])
const detailSql = ref('')

function openDetail(row: SessionRow): void {
  detailRows.value = [
    { k: t('session.colId'), v: row.id },
    { k: t('session.colUser'), v: row.user },
    { k: t('session.colHost'), v: row.host },
    { k: t('session.colProgram'), v: row.program },
    { k: t('session.colState'), v: row.state },
    { k: t('session.colSecs'), v: row.secs },
    { k: t('session.colExtra'), v: row.extra },
    { k: t('session.colBlocking'), v: row.blocking || '-' },
  ]
  detailSql.value = row.sqlText
  detailVisible.value = true
}

function copyDetailSql(): void {
  void copyText(detailSql.value)
  ElMessage.success(t('cell.copied'))
}

// ---------- 终止 / 取消 / 批量 ----------
function markKilled(id: string): void {
  killedIds.value.add(id)
}

async function act(row: SessionRow, mode: 'cancel' | 'kill'): Promise<void> {
  const s = session.value
  if (!s) return
  try {
    await dbKillSession(s.sessionId, row.id, mode)
    if (mode === 'kill') markKilled(row.id)
    ElMessage.success(mode === 'cancel' ? t('session.cancelOk') : t('session.killOk'))
    await load()
  } catch (e) {
    ElMessage.error(String(e))
  }
}

async function doCancel(row: SessionRow): Promise<void> {
  await act(row, 'cancel')
}

/** Oracle 终止模式选择：IMMEDIATE / POST_TRANSACTION */
async function doKill(row: SessionRow): Promise<void> {
  let immediate = true
  if (isOracleKind.value) {
    const choice = await ElMessageBox.confirm(
      t('session.killModeDesc', { id: row.id }),
      t('session.killModeTitle'),
      {
        distinguishCancelAndClose: true,
        confirmButtonText: t('session.killImmediate'),
        cancelButtonText: t('session.killPost'),
        type: 'warning',
      },
    )
      .then(() => true)
      .catch((action: string) => {
        if (action === 'cancel') return false
        return null
      })
    if (choice === null) return
    immediate = choice
  } else {
    const ok = await ElMessageBox.confirm(
      t('session.killConfirm', { id: row.id }),
      t('session.killTitle'),
      { type: 'warning', confirmButtonText: t('session.kill'), cancelButtonText: t('common.cancel') },
    ).then(() => true).catch(() => false)
    if (!ok) return
  }
  const s = session.value
  if (!s) return
  try {
    await dbKillSession(s.sessionId, row.id, immediate ? 'kill' : 'cancel')
    markKilled(row.id)
    ElMessage.success(t('session.killOk'))
    await load()
  } catch (e) {
    ElMessage.error(String(e))
  }
}

async function batchKill(): Promise<void> {
  const targets = selected.value
  if (targets.length === 0) return
  const listText = targets.slice(0, 20).map((r) => `${r.id} (${r.user})`).join('\n')
  const more = targets.length > 20 ? `\n… ${t('session.moreN', { n: targets.length - 20 })}` : ''
  try {
    await ElMessageBox.confirm(
      `${t('session.batchConfirm', { n: targets.length })}\n${listText}${more}`,
      t('session.killTitle'),
      { type: 'warning', confirmButtonText: t('session.kill'), cancelButtonText: t('common.cancel') },
    )
  } catch {
    return
  }
  const s = session.value
  if (!s) return
  let okCount = 0
  const fails: string[] = []
  for (const r of targets) {
    try {
      await dbKillSession(s.sessionId, r.id, 'kill')
      markKilled(r.id)
      okCount += 1
    } catch (e) {
      fails.push(`${r.id}: ${String(e)}`)
    }
  }
  if (fails.length > 0) ElMessage.error(t('session.batchPartial', { ok: okCount, fail: fails.length }))
  else ElMessage.success(t('session.killOk'))
  await load()
}

/** 从锁视图直接终止持有者（id 与会话列表同语义：sid,serial / pid / processlist id） */
async function killFromLock(row: Record<string, string>): Promise<void> {
  const id = String(row.id ?? '')
  if (!id) return
  const s = session.value
  if (!s) return
  const ok = await ElMessageBox.confirm(t('session.killConfirm', { id }), t('session.killTitle'), {
    type: 'warning',
    confirmButtonText: t('session.kill'),
    cancelButtonText: t('common.cancel'),
  }).then(() => true).catch(() => false)
  if (!ok) return
  try {
    await dbKillSession(s.sessionId, id, 'kill')
    ElMessage.success(t('session.killOk'))
    await load()
  } catch (e) {
    ElMessage.error(String(e))
  }
}

// ---------- 导出 CSV ----------
async function exportCsv(): Promise<void> {
  const isLocks = view.value === 'locks'
  const header = isLocks
    ? lockColumns.value
    : [
        t('session.colId'),
        t('session.colUser'),
        t('session.colHost'),
        t('session.colProgram'),
        t('session.colState'),
        t('session.colSecs'),
        t('session.colExtra'),
        t('session.colBlocking'),
        t('session.colSql'),
      ]
  const esc = (v: string): string => `"${(v ?? '').replace(/"/g, '""')}"`
  const lines = [header.map(esc).join(',')]
  if (isLocks) {
    for (const r of lockRows.value) {
      lines.push(lockColumns.value.map((c) => esc(r[c] ?? '')).join(','))
    }
  } else {
    for (const r of filtered.value) {
      lines.push(
        [r.id, r.user, r.host, r.program, r.state, r.secs, r.extra, r.blocking, r.sqlText]
          .map(esc)
          .join(','),
      )
    }
  }
  const path = await saveDialog({
    defaultPath: `${isLocks ? 'locks' : 'sessions'}-${Date.now()}.csv`,
    filters: [{ name: 'CSV', extensions: ['csv'] }],
  })
  if (!path) return
  await saveTextFile(path as string, '\uFEFF' + lines.join('\r\n'))
  notifyFileSaved(path as string, t('transfer.exportDone'))
}

let timer: ReturnType<typeof setInterval> | null = null
/** KeepAlive 激活状态：隐藏期间不建刷新定时器，避免后台轮询 */
let paneAlive = true

function resetTimer(): void {
  if (timer) clearInterval(timer)
  timer = null
  if (paneAlive && intervalSec.value > 0) {
    timer = setInterval(() => {
      if (loading.value || document.hidden) return
      void load()
    }, intervalSec.value * 1000)
  }
}

watch(intervalSec, resetTimer)

// KeepAlive 生命周期：隐藏暂停自动刷新，重新激活立即恢复
onActivated(() => {
  paneAlive = true
  resetTimer()
})
onDeactivated(() => {
  paneAlive = false
  if (timer) {
    clearInterval(timer)
    timer = null
  }
})

void (async () => {
  await ensureCap()
  if (session.value) await load()
  resetTimer()
})()

onBeforeUnmount(() => {
  if (timer) clearInterval(timer)
  colSortable?.destroy()
})
</script>

<style scoped>
.session-pane {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 6px 10px 8px;
  min-height: 0;
}
.center-hint {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}
.toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px 6px;
    flex-wrap: wrap;
    row-gap: 6px;
    background: var(--pd-band-bg);
    border-bottom: var(--pd-band-border);
    }
.toolbar .lbl {
  font-size: 12px;
  opacity: 0.75;
}
.toolbar .last-refresh {
  font-size: 11px;
  opacity: 0.5;
  min-width: 60px;
}
.toolbar .conn-chip {
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 10px;
  background: var(--el-fill-color-light);
  max-width: 320px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.toolbar .filter {
  width: 170px;
}
.toolbar .user-filter {
  width: 110px;
}
    .toolbar .chips {
    display: inline-flex;
    gap: 4px;
    }
.grid {
  flex: 1;
  min-height: 0;
}
.grid :deep(.row-blocked) {
  background: var(--el-color-danger-light-9);
}
.grid :deep(.row-killed) {
  opacity: 0.55;
}
.grid :deep(.row-blocker) td:first-child .cell::before {
  content: '⚡';
  margin-right: 4px;
  color: var(--el-color-warning);
}
.killed-tag {
  margin-left: 4px;
}
.lock-error {
  color: var(--el-color-danger);
  font-size: 12px;
  padding: 8px;
}
.sql-text {
  display: inline-block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: default;
}
.sql-card {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.sql-card-head,
.detail-sql-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.sql-card-title {
  font-size: 12px;
  font-weight: 600;
  opacity: 0.75;
}
.sql-card-body {
  margin: 0;
  max-height: 220px;
  overflow: auto;
  font-size: 12px;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-all;
  font-family: Consolas, Menlo, monospace;
}
.detail-sql {
  max-height: 260px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  padding: 8px;
}
</style>

<template>
  <div class="log-panel" data-tour="log-panel">
    <div class="log-head">
      <span v-if="!embedded" class="log-title">{{ t('logs.title') }}</span>
      <el-radio-group v-model="levelFilter" size="small" class="log-filter">
        <el-radio-button value="all">{{ t('logs.level.all') }}</el-radio-button>
        <el-radio-button value="error">{{ t('logs.level.error') }}</el-radio-button>
        <el-radio-button value="warn">{{ t('logs.level.warn') }}</el-radio-button>
        <el-radio-button value="info">{{ t('logs.level.info') }}</el-radio-button>
      </el-radio-group>
      <span class="pt-sep" />
      <el-checkbox v-model="autoScroll" size="small" class="log-auto">{{ t('logs.autoScroll') }}</el-checkbox>
      <span class="log-spacer" />
      <el-tooltip :content="t('logs.clear')" placement="bottom">
        <el-button size="small" circle :icon="Delete" :aria-label="t('logs.clear')" @click="onClear" />
      </el-tooltip>
      <el-tooltip v-if="!embedded" :content="t('logs.collapse')" placement="bottom">
        <el-button size="small" circle :aria-label="t('logs.collapse')" @click="collapse">
          <el-icon><component :is="objectIcon('act_hide')" /></el-icon>
        </el-button>
      </el-tooltip>
    </div>
    <div ref="bodyRef" class="log-body" @scroll="onScroll">
      <div v-if="filtered.length === 0" class="log-empty">{{ t('logs.empty') }}</div>
      <template v-for="e in filtered" :key="e.id">
        <div class="log-row" :class="'is-' + e.level" @click="toggleDetail(e.id)">
          <span class="log-time">{{ fmtTime(e.at) }}</span>
          <span class="log-badge" :class="'b-' + e.level">{{ t('logs.level.' + e.level) }}</span>
          <span class="log-kind">{{ t('logs.kind.' + e.kind) }}</span>
          <span class="log-text">
            <span v-if="e.conn" class="log-conn">[{{ e.conn }}<template v-if="e.db">@{{ e.db }}</template>]</span>
            {{ e.title }}
          </span>
          <span class="log-meta">
            <template v-if="e.rows != null">{{ t('logs.rows', { n: e.rows }) }} · </template>
            <template v-if="e.durationMs != null">{{ e.durationMs }}ms</template>
          </span>
        </div>
        <div v-if="expandedId === e.id && e.detail" class="log-detail">{{ e.detail }}</div>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Delete } from '@element-plus/icons-vue'
import { objectIcon } from '../utils/objectIcons'
import { useLogStore, type LogLevel } from '../stores/logs'
import { useLayoutStore } from '../stores/layout'

const props = defineProps<{
  /** 悬浮窗内嵌：隐藏自带头部（FloatWindow 提供标题栏） */
  embedded?: boolean
}>()

const { t } = useI18n()
const logs = useLogStore()
const layout = useLayoutStore()

const levelFilter = ref<'all' | LogLevel>('all')
const autoScroll = ref(true)
const expandedId = ref('')
const bodyRef = ref<HTMLElement | null>(null)

const filtered = computed(() =>
  levelFilter.value === 'all' ? logs.entries : logs.entries.filter((e) => e.level === levelFilter.value),
)

function fmtTime(at: number): string {
  const d = new Date(at)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

function toggleDetail(id: string): void {
  expandedId.value = expandedId.value === id ? '' : id
}

function onClear(): void {
  logs.clear()
  expandedId.value = ''
}

/** 收起面板（隐藏；状态栏/菜单可再次打开） */
function collapse(): void {
  layout.hide('log')
}

/** 自动滚动：新日志到达且未手动上翻时贴底 */
let stickBottom = true
function onScroll(): void {
  const el = bodyRef.value
  if (!el) return
  stickBottom = el.scrollTop + el.clientHeight >= el.scrollHeight - 12
}
function scrollToBottom(): void {
  const el = bodyRef.value
  if (el) el.scrollTop = el.scrollHeight
}
watch(
  () => logs.entries.length,
  async () => {
    if (!autoScroll.value) return
    if (!stickBottom) return
    await nextTick()
    scrollToBottom()
  },
)
watch(filtered, async () => {
  await nextTick()
  if (autoScroll.value) scrollToBottom()
})

void props
</script>

<style scoped>
.log-panel {
  position: relative;
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  background: var(--el-bg-color);
}
.log-head {
.log-head :deep(.el-icon) {
  font-size: var(--pd-icon-sm);
}

  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  row-gap: 4px;
  padding: 4px 10px;
  background: var(--pd-band-bg);
  border-bottom: var(--pd-band-border);
  flex-shrink: 0;
}
.log-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--el-text-color-secondary);
}
.log-filter {
  transform: scale(0.92);
  flex-wrap: wrap;
}
.log-auto {
  margin-left: 2px;
  height: auto;
}
.log-spacer {
  flex: 1;
}
.log-body {
  flex: 1;
  overflow: auto;
  font-family: var(--el-font-family-mono, Consolas, monospace);
  font-size: 12px;
  padding: 2px 0;
}
.log-empty {
  text-align: center;
  padding: 28px 0;
  color: var(--el-text-color-placeholder);
  font-family: inherit;
}
.log-row {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 2px 10px;
  cursor: pointer;
  line-height: 1.7;
  white-space: nowrap;
}
.log-row:hover {
  background: var(--el-fill-color-light);
}
.log-time {
  color: var(--el-text-color-placeholder);
  flex-shrink: 0;
}
.log-badge {
  flex-shrink: 0;
  font-size: 10px;
  padding: 0 5px;
  border-radius: 3px;
  border: 1px solid currentColor;
  line-height: 16px;
  align-self: center;
}
.b-error {
  color: var(--el-color-danger);
}
.b-warn {
  color: var(--el-color-warning);
}
.b-info {
  color: var(--el-color-info);
}
.log-kind {
  color: var(--el-text-color-secondary);
  flex-shrink: 0;
}
.log-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  color: var(--el-text-color-primary);
}
.log-conn {
  color: var(--el-text-color-secondary);
  margin-right: 4px;
}
.log-meta {
  margin-left: auto;
  padding-left: 12px;
  color: var(--el-text-color-placeholder);
  flex-shrink: 0;
}
.is-error .log-text {
  color: var(--el-color-danger);
}
.is-warn .log-text {
  color: var(--el-color-warning);
}
.log-detail {
  white-space: pre-wrap;
  word-break: break-all;
  padding: 6px 10px 8px 96px;
  background: var(--el-fill-color-lighter);
  border-block: 1px dashed var(--el-border-color-lighter);
  color: var(--el-text-color-regular);
}
</style>

<template>
  <el-popover
    v-model:visible="visible"
    placement="bottom-end"
    :width="460"
    trigger="click"
    popper-class="history-popper"
  >
    <template #reference>
      <span class="hist-ref">
        <el-tooltip :content="t('history.title')" placement="bottom">
          <el-button size="small" circle data-tour="history" :aria-label="t('history.title')">
            <el-icon><component :is="objectIcon('act_history')" /></el-icon>
          </el-button>
        </el-tooltip>
      </span>
    </template>

    <div class="hist">
      <div class="hist-head">
        <el-input
          v-model="kw"
          size="small"
          clearable
          :placeholder="t('history.search')"
          :prefix-icon="Search"
          class="hist-search"
        />
        <el-checkbox v-model="onlyCurrent" size="small">{{ t('history.onlyCurrent') }}</el-checkbox>
        <el-button size="small" text type="danger" @click="clearAll">
          {{ t('history.clear') }}
        </el-button>
      </div>
      <div class="hist-list">
        <div
          v-for="item in filtered"
          :key="item.id"
          class="hist-item"
          :title="t('history.dblclickRun')"
          @click="apply(item)"
          @dblclick.stop="runItem(item)"
          @contextmenu.prevent.stop="copySql(item)"
        >
          <span class="dot" :class="item.ok ? 'ok' : 'fail'" />
          <div class="body">
            <div class="sql">{{ item.sql }}</div>
            <div class="meta">
              <span class="conn">{{ item.connectionName }}<template v-if="item.database">@{{ item.database }}</template></span>
              <span class="time">{{ fmtTime(item.at) }}</span>
              <span class="dur">{{ item.durationMs }}ms</span>
            </div>
          </div>
        </div>
        <div v-if="filtered.length === 0" class="empty">{{ t('history.empty') }}</div>
      </div>
    </div>
  </el-popover>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Search } from '@element-plus/icons-vue'
import { useHistoryStore, type HistoryEntry } from '../stores/history'
import { copyText } from '../utils/clipboard'
import { objectIcon } from '../utils/objectIcons'

const props = defineProps<{
  /** 当前页签连接名（「仅当前连接」过滤用） */
  connName?: string
}>()

const emit = defineEmits<{
  (e: 'apply', sql: string): void
  (e: 'run', sql: string): void
}>()

const { t } = useI18n()
const historyStore = useHistoryStore()

const visible = ref(false)
const kw = ref('')
const onlyCurrent = ref(false)

onMounted(() => {
  void historyStore.ensureLoaded()
})

let searchTimer: number | undefined
watch(kw, (v) => {
  window.clearTimeout(searchTimer)
  searchTimer = window.setTimeout(() => void historyStore.search(v), 250)
})

const filtered = computed(() => {
  let list = historyStore.list
  if (onlyCurrent.value && props.connName) {
    list = list.filter((h) => h.connectionName === props.connName)
  }
  return list.slice(0, 100)
})

function fmtTime(at: number): string {
  const d = new Date(at)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

function apply(item: HistoryEntry): void {
  emit('apply', item.sql)
  visible.value = false
}

/** 双击：回填并立即在当前页签执行 */
function runItem(item: HistoryEntry): void {
  emit('apply', item.sql)
  emit('run', item.sql)
  visible.value = false
}

/** 右键：仅复制 SQL（不回填、不关闭） */
function copySql(item: HistoryEntry): void {
  void copyText(item.sql)
  ElMessage.success(t('cell.copied'))
}

async function clearAll(): Promise<void> {
  const ok = await ElMessageBox.confirm(t('history.clearConfirm'), t('history.clear'), {
    type: 'warning',
    confirmButtonText: t('history.clear'),
    cancelButtonText: t('common.cancel'),
  }).then(() => true).catch(() => false)
  if (!ok) return
  await historyStore.clear()
}
</script>

<style scoped>
.hist-ref {
  display: inline-flex;
  align-items: center;
}
.hist {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.hist-head {
  display: flex;
  gap: 8px;
  align-items: center;
}
.hist-search {
  flex: 1;
}
.hist-list {
  max-height: 340px;
  overflow: auto;
  display: flex;
  flex-direction: column;
}
.hist-item {
  display: flex;
  gap: 8px;
  padding: 6px 4px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  cursor: pointer;
  border-radius: 4px;
}
.hist-item:hover {
  background: var(--el-fill-color-light);
}
.dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  margin-top: 6px;
  flex-shrink: 0;
}
.dot.ok {
  background: var(--el-color-success);
}
.dot.fail {
  background: var(--el-color-danger);
}
.body {
  min-width: 0;
  flex: 1;
}
.sql {
  font-family: var(--el-font-family-mono, Consolas, monospace);
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.meta {
  display: flex;
  gap: 10px;
  font-size: 11px;
  opacity: 0.65;
  margin-top: 2px;
}
.empty {
  text-align: center;
  padding: 24px 0;
  font-size: 12px;
  opacity: 0.6;
}
</style>

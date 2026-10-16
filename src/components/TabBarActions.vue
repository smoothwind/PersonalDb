<template>
  <div class="tba">
    <!-- 页签列表（图标 + 数量） -->
    <el-popover v-model:visible="listVisible" placement="bottom-end" :width="300" trigger="click">
      <template #reference>
        <button class="tba-btn" :class="{ 'is-on': listVisible }" :title="t('app.tabList')">
          <el-icon><List /></el-icon>
          <span class="tba-count">{{ tabsStore.tabs.length }}</span>
        </button>
      </template>
      <div class="tab-list">
        <el-input v-model="filter" size="small" clearable :placeholder="t('app.tabFilterPlaceholder')" />
        <div class="tab-list-body">
          <div
            v-for="tab in filteredTabs"
            :key="tab.id"
            class="tab-list-item"
            :class="{ 'is-active': tab.id === tabsStore.activeTabId }"
            @click="activate(tab.id)"
          >
            <i
              class="conn-dot"
              :class="{ 'is-lost': connStore.lostIds.has(tab.connectionId), 'is-off': !connStore.isConnected(tab.connectionId) }"
              :style="{ background: connColor(tab.connectionId) }"
            />
            <el-icon class="tl-icon"><component :is="objectIcon(tab.icon)" /></el-icon>
            <span
              class="tl-title"
              :class="{ 'is-lost': connStore.lostIds.has(tab.connectionId), 'is-off': !connStore.isConnected(tab.connectionId) }"
            >{{ tab.title }}</span>
            <span class="tl-conn">{{ tab.connectionName }}</span>
            <el-icon class="tl-close" @click.stop="tabsStore.closeTab(tab.id)">
              <Close />
            </el-icon>
          </div>
          <div v-if="filteredTabs.length === 0" class="tab-list-empty">
            {{ t('app.tabListEmpty') }}
          </div>
        </div>
      </div>
    </el-popover>

    <span class="pt-sep" />

    <!-- 分屏：左右 / 上下 / 单屏（当前方向高亮，未分屏时单屏按钮失效） -->
    <div class="tba-group">
      <el-tooltip :content="t('app.splitSide')" placement="bottom">
        <button class="tba-btn" :class="{ 'is-on': splitActive && splitDir === 'h' }" :aria-label="t('app.splitSide')" @click="emit('split', 'h')">
          <el-icon><component :is="objectIcon('act_split')" /></el-icon>
        </button>
      </el-tooltip>
      <el-tooltip :content="t('app.splitStack')" placement="bottom">
        <button class="tba-btn" :class="{ 'is-on': splitActive && splitDir === 'v' }" :aria-label="t('app.splitStack')" @click="emit('split', 'v')">
          <el-icon><component :is="objectIcon('act_split_stack')" /></el-icon>
        </button>
      </el-tooltip>
      <el-tooltip v-if="splitActive" :content="t('app.splitExit')" placement="bottom">
        <button class="tba-btn" :aria-label="t('app.splitExit')" @click="emit('exit')">
          <el-icon><component :is="objectIcon('act_unsplit')" /></el-icon>
        </button>
      </el-tooltip>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Close, List } from '@element-plus/icons-vue'
import { objectIcon } from '../utils/objectIcons'
import { connColor } from '../utils/color'
import { useTabsStore } from '../stores/tabs'
import { useConnectionStore } from '../stores/connections'

defineProps<{ splitActive: boolean; splitDir: 'h' | 'v' }>()
const emit = defineEmits<{
  (e: 'split', dir: 'h' | 'v'): void
  (e: 'exit'): void
}>()

const { t } = useI18n()
const tabsStore = useTabsStore()
const connStore = useConnectionStore()
const listVisible = ref(false)
const filter = ref('')

const filteredTabs = computed(() => {
  const q = filter.value.trim().toLowerCase()
  if (!q) return tabsStore.tabs
  return tabsStore.tabs.filter(
    (tb) =>
      tb.title.toLowerCase().includes(q) || (tb.connectionName ?? '').toLowerCase().includes(q),
  )
})

function activate(id: string): void {
  tabsStore.activeTabId = id
  listVisible.value = false
}
</script>

<style scoped>
.tba {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

/* 图标按钮：可见态 = 主色 + 淡主色底（与导航条 SPEC-UI-26 同一套状态语言） */
.tba-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 3px;
  height: 24px;
  min-width: 24px;
  padding: 0 5px;
  border: none;
  border-radius: var(--pd-radius);
  background: transparent;
  color: var(--el-text-color-secondary);
  cursor: pointer;
  transition: background-color 0.15s, color 0.15s;
}
.tba-btn :deep(.el-icon) {
  font-size: var(--pd-icon-md);
}
.tba-btn:focus-visible {
  outline: 2px solid var(--el-color-primary);
  outline-offset: -2px;
}
.tba-btn:hover {
  background: var(--el-fill-color);
  color: var(--el-text-color-primary);
}
.tba-btn.is-on {
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
}
.tba-count {
  font-size: 11px;
  line-height: 1;
  color: var(--el-text-color-placeholder);
}
.tba-btn.is-on .tba-count,
.tba-btn:hover .tba-count {
  color: inherit;
}

/* 分屏段：段内按钮以 1px 分隔，整体为一枚圆角容器（分段控件观感） */
.tba-group {
  display: inline-flex;
  align-items: center;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: var(--pd-radius);
  overflow: hidden;
}
.tba-group .tba-btn {
  border-radius: 0;
  width: 26px;
  padding: 0;
}
.tba-group .tba-btn + .tba-btn {
  border-left: 1px solid var(--el-border-color-lighter);
}

/* 页签列表弹层 */
.tab-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.tab-list-body {
  max-height: 320px;
  overflow: auto;
  margin: 0 -4px;
}
.tab-list-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 6px;
  border-radius: var(--pd-radius);
  cursor: pointer;
  font-size: 12px;
}
.tab-list-item:hover {
  background: var(--el-fill-color-light);
}
.tab-list-item.is-active {
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
}
.tl-title {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tl-icon {
  font-size: var(--pd-icon-sm);
}
.tl-title.is-lost {
  color: var(--el-color-danger);
}
.tl-title.is-off {
  opacity: 0.55;
}
.tl-conn {
  color: var(--el-text-color-placeholder);
  font-size: 11px;
  max-width: 110px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tl-conn.is-lost {
  color: var(--el-color-danger);
}
.tl-close {
  color: var(--el-text-color-secondary);
}
.tl-close:hover {
  color: var(--el-color-danger);
}
.tab-list-empty {
  padding: 10px;
  text-align: center;
  font-size: 12px;
  color: var(--el-text-color-placeholder);
}
</style>

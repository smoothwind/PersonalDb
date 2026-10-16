<template>
  <div class="tab-area">
    <!-- 主区：顶部页签栏 + 分屏体 / 欢迎页 -->
    <div class="tab-main">
      <template v-if="tabsStore.tabs.length > 0">
        <div class="tabbar">
          <el-tabs
            :key="splitActive ? `split-${splitDir}` : 'solo'"
            v-model="tabsStore.activeTabId"
            type="card"
            closable
            class="query-tabs"
            data-tour="tabs"
            @tab-remove="onRemoveTab"
          >
            <el-tab-pane v-for="tab in visibleTabs" :key="tab.id" :name="tab.id" :label="tab.title" :closable="true">
              <template #label>
                <span
                  class="tab-label"
                  :class="{ 'is-lost': isTabLost(tab), 'is-off': isTabOff(tab) }"
                  :title="`${t('menu.renameTip')} · ${tabStateText(tab)}`"
                  @dblclick.stop="startRename(tab)"
                  @contextmenu.prevent="openTabMenu(tab, $event)"
                >
                  <i
                    class="conn-dot"
                    :class="{ 'is-lost': isTabLost(tab), 'is-off': isTabOff(tab) }"
                    :style="{ background: connColor(tab.connectionId) }"
                  />
                  <el-icon><component :is="objectIcon(tab.icon)" /></el-icon>
                  <span class="tab-label-text">{{ tab.title }}</span>
                  <span v-if="tab.connectionName" class="tab-conn">
                    {{ tab.connectionName }}<template v-if="tab.database">@{{ tab.database }}</template>
                  </span>
                </span>
              </template>
            </el-tab-pane>
          </el-tabs>
          <TabBarActions :split-active="splitActive" :split-dir="splitDir" @split="openSplitPick" @exit="exitSplit" />
        </div>

        <div class="tab-body" :class="{ 'is-split': splitActive, 'dir-v': splitDir === 'v' }">
          <template v-if="splitActive && splitTab">
            <div class="pane-slot pane-a" :style="paneAstyle">
              <component :is="paneComp(tabsStore.activeTab)" v-if="tabsStore.activeTab" :key="tabsStore.activeTab.id" :tab="tabsStore.activeTab" />
            </div>
            <div class="splitter" @mousedown="startSplitDrag" />
            <div class="pane-slot pane-b">
              <component :is="paneComp(splitTab)" :key="splitTab.id" :tab="splitTab" />
            </div>
          </template>
          <div v-else class="pane-slot">
            <component :is="paneComp(tabsStore.activeTab)" v-if="tabsStore.activeTab" :key="tabsStore.activeTab.id" :tab="tabsStore.activeTab" />
          </div>
        </div>
      </template>
      <WelcomePane v-else :has-connections="connStore.saved.length > 0" @new-connection="emit('newConnection')" />
    </div>
  </div>

  <!-- 选择分屏页签 -->
  <el-dialog
    v-model="splitPickVisible"
    :title="splitPickDir === 'h' ? t('app.splitSide') : t('app.splitStack')"
    width="420px"
    append-to-body
  >
    <p class="split-pick-hint">{{ t('app.splitPickHint') }}</p>
    <div class="split-pick-list">
      <div v-for="tab in splittableTabs" :key="tab.id" class="split-pick-item" @click="confirmSplitPick(tab)">
        <el-icon><component :is="objectIcon(tab.icon)" /></el-icon>
        <span class="sp-title">{{ tab.title }}</span>
        <span class="sp-conn">{{ tab.connectionName }}</span>
      </div>
    </div>
  </el-dialog>

  <!-- 页签右键菜单（公共 ContextMenu 渲染） -->
  <ContextMenu
    v-model:visible="tabMenu.visible"
    :x="tabMenu.x"
    :y="tabMenu.y"
    :items="tabMenuItems"
    @select="onTabMenuSelect"
  />
</template>

<script setup lang="ts">
import { computed, defineAsyncComponent, reactive, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Close, EditPen } from '@element-plus/icons-vue'
import TabBarActions from '../TabBarActions.vue'
import WelcomePane from '../WelcomePane.vue'
import ContextMenu, { type CtxMenuItem } from '../ContextMenu.vue'
import { objectIcon } from '../../utils/objectIcons'
import { connColor } from '../../utils/color'
import { useTabsStore, type QueryTab } from '../../stores/tabs'
import { useConnectionStore } from '../../stores/connections'

const emit = defineEmits<{
  (e: 'newConnection'): void
}>()

const { t } = useI18n()
const tabsStore = useTabsStore()
const connStore = useConnectionStore()

// 重型面板异步加载（CodeMirror/结果集增强等大依赖随用随取）
const QueryTabPane = defineAsyncComponent(() => import('../QueryTabPane.vue'))
const TableInspectorPane = defineAsyncComponent(() => import('../TableInspectorPane.vue'))
const ObjectInspector = defineAsyncComponent(() => import('../ObjectInspector.vue'))
const RoutineEditor = defineAsyncComponent(() => import('../RoutineEditor.vue'))
const SessionPane = defineAsyncComponent(() => import('../SessionPane.vue'))

function paneComp(tab: QueryTab | null) {
  if (!tab) return QueryTabPane
  if (tab.type === 'table') return TableInspectorPane
  if (tab.type === 'object') return ObjectInspector
  if (tab.type === 'routine') return RoutineEditor
  if (tab.type === 'session') return SessionPane
  return QueryTabPane
}

/** 分屏时仅显示两个相关页签（含标题栏） */
const visibleTabs = computed(() =>
  splitActive.value
    ? tabsStore.tabs.filter((t) => t.id === tabsStore.activeTabId || t.id === splitTabId.value)
    : tabsStore.tabs,
)

/** —— 分屏（并排 / 堆叠） —— */
const splitTabId = ref<string | null>(null)
const splitDir = ref<'h' | 'v'>((localStorage.getItem('personaldb.splitDir') as 'h' | 'v') || 'h')
const splitPct = ref(Math.min(80, Math.max(20, Number(localStorage.getItem('personaldb.splitPct')) || 50)))

const splitActive = computed(() => !!splitTabId.value && tabsStore.tabs.some((t) => t.id === splitTabId.value))
const splitTab = computed(() => tabsStore.tabs.find((t) => t.id === splitTabId.value) ?? null)
const paneAstyle = computed(() => ({ flex: `0 0 ${splitPct.value}%` }) as Record<string, string>)

/** 页签连接状态：断开 / 未连接（活跃为默认样式） */
function isTabLost(tab: QueryTab): boolean {
  return connStore.lostIds.has(tab.connectionId)
}

function isTabOff(tab: QueryTab): boolean {
  return !connStore.isConnected(tab.connectionId)
}

function tabStateText(tab: QueryTab): string {
  return isTabLost(tab) ? t('conn.stateLost') : isTabOff(tab) ? t('conn.stateOff') : t('conn.stateConnected')
}

/** 选择分屏的第二个页签 */
const splitPickVisible = ref(false)
const splitPickDir = ref<'h' | 'v'>('h')
const splittableTabs = computed(() => tabsStore.tabs.filter((t) => t.id !== tabsStore.activeTabId))

function openSplitPick(dir: 'h' | 'v'): void {
  splitPickDir.value = dir
  if (splittableTabs.value.length === 0) {
    ElMessage.warning(t('app.splitNeedTwo'))
    return
  }
  splitPickVisible.value = true
}

function confirmSplitPick(tab: QueryTab): void {
  splitTabId.value = tab.id
  splitDir.value = splitPickDir.value
  localStorage.setItem('personaldb.splitDir', splitPickDir.value)
  splitPickVisible.value = false
}

function exitSplit(): void {
  splitTabId.value = null
}

function startSplitDrag(e: MouseEvent): void {
  e.preventDefault()
  const container = (e.currentTarget as HTMLElement).parentElement
  if (!container) return
  const rect = container.getBoundingClientRect()
  const dir = splitDir.value
  const onMove = (ev: MouseEvent): void => {
    const pct =
      dir === 'h' ? ((ev.clientX - rect.left) / rect.width) * 100 : ((ev.clientY - rect.top) / rect.height) * 100
    splitPct.value = Math.min(80, Math.max(20, pct))
  }
  const onUp = (): void => {
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
    localStorage.setItem('personaldb.splitPct', String(Math.round(splitPct.value)))
  }
  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}

function onRemoveTab(name: string | number): void {
  tabsStore.closeTab(String(name))
}

/** 页签重命名（双击标题 / 右键菜单 → 模态输入，稳定可靠） */
async function startRename(tab: QueryTab): Promise<void> {
  try {
    const { value } = await ElMessageBox.prompt(t('menu.renameTab'), t('menu.renameTab'), {
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
      inputValue: tab.title,
      inputPattern: /\S+/,
      inputErrorMessage: t('menu.renameRequired'),
    })
    const name = value.trim()
    if (name) tab.title = name
  } catch {
    // 取消
  }
}

/** —— 页签右键菜单 —— */
const tabMenu = reactive<{ visible: boolean; x: number; y: number; tabId: string }>({
  visible: false,
  x: 0,
  y: 0,
  tabId: '',
})

function openTabMenu(tab: QueryTab, e: MouseEvent): void {
  tabMenu.visible = true
  tabMenu.x = e.clientX
  tabMenu.y = e.clientY
  tabMenu.tabId = tab.id
}

/** 页签右键菜单项（公共 ContextMenu 渲染；分屏项仅在分屏中显示退出） */
const tabMenuItems = computed<CtxMenuItem[]>(() => {
  const items: CtxMenuItem[] = [
    { key: 'rename', label: t('menu.renameTip'), icon: EditPen },
    { key: 'sep1', label: '', sep: true },
    { key: 'splitSide', label: t('app.splitSide'), icon: 'act_split' },
    { key: 'splitStack', label: t('app.splitStack'), icon: 'act_split_stack' },
  ]
  if (splitActive.value) items.push({ key: 'splitExit', label: t('app.splitExit'), icon: 'act_unsplit' })
  items.push(
    { key: 'sep2', label: '', sep: true },
    { key: 'close', label: t('tabMenu.close'), icon: Close },
    { key: 'closeOthers', label: t('tabMenu.closeOthers'), icon: Close },
    { key: 'closeLeft', label: t('tabMenu.closeLeft'), icon: Close },
    { key: 'closeRight', label: t('tabMenu.closeRight'), icon: Close },
    { key: 'sep3', label: '', sep: true },
    { key: 'closeAll', label: t('tabMenu.closeAll'), icon: Close, danger: true },
  )
  return items
})

function onTabMenuSelect(key: string): void {
  const id = tabMenu.tabId
  switch (key) {
    case 'rename': {
      const tab = tabsStore.tabs.find((tb) => tb.id === id)
      if (tab) startRename(tab)
      break
    }
    case 'splitSide':
      openSplitPick('h')
      break
    case 'splitStack':
      openSplitPick('v')
      break
    case 'splitExit':
      exitSplit()
      break
    case 'close':
      tabsStore.closeTab(id)
      break
    case 'closeOthers':
      tabsStore.closeOthers(id)
      break
    case 'closeLeft':
      tabsStore.closeLeft(id)
      break
    case 'closeRight':
      tabsStore.closeRight(id)
      break
    case 'closeAll':
      tabsStore.closeAll()
      break
  }
}

defineExpose({ startRename, openSplitPick, exitSplit })
</script>

<style scoped>
.tab-area {
  display: flex;
  flex: 1;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
}
.tab-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
/* 顶部页签栏：页签 + 动作（列表/分屏） */
.query-tabs {
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
}
.tabbar {
  display: flex;
  align-items: center;
  gap: 6px;
  border-bottom: 1px solid var(--el-border-color-light);
  background: var(--app-panel-bg);
}
.tabbar .query-tabs {
  flex: 1;
  min-width: 0;
}
.tabbar .query-tabs :deep(.el-tabs__header) {
  margin: 0;
  border-bottom: none;
}
.tabbar .query-tabs :deep(.el-tabs__item) {
  border-bottom: none;
}
.tabbar .query-tabs :deep(.el-tabs__item.is-active) {
  font-weight: 600;
  box-shadow: inset 0 -2px 0 var(--el-color-primary);
}
.tabbar-actions {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding-right: 8px;
  flex-shrink: 0;
}

/* 页签内容改由 .tab-body 渲染，隐藏 el-tabs 自带空内容区 */
.query-tabs :deep(.el-tabs__content) {
  display: none;
}
.query-tabs :deep(.el-tab-pane) {
  height: 100%;
}
.query-tabs :deep(.el-tabs__header) {
  margin: 0;
  background: var(--app-panel-bg);
  border-bottom: 1px solid var(--el-border-color-light);
}
.query-tabs :deep(.el-tabs__item) {
  height: 34px;
  line-height: 34px;
  font-size: 13px;
}
.tab-label {
.tab-label :deep(.el-icon) {
  font-size: var(--pd-icon-sm);
}

  display: inline-flex;
  align-items: center;
  gap: 5px;
}
.tab-label-text {
  max-width: 160px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tab-conn {
  max-width: 130px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 10.5px;
  color: var(--el-text-color-placeholder);
}
.tab-label.is-lost .tab-label-text,
.tab-label.is-lost .tab-conn {
  color: var(--el-color-danger);
}
.tab-label.is-off {
  opacity: 0.55;
}

/* 分屏 */
.tab-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: row;
}
.tab-body.dir-v {
  flex-direction: column;
}
.pane-slot {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.pane-slot > * {
  flex: 1;
  min-height: 0;
}
.tab-body .splitter {
  flex: 0 0 4px;
  background: var(--el-border-color-lighter);
  cursor: col-resize;
}
.tab-body.dir-v .splitter {
  cursor: row-resize;
}
.tab-body .splitter:hover {
  background: var(--el-color-primary-light-7);
}

/* 页签右键菜单（teleport 到 body，scoped 属性随行） */

.split-pick-hint {
  margin: 0 0 10px;
  color: var(--el-text-color-secondary);
  font-size: 12px;
}
.split-pick-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 320px;
  overflow: auto;
}
.split-pick-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  border-radius: 6px;
  cursor: pointer;
}
.split-pick-item:hover {
  background: var(--el-fill-color-light);
}
.sp-title {
  font-size: 13px;
}
.sp-conn {
  margin-left: auto;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
</style>

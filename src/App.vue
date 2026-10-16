<template>
  <el-config-provider :locale="epLocale">
    <!-- 自定义标题栏（跟随主题，隐藏系统装饰） -->
    <div class="titlebar">
      <div class="tb-brand">
        <el-icon color="var(--el-color-primary)"><Coin /></el-icon>
        <span class="tb-name">PersonalDb</span>
      </div>
      <div class="tb-drag" data-tauri-drag-region />
      <div class="tb-controls">
        <button class="tb-btn" :title="t('titlebar.minimize')" @click="minimizeWin">
          <el-icon><Minus /></el-icon>
        </button>
        <button class="tb-btn" :title="t('titlebar.maximize')" @click="toggleMaxWin">
          <el-icon><FullScreen /></el-icon>
        </button>
        <button class="tb-btn tb-close" :title="t('titlebar.close')" @click="closeWin">
          <el-icon><Close /></el-icon>
        </button>
      </div>
    </div>

    <el-container class="app-root">
      <el-menu
        mode="horizontal"
        class="app-menu"
        :ellipsis="false"
        @select="onMenuSelect"
      >
        <el-sub-menu index="file">
          <template #title>{{ t('menu.file') }}</template>
          <el-menu-item index="new-connection">
            <el-icon class="menu-icon"><component :is="MENU_ICONS.newConnection" /></el-icon>
            <span class="menu-text">{{ t('app.newConnection') }}</span>
            <span class="menu-shortcut">{{ prefs.shortcutOf('newConnection') }}</span>
          </el-menu-item>
          <el-menu-item index="new-query">
            <el-icon class="menu-icon"><component :is="MENU_ICONS.newQuery" /></el-icon>
            <span class="menu-text">{{ t('app.newQuery') }}</span>
            <span class="menu-shortcut">{{ prefs.shortcutOf('newQuery') }}</span>
          </el-menu-item>
        </el-sub-menu>
        <el-sub-menu index="edit">
          <template #title>{{ t('menu.edit') }}</template>
          <el-menu-item index="format">
            <el-icon class="menu-icon"><component :is="MENU_ICONS.format" /></el-icon>
            <span class="menu-text">{{ t('query.format') }}</span>
            <span class="menu-shortcut">{{ prefs.shortcutOf('format') }}</span>
          </el-menu-item>
          <el-menu-item index="compress">
            <el-icon class="menu-icon"><component :is="MENU_ICONS.compress" /></el-icon>
            <span class="menu-text">{{ t('query.compress') }}</span>
            <span class="menu-shortcut">{{ prefs.shortcutOf('compress') }}</span>
          </el-menu-item>
          <div class="menu-sep" />
          <el-menu-item index="upper">
            <el-icon class="menu-icon"><component :is="MENU_ICONS.upper" /></el-icon>
            <span class="menu-text">{{ t('menu.upper') }}</span>
            <span class="menu-shortcut">{{ prefs.shortcutOf('upper') }}</span>
          </el-menu-item>
          <el-menu-item index="lower">
            <el-icon class="menu-icon"><component :is="MENU_ICONS.lower" /></el-icon>
            <span class="menu-text">{{ t('menu.lower') }}</span>
            <span class="menu-shortcut">{{ prefs.shortcutOf('lower') }}</span>
          </el-menu-item>
        </el-sub-menu>
        <el-sub-menu index="view">
          <template #title>{{ t('menu.view') }}</template>
          <el-menu-item index="view-grid">
            <el-icon class="menu-icon"><component :is="MENU_ICONS.viewGrid" /></el-icon>
            <span class="menu-text">{{ t('query.viewGrid') }}</span>
          </el-menu-item>
          <el-menu-item index="view-record">
            <el-icon class="menu-icon"><component :is="MENU_ICONS.viewRecord" /></el-icon>
            <span class="menu-text">{{ t('query.viewRecord') }}</span>
          </el-menu-item>
          <el-menu-item index="view-plan">
            <el-icon class="menu-icon"><component :is="MENU_ICONS.viewPlan" /></el-icon>
            <span class="menu-text">{{ t('query.viewPlan') }}</span>
          </el-menu-item>
          <div class="menu-sep" />
          <el-menu-item index="toggle-conn-panel">
            <el-icon class="menu-icon"><component :is="MENU_ICONS.connPanel" /></el-icon>
            <span class="menu-text">{{ t('tree.title') }}</span>
          </el-menu-item>
          <el-menu-item index="toggle-log-panel">
            <el-icon class="menu-icon"><component :is="MENU_ICONS.logPanel" /></el-icon>
            <span class="menu-text">{{ t('logs.panel') }}</span>
            <span class="menu-shortcut">{{ prefs.shortcutOf('toggleLogPanel') }}</span>
          </el-menu-item>
          <div class="menu-sep" />
          <el-menu-item index="reset-layout">
            <el-icon class="menu-icon"><component :is="MENU_ICONS.resetLayout" /></el-icon>
            <span class="menu-text">{{ t('menu.resetLayout') }}</span>
            <span class="menu-shortcut">{{ prefs.shortcutOf('resetLayout') }}</span>
          </el-menu-item>
          <el-menu-item index="toggle-theme">
            <el-icon class="menu-icon"><component :is="MENU_ICONS.theme" /></el-icon>
            <span class="menu-text">{{ t('menu.toggleTheme') }}</span>
            <span class="menu-shortcut">{{ prefs.shortcutOf('toggleTheme') }}</span>
          </el-menu-item>
        </el-sub-menu>
        <el-sub-menu index="tools">
          <template #title>{{ t('menu.tools') }}</template>
          <el-menu-item index="snippets">
            <el-icon class="menu-icon"><component :is="MENU_ICONS.snippets" /></el-icon>
            <span class="menu-text">{{ t('query.snippets') }}</span>
          </el-menu-item>
          <el-menu-item index="drivers">
            <el-icon class="menu-icon"><component :is="MENU_ICONS.drivers" /></el-icon>
            <span class="menu-text">{{ t('driver.title') }}</span>
          </el-menu-item>
          <el-menu-item index="config-transfer">
            <el-icon class="menu-icon"><component :is="MENU_ICONS.transfer" /></el-icon>
            <span class="menu-text">{{ t('transfer.title') }}</span>
          </el-menu-item>
          <div class="menu-sep" />
          <el-menu-item index="view-log">
            <el-icon class="menu-icon"><component :is="MENU_ICONS.viewLog" /></el-icon>
            <span class="menu-text">{{ t('menu.viewLog') }}</span>
          </el-menu-item>
          <el-menu-item index="settings">
            <el-icon class="menu-icon"><component :is="MENU_ICONS.settings" /></el-icon>
            <span class="menu-text">{{ t('settings.title') }}</span>
            <span class="menu-shortcut">{{ prefs.shortcutOf('openSettings') }}</span>
          </el-menu-item>
        </el-sub-menu>
        <el-sub-menu index="help">
          <template #title>{{ t('menu.help') }}</template>
          <el-sub-menu index="tours">
            <template #title>
              <el-icon class="menu-icon"><component :is="MENU_ICONS.tour" /></el-icon>
              {{ t('tour.menu') }}
            </template>
            <el-menu-item v-for="tr in TOURS" :key="tr.id" :index="'tour:' + tr.id">
              <el-icon class="menu-icon"><component :is="MENU_ICONS.tourItem" /></el-icon>
              <span class="menu-text">{{ t(tr.labelKey) }}</span>
            </el-menu-item>
          </el-sub-menu>
          <div class="menu-sep" />
          <el-menu-item index="about">
            <el-icon class="menu-icon"><component :is="MENU_ICONS.about" /></el-icon>
            <span class="menu-text">{{ t('menu.about') }}</span>
          </el-menu-item>
          <el-menu-item index="check-update">
            <el-icon class="menu-icon"><component :is="MENU_ICONS.update" /></el-icon>
            <span class="menu-text">{{ t('update.check') }}</span>
          </el-menu-item>
        </el-sub-menu>
      </el-menu>

      <el-header height="42px" class="app-header">
        <div class="brand">PersonalDb</div>
        <el-divider direction="vertical" />
        <el-button size="small" :icon="Plus" data-tour="new-conn" @click="openCreate">
          {{ t('app.newConnection') }}
        </el-button>
        <el-button size="small" :icon="Document" data-tour="new-query" @click="quickNewQuery">
          {{ t('app.newQuery') }}
        </el-button>
        <div class="header-spacer" />

        <!-- 工作区：管理页签集合与文件保存位置 -->
        <el-dropdown trigger="click" @command="onWorkspaceCommand">
          <el-button size="small" text class="ws-btn" data-tour="workspaces">
            <el-icon><Folder /></el-icon>
            <span class="ws-name">{{ workspaceStore.current?.name ?? '' }}</span>
            <el-icon class="ws-arrow"><ArrowDown /></el-icon>
          </el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item
                v-for="w in workspaceStore.list"
                :key="w.id"
                :command="'switch:' + w.id"
                :class="{ 'is-active-ws': w.id === workspaceStore.currentId }"
              >
                <el-icon><component :is="objectIcon('act_workspace')" /></el-icon>
                {{ w.name }}
              </el-dropdown-item>
              <el-dropdown-item divided command="new">
                <el-icon><Plus /></el-icon>
                {{ t('workspace.new') }}
              </el-dropdown-item>
              <el-dropdown-item command="rename">
                <el-icon><EditPen /></el-icon>
                {{ t('workspace.rename') }}
              </el-dropdown-item>
              <el-dropdown-item command="exportDir">
                <el-icon><component :is="objectIcon('act_export')" /></el-icon>
                {{ t('workspace.exportDir') }}
              </el-dropdown-item>
              <el-dropdown-item command="changeDir">
                <el-icon><FolderAdd /></el-icon>
                {{ t('workspace.changeDir') }}
              </el-dropdown-item>
              <el-dropdown-item command="openDir">
                <el-icon><FolderOpened /></el-icon>
                {{ t('workspace.openDir') }}
              </el-dropdown-item>
              <el-dropdown-item
                command="delete"
                divided
                :disabled="workspaceStore.list.length <= 1"
              >
                {{ t('workspace.delete') }}
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
        <el-divider direction="vertical" class="header-divider" />

        <!-- 事务管理（当前激活查询所属连接，能力驱动）：模式分段 + 提交/回滚图标按钮（布局稳定，不分模式跳动） -->
        <template v-if="txAvailable">
          <el-tooltip :content="txTip" placement="bottom">
            <span class="tx-lead">
              <el-icon><component :is="objectIcon('act_tx')" /></el-icon>
            </span>
          </el-tooltip>
          <el-segmented
            class="tx-mode"
            size="small"
            :model-value="txMode"
            :options="txModeOptions"
            :disabled="txBusy"
            @update:model-value="(v: string | number | boolean) => setTxMode(v as 'auto' | 'manual')"
          />
          <span class="pt-sep" />
          <div class="tx-acts">
            <el-tooltip :content="t('tx.commit')" placement="bottom">
              <span class="tx-slot">
                <button class="tx-act is-commit" :aria-label="t('tx.commit')" :disabled="!txManual || txBusy" @click="doCommit">
                  <el-icon><component :is="objectIcon('act_commit')" /></el-icon>
                </button>
              </span>
            </el-tooltip>
            <el-tooltip :content="t('tx.rollback')" placement="bottom">
              <span class="tx-slot">
                <button class="tx-act is-rollback" :aria-label="t('tx.rollback')" :disabled="!txManual || txBusy" @click="doRollback">
                  <el-icon><component :is="objectIcon('act_rollback')" /></el-icon>
                </button>
              </span>
            </el-tooltip>
          </div>
          <el-divider direction="vertical" class="header-divider" />
        </template>

        <span class="conn-state" :class="{ 'is-on': connStore.connectedCount > 0 }">
          <i class="conn-state-dot" />
          {{
            connStore.connectedCount > 0
              ? t('app.activeConnections', { n: connStore.connectedCount })
              : t('app.disconnected')
          }}
        </span>
        <el-divider direction="vertical" class="header-divider" />
        <el-tooltip :content="prefs.theme === 'dark' ? t('app.themeToLight') : t('app.themeToDark')" placement="bottom">
          <button class="hdr-btn" :aria-label="prefs.theme === 'dark' ? t('app.themeToLight') : t('app.themeToDark')" @click="prefs.toggleTheme()">
            <el-icon><component :is="objectIcon('act_theme')" /></el-icon>
          </button>
        </el-tooltip>
        <el-tooltip :content="t('app.settings')" placement="bottom">
          <button class="hdr-btn" data-tour="settings" :aria-label="t('app.settings')" @click="settingsVisible = true">
            <el-icon><component :is="objectIcon('act_settings')" /></el-icon>
          </button>
        </el-tooltip>
        <el-tooltip v-if="sessionsCap" :content="t('session.tabTitle')" placement="bottom">
          <button class="hdr-btn" :aria-label="t('session.tabTitle')" @click="openSessions">
            <el-icon><component :is="objectIcon('session')" /></el-icon>
          </button>
        </el-tooltip>
      </el-header>

      <el-container class="app-body">
        <!-- 主窗左侧常驻面板条：面板开关 + 隐藏后快速唤回（吸附窗口最左） -->
        <PanelRail />
        <!-- 左停靠区：可承载 连接管理器 / 功能区 / 日志查看器（拖拽标题移动 / 悬浮） -->
        <template v-if="layout.leftPanel">
          <aside
            class="wb-dock is-left"
            :class="{ 'drop-hot': dropZone === 'left', 'is-drag-source': layout.dragging === layout.leftPanel }"
            :style="{ width: layout.sizes.left + 'px', minWidth: layout.sizes.left + 'px' }"
            @dragover="onZoneDragOver('left', $event)"
            @drop="onZoneDrop('left', $event)"
          >
            <PanelHeader :panel="layout.leftPanel" :title="panelTitle(layout.leftPanel)" />
            <div class="wb-dock-body">
              <PanelBody
                :id="layout.leftPanel"
                embedded
                @new-connection="openCreate"
                @edit="openEdit"
              />
            </div>
            <div v-if="layout.dragging && dropZone === 'left'" class="drop-mask"><span>{{ zoneMaskLabel('left') }}</span></div>
          </aside>
          <div class="splitter-v" @mousedown="startZoneDrag('left', $event)" />
        </template>

        <!-- 中心区（常驻）：可被交换为任意面板 -->
        <main
          class="wb-center"
          :class="{ 'drop-hot': dropZone === 'center', 'is-drag-source': layout.dragging === layout.centerPanel }"
          @dragover="onZoneDragOver('center', $event)"
          @drop="onZoneDrop('center', $event)"
        >
          <!-- 功能区自带页签栏作头部；换入其它面板时补面板头，保证可再次拖拽/还原 -->
          <PanelHeader
            v-if="layout.centerPanel && layout.centerPanel !== 'tabs'"
            :panel="layout.centerPanel"
            :title="panelTitle(layout.centerPanel)"
          />
          <PanelBody
            v-if="layout.centerPanel"
            :id="layout.centerPanel"
            embedded
            @new-connection="openCreate"
            @edit="openEdit"
          />
          <div v-if="layout.dragging && dropZone === 'center'" class="drop-mask"><span>{{ zoneMaskLabel('center') }}</span></div>
        </main>

        <!-- 右停靠区 -->
        <template v-if="layout.rightPanel">
          <div class="splitter-v" @mousedown="startZoneDrag('right', $event)" />
          <aside
            class="wb-dock is-right"
            :class="{ 'drop-hot': dropZone === 'right', 'is-drag-source': layout.dragging === layout.rightPanel }"
            :style="{ width: layout.sizes.right + 'px', minWidth: layout.sizes.right + 'px' }"
            @dragover="onZoneDragOver('right', $event)"
            @drop="onZoneDrop('right', $event)"
          >
            <PanelHeader :panel="layout.rightPanel" :title="panelTitle(layout.rightPanel)" />
            <div class="wb-dock-body">
              <PanelBody :id="layout.rightPanel" embedded @new-connection="openCreate" @edit="openEdit" />
            </div>
            <div v-if="layout.dragging && dropZone === 'right'" class="drop-mask"><span>{{ zoneMaskLabel('right') }}</span></div>
          </aside>
        </template>
      </el-container>

      <!-- 下停靠区（默认：日志查看器，吸附下边框） -->
      <template v-if="layout.bottomPanel">
        <div class="splitter-h" @mousedown="startZoneDrag('bottom', $event)" />
        <div
          class="wb-dock wb-bottom"
          :class="{ 'drop-hot': dropZone === 'bottom', 'is-drag-source': layout.dragging === layout.bottomPanel }"
          :style="{ height: layout.sizes.bottom + 'px' }"
          @dragover="onZoneDragOver('bottom', $event)"
          @drop="onZoneDrop('bottom', $event)"
        >
          <PanelHeader :panel="layout.bottomPanel" :title="panelTitle(layout.bottomPanel)" />
          <div class="wb-dock-body">
            <PanelBody :id="layout.bottomPanel" embedded @new-connection="openCreate" @edit="openEdit" />
          </div>
          <div v-if="layout.dragging && dropZone === 'bottom'" class="drop-mask"><span>{{ zoneMaskLabel('bottom') }}</span></div>
        </div>
      </template>

      <!-- 拖拽中的空区插入热区（贴窗口边缘，业界通用落点提示） -->
      <template v-if="layout.dragging">
        <div
          v-if="!layout.leftPanel"
          class="edge-hot edge-left"
          :class="{ hot: dropZone === 'left' }"
          @dragenter.prevent.stop="dropZone = 'left'"
          @dragover.prevent.stop="dropZone = 'left'"
          @drop.prevent.stop="onZoneDrop('left', $event)"
        ><span>{{ t('workbench.dropLeft') }}</span></div>
        <div
          v-if="!layout.rightPanel"
          class="edge-hot edge-right"
          :class="{ hot: dropZone === 'right' }"
          @dragenter.prevent.stop="dropZone = 'right'"
          @dragover.prevent.stop="dropZone = 'right'"
          @drop.prevent.stop="onZoneDrop('right', $event)"
        ><span>{{ t('workbench.dropRight') }}</span></div>
        <div
          v-if="!layout.bottomPanel"
          class="edge-hot edge-bottom"
          :class="{ hot: dropZone === 'bottom' }"
          @dragenter.prevent.stop="dropZone = 'bottom'"
          @dragover.prevent.stop="dropZone = 'bottom'"
          @drop.prevent.stop="onZoneDrop('bottom', $event)"
        ><span>{{ t('workbench.dropBottom') }}</span></div>
      </template>

      <!-- 悬浮层：面板脱离停靠区后成为自由窗口（可层叠 / 最小化收进右下角层叠条） -->
      <FloatLayer @new-connection="openCreate" @edit="openEdit" />

      <el-footer height="26px" class="status-bar">
        <span v-if="tabsStore.activeTab">
          {{ statusSummary }}
        </span>
        <span v-else>{{ t('app.ready') }}</span>
        <template v-if="connStore.lostIds.size > 0">
          <el-tag size="small" type="danger" effect="plain" class="lost-tag">
            {{ t('conn.lostCount', { n: connStore.lostIds.size }) }}
          </el-tag>
          <el-button size="small" link type="primary" @click="connStore.reconnectLost()">
            {{ t('conn.reconnectAll') }}
          </el-button>
        </template>
        <span class="status-right">
          <span
            class="status-log-toggle"
            :class="{ 'has-unread': logStore.unreadErrors > 0 }"
            :title="t('logs.panel')"
            role="button"
            tabindex="0"
            @click="toggleLogPanel"
            @keydown.enter="toggleLogPanel"
            @keydown.space.prevent="toggleLogPanel"
          >
            <el-icon><component :is="objectIcon('act_monitor')" /></el-icon>
            {{ t('logs.title') }}<template v-if="logStore.unreadErrors > 0">·{{ logStore.unreadErrors }}</template>
          </span>
          <span class="status-sep">|</span>
          PersonalDb v0.1.0
        </span>
      </el-footer>

      <ConnectionForm v-model="formVisible" :initial="editing" @save="onSave" />
      <SettingsDialog v-model="settingsVisible" />
<LogViewer v-model="logViewerVisible" />
<DriverManager v-model="driverManagerVisible" />
<ConfigTransferDialog v-model:visible="transferVisible" initial-tab="export" />
<CommandPalette v-model:visible="paletteVisible" :commands="paletteCommands" />
<UpdateChecker ref="updateCheckerRef" />

      <!-- 关于 -->
      <AboutDialog v-model="aboutVisible" />
    </el-container>
  </el-config-provider>
</template>

<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import zhCn from 'element-plus/es/locale/lang/zh-cn'
import enGb from 'element-plus/es/locale/lang/en'
import {
  ArrowDown,
  Brush,
  Close,
  Coin,
  DataAnalysis,
  Document,
  Folder,
  FullScreen,
  MagicStick,
  Minus,
  Plus,
  Postcard,
  RefreshLeft,
  ScaleToOriginal,
} from '@element-plus/icons-vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'
import ConnectionForm from './components/ConnectionForm.vue'
import PanelHeader from './components/workbench/PanelHeader.vue'
import PanelBody from './components/workbench/PanelBody.vue'
import PanelRail from './components/workbench/PanelRail.vue'
import FloatLayer from './components/workbench/FloatLayer.vue'
import type { ZoneId } from './stores/layout'
// 重型面板异步加载（CodeMirror/结果集增强等大依赖随用随取）
const SettingsDialog = defineAsyncComponent(() => import('./components/SettingsDialog.vue'))
const LogViewer = defineAsyncComponent(() => import('./components/LogViewer.vue'))
const DriverManager = defineAsyncComponent(() => import('./components/DriverManager.vue'))
const ConfigTransferDialog = defineAsyncComponent(() => import('./components/ConfigTransferDialog.vue'))
import { TOURS, maybeAutoTour, startTour } from './tours'
import { useLayoutStore, type PanelId } from './stores/layout'
import CommandPalette from './components/CommandPalette.vue'
import AboutDialog from './components/AboutDialog.vue'
import UpdateChecker from './components/UpdateChecker.vue'
import { useHistoryStore } from './stores/history'
import { useLogStore } from './stores/logs'
import { useConnectionStore } from './stores/connections'
import { useTabsStore } from './stores/tabs'
import { usePreferencesStore } from './stores/preferences'
import { useSnippetStore } from './stores/snippets'
import { useWorkspaceStore } from './stores/workspaces'
import { dbCommit, dbRegistry, dbRollback, dbSetAutocommit } from './api/db'
import { open as openDirDialog } from '@tauri-apps/plugin-dialog'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { dispatchCommand } from './utils/commands'
import { objectIcon } from './utils/objectIcons'
import { describeConnError } from './utils/connError'
import { SHORTCUT_ACTIONS, matchesShortcut } from './utils/shortcuts'
import type { ConnectionConfig, KindInfo } from './types/db'

const { t } = useI18n()
const connStore = useConnectionStore()
const tabsStore = useTabsStore()
const prefs = usePreferencesStore()
const snippetStore = useSnippetStore()
const workspaceStore = useWorkspaceStore()
const layout = useLayoutStore()

/**
 * 菜单栏图标映射（唯一来源，禁止在模板里散落图标判断）。
 * 语义与图标一一对应，见 docs/specifications.md SPEC-UI-8 映射表。
 */
const MENU_ICONS = {
  newConnection: Plus,
  newQuery: Document,
  format: Brush,
  compress: ScaleToOriginal,
  upper: objectIcon('act_upper'),
  lower: objectIcon('act_lower'),
  viewGrid: objectIcon('act_view_grid'),
  viewRecord: Postcard,
  viewPlan: DataAnalysis,
  connPanel: objectIcon('obj_connection'),
  logPanel: objectIcon('act_monitor'),
  resetLayout: RefreshLeft,
  theme: objectIcon('act_theme'),
  snippets: MagicStick,
  drivers: objectIcon('act_driver'),
  transfer: objectIcon('act_export'),
  viewLog: objectIcon('act_monitor'),
  settings: objectIcon('act_settings'),
  tour: objectIcon('act_help'),
  tourItem: objectIcon('act_help'),
  about: objectIcon('act_info'),
  update: objectIcon('act_update'),
}

const epLocale = computed(() => (prefs.locale === 'en' ? enGb : zhCn))

const formVisible = ref(false)
const editing = ref<ConnectionConfig | null>(null)
const settingsVisible = ref(false)
const logViewerVisible = ref(false)
const driverManagerVisible = ref(false)
const transferVisible = ref(false)
const paletteVisible = ref(false)
const historyStore = useHistoryStore()
const logStore = useLogStore()

/** 面板标题（PanelHeader / 状态栏 / 菜单共用） */
function panelTitle(id: PanelId): string {
  return id === 'conn' ? t('tree.title') : id === 'tabs' ? t('workbench.tabs') : t('logs.panel')
}

/** 拖拽落点预览：当前悬停的目标区（业界通用：在真实布局上高亮插入位置） */
const dropZone = ref<ZoneId | null>(null)

watch(
  () => layout.dragging,
  (v) => {
    if (!v) dropZone.value = null
  },
)

/** 停靠区仅在面板拖拽中的 dragover 生效（否则 sortablejs 列排序/文件等原生拖拽会误触发落点预览并卡住遮罩） */
function onZoneDragOver(zone: ZoneId, e: DragEvent): void {
  if (!layout.dragging) return
  e.preventDefault()
  dropZone.value = zone
}

function onZoneDrop(zone: ZoneId, e: DragEvent): void {
  if (!layout.dragging) return
  e.preventDefault()
  const id = layout.dragging
  dropZone.value = null
  layout.endDrag()
  layout.move(id, zone)
}

/** 高亮遮罩文案：空区=停靠提示；占用区=与占用面板交换 */
function zoneMaskLabel(zone: ZoneId): string {
  const occ = layout.panelOf(zone)
  if (!occ || occ === layout.dragging) {
    return zone === 'center' ? t('workbench.dropCenter') : t('workbench.drop' + zone[0].toUpperCase() + zone.slice(1))
  }
  return t('workbench.swap', { name: panelTitle(occ) })
}

/** 停靠区分割线拖拽（left 向右增 / right 向左增 / bottom 向上增） */
function startZoneDrag(zone: 'left' | 'right' | 'bottom', e: MouseEvent): void {
  e.preventDefault()
  const horizontal = zone === 'bottom'
  const start = horizontal ? e.clientY : e.clientX
  const startSize = layout.sizes[zone]
  const sign = zone === 'left' ? 1 : -1
  const onMove = (ev: MouseEvent): void => {
    const delta = (horizontal ? ev.clientY : ev.clientX) - start
    layout.resizeZone(zone, startSize + sign * delta)
  }
  const onUp = (): void => {
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
  }
  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}

/** 面板显隐切换：停靠→隐藏 / 隐藏→唤回 / 悬浮→置焦（中心常驻面板无动作）；日志可见时清零未读 */
function togglePanel(id: PanelId): void {
  const p = layout.panels[id]
  if (p.zone === 'center') return
  if (p.zone === null && p.float) {
    layout.restore(id)
  } else {
    layout.toggle(id)
  }
  const after = layout.panels[id]
  if (id === 'log' && (after.zone !== null || (Boolean(after.float) && !after.minimized))) logStore.markOpened()
}

/** 切换日志面板 */
function toggleLogPanel(): void {
  togglePanel('log')
}

/** 快速还原工作台默认布局（连接左 / 功能区中 / 日志下，尺寸复位） */
function doResetLayout(): void {
  layout.resetLayout()
  ElMessage.success(t('menu.resetLayoutDone'))
}

interface PaletteCmd {
  id: string
  label: string
  group: string
  icon: string
  action: () => void
}

/** 命令面板条目：应用命令 + 连接 + 最近 SQL */
const paletteCommands = computed<PaletteCmd[]>(() => {
  const cmds: PaletteCmd[] = [
    { id: 'new-query', label: t('app.newQuery'), group: t('palette.gApp'), icon: 'obj_query', action: () => void quickNewQuery() },
    { id: 'new-conn', label: t('app.newConnection'), group: t('palette.gApp'), icon: 'obj_connection', action: () => openCreate() },
    { id: 'settings', label: t('settings.title'), group: t('palette.gApp'), icon: 'act_settings', action: () => (settingsVisible.value = true) },
    { id: 'drivers', label: t('driver.title'), group: t('palette.gTools'), icon: 'act_driver', action: () => (driverManagerVisible.value = true) },
    { id: 'transfer', label: t('transfer.title'), group: t('palette.gTools'), icon: 'act_export', action: () => (transferVisible.value = true) },
    { id: 'log', label: t('menu.viewLog'), group: t('palette.gTools'), icon: 'act_monitor', action: () => (logViewerVisible.value = true) },
    { id: 'theme', label: t('menu.toggleTheme'), group: t('palette.gApp'), icon: 'act_theme', action: () => prefs.toggleTheme() },
    { id: 'reconnect-all', label: t('conn.reconnectAll'), group: t('palette.gApp'), icon: 'act_reconnect', action: () => void connStore.reconnectLost() },
    { id: 'tour', label: t('tour.menu'), group: t('palette.gHelp'), icon: 'act_help', action: () => startTour('overview') },
    { id: 'about', label: t('menu.about'), group: t('palette.gHelp'), icon: 'act_info', action: () => (aboutVisible.value = true) },
  ]
  if (sessionsCap.value) {
    cmds.push({ id: 'sessions', label: t('session.tabTitle'), group: t('palette.gTools'), icon: 'obj_session', action: () => openSessions() })
  }
  for (const c of connStore.saved) {
    cmds.push({
      id: `conn:${c.id}`,
      label: c.name,
      group: t('palette.gConn'),
      icon: 'obj_connection',
      action: () => {
        void connStore.openConnection(c).catch((e) => ElMessage.error(describeConnError(e)))
      },
    })
  }
  for (const h of historyStore.list.slice(0, 20)) {
    cmds.push({
      id: `hist:${h.id}`,
      label: h.sql.replace(/\s+/g, ' ').slice(0, 70),
      group: t('palette.gHistory'),
      icon: 'act_history',
      action: () => {
        const tab = tabsStore.activeTab
        if (tab) tab.sql = h.sql
      },
    })
  }
  return cmds
})
const aboutVisible = ref(false)
onMounted(() => {
  connStore.load().catch((e) => console.error('加载连接配置失败', e))
  snippetStore.load().catch((e) => console.error('加载快捷指令失败', e))
  // 工作区：加载并恢复上次工作区页签
  workspaceStore
    .load()
    .then(() => workspaceStore.restoreTabs())
    .catch((e) => console.error('加载工作区失败', e))
  dbRegistry()
    .then((list) => {
      kinds.value = list
    })
    .catch(() => {
      kinds.value = []
    })
  // 前端就绪：关闭 splash、显示主窗（splash 任务）
  invoke('app_ready').catch(() => { /* dev 无 splash 时忽略 */ })
  // 首次启动漫游引导
  maybeAutoTour()
  window.addEventListener('keydown', onGlobalKeydown)
})

// 页签变化（增删/标题/SQL）自动快照到当前工作区（存储侧防抖）
watch(
  () => tabsStore.tabs,
  () => workspaceStore.syncTabs(),
  { deep: true },
)

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onGlobalKeydown)
})

/** 全局快捷键（输入框/编辑器聚焦时不拦截） */
function onGlobalKeydown(e: KeyboardEvent): void {
  // 命令面板：Ctrl+K 全局生效（含编辑器聚焦时）
  if (e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey && e.key.toLowerCase() === 'k') {
    e.preventDefault()
    paletteVisible.value = true
    return
  }
  const target = e.target as HTMLElement | null
  if (target) {
    const tag = target.tagName
    if (tag === 'INPUT' || tag === 'TEXTAREA' || target.closest('.cm-editor')) return
  }
  for (const action of SHORTCUT_ACTIONS) {
    if (action.scope !== 'global') continue
    if (matchesShortcut(e, prefs.shortcutOf(action.key))) {
      e.preventDefault()
      runGlobalAction(action.key)
      return
    }
  }
}

function runGlobalAction(actionKey: string): void {
  switch (actionKey) {
    case 'newConnection':
      openCreate()
      break
    case 'newQuery':
      void quickNewQuery()
      break
    case 'toggleTheme':
      prefs.toggleTheme()
      break
    case 'openSettings':
      settingsVisible.value = true
      break
    case 'toggleLogPanel':
      toggleLogPanel()
      break
    case 'resetLayout':
      doResetLayout()
      break
  }
}

/** —— 事务管理（作用于当前激活查询 Tab 所属连接） —— */
const kinds = ref<KindInfo[]>([])
const txBusy = ref(false)

/** 当前激活页签的连接是否支持会话管理 */
const sessionsCap = computed(() => {
  const tab = tabsStore.activeTab
  if (!tab) return false
  return kinds.value.find((k) => k.kind === tab.kind)?.sessions ?? false
})

function openSessions(): void {
  const tab = tabsStore.activeTab
  if (!tab) return
  tabsStore.openSessionManager({
    connectionId: tab.connectionId,
    connectionName: tab.connectionName,
    kind: tab.kind,
    database: tab.database,
  })
}

/** 检查更新（updater 未配置时提示未启用） */
const updateCheckerRef = ref<InstanceType<typeof import('./components/UpdateChecker.vue').default>>()
async function runUpdateCheck(): Promise<void> {
  const el = updateCheckerRef.value
  if (!el) return
  const res = await el.checkForUpdate()
  if (res.status === 'latest' || res.status === 'disabled' || res.status === 'error') {
    res.status === 'error' ? ElMessage.error(res.message) : ElMessage.info(res.message)
  }
}

const txAvailable = computed(() => {
  const tab = tabsStore.activeTab
  if (!tab || !connStore.isConnected(tab.connectionId)) return false
  return kinds.value.find((k) => k.kind === tab.kind)?.transactions ?? false
})

const txMode = computed<'auto' | 'manual'>(() => {
  const tab = tabsStore.activeTab
  return tab ? connStore.txModeOf(tab.connectionId) : 'auto'
})

/** 手动模式（提交/回滚可用） */
const txManual = computed(() => txMode.value === 'manual')

/** 事务模式分段选项（自动 / 手动） */
const txModeOptions = computed(() => [
  { label: t('tx.autoShort'), value: 'auto' },
  { label: t('tx.manualShort'), value: 'manual' },
])

/** 事务区提示：手动模式补充行为说明，避免「显式提交」被忽略 */
const txTip = computed(() => (txManual.value ? `${t('tx.label')} · ${t('tx.manualHint')}` : t('tx.label')))

async function setTxMode(mode: 'auto' | 'manual'): Promise<void> {
  const tab = tabsStore.activeTab
  if (!tab || mode === txMode.value) return
  const session = connStore.sessionOf(tab.connectionId)
  if (!session) return
  txBusy.value = true
  try {
    await dbSetAutocommit(session.sessionId, mode === 'auto')
    connStore.setTxMode(tab.connectionId, mode)
    ElMessage.success(t('tx.modeChanged', { mode: t('tx.' + mode) }))
  } catch (e) {
    ElMessage.error(t('tx.failed', { msg: String(e) }))
  } finally {
    txBusy.value = false
  }
}

async function doCommit(): Promise<void> {
  const tab = tabsStore.activeTab
  const session = tab ? connStore.sessionOf(tab.connectionId) : undefined
  if (!session) return
  txBusy.value = true
  try {
    await dbCommit(session.sessionId)
    ElMessage.success(t('tx.committed'))
  } catch (e) {
    ElMessage.error(t('tx.failed', { msg: String(e) }))
  } finally {
    txBusy.value = false
  }
}

async function doRollback(): Promise<void> {
  const tab = tabsStore.activeTab
  const session = tab ? connStore.sessionOf(tab.connectionId) : undefined
  if (!session) return
  txBusy.value = true
  try {
    await dbRollback(session.sessionId)
    ElMessage.success(t('tx.rolledBack'))
  } catch (e) {
    ElMessage.error(t('tx.failed', { msg: String(e) }))
  } finally {
    txBusy.value = false
  }
}

function openCreate(): void {
  editing.value = null
  formVisible.value = true
}

function openEdit(cfg: ConnectionConfig): void {
  editing.value = cfg
  formVisible.value = true
}

async function onSave(cfg: ConnectionConfig): Promise<void> {
  await connStore.upsert(cfg)
  formVisible.value = false
  layout.bumpRefresh()
  ElMessage.success(t('form.saveSuccess'))
}

async function quickNewQuery(): Promise<void> {
  const cfg = connStore.saved[0]
  if (!cfg) {
    ElMessage.warning(t('app.noConnectionToQuery'))
    openCreate()
    return
  }
  try {
    await connStore.openConnection(cfg)
  } catch (e) {
    ElMessage.error(t('common.connectionFailed', { msg: describeConnError(e) }))
    return
  }
  tabsStore.openTab({
    connectionId: cfg.id,
    connectionName: cfg.name,
    kind: cfg.kind,
    database: cfg.database ?? '',
    force: true,
  })
}

/** —— 工作区 —— */
async function onWorkspaceCommand(cmd: string): Promise<void> {
  if (cmd.startsWith('switch:')) {
    await workspaceStore.switchTo(cmd.slice(7))
    return
  }
  const cur = workspaceStore.current
  switch (cmd) {
    case 'new': {
      try {
        const { value } = await ElMessageBox.prompt(t('workspace.name'), t('workspace.new'), {
          confirmButtonText: t('common.confirm'),
          cancelButtonText: t('common.cancel'),
          inputPattern: /\S+/,
          inputErrorMessage: t('workspace.nameRequired'),
        })
        await workspaceStore.create(value.trim())
      } catch {
        // 取消
      }
      break
    }
    case 'rename': {
      if (!cur) return
      try {
        const { value } = await ElMessageBox.prompt(t('workspace.name'), t('workspace.rename'), {
          confirmButtonText: t('common.confirm'),
          cancelButtonText: t('common.cancel'),
          inputValue: cur.name,
          inputPattern: /\S+/,
          inputErrorMessage: t('workspace.nameRequired'),
        })
        await workspaceStore.rename(cur.id, value.trim())
      } catch {
        // 取消
      }
      break
    }
    case 'exportDir': {
      if (!cur) return
      const dir = await openDirDialog({ directory: true })
      if (!dir) return
      await workspaceStore.setExportDir(cur.id, dir)
      ElMessage.success(t('workspace.exportDirSet', { dir }))
      break
    }
    case 'changeDir': {
      if (!cur) return
      const dir = await openDirDialog({ directory: true, title: t('workspace.selectDir', { name: cur.name }) })
      if (!dir) return
      try {
        await workspaceStore.setDir(cur.id, dir)
        ElMessage.success(t('workspace.exportDirSet', { dir }))
      } catch (e) {
        ElMessage.error(String(e))
      }
      break
    }
    case 'openDir': {
      if (!cur?.exportDir) {
        ElMessage.info(t('workspace.noExportDir'))
        return
      }
      void revealItemInDir(cur.exportDir).catch((e) => ElMessage.error(String(e)))
      break
    }
    case 'delete': {
      if (!cur) return
      try {
        await ElMessageBox.confirm(
          t('workspace.deleteConfirm', { name: cur.name }),
          t('workspace.delete'),
          { type: 'warning' },
        )
      } catch {
        return
      }
      await workspaceStore.remove(cur.id)
      break
    }
  }
}

/** 菜单栏命令分发 */
function onMenuSelect(index: string): void {
  if (index.startsWith('tour:')) {
    startTour(index.slice(5))
    return
  }
  const tabId = tabsStore.activeTabId
  switch (index) {
    case 'new-connection':
      openCreate()
      break
    case 'new-query':
      void quickNewQuery()
      break
    case 'toggle-theme':
      prefs.toggleTheme()
      break
      case 'toggle-log-panel':
        toggleLogPanel()
        break
      case 'reset-layout':
        doResetLayout()
        break
    case 'toggle-conn-panel':
      togglePanel('conn')
      break
    case 'settings':
      settingsVisible.value = true
      break
    case 'drivers':
      driverManagerVisible.value = true
      break
    case 'config-transfer':
      transferVisible.value = true
      break
    case 'view-log':
      logViewerVisible.value = true
      break
    case 'config-transfer':
      transferVisible.value = true
      break
    case 'about':
      aboutVisible.value = true
      break
    case 'check-update':
      void runUpdateCheck()
      break
    case 'format':
    case 'compress':
    case 'upper':
    case 'lower':
    case 'view-grid':
    case 'view-record':
    case 'view-plan':
    case 'snippets':
      dispatchCommand(index, tabId)
      break
  }
}

const statusSummary = computed(() => {
  const tab = tabsStore.activeTab
  if (!tab) return t('app.ready')
  const parts = [`${tab.connectionName}${tab.database ? ` @ ${tab.database}` : ''}`]
  if (tab.running) parts.push(t('app.running'))
  else if (tab.error) parts.push(t('app.error'))
  else if (tab.result) {
    if (tab.result.columns.length > 0) {
      parts.push(`${t('common.rows', { n: tab.result.rows.length })} / ${tab.result.durationMs} ms`)
      if (tab.result.truncated) parts.push(t('app.truncated'))
    } else {
      parts.push(`${tab.result.durationMs} ms`)
    }
  }
  return parts.join('  ·  ')
})

/** —— 自定义标题栏窗口控制 —— */
const appWindow = getCurrentWindow()

function minimizeWin(): void {
  void appWindow.minimize()
}

function toggleMaxWin(): void {
  void appWindow.toggleMaximize()
}

function closeWin(): void {
  void appWindow.close()
}
</script>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

:root {
  --app-header-bg: var(--el-bg-color);
  --app-header-fg: var(--el-text-color-primary);
  --app-header-muted: var(--el-text-color-secondary);
  --app-header-btn-border: var(--el-border-color);
  --app-header-hover-bg: var(--el-fill-color);
  --app-sidebar-bg: #fafbfc;
  --app-content-bg: #ffffff;
  --app-panel-bg: #f5f7fa;
  --app-welcome-bg: linear-gradient(160deg, #f6f8fb 0%, #eef2f7 100%);
}

html.dark {
  --app-header-bg: var(--el-bg-color);
  --app-header-fg: var(--el-text-color-primary);
  --app-header-muted: var(--el-text-color-secondary);
  --app-header-btn-border: var(--el-border-color);
  --app-header-hover-bg: var(--el-fill-color);
  --app-sidebar-bg: #101418;
  --app-content-bg: #141414;
  --app-panel-bg: #1b1b1b;
  --app-welcome-bg: linear-gradient(160deg, #0f1216 0%, #171a1f 100%);
}

html,
body,
#app {
  height: 100%;
  font-family: 'Segoe UI', 'Microsoft YaHei', sans-serif;
  font-size: 13px;
  overflow: hidden;
}

body {
  background: var(--app-content-bg);
  user-select: none;
}

.app-root {
  height: calc(100% - 32px);
  display: flex;
  flex-direction: column;
}

/* —— 自定义标题栏（跟随主题） —— */
.titlebar {
  height: 32px;
  display: flex;
  align-items: stretch;
  background: var(--app-panel-bg);
  border-bottom: 1px solid var(--el-border-color-light);
}

.tb-brand {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 12px;
  -webkit-app-region: drag;
}

.tb-name {
  font-size: 12.5px;
  font-weight: 700;
  color: var(--el-color-primary);
}

.tb-drag {
  flex: 1;
  -webkit-app-region: drag;
}

.tb-controls {
  display: flex;
  align-items: stretch;
}

.tb-btn {
  width: 46px;
  border: none;
  background: transparent;
  color: var(--el-text-color-secondary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background-color 0.15s, color 0.15s;
}

.tb-btn :deep(.el-icon) {
.tb-brand :deep(.el-icon) {
  font-size: var(--pd-icon-sm);
}

  font-size: var(--pd-icon-sm);
}

.tb-btn:focus-visible {
  outline: 2px solid var(--el-color-primary);
  outline-offset: -2px;
}

.tb-btn:hover {
  background: var(--el-fill-color);
  color: var(--el-text-color-primary);
}

.tb-btn.tb-close:hover {
  background: var(--el-color-danger);
  color: #fff;
}

/* 工作区下拉：当前项高亮（teleport 到 body，需全局样式） */
.el-dropdown-menu .is-active-ws {
  color: var(--el-color-primary);
  font-weight: 600;
}

</style>

<style scoped>
.app-header {
  display: flex;
  align-items: center;
  gap: 6px;
  border-bottom: 1px solid var(--el-border-color-light);
  background: var(--app-header-bg);
  color: var(--app-header-fg);
}

/* 按钮间距交给容器 gap，去掉 EP 默认相邻外边距 */
.app-header :deep(.el-button + .el-button) {
  margin-left: 0;
}

.brand {
  font-size: 15px;
  font-weight: 700;
  color: var(--el-color-primary);
  letter-spacing: 0.5px;
}

.app-header :deep(.el-button:not(.is-text)) {
  color: var(--app-header-fg);
  border-color: var(--app-header-btn-border);
  background: transparent;
}

.app-header :deep(.el-button:not(.is-text):hover) {
  color: var(--el-color-primary);
  border-color: var(--el-color-primary);
  background: var(--app-header-hover-bg);
}

/* 文本型按钮（主题/设置）：保持无边框透明，hover 仅加浅底 */
.app-header :deep(.el-button.is-text) {
  color: var(--app-header-fg);
  border: none;
  background: transparent;
}

.app-header :deep(.el-button.is-text:hover) {
  color: var(--el-color-primary);
  background: var(--app-header-hover-bg);
}

.header-spacer {
  flex: 1;
}

/* 连接状态 chip：已连接=成功色，未连接=中性 */
.conn-state {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 22px;
  padding: 0 9px;
  border-radius: 11px;
  background: var(--el-fill-color-lighter);
  font-size: 12px;
  color: var(--app-header-muted);
  white-space: nowrap;
}

.conn-state-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
  opacity: 0.5;
}

.conn-state.is-on {
  color: var(--el-color-success);
  background: var(--el-color-success-light-9);
}

.conn-state.is-on .conn-state-dot {
  opacity: 1;
}

/* 工具栏图标按钮（主题 / 首选项 / 会话），与标题栏按钮同型 */
.hdr-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 24px;
  border: none;
  border-radius: var(--pd-radius);
  background: transparent;
  color: var(--app-header-fg);
  cursor: pointer;
  transition: background-color 0.15s, color 0.15s;
}

.hdr-btn :deep(.el-icon) {
  font-size: var(--pd-icon-md);
}

.hdr-btn:focus-visible {
  outline: 2px solid var(--el-color-primary);
  outline-offset: -2px;
}

.hdr-btn:hover {
  background: var(--app-header-hover-bg);
  color: var(--el-color-primary);
}

/* 工具栏工作区下拉（主题化紧凑） */
.ws-btn {
  padding: 0 6px;
  height: 24px;
  gap: 4px;
}

.ws-btn :deep(.el-icon) {
  font-size: var(--pd-icon-sm);
}

.ws-name {
  font-size: 12px;
  color: var(--app-header-fg);
  max-width: 140px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ws-arrow {
  color: var(--app-header-muted);
}

.app-header :deep(.ws-btn:hover) {
  background: var(--app-header-hover-bg);
}

.app-header :deep(.ws-btn:hover .ws-name),
.app-header :deep(.ws-btn:hover .ws-arrow) {
  color: var(--el-color-primary);
}

/* 工具栏事务区：模式分段（自动/手动）+ 提交/回滚图标按钮（手动模式可用，自动模式禁用但常驻，布局不跳动） */
.tx-lead {
  display: inline-flex;
  align-items: center;
  color: var(--app-header-muted);
}

.tx-lead :deep(.el-icon) {
  font-size: var(--pd-icon-md);
}

.tx-mode {
  height: 24px;
  font-size: 12px;
  --el-segmented-item-selected-bg-color: var(--el-color-primary-light-9);
  --el-segmented-item-selected-color: var(--el-color-primary);
}

.tx-acts {
  display: inline-flex;
  align-items: center;
}

.tx-slot {
  display: inline-flex;
}

.tx-act {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 24px;
  border: none;
  border-radius: var(--pd-radius);
  background: transparent;
  color: var(--app-header-fg);
  cursor: pointer;
  transition: background-color 0.15s, color 0.15s;
}

.tx-act :deep(.el-icon) {
  font-size: var(--pd-icon-md);
}

.tx-act:focus-visible {
  outline: 2px solid var(--el-color-primary);
  outline-offset: -2px;
}

.tx-act:disabled {
  opacity: 0.38;
  cursor: not-allowed;
}

/* 提交 = 主色（确认），回滚 = 危险色（破坏性） */
.tx-act.is-commit:hover:not(:disabled) {
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
}

.tx-act.is-rollback:hover:not(:disabled) {
  background: var(--el-color-danger-light-9);
  color: var(--el-color-danger);
}

.app-body {
  flex: 1;
  min-height: 0;
}

/* 顶部：菜单栏（最细，跟随主题）→ 工具栏（品牌深色） */
.app-menu {
  height: 26px;
  padding-left: 6px;
  border-bottom: 1px solid var(--el-border-color-light);
  border-right: none;
  background: var(--app-panel-bg);
  --el-menu-border-color: transparent;
  --el-menu-bg-color: transparent;
  --el-menu-hover-bg-color: var(--el-fill-color);
  --el-menu-active-color: var(--el-color-primary);
}

.app-menu :deep(.el-menu-item),
.app-menu :deep(.el-sub-menu__title) {
  height: 26px;
  line-height: 26px;
  font-size: 12px;
  padding: 0 8px;
  border-bottom: none;
  border-radius: 4px;
  margin: 0 1px;
  transition: background-color 0.15s, color 0.15s;
}

/* 放弃下拉箭头：水平菜单 hover 即弹出，箭头与文本重叠且无必要 */
.app-menu :deep(.el-sub-menu__icon-arrow) {
  display: none;
}

.app-menu :deep(.el-sub-menu__title:hover),
.app-menu :deep(.el-menu-item:hover) {
  background: var(--el-fill-color);
  color: var(--el-color-primary);
}

.app-menu :deep(.el-sub-menu.is-active .el-sub-menu__title) {
  color: var(--el-color-primary);
  border-bottom: none;
}

/* 下拉菜单（el-menu--popup）由 EP teleport 到 body，样式在 styles/overrides.css（紧凑菜单 + 分组分隔线） */

.app-header :deep(.el-divider--vertical) {
  border-color: var(--app-header-btn-border);
  margin: 0 2px;
}

.splitter-v {
  flex: 0 0 4px;
  cursor: col-resize;
  background: transparent;
  transition: background-color 0.15s;
}
.splitter-v:hover {
  background: var(--el-color-primary-light-7);
}

.splitter-h {
  flex: 0 0 5px;
  cursor: row-resize;
  background: transparent;
  transition: background-color 0.15s;
}
.splitter-h:hover {
  background: var(--el-color-primary-light-7);
}

/* 工作台停靠面板（左/右/下，可拖拽重排与悬浮） */
.wb-dock {
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  min-height: 0;
  overflow: hidden;
  background: var(--app-sidebar-bg, var(--el-bg-color));
  position: relative;
}
/* 区间边界：左/右停靠区朝向主区的 1px 边线（分隔线叠加其上，悬停高亮） */
.wb-dock.is-left {
  border-right: var(--pd-band-border);
}
.wb-dock.is-right {
  border-left: var(--pd-band-border);
}
.wb-bottom {
  border-top: 1px solid var(--el-border-color-lighter);
}
.wb-dock-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.wb-dock-body > :deep(*) {
  flex: 1;
  min-height: 0;
}
.wb-center {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
}
.wb-center > :deep(*) {
  flex: 1;
  min-height: 0;
}
/* 换入主区的面板有面板头时，头部保持自然高度，内容区占满剩余空间 */
.wb-center > :deep(.panel-header) {
  flex: 0 0 auto;
}

/* 拖拽落点预览：悬停区域高亮 + 来源面板半透明（业界通用交互） */
.is-drag-source {
  opacity: 0.45;
}
.drop-mask {
  position: absolute;
  inset: 0;
  z-index: 6;
  display: flex;
  align-items: center;
  justify-content: center;
  background: color-mix(in srgb, var(--el-color-primary) 12%, transparent);
  border: 1.5px dashed var(--el-color-primary);
  border-radius: 6px;
  pointer-events: none;
}
.drop-mask span {
  background: var(--el-bg-color);
  border: 1px solid var(--el-color-primary-light-5);
  color: var(--el-color-primary);
  font-size: 12px;
  padding: 2px 12px;
  border-radius: 10px;
  white-space: nowrap;
}

/* 拖拽中空区插入热区（贴窗口边缘） */
.edge-hot {
  position: fixed;
  z-index: 3000;
  display: flex;
  align-items: center;
  justify-content: center;
  background: color-mix(in srgb, var(--el-color-primary) 6%, transparent);
  border: 1.5px dashed var(--el-color-primary-light-5);
}
.edge-hot span {
  font-size: 12px;
  color: var(--el-color-primary);
  background: var(--el-bg-color);
  border: 1px solid var(--el-color-primary-light-5);
  padding: 2px 8px;
  border-radius: 10px;
  white-space: nowrap;
  opacity: 0;
  transition: opacity 0.12s;
}
.edge-hot.hot {
  background: color-mix(in srgb, var(--el-color-primary) 14%, transparent);
  border-style: solid;
}
.edge-hot.hot span {
  opacity: 1;
}
.edge-left {
  left: var(--pd-rail-w);
  top: 96px;
  bottom: 150px;
  width: 26px;
  border-left: none;
}
.edge-left span {
  writing-mode: vertical-lr;
  letter-spacing: 2px;
}
.edge-right {
  right: 0;
  top: 96px;
  bottom: 150px;
  width: 26px;
  border-right: none;
}
.edge-right span {
  writing-mode: vertical-lr;
  letter-spacing: 2px;
}
.edge-bottom {
  left: 60px;
  right: 60px;
  bottom: 26px;
  height: 26px;
  border-bottom: none;
}

/* 关于 */
.about {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 12px 0 4px;
  text-align: center;
}
.about-logo {
  font-size: 30px;
  font-weight: 800;
  letter-spacing: 1px;
  color: var(--el-color-primary);
}
.about-sub {
  font-size: 13px;
  color: var(--el-text-color-secondary);
}
.about-ver {
  margin-top: -2px;
}
.about-stack {
  display: flex;
  gap: 6px;
  margin-top: 4px;
}
.about-credits {
  margin-top: 14px;
  padding-top: 12px;
  border-top: 1px solid var(--el-border-color-lighter);
  width: 100%;
  font-size: 12px;
  color: var(--el-text-color-placeholder);
  line-height: 1.6;
}
.status-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 12px;
  border-top: 1px solid var(--el-border-color-light);
  background: var(--app-panel-bg);
  color: var(--el-text-color-secondary);
  font-size: 12px;
  line-height: 26px;
}

.status-right {
  user-select: none;
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.status-sep {
  opacity: 0.4;
}
.status-log-toggle {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  cursor: pointer;
  color: var(--el-text-color-secondary);
  padding: 0 5px;
  border-radius: 3px;
}

.status-log-toggle :deep(.el-icon) {
  font-size: var(--pd-icon-sm);
}

.status-log-toggle:focus-visible {
  outline: 2px solid var(--el-color-primary);
  outline-offset: 0;
}

.status-log-toggle:hover {
  color: var(--el-color-primary);
  background: var(--el-fill-color-light);
}
.status-log-toggle.has-unread {
  color: var(--el-color-danger);
  font-weight: 600;
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
}
.tab-list-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 6px;
  border-radius: 4px;
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
.tl-conn {
  color: var(--el-text-color-placeholder);
  font-size: 11px;
  max-width: 110px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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

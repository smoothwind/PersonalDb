<template>
  <aside class="panel-rail" data-tour="rail">
    <el-tooltip v-for="it in items" :key="it.id" :content="it.tip" placement="right" :show-after="400">
      <button class="rail-btn" :class="{ 'is-on': it.on }" :data-rail="it.id" :aria-label="it.tip" :title="it.tip" @click="onClick(it.id)">
        <el-icon><component :is="objectIcon(it.icon)" /></el-icon>
        <span v-if="it.badge" class="rail-badge">{{ it.badge > 99 ? '99+' : it.badge }}</span>
      </button>
    </el-tooltip>
  </aside>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { objectIcon } from '../../utils/objectIcons'
import { useLayoutStore, type PanelId } from '../../stores/layout'
import { useLogStore } from '../../stores/logs'
import { usePreferencesStore } from '../../stores/preferences'

/**
 * 主窗左侧常驻面板条（Activity Bar 模式）：
 * 面板的常驻开关 + 隐藏后的快速唤回入口；宽度固定、吸附主窗左边（位于所有停靠区之前）。
 */
const { t } = useI18n()
const layout = useLayoutStore()
const logStore = useLogStore()
const prefs = usePreferencesStore()

interface RailItem {
  id: PanelId
  icon: string
  tip: string
  on: boolean
  badge: number
}

/** 面板可见性：停靠（含中心常驻）或未最小化的悬浮窗 */
function isVisible(id: PanelId): boolean {
  const p = layout.panels[id]
  return p.zone !== null || (Boolean(p.float) && !p.minimized)
}

/** 提示：面板名（+ 快捷键 / 常驻中心） */
function railTip(id: PanelId, name: string): string {
  const parts = [name]
  if (layout.panels[id].zone === 'center') parts.push(t('workbench.pinned'))
  if (id === 'log') {
    const key = prefs.shortcutOf('toggleLogPanel')
    if (key) parts.push(key)
  }
  return parts.join(' · ')
}

const items = computed<RailItem[]>(() => [
  { id: 'conn', icon: 'obj_connection', tip: railTip('conn', t('tree.title')), on: isVisible('conn'), badge: 0 },
  { id: 'tabs', icon: 'act_workspace', tip: railTip('tabs', t('workbench.tabs')), on: isVisible('tabs'), badge: 0 },
  { id: 'log', icon: 'act_monitor', tip: railTip('log', t('logs.panel')), on: isVisible('log'), badge: logStore.unreadErrors },
])

/** 点击：停靠→隐藏 / 隐藏→唤回 / 悬浮→置焦（中心常驻面板无动作） */
function onClick(id: PanelId): void {
  const p = layout.panels[id]
  if (p.zone === 'center') return
  if (p.zone === null && p.float) {
    layout.restore(id)
  } else {
    layout.toggle(id)
  }
  if (id === 'log' && isVisible('log')) logStore.markOpened()
}
</script>

<style scoped>
/* 导航条（Activity Bar）：恒定吸附窗口最左，容纳面板开关 */
.panel-rail {
  flex: 0 0 var(--pd-rail-w);
  width: var(--pd-rail-w);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  padding: 6px 0;
  background: var(--app-sidebar-bg, var(--el-bg-color));
  border-right: var(--pd-band-border);
}
/* 状态区分（VSCode Activity Bar 同型）：
   隐藏 = 灰阶无底；可见 = 主色图标 + 淡主色底 + 左缘指示条；悬停 = 提亮 */
.rail-btn {
  position: relative;
  width: 36px;
  height: 36px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: var(--pd-radius);
  background: transparent;
  color: var(--el-text-color-placeholder);
  cursor: pointer;
  transition: background-color 0.15s, color 0.15s;
}
.rail-btn :deep(.el-icon) {
  font-size: var(--pd-icon-lg);
}
/* 键盘焦点环（原生 button 无默认样式，SPEC-UI-30） */
.rail-btn:focus-visible {
  outline: 2px solid var(--el-color-primary);
  outline-offset: -2px;
}
/* 隐藏态：图标明显压暗，与可见态拉开层次 */
.rail-btn:not(.is-on) :deep(svg) {
  opacity: 0.45;
}
.rail-btn:hover {
  background: var(--el-fill-color);
  color: var(--el-text-color-primary);
}
.rail-btn:not(.is-on):hover :deep(svg) {
  opacity: 1;
}
.rail-btn.is-on {
  color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
}
.rail-btn.is-on:hover {
  background: var(--el-color-primary-light-8);
  color: var(--el-color-primary);
}
/* 可见态指示条：贴主窗左边缘 */
.rail-btn.is-on::before {
  content: '';
  position: absolute;
  left: calc((var(--pd-rail-w) - 36px) / -2);
  top: 7px;
  bottom: 7px;
  width: 2px;
  border-radius: 0 2px 2px 0;
  background: var(--el-color-primary);
}
.rail-badge {
  position: absolute;
  top: 2px;
  right: 2px;
  min-width: 14px;
  height: 14px;
  padding: 0 3px;
  border-radius: 7px;
  background: var(--el-color-danger);
  color: #fff;
  font-size: 9.5px;
  line-height: 14px;
  text-align: center;
  pointer-events: none;
}
</style>

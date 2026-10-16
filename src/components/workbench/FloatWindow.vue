<template>
  <div
    class="float-win"
    :style="{
      left: geom.x + 'px',
      top: geom.y + 'px',
      width: geom.w + 'px',
      height: geom.h + 'px',
      zIndex: z,
    }"
    @mousedown="layout.focusFloat(panel)"
  >
    <div
      class="fw-head"
      @mousedown.prevent="startMove"
      @dblclick="layout.minimize(panel)"
    >
      <span class="fw-grip"><i /><i /><i /></span>
      <span class="fw-title">{{ title }}</span>
      <span class="fw-spacer" />
      <el-tooltip :content="t('workbench.dock')" placement="bottom">
        <button class="fw-btn" :title="t('workbench.dock')" @click.stop="layout.dockBack(panel)">
          <el-icon><component :is="objectIcon('act_dock')" /></el-icon>
        </button>
      </el-tooltip>
      <el-tooltip :content="t('workbench.minimize')" placement="bottom">
        <button class="fw-btn" :title="t('workbench.minimize')" @click.stop="layout.minimize(panel)">
          <el-icon><Minus /></el-icon>
        </button>
      </el-tooltip>
      <el-tooltip :content="t('workbench.hide')" placement="bottom">
        <button class="fw-btn" :title="t('workbench.hide')" @click.stop="layout.hide(panel)">
          <el-icon><component :is="objectIcon('act_hide')" /></el-icon>
        </button>
      </el-tooltip>
    </div>
    <div class="fw-body">
      <slot />
    </div>
    <span class="fw-resize fw-r" @mousedown.prevent="startResize($event, 'right')" />
    <span class="fw-resize fw-b" @mousedown.prevent="startResize($event, 'bottom')" />
    <span class="fw-resize fw-br" @mousedown.prevent="startResize($event, 'br')" />
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Minus } from '@element-plus/icons-vue'
import { objectIcon } from '../../utils/objectIcons'
import { useLayoutStore, type PanelId } from '../../stores/layout'

const props = defineProps<{
  panel: PanelId
  title: string
}>()

const { t } = useI18n()
const layout = useLayoutStore()

const geom = computed(() => layout.panels[props.panel].float!)
const z = computed(() => layout.zOf(props.panel))

function startMove(e: MouseEvent): void {
  const g = { ...geom.value }
  const startX = e.clientX
  const startY = e.clientY
  const onMove = (ev: MouseEvent): void => {
    layout.setGeom(props.panel, { x: g.x + (ev.clientX - startX), y: g.y + (ev.clientY - startY) })
  }
  const onUp = (): void => {
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
  }
  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}

function startResize(e: MouseEvent, edge: 'right' | 'bottom' | 'br'): void {
  const g = { ...geom.value }
  const startX = e.clientX
  const startY = e.clientY
  const onMove = (ev: MouseEvent): void => {
    const dx = ev.clientX - startX
    const dy = ev.clientY - startY
    if (edge === 'right' || edge === 'br') layout.setGeom(props.panel, { w: g.w + dx })
    if (edge === 'bottom' || edge === 'br') layout.setGeom(props.panel, { h: g.h + dy })
  }
  const onUp = (): void => {
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
  }
  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}
</script>

<style scoped>
.float-win {
  position: fixed;
  display: flex;
  flex-direction: column;
  min-width: 280px;
  min-height: 180px;
  background: var(--el-bg-color);
  border: 1px solid var(--el-border-color);
  border-radius: 8px;
  box-shadow: var(--el-box-shadow-dark);
  overflow: hidden;
}
.fw-head {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 6px 0 8px;
  background: var(--el-fill-color-light);
  border-bottom: 1px solid var(--el-border-color-lighter);
  cursor: move;
  user-select: none;
  flex-shrink: 0;
}
.fw-grip {
  display: inline-flex;
  gap: 2px;
}
.fw-grip i {
  width: 3px;
  height: 3px;
  border-radius: 50%;
  background: var(--el-text-color-placeholder);
  box-shadow:
    0 -3px 0 var(--el-text-color-placeholder),
    0 3px 0 var(--el-text-color-placeholder);
}
.fw-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--el-text-color-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.fw-spacer {
  flex: 1;
}
.fw-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--el-text-color-secondary);
  cursor: pointer;
  padding: 0;
}
.fw-btn :deep(.el-icon) {
  font-size: var(--pd-icon-sm);
}
.fw-btn:hover {
  background: var(--el-fill-color);
  color: var(--el-color-primary);
}
.fw-btn:focus-visible {
  outline: 2px solid var(--el-color-primary);
  outline-offset: -1px;
}
.fw-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.fw-body > :deep(*) {
  flex: 1;
  min-height: 0;
}
.fw-resize {
  position: absolute;
  z-index: 2;
}
.fw-r {
  top: 28px;
  right: -2px;
  bottom: 6px;
  width: 6px;
  cursor: ew-resize;
}
.fw-b {
  left: 6px;
  right: 6px;
  bottom: -2px;
  height: 6px;
  cursor: ns-resize;
}
.fw-br {
  right: -2px;
  bottom: -2px;
  width: 12px;
  height: 12px;
  cursor: nwse-resize;
}
</style>

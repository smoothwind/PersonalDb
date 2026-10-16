<template>
  <div
    class="panel-header"
    :class="{ 'is-dragging': dragging }"
    draggable="true"
    @dragstart="onDragStart"
    @dragend="onDragEnd"
    @dblclick="onDblclick"
  >
    <span class="ph-grip" title="">
      <i /><i /><i />
    </span>
    <span class="ph-title">{{ title }}</span>
    <span class="ph-spacer" />
    <el-tooltip v-if="canFloat" :content="t('workbench.float')" placement="bottom">
      <button class="ph-btn" :title="t('workbench.float')" @click="layout.float(panel)">
        <el-icon><component :is="objectIcon('act_float')" /></el-icon>
      </button>
    </el-tooltip>
    <el-tooltip v-if="closable" :content="t('workbench.hide')" placement="bottom">
      <button class="ph-btn" :title="t('workbench.hide')" @click="layout.hide(panel)">
        <el-icon><component :is="objectIcon('act_hide')" /></el-icon>
      </button>
    </el-tooltip>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { objectIcon } from '../../utils/objectIcons'
import { useLayoutStore, type PanelId } from '../../stores/layout'

const props = defineProps<{
  panel: PanelId
  title: string
}>()

const { t } = useI18n()
const layout = useLayoutStore()

const dragging = computed(() => layout.dragging === props.panel)
const canFloat = computed(() => layout.panels[props.panel].zone !== 'center')
/** 中心区面板不可隐藏（中心必须常驻内容） */
const closable = computed(() => layout.panels[props.panel].zone !== 'center')

function onDragStart(e: DragEvent): void {
  layout.beginDrag(props.panel)
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = 'move'
    e.dataTransfer.setData('text/x-pd-panel', props.panel)
  }
}

function onDragEnd(): void {
  layout.endDrag()
}

/** 双击面板头：停靠 ⇄ 悬浮（业界通用；中心区面板不可悬浮） */
function onDblclick(): void {
  const p = layout.panels[props.panel]
  if (p.zone === 'center') return
  if (p.zone === null) layout.dockBack(props.panel)
  else layout.float(props.panel)
}
</script>

<style scoped>
.panel-header {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 26px;
  padding: 0 6px 0 8px;
  flex-shrink: 0;
  border-bottom: var(--pd-band-border);
  background: var(--pd-band-bg);
  user-select: none;
}
.ph-grip {
  display: inline-flex;
  gap: 2px;
  cursor: grab;
  padding: 2px 2px;
}
.ph-grip i {
  width: 3px;
  height: 3px;
  border-radius: 50%;
  background: var(--el-text-color-placeholder);
  box-shadow:
    0 -3px 0 var(--el-text-color-placeholder),
    0 3px 0 var(--el-text-color-placeholder);
}
.panel-header:active .ph-grip {
  cursor: grabbing;
}
.is-dragging {
  opacity: 0.6;
}
.ph-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--el-text-color-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.ph-spacer {
  flex: 1;
}
.ph-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--el-text-color-secondary);
  cursor: pointer;
  padding: 0;
}

.ph-btn :deep(.el-icon) {
  font-size: var(--pd-icon-sm);
}

.ph-btn:focus-visible {
  outline: 2px solid var(--el-color-primary);
  outline-offset: -1px;
}
.ph-btn:hover {
  background: var(--el-fill-color);
  color: var(--el-color-primary);
}
</style>

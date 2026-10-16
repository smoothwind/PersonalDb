<template>
  <template v-for="id in layout.floatingPanels" :key="id">
    <FloatWindow v-if="!layout.panels[id].minimized" :panel="id" :title="panelTitle(id)">
      <PanelBody :id="id" embedded @new-connection="emit('newConnection')" @edit="(c) => emit('edit', c)" />
    </FloatWindow>
  </template>

  <!-- 最小化层叠条：右下角堆叠，点击还原 -->
  <div v-if="minimized.length > 0" class="float-chips">
    <button
      v-for="id in minimized"
      :key="id"
      class="chip"
      :title="panelTitle(id)"
      @click="layout.restore(id)"
    >
      <el-icon><component :is="objectIcon(panelIcon(id))" /></el-icon>
      <span>{{ panelTitle(id) }}</span>
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import FloatWindow from './FloatWindow.vue'
import PanelBody from './PanelBody.vue'
import { objectIcon } from '../../utils/objectIcons'
import { useLayoutStore, type PanelId } from '../../stores/layout'
import type { ConnectionConfig } from '../../types/db'

const emit = defineEmits<{
  (e: 'newConnection'): void
  (e: 'edit', cfg: ConnectionConfig): void
}>()

const { t } = useI18n()
const layout = useLayoutStore()

const minimized = computed(() => layout.floatingPanels.filter((id) => layout.panels[id].minimized))

function panelTitle(id: PanelId): string {
  return id === 'conn' ? t('tree.title') : id === 'tabs' ? t('workbench.tabs') : t('logs.panel')
}

function panelIcon(id: PanelId): string {
  return id === 'conn' ? 'connection' : id === 'tabs' ? 'query' : 'query'
}
</script>

<style scoped>
.float-chips {
  position: fixed;
  right: 12px;
  bottom: 34px;
  z-index: 2900;
  display: flex;
  flex-direction: column-reverse;
  gap: 6px;
  align-items: flex-end;
}
.chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  max-width: 220px;
  height: 26px;
  padding: 0 10px;
  border: 1px solid var(--el-border-color);
  border-radius: 13px;
  background: var(--el-bg-color);
  color: var(--el-text-color-regular);
  font-size: 12px;
  cursor: pointer;
  box-shadow: var(--el-box-shadow-light);
}
.chip :deep(.el-icon) {
  font-size: var(--pd-icon-sm);
}
.chip:focus-visible {
  outline: 2px solid var(--el-color-primary);
  outline-offset: 1px;
}
.chip:hover {
  color: var(--el-color-primary);
  border-color: var(--el-color-primary-light-5);
}
.chip span {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>

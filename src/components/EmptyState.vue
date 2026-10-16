<template>
  <div class="empty-state" :class="{ 'is-compact': size === 'compact' }">
    <el-icon :size="iconSize" color="var(--el-border-color)">
      <component :is="objectIcon(icon)" />
    </el-icon>
    <div class="es-title">{{ title }}</div>
    <div v-if="hint" class="es-hint">{{ hint }}</div>
    <div v-if="$slots.action" class="es-action">
      <slot name="action" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { objectIcon } from '../utils/objectIcons'

const props = defineProps<{
  /** 图标 key（utils/icons.ts 的 APP_ICONS） */
  icon: string
  title: string
  hint?: string
  size?: 'normal' | 'compact'
}>()

const iconSize = computed(() => (props.size === 'compact' ? 26 : 34))
</script>

<style scoped>
.empty-state {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  opacity: 0.85;
  padding: 20px;
  text-align: center;
}
.es-title {
  font-size: 13px;
  color: var(--el-text-color-secondary);
}
.es-hint {
  font-size: 12px;
  color: var(--el-text-color-placeholder);
  max-width: 320px;
}
.es-action {
  margin-top: 4px;
}
</style>

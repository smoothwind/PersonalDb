<template>
  <div class="json-tree">
    <template v-if="isContainer">
      <div
        class="jt-row"
        :style="{ paddingLeft: depth * 14 + 'px' }"
        @click="expanded = !expanded"
      >
        <el-icon class="jt-arrow" :class="{ 'is-open': expanded }">
          <CaretRight />
        </el-icon>
        <span v-if="name !== undefined" class="jt-key">{{ name }}</span>
        <span class="jt-meta">{{ containerMeta }}</span>
      </div>
      <template v-if="expanded">
        <JsonTree
          v-for="(v, k) in data as Record<string, unknown>"
          :key="String(k)"
          :data="v"
          :name="Array.isArray(data) ? undefined : String(k)"
          :depth="depth + 1"
        />
      </template>
    </template>
    <div v-else class="jt-row" :style="{ paddingLeft: depth * 14 + 'px' }">
      <span class="jt-arrow jt-leaf" />
      <span v-if="name !== undefined" class="jt-key">{{ name }}</span>
      <span class="jt-value" :class="valueClass">{{ display }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { CaretRight } from '@element-plus/icons-vue'

const props = withDefaults(
  defineProps<{
    data: unknown
    name?: string
    depth?: number
    defaultExpandDepth?: number
  }>(),
  { name: undefined, depth: 0, defaultExpandDepth: 1 },
)

const expanded = ref(props.depth < props.defaultExpandDepth)

const isContainer = computed(
  () =>
    props.data !== null &&
    typeof props.data === 'object',
)

const containerMeta = computed(() => {
  if (Array.isArray(props.data)) return `[${props.data.length}]`
  return `{${Object.keys(props.data as Record<string, unknown>).length}}`
})

const display = computed(() => {
  const v = props.data
  if (v === null) return 'null'
  if (typeof v === 'string') return `"${v}"`
  return String(v)
})

const valueClass = computed(() => {
  const v = props.data
  if (v === null) return 'vt-null'
  switch (typeof v) {
    case 'string':
      return 'vt-string'
    case 'number':
      return 'vt-number'
    case 'boolean':
      return 'vt-bool'
    default:
      return ''
  }
})
</script>

<style scoped>
.jt-row {
  display: flex;
  align-items: center;
  gap: 4px;
  line-height: 20px;
  font-family: Consolas, 'Cascadia Mono', monospace;
  font-size: 12px;
  cursor: default;
  border-radius: 3px;
  padding-right: 6px;
}

.jt-row:hover {
  background: var(--el-fill-color-light);
}

.jt-arrow {
  font-size: 11px;
  color: var(--el-text-color-secondary);
  transition: transform 0.15s;
  cursor: pointer;
}

.jt-arrow.is-open {
  transform: rotate(90deg);
}

.jt-leaf {
  visibility: hidden;
}

.jt-key {
  color: var(--el-color-primary);
}

.jt-meta {
  color: var(--el-text-color-secondary);
  font-size: 11px;
}

.jt-value {
  white-space: pre-wrap;
  word-break: break-all;
}

.vt-string {
  color: var(--el-color-success);
}

.vt-number {
  color: var(--el-color-warning);
}

.vt-bool {
  color: #b37ceb;
}

.vt-null {
  color: var(--el-text-color-placeholder);
  font-style: italic;
}
</style>

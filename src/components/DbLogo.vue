<template>
  <span class="db-logo" :style="logoStyle" :title="title">{{ label }}</span>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { DbKind } from '../types/db'

const props = withDefaults(
  defineProps<{ kind: DbKind; size?: number; title?: string }>(),
  { size: 16, title: '' },
)

/** 品牌色 + 缩写标识（离线内置，无外部资源依赖） */
const BRANDS: Record<DbKind, { bg: string; label: string }> = {
  mysql: { bg: '#00758F', label: 'My' },
  oceanbase: { bg: '#1B7EDE', label: 'OB' },
  'oceanbase-oracle': { bg: '#0F5FAF', label: 'OBO' },
  postgresql: { bg: '#336791', label: 'Pg' },
  gaussdb: { bg: '#C7000B', label: 'Gs' },
  oracle: { bg: '#E01B22', label: 'Or' },
  sqlite: { bg: '#0F80CC', label: 'Sq' },
  jdbc: { bg: '#5B6B7A', label: 'JD' },
}

const brand = computed(() => BRANDS[props.kind])
const label = computed(() => brand.value.label)
const logoStyle = computed(() => ({
  width: `${props.size}px`,
  height: `${props.size}px`,
  background: brand.value.bg,
  fontSize: `${Math.round(props.size * 0.42)}px`,
}))
</script>

<style scoped>
.db-logo {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  border-radius: 4px;
  color: #fff;
  font-weight: 700;
  font-family: 'Segoe UI', sans-serif;
  letter-spacing: 0;
  line-height: 1;
  user-select: none;
}
</style>

<template>
  <svg
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.8"
    stroke-linecap="round"
    stroke-linejoin="round"
    :width="size"
    :height="size"
    aria-hidden="true"
    class="app-icon"
    v-html="markup"
  />
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { APP_ICONS } from '../utils/icons'

const props = defineProps<{
  /** 图标 key，见 utils/icons.ts 的 APP_ICONS */
  name: string
  size?: number | string
}>()

const markup = computed(() => APP_ICONS[props.name] ?? APP_ICONS.obj_query)
</script>

<style scoped>
.app-icon {
  flex-shrink: 0;
  vertical-align: -0.15em;
}
</style>

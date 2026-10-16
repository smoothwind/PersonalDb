<template>
  <div class="plan-tree">
    <PlanBranch :node="root" />
    <div class="plan-tip">{{ t('query.planTip') }}</div>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import PlanBranch from './PlanBranch.vue'
import type { PlanNode } from '../../types/db'

defineProps<{ root: PlanNode }>()

const { t } = useI18n()
</script>

<style scoped>
.plan-tree {
  padding: 10px 6px;
  overflow: auto;
  height: 100%;
}

.plan-tip {
  margin-top: 12px;
  padding-left: 6px;
  font-size: 11px;
  color: var(--el-text-color-placeholder);
}
</style>

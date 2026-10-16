<template>
  <div class="pb-node">
    <div
      class="pb-card"
      :class="{ 'is-hot': highlight }"
      @click="expanded = !expanded"
    >
      <span
        class="pb-arrow"
        :class="{ 'is-open': expanded, 'is-leaf': node.children.length === 0 }"
      >▸</span>
      <span class="pb-title">{{ node.title }}</span>
      <span v-if="node.cost != null" class="pb-cost">
        cost {{ Number(node.cost).toFixed(2) }}
      </span>
      <span v-if="node.rows != null" class="pb-rows">{{ t('common.rows', { n: node.rows }) }}</span>
    </div>
    <div v-if="node.attrs.length > 0" class="pb-attrs">
      <span v-for="pair in node.attrs" :key="pair[0]" class="pb-attr">
        {{ pair[0] }}: {{ pair[1] }}
      </span>
    </div>
    <div v-if="expanded && node.children.length > 0" class="pb-children">
      <PlanBranch
        v-for="(c, i) in node.children"
        :key="i"
        :node="c"
        :highlight="isHotChild(c)"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import type { PlanNode } from '../../types/db'

const { t } = useI18n()

const props = withDefaults(
  defineProps<{
    node: PlanNode
    /** 同级最贵分支高亮（优化重点提示） */
    highlight?: boolean
  }>(),
  { highlight: false },
)

const expanded = ref(true)

function isHotChild(c: PlanNode): boolean {
  if (props.node.children.length <= 1) return false
  let best: PlanNode | null = null
  for (const x of props.node.children) {
    if (x.cost != null && (best == null || (best.cost ?? 0) < x.cost)) best = x
  }
  return c === best
}
</script>

<style scoped>
.pb-node {
  min-width: 0;
}

.pb-children {
  margin-left: 22px;
  padding-left: 12px;
  border-left: 2px solid var(--el-border-color-lighter);
}

.pb-card {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 4px 10px;
  margin: 3px 0;
  background: var(--el-fill-color-light);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  cursor: default;
  user-select: none;
}

.pb-card.is-hot {
  border-color: var(--el-color-danger);
  background: var(--el-color-danger-light-9);
}

.pb-arrow {
  font-size: 11px;
  color: var(--el-text-color-secondary);
  transition: transform 0.15s;
  width: 12px;
  display: inline-block;
}

.pb-arrow.is-open {
  transform: rotate(90deg);
}

.pb-arrow.is-leaf {
  visibility: hidden;
}

.pb-title {
  font-weight: 600;
  font-size: 13px;
}

.pb-cost {
  font-size: 11px;
  color: var(--el-color-warning);
  font-variant-numeric: tabular-nums;
}

.pb-rows {
  font-size: 11px;
  color: var(--el-text-color-secondary);
  font-variant-numeric: tabular-nums;
}

.pb-attrs {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 12px;
  padding: 0 6px 2px 24px;
}

.pb-attr {
  font-size: 11px;
  color: var(--el-text-color-secondary);
  font-family: Consolas, monospace;
}
</style>

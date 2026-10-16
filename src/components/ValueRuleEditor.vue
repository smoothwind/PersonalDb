<template>
  <div class="vre">
    <div v-for="(r, i) in rules" :key="i" class="vre-row">
      <el-select
        :model-value="r.semantic"
        size="small"
        class="vre-sem"
        @update:model-value="(val: string) => updateRule(i, val, r.template)"
      >
        <el-option v-for="s in semanticOptions" :key="s" :value="s" :label="s" />
      </el-select>
      <el-input
        :model-value="r.template"
        size="small"
        placeholder="TO_CLOB({v})"
        @update:model-value="(val: string) => updateRule(i, r.semantic, val)"
      />
      <el-button link type="danger" size="small" :icon="Delete" @click="remove(i)" />
    </div>
    <div v-if="rules.length === 0" class="vre-empty">{{ t('settings.ruleEmpty') }}</div>
    <div class="vre-actions">
      <el-button size="small" :icon="Plus" @click="add">{{ t('settings.ruleAdd') }}</el-button>
      <el-button size="small" @click="preset('longtext', 'TO_CLOB({v})')">TO_CLOB</el-button>
      <el-button size="small" @click="preset('text', `NVL({v}, '')`)">NVL</el-button>
      <el-button size="small" @click="preset('text', `COALESCE({v}, '')`)">COALESCE</el-button>
    </div>
    <div class="vre-hint">{{ t('settings.ruleHint') }}</div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Delete, Plus } from '@element-plus/icons-vue'
import type { ValueRule } from '../stores/preferences'

const props = defineProps<{ modelValue: ValueRule[] }>()
const emit = defineEmits<{ (e: 'update:modelValue', v: ValueRule[]): void }>()

const { t } = useI18n()

const semanticOptions = [
  'text',
  'longtext',
  'json',
  'binary',
  'number',
  'decimal',
  'date',
  'datetime',
  'time',
  'bool',
]

/** 本地可变副本（通过 emit 回写，避免直接改父级对象） */
const rules = computed<ValueRule[]>(() => props.modelValue)

function commit(next: ValueRule[]): void {
  emit('update:modelValue', next)
}

function updateRule(i: number, semantic: string, template: string): void {
  const next = [...rules.value]
  next[i] = { semantic, template }
  commit(next)
}

function add(): void {
  commit([...rules.value, { semantic: 'longtext', template: '' }])
}

function remove(i: number): void {
  const next = [...rules.value]
  next.splice(i, 1)
  commit(next)
}

function preset(semantic: string, template: string): void {
  // 同语义已有则覆盖，否则新增
  const idx = rules.value.findIndex((r) => r.semantic === semantic)
  const next = [...rules.value]
  if (idx >= 0) {
    next[idx] = { semantic, template }
  } else {
    next.push({ semantic, template })
  }
  commit(next)
}
</script>

<style scoped>
.vre {
  width: 100%;
}
.vre-row {
  display: flex;
  gap: 6px;
  align-items: center;
  margin-bottom: 6px;
}
.vre-sem {
  width: 110px;
  flex-shrink: 0;
}
.vre-empty {
  font-size: 12px;
  color: var(--el-text-color-placeholder);
  margin-bottom: 6px;
}
.vre-actions {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}
.vre-hint {
  margin-top: 6px;
  font-size: 12px;
  color: var(--el-text-color-placeholder);
  line-height: 1.4;
}
</style>

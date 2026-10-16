<template>
  <div class="cvf" @click.stop>
    <div class="cvf-head">
      <span class="cs-label">{{ t('result.colValues') }}</span>
      <el-button
        v-if="staged.length > 0"
        size="small"
        text
        type="danger"
        @click="staged = []"
      >
        {{ t('result.valueFilterClear') }}
      </el-button>
    </div>
    <div class="cvf-list">
      <label v-for="v in values" :key="v" class="cvf-item">
        <input
          type="checkbox"
          :checked="staged.includes(v)"
          @change="toggle(v, ($event.target as HTMLInputElement).checked)"
        />
        <span :title="v">{{ v }}</span>
      </label>
    </div>
    <div class="cvf-actions">
      <el-button size="small" @click="cancel">{{ t('common.cancel') }}</el-button>
      <el-button size="small" type="primary" @click="confirm">{{ t('common.confirm') }}</el-button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

const props = defineProps<{
  values: string[]
  modelValue: string[]
}>()

const emit = defineEmits<{ (e: 'update:modelValue', values: string[]): void }>()

const { t } = useI18n()
const staged = ref<string[]>([...props.modelValue])

// 列值集合变化（重新执行）时同步初始勾选
watch(
  () => props.values,
  () => {
    staged.value = [...props.modelValue]
  },
)

function toggle(v: string, checked: boolean): void {
  staged.value = checked ? [...staged.value, v] : staged.value.filter((x) => x !== v)
}

function cancel(): void {
  staged.value = [...props.modelValue]
}

function confirm(): void {
  emit('update:modelValue', [...staged.value])
}
</script>

<style scoped>
.cvf {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.cvf-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.cvf-list {
  max-height: 150px;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 4px 2px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
}
.cvf-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 2px 6px;
  font-size: 12px;
  cursor: pointer;
  border-radius: 4px;
}
.cvf-item:hover {
  background: var(--el-fill-color-light);
}
.cvf-item span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.cvf-item input {
  accent-color: var(--el-color-primary);
  flex-shrink: 0;
}
.cvf-actions {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
}
</style>

<template>
  <el-popover v-model:visible="visible" placement="bottom-end" :width="260" trigger="click" @show="onShow">
    <template #reference>
      <el-button size="small" circle :icon="Grid" :title="t('result.showCols')" />
    </template>
    <div class="cvp">
      <el-input v-model="kw" size="small" clearable :prefix-icon="Search" :placeholder="t('result.searchCols')" />
      <el-checkbox
        class="cvp-all"
        :model-value="allVisible"
        :indeterminate="someHidden"
        @update:model-value="setAll"
      >
        {{ t('result.selectAll') }}
      </el-checkbox>
      <div ref="listEl" class="cvp-list">
        <label v-for="prop in shownOrder" :key="prop" class="cvp-item" :data-prop="prop">
          <input
            type="checkbox"
            :checked="!staged[prop]"
            @change="toggle(prop, !($event.target as HTMLInputElement).checked)"
          />
          <span :title="colName(prop)">{{ colName(prop) }}</span>
        </label>
        <div v-if="shownOrder.length === 0" class="cvp-empty">{{ t('palette.empty') }}</div>
      </div>
      <div class="cvp-tip">{{ t('result.dragColsTip') }}</div>
      <div class="cvp-actions">
        <el-button size="small" text type="danger" class="cvp-reset" @click="emit('reset')">
          {{ t('result.resetCols') }}
        </el-button>
        <span class="cvp-spacer" />
        <el-button size="small" @click="visible = false">{{ t('common.cancel') }}</el-button>
        <el-button size="small" type="primary" @click="confirm">{{ t('common.confirm') }}</el-button>
      </div>
    </div>
  </el-popover>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Grid, Search } from '@element-plus/icons-vue'
import Sortable from 'sortablejs'

const props = defineProps<{
  /** 全部列（父级已按当前列序排列） */
  cols: { prop: string; name: string }[]
  /** 当前隐藏映射（确认前不回写父级，勾选过程零父渲染） */
  hidden: Record<string, boolean>
}>()

const emit = defineEmits<{
  (e: 'apply', hidden: Record<string, boolean>): void
  (e: 'reorder', order: string[]): void
  (e: 'reset'): void
}>()

const { t } = useI18n()
const visible = ref(false)
const staged = ref<Record<string, boolean>>({})
const kw = ref('')
/** 弹层内的列序（拖拽直接改这里，确认时随 apply 一起提交） */
const localOrder = ref<string[]>([])
const listEl = ref<HTMLElement>()
let sortable: Sortable | null = null

const colMap = computed(() => new Map(props.cols.map((c) => [c.prop, c.name])))

function colName(prop: string): string {
  return colMap.value.get(prop) ?? prop
}

const shown = computed(() => {
  const k = kw.value.trim().toLowerCase()
  if (!k) return props.cols
  return props.cols.filter((c) => c.name.toLowerCase().includes(k))
})

/** 展示顺序 = 弹层内拖拽顺序（搜索时为过滤子集，此时拖拽禁用） */
const shownOrder = computed(() => {
  const set = new Set(shown.value.map((c) => c.prop))
  return localOrder.value.filter((p) => set.has(p))
})

const visibleCount = computed(
  () => props.cols.filter((c) => !staged.value[c.prop]).length,
)
const allVisible = computed(() => visibleCount.value === props.cols.length)
const someHidden = computed(() => visibleCount.value > 0 && visibleCount.value < props.cols.length)

function onShow(): void {
  staged.value = { ...props.hidden }
  localOrder.value = props.cols.map((c) => c.prop)
  kw.value = ''
  void nextTick(initSortable)
}

function initSortable(): void {
  sortable?.destroy()
  sortable = null
  const el = listEl.value
  if (!el) return
  sortable = Sortable.create(el, {
    animation: 150,
    // 搜索过滤时禁用拖拽（局部列表无法还原完整顺序）
    disabled: kw.value.trim() !== '',
    onEnd: ({ oldIndex, newIndex }) => {
      if (oldIndex == null || newIndex == null || oldIndex === newIndex) return
      const order = [...localOrder.value]
      const [moved] = order.splice(oldIndex, 1)
      order.splice(newIndex, 0, moved)
      localOrder.value = order
    },
  })
}

watch(kw, () => {
  sortable?.option('disabled', kw.value.trim() !== '')
})

function toggle(prop: string, hidden: boolean): void {
  if (hidden) staged.value = { ...staged.value, [prop]: true }
  else {
    const next = { ...staged.value }
    delete next[prop]
    staged.value = next
  }
}

function setAll(v: boolean | string | number | undefined): void {
  staged.value = v ? {} : Object.fromEntries(props.cols.map((c) => [c.prop, true]))
}

function confirm(): void {
  // 勾选与顺序一并提交（顺序为全量列序，含隐藏列）
  emit('reorder', [...localOrder.value])
  emit('apply', { ...staged.value })
  visible.value = false
}
</script>

<style scoped>
.cvp {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.cvp-all {
  padding: 0 2px;
}
.cvp-list {
  max-height: 240px;
  overflow: auto;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
  padding: 4px 2px;
}
.cvp-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 2px 6px;
  font-size: 12px;
  cursor: pointer;
  border-radius: 4px;
}
.cvp-item:hover {
  background: var(--el-fill-color-light);
}
.cvp-item span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.cvp-item input {
  accent-color: var(--el-color-primary);
  flex-shrink: 0;
}
.cvp-empty {
  text-align: center;
  font-size: 12px;
  opacity: 0.6;
  padding: 10px 0;
}
.cvp-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}
.cvp-spacer {
  flex: 1;
}
.cvp-tip {
  font-size: 10.5px;
  opacity: 0.5;
  text-align: center;
}
</style>

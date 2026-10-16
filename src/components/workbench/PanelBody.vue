<template>
  <ConnectionTree
    v-if="id === 'conn'"
    ref="inst"
    @new-connection="emit('newConnection')"
    @edit="(cfg) => emit('edit', cfg)"
  />
  <TabArea v-else-if="id === 'tabs'" />
  <LogPanel v-else :embedded="embedded" />
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import ConnectionTree from '../ConnectionTree.vue'
import LogPanel from '../LogPanel.vue'
import TabArea from './TabArea.vue'
import { useLayoutStore, type PanelId } from '../../stores/layout'
import type { ConnectionConfig } from '../../types/db'

const props = defineProps<{
  id: PanelId
  /** 悬浮窗内嵌时隐藏自带头部（由 FloatWindow 提供标题栏） */
  embedded?: boolean
}>()

const emit = defineEmits<{
  (e: 'newConnection'): void
  (e: 'edit', cfg: ConnectionConfig): void
}>()

const layout = useLayoutStore()
const inst = ref<InstanceType<typeof ConnectionTree> | null>(null)

// 保存连接后刷新所有挂载中的连接树
watch(
  () => layout.refreshSeq,
  () => {
    if (props.id === 'conn') void inst.value?.refresh()
  },
)

defineExpose({
  /** 仅连接树支持（保存连接后刷新树） */
  refresh: () => {
    void inst.value?.refresh()
  },
})
</script>

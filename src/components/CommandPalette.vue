<template>
  <Teleport to="body">
    <Transition name="cp-fade">
      <div v-if="visible" class="cp-mask" @mousedown.self="close">
        <div class="cp-panel">
          <input
            ref="inputRef"
            v-model="kw"
            class="cp-input"
            :placeholder="t('palette.placeholder')"
            @keydown.down.prevent="move(1)"
            @keydown.up.prevent="move(-1)"
            @keydown.enter.prevent="run(active)"
            @keydown.esc.prevent="close"
          />
          <div class="cp-list">
            <div
              v-for="(item, i) in items"
              :key="item.id"
              class="cp-item"
              :class="{ active: i === active }"
              @mouseenter="active = i"
              @click="run(i)"
            >
              <el-icon><component :is="objectIcon(item.icon)" /></el-icon>
              <span class="cp-label">{{ item.label }}</span>
              <span class="cp-group">{{ item.group }}</span>
            </div>
            <div v-if="items.length === 0" class="cp-empty">{{ t('palette.empty') }}</div>
          </div>
          <div class="cp-foot">
            <span>↑↓ {{ t('palette.nav') }}</span>
            <span>Enter {{ t('palette.run') }}</span>
            <span>Esc {{ t('palette.close') }}</span>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { objectIcon } from '../utils/objectIcons'

interface CmdItem {
  id: string
  label: string
  group: string
  icon: string
  action: () => void
}

const props = defineProps<{
  visible: boolean
  /** 动态命令（命令/连接/历史等由宿主注入） */
  commands: CmdItem[]
}>()

const emit = defineEmits<{ (e: 'update:visible', v: boolean): void }>()

const { t } = useI18n()
const kw = ref('')
const active = ref(0)
const inputRef = ref<HTMLInputElement>()

const items = computed(() => {
  const k = kw.value.trim().toLowerCase()
  if (!k) return props.commands
  return props.commands.filter((c) => `${c.label} ${c.group}`.toLowerCase().includes(k))
})

watch(
  () => props.visible,
  async (v) => {
    if (v) {
      kw.value = ''
      active.value = 0
      await nextTick()
      inputRef.value?.focus()
    }
  },
)

watch(items, () => {
  active.value = 0
})

function move(delta: number): void {
  const n = items.value.length
  if (n === 0) return
  active.value = (active.value + delta + n) % n
}

function run(i: number): void {
  const item = items.value[i]
  if (!item) return
  close()
  item.action()
}

function close(): void {
  emit('update:visible', false)
}
</script>

<style scoped>
.cp-mask {
  position: fixed;
  inset: 0;
  z-index: 3000;
  background: rgba(0, 0, 0, 0.35);
  display: flex;
  align-items: flex-start;
  justify-content: center;
  padding-top: 12vh;
}
.cp-panel {
  width: 520px;
  max-width: 90vw;
  background: var(--el-bg-color);
  border: 1px solid var(--el-border-color);
  border-radius: 10px;
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.25);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.cp-input {
  border: none;
  outline: none;
  background: transparent;
  color: var(--el-text-color-primary);
  font-size: 15px;
  padding: 14px 16px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}
.cp-list {
  max-height: 320px;
  overflow: auto;
  padding: 6px;
}
.cp-item {
.cp-item :deep(.el-icon) {
  font-size: var(--pd-icon-sm);
}

  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 13px;
  color: var(--el-text-color-primary);
}
.cp-item.active {
  background: var(--el-color-primary-light-9);
}
.cp-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.cp-group {
  font-size: 11px;
  opacity: 0.55;
}
.cp-empty {
  text-align: center;
  padding: 28px 0;
  font-size: 12px;
  opacity: 0.6;
}
.cp-foot {
  display: flex;
  gap: 16px;
  padding: 8px 14px;
  font-size: 11px;
  opacity: 0.55;
  border-top: 1px solid var(--el-border-color-lighter);
}
.cp-fade-enter-active,
.cp-fade-leave-active {
  transition: opacity 0.12s ease;
}
.cp-fade-enter-from,
.cp-fade-leave-to {
  opacity: 0;
}
</style>

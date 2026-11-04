<template>
  <Teleport to="body">
    <div v-if="visible" ref="elRef" class="pd-ctx" :style="style" @click.stop @contextmenu.prevent>
      <template v-for="(it, i) in items" :key="it.sep ? `sep-${i}` : it.key">
        <div v-if="it.sep" class="pd-ctx-sep" />
        <div
          v-else
          class="pd-ctx-item"
          :class="{
            'is-danger': it.danger,
            'is-disabled': it.disabled,
            'is-open': it.children?.length && openKey === it.key,
          }"
          :title="it.title"
          @click="onClick(it)"
          @mouseenter="onEnter(it)"
        >
          <el-icon v-if="it.icon" class="pd-ctx-icon"><component :is="resolveIcon(it.icon)" /></el-icon>
          <span class="pd-ctx-label">{{ it.label }}</span>
          <span v-if="it.hint" class="pd-ctx-hint">{{ it.hint }}</span>
          <span v-if="it.children?.length" class="pd-ctx-arrow">▸</span>
          <!-- 一级级联子菜单（不递归多级，SPEC-UI-31）：absolute 悬挂于父项右缘 -->
          <div
            v-if="it.children?.length && openKey === it.key"
            class="pd-ctx-sub"
            :class="{ 'is-flip': subFlip }"
          >
            <template v-for="(c, ci) in it.children" :key="c.sep ? `sub-${ci}` : c.key">
              <div v-if="c.sep" class="pd-ctx-sep" />
              <div
                v-else
                class="pd-ctx-item"
                :class="{ 'is-danger': c.danger, 'is-disabled': c.disabled }"
                :title="c.title"
                @click.stop="onSubClick(c)"
              >
                <el-icon v-if="c.icon" class="pd-ctx-icon"><component :is="resolveIcon(c.icon)" /></el-icon>
                <span class="pd-ctx-label">{{ c.label }}</span>
                <span v-if="c.hint" class="pd-ctx-hint">{{ c.hint }}</span>
              </div>
            </template>
          </div>
        </div>
      </template>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch, type Component } from 'vue'
import { objectIcon } from '../utils/objectIcons'

/** 右键菜单项（唯一菜单实现，SPEC-UI-31） */
export interface CtxMenuItem {
  key: string
  label: string
  /** APP_ICONS key（如 'act_copy'）或 EP 图标组件 */
  icon?: string | Component
  danger?: boolean
  disabled?: boolean
  /** 分组分隔线（此项其它字段忽略） */
  sep?: boolean
  /** 右侧提示（快捷键等） */
  hint?: string
  title?: string
  /** 一级级联子菜单（子项不支持再嵌套 children；父项点击/hover 展开，子项点击 emit select(key) 后整菜单收起） */
  children?: CtxMenuItem[]
}

const props = defineProps<{
  visible: boolean
  x: number
  y: number
  items: CtxMenuItem[]
}>()

const emit = defineEmits<{
  (e: 'select', key: string): void
  (e: 'update:visible', v: boolean): void
}>()

const elRef = ref<HTMLElement | null>(null)
const pos = ref({ x: 0, y: 0 })
const style = computed(() => ({ left: `${pos.value.x}px`, top: `${pos.value.y}px` }))

/** 当前展开子菜单的父项 key（null = 无展开） */
const openKey = ref<string | null>(null)
/** 子菜单是否向左弹出（视口右缘空间不足时翻转） */
const subFlip = ref(false)

/** 子菜单宽度估算值（翻转判定用，非精确渲染宽度） */
const SUB_MENU_W = 200

function resolveIcon(icon: string | Component): Component {
  return typeof icon === 'string' ? objectIcon(icon) : icon
}

/** 定位 + 视口内收边（贴右下边界时自动回退，避免菜单溢出窗口） */
async function place(): Promise<void> {
  pos.value = { x: Math.max(4, props.x), y: Math.max(4, props.y) }
  await nextTick()
  const el = elRef.value
  if (!el) return
  const r = el.getBoundingClientRect()
  let { x, y } = pos.value
  if (x + r.width > window.innerWidth - 4) x = Math.max(4, window.innerWidth - r.width - 4)
  if (y + r.height > window.innerHeight - 4) y = Math.max(4, window.innerHeight - r.height - 4)
  pos.value = { x, y }
}

function close(): void {
  openKey.value = null
  emit('update:visible', false)
}

/** 展开子菜单并按视口右缘空间决定是否向左弹出 */
async function openSub(key: string): Promise<void> {
  openKey.value = key
  subFlip.value = false
  await nextTick()
  const parent = elRef.value?.querySelector('.pd-ctx-item.is-open')
  if (!parent) return
  const r = parent.getBoundingClientRect()
  subFlip.value = r.right + SUB_MENU_W > window.innerWidth - 8
}

function onClick(it: CtxMenuItem): void {
  if (it.sep || it.disabled) return
  if (it.children?.length) {
    // 点击展开（幂等不收起）；不 emit select、不关菜单
    if (openKey.value !== it.key) void openSub(it.key)
    return
  }
  emit('select', it.key)
  close()
}

function onSubClick(c: CtxMenuItem): void {
  if (c.sep || c.disabled) return
  emit('select', c.key)
  close()
}

/** 悬停：父项展开子菜单，其余项收起已展开的子菜单 */
function onEnter(it: CtxMenuItem): void {
  if (it.disabled) return
  if (it.children?.length) {
    if (openKey.value !== it.key) void openSub(it.key)
    return
  }
  if (openKey.value) openKey.value = null
}

function onKeydown(e: KeyboardEvent): void {
  if (e.key !== 'Escape') return
  // 两段关闭：先子菜单后主菜单
  if (openKey.value) {
    openKey.value = null
    return
  }
  close()
}

function onDocClick(e: MouseEvent): void {
  // 菜单内点击不在此收起——由 onClick/onSubClick 决定（子菜单展开需要点击存活）
  if (elRef.value && e.composedPath().includes(elRef.value)) return
  close()
}

/** 全局收起：失焦 / 缩放 / 他处点击（含右键）/ 滚动 / Esc —— 一处实现，各调用点无需重复注册 */
let attached = false
function attach(): void {
  if (attached) return
  attached = true
  window.addEventListener('blur', close)
  window.addEventListener('resize', close)
  document.addEventListener('click', onDocClick, true)
  document.addEventListener('contextmenu', close, true)
  document.addEventListener('scroll', close, true)
  document.addEventListener('keydown', onKeydown, true)
}

function detach(): void {
  if (!attached) return
  attached = false
  window.removeEventListener('blur', close)
  window.removeEventListener('resize', close)
  document.removeEventListener('click', onDocClick, true)
  document.removeEventListener('contextmenu', close, true)
  document.removeEventListener('scroll', close, true)
  document.removeEventListener('keydown', onKeydown, true)
}

watch(
  () => props.visible,
  async (v) => {
    if (v) {
      await nextTick()
      attach()
      void place()
    } else {
      openKey.value = null
      detach()
    }
  },
  // 直接以 visible=true 挂载（测试/未来调用点）也要 attach
  { immediate: true },
)

onBeforeUnmount(detach)
</script>

<style scoped>
.pd-ctx {
  position: fixed;
  z-index: 3000;
  min-width: 168px;
  max-width: 320px;
  padding: 4px;
  background: var(--el-bg-color-overlay);
  border: 1px solid var(--el-border-color-light);
  border-radius: 6px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
}

.pd-ctx-item {
  position: relative;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 10px;
  font-size: 13px;
  border-radius: 4px;
  cursor: pointer;
  user-select: none;
  color: var(--el-text-color-primary);
  white-space: nowrap;
}

.pd-ctx-item:hover {
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
}

/* 子菜单展开时父项保持高亮（悬停态之外的持久态） */
.pd-ctx-item.is-open {
  background: var(--el-color-primary-light-9);
  color: var(--el-color-primary);
}

.pd-ctx-item.is-danger {
  color: var(--el-color-danger);
}

.pd-ctx-item.is-danger:hover {
  background: var(--el-color-danger-light-9);
}

.pd-ctx-item.is-disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.pd-ctx-item.is-disabled:hover {
  background: transparent;
  color: var(--el-text-color-primary);
}

.pd-ctx-icon {
  font-size: var(--pd-icon-sm);
  flex-shrink: 0;
}

.pd-ctx-label {
  flex: 1;
}

.pd-ctx-hint {
  margin-left: 16px;
  font-size: 10.5px;
  color: var(--el-text-color-placeholder);
  font-family: Consolas, monospace;
  letter-spacing: 0.5px;
}

.pd-ctx-arrow {
  flex-shrink: 0;
  margin-left: 4px;
  font-size: 10px;
  color: var(--el-text-color-secondary);
}

/* 一级级联子菜单：悬挂于父项右缘，视觉与主菜单一致 */
.pd-ctx-sub {
  position: absolute;
  left: 100%;
  top: -4px;
  min-width: 180px;
  max-width: 320px;
  padding: 4px;
  background: var(--el-bg-color-overlay);
  border: 1px solid var(--el-border-color-light);
  border-radius: 6px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
}

/* 视口右缘空间不足 → 向左弹出 */
.pd-ctx-sub.is-flip {
  left: auto;
  right: 100%;
}

.pd-ctx-sep {
  height: 1px;
  margin: 4px 0;
  background: var(--el-border-color-lighter);
}
</style>

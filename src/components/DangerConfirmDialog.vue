<template>
  <el-dialog
    :model-value="!!danger.pending"
    width="480px"
    top="12vh"
    :close-on-click-modal="false"
    @update:model-value="(v: boolean) => { if (!v) cancel() }"
  >
    <template #header>
      <span class="dlg-title danger-title">
        <el-icon><WarningFilled /></el-icon>
        {{ t('danger.title') }}
      </span>
    </template>
    <template v-if="ctx">
      <div class="danger-line">
        <el-tag :type="ctx.level === 'danger' ? 'danger' : 'warning'" size="small" effect="dark">
          {{ ctx.level === 'danger' ? t('danger.riskDanger') : t('danger.riskWarn') }}
        </el-tag>
        <span class="danger-scope">
          {{ ctx.connectionName }}<template v-if="ctx.database"> · {{ ctx.database }}</template>
        </span>
      </div>
      <div class="danger-txn" :class="{ 'is-manual': ctx.txnManual }">
        {{ ctx.txnManual ? t('danger.txnManual') : t('danger.irreversible') }}
      </div>
      <div class="danger-items">
        <div v-for="(it, i) in ctx.items" :key="i" class="danger-item">
          <div class="danger-item-head">
            <span class="danger-type">{{ it.type === 'destructiveDdl' ? t('danger.typeDdl') : t('danger.typeDml') }}</span>
            <span v-if="it.objects.length" class="danger-objects">{{ it.objects.join(', ') }}</span>
          </div>
          <div class="danger-stmt">
            {{ it.statement.slice(0, 200) }}<span v-if="it.statement.length > 200">…</span>
          </div>
        </div>
      </div>
      <div v-if="ctx.needTypeConfirm && ctx.confirmTarget" class="danger-typein">
        <div class="danger-typein-hint">{{ t('danger.typeConfirmHint', { target: ctx.confirmTarget }) }}</div>
        <el-input v-model="typed" autocomplete="off" spellcheck="false" @keyup.enter="confirm" />
      </div>
      <el-checkbox v-model="exemptChecked" class="danger-exempt">{{ exemptLabel }}</el-checkbox>
    </template>
    <template #footer>
      <el-button ref="cancelBtnRef" @click="cancel">{{ t('danger.cancel') }}</el-button>
      <el-button type="danger" :disabled="typeBlocked" @click="confirm">{{ confirmLabel }}</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { WarningFilled } from '@element-plus/icons-vue'
import { useDangerStore } from '../stores/danger'
import { usePreferencesStore } from '../stores/preferences'

/**
 * 危险操作确认弹窗（全局唯一，App.vue 挂载；SPEC-SAFE-1）。
 *
 * 交互约定（NN/g + GitHub type-to-confirm 惯例）：
 * - 取消按钮默认焦点（不给危险按钮默认答案）
 * - danger 且未降级 → 输入首个对象名激活确认按钮（大小写不敏感，对齐结果集编辑预览惯例）
 * - 手动事务模式显眼提示「可回滚」（降级由 runTab 决定，本组件只负责展示）
 * - 勾选「不再提示」按偏好粒度记录豁免
 */
const { t } = useI18n()
const danger = useDangerStore()
const prefs = usePreferencesStore()

const typed = ref('')
const exemptChecked = ref(false)
const cancelBtnRef = ref<{ focus?: () => void } | null>(null)

const ctx = computed(() => danger.pending)

watch(
  () => danger.pending,
  async (p) => {
    typed.value = ''
    exemptChecked.value = false
    if (p) {
      await nextTick()
      cancelBtnRef.value?.focus?.()
    }
  },
)

const typeBlocked = computed(
  () =>
    !!ctx.value?.needTypeConfirm &&
    !!ctx.value.confirmTarget &&
    typed.value.trim().toLowerCase() !== ctx.value.confirmTarget.toLowerCase(),
)

const exemptLabel = computed(() =>
  prefs.danger.exemptionScope === 'persistent' ? t('danger.exemptPersistent') : t('danger.exemptSession'),
)

const confirmLabel = computed(() =>
  ctx.value?.needTypeConfirm
    ? t('danger.confirmTyped')
    : t('danger.confirmRun', { n: ctx.value?.items.length ?? 1 }),
)

function cancel(): void {
  danger.settle(false)
}

function confirm(): void {
  if (typeBlocked.value) return
  const c = ctx.value
  if (c && exemptChecked.value) danger.addExemption(c.exemptKey, prefs.danger.exemptionScope)
  danger.settle(true)
}
</script>

<style scoped>
.danger-title :deep(.el-icon) {
  color: var(--el-color-danger);
}

.danger-line {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.danger-scope {
  font-weight: 600;
}

.danger-txn {
  margin-bottom: 10px;
  padding: 6px 10px;
  border-radius: 4px;
  font-size: 12.5px;
  background: var(--el-color-danger-light-9);
  color: var(--el-color-danger);
}

.danger-txn.is-manual {
  background: var(--el-color-warning-light-9);
  color: var(--el-color-warning-dark-2);
}

.danger-items {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 200px;
  overflow: auto;
  margin-bottom: 10px;
}

.danger-item {
  padding: 6px 8px;
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 4px;
}

.danger-item-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}

.danger-type {
  font-size: 12px;
  font-weight: 600;
  color: var(--el-text-color-secondary);
}

.danger-objects {
  font-size: 12px;
  font-weight: 600;
  color: var(--el-color-danger);
}

.danger-stmt {
  font-family: Consolas, 'Cascadia Mono', 'Courier New', monospace;
  font-size: 12px;
  color: var(--el-text-color-regular);
  word-break: break-all;
  white-space: pre-wrap;
}

.danger-typein {
  margin-bottom: 10px;
}

.danger-typein-hint {
  font-size: 12.5px;
  margin-bottom: 4px;
  color: var(--el-text-color-regular);
}

.danger-typein-hint strong,
.danger-typein-hint code {
  color: var(--el-color-danger);
  font-weight: 600;
}

.danger-exempt {
  display: flex;
}
</style>

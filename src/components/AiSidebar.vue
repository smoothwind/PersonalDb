<template>
  <aside class="ai-side">
    <!-- 头部：标题 + 当前提供方 + 收起 -->
    <div class="ai-head">
      <el-icon class="ai-logo"><component :is="objectIcon('act_ai')" /></el-icon>
      <span class="ai-title">{{ t('ai.sidebar') }}</span>
      <span v-if="activeName" class="ai-provider">{{ activeName }}</span>
      <span class="flex-sp" />
      <el-tooltip :content="t('ai.settings')" placement="bottom">
        <button class="ai-btn" type="button" @click="openSettings">
          <el-icon><component :is="objectIcon('act_settings')" /></el-icon>
        </button>
      </el-tooltip>
      <el-tooltip :content="t('ai.collapse')" placement="bottom">
        <button class="ai-btn" type="button" @click="emit('close')">
          <el-icon><component :is="objectIcon('act_hide')" /></el-icon>
        </button>
      </el-tooltip>
    </div>

    <!-- 模式切换 -->
    <div class="ai-modes">
      <button
        v-for="m in MODES"
        :key="m"
        type="button"
        class="ai-mode-chip"
        :class="{ 'is-active': mode === m }"
        @click="switchMode(m)"
      >
        {{ t(`ai.mode_${m}`) }}
      </button>
    </div>

    <!-- NL2SQL 上下文行：表结构开关 + 表多选 -->
    <div v-if="mode === 'nl2sql'" class="ai-ctx">
      <el-checkbox
        :model-value="aiStore.prefs.contextEnabled"
        size="small"
        @update:model-value="(v: boolean | string | number) => aiStore.updateFlags({ contextEnabled: !!v })"
      >
        {{ t('ai.contextEnabled') }}
      </el-checkbox>
      <el-select
        v-if="aiStore.prefs.contextEnabled"
        v-model="pickedTables"
        multiple
        collapse-tags
        clearable
        filterable
        size="small"
        :placeholder="t('ai.pickTables')"
        class="ai-tables"
      >
        <el-option v-for="tb in tableOptions" :key="tb.value" :value="tb.value" :label="tb.label" />
      </el-select>
    </div>

    <!-- 输入区 -->
    <el-input
      v-model="input"
      type="textarea"
      :rows="3"
      resize="none"
      :placeholder="inputPlaceholder"
      class="ai-input"
      @keydown.ctrl.enter.prevent="send"
      @keydown.meta.enter.prevent="send"
    />

    <!-- 动作行 -->
    <div class="ai-actions">
      <el-button
        type="primary"
        size="small"
        :disabled="!canSend"
        @click="send"
      >
        {{ t('ai.generate') }}
      </el-button>
      <el-button v-if="aiStore.running" size="small" type="warning" plain @click="aiStore.cancel()">
        {{ t('ai.stop') }}
      </el-button>
      <span v-if="!aiStore.active && !aiStore.running" class="ai-warn">{{ t('ai.noProvider') }}</span>
    </div>

    <!-- 输出区 -->
    <div ref="outRef" class="ai-out">
      <div v-if="!output && !aiStore.running" class="ai-empty">{{ t('ai.empty') }}</div>
      <pre v-else class="ai-out-text">{{ output }}<span v-if="aiStore.running" class="ai-cursor">▌</span></pre>
      <div v-if="errorMsg" class="ai-error">{{ errorMsg }}</div>
    </div>

    <!-- 产出动作行 -->
    <div v-if="generated" class="ai-foot">
      <el-button size="small" @click="copyOut">{{ t('common.copy') }}</el-button>
      <el-button size="small" @click="emit('insert', generated)">{{ t('ai.insert') }}</el-button>
      <el-button size="small" type="primary" plain @click="emit('replace', generated)">
        {{ t('ai.replace') }}
      </el-button>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import { dbSchemaSnapshot } from '../api/db'
import { dispatchCommand } from '../utils/commands'
import { objectIcon } from '../utils/objectIcons'
import { copyText } from '../utils/clipboard'
import {
  buildDiagnose,
  buildExplain,
  buildInsight,
  buildNl2sql,
  buildOptimize,
  compressSchema,
  stripSqlFences,
} from '../utils/aiPrompt'
import { useAiStore } from '../stores/ai'
import { useConnectionStore } from '../stores/connections'
import type { QueryTab } from '../stores/tabs'
import type { AiMode } from '../types/ai'
import type { SchemaSnapshot } from '../types/db'

const MODES: AiMode[] = ['nl2sql', 'explain', 'optimize', 'diagnose', 'insight']

const props = defineProps<{
  /** 所属查询页签（连接/库/方言上下文） */
  tab: QueryTab
  /** 取编辑器选中文本（解释/优化默认输入） */
  getSelection: () => string
  /** 外部预设模式（错误条「AI 诊断」等入口） */
  mode?: AiMode | null
  /** 外部预设输入（错误摘要等） */
  presetText?: string | null
  /** 外部预选表（树右键「AI 生成查询」） */
  presetTables?: string[] | null
  /** 数据洞察采样（结果集入口传入；insight 模式的分析对象） */
  presetSample?: { columns: string[]; rows: unknown[] } | null
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'insert', sql: string): void
  (e: 'replace', sql: string): void
}>()

const { t } = useI18n()
const aiStore = useAiStore()
const connStore = useConnectionStore()

const mode = ref<AiMode>('nl2sql')
const input = ref('')
const output = ref('')
const errorMsg = ref('')
const pickedTables = ref<string[]>([])
const snapshot = ref<SchemaSnapshot | null>(null)

const outRef = ref<HTMLElement>()
const activeName = computed(() => aiStore.active?.name ?? '')
const canSend = computed(
  () => !aiStore.running && !!aiStore.active && input.value.trim().length > 0,
)

const inputPlaceholder = computed(() =>
  mode.value === 'nl2sql'
    ? t('ai.phNl2sql')
    : mode.value === 'diagnose'
      ? t('ai.phDiagnose')
      : mode.value === 'insight'
        ? t('ai.phInsight')
        : t('ai.phGeneric'),
)

const tableOptions = computed(() =>
  (snapshot.value?.tables ?? []).map((tb) => ({
    value: tb.name,
    label: tb.comment ? `${tb.name} (${tb.comment})` : tb.name,
  })),
)

// ---- 上下文快照（懒加载：侧栏挂载 / 连接·库变化时刷新） ----

async function loadSnapshot(): Promise<void> {
  const sessionId = connStore.sessionOf(props.tab.connectionId)?.sessionId
  if (!sessionId) {
    snapshot.value = null
    return
  }
  try {
    snapshot.value = await dbSchemaSnapshot(sessionId, props.tab.database || null)
  } catch (e) {
    console.warn('AI 侧栏加载 schema 快照失败', e)
    snapshot.value = null
  }
}

onMounted(() => {
  if (props.mode) mode.value = props.mode
  if (props.presetText) input.value = props.presetText
  if (props.presetTables?.length) pickedTables.value = [...props.presetTables]
  void loadSnapshot()
})

watch(
  () => [props.tab.connectionId, props.tab.database] as const,
  () => void loadSnapshot(),
)

// 外部入口（错误条/树右键等）切换模式/预填
watch(
  () => [props.mode, props.presetText, props.presetTables] as const,
  ([m, text, tables]) => {
    if (m) mode.value = m
    if (text != null) {
      input.value = text
      output.value = ''
      errorMsg.value = ''
    }
    if (tables != null) pickedTables.value = [...tables]
  },
)

function switchMode(m: AiMode): void {
  mode.value = m
  output.value = ''
  errorMsg.value = ''
}

function openSettings(): void {
  dispatchCommand('open-settings', undefined, 'ai')
}

// ---- 发送与流式接收 ----

function buildMessages() {
  const kind = props.tab.kind
  switch (mode.value) {
    case 'nl2sql': {
      const ctx =
        aiStore.prefs.contextEnabled && snapshot.value
          ? compressSchema(snapshot.value, pickedTables.value.length ? pickedTables.value : null)
          : ''
      return buildNl2sql(kind, ctx, input.value.trim())
    }
    case 'explain':
      return buildExplain(kind, props.getSelection() || props.tab.sql)
    case 'optimize':
      return buildOptimize(kind, props.getSelection() || props.tab.sql)
    case 'diagnose':
      return buildDiagnose(kind, input.value.trim(), props.tab.error || input.value.trim())
    case 'insight': {
      // 数据洞察：隐私红线——入口已门控 insightEnabled；补充问题可选
      const sample = props.presetSample ?? { columns: [], rows: [] }
      const extra = input.value.trim()
      const msgs = buildInsight(kind, sample.columns, sample.rows)
      if (extra) msgs.push({ role: 'user', content: extra })
      return msgs
    }
    default:
      return []
  }
}

function send(): void {
  if (!canSend.value) return
  // 隐私门控：数据洞察外发采样需显式开启（设置 → AI 能力）
  if (mode.value === 'insight' && !aiStore.prefs.insightEnabled) {
    errorMsg.value = t('ai.insightBlocked')
    return
  }
  const messages = buildMessages()
  output.value = ''
  errorMsg.value = ''
  void aiStore
    .stream(
      messages,
      (delta) => {
        output.value += delta
        scrollBottom()
      },
      () => finalize(),
      (msg) => {
        errorMsg.value = msg
      },
    )
    .catch((e: unknown) => {
      errorMsg.value = e instanceof Error ? e.message : String(e)
    })
}

function finalize(): void {
  if (mode.value === 'nl2sql' || mode.value === 'optimize') {
    const stripped = stripSqlFences(output.value)
    if (stripped) output.value = stripped
  }
}

function scrollBottom(): void {
  requestAnimationFrame(() => {
    const el = outRef.value
    if (el) el.scrollTop = el.scrollHeight
  })
}

const generated = computed(() => {
  if (!output.value || aiStore.running) return ''
  if (mode.value === 'nl2sql' || mode.value === 'optimize') return stripSqlFences(output.value)
  return ''
})

async function copyOut(): Promise<void> {
  await copyText(generated.value || output.value)
  ElMessage.success(t('common.copied'))
}
</script>

<style scoped>
.ai-side {
  display: flex;
  flex-direction: column;
  width: 340px;
  min-width: 280px;
  border-left: 1px solid var(--pd-band-border, var(--el-border-color-lighter));
  background: var(--pd-band-bg, var(--el-bg-color-page));
  overflow: hidden;
}

.ai-head {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  border-bottom: 1px solid var(--el-border-color-lighter);
}

.ai-logo {
  color: var(--el-color-primary);
}

.ai-title {
  font-weight: 600;
  font-size: 12px;
}

.ai-provider {
  font-size: 11px;
  color: var(--el-text-color-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.flex-sp {
  flex: 1;
}

.ai-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border: none;
  background: transparent;
  color: var(--el-text-color-regular);
  cursor: pointer;
  border-radius: 4px;
}

.ai-btn:hover {
  background: var(--el-fill-color);
}

.ai-modes {
  display: flex;
  gap: 4px;
  padding: 6px 8px 2px;
  flex-wrap: wrap;
}

.ai-mode-chip {
  border: 1px solid var(--el-border-color);
  background: transparent;
  color: var(--el-text-color-regular);
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 10px;
  cursor: pointer;
}

.ai-mode-chip.is-active {
  color: var(--el-color-primary);
  border-color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
}

.ai-ctx {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 4px 8px;
}

.ai-tables {
  width: 100%;
}

.ai-input {
  padding: 4px 8px;
}

.ai-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 8px 6px;
}

.ai-warn {
  font-size: 11px;
  color: var(--el-color-warning);
}

.ai-out {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 6px 10px;
  border-top: 1px solid var(--el-border-color-lighter);
}

.ai-empty {
  font-size: 12px;
  color: var(--el-text-color-placeholder);
  padding-top: 16px;
  text-align: center;
}

.ai-out-text {
  margin: 0;
  font-family: var(--el-font-family, monospace);
  font-size: 12px;
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-word;
}

.ai-cursor {
  animation: ai-blink 1s step-start infinite;
  color: var(--el-color-primary);
}

@keyframes ai-blink {
  50% {
    opacity: 0;
  }
}

.ai-error {
  margin-top: 6px;
  font-size: 12px;
  color: var(--el-color-danger);
  white-space: pre-wrap;
}

.ai-foot {
  display: flex;
  gap: 6px;
  padding: 6px 8px;
  border-top: 1px solid var(--el-border-color-lighter);
}
</style>

<template>
  <el-drawer
    :model-value="modelValue"
    :title="t('cell.field', { name: detail?.name ?? '' })"
    size="620px"
    destroy-on-close
    @update:model-value="emit('update:modelValue', $event)"
  >
    <template v-if="detail">
      <div class="detail-meta">
        <el-tag size="small" type="info">{{ detail.typeLabel || t('cell.semText') }}</el-tag>
        <el-tag size="small">{{ semanticLabel }}</el-tag>
        <span v-if="statsText" class="detail-stats">{{ statsText }}</span>
        <el-button size="small" text type="primary" class="copy-btn" @click="copyValue">
          {{ t('cell.copy') }}
        </el-button>
      </div>

      <el-tabs v-model="activeTab" class="detail-tabs">
        <!-- JSON -->
        <template v-if="detail.semantic === 'json'">
          <el-tab-pane :label="t('cell.jsonTree')" name="tree">
            <div class="detail-scroll">
              <JsonTree v-if="parsedJson !== undefined" :data="parsedJson" />
              <el-alert v-else type="warning" :closable="false" :title="t('cell.invalidJson')" />
            </div>
          </el-tab-pane>
          <el-tab-pane :label="t('cell.pretty')" name="pretty">
            <pre class="detail-pre">{{ prettyJson }}</pre>
          </el-tab-pane>
        </template>

        <!-- 二进制 -->
        <template v-else-if="detail.semantic === 'binary'">
          <el-tab-pane label="HEX" name="hex">
            <HexViewer v-if="binaryBase64" :base64="binaryBase64" />
            <el-alert v-else type="warning" :closable="false" :title="t('cell.invalidBinary')" />
          </el-tab-pane>
        </template>

        <!-- 日期时间 -->
        <template
          v-else-if="
            detail.semantic === 'date' || detail.semantic === 'datetime' || detail.semantic === 'time'
          "
        >
          <el-tab-pane :label="t('cell.formats')" name="formats">
            <div class="detail-scroll">
              <table class="format-table">
                <tbody>
                  <tr v-for="f in dateFormats" :key="f.label">
                    <td class="ft-label">{{ f.label }}</td>
                    <td class="ft-value">{{ f.value }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
          </el-tab-pane>
          <el-tab-pane :label="t('cell.raw')" name="raw">
            <pre class="detail-pre">{{ rawText }}</pre>
          </el-tab-pane>
        </template>

        <!-- 文本 / 其他 -->
        <template v-else>
          <el-tab-pane :label="t('cell.content')" name="text">
            <div class="text-tools">
              <el-radio-group v-model="textView" size="small">
                <el-radio-button value="text">{{ t('cell.viewText') }}</el-radio-button>
                <el-radio-button value="hex">{{ t('cell.viewHex') }}</el-radio-button>
              </el-radio-group>
              <template v-if="textView === 'text'">
                <el-checkbox v-model="wrapText">{{ t('cell.wrapText') }}</el-checkbox>
                <el-checkbox v-model="showInvisible">{{ t('cell.showInvisible') }}</el-checkbox>
              </template>
            </div>
            <pre v-if="textView === 'text'" class="detail-pre" :class="{ wrap: wrapText }">{{ displayText }}</pre>
            <HexViewer v-else :base64="textBase64" />
          </el-tab-pane>
        </template>
      </el-tabs>
    </template>
  </el-drawer>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import JsonTree from './JsonTree.vue'
import HexViewer from './HexViewer.vue'
import { usePreferencesStore } from '../../stores/preferences'
import type { CellValue, SemanticType } from '../../types/db'

export interface CellDetail {
  name: string
  typeLabel: string
  semantic: SemanticType
  value: CellValue
}

const props = defineProps<{ modelValue: boolean; detail: CellDetail | null }>()

const emit = defineEmits<{ (e: 'update:modelValue', v: boolean): void }>()

const { t } = useI18n()
const prefs = usePreferencesStore()
const activeTab = ref('text')

watch(
  () => props.modelValue,
  (open) => {
    if (!open || !props.detail) return
    activeTab.value =
      props.detail.semantic === 'json'
        ? 'tree'
        : props.detail.semantic === 'binary'
          ? 'hex'
          : ['date', 'datetime', 'time'].includes(props.detail.semantic)
            ? 'formats'
            : 'text'
  },
)

const rawText = computed(() => {
  const v = props.detail?.value
  if (v === null || v === undefined) {
    const nullText = prefs.dataFormat.nullText.trim()
    return nullText || t('cell.null')
  }
  return String(v)
})

/** 文本查看器：视图模式 / 不可见字符显隐 / 换行 */
const textView = ref<'text' | 'hex'>('text')
const showInvisible = ref(false)
const wrapText = ref(true)

/** 显示不可见字符（CR/LF/TAB 等控制符符号化） */
const displayText = computed(() => {
  const s = rawText.value
  if (!showInvisible.value) return s
  return s
    .replace(/\r/g, '␍')
    .replace(/\n/g, '␊\n')
    .replace(/\t/g, '␉')
    .replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/g, '·')
})

/** 文本 UTF-8 编码为 base64（HEX 视图复用 HexViewer） */
const textBase64 = computed(() => {
  const bytes = new TextEncoder().encode(rawText.value)
  let bin = ''
  for (const b of bytes) bin += String.fromCharCode(b)
  return btoa(bin)
})

const parsedJson = computed<unknown | undefined>(() => {
  try {
    return JSON.parse(rawText.value)
  } catch {
    return undefined
  }
})

const prettyJson = computed(() =>
  parsedJson.value === undefined ? rawText.value : JSON.stringify(parsedJson.value, null, 2),
)

/** base64 解码失败返回 null */
const binaryBase64 = computed<string | null>(() => {
  const v = props.detail?.value
  if (typeof v !== 'string' || v.length === 0) return null
  try {
    atob(v)
    return v
  } catch {
    return null
  }
})

const dateFormats = computed(() => {
  const raw = rawText.value
  const list: Array<{ label: string; value: string }> = [
    { label: t('cell.raw'), value: raw },
  ]
  const ts = Date.parse(raw.replace(' ', 'T'))
  if (!Number.isNaN(ts)) {
    const d = new Date(ts)
    list.push({ label: t('cell.fmtIso'), value: d.toISOString() })
    list.push({ label: t('cell.fmtLocal'), value: d.toLocaleString() })
    list.push({ label: t('cell.fmtUnixSec'), value: String(Math.floor(ts / 1000)) })
    list.push({ label: t('cell.fmtUnixMs'), value: String(ts) })
  }
  return list
})

const statsText = computed(() => {
  const s = rawText.value
  if (props.detail?.semantic === 'binary' || s === t('cell.null')) return ''
  return t('cell.chars', { n: s.length.toLocaleString() })
})

const semanticLabel = computed(() => t(`cell.sem${capitalize(props.detail?.semantic ?? 'unknown')}`))

function capitalize(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1)
}

async function copyValue(): Promise<void> {
  await navigator.clipboard.writeText(rawText.value)
  ElMessage.success(t('cell.copied'))
}
</script>

<style scoped>
.detail-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
}

.detail-stats {
  color: var(--el-text-color-secondary);
  font-size: 12px;
}

.copy-btn {
  margin-left: auto;
}

.detail-tabs {
  height: calc(100% - 40px);
}

.text-tools {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 8px;
}

.detail-scroll {
  overflow: auto;
  max-height: 68vh;
}

.detail-pre {
  margin: 0;
  font-family: Consolas, 'Cascadia Mono', monospace;
  font-size: 12px;
  line-height: 1.6;
  white-space: pre;
  overflow: auto;
  max-height: 68vh;
}

.detail-pre.wrap {
  white-space: pre-wrap;
  word-break: break-all;
}

.format-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}

.format-table td {
  border: 1px solid var(--el-border-color-lighter);
  padding: 6px 10px;
}

.ft-label {
  width: 100px;
  color: var(--el-text-color-secondary);
  background: var(--el-fill-color-light);
}
</style>

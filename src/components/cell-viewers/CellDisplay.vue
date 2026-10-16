<template>
  <a
    v-if="isUrl"
    class="cell-display cell-link"
    :class="{ 'is-nowrap': compact }"
    :href="urlText"
    :title="text"
    @click.prevent="openLink"
  >{{ text }}</a>
  <span
    v-else
    class="cell-display"
    :class="[cellClass, { 'is-nowrap': compact }]"
    :title="titleText"
  >{{ text }}</span>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { openUrl } from '@tauri-apps/plugin-opener'
import { usePreferencesStore } from '../../stores/preferences'
import type { CellValue, SemanticType } from '../../types/db'

/** 单元格显示格式（列级 / 全局默认） */
export type CellFormat = 'raw' | 'pretty' | 'local' | 'iso' | 'thousand' | 'full'

const props = withDefaults(
  defineProps<{
    value: CellValue
    semantic?: SemanticType
    /** 紧凑模式：长文本截断（表格单元格）；详情模式展示全部 */
    compact?: boolean
    /** 显示格式：按语义应用于 JSON/日期/数值/长文本（未设置时应用全局数据格式偏好） */
    format?: CellFormat
  }>(),
  { semantic: 'unknown', compact: true, format: 'raw' },
)

const { t } = useI18n()
const prefs = usePreferencesStore()

const isBinary = computed(
  () => props.semantic === 'binary' && typeof props.value === 'string',
)

const dateLike = computed(
  () => props.semantic === 'date' || props.semantic === 'datetime' || props.semantic === 'time',
)

const text = computed(() => {
  const v = props.value
  if (v === null || v === undefined) {
    return prefs.dataFormat.nullText.trim() || t('cell.null')
  }
  if (isBinary.value && typeof v === 'string') {
    const bytes = Math.floor((v.length * 3) / 4)
    return bytes >= 1024
      ? t('cell.binaryKb', { n: Math.round(bytes / 1024) })
      : t('cell.binaryBytes', { n: bytes })
  }
  if (typeof v === 'boolean') {
    return prefs.dataFormat.boolStyle === 'yesNo' ? (v ? 'YES' : 'NO') : v ? 'true' : 'false'
  }

  const s = String(v)

  // —— 格式化展示（列级设置优先） ——
  if (props.format === 'pretty' && props.semantic === 'json') {
    try {
      return JSON.stringify(JSON.parse(s), null, 2)
    } catch {
      return s
    }
  }
  const useThousand =
    props.format === 'thousand' ||
    (props.format === 'raw' &&
      prefs.dataFormat.thousand &&
      (props.semantic === 'number' || props.semantic === 'decimal'))
  if (useThousand) {
    const n = Number(s)
    if (!Number.isNaN(n)) return n.toLocaleString()
  }
  if (dateLike.value && (props.format === 'local' || props.format === 'iso')) {
    const ts = Date.parse(s.replace(' ', 'T'))
    if (!Number.isNaN(ts)) {
      const d = new Date(ts)
      return props.format === 'iso' ? d.toISOString() : d.toLocaleString()
    }
  }
  // 无列级覆盖：应用全局日期格式偏好
  if (dateLike.value && props.format === 'raw') {
    const ds = prefs.dataFormat.dateStyle
    if (ds !== 'original') {
      const ts = Date.parse(s.replace(' ', 'T'))
      if (!Number.isNaN(ts)) {
        const d = new Date(ts)
        return ds === 'iso' ? d.toISOString() : d.toLocaleString()
      }
    }
  }

  // 长文本：full 不截断，否则按偏好的截断长度
  const limit = Math.max(20, prefs.dataFormat.truncateLength)
  const effectiveCompact = props.format === 'full' ? false : props.compact
  if (effectiveCompact && s.length > limit) {
    return `${s.slice(0, limit)}…（${t('cell.chars', { n: s.length })}）`
  }
  return s
})

const cellClass = computed(() => ({
  'is-null': props.value === null || props.value === undefined,
  'is-number': props.semantic === 'number' || props.semantic === 'decimal',
  'is-bool': props.semantic === 'bool',
  'is-binary': isBinary.value,
  'is-json': props.semantic === 'json' && props.format === 'pretty',
}))

const titleText = computed(() => {
  const v = props.value
  if (v === null || v === undefined || isBinary.value) return ''
  const s = String(v)
  return s.length > 120 ? s.slice(0, 200) : ''
})

/** URL 可点击（仅纯文本值且以 http(s) 开头） */
const urlText = computed(() =>
  typeof props.value === 'string' && /^https?:\/\//i.test(props.value.trim()) ? props.value.trim() : '',
)
const isUrl = computed(
  () => urlText.value !== '' && !urlText.value.includes('\n'),
)

async function openLink(): Promise<void> {
  try {
    await openUrl(urlText.value)
  } catch {
    window.open(urlText.value, '_blank', 'noopener')
  }
}
</script>

<style scoped>
.cell-display {
  white-space: pre-wrap;
  word-break: break-all;
}

/* 表格内固定行高：不换行 + 省略号，长文本经 show-overflow-tooltip / 悬浮卡查看 */
.cell-display.is-nowrap {
  display: block;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  word-break: keep-all;
}

.is-null {
  color: var(--el-text-color-placeholder);
  font-style: italic;
  font-size: 11px;
}

.is-number {
  font-variant-numeric: tabular-nums;
}

.is-bool {
  color: var(--el-color-warning);
  font-family: Consolas, monospace;
  font-size: 12px;
}

.is-binary {
  color: var(--el-color-info);
  font-size: 12px;
}

.is-json {
  font-family: Consolas, monospace;
  font-size: 12px;
}

.cell-link {
  color: var(--el-color-primary);
  text-decoration: none;
  cursor: pointer;
}
.cell-link:hover {
  text-decoration: underline;
}
</style>
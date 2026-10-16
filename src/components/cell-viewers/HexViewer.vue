<template>
  <el-tabs v-model="activeTab" class="hex-viewer">
    <el-tab-pane label="HEX" name="hex">
      <div class="hex-meta">
        {{ t('cell.bytes', { n: totalBytes.toLocaleString() }) }}
        <span v-if="truncated">（{{ t('cell.onlyFirst', { n: MAX_VIEW }) }}）</span>
        <el-button size="small" text type="primary" @click="copyHex">
          {{ t('cell.copyHex') }}
        </el-button>
      </div>
      <div class="hex-body">
        <div v-for="row in rows" :key="row.offset" class="hex-row">
          <span class="hex-offset">{{ row.offsetHex }}</span>
          <span class="hex-bytes">
            <span v-for="(b, i) in row.bytes" :key="i" class="hex-byte" :class="{ 'is-ascii': b >= 0x20 && b < 0x7f }">{{ b.toString(16).padStart(2, '0') }}</span>
          </span>
          <span class="hex-ascii">{{ row.ascii }}</span>
        </div>
      </div>
    </el-tab-pane>
    <el-tab-pane v-if="imageDataUri" :label="t('cell.imagePreview')" name="image">
      <div class="image-wrap">
        <img :src="imageDataUri" alt="blob preview" />
      </div>
    </el-tab-pane>
  </el-tabs>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'

const props = defineProps<{ base64: string }>()

const { t } = useI18n()
const MAX_VIEW = 8192
const activeTab = ref('hex')

const bytes = computed<Uint8Array>(() => {
  const bin = atob(props.base64)
  const arr = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) arr[i] = bin.charCodeAt(i)
  return arr
})

const totalBytes = computed(() => bytes.value.length)
const truncated = computed(() => totalBytes.value > MAX_VIEW)

/** 图片魔数检测：PNG / JPEG / GIF / BMP / WEBP */
const imageMime = computed<string | null>(() => {
  const b = bytes.value
  if (b.length < 12) return null
  if (b[0] === 0x89 && b[1] === 0x50 && b[2] === 0x4e && b[3] === 0x47) return 'image/png'
  if (b[0] === 0xff && b[1] === 0xd8 && b[2] === 0xff) return 'image/jpeg'
  if (b[0] === 0x47 && b[1] === 0x49 && b[2] === 0x46) return 'image/gif'
  if (b[0] === 0x42 && b[1] === 0x4d) return 'image/bmp'
  if (
    b[0] === 0x52 && b[1] === 0x49 && b[2] === 0x46 && b[3] === 0x46 &&
    b[8] === 0x57 && b[9] === 0x45 && b[10] === 0x42 && b[11] === 0x50
  ) {
    return 'image/webp'
  }
  return null
})

const imageDataUri = computed(() => {
  if (!imageMime.value) return null
  return `data:${imageMime.value};base64,${props.base64}`
})

const rows = computed(() => {
  const data = bytes.value.subarray(0, MAX_VIEW)
  const out: Array<{ offset: number; offsetHex: string; bytes: number[]; ascii: string }> = []
  for (let off = 0; off < data.length; off += 16) {
    const chunk = Array.from(data.subarray(off, off + 16))
    const ascii = chunk
      .map((b) => (b >= 0x20 && b < 0x7f ? String.fromCharCode(b) : '.'))
      .join('')
    out.push({
      offset: off,
      offsetHex: off.toString(16).padStart(8, '0'),
      bytes: chunk,
      ascii,
    })
  }
  return out
})

async function copyHex(): Promise<void> {
  const hex = Array.from(bytes.value.subarray(0, MAX_VIEW))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('')
  await navigator.clipboard.writeText(hex)
  ElMessage.success(t('cell.hexCopied'))
}
</script>

<style scoped>
.hex-viewer {
  height: 100%;
}

.hex-meta {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-bottom: 6px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.hex-body {
  font-family: Consolas, 'Cascadia Mono', monospace;
  font-size: 12px;
  overflow: auto;
  max-height: 60vh;
}

.hex-row {
  display: flex;
  gap: 10px;
  line-height: 18px;
  white-space: nowrap;
}

.hex-offset {
  color: var(--el-text-color-secondary);
}

.hex-bytes {
  display: inline-flex;
  gap: 3px;
}

.hex-byte {
  width: 20px;
  text-align: center;
  color: var(--el-text-color-regular);
}

.hex-byte.is-ascii {
  color: var(--el-color-primary);
}

.hex-ascii {
  color: var(--el-text-color-secondary);
}

.image-wrap {
  display: flex;
  justify-content: center;
  padding: 12px;
}

.image-wrap img {
  max-width: 100%;
  max-height: 55vh;
  border: 1px solid var(--el-border-color-light);
  background:
    conic-gradient(#eee 0 25%, #fff 0 50%, #eee 0 75%, #fff 0) 0 0/16px 16px;
}
</style>

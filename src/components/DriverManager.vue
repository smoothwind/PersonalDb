<template>
  <el-dialog
    :model-value="modelValue"
    width="920px"
    top="6vh"
    @update:model-value="emit('update:modelValue', $event)"
    @open="load"
  >
    <template #header>
      <span class="dlg-title">
        <el-icon><component :is="objectIcon('act_driver')" /></el-icon>
        {{ t('driver.title') }}
      </span>
    </template>
    <div class="dm" @dragover.prevent @drop.prevent="onFileDrop">
      <div class="dm-list">
        <div class="dm-toolbar">
          <el-button size="small" :icon="Plus" @click="onNew">{{ t('driver.new') }}</el-button>
          <el-button size="small" :disabled="!current" @click="onCopy">{{ t('driver.copy') }}</el-button>
          <el-button size="small" :disabled="!current || current.builtin" @click="onEdit">
            {{ t('driver.edit') }}
          </el-button>
          <el-button
            size="small"
            type="danger"
            plain
            :disabled="!current || current.builtin"
            @click="onDelete"
          >
            {{ t('driver.delete') }}
          </el-button>
        </div>
        <el-table
          :data="drivers"
          height="420"
          highlight-current-row
          row-key="id"
          @current-change="onSelect"
        >
          <el-table-column prop="name" :label="t('driver.name')" min-width="120" />
          <el-table-column prop="driverClass" :label="t('driver.class')" min-width="180" show-overflow-tooltip />
          <el-table-column :label="t('driver.builtin')" width="70">
            <template #default="{ row }">
              <el-tag v-if="row.builtin" size="small" type="info">{{ t('driver.builtinTag') }}</el-tag>
            </template>
          </el-table-column>
        </el-table>
      </div>

      <div v-if="showForm" class="dm-form">
        <el-form label-position="top" size="small">
          <el-form-item :label="t('driver.name')">
            <el-input v-model="form.name" />
          </el-form-item>
          <el-form-item :label="t('driver.class')">
            <div class="row">
              <el-input v-model="form.driverClass" :placeholder="t('driver.classPlaceholder')" />
              <el-button :disabled="form.jars.length === 0" :loading="scanning" @click="onFindClass">
                {{ t('driver.findClass') }}
              </el-button>
            </div>
          </el-form-item>
          <el-form-item :label="t('driver.urlTemplate')">
            <el-input v-model="form.urlTemplate" placeholder="jdbc:mysql://{host}:{port}/{database}" />
          </el-form-item>
          <el-form-item :label="t('driver.defaultPort')">
            <el-input-number v-model="form.defaultPort" :min="0" :max="65535" controls-position="right" />
          </el-form-item>

          <el-form-item :label="t('driver.jars')">
            <div class="jars">
              <div v-for="(j, i) in form.jars" :key="i" class="jar-row">
                <span class="jar-path" :title="j">{{ j }}</span>
                <el-button link type="danger" size="small" @click="form.jars.splice(i, 1)">
                  {{ t('driver.remove') }}
                </el-button>
              </div>
              <div class="row">
                <el-button size="small" :icon="Plus" @click="addJarFiles">{{ t('driver.addFile') }}</el-button>
                <el-button size="small" @click="addJarDir">{{ t('driver.addFolder') }}</el-button>
              </div>
              <div class="hint">{{ t('driver.localHint') }}</div>
            </div>
          </el-form-item>
        </el-form>

        <div v-if="classOptions.length" class="classes">
          <div class="classes-title">{{ t('driver.foundClasses') }}</div>
          <el-radio-group v-model="form.driverClass" size="small">
            <el-radio v-for="c in classOptions" :key="c" :label="c" border>{{ c }}</el-radio>
          </el-radio-group>
        </div>

        <div class="form-actions">
          <el-button size="small" @click="showForm = false">{{ t('common.cancel') }}</el-button>
          <el-button size="small" type="primary" @click="onSave">{{ t('common.save') }}</el-button>
        </div>
      </div>
      <div v-else class="dm-empty">{{ t('driver.selectHint') }}</div>
    </div>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { objectIcon } from '../utils/objectIcons'
import { ElMessage } from 'element-plus'
import { Plus } from '@element-plus/icons-vue'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import {
  jdbcDriversList,
  jdbcDriverSave,
  jdbcDriverDelete,
  jdbcDriverScanClasses,
} from '../api/db'
import type { JdbcDriver } from '../types/db'

defineProps<{ modelValue: boolean }>()
const emit = defineEmits<{ (e: 'update:modelValue', v: boolean): void }>()

const { t } = useI18n()

const drivers = ref<JdbcDriver[]>([])
const current = ref<JdbcDriver | null>(null)
const showForm = ref(false)
const form = ref<JdbcDriver>(blank())
const classOptions = ref<string[]>([])
const scanning = ref(false)

function blank(): JdbcDriver {
  return {
    id: crypto.randomUUID(),
    name: '',
    driverClass: '',
    urlTemplate: '',
    defaultPort: null,
    jars: [],
    builtin: false,
    description: null,
  }
}

async function load(): Promise<void> {
  try {
    drivers.value = await jdbcDriversList()
  } catch (e) {
    ElMessage.error(String(e))
  }
}

function onSelect(row: JdbcDriver | null): void {
  current.value = row
}

function onNew(): void {
  classOptions.value = []
  form.value = blank()
  showForm.value = true
}

function onCopy(): void {
  if (!current.value) return
  classOptions.value = []
  form.value = {
    ...JSON.parse(JSON.stringify(current.value)),
    id: crypto.randomUUID(),
    name: `${current.value.name} copy`,
    builtin: false,
  }
  showForm.value = true
}

function onEdit(): void {
  if (!current.value) return
  classOptions.value = []
  form.value = JSON.parse(JSON.stringify(current.value))
  showForm.value = true
}

async function onDelete(): Promise<void> {
  if (!current.value) return
  try {
    drivers.value = await jdbcDriverDelete(current.value.id)
    current.value = null
  } catch (e) {
    ElMessage.error(String(e))
  }
}

async function onSave(): Promise<void> {
  if (!form.value.name.trim()) {
    ElMessage.warning(t('driver.nameRequired'))
    return
  }
  try {
    drivers.value = await jdbcDriverSave({ ...form.value, builtin: false })
    showForm.value = false
    ElMessage.success(t('common.saved'))
  } catch (e) {
    ElMessage.error(String(e))
  }
}

async function addJarFiles(): Promise<void> {
  const sel = await openDialog({
    multiple: true,
    filters: [{ name: 'JAR', extensions: ['jar'] }],
  })
  if (!sel) return
  const list = Array.isArray(sel) ? sel : [sel]
  form.value.jars.push(...list)
}

async function addJarDir(): Promise<void> {
  const sel = await openDialog({ directory: true })
  if (!sel || Array.isArray(sel)) return
  form.value.jars.push(sel)
}

/**
 * 拖拽导入 .jar：使用 **HTML5 拖放**（主窗已设 `dragDropEnabled:false`，
 * Windows WebView2 下原生拖放会吞掉 HTML5 的 dragover/drop 事件）。
 * 路径来源：`text/uri-list`（Chromium 拖入文件时为 file:/// 列表），
 * 兜底读取 `dataTransfer.files[i].path`。
 */
function onFileDrop(e: DragEvent): void {
  const dt = e.dataTransfer
  if (!dt) return
  const files: string[] = []
  const uriList = dt.getData('text/uri-list') || dt.getData('text/plain')
  for (const line of uriList.split(/\r?\n/)) {
    const t2 = line.trim()
    if (!t2.startsWith('file:')) continue
    try {
      let p = decodeURIComponent(new URL(t2).pathname)
      if (/^\/[A-Za-z]:/.test(p)) p = p.slice(1)
      files.push(p)
    } catch {
      /* ignore */
    }
  }
  if (files.length === 0 && dt.files?.length) {
    for (const f of Array.from(dt.files)) {
      const p = (f as File & { path?: string }).path
      if (p) files.push(p)
    }
  }
  const jars = files.filter((p) => p.toLowerCase().endsWith('.jar'))
  if (jars.length === 0) {
    if (files.length > 0) ElMessage.warning(t('driver.dropNoJar'))
    else ElMessage.warning(t('driver.dropNoPath'))
    return
  }
  const set = new Set(form.value.jars)
  const add = jars.filter((p) => !set.has(p))
  if (add.length === 0) return
  form.value.jars.push(...add)
  ElMessage.success(t('driver.jarsAdded', { n: add.length }))
}

async function onFindClass(): Promise<void> {
  scanning.value = true
  try {
    classOptions.value = await jdbcDriverScanClasses(form.value.jars)
    if (classOptions.value.length === 0) {
      ElMessage.warning(t('driver.noClassFound'))
    }
  } catch (e) {
    ElMessage.error(String(e))
  } finally {
    scanning.value = false
  }
}
</script>

<style scoped>
.dm {
  display: flex;
  gap: 16px;
  min-height: 420px;
}
.dm-list {
  flex: 0 0 420px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.dm-toolbar {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}
.dm-form {
  flex: 1;
  overflow: auto;
  max-height: 520px;
  padding-right: 4px;
}
.dm-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--el-text-color-secondary);
  font-size: 13px;
}
.row {
  display: flex;
  gap: 6px;
  width: 100%;
}
.jars {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.hint {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.jar-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.jar-path {
  flex: 1;
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--el-text-color-secondary);
}
.classes {
  margin: 4px 0 12px;
}
.classes-title {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  margin-bottom: 4px;
}
.form-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 8px;
}
</style>

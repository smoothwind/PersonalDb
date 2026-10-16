<template>
  <el-dialog
    :model-value="modelValue"
    width="720px"
    destroy-on-close
    @update:model-value="emit('update:modelValue', $event)"
    @open="onOpen"
  >
    <template #header>
      <span class="dlg-title">
        <el-icon><component :is="objectIcon('act_settings')" /></el-icon>
        {{ t('snippets.title') }}
      </span>
    </template>
    <el-alert
      type="info"
      :closable="false"
      show-icon
      class="usage-tip"
      :title="usageText"
    />

    <div class="toolbar">
      <el-button type="primary" size="small" :icon="Plus" @click="addRow">
        {{ t('snippets.add') }}
      </el-button>
      <el-button size="small" :icon="RefreshLeft" @click="resetDefaults">
        {{ t('snippets.reset') }}
      </el-button>
      <span class="count">
        {{
          t('snippets.count', {
            total: rows.length,
            builtin: builtinCount,
            custom: snippetStore.custom.length,
          })
        }}
      </span>
    </div>

    <el-table :data="rows" size="small" border max-height="380">
      <el-table-column prop="abbrev" :label="t('snippets.abbrev')" width="110">
        <template #default="{ row }">
          <el-tag size="small" :type="row.source === builtinLabel ? 'info' : 'primary'">
            {{ row.abbrev }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="description" :label="t('snippets.description')" min-width="140" show-overflow-tooltip />
      <el-table-column prop="template" :label="t('snippets.template')" min-width="260" show-overflow-tooltip>
        <template #default="{ row }">
          <code class="tpl">{{ row.template }}</code>
        </template>
      </el-table-column>
      <el-table-column prop="source" :label="t('snippets.source')" width="90" />
      <el-table-column :label="t('snippets.actions')" width="130" fixed="right">
        <template #default="{ row }">
          <el-button size="small" text type="primary" @click="editRow(row)">
            {{ row.source === builtinLabel ? t('snippets.override') : t('common.edit') }}
          </el-button>
          <el-button
            v-if="row.source === customLabel"
            size="small"
            text
            type="danger"
            @click="removeRow(row)"
          >
            {{ t('common.delete') }}
          </el-button>
        </template>
      </el-table-column>
    </el-table>

    <el-dialog
      v-model="editVisible"
      :title="editingAbbrev ? t('snippets.edit') : t('snippets.addOne')"
      width="520px"
      append-to-body
    >
      <el-form :model="editForm" label-width="70px">
        <el-form-item :label="t('snippets.abbrev')" required>
          <el-input v-model="editForm.abbrev" :placeholder="t('snippets.abbrevPlaceholder')" maxlength="32">
            <template #append>=</template>
          </el-input>
        </el-form-item>
        <el-form-item :label="t('snippets.description')" required>
          <el-input v-model="editForm.description" :placeholder="t('snippets.descriptionPlaceholder')" />
        </el-form-item>
        <el-form-item :label="t('snippets.template')" required>
          <el-input
            v-model="editForm.template"
            type="textarea"
            :rows="4"
            :placeholder="t('snippets.tplPlaceholder')"
          />
          <div class="tpl-hint">{{ tplHint }}</div>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="editVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" @click="saveRow">{{ t('common.save') }}</el-button>
      </template>
    </el-dialog>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { objectIcon } from '../utils/objectIcons'
import { ElMessage } from 'element-plus'
import { Plus, RefreshLeft } from '@element-plus/icons-vue'
import { useSnippetStore } from '../stores/snippets'
import { defaultSnippets } from '../utils/snippetDefaults'
import type { DbKind, SqlSnippet } from '../types/db'

const props = defineProps<{ modelValue: boolean; kind: DbKind }>()

const emit = defineEmits<{ (e: 'update:modelValue', v: boolean): void }>()

const { t, te } = useI18n()
const snippetStore = useSnippetStore()

interface Row extends SqlSnippet {
  source: string
}

const rows = ref<Row[]>([])
const editVisible = ref(false)
const editingAbbrev = ref<string | null>(null)

const editForm = ref<SqlSnippet>({ abbrev: '', description: '', template: '' })

const builtinLabel = computed(() => t('snippets.builtin'))
const customLabel = computed(() => t('snippets.custom'))
const usageText = computed(() => (te('snippets.usage') ? t('snippets.usage') : ''))
const tplHint = computed(() => t('snippets.tplHint'))

const builtinCount = computed(() => defaultSnippets(props.kind).length)

function onOpen(): void {
  void snippetStore.load()
  rebuild()
}

function rebuild(): void {
  const map = snippetStore.mergedFor(props.kind)
  const customs = new Set(snippetStore.custom.map((s) => s.abbrev.toLowerCase()))
  rows.value = [...map.values()].map((s) => {
    const isCustom = customs.has(s.abbrev.toLowerCase())
    return {
      ...s,
      // 内置条目的 description 为 i18n key，展示时翻译
      description: isCustom ? s.description : t(s.description),
      source: isCustom ? customLabel.value : builtinLabel.value,
    }
  })
}

function addRow(): void {
  editingAbbrev.value = null
  editForm.value = { abbrev: '', description: '', template: '' }
  editVisible.value = true
}

function editRow(row: Row): void {
  editingAbbrev.value = row.source === customLabel.value ? row.abbrev : null
  editForm.value = { abbrev: row.abbrev, description: row.description, template: row.template }
  editVisible.value = true
}

async function saveRow(): Promise<void> {
  const f = editForm.value
  const abbrev = f.abbrev.trim()
  if (!/^[A-Za-z0-9_]+$/.test(abbrev)) {
    ElMessage.warning(t('snippets.invalidAbbrev'))
    return
  }
  if (!f.description.trim() || !f.template.trim()) {
    ElMessage.warning(t('snippets.required'))
    return
  }
  // 内置项覆盖时先移除同名旧自定义（编辑场景）
  let list = snippetStore.custom.filter(
    (s) => s.abbrev.toLowerCase() !== abbrev.toLowerCase(),
  )
  list = [...list, { abbrev, description: f.description.trim(), template: f.template }]
  await snippetStore.persist(list)
  editVisible.value = false
  rebuild()
  ElMessage.success(t('snippets.saved', { abbrev }))
}

async function removeRow(row: Row): Promise<void> {
  const list = snippetStore.custom.filter(
    (s) => s.abbrev.toLowerCase() !== row.abbrev.toLowerCase(),
  )
  await snippetStore.persist(list)
  rebuild()
}

async function resetDefaults(): Promise<void> {
  await snippetStore.persist([])
  rebuild()
  ElMessage.success(t('snippets.resetDone'))
}
</script>

<style scoped>
.usage-tip {
  margin-bottom: 12px;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
}

.count {
  margin-left: auto;
  color: var(--el-text-color-secondary);
  font-size: 12px;
}

.tpl {
  font-family: Consolas, monospace;
  font-size: 12px;
}

.tpl-hint {
  color: var(--el-text-color-secondary);
  font-size: 12px;
  line-height: 1.5;
  margin-top: 4px;
}
</style>

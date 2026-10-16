<template>
  <div class="routine-editor">
    <div class="rt-head">
      <el-icon :class="'rt-' + routineType.toLowerCase()">
        <component :is="objectIcon(routineType.toLowerCase())" />
      </el-icon>
      <span class="rt-ctx">{{ tab.database ? tab.database + '.' : '' }}{{ tab.objectName }}</span>
      <el-tag size="small" effect="dark">{{ t('dbKind.' + tab.kind) }}</el-tag>
      <el-tag size="small" :type="routineType === 'FUNCTION' ? 'success' : 'warning'">
        {{ t('objectType.' + routineType.toLowerCase()) }}
      </el-tag>
      <span v-if="info?.returnType" class="rt-returns">
        <el-icon><Right /></el-icon>
        <span class="returns-label">{{ t('routine.returns') }}:</span>
        <code>{{ info.returnType }}</code>
      </span>
    </div>

    <div class="rt-body" v-loading="loading">
      <el-alert
        v-if="error"
        :title="error"
        type="error"
        show-icon
        :closable="false"
        class="rt-error"
      />

      <!-- 参数（可折叠） -->
      <el-collapse v-model="collapsePanels" class="rt-params">
        <el-collapse-item name="params">
          <template #title>
            <span class="params-title">
              <el-icon><component :is="objectIcon('obj_column')" /></el-icon>
              {{ t('routine.params') }}
              <el-tag size="small" type="info">{{ info?.params.length ?? 0 }}</el-tag>
            </span>
          </template>
          <el-table
            v-if="info && info.params.length > 0"
            :data="info.params"
            size="small"
            border
            max-height="200"
          >
            <el-table-column type="index" width="44" />
            <el-table-column prop="name" :label="t('routine.paramName')" min-width="140">
              <template #default="{ row }">
                <code>{{ row.name ?? '-' }}</code>
              </template>
            </el-table-column>
            <el-table-column :label="t('routine.paramMode')" width="90" align="center">
              <template #default="{ row }">
                <el-tag
                  size="small"
                  :type="row.mode === 'OUT' ? 'warning' : row.mode === 'INOUT' ? 'info' : 'success'"
                >
                  {{ row.mode }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="dataType" :label="t('routine.paramType')" min-width="180">
              <template #default="{ row }">
                <code>{{ row.dataType }}</code>
              </template>
            </el-table-column>
          </el-table>
          <EmptyState v-else size="compact" icon="obj_column" :title="t('routine.noParams')" />
        </el-collapse-item>
      </el-collapse>

      <!-- 源码编辑区 -->
      <div class="rt-toolbar">
        <!-- 段2：刷新 -->
        <el-button size="small" :loading="loading" @click="load">
          <el-icon><component :is="objectIcon('act_refresh')" /></el-icon>
          {{ t('common.refresh') }}
        </el-button>
        <span class="pt-sep" />
        <!-- 段3：编辑动作 -->
        <el-button size="small" :icon="CopyDocument" @click="copySource">
          {{ t('cell.copy') }}
        </el-button>
        <el-button
          size="small"
          :type="editMode ? 'warning' : 'default'"
          :icon="EditPen"
          @click="toggleEdit"
        >
          {{ editMode ? t('common.cancel') : t('common.edit') }}
        </el-button>
        <el-tooltip :content="t('query.format')" placement="bottom">
          <el-button size="small" circle :icon="Brush" :disabled="!editMode" @click="beautify" />
        </el-tooltip>
        <el-tooltip :content="t('codeViewer.upper')" placement="bottom">
          <el-button size="small" circle :disabled="!editMode" @click="caseConvert('upper')">
            <el-icon><component :is="objectIcon('act_upper')" /></el-icon>
          </el-button>
        </el-tooltip>
        <el-tooltip :content="t('codeViewer.lower')" placement="bottom">
          <el-button size="small" circle :disabled="!editMode" @click="caseConvert('lower')">
            <el-icon><component :is="objectIcon('act_lower')" /></el-icon>
          </el-button>
        </el-tooltip>
        <el-checkbox v-model="stripQualifier" size="small" :disabled="!editMode">{{ t('inspector.stripQualifier') }}</el-checkbox>
        <span class="pt-sep" />
        <el-button size="small" :icon="Document" @click="openInQuery">
          {{ t('inspector.openInQuery') }}
        </el-button>
        <span class="pt-spacer" />
        <el-tooltip :content="t('routine.saveTip')" placement="bottom">
          <el-button
            size="small"
            type="success"
            :loading="saving"
            :disabled="!editMode || !source.trim()"
            @click="save"
          >
            <el-icon><component :is="objectIcon('act_save')" /></el-icon>
            {{ t('routine.save') }}
          </el-button>
        </el-tooltip>
      </div>

      <div class="rt-source">
        <SqlEditor v-model="source" :kind="tab.kind" :snippets="snippetMap" :readonly="!editMode" fill />
      </div>

      <div v-if="saveResult" class="rt-save-result">{{ saveResult }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import {
  Brush,
  CopyDocument,
  Document,
  EditPen,
  Right,
} from '@element-plus/icons-vue'
import SqlEditor from './SqlEditor.vue'
import EmptyState from './EmptyState.vue'
import { objectIcon } from '../utils/objectIcons'
import { dbDescribeRoutine, dbExecute, dbFetchObjectDdl } from '../api/db'
import { useConnectionStore } from '../stores/connections'
import { usePreferencesStore } from '../stores/preferences'
import { formatSql } from '../utils/sqlFormat'
import { convertOutsideLiterals } from '../utils/sqlCase'
import { stripQualifier as stripQualifierText } from '../utils/ddlClean'
import { useSnippetStore } from '../stores/snippets'
import { useTabsStore, type QueryTab } from '../stores/tabs'
import type { RoutineInfo } from '../types/db'

/** 例程（存储过程 / 函数）编辑面板（Tab 形式：参数 + 可编辑源码 + 保存） */
const props = defineProps<{ tab: QueryTab }>()

const { t } = useI18n()
const connStore = useConnectionStore()
const snippetStore = useSnippetStore()
const tabsStore = useTabsStore()
const prefs = usePreferencesStore()

const loading = ref(false)
const saving = ref(false)
const error = ref('')
const saveResult = ref('')
const info = ref<RoutineInfo | null>(null)
const source = ref('')
/** 编辑状态：默认只读，点「编辑」后方可修改 */
const editMode = ref(false)
/** 去除限定符开关（作用于源码） */
const stripQualifier = ref(false)

function toggleEdit(): void {
  if (editMode.value) {
    editMode.value = false
    stripQualifier.value = false
    void load() // 取消：重新加载原始源码
  } else {
    editMode.value = true
  }
}

watch(stripQualifier, (v) => {
  if (v && source.value) source.value = stripQualifierText(source.value)
})

function beautify(): void {
  if (!source.value.trim()) return
  source.value = formatSql(props.tab.kind, source.value, prefs.formatPrefs)
}

function caseConvert(mode: 'upper' | 'lower'): void {
  source.value = convertOutsideLiterals(source.value, mode)
}
const collapsePanels = ref<string[]>(['params'])

const routineType = computed(() =>
  props.tab.objType === 'function' ? 'FUNCTION' : 'PROCEDURE',
)
const snippetMap = computed(() => snippetStore.mergedFor(props.tab.kind))

onMounted(load)

async function load(): Promise<void> {
  loading.value = true
  error.value = ''
  saveResult.value = ''
  info.value = null
  source.value = ''
  editMode.value = false
  stripQualifier.value = false
  try {
    const session = connStore.sessionOf(props.tab.connectionId)
    if (!session) {
      error.value = t('query.connectionLost')
      return
    }
    const db = props.tab.database || null
    const objType = props.tab.objType === 'function' ? 'function' : 'procedure'
    const name = props.tab.objectName ?? ''
    const [routine, ddl] = await Promise.all([
      dbDescribeRoutine(session.sessionId, db, name, objType),
      dbFetchObjectDdl(session.sessionId, db, objType, name),
    ])
    info.value = routine
    source.value = ddl
  } catch (e) {
    error.value = String(e)
  } finally {
    loading.value = false
  }
}

async function save(): Promise<void> {
  const session = connStore.sessionOf(props.tab.connectionId)
  if (!session) {
    ElMessage.error(t('query.connectionLost'))
    return
  }
  saving.value = true
  error.value = ''
  saveResult.value = ''
  try {
    const result = await dbExecute(session.sessionId, props.tab.database || null, source.value, 1)
    saveResult.value = t('routine.saveOk', { ms: result.durationMs })
    ElMessage.success(t('routine.saved'))
    await load()
  } catch (e) {
    error.value = String(e)
    ElMessage.error(t('routine.saveFailed', { msg: String(e) }))
  } finally {
    saving.value = false
  }
}

async function copySource(): Promise<void> {  await navigator.clipboard.writeText(source.value)
  ElMessage.success(t('cell.copied'))
}

function openInQuery(): void {
  const tab = tabsStore.openTab({
    connectionId: props.tab.connectionId,
    connectionName: props.tab.connectionName,
    kind: props.tab.kind,
    database: props.tab.database,
    title: props.tab.objectName,
    icon: props.tab.objType,
  })
  tab.sql = source.value
  tab.result = null
  tab.error = ''
}
</script>

<style scoped>
.routine-editor {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 8px 12px;
  overflow: hidden;
}

.rt-head {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
  min-width: 0;
}

.rt-ctx {
  font-family: Consolas, monospace;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.rt-function {
  color: #b37ceb;
}

.rt-procedure {
  color: var(--el-color-warning);
}

.rt-returns {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.rt-returns code {
  color: var(--el-color-success);
  font-family: Consolas, monospace;
}

.returns-label {
  margin-left: 2px;
}

.rt-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.rt-error {
  flex-shrink: 0;
}

.rt-params {
  flex-shrink: 0;
}

.params-title {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  font-weight: 600;
}

.rt-toolbar {
.rt-head :deep(.el-icon) {
  font-size: var(--pd-icon-md);
}
.rt-toolbar :deep(.el-icon) {
  font-size: var(--pd-icon-sm);
}

  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}


.rt-source {
  flex: 1;
  min-height: 220px;
}

.rt-save-result {
  flex-shrink: 0;
  font-size: 12px;
  color: var(--el-color-success);
}
</style>

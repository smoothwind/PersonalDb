<template>
  <div class="object-inspector">
    <div class="oi-toolbar">
      <!-- 段1：对象身份 -->
      <el-tag size="small" effect="dark">{{ t('dbKind.' + tab.kind) }}</el-tag>
      <span class="oi-ctx">{{ tab.database ? tab.database + '.' : '' }}{{ tab.objectName }}</span>
      <el-tag size="small">{{ t('objectType.' + (tab.objType ?? 'view')) }}</el-tag>
      <span class="pt-sep" />
      <!-- 段3：动作 + 状态 -->
      <el-button size="small" :loading="loading" @click="load">
        <el-icon><component :is="objectIcon('act_refresh')" /></el-icon>
        {{ t('common.refresh') }}
      </el-button>
      <el-button size="small" type="primary" :icon="Document" :disabled="!source" @click="openInQuery">
        {{ t('inspector.openInQuery') }}
      </el-button>
      <span class="pt-spacer" />
      <span v-if="source" class="oi-hint">{{ t('cell.chars', { n: source.length }) }}</span>
    </div>

    <el-alert
      v-if="error"
      :title="error"
      type="error"
      show-icon
      :closable="false"
      class="oi-error"
    />
    <div class="oi-source">
      <CodeViewer :model-value="source" :kind="tab.kind" :ddl-options="true" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Document } from '@element-plus/icons-vue'
import { objectIcon } from '../utils/objectIcons'
import CodeViewer from './CodeViewer.vue'
import { dbFetchObjectDdl } from '../api/db'
import { useConnectionStore } from '../stores/connections'
import { useTabsStore, type QueryTab } from '../stores/tabs'

/** 非表对象的只读定义查看面板（Tab 形式，表结构与源码见 TableInspectorPane） */
const props = defineProps<{ tab: QueryTab }>()

const { t } = useI18n()
const connStore = useConnectionStore()
const tabsStore = useTabsStore()

const loading = ref(false)
const error = ref('')
const source = ref('')

async function load(): Promise<void> {
  loading.value = true
  error.value = ''
  source.value = ''
  try {
    const session = connStore.sessionOf(props.tab.connectionId)
    if (!session) {
      error.value = t('query.connectionLost')
      return
    }
    source.value = await dbFetchObjectDdl(
      session.sessionId,
      props.tab.database || null,
      props.tab.objType ?? 'view',
      props.tab.objectName ?? '',
    )
    if (!source.value) source.value = t('query.emptyDefinition')
  } catch (e) {
    error.value = String(e)
  } finally {
    loading.value = false
  }
}

onMounted(load)

/** 将定义送入新查询 Tab（编辑/执行职责由编辑器承担） */
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
.object-inspector {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 8px 12px;
  overflow: hidden;
}

.oi-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.oi-ctx {
  font-family: Consolas, monospace;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.oi-hint {
  margin-left: auto;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}

.oi-error {
  flex-shrink: 0;
}

.oi-source {
  flex: 1;
  min-height: 0;
}
</style>

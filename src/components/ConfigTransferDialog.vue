<template>
  <el-dialog
    :model-value="visible"
    width="720px"
    :close-on-click-modal="false"
    @update:model-value="(v: boolean) => emit('update:visible', v)"
  >
    <template #header>
      <span class="dlg-title">
        <el-icon><component :is="objectIcon('act_export')" /></el-icon>
        {{ t('transfer.title') }}
      </span>
    </template>
    <el-tabs v-model="activeTab">
      <!-- ============ 导出 ============ -->
      <el-tab-pane :label="t('transfer.tabExport')" name="export">
        <el-checkbox-group v-model="exportCats" class="cat-group">
          <el-checkbox value="connections">{{ t('transfer.catConnections') }}</el-checkbox>
          <el-checkbox value="snippets">{{ t('transfer.catSnippets') }}</el-checkbox>
          <el-checkbox value="preferences">{{ t('transfer.catPreferences') }}</el-checkbox>
          <el-checkbox value="drivers">{{ t('transfer.catDrivers') }}</el-checkbox>
        </el-checkbox-group>
        <el-divider />
        <div class="row">
          <el-switch v-model="includePasswords" />
          <span class="row-label">{{ t('transfer.includePasswords') }}</span>
        </div>
        <el-input
          v-if="includePasswords"
          v-model="passphrase"
          type="password"
          show-password
          :placeholder="t('transfer.passphrasePlaceholder')"
          class="pass-input"
        />
        <div class="foot">
          <el-button type="primary" :loading="busy" @click="doExport">
            {{ t('transfer.btnExport') }}
          </el-button>
        </div>
      </el-tab-pane>

      <!-- ============ 导入 ============ -->
      <el-tab-pane :label="t('transfer.tabImport')" name="import">
        <div class="row">
          <span class="row-label">{{ t('transfer.mergeMode') }}</span>
          <el-radio-group v-model="mergeMode" size="small">
            <el-radio-button value="skip">{{ t('transfer.mergeSkip') }}</el-radio-button>
            <el-radio-button value="overwrite">{{ t('transfer.mergeOverwrite') }}</el-radio-button>
          </el-radio-group>
        </div>
        <div class="row">
          <span class="row-label">{{ t('transfer.vaultPass') }}</span>
          <el-input
            v-model="importPass"
            type="password"
            show-password
            :placeholder="t('transfer.importPassPlaceholder')"
            class="pass-input short"
          />
        </div>
        <div class="foot">
          <el-button type="primary" :loading="busy" @click="pickAndImport">
            {{ t('transfer.btnPickImportFile') }}
          </el-button>
        </div>
      </el-tab-pane>

      <!-- ============ 三方导入 ============ -->
      <el-tab-pane :label="t('transfer.tabThird')" name="third">
        <div class="row">
          <span class="row-label">{{ t('transfer.thirdSource') }}</span>
          <el-radio-group v-model="thirdSource" size="small">
            <el-radio-button value="dbeaver">DBeaver</el-radio-button>
            <el-radio-button value="navicat">Navicat</el-radio-button>
            <el-radio-button value="plsql">PL/SQL Developer</el-radio-button>
          </el-radio-group>
        </div>
        <el-table
          :data="thirdRows"
          size="small"
          height="240"
          class="third-table"
          @selection-change="(rows: ImportedConn[]) => (thirdSel = rows)"
        >
          <el-table-column type="selection" width="36" />
          <el-table-column prop="name" :label="t('transfer.colName')" min-width="110" show-overflow-tooltip />
          <el-table-column :label="t('transfer.colKind')" width="110">
            <template #default="{ row }">{{ t('dbKind.' + row.kind) }}</template>
          </el-table-column>
          <el-table-column prop="host" :label="t('transfer.colHost')" min-width="110" show-overflow-tooltip />
          <el-table-column prop="username" :label="t('transfer.colUser')" min-width="90" show-overflow-tooltip />
          <el-table-column :label="t('transfer.colPwd')" width="70">
            <template #default="{ row }">
              <el-tag size="small" :type="row.password ? 'success' : 'info'">
                {{ row.password ? t('transfer.pwdYes') : t('transfer.pwdNo') }}
              </el-tag>
            </template>
          </el-table-column>
        </el-table>
        <div class="foot">
          <el-button :loading="busy" @click="scanThird">{{ t('transfer.thirdPick') }}</el-button>
          <el-button
            type="primary"
            :disabled="thirdSel.length === 0"
            :loading="busy"
            @click="importThird"
          >
            {{ t('transfer.thirdImport', { n: thirdSel.length }) }}
          </el-button>
        </div>
      </el-tab-pane>
    </el-tabs>
  </el-dialog>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { objectIcon } from '../utils/objectIcons'
import { ElMessage } from 'element-plus'
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog'
import {
  importConnectionsScan,
  jdbcDriversList,
  jdbcDriverSave,
  readTextFile,
  saveTextFile,
  secretGet,
  vaultDecrypt,
  vaultEncrypt,
  type ImportedConn,
} from '../api/db'
import { useConnectionStore } from '../stores/connections'
import { useSnippetStore } from '../stores/snippets'
import type { ConnectionConfig, ConnectionGroup, JdbcDriver, SqlSnippet } from '../types/db'

const props = defineProps<{
  visible: boolean
  initialTab?: 'export' | 'import' | 'third'
}>()

const emit = defineEmits<{ (e: 'update:visible', v: boolean): void }>()

const { t } = useI18n()
const connStore = useConnectionStore()
const snippetStore = useSnippetStore()

const activeTab = ref(props.initialTab ?? 'export')
const busy = ref(false)

// ---------- 导出 ----------
const exportCats = ref<string[]>(['connections', 'snippets', 'preferences', 'drivers'])
const includePasswords = ref(false)
const passphrase = ref('')

function collectPreferences(): Record<string, unknown> {
  const out: Record<string, unknown> = {}
  for (let i = 0; i < localStorage.length; i++) {
    const key = localStorage.key(i)
    if (!key?.startsWith('personaldb.')) continue
    try {
      out[key] = JSON.parse(localStorage.getItem(key) ?? 'null')
    } catch {
      /* 跳过损坏项 */
    }
  }
  return out
}

async function doExport(): Promise<void> {
  if (includePasswords.value && !passphrase.value) {
    ElMessage.warning(t('transfer.needPass'))
    return
  }
  busy.value = true
  try {
    const data: Record<string, unknown> = {}
    if (exportCats.value.includes('connections')) {
      const conns: ConnectionConfig[] = []
      for (const c of connStore.saved) {
        const item: ConnectionConfig = { ...c, password: '' }
        if (includePasswords.value && c.passwordRef) {
          item.password = (await secretGet(c.passwordRef)) ?? ''
        }
        conns.push(item)
      }
      data.connections = conns
      data.groups = connStore.groups
    }
    if (exportCats.value.includes('snippets')) data.snippets = snippetStore.custom
    if (exportCats.value.includes('drivers')) data.drivers = await jdbcDriversList()
    if (exportCats.value.includes('preferences')) data.preferences = collectPreferences()
    let content: unknown = {
      app: 'personaldb-config',
      version: 1,
      createdAt: new Date().toISOString(),
      data,
    }
    if (includePasswords.value) {
      content = await vaultEncrypt(content, passphrase.value)
    }
    const ts = new Date().toISOString().slice(0, 10).replace(/-/g, '')
    const path = await saveDialog({
      defaultPath: `personaldb-config-${ts}.json`,
      filters: [{ name: 'JSON', extensions: ['json'] }],
    })
    if (!path) return
    await saveTextFile(path as string, JSON.stringify(content, null, 2))
    ElMessage.success(t('transfer.exportDone'))
    emit('update:visible', false)
  } catch (e) {
    ElMessage.error(String(e))
  } finally {
    busy.value = false
  }
}

// ---------- 导入 ----------
const mergeMode = ref<'skip' | 'overwrite'>('skip')
const importPass = ref('')

function mergeById<T extends { id: string }>(current: T[], incoming: T[]): { list: T[]; n: number } {
  const map = new Map(current.map((x) => [x.id, x]))
  let n = 0
  for (const item of incoming) {
    if (map.has(item.id) && mergeMode.value === 'skip') continue
    map.set(item.id, item)
    n++
  }
  return { list: [...map.values()], n }
}

function mergeSnippets(current: SqlSnippet[], incoming: SqlSnippet[]): { list: SqlSnippet[]; n: number } {
  const map = new Map(current.map((x) => [x.abbrev.toLowerCase(), x]))
  let n = 0
  for (const item of incoming) {
    const key = item.abbrev.toLowerCase()
    if (map.has(key) && mergeMode.value === 'skip') continue
    map.set(key, item)
    n++
  }
  return { list: [...map.values()], n }
}

async function pickAndImport(): Promise<void> {
  const picked = await openDialog({
    multiple: false,
    filters: [{ name: 'JSON', extensions: ['json'] }],
  })
  if (!picked) return
  busy.value = true
  try {
    let obj: Record<string, unknown> = JSON.parse(await readTextFile(picked as string))
    if (obj?.format === 'personaldb-vault') {
      if (!importPass.value) {
        ElMessage.warning(t('transfer.needVaultPass'))
        return
      }
      obj = (await vaultDecrypt(obj, importPass.value)) as Record<string, unknown>
    }
    if (obj?.app !== 'personaldb-config') {
      ElMessage.error(t('transfer.invalidFile'))
      return
    }
    const data = (obj.data ?? {}) as Record<string, unknown>
    let total = 0
    if (Array.isArray(data.groups)) {
      await connStore.mergeGroups(data.groups as ConnectionGroup[])
      total += (data.groups as unknown[]).length
    }
    if (Array.isArray(data.connections)) {
      for (const raw of data.connections as ConnectionConfig[]) {
        const exists = connStore.configOf(raw.id)
        if (exists && mergeMode.value === 'skip') continue
        await connStore.upsert({ ...raw, groupId: raw.groupId ?? null })
        total++
      }
    }
    if (Array.isArray(data.snippets)) {
      const { list, n } = mergeSnippets(snippetStore.custom, data.snippets as SqlSnippet[])
      await snippetStore.persist(list)
      total += n
    }
    if (Array.isArray(data.drivers)) {
      const current = await jdbcDriversList()
      const { list, n } = mergeById(current, data.drivers as JdbcDriver[])
      for (const d of list) {
        if (!d.builtin) await jdbcDriverSave(d)
      }
      total += n
    }
    if (data.preferences && typeof data.preferences === 'object') {
      for (const [k, v] of Object.entries(data.preferences as Record<string, unknown>)) {
        localStorage.setItem(k, JSON.stringify(v))
      }
      ElMessage.info(t('transfer.prefsRestart'))
    }
    ElMessage.success(t('transfer.importDone', { n: total }))
    emit('update:visible', false)
  } catch (e) {
    ElMessage.error(String(e))
  } finally {
    busy.value = false
  }
}

// ---------- 三方导入 ----------
const thirdSource = ref<'dbeaver' | 'navicat' | 'plsql'>('dbeaver')
const thirdRows = ref<ImportedConn[]>([])
const thirdSel = ref<ImportedConn[]>([])

async function scanThird(): Promise<void> {
  const ext = thirdSource.value === 'navicat' ? ['ncx', 'xml'] : thirdSource.value === 'plsql' ? ['ini', 'txt', 'inf'] : ['json']
  const label = thirdSource.value === 'navicat' ? 'Navicat NCX' : thirdSource.value === 'plsql' ? 'INI' : 'JSON'
  const picked = await openDialog({
    multiple: false,
    filters: [{ name: label, extensions: ext }, { name: 'All', extensions: ['*'] }],
  })
  if (!picked) return
  busy.value = true
  try {
    thirdRows.value = await importConnectionsScan(thirdSource.value, picked as string)
    thirdSel.value = []
    if (thirdRows.value.length === 0) ElMessage.info(t('transfer.thirdNone'))
  } catch (e) {
    ElMessage.error(String(e))
  } finally {
    busy.value = false
  }
}

async function importThird(): Promise<void> {
  busy.value = true
  try {
    let n = 0
    for (const row of thirdSel.value) {
      const cfg: ConnectionConfig = {
        id: crypto.randomUUID(),
        name: row.name || `${row.username}@${row.host}`,
        kind: row.kind,
        host: row.host,
        port: row.port ?? null,
        username: row.username,
        password: row.password ?? '',
        database: row.database ?? '',
        params: { ...(row.params ?? {}) },
        groupId: null,
      }
      await connStore.upsert(cfg)
      n++
    }
    ElMessage.success(t('transfer.thirdDone', { n }))
    if (thirdSel.value.some((r) => !r.password)) {
      ElMessage.info(t('transfer.pwdMissingHint'))
    }
    thirdRows.value = []
    thirdSel.value = []
    emit('update:visible', false)
  } catch (e) {
    ElMessage.error(String(e))
  } finally {
    busy.value = false
  }
}
</script>

<style scoped>
.cat-group {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 4px 0 12px;
}
.row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 10px 0;
}
.row-label {
  min-width: 92px;
  font-size: 13px;
}
.pass-input {
  flex: 1;
}
.pass-input.short {
  max-width: 260px;
}
.third-table {
  margin-top: 6px;
}
.foot {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 12px;
}
</style>

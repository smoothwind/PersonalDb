<template>
  <el-dialog
    :model-value="modelValue"
    width="560px"
    destroy-on-close
    @update:model-value="emit('update:modelValue', $event)"
    @open="onOpen"
  >
    <template #header>
      <span class="dlg-title">
        <el-icon><component :is="objectIcon('obj_connection')" /></el-icon>
        {{ initial ? t('form.edit') : t('form.create') }}
      </span>
    </template>
    <el-form ref="formRef" :model="form" :rules="rules" label-width="90px">
      <el-form-item :label="t('form.name')" prop="name">
        <el-input v-model="form.name" :placeholder="t('form.namePlaceholder')" />
      </el-form-item>
      <el-form-item :label="t('form.kind')" prop="kind">
        <el-select v-model="form.kind" style="width: 100%" @change="onKindChange">
          <el-option
            v-for="k in allKinds"
            :key="k.value"
            :label="k.label"
            :value="k.value"
            :disabled="k.disabled"
          />
        </el-select>
      </el-form-item>
      <el-form-item v-if="!isSqlite" :label="t('form.host')" prop="host">
        <el-input v-model="form.host" placeholder="127.0.0.1" />
      </el-form-item>
      <el-form-item v-if="!isSqlite" :label="t('form.port')" prop="port">
        <el-input-number v-model="form.port" :min="1" :max="65535" controls-position="right" style="width: 100%" />
      </el-form-item>
      <el-form-item v-if="!isSqlite" :label="t('form.username')" prop="username">
        <el-input
          v-model="form.username"
          :placeholder="usernamePlaceholder"
        />
      </el-form-item>
      <el-form-item v-if="!isSqlite" :label="t('form.password')" prop="password">
        <el-input
          v-model="form.password"
          type="password"
          show-password
          autocomplete="new-password"
          :placeholder="passwordPlaceholder"
        />
      </el-form-item>
      <el-form-item v-if="isSqlite" :label="t('form.filePath')" prop="filePath">
        <div class="file-row">
          <el-input
            :model-value="sqliteFile"
            :placeholder="t('form.filePathPlaceholder')"
            @update:model-value="(v: string) => (sqliteFile = v)"
          />
          <el-button @click="browseSqliteFile">{{ t('form.browse') }}</el-button>
        </div>
      </el-form-item>
      <el-form-item v-if="!isSqlite" :label="databaseLabel" prop="database">
        <el-input v-model="form.database" :placeholder="databasePlaceholder" />
      </el-form-item>
      <el-form-item v-if="isJdbc" :label="t('form.jdbcDriver')">
        <el-select v-model="jdbcDriverId" style="width: 100%" :placeholder="t('form.jdbcDriverPlaceholder')">
          <el-option v-for="d in jdbcDrivers" :key="d.id" :value="d.id" :label="d.name" />
        </el-select>
      </el-form-item>
      <el-form-item v-if="isOracle" :label="t('form.oracleDriver')">
        <el-select v-model="driverMode" style="width: 100%">
          <el-option
            value="builtin"
            :label="t('form.driverBuiltin')"
            :disabled="!bridgeAvailable"
          />
          <el-option value="oci" :label="t('form.driverOci')" />
        </el-select>
        <div class="driver-hint">
          {{ bridgeAvailable ? t('form.driverBuiltinHint') : t('form.driverBuiltinUnavailable') }}
        </div>
      </el-form-item>
      <el-form-item :label="t('form.group')">
        <el-select v-model="form.groupId" clearable style="width: 100%">
          <el-option value="" :label="t('tree.groupNone')" />
          <el-option
            v-for="g in connStore.groups"
            :key="g.id"
            :value="g.id"
            :label="g.name"
          />
        </el-select>
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button @click="emit('update:modelValue', false)">{{ t('common.cancel') }}</el-button>
      <el-button :loading="testing" @click="onTest">{{ t('form.testBtn') }}</el-button>
      <el-button type="primary" @click="onSave">{{ t('common.save') }}</el-button>
    </template>
  </el-dialog>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { objectIcon } from '../utils/objectIcons'
import { ElMessage } from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import { open } from '@tauri-apps/plugin-dialog'
import { dbTestConnection, bridgeStatus, jdbcDriversList } from '../api/db'
import { describeConnError } from '../utils/connError'
import { DB_DEFAULT_PORTS, DB_KIND_LABELS } from '../types/db'
import type { ConnectionConfig, DbKind, JdbcDriver, KindInfo } from '../types/db'
import { dbRegistry } from '../api/db'
import { useConnectionStore } from '../stores/connections'

const { t } = useI18n()
const connStore = useConnectionStore()

const props = defineProps<{
  modelValue: boolean
  initial: ConnectionConfig | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'save', cfg: ConnectionConfig): void
}>()

const formRef = ref<FormInstance>()

/** 类型注册表（后端能力声明驱动，新增数据库前端零改动） */
const registry = ref<KindInfo[]>([])

void dbRegistry()
  .then((list) => {
    registry.value = list
  })
  .catch(() => {
    // 注册表加载失败时回退内置列表
    registry.value = (
      Object.keys(DB_KIND_LABELS) as DbKind[]
    ).map((kind) => ({
      kind,
      label: t('dbKind.' + kind),
      defaultPort: DB_DEFAULT_PORTS[kind],
      transport: '',
      available: true,
      experimental: false,
      usernameHint: null,
      formatLanguage: 'sql',
      explain: false,
      exportDdl: false,
      exportInsert: false,
      transactions: false,
      sessions: false,
      objectTypes: [],
    }))
  })

const allKinds = computed(() =>
  registry.value.map((k) => {
    const base = `${t('dbKind.' + k.kind)}${k.experimental ? t('form.experimental') : ''}`
    return {
      value: k.kind,
      label: k.available ? base : `${base}${t('form.disabledHint')}`,
      disabled: !k.available,
      usernameHint: k.usernameHint,
      defaultPort: k.defaultPort,
    }
  }),
)

const form = ref<ConnectionConfig>(emptyForm())

function emptyForm(): ConnectionConfig {
  return {
    id: crypto.randomUUID(),
    name: '',
    kind: 'mysql',
    host: '127.0.0.1',
    port: 3306,
    username: '',
    password: '',
    database: '',
    groupId: null,
  }
}

const rules = computed<FormRules>(() => ({
  name: [{ required: true, message: t('form.nameRequired'), trigger: 'blur' }],
  host: [{ required: !isSqlite.value, message: t('form.hostRequired'), trigger: 'blur' }],
  username: [{ required: !isSqlite.value, message: t('form.usernameRequired'), trigger: 'blur' }],
  port: [
    {
      validator: (_r: unknown, v: unknown, cb: (e?: Error) => void) => {
        if (v === null || v === undefined || v === '') return cb()
        const n = Number(v)
        if (!Number.isInteger(n) || n < 1 || n > 65535) return cb(new Error(t('form.portInvalid')))
        cb()
      },
      trigger: 'blur',
    },
  ],
}))

const databaseLabel = computed(() => {
  if (form.value.kind === 'oracle') return t('form.serviceName')
  if (form.value.kind === 'oceanbase-oracle') return t('form.schema')
  return t('form.database')
})

const databasePlaceholder = computed(() => {
  if (form.value.kind === 'oracle') return 'ORCLPDB1'
  if (form.value.kind === 'oceanbase-oracle') return t('form.obSchemaPlaceholder')
  if (form.value.kind === 'postgresql' || form.value.kind === 'gaussdb') return '默认 postgres'
  return ''
})

const usernamePlaceholder = computed(() => {
  if (form.value.kind === 'oceanbase') return t('form.obUsername')
  if (form.value.kind === 'oceanbase-oracle') return t('form.obOracleUsername')
  return ''
})

/** 已存凭据时提示留空保持不变 */
const passwordPlaceholder = computed(() =>
  props.initial?.passwordRef && !form.value.password ? t('form.passwordStored') : '',
)

const isOracle = computed(
  () => form.value.kind === 'oracle' || form.value.kind === 'oceanbase-oracle',
)

/** SQLite：连接 = 本地数据库文件（路径存 params.file） */
const isSqlite = computed(() => form.value.kind === 'sqlite')
const sqliteFile = computed({
  get: () => form.value.params?.file ?? '',
  set: (v: string) => {
    form.value.params = { ...(form.value.params ?? {}), file: v }
  },
})

async function browseSqliteFile(): Promise<void> {
  try {
    const picked = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'SQLite', extensions: ['db', 'sqlite', 'sqlite3', 'db3'] }],
    })
    if (typeof picked === 'string' && picked) {
      sqliteFile.value = picked
      // 文件名作为默认连接名（新建时更友好）
      if (!form.value.name) {
        form.value.name = picked.split(/[\\/]/).pop()?.replace(/\.(db|sqlite|sqlite3|db3)$/i, '') ?? ''
      }
    }
  } catch {
    // 用户取消选择
  }
}

/** 内置桥接驱动可用状态（Oracle: go-ora / OceanBase Oracle: Connector/J） */
const bridges = ref({ oracleGo: false, obJdbc: false })
void bridgeStatus()
  .then((s) => {
    bridges.value = s
  })
  .catch(() => {
    bridges.value = { oracleGo: false, obJdbc: false }
  })

const bridgeAvailable = computed(() =>
  form.value.kind === 'oracle' ? bridges.value.oracleGo : bridges.value.obJdbc,
)

/** 通用 JDBC：驱动选择 */
const isJdbc = computed(() => form.value.kind === 'jdbc')
const jdbcDrivers = ref<JdbcDriver[]>([])
void jdbcDriversList()
  .then((list) => {
    jdbcDrivers.value = list
  })
  .catch(() => {
    jdbcDrivers.value = []
  })
const jdbcDriverId = computed({
  get: () => form.value.params?.jdbcDriverId ?? '',
  set: (v: string) => {
    form.value.params = { ...(form.value.params ?? {}), jdbcDriverId: v }
    const d = jdbcDrivers.value.find((x) => x.id === v)
    if (d?.defaultPort) form.value.port = d.defaultPort
  },
})

/** Oracle 连接方式：builtin（内置驱动）/ oci（OCI 客户端） */
const driverMode = computed({
  get: () => form.value.params?.driver ?? 'builtin',
  set: (v: string) => {
    form.value.params = { ...(form.value.params ?? {}), driver: v }
  },
})

function onOpen(): void {
  form.value = props.initial ? { ...props.initial } : emptyForm()
}

function onKindChange(kind: DbKind): void {
  const meta = allKinds.value.find((k) => k.value === kind)
  form.value.port = meta?.defaultPort ?? DB_DEFAULT_PORTS[kind]
}

const testing = ref(false)

async function onTest(): Promise<void> {
  testing.value = true
  try {
    const info = await dbTestConnection(form.value)
    ElMessage.success(t('common.connectionSuccess', { version: info.serverVersion }))
  } catch (e) {
    ElMessage.error(t('common.connectionFailed', { msg: describeConnError(e) }))
  } finally {
    testing.value = false
  }
}

async function onSave(): Promise<void> {
  const valid = await formRef.value?.validate().catch(() => false)
  if (!valid) return
  if (isSqlite.value && !sqliteFile.value.trim()) {
    ElMessage.warning(t('form.filePathRequired'))
    return
  }
  const params = { ...(form.value.params ?? {}) }
  if (!isOracle.value) {
    delete params.driver
  }
  if (!isJdbc.value) {
    delete params.jdbcDriverId
  } else if (!params.jdbcDriverId) {
    ElMessage.warning(t('form.jdbcDriverRequired'))
    return
  }
  emit('save', {
    ...form.value,
    database: form.value.database || undefined,
    params: Object.keys(params).length > 0 ? params : undefined,
  })
}
</script>

<style scoped>
.driver-hint {
  margin-top: 4px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
  line-height: 1.4;
}

.file-row {
  display: flex;
  gap: 8px;
  width: 100%;
}
</style>

<template>
  <div class="conn-tree" data-tour="tree" @click="hideMenu">
    <div class="tree-toolbar">
      <el-tooltip :content="t('tree.newGroup')" placement="top">
        <el-button :icon="FolderAdd" size="small" text @click="onCreateGroup" />
      </el-tooltip>
      <el-tooltip :content="t('tree.newConnection')" placement="top">
        <el-button :icon="Plus" size="small" text @click="emit('newConnection')" />
      </el-tooltip>
      <el-tooltip :content="t('tree.expandLoaded')" placement="top">
        <el-button :icon="Expand" size="small" text @click="expandLoaded" />
      </el-tooltip>
      <el-tooltip :content="t('tree.collapseAll')" placement="top">
        <el-button :icon="Fold" size="small" text @click="collapseAll" />
      </el-tooltip>
      <el-tooltip :content="t('common.refresh')" placement="top">
        <el-button :icon="Refresh" size="small" text @click="refresh" />
      </el-tooltip>
    </div>

    <div class="tree-search">
      <el-input
        v-model="filterText"
        size="small"
        clearable
        :prefix-icon="Search"
        :placeholder="t('tree.searchPlaceholder')"
      />
    </div>

    <div v-if="connStore.saved.length === 0" class="tree-empty">
      <EmptyState icon="obj_connection" :title="t('tree.empty')">
        <template #action>
          <el-button type="primary" size="small" :icon="Plus" @click="emit('newConnection')">
            {{ t('tree.newConnection') }}
          </el-button>
        </template>
      </EmptyState>
    </div>

<el-tree
      v-else
      ref="treeRef"
      :key="treeKey"
      lazy
      draggable
      :allow-drag="allowDrag"
      :allow-drop="allowDrop"
      :load="loadNode"
      :props="treeProps"
      :filter-node-method="filterNode"
      node-key="key"
      highlight-current
      :expand-on-click-node="false"
      class="tree-body"
      @node-click="onNodeClick"
      @node-contextmenu="onContextMenu"
      @node-drop="onNodeDrop"
      @node-expand="saveExpandedDebounced"
      @node-collapse="saveExpandedDebounced"
    >
      <template #default="{ data }">
        <span
          class="tree-node"
          :class="{ 'is-active-conn': data.nodeType === 'conn' && connStore.isConnected(data.connectionId) }"
          @dblclick.stop="onNodeDblClick(data as TreeTarget)"
        >
          <DbLogo
            v-if="data.nodeType === 'conn'"
            :kind="data.kind as DbKind"
            :size="15"
            :title="t('dbKind.' + data.kind)"
          />
          <el-icon
            v-else
            class="node-icon"
            :class="'icon-' + nodeIconKey(data)"
          >
            <component :is="objectIcon(nodeIconKey(data))" />
          </el-icon>
          <span class="node-label" :title="data.comment || data.label">{{ data.label }}</span>
          <el-icon
            v-if="data.nodeType === 'conn' && connStore.isConnecting(data.connectionId)"
            class="node-loading is-loading"
          >
            <Loading />
          </el-icon>
          <span
            v-else-if="data.nodeType === 'conn'"
            class="node-state"
            :class="stateClass(data.connectionId)"
          />
          <span
            v-if="data.nodeType === 'object'"
            class="node-view-tag"
          >
            {{ t('objectType.' + data.objType) }}
          </span>
          <span
            v-if="data.nodeType === 'object' && data.status && data.status !== 'VALID'"
            class="node-status"
            :class="{ 'is-invalid': data.status === 'INVALID' }"
            :title="t('objectStatus.' + data.status.toLowerCase())"
          >
            {{ t('objectStatus.' + data.status.toLowerCase()) }}
          </span>
        </span>
      </template>
    </el-tree>

    <!-- 节点右键菜单（公共 ContextMenu 渲染） -->
    <ContextMenu
      v-model:visible="menu.visible"
      :x="menu.x"
      :y="menu.y"
      :items="menuItems"
      @select="onMenuSelect"
    />

    <ExportDialog
      v-if="exportTarget"
      v-model="exportVisible"
      :connection-id="exportTarget.connectionId"
      :kind="exportTarget.kind"
      :database="exportTarget.database"
      :table="exportTarget.table"
    />

    <!-- 移动到文件夹（多级缩进展示） -->
    <el-dialog v-model="moveVisible" :title="t('tree.moveToGroup')" width="380px">
      <el-select v-model="moveGroupId" style="width: 100%">
        <el-option value="" :label="t('tree.groupNone')" />
        <el-option
          v-for="g in groupOptions"
          :key="g.id"
          :value="g.id"
          :label="g.label"
        />
      </el-select>
      <template #footer>
        <el-button @click="moveVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button type="primary" @click="confirmMove">{{ t('common.confirm') }}</el-button>
      </template>
    </el-dialog>

    <!-- 删除文件夹：策略选择 -->
    <el-dialog v-model="deleteVisible" :title="t('tree.deleteGroup')" width="460px">
      <p v-if="deleteTarget" class="del-title">
        {{ t('tree.deleteGroupConfirm', { name: deleteTarget.name }) }}
      </p>
      <el-radio-group v-model="deleteStrategy" class="del-group">
        <el-radio value="promote">{{ t('tree.delStrategyPromote') }}</el-radio>
        <el-radio value="flatten">{{ t('tree.delStrategyFlatten') }}</el-radio>
        <el-radio value="purge">
          <span class="del-danger">{{ t('tree.delStrategyPurge') }}</span>
        </el-radio>
      </el-radio-group>
      <div class="del-stats">
        {{ t('tree.delStats', { folders: deleteStats.folders, connections: deleteStats.connections }) }}
      </div>
      <template #footer>
        <el-button @click="deleteVisible = false">{{ t('common.cancel') }}</el-button>
        <el-button
          :type="deleteStrategy === 'purge' ? 'danger' : 'primary'"
          @click="confirmDeleteGroup"
        >
          {{ t('tree.deleteGroup') }}
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  Connection,
  CopyDocument,
  Delete,
  Document,
  Download,
  EditPen,
  Expand,
  Fold,
  Folder,
  FolderAdd,
  Grid,
  Loading,
  Plus,
  Refresh,
  Search,
  SwitchButton,
  Tickets,
} from '@element-plus/icons-vue'
import DbLogo from './DbLogo.vue'
import EmptyState from './EmptyState.vue'
import ExportDialog from './ExportDialog.vue'
import ContextMenu, { type CtxMenuItem } from './ContextMenu.vue'
import { objectIcon } from '../utils/objectIcons'
import { describeConnError } from '../utils/connError'
import { dispatchCommand } from '../utils/commands'
import { useI18n } from 'vue-i18n'
import { useConnectionStore } from '../stores/connections'
import { useTabsStore } from '../stores/tabs'
import {
  dbFetchTableDdl,
  dbListObjectFolders,
  dbListObjects,
  dbRecompileObject,
  dbTestConnection,
} from '../api/db'
import type { ConnectionConfig, ConnectionGroup, DbKind, DbObjectType } from '../types/db'

const { t } = useI18n()

type TreeTarget =
  | { nodeType: 'group'; key: string; label: string; groupId: string }
  | { nodeType: 'conn'; key: string; label: string; connectionId: string; kind: DbKind }
  | { nodeType: 'db'; key: string; label: string; connectionId: string; kind: DbKind }
  | {
      nodeType: 'folder'
      key: string
      label: string
      connectionId: string
      kind: DbKind
      database: string
      objType: DbObjectType
    }
  | {
      nodeType: 'table'
      key: string
      label: string
      connectionId: string
      kind: DbKind
      database: string
      typeLabel: string
    }
  | {
      nodeType: 'object'
      key: string
      label: string
      connectionId: string
      kind: DbKind
      database: string
      objType: DbObjectType
      comment?: string | null
      /** 对象状态（Oracle：VALID/INVALID/UNUSABLE） */
      status?: string | null
    }

const emit = defineEmits<{
  (e: 'newConnection'): void
  (e: 'edit', cfg: ConnectionConfig): void
}>()

const connStore = useConnectionStore()
const tabsStore = useTabsStore()

const treeKey = ref(0)
const treeRef = ref()
const filterText = ref('')

// 搜索过滤（作用于已加载节点）
watch(filterText, (v) => {
  treeRef.value?.filter(v)
  // 搜索时自动展开「已加载」的父链，让已浏览过的库/表参与匹配；
  // 未加载的懒加载节点保持折叠（不触发隐式连接，配合 loadNode 展开不连接的约定）
  if (v) expandLoadedAll()
})

function expandLoadedAll(): void {
  type N = { loaded?: boolean; expanded?: boolean; expand?: (v?: boolean) => void; childNodes?: N[] }
  const root = (treeRef.value as unknown as { store?: { root?: N } } | null)?.store?.root
  const walk = (n?: N): void => {
    if (!n) return
    for (const child of n.childNodes ?? []) {
      if (child.loaded && child.childNodes?.length && !child.expanded) {
        child.expand?.(true)
      }
      walk(child)
    }
  }
  walk(root)
}

function filterNode(value: string, data: TreeTarget): boolean {
  if (!value) return true
  const q = value.toLowerCase()
  const comment = (data as { comment?: string | null }).comment ?? ''
  return data.label.toLowerCase().includes(q) || comment.toLowerCase().includes(q)
}

const treeProps = {
  label: 'label',
  // 叶节点：表/视图与其余对象（过程/函数/触发器/索引/序列）均不可再展开
  isLeaf: (data: TreeTarget) => data.nodeType === 'table' || data.nodeType === 'object',
}

const menu = reactive<{
  visible: boolean
  x: number
  y: number
  target: TreeTarget | null
}>({ visible: false, x: 0, y: 0, target: null })

/** 节点右键菜单项（公共 ContextMenu 渲染；按节点类型动态成组，图标沿用既有语义） */
const menuItems = computed<CtxMenuItem[]>(() => {
  const tgt = menu.target
  if (!tgt) return []
  const items: CtxMenuItem[] = []
  const push = (key: string, label: string, icon: CtxMenuItem['icon']): void => {
    items.push({ key, label, icon })
  }
  const sep = (): void => {
    items.push({ key: `sep-${items.length}`, label: '', sep: true })
  }
  if (tgt.nodeType === 'group') {
    push('newConnInGroup', t('tree.newConnection'), Plus)
    push('renameGroup', t('tree.renameGroup'), EditPen)
    push('newSubGroup', t('tree.newSubGroup'), FolderAdd)
    sep()
    items.push({ key: 'deleteGroup', label: t('tree.deleteGroup'), icon: Delete, danger: true })
  } else if (tgt.nodeType === 'conn') {
    if (!connStore.isConnected(tgt.connectionId)) push('connect', t('tree.connect'), Connection)
    push('newQuery', t('tree.newQuery'), Document)
    push('moveToGroup', t('tree.moveToGroup'), Folder)
    push('edit', t('tree.editConnection'), EditPen)
    push('test', t('tree.testConnection'), Connection)
    push('sessions', t('session.tabTitle'), objectIcon('session'))
    if (connStore.isConnected(tgt.connectionId)) push('disconnect', t('tree.disconnect'), SwitchButton)
    sep()
    items.push({ key: 'delete', label: t('tree.deleteConnection'), icon: Delete, danger: true })
  } else if (tgt.nodeType === 'table') {
    push('queryRows', t('tree.queryRows'), Tickets)
    push('tableMeta', t('tree.viewStructure'), Grid)
    push('tableSource', t('tree.viewSource'), Document)
    push('copyDdl', t('tree.copyDdl'), CopyDocument)
    sep()
    push('export', t('tree.exportData'), Download)
  } else if (tgt.nodeType === 'db') {
    push('newQuery', t('tree.newQuery'), Document)
    push('createTable', t('tree.newTable'), Plus)
  } else if (tgt.nodeType === 'folder') {
    push('newQuery', t('tree.newQuery'), Document)
    if (tgt.objType === 'table') push('createTable', t('tree.newTable'), Plus)
  } else if (tgt.nodeType === 'object') {
    const isRoutine = tgt.objType === 'function' || tgt.objType === 'procedure'
    push('objectDdl', isRoutine ? t('tree.editRoutine') : t('tree.viewDefinition'), Document)
    if (tgt.kind === 'oracle' || tgt.kind === 'oceanbase-oracle') push('recompile', t('tree.recompile'), Refresh)
  }
  if (isExpandable(tgt)) {
    sep()
    push('expandNode', t('tree.expandNode'), Expand)
    push('collapseNode', t('tree.collapseNode'), Fold)
  }
  return items
})

function onMenuSelect(key: string): void {
  switch (key) {
    case 'newConnInGroup':
      onMenuNewConnInGroup()
      break
    case 'renameGroup':
      void onMenuRenameGroup()
      break
    case 'newSubGroup':
      void onMenuNewSubGroup()
      break
    case 'deleteGroup':
      onMenuDeleteGroup()
      break
    case 'connect':
      void onMenuConnect()
      break
    case 'newQuery':
      void onMenuNewQuery()
      break
    case 'moveToGroup':
      onMenuMoveToGroup()
      break
    case 'edit':
      onMenuEdit()
      break
    case 'test':
      void onMenuTest()
      break
    case 'sessions':
      void onMenuSessions()
      break
    case 'disconnect':
      void onMenuDisconnect()
      break
    case 'delete':
      void onMenuDelete()
      break
    case 'queryRows':
      onMenuQueryRows()
      break
    case 'tableMeta':
      onMenuTableMeta()
      break
    case 'tableSource':
      onMenuTableSource()
      break
    case 'copyDdl':
      void onMenuCopyDdl()
      break
    case 'export':
      onMenuExport()
      break
    case 'createTable':
      onMenuCreateTable()
      break
    case 'objectDdl':
      onMenuObjectDdl()
      break
    case 'recompile':
      void onMenuRecompile()
      break
    case 'expandNode':
      onMenuExpandNode()
      break
    case 'collapseNode':
      onMenuCollapseNode()
      break
  }
}

const exportVisible = ref(false)
const exportTarget = ref<{
  connectionId: string
  kind: DbKind
  database: string
  table: string
} | null>(null)

/** 移动连接到文件夹对话框 */
const moveVisible = ref(false)
const moveTarget = ref<ConnectionConfig | null>(null)
const moveGroupId = ref<string>('')

function refresh(): void {
  // 重建前记录已展开节点，重建后恢复，避免刷新导致展开状态丢失
  const keys = topLevelExpandedKeys()
  treeKey.value += 1
  if (keys.length > 0) {
    void (async () => {
      await nextTick()
      await nextTick()
      keys.forEach((k) => expandNode(k))
    })()
  }
}

/** —— 展开 / 收起（Element Plus Node：expand()/collapse()） —— */
interface TreeNodeLike2 {
  expand: (callback?: () => void, expandParent?: boolean) => void
  collapse: () => void
  isLeaf?: boolean
  loaded?: boolean
}

function allNodes(): TreeNodeLike2[] {
  const store = treeRef.value?.store as
    | { nodesMap: Record<string, TreeNodeLike2> }
    | undefined
  return store ? Object.values(store.nodesMap) : []
}

/**
 * 展开「已加载」节点：仅展开子节点已加载的非叶节点。
 * 关键：对 `loaded === false` 的懒加载节点展开会触发 loadNode
 * （对连接节点即发起连接、对库节点即查询对象），因此必须跳过。
 */
function expandLoaded(): void {
  allNodes().forEach((n) => {
    if (n.isLeaf) return
    if (n.loaded === false) return
    n.expand()
  })
}

function collapseAll(): void {
  allNodes().forEach((n) => n.collapse())
}

function expandNode(key: string): void {
  const n = treeRef.value?.getNode(key) as TreeNodeLike2 | undefined
  // expandParent=true：恢复/定位时逐级展开祖先，确保可见
  n?.expand(undefined, true)
}

function collapseNode(key: string): void {
  ;(treeRef.value?.getNode(key) as TreeNodeLike2 | undefined)?.collapse()
}

// 窗体打开时自动加载已保存的连接配置（加载完成后重建树，无需手动刷新）
onMounted(async () => {
  await connStore.load().catch(() => undefined)
  refresh()
  // 恢复上次的展开状态（布局切换/重启后保持）
  await nextTick()
  await nextTick()
  restoreExpanded()
})

onBeforeUnmount(() => {
  saveExpanded()
})

/** 展开状态持久化（跨布局切换/重启保留） */
const EXPANDED_KEY = 'personaldb.treeExpanded'
let saveTimer: number | undefined

function saveExpanded(): void {
  try {
    const keys = topLevelExpandedKeys()
    localStorage.setItem(EXPANDED_KEY, JSON.stringify(keys))
  } catch {
    // ignore
  }
}

function saveExpandedDebounced(): void {
  window.clearTimeout(saveTimer)
  saveTimer = window.setTimeout(saveExpanded, 300)
}

function restoreExpanded(): void {
  try {
    const raw = localStorage.getItem(EXPANDED_KEY)
    if (!raw) return
    const keys = JSON.parse(raw) as string[]
    keys.forEach((k) => expandNode(k))
  } catch {
    // ignore
  }
}

defineExpose({ refresh })

function stateClass(connectionId: string): string {
  return connStore.isConnected(connectionId) ? 'state-connected' : 'state-disconnected'
}

/** 连接配置 → 树节点 */
function toConnNode(c: ConnectionConfig): TreeTarget {
  return {
    nodeType: 'conn',
    key: `conn:${c.id}`,
    label: c.name,
    connectionId: c.id,
    kind: c.kind,
  }
}

/** 文件夹 → 树节点 */
function toGroupNode(g: ConnectionGroup): TreeTarget {
  return {
    nodeType: 'group',
    key: `group:${g.id}`,
    label: g.name,
    groupId: g.id,
  }
}

async function loadNode(
  node: unknown,
  resolve: (data: TreeTarget[]) => void,
): Promise<void> {
  const n = node as { level: number; data?: TreeTarget }
  try {
    // 根：文件夹 + 未分组连接
    if (n.level === 0) {
      const groupNodes: TreeTarget[] = connStore.groupChildren(null).map(toGroupNode)
      const ungrouped = connStore.saved.filter((c) => !c.groupId).map(toConnNode)
      resolve([...groupNodes, ...ungrouped])
      return
    }
    const data = n.data
    if (!data) {
      resolve([])
      return
    }
    // 展开文件夹 → 子文件夹 + 组内连接（多级）
    if (data.nodeType === 'group') {
      const subGroups = connStore.groupChildren(data.groupId).map(toGroupNode)
      const conns = connStore.saved.filter((c) => c.groupId === data.groupId).map(toConnNode)
      resolve([...subGroups, ...conns])
      return
    }
    // 展开连接节点 → 自动连接并列出库
    if (data.nodeType === 'conn') {
      const cfg = connStore.configOf(data.connectionId)
      if (!cfg) {
        resolve([])
        return
      }
      // 展开不作为连接触发器：未连接的连接节点展开为空（双击节点或右键「连接」建立会话），
      // 避免断开后树刷新/启动恢复展开状态时发生隐式重连
      if (!connStore.sessionOf(data.connectionId)) {
        resolve([])
        return
      }
      const dbs = await connStore.listDatabases(data.connectionId)
      resolve(
        dbs.map((d) => ({
          nodeType: 'db' as const,
          key: `db:${data.connectionId}:${d}`,
          label: d,
          connectionId: data.connectionId,
          kind: data.kind,
        })),
      )
      return
    }
    const session = connStore.sessionOf(data.connectionId)
    if (!session) {
      resolve([])
      return
    }
    // 展开库节点 → 对象类型文件夹（数量 > 0）
    if (data.nodeType === 'db') {
      const dbNode = data
      const folders = await dbListObjectFolders(session.sessionId, dbNode.label)
      resolve(
        folders.map((f) => ({
          nodeType: 'folder' as const,
          key: `folder:${dbNode.connectionId}:${dbNode.label}:${f.objectType}`,
          label: `${t("objectType." + f.objectType)} (${f.count})`,
          connectionId: dbNode.connectionId,
          kind: dbNode.kind,
          database: dbNode.label,
          objType: f.objectType,
        })),
      )
      return
    }
    // 展开文件夹 → 对象实例
    if (data.nodeType === 'folder') {
      const folder = data
      const objects = await dbListObjects(session.sessionId, folder.database, folder.objType)
      if (folder.objType === 'table' || folder.objType === 'view') {
        resolve(
          objects.map((o) => ({
            nodeType: 'table' as const,
            key: `tbl:${folder.connectionId}:${folder.database}:${o.name}`,
            label: o.name,
            connectionId: folder.connectionId,
            kind: folder.kind,
            database: folder.database,
            typeLabel: folder.objType === 'view' ? 'VIEW' : 'TABLE',
          })),
        )
      } else {
        resolve(
          objects.map((o) => ({
            nodeType: 'object' as const,
            key: `obj:${folder.connectionId}:${folder.database}:${folder.objType}:${o.name}`,
            label: o.name,
            connectionId: folder.connectionId,
            kind: folder.kind,
            database: folder.database,
            objType: folder.objType,
            comment: o.comment,
            status: o.status,
          })),
        )
      }
      return
    }
    resolve([])
  } catch (e) {
    ElMessage.error(t('common.loadFailed', { msg: String(e) }))
    resolve([])
  }
}

function onNodeClick(_data: TreeTarget): void {
  // 单击仅选中节点（不执行任何操作）
}

/** 双击表/视图：打开 SQL 编辑器并生成预览 SQL（不自动执行） */
function onNodeDblClick(data: TreeTarget): void {
  // 双击连接节点 = 连接并加载库列表
  if (data.nodeType === 'conn') {
    void connectAndReload(data)
    return
  }
  if (data.nodeType !== 'table') return
  void tabsStore.openTableQuery({
    connectionId: data.connectionId,
    connectionName: connStore.configOf(data.connectionId)?.name ?? '',
    kind: data.kind,
    database: data.database,
    table: data.label,
    typeLabel: data.typeLabel,
  })
}

/** 连接（若未连接）并重载该节点子级（保持展开） */
async function connectAndReload(data: Extract<TreeTarget, { nodeType: 'conn' }>): Promise<void> {
  const cfg = connStore.configOf(data.connectionId)
  if (!cfg) return
  if (!connStore.isConnected(data.connectionId)) {
    try {
      await connStore.openConnection(cfg)
    } catch (e) {
      ElMessage.error(t('common.connectionFailed', { msg: describeConnError(e) }))
      return
    }
  }
  const node = treeRef.value?.getNode?.(data.key)
  if (node) {
    node.loaded = false
    if (node.expanded) {
      node.collapse()
      node.expand()
    } else {
      node.expand()
    }
  }
}

/** 树节点图标 key（objectIcons 映射，表/视图/文件夹/对象统一） */
function nodeIconKey(data: TreeTarget): string {
  switch (data.nodeType) {
    case 'db':
      return 'database'
    case 'group':
    case 'folder':
      return data.nodeType === 'group' ? 'folder' : data.objType
    case 'table':
      return data.typeLabel === 'VIEW' ? 'view' : 'table'
    case 'object':
      return data.objType
    default:
      return 'connection'
  }
}

/** —— 文件夹管理 —— */
async function onCreateGroup(): Promise<void> {
  try {
    const { value } = await ElMessageBox.prompt(t('tree.groupName'), t('tree.newGroup'), {
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
      inputPattern: /\S+/,
      inputErrorMessage: t('tree.groupNameRequired'),
    })
    await connStore.createGroup(value.trim())
    refresh()
  } catch {
    // 取消
  }
}

function onMenuNewConnInGroup(): void {
  hideMenu()
  emit('newConnection')
}

async function onMenuRenameGroup(): Promise<void> {
  const target = menu.target
  hideMenu()
  if (target?.nodeType !== 'group') return
  try {
    const { value } = await ElMessageBox.prompt(t('tree.groupName'), t('tree.renameGroup'), {
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
      inputValue: target.label,
      inputPattern: /\S+/,
      inputErrorMessage: t('tree.groupNameRequired'),
    })
    await connStore.renameGroup(target.groupId, value.trim())
    refresh()
  } catch {
    // 取消
  }
}

/** 文件夹选项（深度优先 + 缩进，供移动对话框多级展示） */
const groupOptions = computed(() => {
  const out: { id: string; label: string }[] = []
  const walk = (parentId: string | null, depth: number): void => {
    for (const g of connStore.groupChildren(parentId)) {
      out.push({ id: g.id, label: `${'　'.repeat(depth)}${depth > 0 ? '└ ' : ''}${g.name}` })
      walk(g.id, depth + 1)
    }
  }
  walk(null, 0)
  return out
})

/** 新建子文件夹 */
async function onMenuNewSubGroup(): Promise<void> {
  const target = menu.target
  hideMenu()
  if (target?.nodeType !== 'group') return
  try {
    const { value } = await ElMessageBox.prompt(t('tree.groupName'), t('tree.newSubGroup'), {
      confirmButtonText: t('common.confirm'),
      cancelButtonText: t('common.cancel'),
      inputPattern: /\S+/,
      inputErrorMessage: t('tree.groupNameRequired'),
    })
    await connStore.createGroup(value.trim(), target.groupId)
    ElMessage.success(t('tree.groupCreated'))
    await refreshPreservingExpanded()
  } catch {
    // 取消
  }
}

/** 删除文件夹：打开策略选择对话框 */
function onMenuDeleteGroup(): void {
  const target = menu.target
  hideMenu()
  if (target?.nodeType !== 'group') return
  deleteTarget.value = { id: target.groupId, name: target.label }
  deleteStrategy.value = 'promote'
  deleteVisible.value = true
}

const deleteVisible = ref(false)
const deleteTarget = ref<{ id: string; name: string } | null>(null)
const deleteStrategy = ref<'promote' | 'flatten' | 'purge'>('promote')
const deleteStats = computed(() =>
  deleteTarget.value ? connStore.groupDeleteStats(deleteTarget.value.id) : { folders: 0, connections: 0 },
)

async function confirmDeleteGroup(): Promise<void> {
  if (!deleteTarget.value) return
  const strategy = deleteStrategy.value
  if (strategy === 'purge') {
    const ok = await ElMessageBox.confirm(
      t('tree.purgeConfirm', { n: deleteStats.value.connections }),
      t('tree.deleteConfirmTitle'),
      { type: 'warning', confirmButtonText: t('tree.deleteGroup'), cancelButtonText: t('common.cancel') },
    ).then(() => true).catch(() => false)
    if (!ok) return
  }
  const targetId = deleteTarget.value.id
  deleteVisible.value = false
  await connStore.removeGroup(targetId, strategy)
  ElMessage.success(t('tree.groupDeleted'))
  refresh()
}

function onMenuMoveToGroup(): void {
  const target = menu.target
  hideMenu()
  if (target?.nodeType !== 'conn') return
  const cfg = connStore.configOf(target.connectionId)
  if (!cfg) return
  moveTarget.value = cfg
  moveGroupId.value = cfg.groupId ?? ''
  moveVisible.value = true
}

/** —— 拖拽移动（连接/文件夹）—— */

interface TreeNodeLike {
  data?: TreeTarget
  key?: string
  /** el-tree 懒加载节点：展开并触发子节点加载 */
  expand?: (cb?: () => void) => void
  expanded?: boolean
}

/**
 * 拖拽源：**仅连接与文件夹可拖拽**。
 * 库/对象/表等节点返回 false → el-tree 在 dragstart 中 preventDefault，
 * 浏览器显示「禁止」光标（符合预期，避免拖出「看似移动、实际不落库」的假象）。
 */
function allowDrag(_node?: TreeNodeLike, data?: TreeTarget): boolean {
  const d = (data ?? _node?.data) as TreeTarget | undefined
  return d?.nodeType === 'conn' || d?.nodeType === 'group'
}

/**
 * 允许的落点：**一律放行**（纯谓词）。
 * el-tree 在 allow-drop 返回 false 时显示「禁止」光标；且拖拽悬浮阶段的任何副作用
 * （如展开懒加载节点）都会重建节点导致拖拽中断。
 * 因此这里保持恒真，**归属/层级校验全部在 node-drop 中执行**（信息完整、可提示）；
 * 非法落点（库/对象/表等层级）在 node-drop 中提示并恢复树。
 */
function allowDrop(
  _dragging?: TreeNodeLike,
  _drop?: TreeNodeLike,
  _type?: string,
): boolean {
  return true
}

/** 记录当前已展开的连接/文件夹节点 key（浅层，重建后恢复，代价可控） */
function topLevelExpandedKeys(): string[] {
  const store = treeRef.value?.store as
    | { nodesMap: Record<string, TreeNodeLike & { expanded?: boolean }> }
    | undefined
  if (!store) return []
  return Object.entries(store.nodesMap)
    .filter(
      ([key, n]) =>
        n.expanded &&
        (key.startsWith('group:') || key.startsWith('conn:') || key.startsWith('db:')),
    )
    .map(([key]) => key)
}

/** 重建树并恢复浅层展开状态（可附加额外需展开的节点 key） */
async function refreshPreservingExpanded(extraKeys: string[] = []): Promise<void> {
  const keys = [...new Set([...topLevelExpandedKeys(), ...extraKeys])]
  refresh()
  await nextTick()
  await nextTick()
  keys.forEach((k) => expandNode(k))
}

async function onNodeDrop(
  dragging: TreeNodeLike,
  drop: TreeNodeLike,
  type: 'prev' | 'inner' | 'next',
): Promise<void> {
  const drag = dragging.data
  const target = drop.data
  // 根级空白：ep-tree 传入 root 节点（level 0）；其余 data 缺失一律视为非法落点
  const isRootBlank = (drop as { level?: number }).level === 0
  // 拖拽源兜底（allowDrag 已限制为连接/文件夹）：其余一律提示并恢复
  if (drag?.nodeType !== 'group' && drag?.nodeType !== 'conn') {
    ElMessage.warning(t('tree.dropInvalid'))
    await refreshPreservingExpanded()
    return
  }
  // 非法落点（库/对象/表等层级，或无法识别的节点）：提示并恢复树
  if (!isRootBlank && (!target || (target.nodeType !== 'group' && target.nodeType !== 'conn'))) {
    ElMessage.warning(t('tree.dropInvalid'))
    await refreshPreservingExpanded()
    return
  }
  // 文件夹拖拽：多级移动/排序（inner=嵌套；prev/next=同级；连接上=进入其所属文件夹；根级=移回根）
  if (drag?.nodeType === 'group') {
    let targetGroupId: string | null
    let position: 'inner' | 'before' | 'after' = type === 'inner' ? 'inner' : type === 'prev' ? 'before' : 'after'
    if (target?.nodeType === 'group') {
      targetGroupId = target.groupId
    } else if (target?.nodeType === 'conn') {
      targetGroupId = connStore.configOf(target.connectionId)?.groupId ?? null
      position = 'inner'
    } else {
      // 根级空白
      targetGroupId = null
      position = 'inner'
    }
    const moved = await connStore.moveGroup(drag.groupId, targetGroupId, position)
    if (!moved) {
      ElMessage.warning(t('tree.groupMoveInvalid'))
    } else {
      ElMessage.success(t('tree.groupReordered'))
    }
    // 统一重建：store 为唯一事实源（懒加载/展开状态由展开键恢复）
    const extra = targetGroupId ? [`group:${targetGroupId}`] : []
    await refreshPreservingExpanded(extra)
    return
  }
  if (drag?.nodeType !== 'conn') return
  let groupId: string | null = null
  // 落在文件夹节点上 → 移入该文件夹（任意层级）
  if (target?.nodeType === 'group') {
    groupId = target.groupId
  } else if (target?.nodeType === 'conn') {
    // 落在连接同级 → 保持同组
    groupId = connStore.configOf(target.connectionId)?.groupId ?? null
  } else if (!isRootBlank) {
    // 其余（理论上已在上方拦截）：提示并恢复
    ElMessage.warning(t('tree.dropInvalid'))
    await refreshPreservingExpanded()
    return
  }
  // 根级空白 → 移出分组
  await connStore.moveToGroup(drag.connectionId, groupId)
  ElMessage.success(t('tree.movedToGroup'))
  // 统一重建（展开目标文件夹以便看到移动结果）
  await refreshPreservingExpanded(groupId ? [`group:${groupId}`] : [])
}

async function confirmMove(): Promise<void> {
  if (!moveTarget.value) return
  await connStore.moveToGroup(moveTarget.value.id, moveGroupId.value || null)
  moveVisible.value = false
  refresh()
}

function onContextMenu(event: MouseEvent, data: TreeTarget): void {
  event.preventDefault()
  menu.visible = true
  menu.x = event.clientX
  menu.y = event.clientY
  menu.target = data
}

function hideMenu(): void {
  menu.visible = false
}

async function onMenuNewQuery(): Promise<void> {
  const target = menu.target
  hideMenu()
  if (!target || target.nodeType === 'group') return
  const cfg = connStore.configOf(target.connectionId)
  if (!cfg) return
  try {
    await connStore.openConnection(cfg)
  } catch (e) {
    ElMessage.error(t('common.connectionFailed', { msg: describeConnError(e) }))
    return
  }
  tabsStore.openTab({
    connectionId: cfg.id,
    connectionName: cfg.name,
    kind: cfg.kind,
    database:
      target.nodeType === 'db'
        ? target.label
        : target.nodeType === 'folder'
          ? target.database
          : '',
  })
}

function onMenuEdit(): void {
  const target = menu.target
  hideMenu()
  if (target?.nodeType !== 'conn') return
  const cfg = connStore.configOf(target.connectionId)
  if (cfg) emit('edit', cfg)
}

async function onMenuTest(): Promise<void> {
  const target = menu.target
  hideMenu()
  if (target?.nodeType !== 'conn') return
  const cfg = connStore.configOf(target.connectionId)
  if (!cfg) return
  try {
    const info = await dbTestConnection(cfg)
    ElMessage.success(t('common.connectionSuccess', { version: info.serverVersion }))
  } catch (e) {
    ElMessage.error(t('common.connectionFailed', { msg: describeConnError(e) }))
  }
}

/** 会话管理（Tab 形式；未连接则先连接） */
async function onMenuSessions(): Promise<void> {
  const target = menu.target
  hideMenu()
  if (target?.nodeType !== 'conn') return
  const cfg = connStore.configOf(target.connectionId)
  if (!cfg) return
  if (!connStore.isConnected(cfg.id)) {
    try {
      await connStore.openConnection(cfg)
    } catch (e) {
      ElMessage.error(t('common.connectionFailed', { msg: describeConnError(e) }))
      return
    }
  }
  tabsStore.openSessionManager({
    connectionId: cfg.id,
    connectionName: cfg.name,
    kind: cfg.kind,
    database: cfg.database ?? '',
  })
}

/** 右键「连接」：建立会话并重载该节点子级 */
async function onMenuConnect(): Promise<void> {
  const target = menu.target
  hideMenu()
  if (target?.nodeType !== 'conn') return
  await connectAndReload(target)
}

async function onMenuDisconnect(): Promise<void> {
  const target = menu.target
  hideMenu()
  if (target?.nodeType !== 'conn') return
  await connStore.closeConnection(target.connectionId)
  refresh()
}

async function onMenuDelete(): Promise<void> {
  const target = menu.target
  hideMenu()
  if (target?.nodeType !== 'conn') return
  const cfg = connStore.configOf(target.connectionId)
  if (!cfg) return
  try {
    await ElMessageBox.confirm(t('tree.deleteConfirm', { name: cfg.name }), t('tree.deleteConfirmTitle'), { type: 'warning' })
  } catch {
    return
  }
  tabsStore.closeTabsOfConnection(cfg.id)
  await connStore.remove(cfg.id)
  refresh()
}

/** 右键表节点：查询数据（与双击一致——仅生成预览 SQL，不自动执行） */
function onMenuQueryRows(): void {
  const target = menu.target
  hideMenu()
  if (target?.nodeType !== 'table') return
  void tabsStore.openTableQuery({
    connectionId: target.connectionId,
    connectionName: connStore.configOf(target.connectionId)?.name ?? '',
    kind: target.kind,
    database: target.database,
    table: target.label,
    typeLabel: target.typeLabel,
  })
}

function onMenuTableMeta(): void {
  const target = menu.target
  hideMenu()
  if (target?.nodeType !== 'table') return
  clickTableInspector(target)
}

/** 打开表查看面板 Tab（列/索引/源码） */
function clickTableInspector(target: Extract<TreeTarget, { nodeType: 'table' }>): void {
  tabsStore.openTableInspector({
    connectionId: target.connectionId,
    connectionName: connStore.configOf(target.connectionId)?.name ?? '',
    kind: target.kind,
    database: target.database,
    table: target.label,
  })
}

function onMenuExport(): void {
  const target = menu.target
  hideMenu()
  if (target?.nodeType !== 'table') return
  exportTarget.value = {
    connectionId: target.connectionId,
    kind: target.kind,
    database: target.database,
    table: target.label,
  }
  exportVisible.value = true
}

/** 可展开节点类型（连接/文件夹/库及其下） */
function isExpandable(target: TreeTarget): boolean {
  return (
    target.nodeType === 'group' ||
    target.nodeType === 'conn' ||
    target.nodeType === 'db' ||
    target.nodeType === 'folder'
  )
}

function onMenuExpandNode(): void {
  const target = menu.target
  hideMenu()
  if (target) expandNode(target.key)
}

function onMenuCollapseNode(): void {
  const target = menu.target
  hideMenu()
  if (target) collapseNode(target.key)
}

/** 重新编译（仅 Oracle 编译类对象） */
async function onMenuRecompile(): Promise<void> {
  const target = menu.target
  hideMenu()
  if (target?.nodeType !== 'object') return
  const cfg = connStore.configOf(target.connectionId)
  if (cfg?.kind !== 'oracle' && cfg?.kind !== 'oceanbase-oracle') return
  const session = connStore.sessionOf(target.connectionId)
  if (!session) return
  try {
    const msg = await dbRecompileObject(
      session.sessionId,
      target.database || null,
      target.objType,
      target.label,
    )
    ElMessage.success(msg)
    refresh()
  } catch (e) {
    ElMessage.error(String(e))
  }
}

/** 对象查看 / 例程编辑均以 Tab 形式打开（见 tabs.openObjectInspector/openRoutineEditor） */

function onMenuObjectDdl(): void {
  const target = menu.target
  hideMenu()
  if (target?.nodeType !== 'object') return
  // 存储过程 / 函数 → 例程编辑 Tab
  if (target.objType === 'function' || target.objType === 'procedure') {
    tabsStore.openRoutineEditor({
      connectionId: target.connectionId,
      connectionName: connStore.configOf(target.connectionId)?.name ?? '',
      kind: target.kind,
      database: target.database,
      name: target.label,
      routineType: target.objType === 'function' ? 'FUNCTION' : 'PROCEDURE',
    })
    return
  }
  tabsStore.openObjectInspector({
    connectionId: target.connectionId,
    connectionName: connStore.configOf(target.connectionId)?.name ?? '',
    kind: target.kind,
    database: target.database,
    name: target.label,
    objType: target.objType,
  })
}

/** 表节点：查看源码（表查看面板的源码页签） */
function onMenuTableSource(): void {
  const target = menu.target
  hideMenu()
  if (target?.nodeType !== 'table') return
  const tab = tabsStore.openTableInspector({
    connectionId: target.connectionId,
    connectionName: connStore.configOf(target.connectionId)?.name ?? '',
    kind: target.kind,
    database: target.database,
    table: target.label,
  })
  // 通过全局事件切换面板内部页签到「源码」
  void dispatchCommand('table-source', tab.id)
}

/** 表节点：复制 DDL 到剪贴板 */
async function onMenuCopyDdl(): Promise<void> {
  const target = menu.target
  hideMenu()
  if (target?.nodeType !== 'table') return
  const session = connStore.sessionOf(target.connectionId)
  if (!session) return
  try {
    const ddl = await dbFetchTableDdl(session.sessionId, target.database || null, target.label)
    await navigator.clipboard.writeText(ddl)
    ElMessage.success(t('tree.ddlCopied'))
  } catch (e) {
    ElMessage.error(String(e))
  }
}

/** 库/表文件夹：新建表模板 */
function onMenuCreateTable(): void {
  const target = menu.target
  hideMenu()
  if (!target || (target.nodeType !== 'db' && target.nodeType !== 'folder')) return
  const database = target.nodeType === 'db' ? target.label : target.database
  const cfg = connStore.configOf(target.connectionId)
  if (!cfg) return
  void connStore
    .openConnection(cfg)
    .then(() => {
      tabsStore.openCreateTableTemplate({
        connectionId: cfg.id,
        connectionName: cfg.name,
        kind: cfg.kind,
        database,
      })
    })
    .catch((e) => ElMessage.error(t('common.connectionFailed', { msg: describeConnError(e) })))
}
</script>

<style scoped>
.conn-tree {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--app-sidebar-bg);
}

.tree-toolbar {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 2px;
  padding: 4px 8px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  background: var(--el-bg-color);
}

.tree-search {
  padding: 4px 8px;
  border-bottom: 1px solid var(--el-border-color-lighter);
  background: var(--el-bg-color);
}

.tree-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

.tree-body {
  flex: 1;
  overflow: auto;
  padding: 4px;
  --el-tree-node-content-height: 26px;
}

/* 当前选中节点：主色高亮（连接 / 文件夹 / 库 / 对象等） */
.tree-body :deep(.el-tree-node.is-current > .el-tree-node__content) {
  background: var(--el-color-primary-light-9);
  border-radius: 4px;
}

/* 拖拽：可拖动连接的手型提示 */
.tree-body :deep(.el-tree-node__content) {
  cursor: default;
}

.tree-body :deep(.el-tree-node__content[draggable='true']) {
  cursor: grab;
}

/* 拖拽落点：文件夹内部放置高亮 */
.tree-body :deep(.el-tree-node__content.is-drop-inner) {
  background: var(--el-color-primary-light-7) !important;
  outline: 1px dashed var(--el-color-primary);
  border-radius: 4px;
}

/* 拖拽时同级插入指示线 */
.tree-body :deep(.el-tree-node__drop-indicator) {
  background: var(--el-color-primary);
}

.tree-body :deep(.el-tree-node.is-current > .el-tree-node__content .node-label) {
  color: var(--el-color-primary);
  font-weight: 600;
}

.tree-body :deep(.el-tree-node__content:hover) {
  background: var(--el-fill-color-light);
}

/* 已连接的连接节点：绿色侧条 + 加粗 */
.tree-node.is-active-conn::before {
  content: '';
  width: 3px;
  height: 13px;
  border-radius: 2px;
  background: var(--el-color-success);
  flex-shrink: 0;
}

.tree-node.is-active-conn .node-label {
  font-weight: 600;
  color: var(--el-color-success);
}

.tree-node {
  display: flex;
  align-items: center;
  gap: 5px;
  flex: 1;
  min-width: 0;
  font-size: 13px;
  padding-right: 4px;
}

.node-icon {
  flex-shrink: 0;
  font-size: 14px;
}

.icon-conn {
  color: var(--el-color-primary);
}

.icon-database {
  color: var(--el-color-warning);
}

.icon-folder {
  color: var(--el-text-color-secondary);
}

.icon-table {
  color: var(--el-text-color-secondary);
}

.icon-view {
  color: var(--el-color-success);
}

.icon-index {
  color: var(--el-color-info);
}

.icon-sequence {
  color: var(--el-color-primary);
}

.icon-function,
.icon-procedure {
  color: #b37ceb;
}

.icon-trigger {
  color: var(--el-color-danger);
}

.node-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.node-state {
  flex-shrink: 0;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  margin-left: auto;
}

.state-connected {
  background: var(--el-color-success);
  box-shadow: 0 0 3px var(--el-color-success);
}

.state-disconnected {
  background: var(--el-border-color);
}

.node-loading {
  flex-shrink: 0;
  color: var(--el-color-primary);
  font-size: 12px;
}

.node-view-tag {
  flex-shrink: 0;
  font-size: 10px;
  color: var(--el-text-color-secondary);
  border: 1px solid var(--el-border-color);
  border-radius: 3px;
  padding: 0 3px;
}

/* 对象状态（异常：编译错误/不可用） */
.node-status {
  flex-shrink: 0;
  font-size: 10px;
  color: var(--el-color-warning);
  border: 1px solid var(--el-color-warning);
  border-radius: 3px;
  padding: 0 3px;
}

.node-status.is-invalid {
  color: var(--el-color-danger);
  border-color: var(--el-color-danger);
  font-weight: 600;
}







.del-title {
  margin: 0 0 10px;
  font-size: 13px;
}
.del-group {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 8px;
}
.del-danger {
  color: var(--el-color-danger);
}
.del-stats {
  margin-top: 12px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
</style>

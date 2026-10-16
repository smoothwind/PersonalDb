import { invoke } from '@tauri-apps/api/core'
import type {
  CellValue,
  ColumnMeta,
  ConnectionConfig,
  ConnectionGroup,
  DbKind,
  DbObjectType,
  IndexInfo,
  JdbcDriver,
  KindInfo,
  ObjectFolder,
  ObjectInfo,
  PlanNode,
  QueryResult,
  RoutineInfo,
  SchemaSnapshot,
  SemanticType,
  ServerInfo,
  SessionInfo,
  SqlSnippet,
  TableChange,
  TableInfo,
  WorkspaceFile,
} from '../types/db'

export const dbTestConnection = (config: ConnectionConfig) =>
  invoke<ServerInfo>('db_test_connection', { config })

export const dbConnect = (config: ConnectionConfig) =>
  invoke<SessionInfo>('db_connect', { config })

export const dbDisconnect = (sessionId: string) =>
  invoke<void>('db_disconnect', { sessionId })

export const dbListSessions = () => invoke<SessionInfo[]>('db_list_sessions')

// ================= 系统凭据（Windows 凭据管理器） =================

export const secretStore = (id: string, password: string) =>
  invoke<void>('secret_store', { id, password })

export const secretGet = (id: string) => invoke<string | null>('secret_get', { id })

export const secretDelete = (id: string) => invoke<void>('secret_delete', { id })

export const secretCopy = (from: string, to: string) => invoke<void>('secret_copy', { from, to })

export const dbListDatabases = (sessionId: string) =>
  invoke<string[]>('db_list_databases', { sessionId })

export const dbListTables = (sessionId: string, database?: string | null) =>
  invoke<TableInfo[]>('db_list_tables', { sessionId, database: database ?? null })

export const dbDescribeTable = (sessionId: string, database: string | null, table: string) =>
  invoke<ColumnMeta[]>('db_describe_table', { sessionId, database, table })

export const dbExecute = (
  sessionId: string,
  database: string | null,
  sql: string,
  maxRows = 1000,
) => invoke<QueryResult>('db_execute', { sessionId, database, sql, maxRows })

export const configSave = (connections: ConnectionConfig[]) =>
  invoke<string>('config_save', { connections })

export const configLoad = () => invoke<ConnectionConfig[]>('config_load')

export const groupSave = (groups: ConnectionGroup[]) =>
  invoke<string>('group_save', { groups })

export const groupLoad = () => invoke<ConnectionGroup[]>('group_load')

export const snippetSave = (snippets: SqlSnippet[]) =>
  invoke<string>('snippet_save', { snippets })

export const snippetLoad = () => invoke<SqlSnippet[]>('snippet_load')

export const dbRegistry = () => invoke<KindInfo[]>('db_registry')

export interface BridgeStatus {
  /** go-ora 内置驱动（真实 Oracle） */
  oracleGo: boolean
  /** OceanBase Connector/J（OceanBase Oracle 模式） */
  obJdbc: boolean
}

export const bridgeStatus = () => invoke<BridgeStatus>('bridge_status')

// ================= 通用 JDBC 驱动管理 =================

export const jdbcDriversList = () => invoke<JdbcDriver[]>('jdbc_drivers_list')

export const jdbcDriverSave = (driver: JdbcDriver) =>
  invoke<JdbcDriver[]>('jdbc_driver_save', { driver })

export const jdbcDriverDelete = (id: string) =>
  invoke<JdbcDriver[]>('jdbc_driver_delete', { id })

export const jdbcDriverScanClasses = (jars: string[]) =>
  invoke<string[]>('jdbc_driver_scan_classes', { jars })

// ================= 结果集选中行导出 =================

export type RowsExportFormat = 'sql' | 'csv' | 'excel' | 'txt' | 'json'

export interface ExportColumn {
  name: string
  semantic?: SemanticType | null
}

export interface SqlExportOptions {
  unquoteIdent?: boolean
  identCase?: 'original' | 'upper' | 'lower'
  dateConvert?: boolean
  bom?: boolean
  batchSize?: number
  /** 值转换规则：按语义包裹函数模板（{v} = 值字面量） */
  valueRules?: Array<{ semantic: string; template: string }>
}

export interface RowsExportRequest {
  kind: DbKind
  table: string
  schema?: string | null
  columns: ExportColumn[]
  rows: CellValue[][]
  format: RowsExportFormat
  path: string
  sqlOptions?: SqlExportOptions
}

export interface ExportSummary {
  path: string
  rows: number
  bytes: number
  durationMs: number
}

export const exportResultRows = (req: RowsExportRequest) =>
  invoke<ExportSummary>('export_result_rows', { req })

// ================= 结果集编辑（DML） =================

export interface DmlColumn {
  name: string
  semantic?: SemanticType | null
}

export interface DmlUpdate {
  old: CellValue[]
  new: CellValue[]
}

export interface DmlRequest {
  kind: DbKind
  schema?: string | null
  table: string
  columns: DmlColumn[]
  /** WHERE 定位列下标（主键 / ROWID / 全列） */
  whereIndices: number[]
  inserts?: CellValue[][]
  updates?: DmlUpdate[]
  deletes?: CellValue[][]
}

export const dbBuildDml = (req: DmlRequest) => invoke<string[]>('db_build_dml', { req })

/** 元数据快照（表/视图 + 列 + 注释），供 SQL 自动补全 */
export const dbSchemaSnapshot = (sessionId: string, database?: string | null) =>
  invoke<SchemaSnapshot>('db_schema_snapshot', { sessionId, database: database ?? null })

/** 判定表是否为分区表 */
export const dbTablePartitioned = (sessionId: string, database: string | null, table: string) =>
  invoke<boolean>('db_table_partitioned', { sessionId, database, table })

// ================= SQL 脚本文件 =================

export const saveTextFile = (path: string, content: string) =>
  invoke<void>('save_text_file', { path, content })

export const readTextFile = (path: string) => invoke<string>('read_text_file', { path })

// ================= 配置导入 / 导出 =================

export const vaultEncrypt = (data: unknown, passphrase: string) =>
  invoke<Record<string, unknown>>('vault_encrypt', { data, passphrase })

export const vaultDecrypt = (envelope: unknown, passphrase: string) =>
  invoke<Record<string, unknown>>('vault_decrypt', { envelope, passphrase })

/** 三方工具导入草稿（后端归一化） */
export interface ImportedConn {
  name: string
  kind: DbKind
  host: string
  port?: number | null
  username: string
  password?: string | null
  database?: string | null
  params?: Record<string, string>
  source: string
}

export const importConnectionsScan = (source: string, path: string) =>
  invoke<ImportedConn[]>('import_connections_scan', { source, path })

// ================= 服务器会话管理 =================

export const dbServerSessions = (sessionId: string) =>
  invoke<QueryResult>('db_server_sessions', { sessionId })

export const dbServerLocks = (sessionId: string) =>
  invoke<QueryResult>('db_server_locks', { sessionId })

export const dbKillSession = (sessionId: string, id: string, mode: 'cancel' | 'kill') =>
  invoke<QueryResult>('db_kill_session', { sessionId, id, mode })

// ================= 查询历史（SQLite） =================

export interface HistoryItem {
  id: string
  connectionName: string
  kind: string
  database: string
  sql: string
  durationMs: number
  ok: boolean
  at: number
}

export const historyAdd = (item: HistoryItem) => invoke<void>('history_add', { item })

export const historyList = (keyword: string, limit = 100) =>
  invoke<HistoryItem[]>('history_list', { keyword, limit })

export const historyClear = () => invoke<void>('history_clear')

export const dbListObjectFolders = (sessionId: string, database?: string | null) =>
  invoke<ObjectFolder[]>('db_list_object_folders', { sessionId, database: database ?? null })

export const dbListObjects = (
  sessionId: string,
  database: string | null,
  objectType: DbObjectType,
) => invoke<ObjectInfo[]>('db_list_objects', { sessionId, database, objectType })

export const dbPreviewRowsSql = (
  sessionId: string,
  database: string | null,
  table: string,
  limit = 100,
) => invoke<string>('db_preview_rows_sql', { sessionId, database, table, limit })

export interface ExportRequest {
  sessionId: string
  database: string | null
  table: string
  format: 'sql' | 'csv' | 'excel'
  includeDdl: boolean
  includeData: boolean
  whereClause?: string | null
  maxRows: number
  path: string
}

export interface ExportSummary {
  path: string
  rows: number
  bytes: number
  durationMs: number
}

export const dbExport = (req: ExportRequest) =>
  invoke<ExportSummary>('db_export', { req })

export const dbFetchObjectDdl = (
  sessionId: string,
  database: string | null,
  objectType: DbObjectType,
  name: string,
) => invoke<string>('db_fetch_object_ddl', { sessionId, database, objectType, name })

export const dbFetchTableDdl = (
  sessionId: string,
  database: string | null,
  table: string,
) => invoke<string>('db_fetch_table_ddl', { sessionId, database, table })

/** 重新编译对象（Oracle） */
export const dbRecompileObject = (
  sessionId: string,
  database: string | null,
  objectType: DbObjectType,
  name: string,
) => invoke<string>('db_recompile_object', { sessionId, database, objectType, name })

export const dbDescribeRoutine = (
  sessionId: string,
  database: string | null,
  name: string,
  objectType: DbObjectType,
) => invoke<RoutineInfo>('db_describe_routine', { sessionId, database, name, objectType })

export const dbListIndexes = (
  sessionId: string,
  database: string | null,
  table: string,
) => invoke<IndexInfo[]>('db_list_indexes', { sessionId, database, table })

export const dbTableChangeSql = (
  sessionId: string,
  database: string | null,
  table: string,
  changes: TableChange[],
) => invoke<string[]>('db_table_change_sql', { sessionId, database, table, changes })

export const dbExplain = (
  sessionId: string,
  database: string | null,
  sql: string,
) => invoke<PlanNode>('db_explain', { sessionId, database, sql })

/** 切换自动提交（enabled=false 进入手动事务模式） */
export const dbSetAutocommit = (sessionId: string, enabled: boolean) =>
  invoke<void>('db_set_autocommit', { sessionId, enabled })

export const dbCommit = (sessionId: string) => invoke<void>('db_commit', { sessionId })

export const dbRollback = (sessionId: string) => invoke<void>('db_rollback', { sessionId })

/** 存储类别目录信息 */
export interface StorageDirInfo {
  key: string
  labelKey: string
  dir: string
  isCustom: boolean
}

export const storageInfo = () => invoke<StorageDirInfo[]>('storage_info')

export const storageSetDir = (key: string, path: string) =>
  invoke<StorageDirInfo[]>('storage_set_dir', { key, path })

export const storageResetDir = (key: string) =>
  invoke<StorageDirInfo[]>('storage_reset_dir', { key })

export const storageOpenDir = (key: string) =>
  invoke<string>('storage_open_dir', { key })

export const workspaceSave = (data: WorkspaceFile) =>
  invoke<string>('workspace_save', { data })

export const workspaceLoad = () => invoke<WorkspaceFile>('workspace_load')

export const workspaceSetDir = (id: string, dir: string) =>
  invoke<void>('workspace_set_dir', { id, dir })

/** 读取日志（末尾 N 行） */
export const logRead = (limit?: number) => invoke<string>('log_read', { limit: limit ?? null })

/** 清空日志 */
export const logClear = () => invoke<void>('log_clear')

export type DbKind = 'mysql' | 'postgresql' | 'oceanbase' | 'oceanbase-oracle' | 'oracle' | 'sqlite' | 'jdbc'

export const DB_KIND_LABELS: Record<DbKind, string> = {
  mysql: 'MySQL',
  postgresql: 'PostgreSQL',
  oceanbase: 'OceanBase',
  'oceanbase-oracle': 'OceanBase (Oracle)',
  oracle: 'Oracle',
  sqlite: 'SQLite',
  jdbc: 'JDBC',
}

export const DB_DEFAULT_PORTS: Record<DbKind, number> = {
  mysql: 3306,
  postgresql: 5432,
  oceanbase: 2881,
  'oceanbase-oracle': 2881,
  oracle: 1521,
  sqlite: 0,
  jdbc: 0,
}

/** JDBC 驱动配置（驱动管理，来源仅本地 jar） */
export interface JdbcDriver {  id: string
  name: string
  driverClass: string
  urlTemplate: string
  defaultPort?: number | null
  jars: string[]
  builtin: boolean
  description?: string | null
}

export interface ConnectionConfig {
  id: string
  name: string
  kind: DbKind
  host: string
  port?: number | null
  username: string
  password: string
  /** 密码在系统凭据管理器中的引用（存有引用时 password 恒为空） */
  passwordRef?: string | null
  database?: string | null
  params?: Record<string, string>
  /** 所属连接文件夹（null/undefined 为未分组） */
  groupId?: string | null
}

/** 连接文件夹（支持多级嵌套） */
export interface ConnectionGroup {
  id: string
  name: string
  /** 上级文件夹（null/undefined = 根级） */
  parentId?: string | null
}

/** 页签快照（工作区持久化用，不含结果集） */
export interface TabSnapshot {
  type: 'query' | 'table' | 'object' | 'routine' | 'session'
  title: string
  connectionId: string
  connectionName: string
  kind: DbKind
  database: string
  sql: string
  icon: string
  table?: string
  objectName?: string
  objType?: DbObjectType
}

/** 工作区：命名容器（页签快照 + 文件保存位置） */
export interface Workspace {
  id: string
  name: string
  /** 该工作区的文件保存（导出）目录 */
  exportDir?: string
  /** 工作区数据在磁盘上的存储目录（空 = 默认 Workspaces 存储类别目录） */
  dir?: string
  tabs: TabSnapshot[]
}

/** workspaces.json 结构 */
export interface WorkspaceFile {
  currentId: string
  list: Workspace[]
}

export interface ServerInfo {
  kindLabel: string
  serverVersion: string
}

export interface SessionInfo {
  sessionId: string
  config: ConnectionConfig
  serverInfo: ServerInfo
}

export type CellValue = null | boolean | number | string

/** 列语义类型（后端按原生类型推断，路由查看组件） */
export type SemanticType =
  | 'text'
  | 'longtext'
  | 'json'
  | 'binary'
  | 'date'
  | 'datetime'
  | 'time'
  | 'number'
  | 'decimal'
  | 'bool'
  | 'uuid'
  | 'unknown'

export interface ColumnInfo {
  name: string
  typeLabel: string
  semantic: SemanticType
}

export interface QueryResult {
  columns: ColumnInfo[]
  rows: CellValue[][]
  affectedRows?: number | null
  durationMs: number
  truncated: boolean
}

export interface TableInfo {
  name: string
  tableType: string
  comment?: string | null
}

export interface ColumnMeta {
  name: string
  dataType: string
  nullable: boolean
  defaultValue?: string | null
  comment?: string | null
  key?: string | null
}

/** 索引元数据 */
export interface IndexInfo {
  name: string
  columns: string[]
  unique: boolean
  primary: boolean
}

/** 列定义（表结构编辑） */
export interface ColumnChange {
  name: string
  dataType: string
  nullable: boolean
  defaultValue?: string | null
  comment?: string | null
}

/** 表结构变更（kind 决定字段组合） */
export type TableChange =
  | { kind: 'addColumn'; column: ColumnChange }
  | { kind: 'dropColumn'; name: string }
  | { kind: 'modifyColumn'; column: ColumnChange }
  | { kind: 'addIndex'; name: string; columns: string[]; unique: boolean }
  | { kind: 'dropIndex'; name: string }

/** 例程（存储过程 / 函数）参数 */
export interface RoutineParam {
  name?: string | null
  /** IN / OUT / INOUT */
  mode: string
  dataType: string
}

/** 例程（存储过程 / 函数）元数据 */
export interface RoutineInfo {
  name: string
  /** PROCEDURE | FUNCTION */
  routineType: string
  returnType?: string | null
  params: RoutineParam[]
}

/** SQL 快捷指令（可配置，占位符语法 ${1:默认值}） */
export interface SqlSnippet {
  abbrev: string
  description: string
  template: string
}

/** 数据库对象类型（对象浏览器分组） */
export type DbObjectType =
  | 'table'
  | 'view'
  | 'index'
  | 'sequence'
  | 'function'
  | 'procedure'
  | 'trigger'

export const DB_OBJECT_TYPE_LABELS: Record<DbObjectType, string> = {
  table: '表',
  view: '视图',
  index: '索引',
  sequence: '序列',
  function: '函数',
  procedure: '存储过程',
  trigger: '触发器',
}

/** 对象类型文件夹（数量 > 0 才返回） */
export interface ObjectFolder {
  objectType: DbObjectType
  count: number
}

/** 对象实例 */
export interface ObjectInfo {
  name: string
  objectType: DbObjectType
  comment?: string | null
  /** 对象状态（Oracle：VALID/INVALID/UNUSABLE；其他数据库为空） */
  status?: string | null
}

/** 元数据快照（SQL 补全用） */
export interface ColumnSnap {
  name: string
  dataType: string
  comment?: string | null
}

export interface TableMeta {
  name: string
  /** TABLE / VIEW */
  kind: string
  comment?: string | null
  columns: ColumnSnap[]
}

export interface SchemaSnapshot {
  tables: TableMeta[]
}

/** 执行计划节点（属性为键值对数组） */
export interface PlanNode {
  title: string
  attrs: [string, string][]
  cost?: number | null
  rows?: number | null
  children: PlanNode[]
}

/** 数据库类型注册信息（后端能力声明） */
export interface KindInfo {
  kind: DbKind
  label: string
  defaultPort: number
  transport: string
  available: boolean
  experimental: boolean
  usernameHint?: string | null
  formatLanguage: string
  explain: boolean
  exportDdl: boolean
  exportInsert: boolean
  /** 是否支持手动事务（自动/手动提交切换） */
  transactions: boolean
  /** 是否支持服务器会话管理 */
  sessions: boolean
  objectTypes: DbObjectType[]
}

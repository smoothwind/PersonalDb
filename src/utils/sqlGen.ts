import type { DbKind } from '../types/db'

/**
 * 右键菜单「生成 SQL / 复制列名」的方言化 SQL 生成（SPEC-TREE-7）。
 *
 * 方言矩阵：
 * - 标识符引号：mysql / oceanbase（MySQL 模式）= 反引号，其余 = 双引号（内嵌引号双写转义）
 * - INSERT 占位符：oracle / oceanbase-oracle = :1,:2…；其余 = ?
 * - 库限定：mysql / oceanbase = `db`.`tbl`；postgresql / gaussdb = "db"."tbl"；其余无（schema/单库上下文）
 * - 结尾分号：oracle 系不加分号，其余加分号
 */

/** Oracle 系（含 OB Oracle 模式）共同规则：无库前缀、:N 占位符、无分号 */
function isOracleFamily(kind: DbKind): boolean {
  return kind === 'oracle' || kind === 'oceanbase-oracle'
}

/** 标识符引用字符 */
function quoteChar(kind: DbKind): string {
  return kind === 'mysql' || kind === 'oceanbase' ? '`' : '"'
}

/** 标识符按方言引用（内嵌引号双写转义） */
export function quoteIdent(kind: DbKind, name: string): string {
  const q = quoteChar(kind)
  return q + name.split(q).join(q + q) + q
}

/** INSERT VALUES 占位符列表 */
function placeholders(kind: DbKind, n: number): string[] {
  if (isOracleFamily(kind)) {
    return Array.from({ length: n }, (_, i) => `:${i + 1}`)
  }
  return Array.from({ length: n }, () => '?')
}

/** 库限定表名（Oracle 系走 schema 上下文、SQLite/JDBC 单库，不加前缀） */
function tableRef(kind: DbKind, table: string, database: string | null): string {
  const ref = quoteIdent(kind, table)
  if (!database || isOracleFamily(kind) || kind === 'sqlite' || kind === 'jdbc') return ref
  return `${quoteIdent(kind, database)}.${ref}`
}

/** 列数超过该值时列清单逐行缩进 */
const MULTILINE_THRESHOLD = 8

/** 列清单渲染：单行 = 逗号空格连接；多行 = 每项一行两格缩进 */
function renderList(items: string[], multiline: boolean): string {
  return multiline ? items.map((s) => `  ${s}`).join(',\n') : items.join(', ')
}

export interface GenSqlOptions {
  table: string
  /** 库名（Oracle 系 / SQLite / 通用 JDBC 忽略） */
  database?: string | null
  columns: string[]
}

/** 生成 SELECT 列清单语句（区别于「查询数据」的 SELECT * 预览） */
export function buildSelectSql(kind: DbKind, opts: GenSqlOptions): string {
  const cols = opts.columns.map((c) => quoteIdent(kind, c))
  const multiline = cols.length > MULTILINE_THRESHOLD
  const semi = isOracleFamily(kind) ? '' : ';'
  const ref = tableRef(kind, opts.table, opts.database ?? null)
  if (!multiline) return `SELECT ${cols.join(', ')} FROM ${ref}${semi}`
  return `SELECT\n${renderList(cols, true)}\nFROM ${ref}${semi}`
}

/** 生成 INSERT 模板（VALUES 用方言占位符，等待逐值填写） */
export function buildInsertSql(kind: DbKind, opts: GenSqlOptions): string {
  const cols = opts.columns.map((c) => quoteIdent(kind, c))
  const vals = placeholders(kind, cols.length)
  const multiline = cols.length > MULTILINE_THRESHOLD
  const semi = isOracleFamily(kind) ? '' : ';'
  const ref = tableRef(kind, opts.table, opts.database ?? null)
  if (!multiline) {
    return `INSERT INTO ${ref} (${cols.join(', ')}) VALUES (${vals.join(', ')})${semi}`
  }
  return `INSERT INTO ${ref} (\n${renderList(cols, true)}\n) VALUES (\n${renderList(vals, true)}\n)${semi}`
}

/** 复制列名的预设变体 */
export type ColumnFormat = 'plain' | 'quoted' | 'aliased' | 'lines'

/** 按预设格式化列名清单（SPEC-TREE-7：不开放全组合，仅 4 个常用预设） */
export function formatColumns(kind: DbKind, columns: string[], variant: ColumnFormat): string {
  switch (variant) {
    case 'plain':
      return columns.join(', ')
    case 'quoted':
      return columns.map((c) => quoteIdent(kind, c)).join(', ')
    case 'aliased':
      // JOIN / UPDATE 场景：统一 t. 前缀（别名本身不引用）
      return columns.map((c) => `t.${c}`).join(', ')
    case 'lines':
      return columns.join('\n')
  }
}

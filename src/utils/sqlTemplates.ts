/**
 * 查询 SQL 模板：内置默认 + 用户覆盖（设置 → SQL 模板），生效顺序
 * **`<key>.<kind>` 方言覆盖 → `<key>.default` 通用默认 → 内置兜底**。
 *
 * 分页别名约定：**必须字母开头** —— Oracle 系未引用标识符不允许 `_` 开头，
 * 历史 `_pdb_rn` 别名触发 ORA-00911，故统一为 `pdb_rn` / `pdb_p`（各类数据库
 * 独立模板，杜绝跨方言串扰）。
 */
import type { DbKind } from '../types/db'

/** 用户覆盖表（key → 覆盖 SQL，仅含用户设置项）；由 sqlTemplates store 启动时注入 */
let overrides: Record<string, string> = {}

/** 注入后端用户覆盖表（稀疏；离线/失败时保持空表，全部走内置兜底） */
export function setResolvedTemplates(map: Record<string, string>): void {
  overrides = map ?? {}
}

/** 占位符替换：{name} → value；未提供的占位符原样保留 */
export function applyTemplate(tpl: string, vars: Record<string, string | number>): string {
  let out = tpl
  for (const [k, v] of Object.entries(vars)) {
    out = out.split(`{${k}}`).join(String(v))
  }
  return out
}

/**
 * 内置分页模板（与后端 templates.rs 同源；仅作后端不可达时的兜底）。
 * 占位符：{sql} 原始 SQL（已去分号）、{offset} 偏移、{limit} 页大小、{end} = offset+limit
 */
export function builtinPaged(kind: DbKind): string {
  if (kind === 'oracle' || kind === 'oceanbase-oracle') {
    return (
      'SELECT * FROM (SELECT t.*, ROWNUM pdb_rn FROM ({sql}) t WHERE ROWNUM <= {end}) ' +
      'WHERE pdb_rn > {offset}'
    )
  }
  return 'SELECT * FROM ({sql}) pdb_p LIMIT {limit} OFFSET {offset}'
}

/** 内置双击表预览模板。占位符：{database} 库（Oracle/SQLite/JDBC 留空）、{table} 表、{limit} 行数 */
export function builtinPreview(kind: DbKind): string {
  switch (kind) {
    case 'mysql':
    case 'oceanbase':
      return 'SELECT * FROM `{database}`.`{table}` LIMIT {limit};'
    case 'postgresql':
    case 'gaussdb':
      return 'SELECT * FROM "{database}"."{table}" LIMIT {limit};'
    case 'oracle':
    case 'oceanbase-oracle':
      return 'SELECT * FROM "{table}" WHERE ROWNUM <= {limit}'
    // SQLite 无库概念；通用 JDBC 保守不加库前缀（传输层 max_rows 截断行数）
    case 'sqlite':
      return 'SELECT * FROM "{table}" LIMIT {limit};'
    case 'jdbc':
      return 'SELECT * FROM "{table}"'
    default:
      return 'SELECT * FROM "{table}" LIMIT {limit};'
  }
}

/** 分页生效模板：方言覆盖 → 通用默认 → 内置；缺 {sql} 占位符的覆盖无法包装，回退内置 */
function pagedTemplateOf(kind: DbKind): string {
  const tpl =
    (overrides[`paged.${kind}`] ?? '').trim() ||
    (overrides['paged.default'] ?? '').trim() ||
    builtinPaged(kind)
  return tpl.includes('{sql}') ? tpl : builtinPaged(kind)
}

/** 预览生效模板：方言覆盖 → 通用默认 → 内置 */
function previewTemplateOf(kind: DbKind): string {
  return (
    (overrides[`preview.${kind}`] ?? '').trim() ||
    (overrides['preview.default'] ?? '').trim() ||
    builtinPreview(kind)
  )
}

/**
 * 任意 SQL 包装为分页查询（与后端方言分页策略对应：Oracle ROWNUM / 其余 LIMIT OFFSET）。
 * 模板可按 `paged.<kind>` / `paged.default` 覆盖（设置 → SQL 模板）。
 */
export function pagedSql(kind: DbKind, sql: string, offset: number, limit: number): string {
  const inner = sql.trim().replace(/;+\s*$/, '')
  return applyTemplate(pagedTemplateOf(kind), {
    sql: inner,
    offset,
    limit,
    end: offset + limit,
  })
}

/** 双击表预览 SQL（`preview.<kind>` / `preview.default` 可覆盖） */
export function previewSelectSql(kind: DbKind, database: string, table: string): string {
  const tpl = previewTemplateOf(kind)
  // Oracle 系/SQLite/JDBC 无库前缀：database 传空避免误导覆盖模板作者
  const dbVar = kind === 'mysql' || kind === 'oceanbase' || kind === 'postgresql' || kind === 'gaussdb' ? database : ''
  return applyTemplate(tpl, { database: dbVar, table, limit: 100 })
}

import type { DbKind, SqlSnippet } from '../types/db'

/** 分页预览模板按方言生成 */
function previewTemplate(kind: DbKind): string {
  if (kind === 'oracle' || kind === 'oceanbase-oracle') {
    return 'SELECT * FROM ${1:table} WHERE ROWNUM <= ${2:100}'
  }
  return 'SELECT * FROM ${1:table} LIMIT ${2:100}'
}

/**
 * 内置快捷指令（自定义同名条目可覆盖）。
 * description 为 i18n key（snippetBuiltin.*），展示时由 UI 层翻译。
 * 占位符语法：${序号:默认文本}，展开后 Tab 逐个跳转。
 */
export function defaultSnippets(kind: DbKind): SqlSnippet[] {
  return [
    { abbrev: 'sf', description: 'snippetBuiltin.sf', template: 'SELECT * FROM ${1:table}' },
    { abbrev: 'sfw', description: 'snippetBuiltin.sfw', template: 'SELECT * FROM ${1:table} WHERE ${2:cond}' },
    { abbrev: 'sfj', description: 'snippetBuiltin.sfj', template: 'SELECT * FROM ${1:table} t1 INNER JOIN ${2:table2} t2 ON ${3:cond}' },
    { abbrev: 'sc', description: 'snippetBuiltin.sc', template: 'SELECT COUNT(*) FROM ${1:table}' },
    { abbrev: 'sl', description: 'snippetBuiltin.sl', template: previewTemplate(kind) },
    { abbrev: 'ins', description: 'snippetBuiltin.ins', template: 'INSERT INTO ${1:table} (${2:cols}) VALUES (${3:vals})' },
    { abbrev: 'up', description: 'snippetBuiltin.up', template: 'UPDATE ${1:table} SET ${2:col = val} WHERE ${3:cond}' },
    { abbrev: 'del', description: 'snippetBuiltin.del', template: 'DELETE FROM ${1:table} WHERE ${2:cond}' },
    { abbrev: 'whr', description: 'snippetBuiltin.whr', template: 'WHERE ${1:cond}' },
    { abbrev: 'ord', description: 'snippetBuiltin.ord', template: 'ORDER BY ${1:col}' },
    { abbrev: 'grp', description: 'snippetBuiltin.grp', template: 'GROUP BY ${1:col}' },
    { abbrev: 'jin', description: 'snippetBuiltin.jin', template: 'INNER JOIN ${1:table} ON ${2:cond}' },
  ]
}

import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import {
  applyTemplate,
  builtinPaged,
  builtinPreview,
  pagedSql,
  previewSelectSql,
  setResolvedTemplates,
} from '../src/utils/sqlTemplates'

/** 保存/恢复模块级 resolved 表，避免测试间串扰 */
let saved: Record<string, string> | null = null
beforeEach(() => {
  saved = null
})
afterEach(() => {
  setResolvedTemplates(saved ?? {})
})

describe('内置分页模板（跨方言隔离，回归 _pdb_rn ORA-00911）', () => {
  it('Oracle 系走 ROWNUM + 字母开头别名，不含 LIMIT 与下划线开头别名', () => {
    for (const kind of ['oracle', 'oceanbase-oracle'] as const) {
      const tpl = builtinPaged(kind)
      expect(tpl).toContain('ROWNUM')
      expect(tpl).toContain('pdb_rn')
      expect(tpl).not.toMatch(/_pdb/)
      expect(tpl).not.toContain('LIMIT')
    }
  })

  it('LIMIT 系（MySQL/PG/GaussDB/SQLite）不含 ROWNUM，别名 pdb_p', () => {
    for (const kind of ['mysql', 'oceanbase', 'postgresql', 'gaussdb', 'sqlite', 'jdbc'] as const) {
      const tpl = builtinPaged(kind)
      expect(tpl).toContain('LIMIT {limit} OFFSET {offset}')
      expect(tpl).toContain('pdb_p')
      expect(tpl).not.toContain('ROWNUM')
      expect(tpl).not.toMatch(/_pdb/)
    }
  })

  it('pagedSql 代入占位符并剥离尾分号', () => {
    const sql = pagedSql('oracle', 'SELECT * FROM t;', 20, 10)
    expect(sql).toContain('ROWNUM <= 30')
    expect(sql).toContain('pdb_rn > 20')
    expect(sql).not.toContain(';')
    const sql2 = pagedSql('mysql', 'SELECT * FROM t', 40, 20)
    expect(sql2).toContain('LIMIT 20 OFFSET 40')
  })
})

describe('用户覆盖解析链（方言覆盖 → 通用默认 → 内置）', () => {
  it('方言覆盖优先', () => {
    setResolvedTemplates({ 'paged.oracle': 'SELECT * FROM ({sql}) o LIMIT {limit}' })
    expect(pagedSql('oracle', 'SELECT 1', 0, 5)).toBe('SELECT * FROM (SELECT 1) o LIMIT 5')
    // 其他种类不受影响
    expect(pagedSql('mysql', 'SELECT 1', 0, 5)).toContain('LIMIT 5 OFFSET 0')
  })

  it('通用默认条目对全部种类生效（含 Oracle 系）', () => {
    setResolvedTemplates({ 'paged.default': 'WRAP({sql}, {offset}, {limit}, {end})' })
    expect(pagedSql('postgresql', 'SELECT 1', 10, 5)).toBe('WRAP(SELECT 1, 10, 5, 15)')
    expect(pagedSql('oracle', 'SELECT 1', 10, 5)).toBe('WRAP(SELECT 1, 10, 5, 15)')
  })

  it('方言覆盖优先于通用默认', () => {
    setResolvedTemplates({
      'paged.default': 'WRAP({sql}, {offset}, {limit}, {end})',
      'paged.oracle': 'SELECT * FROM ({sql}) o LIMIT {limit}',
    })
    expect(pagedSql('oracle', 'SELECT 1', 0, 5)).toBe('SELECT * FROM (SELECT 1) o LIMIT 5')
    expect(pagedSql('mysql', 'SELECT 1', 0, 5)).toBe('WRAP(SELECT 1, 0, 5, 5)')
  })

  it('缺 {sql} 占位符的分页覆盖无法包装，回退内置（防配置打穿语法）', () => {
    setResolvedTemplates({ 'paged.mysql': 'SELECT * FROM t LIMIT {limit}' })
    expect(pagedSql('mysql', 'SELECT 1', 0, 5)).toContain('pdb_p')
    expect(pagedSql('mysql', 'SELECT 1', 0, 5)).toContain('(SELECT 1)')
  })

  it('空白覆盖视为未覆盖', () => {
    setResolvedTemplates({ 'paged.oracle': '   ' })
    expect(pagedSql('oracle', 'SELECT 1', 0, 5)).toContain('pdb_rn')
  })
})

describe('双击表预览 SQL（preview.<kind> 覆盖）', () => {
  it('内置按种类分流：MySQL 带库前缀反引号 / Oracle ROWNUM / jdbc 无 LIMIT', () => {
    expect(builtinPreview('mysql')).toContain('`{database}`.`{table}`')
    expect(builtinPreview('oracle')).toContain('ROWNUM <= {limit}')
    expect(builtinPreview('jdbc')).toBe('SELECT * FROM "{table}"')
  })

  it('previewSelectSql 代入库名与表名', () => {
    expect(previewSelectSql('mysql', 'shop', 'orders')).toContain('`shop`.`orders`')
    expect(previewSelectSql('oracle', '', 'ORDERS')).toContain('"ORDERS"')
  })

  it('preview.default 兜底与方言覆盖', () => {
    setResolvedTemplates({ 'preview.default': 'SELECT * FROM {table} FETCH FIRST {limit} ROWS ONLY' })
    expect(previewSelectSql('sqlite', '', 't')).toContain('FETCH FIRST 100 ROWS ONLY')
    setResolvedTemplates({ 'preview.sqlite': 'SELECT * FROM {table} LIMIT 5' })
    expect(previewSelectSql('sqlite', '', 't')).toContain('LIMIT 5')
  })
})

describe('applyTemplate 占位符替换', () => {
  it('替换已知并保留未知占位符', () => {
    expect(applyTemplate('A {x} B {y} C {z}', { x: 1, y: 's' })).toBe('A 1 B s C {z}')
  })
})

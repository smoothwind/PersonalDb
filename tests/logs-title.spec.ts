import { describe, expect, it } from 'vitest'
import { sqlTitle } from '../src/stores/logs'

describe('sqlTitle：匿名块摘要（取块体内首个有效行）', () => {
  it('DECLARE 块：跳过声明段，取 BEGIN 后的过程调用行', () => {
    const sql = ['DECLARE', '  pdb_a2 VARCHAR2(32767);', 'BEGIN', '  "ETL"."P_SYNC"(1, pdb_a2);', 'END;'].join('\n')
    expect(sqlTitle(sql)).toBe('"ETL"."P_SYNC"(1, pdb_a2);')
  })

  it('BEGIN 块：取下一有效行', () => {
    const sql = ['BEGIN', '  CALL_PROC(1);', 'END;'].join('\n')
    expect(sqlTitle(sql)).toBe('CALL_PROC(1);')
  })

  it('普通 SQL：取首个非空行（行为不变）', () => {
    expect(sqlTitle('SELECT 1 FROM DUAL')).toBe('SELECT 1 FROM DUAL')
    expect(sqlTitle('\n  \n  UPDATE t SET a = 1;')).toBe('UPDATE t SET a = 1;')
  })

  it('块内只有标记行时回退首个非空行', () => {
    expect(sqlTitle('DECLARE\nBEGIN\nEND;')).toBe('DECLARE')
  })

  it('超长行截断到 160 + 省略号', () => {
    const long = 'SELECT ' + 'x'.repeat(200)
    const t = sqlTitle(long)
    expect(t.length).toBe(161)
    expect(t.endsWith('…')).toBe(true)
  })
})

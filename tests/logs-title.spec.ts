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

  it('DECLARE 无 BEGIN：行内声明不被当标记，整行作摘要', () => {
    // 标记识别要求裸行（^DECLARE$）；行内 "DECLARE x NUMBER;" 属内容行
    expect(sqlTitle('DECLARE x NUMBER;\nfoo;')).toBe('DECLARE x NUMBER;')
  })

  it('多行 DECLARE 无 BEGIN（畸形块）：回退标记行', () => {
    // 无 BEGIN 时声明段扫描越界 → 回退首个非空行 = DECLARE 本身（畸形 PL 的兜底边界）
    expect(sqlTitle('DECLARE\n  x NUMBER;\n  foo;')).toBe('DECLARE')
  })

  it('嵌套块：取第一个 BEGIN 后的内层首个有效行', () => {
    const sql = ['DECLARE', '  pdb_a1 NUMBER;', 'BEGIN', '  INNER_STMT;', 'END;', 'BEGIN', '  TAIL;', 'END;'].join('\n')
    expect(sqlTitle(sql)).toBe('INNER_STMT;')
  })

  it('超长行截断到 160 + 省略号', () => {
    const long = 'SELECT ' + 'x'.repeat(200)
    const t = sqlTitle(long)
    expect(t.length).toBe(161)
    expect(t.endsWith('…')).toBe(true)
  })
})

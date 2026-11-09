import { describe, expect, it } from 'vitest'
import {
  buildRoutineCallSql,
  emptyArg,
  routineArgLiteral,
  type RoutineArgInput,
} from '../src/utils/routineExec'
import type { DbKind, RoutineInfo } from '../src/types/db'

const arg = (text: string, extra?: Partial<RoutineArgInput>): RoutineArgInput => ({
  text,
  isNull: false,
  raw: false,
  ...extra,
})

const proc = (params: RoutineInfo['params']): RoutineInfo => ({
  name: 'P_SYNC',
  routineType: 'PROCEDURE',
  params,
})

const func = (params: RoutineInfo['params'], returnType?: string): RoutineInfo => ({
  name: 'F_CALC',
  routineType: 'FUNCTION',
  returnType,
  params,
})

describe('routineArgLiteral：参数字面量规则', () => {
  it('NULL 勾选或空文本 → NULL（优先级最高）', () => {
    expect(routineArgLiteral('oracle', arg('1', { isNull: true }))).toBe('NULL')
    expect(routineArgLiteral('oracle', arg(''))).toBe('NULL')
    expect(routineArgLiteral('oracle', arg('   '))).toBe('NULL')
    expect(routineArgLiteral('oracle', undefined)).toBe('NULL')
  })

  it('字符串边界：多行文本 / 混合引号反斜杠 / 超长输入', () => {
    // 多行文本（换行随字面量原样进入）
    const multiline = 'line1\nline2'
    const multilineOracle = routineArgLiteral('oracle', arg(multiline))
    expect(multilineOracle.startsWith("'") && multilineOracle.endsWith("'")).toBe(true)
    expect(multilineOracle).toContain('line1\nline2')
    // 单引号翻倍；MySQL 另转义反斜杠（会话语义），Oracle 保留
    expect(routineArgLiteral('mysql', arg("it's\\行"))).toBe("'it''s\\\\行'")
    expect(routineArgLiteral('oracle', arg("it's\\行"))).toBe("'it''s\\行'")
    // 超长输入（10KB）：包装完整、单引号翻倍不丢字符、反斜杠按方言语义
    const long = 'x'.repeat(10_000) + "'\\"
    const longOracle = routineArgLiteral('oracle', arg(long))
    expect(longOracle.startsWith("'") && longOracle.endsWith("'")).toBe(true)
    expect(longOracle).toContain("x''\\")
    expect(longOracle.length).toBe(long.replace(/'/g, "''").length + 2)
    const longMysql = routineArgLiteral('mysql', arg(long))
    expect(longMysql.length).toBe(long.replace(/'/g, "''").replace(/\\/g, '\\\\').length + 2)
  })

  it('原样 → 原文内联（表达式逃生门）', () => {
    expect(routineArgLiteral('oracle', arg("TO_DATE('2024-01-01','YYYY-MM-DD')", { raw: true }))).toBe(
      "TO_DATE('2024-01-01','YYYY-MM-DD')",
    )
  })

  it('纯数字 → 数值原样', () => {
    expect(routineArgLiteral('mysql', arg('42'))).toBe('42')
    expect(routineArgLiteral('mysql', arg('-3.14'))).toBe('-3.14')
    expect(routineArgLiteral('mysql', arg('+7'))).toBe('+7')
  })

  it('其余 → 方言字符串字面量（单引号翻倍；MySQL 系转义反斜杠）', () => {
    expect(routineArgLiteral('oracle', arg("O'Brien"))).toBe("'O''Brien'")
    expect(routineArgLiteral('postgresql', arg('a\\b'))).toBe("'a\\b'")
    expect(routineArgLiteral('mysql', arg('a\\b'))).toBe("'a\\\\b'")
    expect(routineArgLiteral('mysql', arg("it's"))).toBe("'it''s'")
  })
})

describe('buildRoutineCallSql：函数', () => {
  it('Oracle / OB Oracle：SELECT ... FROM DUAL', () => {
    const r = buildRoutineCallSql('oracle', func([], 'NUMBER'), [], 'RISKRCI')
    expect(r.sql).toBe('SELECT "RISKRCI"."F_CALC"() FROM DUAL;')
    const ob = buildRoutineCallSql('oceanbase-oracle', func([{ mode: 'IN', dataType: 'NUMBER' }]), [arg('1')], '')
    expect(ob.sql).toBe('SELECT "F_CALC"(1) FROM DUAL;')
  })

  it('MySQL / PG：SELECT ...（无 FROM DUAL）', () => {
    expect(buildRoutineCallSql('mysql', func([]), [], 'db1').sql).toBe('SELECT `db1`.`F_CALC`();')
    expect(buildRoutineCallSql('postgresql', func([]), [], '').sql).toBe('SELECT "F_CALC"();')
  })
})

describe('buildRoutineCallSql：过程（无 OUT）', () => {
  const ins = [{ mode: 'IN', dataType: 'NUMBER' }, { mode: 'IN', dataType: 'VARCHAR2' }]

  it('Oracle 系匿名块 BEGIN..END', () => {
    const r = buildRoutineCallSql('oracle', proc(ins), [arg('1'), arg('x')], 'RISKRCI')
    expect(r.sql).toBe('BEGIN\n  "RISKRCI"."P_SYNC"(1, \'x\');\nEND;')
    expect(r.outNoteKey).toBeUndefined()
  })

  it('MySQL 系 CALL', () => {
    const r = buildRoutineCallSql('mysql', proc(ins), [arg('1'), arg("O'B")], 'db1')
    expect(r.sql).toBe("CALL `db1`.`P_SYNC`(1, 'O''B');")
  })

  it('PG / GaussDB：CALL', () => {
    const r = buildRoutineCallSql('gaussdb', proc(ins), [arg('1'), arg('x')], 's')
    expect(r.sql).toBe('CALL "s"."P_SYNC"(1, \'x\');')
  })
})

describe('buildRoutineCallSql：过程（含 OUT）', () => {
  const mixed = [
    { mode: 'IN', dataType: 'NUMBER' },
    { mode: 'OUT', dataType: 'VARCHAR2(100)' },
    { mode: 'INOUT', dataType: 'NUMBER' },
  ]

  it('Oracle 系：DECLARE 本地变量承载 OUT/INOUT（INOUT 以输入值初始化），提示不回显', () => {
    const r = buildRoutineCallSql('oracle', proc(mixed), [arg('1'), emptyArg(), arg('9')], null)
    expect(r.sql).toBe(
      'DECLARE\n  pdb_a2 VARCHAR2(100);\n  pdb_a3 NUMBER := 9;\nBEGIN\n  "P_SYNC"(1, pdb_a2, pdb_a3);\nEND;',
    )
    expect(r.outNoteKey).toBe('routineExec.outNoEcho')
  })

  it('MySQL 系：CALL @会话变量 + 追加 SELECT 回显，不发提示', () => {
    const r = buildRoutineCallSql('mysql', proc(mixed), [arg('1'), emptyArg(), arg('9')], 'db1')
    expect(r.sql).toBe(
      "CALL `db1`.`P_SYNC`(1, @pdb_a2, @pdb_a3);\nSELECT @pdb_a2 AS `pdb_a2`, @pdb_a3 AS `pdb_a3`;",
    )
    expect(r.outNoteKey).toBeUndefined()
  })

  it('PG：OUT 位 NULL 占位 + 提示；INOUT 传值', () => {
    const r = buildRoutineCallSql('postgresql', proc(mixed), [arg('1'), emptyArg(), arg('9')], '')
    expect(r.sql).toBe('CALL "P_SYNC"(1, NULL, 9);')
    expect(r.outNoteKey).toBe('routineExec.outNoEcho')
  })
})

describe('buildRoutineCallSql：数据类型缺失兜底', () => {
  it('OUT 参数 dataType 为空时回退 VARCHAR2(4000)', () => {
    const r = buildRoutineCallSql('oracle', proc([{ mode: 'OUT', dataType: '' }]), [emptyArg()], '')
    expect(r.sql).toContain('pdb_a1 VARCHAR2(4000);')
  })
})

describe('buildRoutineCallSql：裸类型归一化（OB ORA-00600 [-5544] 修复）', () => {
  it('裸 VARCHAR2 / RAW / CHAR 补默认长度', () => {
    const r = buildRoutineCallSql(
      'oracle',
      proc([
        { mode: 'OUT', dataType: 'VARCHAR2' },
        { mode: 'OUT', dataType: 'RAW' },
        { mode: 'INOUT', dataType: 'CHAR' },
      ]),
      [emptyArg(), emptyArg(), arg('9')],
      '',
    )
    expect(r.sql).toContain('pdb_a1 VARCHAR2(32767);')
    expect(r.sql).toContain('pdb_a2 RAW(2000);')
    expect(r.sql).toContain('pdb_a3 CHAR(32767) := 9;')
  })

  it('带长度的类型与裸 NUMBER 不动', () => {
    const r = buildRoutineCallSql(
      'oracle',
      proc([
        { mode: 'OUT', dataType: 'VARCHAR2(100)' },
        { mode: 'OUT', dataType: 'NUMBER' },
        { mode: 'OUT', dataType: 'CLOB' },
      ]),
      [emptyArg(), emptyArg(), emptyArg()],
      '',
    )
    expect(r.sql).toContain('pdb_a1 VARCHAR2(100);')
    expect(r.sql).toContain('pdb_a2 NUMBER;')
    expect(r.sql).toContain('pdb_a3 CLOB;')
  })
})

describe('buildRoutineCallSql：kind 矩阵（全库覆盖）', () => {
  const kinds: DbKind[] = ['mysql', 'oceanbase', 'postgresql', 'gaussdb', 'oracle', 'oceanbase-oracle']
  it('六种库的函数调用均生成可执行 SELECT', () => {
    for (const k of kinds) {
      const r = buildRoutineCallSql(k, func([]), [], '')
      expect(r.sql.startsWith('SELECT')).toBe(true)
      expect(k === 'oracle' || k === 'oceanbase-oracle' ? r.sql.includes('FROM DUAL') : true).toBe(true)
    }
  })
})

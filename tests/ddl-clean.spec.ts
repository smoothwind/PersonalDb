import { describe, expect, it } from 'vitest'
import { cleanDdl, stripQualifier, stripSchemaPrefix, stripQuoteMarks, stripStorage } from '../src/utils/ddlClean'

describe('cleanDdl：去除存储相关（stripStorage）', () => {
  it('Oracle：STORAGE/TABLESPACE/PCTFREE/SEGMENT CREATION/LOGGING 等全部去除', () => {
    const ddl = [
      'CREATE TABLE "SCHEMA"."T" ("ID" NUMBER(10)',
      ') SEGMENT CREATION IMMEDIATE NOLOGGING NOCOMPRESS',
      ' PCTFREE 10 INITRANS 1 MAXTRANS 255',
      ' STORAGE(INITIAL 65536 NEXT 1048576 MINEXTENTS 1)',
      ' TABLESPACE "USERS"',
    ].join('\n')
    const out = cleanDdl(ddl, 'oracle', { stripStorage: true })
    expect(out).not.toContain('STORAGE')
    expect(out).not.toContain('TABLESPACE')
    expect(out).not.toContain('PCTFREE')
    expect(out).not.toContain('SEGMENT CREATION')
    expect(out).not.toContain('NOLOGGING')
    expect(out).toContain('CREATE TABLE')
    expect(out).toContain('"ID" NUMBER(10)')
  })

  it('MySQL：ENGINE/CHARSET/COLLATE/AUTO_INCREMENT/ROW_FORMAT 全部去除', () => {
    const ddl =
      'CREATE TABLE `t` (`id` INT) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_general_ci AUTO_INCREMENT=100 ROW_FORMAT=DYNAMIC'
    const out = cleanDdl(ddl, 'mysql', { stripStorage: true })
    expect(out).not.toContain('ENGINE')
    expect(out).not.toContain('CHARSET')
    expect(out).not.toContain('COLLATE')
    expect(out).not.toContain('AUTO_INCREMENT')
    expect(out).not.toContain('ROW_FORMAT')
    expect(out).toContain('CREATE TABLE `t`')
  })

  it('Oracle 方言不去除 MySQL 存储子句（按 kind 分支）', () => {
    const ddl = 'CREATE TABLE t (a INT) ENGINE=InnoDB'
    expect(stripStorage(ddl, 'oracle')).toContain('ENGINE=InnoDB')
    expect(stripStorage(ddl, 'mysql')).not.toContain('ENGINE=InnoDB')
  })
})

describe('cleanDdl：schema 前缀与标识符引号独立开关', () => {
  it('stripSchemaPrefix：双引号 / 反引号 / 方括号 / 裸 schema 前缀去除，引号保留', () => {
    expect(stripSchemaPrefix('"SCHEMA"."T"')).toBe('"T"')
    expect(stripSchemaPrefix('`db`.`t`')).toBe('`t`')
    expect(stripSchemaPrefix('[dbo].[user]')).toBe('[user]')
    expect(stripSchemaPrefix('CREATE TABLE s.t (a INT)')).toBe('CREATE TABLE t (a INT)')
  })

  it('stripQuoteMarks：仅去引号，schema 前缀保留', () => {
    expect(stripQuoteMarks('"SCHEMA"."T"')).toBe('SCHEMA.T')
    expect(stripQuoteMarks('`db`.`t`')).toBe('db.t')
    expect(stripQuoteMarks('[dbo].[user]')).toBe('dbo.user')
  })

  it('独立组合：只去引号 / 只去 schema / 两者都开', () => {
    const ddl = 'CREATE TABLE "S"."T" (a INT)'
    expect(cleanDdl(ddl, 'oracle', { stripQuote: true })).toBe('CREATE TABLE S.T (a INT)')
    expect(cleanDdl(ddl, 'oracle', { stripSchema: true })).toBe('CREATE TABLE "T" (a INT)')
    expect(cleanDdl(ddl, 'oracle', { stripSchema: true, stripQuote: true })).toBe('CREATE TABLE T (a INT)')
  })

  it('兼容旧参数 stripQualifier（= schema + quote）', () => {
    const ddl = 'CREATE TABLE "S"."T" (a INT) TABLESPACE x'
    expect(cleanDdl(ddl, 'oracle', { stripQualifier: true })).toBe(cleanDdl(ddl, 'oracle', { stripSchema: true, stripQuote: true }))
    expect(stripQualifier('"SCHEMA"."T"')).toBe('T')
  })

  it('组合：限定符 + 存储 + 空白归一', () => {
    const ddl = 'CREATE TABLE "S"."T" (\n  "A"  VARCHAR2(10)\n)  TABLESPACE  "USERS" ;'
    const out = cleanDdl(ddl, 'oracle', { stripSchema: true, stripQuote: true, stripStorage: true })
    expect(out).not.toContain('"S"')
    expect(out).not.toContain('TABLESPACE')
    expect(out).toContain('CREATE TABLE T (')
  })

  it('默认不清洗（{} 选项）', () => {
    expect(cleanDdl('CREATE TABLE t (a INT) TABLESPACE x', 'oracle', {})).toContain('TABLESPACE')
    expect(cleanDdl('CREATE TABLE "S"."T" (a INT)', 'oracle', {})).toContain('"S"."T"')
  })
})

import { describe, expect, it } from 'vitest'
import { buildInsertSql, buildSelectSql, formatColumns, quoteIdent } from '../src/utils/sqlGen'

describe('quoteIdent 方言标识符引用', () => {
  it('MySQL 系反引号、其余双引号', () => {
    expect(quoteIdent('mysql', 'id')).toBe('`id`')
    expect(quoteIdent('oceanbase', 't1')).toBe('`t1`')
    expect(quoteIdent('postgresql', 'id')).toBe('"id"')
    expect(quoteIdent('gaussdb', 't1')).toBe('"t1"')
    expect(quoteIdent('oracle', 'ID')).toBe('"ID"')
    expect(quoteIdent('oceanbase-oracle', 'ID')).toBe('"ID"')
    expect(quoteIdent('sqlite', 't')).toBe('"t"')
    expect(quoteIdent('jdbc', 'x')).toBe('"x"')
  })

  it('内嵌引号双写转义', () => {
    expect(quoteIdent('mysql', 'a`b')).toBe('`a``b`')
    expect(quoteIdent('oracle', 'A"B')).toBe('"A""B"')
  })
})

describe('buildSelectSql 生成 SELECT 列清单', () => {
  it('MySQL / OceanBase：反引号 + 库前缀 + 分号', () => {
    expect(
      buildSelectSql('mysql', { table: 'user', database: 'mydb', columns: ['id', 'name'] }),
    ).toBe('SELECT `id`, `name` FROM `mydb`.`user`;')
    expect(
      buildSelectSql('oceanbase', { table: 't1', database: 'obdb', columns: ['c1'] }),
    ).toBe('SELECT `c1` FROM `obdb`.`t1`;')
  })

  it('PostgreSQL / GaussDB：双引号 + 库（schema）前缀 + 分号', () => {
    expect(
      buildSelectSql('postgresql', { table: 'user', database: 'mydb', columns: ['id'] }),
    ).toBe('SELECT "id" FROM "mydb"."user";')
    expect(
      buildSelectSql('gaussdb', { table: 't1', database: 's1', columns: ['c1', 'c2'] }),
    ).toBe('SELECT "c1", "c2" FROM "s1"."t1";')
  })

  it('Oracle / OB Oracle：无库前缀（schema 上下文）、无分号、库名被忽略', () => {
    expect(
      buildSelectSql('oracle', { table: 'USER', database: 'mydb', columns: ['ID', 'NAME'] }),
    ).toBe('SELECT "ID", "NAME" FROM "USER"')
    expect(
      buildSelectSql('oceanbase-oracle', { table: 'T1', database: 'tenant', columns: ['C1'] }),
    ).toBe('SELECT "C1" FROM "T1"')
  })

  it('SQLite / 通用 JDBC：单库无前缀、双引号、分号', () => {
    expect(buildSelectSql('sqlite', { table: 't', database: 'main', columns: ['a'] })).toBe(
      'SELECT "a" FROM "t";',
    )
    expect(buildSelectSql('jdbc', { table: 't', columns: ['a'] })).toBe('SELECT "a" FROM "t";')
  })

  it('≤8 列单行（边界值 8 仍单行）', () => {
    expect(
      buildSelectSql('mysql', { table: 't', columns: ['c1', 'c2', 'c3', 'c4', 'c5', 'c6', 'c7', 'c8'] }),
    ).toBe('SELECT `c1`, `c2`, `c3`, `c4`, `c5`, `c6`, `c7`, `c8` FROM `t`;')
  })

  it('>8 列逐行缩进', () => {
    const cols = ['c1', 'c2', 'c3', 'c4', 'c5', 'c6', 'c7', 'c8', 'c9']
    expect(buildSelectSql('mysql', { table: 't', database: 'db', columns: cols })).toBe(
      [
        'SELECT',
        '  `c1`,',
        '  `c2`,',
        '  `c3`,',
        '  `c4`,',
        '  `c5`,',
        '  `c6`,',
        '  `c7`,',
        '  `c8`,',
        '  `c9`',
        'FROM `db`.`t`;',
      ].join('\n'),
    )
  })
})

describe('buildInsertSql 生成 INSERT 模板', () => {
  it('MySQL / OceanBase：? 占位符 + 库前缀 + 分号', () => {
    expect(
      buildInsertSql('mysql', { table: 'user', database: 'mydb', columns: ['id', 'name'] }),
    ).toBe('INSERT INTO `mydb`.`user` (`id`, `name`) VALUES (?, ?);')
  })

  it('PostgreSQL：? 占位符 + schema 前缀', () => {
    expect(
      buildInsertSql('postgresql', { table: 'user', database: 'mydb', columns: ['id', 'name'] }),
    ).toBe('INSERT INTO "mydb"."user" ("id", "name") VALUES (?, ?);')
  })

  it('Oracle / OB Oracle：:N 占位符、无分号、无库前缀', () => {
    expect(
      buildInsertSql('oracle', { table: 'USER', columns: ['ID', 'NAME'] }),
    ).toBe('INSERT INTO "USER" ("ID", "NAME") VALUES (:1, :2)')
    expect(
      buildInsertSql('oceanbase-oracle', { table: 'T1', database: 'ignored', columns: ['A', 'B', 'C'] }),
    ).toBe('INSERT INTO "T1" ("A", "B", "C") VALUES (:1, :2, :3)')
  })

  it('SQLite / 通用 JDBC：? 占位符、单库', () => {
    expect(buildInsertSql('sqlite', { table: 't', columns: ['a', 'b'] })).toBe(
      'INSERT INTO "t" ("a", "b") VALUES (?, ?);',
    )
    expect(buildInsertSql('jdbc', { table: 't', columns: ['a'] })).toBe(
      'INSERT INTO "t" ("a") VALUES (?);',
    )
  })

  it('>8 列时列清单与 VALUES 对称逐行', () => {
    const cols = ['c1', 'c2', 'c3', 'c4', 'c5', 'c6', 'c7', 'c8', 'c9']
    expect(buildInsertSql('mysql', { table: 't', columns: cols })).toBe(
      [
        'INSERT INTO `t` (',
        '  `c1`,',
        '  `c2`,',
        '  `c3`,',
        '  `c4`,',
        '  `c5`,',
        '  `c6`,',
        '  `c7`,',
        '  `c8`,',
        '  `c9`',
        ') VALUES (',
        '  ?,',
        '  ?,',
        '  ?,',
        '  ?,',
        '  ?,',
        '  ?,',
        '  ?,',
        '  ?,',
        '  ?',
        ');',
      ].join('\n'),
    )
    expect(buildInsertSql('oracle', { table: 'T', columns: cols })).toContain(':9')
  })
})

describe('formatColumns 复制列名 4 预设', () => {
  const cols = ['a', 'b', 'c']
  it('裸列名逗号分隔', () => {
    expect(formatColumns('mysql', cols, 'plain')).toBe('a, b, c')
  })
  it('方言引号逗号分隔（含转义）', () => {
    expect(formatColumns('mysql', cols, 'quoted')).toBe('`a`, `b`, `c`')
    expect(formatColumns('postgresql', ['x"y'], 'quoted')).toBe('"x""y"')
    expect(formatColumns('oracle', ['A'], 'quoted')).toBe('"A"')
  })
  it('别名限定 t.列名（别名不引用）', () => {
    expect(formatColumns('mysql', cols, 'aliased')).toBe('t.a, t.b, t.c')
  })
  it('换行分隔', () => {
    expect(formatColumns('mysql', cols, 'lines')).toBe('a\nb\nc')
  })
})

describe('SqlGenBehavior 行为偏好（SQL 生成设置，SPEC-TREE-7）', () => {
  const cols = ['a', 'b', 'c']
  it('stripQualifier：生成语句不带库/模式前缀', () => {
    expect(
      buildSelectSql('mysql', { table: 'user', database: 'mydb', columns: ['id'], behavior: { stripQualifier: true } }),
    ).toBe('SELECT `id` FROM `user`;')
    expect(
      buildInsertSql('postgresql', { table: 'user', database: 'mydb', columns: ['id'], behavior: { stripQualifier: true } }),
    ).toBe('INSERT INTO "user" ("id") VALUES (?);')
  })

  it('identifierCase upper / lower（表名/列名/库名统一转换，关键字不受影响）', () => {
    expect(
      buildSelectSql('mysql', {
        table: 'user', database: 'mydb', columns: ['id', 'name'],
        behavior: { identifierCase: 'upper' },
      }),
    ).toBe('SELECT `ID`, `NAME` FROM `MYDB`.`USER`;')
    expect(
      buildSelectSql('oracle', { table: 'User', columns: ['Id'], behavior: { identifierCase: 'lower' } }),
    ).toBe('SELECT "id" FROM "user"')
    expect(
      buildInsertSql('oracle', { table: 'T1', columns: ['A', 'B'], behavior: { identifierCase: 'lower' } }),
    ).toBe('INSERT INTO "t1" ("a", "b") VALUES (:1, :2)')
  })

  it('formatColumns 接受行为偏好', () => {
    expect(formatColumns('mysql', cols, 'plain', { identifierCase: 'upper' })).toBe('A, B, C')
    expect(formatColumns('oracle', ['a'], 'quoted', { identifierCase: 'lower' })).toBe('"a"')
    expect(formatColumns('mysql', cols, 'aliased', { identifierCase: 'upper' })).toBe('t.A, t.B, t.C')
  })
})

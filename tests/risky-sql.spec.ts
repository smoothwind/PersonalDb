import { describe, expect, it } from 'vitest'
import { classifySql } from '../src/utils/riskySql'

const cls = (sql: string, kind: Parameters<typeof classifySql>[1] = 'mysql') => classifySql(sql, kind)

describe('classifySql 安全语句零打扰', () => {
  it('SELECT / INSERT / WITH 等 safe 不进 items', () => {
    expect(cls('SELECT * FROM t').level).toBe('safe')
    expect(cls('SELECT * FROM t').items).toHaveLength(0)
    expect(cls("INSERT INTO t (a) VALUES ('x')").level).toBe('safe')
    expect(cls('WITH x AS (SELECT 1) SELECT * FROM x').level).toBe('safe')
    expect(cls('CREATE TABLE t (a INT)').level).toBe('safe')
  })

  it('字符串字面值中的危险词不误报', () => {
    expect(cls("SELECT * FROM t WHERE note = 'delete from t; truncate u'").level).toBe('safe')
    expect(cls("UPDATE t SET note = 'DROP TABLE x' WHERE id = 1").level).toBe('safe')
  })

  it('注释中的 WHERE 不算数，字符串/注释剥离正确', () => {
    expect(cls('UPDATE t SET a = 1 /* WHERE */').level).toBe('warn')
    expect(cls('UPDATE t SET a = 1 -- where id=1\n').level).toBe('warn')
    expect(cls('UPDATE t SET a = 1 WHERE id = 1 -- 注释\n').level).toBe('safe')
  })
})

describe('classifySql 无 WHERE 的 DML（warn）', () => {
  it('UPDATE 无 WHERE → warn，提取目标表', () => {
    const v = cls('UPDATE users SET status = 1')
    expect(v.level).toBe('warn')
    expect(v.items[0].type).toBe('noWhereDml')
    expect(v.items[0].objects).toEqual(['users'])
  })

  it('DELETE 无 WHERE → warn；带 WHERE → safe；带 LIMIT → 豁免', () => {
    expect(cls('DELETE FROM logs').level).toBe('warn')
    expect(cls('DELETE FROM logs WHERE created_at < NOW()').level).toBe('safe')
    expect(cls('DELETE FROM logs LIMIT 100').level).toBe('safe')
  })

  it('限定名去引号：反引号 / 双引号 / 点号', () => {
    expect(cls('DELETE FROM `mydb`.`t`').items[0].objects).toEqual(['mydb.t'])
    expect(cls('DELETE FROM "sch"."t"', 'postgresql').items[0].objects).toEqual(['sch.t'])
  })

  it('sqlite UPDATE OR REPLACE 修饰词跳过', () => {
    expect(cls('UPDATE OR REPLACE t SET a = 1', 'sqlite').items[0].objects).toEqual(['t'])
  })

  it('多表 UPDATE（MySQL）取首表', () => {
    expect(cls('UPDATE a JOIN b ON a.id = b.id SET a.x = 1').items[0].objects).toEqual(['a'])
  })
})

describe('classifySql 破坏性 DDL（danger）', () => {
  it('DROP TABLE 单表 / 多表 / IF EXISTS', () => {
    expect(cls('DROP TABLE orders').items[0]).toMatchObject({ level: 'danger', objects: ['orders'] })
    expect(cls('DROP TABLE a, b').items[0].objects).toEqual(['a', 'b'])
    expect(cls('DROP TABLE IF EXISTS t1').items[0].objects).toEqual(['t1'])
    expect(cls('DROP TABLE "MyTable"', 'postgresql').items[0].objects).toEqual(['MyTable'])
  })

  it('TRUNCATE → danger（TABLE 关键字可省略）', () => {
    expect(cls('TRUNCATE TABLE logs').items[0]).toMatchObject({ level: 'danger', objects: ['logs'] })
    expect(cls('TRUNCATE logs').items[0].level).toBe('danger')
  })

  it('ALTER TABLE DROP COLUMN/PARTITION → danger；DROP CONSTRAINT → warn；普通 ALTER → safe', () => {
    expect(cls('ALTER TABLE t DROP COLUMN c').items[0]).toMatchObject({ level: 'danger', objects: ['c'] })
    expect(cls('ALTER TABLE t DROP PARTITION p20260101', 'oracle').items[0].level).toBe('danger')
    expect(cls('ALTER TABLE t DROP CONSTRAINT k', 'postgresql').items[0].level).toBe('warn')
    expect(cls('ALTER TABLE t ADD COLUMN c INT').level).toBe('safe')
  })

  it('库/模式概念按方言：mysql DATABASE=danger、oracle 下 warn；pg SCHEMA=danger', () => {
    expect(cls('DROP DATABASE app', 'mysql').items[0].level).toBe('danger')
    expect(cls('DROP DATABASE app', 'oracle').items[0].level).toBe('warn')
    expect(cls('DROP SCHEMA s CASCADE', 'postgresql').items[0].level).toBe('danger')
    expect(cls('DROP SCHEMA s', 'mysql').items[0].level).toBe('warn')
  })

  it('可重建对象 → warn（INDEX / VIEW / TRIGGER / SEQUENCE / PROCEDURE）', () => {
    expect(cls('DROP INDEX idx_a ON t').items[0]).toMatchObject({ level: 'warn', objects: ['idx_a'] })
    expect(cls('DROP VIEW v1').items[0].level).toBe('warn')
    expect(cls('DROP TRIGGER trg', 'oracle').items[0].level).toBe('warn')
    expect(cls('DROP SEQUENCE seq', 'oracle').items[0].level).toBe('warn')
    expect(cls('DROP PROCEDURE p', 'mysql').items[0].level).toBe('warn')
  })

  it('未登记的 DROP 对象类型保守按 warn', () => {
    expect(cls('DROP SOMETHING x').items[0].level).toBe('warn')
  })

  it('用户表名恰为保留字样式仍正确提取', () => {
    expect(cls('DROP TABLE `table`', 'mysql').items[0].objects).toEqual(['table'])
  })
})

describe('classifySql 多语句与逃生门', () => {
  it('多语句混合取最严级并列出风险清单', () => {
    const v = cls('SELECT 1; UPDATE t SET a = 1; DROP TABLE x;')
    expect(v.level).toBe('danger')
    expect(v.items).toHaveLength(2)
    expect(v.items.map((i) => i.type).sort()).toEqual(['destructiveDdl', 'noWhereDml'])
  })

  it('多语句含被豁免语句 → 不计风险仅计数', () => {
    const v = cls('SELECT 1; -- pdb-ignore\nUPDATE t SET a = 1;')
    expect(v.level).toBe('safe')
    expect(v.suppressed).toBe(1)
    expect(v.items).toHaveLength(0)
  })

  it('pdb-ignore 对 DROP 也生效', () => {
    const v = cls('-- pdb-ignore\nDROP TABLE x;')
    expect(v.level).toBe('safe')
    expect(v.suppressed).toBe(1)
  })
})

describe('classifySql Oracle 系整体检测（不分割）', () => {
  it('多语句文本里的 DROP TABLE 被块扫描兜住', () => {
    const v = cls('UPDATE t SET a = 1; DROP TABLE x;', 'oracle')
    expect(v.level).toBe('danger')
    expect(v.items[0].objects).toEqual(['x'])
  })

  it('PL/SQL 块中 EXECUTE IMMEDIATE 动态 DROP → danger', () => {
    const v = cls("BEGIN EXECUTE IMMEDIATE 'DROP TABLE tmp_t'; END;", 'oracle')
    expect(v.level).toBe('danger')
    expect(v.items[0].objects).toEqual(['tmp_t'])
  })

  it('普通 Oracle SELECT 含字面量不受块扫描影响', () => {
    expect(cls("SELECT 'DROP TABLE x' FROM dual", 'oracle').level).toBe('safe')
  })
})

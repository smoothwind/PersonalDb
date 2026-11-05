import { describe, expect, it } from 'vitest'
import { hasPlSqlBlock, splitStatements } from '../src/utils/sqlSplit'

describe('hasPlSqlBlock PL/SQL 块检测', () => {
  it('匿名块 BEGIN..END 命中', () => {
    expect(hasPlSqlBlock("BEGIN\n  DBMS_OUTPUT.PUT_LINE('x');\nEND;")).toBe(true)
  })

  it('DECLARE 块命中', () => {
    expect(hasPlSqlBlock('DECLARE v INT;\nBEGIN v := 1; END;')).toBe(true)
  })

  it('CREATE PROCEDURE / FUNCTION / TRIGGER / PACKAGE 命中', () => {
    expect(hasPlSqlBlock('CREATE OR REPLACE PROCEDURE p AS BEGIN NULL; END;')).toBe(true)
    expect(hasPlSqlBlock('CREATE FUNCTION f RETURN NUMBER IS BEGIN RETURN 1; END;')).toBe(true)
    expect(hasPlSqlBlock('CREATE OR REPLACE TRIGGER trg BEFORE INSERT ON t BEGIN NULL; END;')).toBe(true)
    expect(hasPlSqlBlock('CREATE PACKAGE BODY pkg AS BEGIN NULL; END;')).toBe(true)
    expect(hasPlSqlBlock('CREATE TYPE ty AS OBJECT (a INT);')).toBe(true)
  })

  it('普通单条 DDL/DML 不命中（走分号分割）', () => {
    expect(hasPlSqlBlock("COMMENT ON TABLE t IS 'x';")).toBe(false)
    expect(hasPlSqlBlock("COMMENT ON COLUMN t.ID IS 'y';")).toBe(false)
    expect(hasPlSqlBlock('UPDATE t SET a = 1 WHERE id = 1;')).toBe(false)
  })

  it('多条 COMMENT 不命中（此前 OB Oracle 整段发送触发 ORA-00600 的场景）', () => {
    const sql = "COMMENT ON TABLE t IS '表注释';\nCOMMENT ON COLUMN t.ID IS '主键';"
    expect(hasPlSqlBlock(sql)).toBe(false)
  })

  it('字符串字面值里的 BEGIN/END 不误判', () => {
    expect(hasPlSqlBlock("SELECT 'BEGIN x; END' FROM dual")).toBe(false)
    expect(hasPlSqlBlock("INSERT INTO t VALUES ('BEGIN')")).toBe(false)
  })

  it('注释里的 BEGIN 不误判', () => {
    expect(hasPlSqlBlock('UPDATE t SET a = 1 -- BEGIN\nWHERE id = 1')).toBe(false)
    expect(hasPlSqlBlock('/* BEGIN x END */ UPDATE t SET a = 1')).toBe(false)
  })
})

describe('splitStatements 与块检测协同（runTab Oracle 分支行为）', () => {
  it('不含块的多条语句分割为逐条（分号已剥离）', () => {
    const parts = splitStatements("COMMENT ON TABLE t IS 'x';\nCOMMENT ON COLUMN t.ID IS 'y';")
    expect(parts).toEqual(["COMMENT ON TABLE t IS 'x'", "COMMENT ON COLUMN t.ID IS 'y'"])
  })

  it('字符串内的分号不分割', () => {
    expect(splitStatements("INSERT INTO t VALUES ('a;b')")).toHaveLength(1)
  })

  it('hasPlSqlBlock 命中的文本应整段保留（不分割）', () => {
    const block = "BEGIN\n  UPDATE t SET a = 1;\n  UPDATE u SET b = 2;\nEND;"
    expect(hasPlSqlBlock(block)).toBe(true)
  })
})

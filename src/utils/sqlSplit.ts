/**
 * SQL 语句分割（字符串/注释感知）。
 * 注意：Oracle PL/SQL 块（BEGIN..END）内含分号，调用方应对含块文本跳过分割（见 hasPlSqlBlock）。
 */
export function splitStatements(sql: string): string[] {
  const out: string[] = []
  let cur = ''
  let quote: string | null = null
  let i = 0
  while (i < sql.length) {
    const ch = sql[i]
    if (quote) {
      cur += ch
      if (ch === quote) {
        // 双写转义（'' / ""）
        if ((quote === "'" || quote === '"') && sql[i + 1] === quote) {
          cur += sql[i + 1]
          i += 2
          continue
        }
        quote = null
      }
      i += 1
      continue
    }
    if (ch === "'" || ch === '"' || ch === '`') {
      quote = ch
      cur += ch
      i += 1
      continue
    }
    if (ch === '-' && sql[i + 1] === '-') {
      const end = sql.indexOf('\n', i)
      const stop = end === -1 ? sql.length : end
      cur += sql.slice(i, stop)
      i = stop
      continue
    }
    if (ch === '/' && sql[i + 1] === '*') {
      const end = sql.indexOf('*/', i + 2)
      const stop = end === -1 ? sql.length : end + 2
      cur += sql.slice(i, stop)
      i = stop
      continue
    }
    if (ch === ';') {
      out.push(cur)
      cur = ''
      i += 1
      continue
    }
    cur += ch
    i += 1
  }
  out.push(cur)
  return out.map((s) => s.trim()).filter(Boolean)
}

/** 剥离注释与字符串字面值内容（块检测用；字符串内的 BEGIN/END 不参与判定） */
function stripForBlockDetect(sql: string): string {
  const out: string[] = []
  let i = 0
  while (i < sql.length) {
    const ch = sql[i]
    if (ch === '-' && sql[i + 1] === '-') {
      const end = sql.indexOf('\n', i)
      i = end === -1 ? sql.length : end
      continue
    }
    if (ch === '/' && sql[i + 1] === '*') {
      const end = sql.indexOf('*/', i + 2)
      i = end === -1 ? sql.length : end + 2
      continue
    }
    if (ch === "'") {
      out.push(' ')
      i += 1
      while (i < sql.length) {
        if (sql[i] === "'") {
          if (sql[i + 1] === "'") {
            i += 2
            continue
          }
          i += 1
          break
        }
        i += 1
      }
      continue
    }
    out.push(ch)
    i += 1
  }
  return out.join('')
}

/**
 * 检测文本是否包含 PL/SQL 块（BEGIN..END / DECLARE / CREATE PROCEDURE 等存储对象定义）。
 * 命中时执行器须整段发送，不可按分号分割（对齐 DBeaver 的块识别规则，仅 Oracle 系调用）。
 */
export function hasPlSqlBlock(sql: string): boolean {
  const bare = stripForBlockDetect(sql)
  if (/\bBEGIN\b/i.test(bare) && /\bEND\b/i.test(bare)) return true
  if (/\bDECLARE\b/i.test(bare)) return true
  if (
    /\bCREATE\s+(?:OR\s+REPLACE\s+)?(?:PROCEDURE|FUNCTION|TRIGGER|PACKAGE|TYPE)\b/i.test(bare)
  ) {
    return true
  }
  return false
}

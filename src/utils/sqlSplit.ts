/**
 * SQL 语句分割（字符串/注释感知）。
 * 注意：Oracle PL/SQL 块（BEGIN..END）内含分号，调用方应对 Oracle 类连接跳过分割。
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

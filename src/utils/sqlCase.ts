/**
 * 大小写转换：仅转换字符串字面值之外的部分。
 * 支持 ' " ` 三种引号，'' / "" 转义视为字面值内。
 * 用于 SQL / PL/SQL 的大小写转换，避免修改字面值与变量值。
 */
export function convertOutsideLiterals(s: string, mode: 'upper' | 'lower'): string {
  let out = ''
  let i = 0
  while (i < s.length) {
    const ch = s[i]
    if (ch === "'" || ch === '"' || ch === '`') {
      out += ch
      i += 1
      while (i < s.length) {
        out += s[i]
        if (s[i] === ch) {
          if (s[i + 1] === ch) {
            out += s[i + 1]
            i += 2
            continue
          }
          i += 1
          break
        }
        i += 1
      }
    } else {
      out += mode === 'upper' ? ch.toUpperCase() : ch.toLowerCase()
      i += 1
    }
  }
  return out
}

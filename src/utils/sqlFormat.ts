import type { DbKind } from '../types/db'
import { beautifySql, type BeautifyOptions } from './plsqlBeautifier'

export type FormatOptions = BeautifyOptions

/** 按 PL/SQL Beautifier 规则美化 SQL（对齐 PL/SQL Developer `.br`） */
export function formatSql(_kind: DbKind, sql: string, opts: BeautifyOptions): string {
  return beautifySql(sql, opts)
}

function isIdentChar(c: string | undefined): boolean {
  return !!c && /[A-Za-z0-9_$]/.test(c)
}

/**
 * 压缩 SQL：去除注释（`--`、`#`、`/* *∕`）与多余空白，折叠为单行。
 * 字符串字面值（'...'、"..."、`...`，含 '' 转义）内容保持原样。
 */
export function compressSql(sql: string): string {
  let out = ''
  let i = 0
  const n = sql.length
  let pendingSpace = false
  while (i < n) {
    const ch = sql[i]
    // 字符串字面值：原样保留
    if (ch === "'" || ch === '"' || ch === '`') {
      if (pendingSpace && isIdentChar(out[out.length - 1])) out += ' '
      pendingSpace = false
      out += ch
      i += 1
      while (i < n) {
        out += sql[i]
        if (sql[i] === ch) {
          if (sql[i + 1] === ch) {
            out += sql[i + 1]
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
    // 行注释（-- 与 #）：跳过至行尾
    if ((ch === '-' && sql[i + 1] === '-') || ch === '#') {
      while (i < n && sql[i] !== '\n') i += 1
      pendingSpace = true
      continue
    }
    // 块注释：跳过并视为空白
    if (ch === '/' && sql[i + 1] === '*') {
      i += 2
      while (i < n && !(sql[i] === '*' && sql[i + 1] === '/')) i += 1
      i += 2
      pendingSpace = true
      continue
    }
    // 空白折叠
    if (/\s/.test(ch)) {
      pendingSpace = true
      i += 1
      continue
    }
    // 标识符之间需要保留一个空格，其余紧贴
    if (pendingSpace) {
      if (isIdentChar(out[out.length - 1]) && isIdentChar(ch)) out += ' '
      pendingSpace = false
    }
    out += ch
    i += 1
  }
  return out.trim()
}

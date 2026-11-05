import type { DbKind } from '../types/db'

export interface DdlCleanOptions {
  /** 去除对象属主/schema 前缀（"OWNER". / `schema`. / [dbo]. / SCHEMA.） */
  stripSchema?: boolean
  /** 去除标识符引号（" / ` / []） */
  stripQuote?: boolean
  /** 去除存储相关子句（STORAGE/TABLESPACE/PCTFREE/ENGINE/CHARSET 等） */
  stripStorage?: boolean
  /** @deprecated 兼容旧调用：= stripSchema + stripQuote */
  stripQualifier?: boolean
}

/** 清洗 DDL：按选项去除 schema 前缀/标识符引号/存储子句，并归一空白 */
export function cleanDdl(ddl: string, kind: DbKind, opts: DdlCleanOptions): string {
  let s = ddl
  const stripSchema = opts.stripSchema || opts.stripQualifier || false
  const stripQuote = opts.stripQuote || opts.stripQualifier || false
  if (stripSchema) s = stripSchemaPrefix(s)
  if (stripQuote) s = stripQuoteMarks(s)
  if (opts.stripStorage) s = stripStorage(s, kind)
  return s
    .replace(/[ \t]+\n/g, '\n')
    .replace(/[ \t]{2,}/g, ' ')
    .replace(/\s+;/g, ';')
    .replace(/\(\s+/g, '(')
    .replace(/\s+\)/g, ')')
    .replace(/\n{3,}/g, '\n\n')
    .trim()
}

/** 去除属主/schema 前缀（"OWNER". / `schema`. / [dbo]. / 未引用 SCHEMA.） */
export function stripSchemaPrefix(s: string): string {
  return s
    .replace(/"[^"]+"\./g, '') // "OWNER".
    .replace(/`[^`]+`\./g, '') // `schema`.
    .replace(/\[[^\]]+\]\./g, '') // [schema].
    // 未加引号的 schema 前缀（任意大小写）：word.
    .replace(/(^|[\s(,])([A-Za-z_][\w$]*)\.(?=[A-Za-z_"`[\s])/g, '$1')
}

/** 去除标识符引号（反引号 / 方括号 / 双引号） */
export function stripQuoteMarks(s: string): string {
  return s
    .replace(/[`"]/g, '')
    .replace(/\[([^\]]+)\]/g, '$1')
}

/**
 * 去除限定符（schema 前缀 + 标识符引号）。
 * @deprecated 由 stripSchemaPrefix / stripQuoteMarks 取代（两者可独立开关，SPEC-TREE-7）
 */
export function stripQualifier(s: string): string {
  return stripQuoteMarks(stripSchemaPrefix(s))
}

export function stripStorage(s: string, kind: DbKind): string {
  let out = s
  out = out.replace(/STORAGE\s*\([^)]*\)/gis, '')
  out = out.replace(/\bTABLESPACE\s+[^\s,;)]+/gi, '')
  out = out.replace(/\b(?:PCTFREE|PCTUSED|INITRANS|MAXTRANS|BUFFER_POOL|FILLFACTOR)\s+\S+/gi, '')
  out = out.replace(/\bSEGMENT\s+CREATION\s+(?:IMMEDIATE|DEFERRED)/gi, '')
  out = out.replace(
    /\b(?:NOLOGGING|LOGGING|NOMONITORING|MONITORING|NOCACHE|CACHE|NOPARALLEL|PARALLEL(?:\s+\d+)?|NOCOMPRESS|COMPRESS)\b/gi,
    '',
  )
  out = out.replace(/\b(?:ENABLE|DISABLE)\s+ROW\s+MOVEMENT/gi, '')
  if (kind === 'mysql' || kind === 'oceanbase') {
    out = out.replace(/\s*ENGINE=\w+/gi, '')
    out = out.replace(/\s*(?:DEFAULT\s+)?CHARSET=\w+/gi, '')
    out = out.replace(/\s*COLLATE=\w+/gi, '')
    out = out.replace(/\s*AUTO_INCREMENT=\d+/gi, '')
    out = out.replace(/\s*ROW_FORMAT=\w+/gi, '')
  }
  return out
}

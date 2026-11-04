import { splitStatements } from './sqlSplit'
import type { DbKind } from '../types/db'

/**
 * 危险 SQL 检测器（SPEC-SAFE-1）。
 *
 * 等级：
 * - danger  数据不可恢复：DROP TABLE/USER/TABLESPACE + 方言附加（库/模式概念）、TRUNCATE、
 *           ALTER TABLE DROP COLUMN/PARTITION
 * - warn    无 WHERE（且无 LIMIT）的 UPDATE/DELETE；其余 DROP 已知对象（可由源码重建）
 * - safe    其余（含字符串字面值中的「危险词」——已剥离，不误报）
 *
 * 逃生门：语句内任意位置写 `-- pdb-ignore` 行注释 → 该语句跳过检测。
 * Oracle 系连接不分割语句（PL/SQL 块含分号）→ 整体检测，并对 BEGIN/DECLARE 块
 * 额外扫描原始文本中的动态 SQL（EXECUTE IMMEDIATE 'DROP TABLE …'）。
 */

export type RiskLevel = 'safe' | 'warn' | 'danger'
export type RiskType = 'noWhereDml' | 'destructiveDdl'

export interface RiskItem {
  /** 原语句（展示截断由调用方处理） */
  statement: string
  level: RiskLevel
  type: RiskType
  /** 涉及对象名（去引号；UPDATE/DELETE = 目标表，DROP/TRUNCATE = 对象清单） */
  objects: string[]
}

export interface RiskVerdict {
  /** 多语句取最严级 */
  level: RiskLevel
  /** 仅风险语句（safe 不进） */
  items: RiskItem[]
  /** 被 pdb-ignore 抑制的风险语句数 */
  suppressed: number
}

const ORACLE_KINDS: DbKind[] = ['oracle', 'oceanbase-oracle']

/** 全方言通行的破坏性 DDL 对象类型（数据不可恢复） */
const DDL_DANGER_ALWAYS = new Set(['TABLE', 'USER', 'TABLESPACE'])

/** 方言附加的破坏性对象类型（库/模式概念按 kind 有无） */
const DDL_DANGER_BY_KIND: Partial<Record<DbKind, string[]>> = {
  mysql: ['DATABASE'],
  oceanbase: ['DATABASE'],
  postgresql: ['DATABASE', 'SCHEMA'],
  gaussdb: ['DATABASE', 'SCHEMA'],
  jdbc: ['DATABASE', 'SCHEMA'],
  sqlite: [],
  oracle: [],
  'oceanbase-oracle': [],
}

/** 其余已知 DDL 对象类型 → warn（可由源码重建） */
const DDL_WARN_WORDS = new Set([
  'VIEW', 'INDEX', 'SEQUENCE', 'TRIGGER', 'PROCEDURE', 'FUNCTION', 'PACKAGE',
  'SYNONYM', 'ROLE', 'PROFILE', 'MATERIALIZED', 'TYPE', 'DOMAIN', 'SCHEMA',
  'DATABASE', 'LINK', 'DIRECTORY', 'EVENT', 'SERVER', 'WRAPPER', 'EXTENSION',
  'OPERATOR', 'PUBLICATION', 'SUBSCRIPTION', 'CLUSTER', 'CONVERSION', 'RULE',
])

/** UPDATE 语句首部的忽略词（提示词/方言修饰，非表名） */
const UPDATE_HINTS = new Set([
  'IGNORE', 'LOW_PRIORITY', 'ONLY', 'QUICK', 'OR', 'ROLLBACK', 'ABORT',
  'REPLACE', 'FAIL',
])

/** 对象名去引号（"a"、`a`、[a] → a） */
function unquote(name: string): string {
  return name.replace(/^["`[]/, '').replace(/["`\]]$/, '')
}

/**
 * 剥离注释与字符串字面值内容（双引号/反引号标识符保留），折叠空白。
 * 单引号内容整体抹掉（含 '' 转义），防止 'delete from t' 字面值误报；
 * 双引号/反引号按标识符保留（引号内可能就是对象名）。
 */
function stripForDetect(sql: string): string {
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
  return out.join('').replace(/\s+/g, ' ').trim()
}

/** 从 bare 的 from 位置起抓取一个（可限定的）标识符，返回去引号名；自动跳过前导空白 */
function nextIdent(bare: string, from: number): { name: string; end: number } | null {
  const rest = bare.slice(from)
  const lead = /^\s*/.exec(rest)?.[0].length ?? 0
  const re = /(?:"([^"]*)"|`([^`]*)`|\[([^\]]*)\]|[A-Za-z_][\w$#]*)((?:\s*\.\s*(?:"[^"]*"|`[^`]*`|\[[^\]]*\]|[A-Za-z_][\w$#]*))*)/
  const m = re.exec(rest.slice(lead))
  if (!m || m.index !== 0) return null
  const head = m[1] ?? m[2] ?? m[3] ?? m[0].match(/^[A-Za-z_][\w$#]*/)?.[0] ?? m[0]
  const tail = (m[4] ?? '').replace(/\s+/g, '')
  const qualified = tail
    .split('.')
    .filter((seg) => seg.length > 0)
    .map((seg) => unquote(seg))
    .join('.')
  return { name: unquote(head) + (qualified ? `.${qualified}` : ''), end: from + lead + m[0].length }
}

/** 跳过标识符前的修饰词与空白（如 sqlite UPDATE OR REPLACE、DROP TABLE IF EXISTS 里的 IF/EXISTS） */
function skipWords(bare: string, pos: number, words: Set<string>): number {
  let p = pos
  for (;;) {
    const ws = /^\s+/.exec(bare.slice(p))
    if (ws) p += ws[0].length
    const m = /^[A-Za-z_]+/.exec(bare.slice(p))
    if (!m || !words.has(m[0].toUpperCase())) return p
    p += m[0].length
  }
}

/** DROP/TRUNCATE/UPDATE/DELETE 后的对象名清单：按逗号切段，取每段首个标识符 */
function collectObjects(bare: string, from: number): string[] {
  const rest = bare.slice(from)
  const names: string[] = []
  for (const part of rest.split(',')) {
    let p = part.search(/\S/)
    if (p < 0) continue
    p = skipWords(part, p, new Set(['IF', 'EXISTS', 'ONLY', 'TABLE', 'TEMPORARY', 'TEMP', 'GLOBAL', 'LOCAL']))
    const ident = nextIdent(part, p)
    if (ident) names.push(ident.name)
  }
  return [...new Set(names)]
}

/** 单语句分析；safe 返回 null */
function analyzeOne(stmt: string, kind: DbKind): RiskItem | null {
  const bare = stripForDetect(stmt)
  if (!bare) return null
  const lead = /^[A-Za-z]+/.exec(bare)?.[0]?.toUpperCase() ?? ''

  // Oracle 系不分割：仅当为 PL/SQL 块（BEGIN/DECLARE）或多语句拼接（含分号）时，
  // 才对原始文本整体扫描破坏性关键字（覆盖 EXECUTE IMMEDIATE 'DROP TABLE …' 等动态 SQL；
  // 普通 SELECT 的字符串字面量不误报）
  if (ORACLE_KINDS.includes(kind)) {
    const leadWord = /^[A-Za-z]+/.exec(bare)?.[0]?.toUpperCase() ?? ''
    const isBlockish = leadWord === 'BEGIN' || leadWord === 'DECLARE' || stmt.includes(';')
    if (isBlockish) {
      const blockHit = /\b(TRUNCATE\s+TABLE|DROP\s+TABLE|DROP\s+USER|DROP\s+SCHEMA|DROP\s+TABLESPACE)\b/i.exec(stmt)
      if (blockHit) {
        const mObj = /\s+([A-Za-z_][\w$#]*|"[^"]+"|`[^`]+`|\[[^\]]+\])/.exec(
          stmt.slice((blockHit.index ?? 0) + blockHit[0].length),
        )
        return {
          statement: stmt,
          level: 'danger',
          type: 'destructiveDdl',
          objects: mObj ? [unquote(mObj[1])] : [],
        }
      }
    }
  }

  const hasWhere = /\bWHERE\b/i.test(bare)
  const hasLimit = /\bLIMIT\b/i.test(bare)

  if (lead === 'UPDATE') {
    if (!hasWhere && !hasLimit) {
      const p = skipWords(bare, 'UPDATE'.length, UPDATE_HINTS)
      const ident = nextIdent(bare, p)
      return { statement: stmt, level: 'warn', type: 'noWhereDml', objects: ident ? [ident.name] : [] }
    }
    return null
  }

  if (lead === 'DELETE') {
    const mFrom = /\bFROM\b/i.exec(bare)
    if (!hasWhere && !hasLimit) {
      let objects: string[] = []
      if (mFrom) {
        const p = skipWords(bare, mFrom.index + mFrom[0].length, new Set(['ONLY']))
        const ident = nextIdent(bare, p)
        if (ident) objects = [ident.name]
      }
      return { statement: stmt, level: 'warn', type: 'noWhereDml', objects }
    }
    return null
  }

  if (lead === 'TRUNCATE') {
    const after = skipWords(bare, 'TRUNCATE'.length, new Set(['TABLE', 'ONLY']))
    const ident = nextIdent(bare, after)
    return { statement: stmt, level: 'danger', type: 'destructiveDdl', objects: ident ? [ident.name] : [] }
  }

  if (lead === 'DROP') {
    const mWord = /\b([A-Za-z_]+)\b/.exec(bare.slice('DROP'.length).trimStart())
    const word = mWord?.[1]?.toUpperCase() ?? ''
    const danger = DDL_DANGER_ALWAYS.has(word) || (DDL_DANGER_BY_KIND[kind] ?? []).includes(word)
    if (!danger && !DDL_WARN_WORDS.has(word)) {
      // 未登记的 DROP 对象类型：按 warn 保守提示
      return { statement: stmt, level: 'warn', type: 'destructiveDdl', objects: [] }
    }
    const mType = new RegExp(`\\b${word}\\b`, 'i').exec(bare)
    const objects = collectObjects(bare, (mType?.index ?? 0) + word.length)
    return {
      statement: stmt,
      level: danger ? 'danger' : 'warn',
      type: 'destructiveDdl',
      objects,
    }
  }

  if (lead === 'ALTER' && /\bTABLE\b/i.test(bare) && /\bDROP\b/i.test(bare)) {
    const mCol = /\bDROP\s+(COLUMN|PARTITION|SUBPARTITION|PARTITIONS)\b/i.exec(bare)
    if (mCol) {
      const after = bare.slice((mCol.index ?? 0) + mCol[0].length)
      const ident = nextIdent(after.trimStart(), 0)
      return {
        statement: stmt,
        level: 'danger',
        type: 'destructiveDdl',
        objects: ident ? [ident.name] : [],
      }
    }
    // DROP CONSTRAINT/INDEX/PRIMARY 等：可重建，warn
    return { statement: stmt, level: 'warn', type: 'destructiveDdl', objects: [] }
  }

  return null
}

/** 整体分类：Oracle 系不分割整体检测，其余按语句分割逐条判定 */
export function classifySql(sql: string, kind: DbKind): RiskVerdict {
  const stmts = ORACLE_KINDS.includes(kind) ? [sql] : splitStatements(sql)
  const items: RiskItem[] = []
  let suppressed = 0
  for (const stmt of stmts) {
    const hit = analyzeOne(stmt, kind)
    if (!hit) continue
    if (/--\s*pdb-ignore/i.test(stmt)) {
      suppressed += 1
      continue
    }
    items.push(hit)
  }
  const level: RiskLevel = items.some((i) => i.level === 'danger')
    ? 'danger'
    : items.length
      ? 'warn'
      : 'safe'
  return { level, items, suppressed }
}

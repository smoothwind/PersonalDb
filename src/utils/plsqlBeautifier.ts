/**
 * PL/SQL Beautifier 引擎（对齐 PL/SQL Developer 的 Beautifier 规则 `.br`）。
 *
 * 覆盖规则：
 * - KeywordCase / IdentifierCase（0 不变 / 1 大写 / 2 小写 / 3 首字母大写）
 * - ItemList Format（0 同行 / 1 尽量同行 / 2 每项一行 / 3 自适应）、Align、CommaAfter、AtLeftMargin
 * - RightMargin、Indent、UseTab/TabSize
 * - SplitAndOr / AndOrAfterExpression / WhereAndOrUnderWhere
 * - JoinSplitBeforeOn、OnOneLineIfPossible、LeftAlignKeywords/Items
 * - EmptyLines（语句间空行）
 *
 * 说明：面向 DML（SELECT/INSERT/UPDATE/DELETE）与常见 PL/SQL 语句；
 * 复杂控制结构（IF/LOOP 等）按通用换行处理，未做逐条规则对齐。
 */

export interface BeautifyOptions {
  rightMargin: number
  indent: number
  useTab: boolean
  tabSize: number
  keywordCase: CaseMode
  identifierCase: CaseMode
  emptyLines: 0 | 1 | 2
  onOneLineIfPossible: boolean
  selectItemFormat: ItemFormat
  selectItemAlign: boolean
  selectCommaAfter: boolean
  whereSplitAndOr: boolean
  andOrAfterExpression: boolean
  whereAndOrUnderWhere: boolean
  joinSplitBeforeOn: boolean
  /** THEN/ELSE/ELSIF 换行 */
  thenOnNewLine: boolean
  /** LOOP 换行 */
  loopOnNewLine: boolean
  /** 对齐变量声明组 */
  alignDeclarationGroups: boolean
  /** 对齐赋值组 */
  alignAssignmentGroups: boolean
}

export type CaseMode = 0 | 1 | 2 | 3
/** 0 同行 / 1 尽量同行 / 2 每项一行 / 3 自适应 */
export type ItemFormat = 0 | 1 | 2 | 3

type TokType = 'word' | 'string' | 'qident' | 'num' | 'punct' | 'comment'
interface Tok {
  t: TokType
  v: string
}

const KEYWORDS = new Set(
  (
    'SELECT FROM WHERE GROUP BY ORDER HAVING INSERT INTO VALUES UPDATE SET DELETE ' +
    'LEFT RIGHT INNER OUTER FULL CROSS JOIN ON AND OR NOT NULL IS IN LIKE BETWEEN EXISTS ' +
    'UNION ALL MINUS INTERSECT DISTINCT AS ASC DESC CASE WHEN THEN ELSE END ' +
    'CREATE ALTER DROP TABLE VIEW INDEX SEQUENCE TRIGGER PROCEDURE FUNCTION PACKAGE BODY ' +
    'BEGIN DECLARE IF ELSIF LOOP WHILE FOR RETURN RETURNING WITH OVER PARTITION ' +
    'PRIMARY KEY FOREIGN REFERENCES CONSTRAINT DEFAULT CHECK UNIQUE GRANT REVOKE ' +
    'COMMIT ROLLBACK SAVEPOINT MERGE USING MATCHED ' +
    'NUMBER VARCHAR VARCHAR2 NVARCHAR2 CHAR NCHAR DATE TIMESTAMP INTERVAL CLOB BLOB NCLOB BFILE ' +
    'INT INTEGER SMALLINT BIGINT FLOAT REAL DOUBLE DECIMAL NUMERIC BOOLEAN ROWID'
  ).split(/\s+/),
)

const JOIN_WORDS = new Set(['LEFT', 'RIGHT', 'INNER', 'OUTER', 'FULL', 'CROSS'])
const CLAUSE_START = new Set([
  'FROM', 'WHERE', 'GROUP', 'ORDER', 'HAVING', 'CONNECT', 'START', 'UNION', 'MINUS', 'INTERSECT',
])
const LOGIC = new Set(['AND', 'OR'])

function pad(unit: string, level: number): string {
  return unit.repeat(Math.max(0, level))
}

function applyCase(word: string, mode: CaseMode): string {
  if (mode === 0) return word
  if (mode === 1) return word.toUpperCase()
  if (mode === 2) return word.toLowerCase()
  return word.charAt(0).toUpperCase() + word.slice(1).toLowerCase()
}

/** 词法分析：保留字符串/引号标识/注释 */
function tokenize(sql: string): Tok[] {
  const toks: Tok[] = []
  let i = 0
  const n = sql.length
  while (i < n) {
    const c = sql[i]
    if (/\s/.test(c)) {
      i++
      continue
    }
    // 行注释
    if (c === '-' && sql[i + 1] === '-') {
      const end = sql.indexOf('\n', i)
      toks.push({ t: 'comment', v: sql.slice(i, end < 0 ? n : end) })
      i = end < 0 ? n : end
      continue
    }
    // 块注释
    if (c === '/' && sql[i + 1] === '*') {
      const end = sql.indexOf('*/', i + 2)
      const stop = end < 0 ? n : end + 2
      toks.push({ t: 'comment', v: sql.slice(i, stop) })
      i = stop
      continue
    }
    // 字符串
    if (c === "'") {
      let j = i + 1
      while (j < n) {
        if (sql[j] === "'" && sql[j + 1] === "'") {
          j += 2
          continue
        }
        if (sql[j] === "'") {
          j++
          break
        }
        j++
      }
      toks.push({ t: 'string', v: sql.slice(i, j) })
      i = j
      continue
    }
    // 引号标识符（"..." 或 `...`）
    if (c === '"' || c === '`') {
      const q = c
      let j = i + 1
      while (j < n) {
        if (sql[j] === q && sql[j + 1] === q) {
          j += 2
          continue
        }
        if (sql[j] === q) {
          j++
          break
        }
        j++
      }
      toks.push({ t: 'qident', v: sql.slice(i, j) })
      i = j
      continue
    }
    // 数字
    if (/[0-9]/.test(c)) {
      let j = i
      while (j < n && /[0-9a-fA-FxX.]/.test(sql[j])) j++
      toks.push({ t: 'num', v: sql.slice(i, j) })
      i = j
      continue
    }
    // 标识符/单词
    if (/[A-Za-z_\u4e00-\u9fa5]/.test(c)) {
      let j = i
      while (j < n && /[\w$\u4e00-\u9fa5]/.test(sql[j])) j++
      toks.push({ t: 'word', v: sql.slice(i, j) })
      i = j
      continue
    }
    // 多字符运算符
    const two = sql.substr(i, 2)
    if (['<=', '>=', '<>', '!=', ':=', '||', '=>'].includes(two)) {
      toks.push({ t: 'punct', v: two })
      i += 2
      continue
    }
    toks.push({ t: 'punct', v: c })
    i++
  }
  return toks
}

/** 按顶层分号拆分语句（保留分号）；识别 BEGIN/END、IF/LOOP/CASE 块，块内分号不拆分 */
function splitStatements(tokens: Tok[]): Tok[][] {
  const out: Tok[][] = []
  let cur: Tok[] = []
  let paren = 0
  let block = 0
  let prevEnd = false
  const SECONDARY_OPENERS = new Set(['LOOP', 'IF', 'CASE'])
  let inDeclare = false
  for (const t of tokens) {
    if (t.v === '(') paren++
    else if (t.v === ')') paren = Math.max(0, paren - 1)
    const up = t.t === 'word' ? t.v.toUpperCase() : ''
    if (up === 'DECLARE') {
      block++
      inDeclare = true
      prevEnd = false
    } else if (up === 'BEGIN') {
      if (inDeclare) inDeclare = false
      else block++
      prevEnd = false
    } else if (up === 'END') {
      block = Math.max(0, block - 1)
      prevEnd = true
    } else if (prevEnd && SECONDARY_OPENERS.has(up)) {
      prevEnd = false
    } else {
      if (SECONDARY_OPENERS.has(up)) block++
      prevEnd = false
    }
    cur.push(t)
    if (t.v === ';' && paren === 0 && block === 0) {
      out.push(cur)
      cur = []
    }
  }
  if (cur.length) out.push(cur)
  return out
}

/** 应用大小写（保留字符串/引号标识/注释） */
function applyCaseTokens(tokens: Tok[], o: BeautifyOptions): Tok[] {
  return tokens.map((t) => {
    if (t.t === 'word') {
      if (KEYWORDS.has(t.v.toUpperCase())) return { ...t, v: applyCase(t.v, o.keywordCase) }
      return { ...t, v: applyCase(t.v, o.identifierCase) }
    }
    return t
  })
}

function joinTokens(tokens: Tok[]): string {
  let s = ''
  let prev: Tok | null = null
  for (const t of tokens) {
    if (prev) {
      const noSpaceAfter = prev.v === '(' || prev.v === '.' || t.v === ')' || t.v === ',' || t.v === '.' || t.v === ';'
      const noSpaceBefore = t.v === '(' && (prev.t === 'word')
      if (!noSpaceAfter && !noSpaceBefore) s += ' '
    }
    s += t.v
    prev = t
  }
  return s
}

/** 收集顶层逗号分隔的元素 */
function splitItems(tokens: Tok[]): Tok[][] {
  const items: Tok[][] = []
  let cur: Tok[] = []
  let depth = 0
  for (const t of tokens) {
    if (t.v === '(') depth++
    else if (t.v === ')') depth = Math.max(0, depth - 1)
    if (t.v === ',' && depth === 0) {
      items.push(cur)
      cur = []
      continue
    }
    cur.push(t)
  }
  if (cur.length) items.push(cur)
  return items
}

/** 渲染元素列表（SELECT/SET/GROUP BY 等共用） */
function renderItemList(
  head: string,
  items: Tok[][],
  o: BeautifyOptions,
  unit: string,
  baseLevel: number,
  alignCol: number,
): string[] {
  const texts = items.map(joinTokens)
  const single = `${head} ${texts.join(', ')}`
  const fits = single.length <= o.rightMargin
  const oneLine = o.selectItemFormat === 0 || (o.selectItemFormat === 1 && fits) || (o.selectItemFormat === 3 && fits)
  if (oneLine) return [single]

  const itemIndent = o.selectItemAlign
    ? pad(' ', alignCol)
    : pad(unit, baseLevel + 1)
  const lines: string[] = []
  texts.forEach((tx, idx) => {
    if (idx === 0) {
      lines.push(`${head} ${tx}`)
    } else {
      lines.push(`${itemIndent}${tx}`)
    }
  })
  // 逗号位置
  if (o.selectCommaAfter) {
    for (let k = 0; k < lines.length - 1; k++) lines[k] += ','
  } else {
    for (let k = 1; k < lines.length; k++) lines[k] = lines[k].replace(/^(\s*)/, '$1,')
  }
  return lines
}

function renderSelect(tokens: Tok[], o: BeautifyOptions, unit: string): string[] {
  // 找到首个 FROM
  let fromIdx = -1
  let depth = 0
  for (let i = 1; i < tokens.length; i++) {
    const t = tokens[i]
    if (t.v === '(') depth++
    else if (t.v === ')') depth = Math.max(0, depth - 1)
    else if (depth === 0 && t.v.toUpperCase() === 'FROM') {
      fromIdx = i
      break
    }
  }
  const selectItems = splitItems(tokens.slice(1, fromIdx < 0 ? tokens.length : fromIdx))
  const lines = renderItemList(tokens[0].v.toUpperCase(), selectItems, o, unit, 0, 'SELECT '.length)

  if (fromIdx < 0) return lines

  const rest = tokens.slice(fromIdx)
  // 按顶层子句切分
  const clauses: { kw: string; toks: Tok[] }[] = []
  let cur: Tok[] = []
  let curKw = ''
  let pendingJoin = false
  depth = 0
  for (const t of rest) {
    if (t.v === '(') depth++
    else if (t.v === ')') depth = Math.max(0, depth - 1)
    const up = t.v.toUpperCase()
    const isJoinWord = JOIN_WORDS.has(up)
    const isJoinKw = up === 'JOIN'
    let startClause = depth === 0 && CLAUSE_START.has(up)
    if (depth === 0 && (isJoinWord || isJoinKw)) {
      if (isJoinWord) {
        startClause = true
        pendingJoin = true
      } else if (isJoinKw) {
        // JOIN 紧跟 LEFT/RIGHT/... 时同一子句
        startClause = !pendingJoin
        pendingJoin = false
      }
    } else {
      pendingJoin = false
    }
    if (startClause && cur.length) {
      clauses.push({ kw: curKw, toks: cur })
      cur = []
      curKw = up
    } else if (startClause && !cur.length) {
      curKw = up
    }
    cur.push(t)
  }
  if (cur.length) clauses.push({ kw: curKw, toks: cur })

  for (const cl of clauses) {
    lines.push(...renderClause(cl, o, unit))
  }
  return lines
}

function renderClause(cl: { kw: string; toks: Tok[] }, o: BeautifyOptions, unit: string): string[] {
  const kw = cl.kw
  const toks = cl.toks
  const text = joinTokens(toks)
  if (kw === 'UNION' || kw === 'MINUS' || kw === 'INTERSECT') {
    const selIdx = toks.findIndex((t) => t.v.toUpperCase() === 'SELECT')
    if (selIdx > 0) {
      return [joinTokens(toks.slice(0, selIdx)), joinTokens(toks.slice(selIdx))]
    }
    return [text]
  }
  if (kw === 'WHERE') {
    return renderWhere(toks, o, unit)
  }
  if (JOIN_WORDS.has(kw) || kw === 'JOIN') {
    // 连接子句：整段（含 ON）按需拆行
    if (o.joinSplitBeforeOn) {
      const onIdx = toks.findIndex((t) => t.v.toUpperCase() === 'ON')
      if (onIdx > 0) {
        const head = joinTokens(toks.slice(0, onIdx))
        const onPart = joinTokens(toks.slice(onIdx))
        return [head, `${pad(unit, 1)}${onPart}`]
      }
    }
    return [text]
  }
  if (kw === 'GROUP' || kw === 'ORDER' || kw === 'CONNECT' || kw === 'START') {    // GROUP BY / ORDER BY：元素列表
    const byIdx = toks.findIndex((t) => t.v.toUpperCase() === 'BY')
    if (byIdx >= 0) {
      const head = `${toks[0].v.toUpperCase()} ${toks[1].v.toUpperCase()}`
      const items = splitItems(toks.slice(byIdx + 1))
      return renderItemList(head, items, { ...o, selectItemAlign: false }, unit, 0, head.length + 1)
    }
  }
  // 其他子句：整行
  return [text]
}

function renderWhere(toks: Tok[], o: BeautifyOptions, unit: string): string[] {
  // toks[0] = WHERE
  const head = toks[0].v.toUpperCase()
  const cond = toks.slice(1)
  if (!o.whereSplitAndOr) return [`${head} ${joinTokens(cond)}`]

  // 按顶层 AND/OR 拆分（op 记录该段「之前」的逻辑操作符）
  const parts: { op: string; toks: Tok[] }[] = []
  let depth = 0
  let cur: Tok[] = []
  let prevOp = ''
  for (const t of cond) {
    if (t.v === '(') depth++
    else if (t.v === ')') depth = Math.max(0, depth - 1)
    const up = t.v.toUpperCase()
    if (depth === 0 && LOGIC.has(up)) {
      parts.push({ op: prevOp, toks: cur })
      cur = []
      prevOp = up
      continue
    }
    cur.push(t)
  }
  parts.push({ op: prevOp, toks: cur })

  const indent = o.whereAndOrUnderWhere ? ' '.repeat(head.length + 1) : pad(unit, 1)
  const lines: string[] = []
  parts.forEach((p, i) => {
    const body = joinTokens(p.toks)
    if (i === 0) {
      lines.push(`${head} ${body}`)
    } else if (o.andOrAfterExpression) {
      lines[lines.length - 1] += ` ${p.op}`
      lines.push(`${indent}${body}`)
    } else {
      lines.push(`${indent}${p.op} ${body}`)
    }
  })
  return lines
}

function renderStatement(tokens: Tok[], o: BeautifyOptions): string[] {
  if (tokens.length === 0) return []
  const unit = o.useTab ? '\t' : ' '.repeat(o.indent)
  const first = tokens[0].v.toUpperCase()
  let lines: string[] = []
  if (first === 'SELECT' || first === 'WITH') {
    lines = first === 'SELECT' ? renderSelect(tokens, o, unit) : [joinTokens(tokens)]
  } else {
    // 非 SELECT：在顶层子句关键字前换行（左对齐关键字）+ 块级缩进
    const flat: string[] = []
    let depth = 0
    let level = 0
    let buf: Tok[] = []
    const flush = (): void => {
      if (!buf.length) return
      const firstTok = buf[0].v.toUpperCase()
      const secondTok = buf[1]?.v.toUpperCase()
      // END（非 END IF/LOOP/CASE）前减层；BEGIN 与 DECLARE 同级
      if (firstTok === 'END' && !(secondTok && ['IF', 'LOOP', 'CASE', 'WHILE'].includes(secondTok))) {
        level = Math.max(0, level - 1)
      } else if (firstTok === 'BEGIN') {
        level = Math.max(0, level - 1)
      }
      flat.push(pad(unit, level) + joinTokens(buf))
      const lastTok = buf[buf.length - 1].v.toUpperCase()
      // DECLARE 段 / BEGIN 之后加层
      if (firstTok === 'DECLARE' || lastTok === 'BEGIN') level++
      buf = []
    }
    let prevWasEnd = false
    for (const t of tokens) {
      if (t.v === '(') depth++
      else if (t.v === ')') depth = Math.max(0, depth - 1)
      const up = t.v.toUpperCase()
      const isEndPair = prevWasEnd && (up === 'IF' || up === 'LOOP')
      const controlBreak =
        (o.thenOnNewLine && (up === 'THEN' || up === 'ELSE' || up === 'ELSIF' || up === 'IF')) ||
        (o.loopOnNewLine && up === 'LOOP')
      const blockBreak = up === 'DECLARE' || up === 'BEGIN' || up === 'END' || up === 'FOR' || up === 'WHILE'
      const brk =
        depth === 0 &&
        !isEndPair &&
        (CLAUSE_START.has(up) ||
          JOIN_WORDS.has(up) ||
          up === 'JOIN' ||
          up === 'SET' ||
          up === 'VALUES' ||
          blockBreak ||
          controlBreak)
      if (brk) {
        flush()
      }
      buf.push(t)
      if (up === 'DECLARE' || up === 'BEGIN') {
        flush()
      } else if (t.v === ';' && depth === 0) {
        flush()
      }
      prevWasEnd = up === 'END'
    }
    flush()
    lines = flat.length > 0 ? flat : lines
  }

  // 单行可容纳则合并（OnOneLineIfPossible）
  if (o.onOneLineIfPossible) {
    const one = joinTokens(tokens)
    if (one.length <= o.rightMargin) return [one]
  }
  return lines
}

function isTypeWord(w: string): boolean {
  const u = w.toUpperCase()
  return KEYWORDS.has(u) && /^(NUMBER|VARCHAR2?|NVARCHAR2|CHAR|NCHAR|DATE|TIMESTAMP|CLOB|BLOB|NCLOB|BFILE|INT|INTEGER|SMALLINT|BIGINT|FLOAT|REAL|DOUBLE|DECIMAL|NUMERIC|BOOLEAN|INTERVAL|ROWID)/.test(u)
}

function declParts(line: string): { indent: string; name: string; rest: string } | null {
  const m = /^(\s*)([A-Za-z_"][\w$"]*)\s+(\S.*)$/.exec(line)
  if (!m) return null
  const rest = m[3]
  const firstWord = rest.split(/\s+/)[0].replace(/[(;].*$/, '')
  if (isTypeWord(firstWord) || rest.toUpperCase().startsWith('CONSTANT')) {
    return { indent: m[1], name: m[2], rest }
  }
  return null
}

/** 对齐变量声明组（`name type ...` 的 type 列对齐） */
function alignDeclarations(lines: string[]): string[] {
  const out = lines.slice()
  let i = 0
  while (i < out.length) {
    if (declParts(out[i])) {
      const rows: { idx: number; indent: string; name: string; rest: string }[] = []
      let j = i
      while (j < out.length) {
        const p = declParts(out[j])
        if (!p) break
        rows.push({ idx: j, ...p })
        j++
      }
      if (rows.length >= 2) {
        const w = Math.max(...rows.map((r) => r.name.length))
        for (const r of rows) out[r.idx] = `${r.indent}${r.name.padEnd(w)} ${r.rest}`
      }
      i = j
    } else {
      i++
    }
  }
  return out
}

/** 对齐赋值组（`:=` 列对齐） */
function alignAssignments(lines: string[]): string[] {
  const out = lines.slice()
  let i = 0
  while (i < out.length) {
    if (out[i].includes(':=')) {
      const rows: number[] = []
      let j = i
      while (j < out.length && out[j].includes(':=')) {
        rows.push(j)
        j++
      }
      if (rows.length >= 2) {
        const leftW = Math.max(...rows.map((k) => out[k].indexOf(':=')))
        for (const k of rows) {
          const p = out[k].indexOf(':=')
          out[k] = out[k].slice(0, p).padEnd(leftW) + out[k].slice(p)
        }
      }
      i = j
    } else {
      i++
    }
  }
  return out
}

export function beautifySql(sql: string, o: BeautifyOptions): string {
  const trimmed = sql.trim()
  if (!trimmed) return sql
  const tokens = splitStatements(tokenize(trimmed)).filter((s) => s.length > 0)
  const all: string[] = []
  tokens.forEach((st, idx) => {
    if (idx > 0 && o.emptyLines !== 0) all.push('')
    all.push(...renderStatement(applyCaseTokens(st, o), o))
  })
  let lines = all
  if (o.alignDeclarationGroups) lines = alignDeclarations(lines)
  if (o.alignAssignmentGroups) lines = alignAssignments(lines)
  return lines.join('\n')
}

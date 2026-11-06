import type { DbKind, RoutineInfo, RoutineParam } from '../types/db'
import { quoteIdent } from './sqlGen'

/** 例程执行参数输入（RoutineExecDialog 收集） */
export interface RoutineArgInput {
  /** 用户输入原文 */
  text: string
  /** 勾选 → 传 NULL（优先级最高） */
  isNull: boolean
  /** 勾选 → 原样内联（表达式 / 日期函数等逃生门，不做任何加工） */
  raw: boolean
}

export interface RoutineCallSql {
  /** 可执行 SQL（可含多条语句，走查询页签 runTab 分割路径） */
  sql: string
  /** 含 OUT/INOUT 且输出值无法回显时的提示 i18n key（MySQL @变量回显不发） */
  outNoteKey?: string
}

const NUMERIC_RE = /^[+-]?\d+(?:\.\d+)?$/

/** 空参数输入（对话框缺省：NULL） */
export const emptyArg = (): RoutineArgInput => ({ text: '', isNull: false, raw: false })

/** 字符串字面量（单引号翻倍；MySQL 系会话执行与 dml_literal 同约定：反斜杠转义开启） */
function quoteString(kind: DbKind, s: string): string {
  const escaped = s.replace(/'/g, "''")
  if (kind === 'mysql' || kind === 'oceanbase') {
    return `'${escaped.replace(/\\/g, '\\\\')}'`
  }
  return `'${escaped}'`
}

/** 单参数 → SQL 字面量：NULL > 原样 > 纯数字 > 方言字符串 */
export function routineArgLiteral(kind: DbKind, arg: RoutineArgInput | undefined): string {
  const a = arg ?? emptyArg()
  const t = a.text.trim()
  if (a.isNull || t === '') return 'NULL'
  if (a.raw) return t
  if (NUMERIC_RE.test(t)) return t
  return quoteString(kind, t)
}

const isOutParam = (p: RoutineParam): boolean => p.mode === 'OUT' || p.mode === 'INOUT'
const isOracleFamily = (kind: DbKind): boolean => kind === 'oracle' || kind === 'oceanbase-oracle'

/**
 * 按方言生成例程调用 SQL（SPEC-ROUTINE-1：树右键 / 例程编辑「执行」共用）。
 *
 * - 函数：Oracle 系 `SELECT f(args) FROM DUAL`，其余 `SELECT f(args)`
 * - 过程（无 OUT）：Oracle 系匿名块 `BEGIN p(args); END;`，其余 `CALL p(args)`
 * - 过程（含 OUT）：
 *   - Oracle 系匿名块 DECLARE 本地变量（pdb_a{n}）承载 OUT/INOUT——执行副作用生效但**输出值不回显**
 *     （dbExecute 无绑定参数能力；P2 桥接扩展 CallableStatement 回显）
 *   - MySQL 系 `CALL p(..., @pdb_a{n})` + 追加 `SELECT @pdb_a{n} ...` 同会话回显
 *   - PG/GaussDB `CALL p(...)` OUT 位以 NULL 占位，不回显
 */
export function buildRoutineCallSql(
  kind: DbKind,
  info: RoutineInfo,
  args: RoutineArgInput[],
  schema?: string | null,
): RoutineCallSql {
  const name = quoteIdent(kind, info.name)
  const qualified = schema ? `${quoteIdent(kind, schema)}.${name}` : name
  const lits = info.params.map((_, i) => routineArgLiteral(kind, args[i]))
  const hasOut = info.params.some(isOutParam)

  // 函数：SELECT 调用，返回值入结果网格
  if (info.routineType === 'FUNCTION') {
    const tail = isOracleFamily(kind) ? ' FROM DUAL' : ''
    return { sql: `SELECT ${qualified}(${lits.join(', ')})${tail};` }
  }

  // 过程：Oracle 系一律匿名块（CALL 对 OUT 形参限制多，块内变量最通用）
  if (isOracleFamily(kind)) {
    if (!hasOut) {
      return { sql: `BEGIN\n  ${qualified}(${lits.join(', ')});\nEND;` }
    }
    const decls: string[] = []
    const callArgs = info.params.map((p, i) => {
      const varName = `pdb_a${i + 1}`
      if (!isOutParam(p)) return lits[i]
      // OUT 声明变量；INOUT 以输入值初始化
      decls.push(`  ${varName} ${p.dataType || 'VARCHAR2(4000)'}${p.mode === 'INOUT' ? ` := ${lits[i]}` : ''};`)
      return varName
    })
    return {
      sql: `DECLARE\n${decls.join('\n')}\nBEGIN\n  ${qualified}(${callArgs.join(', ')});\nEND;`,
      outNoteKey: 'routineExec.outNoEcho',
    }
  }

  // MySQL 系：CALL + @会话变量回显（追加 SELECT 同会话取回输出值）
  if (kind === 'mysql' || kind === 'oceanbase') {
    if (!hasOut) {
      return { sql: `CALL ${qualified}(${lits.join(', ')});` }
    }
    const callArgs = info.params.map((p, i) => (isOutParam(p) ? `@pdb_a${i + 1}` : lits[i]))
    const selects = info.params
      .map((p, i) => ({ p, i }))
      .filter(({ p }) => isOutParam(p))
      .map(({ p, i }) => `@pdb_a${i + 1} AS \`${p.name || `pdb_a${i + 1}`}\``)
    return {
      sql: `CALL ${qualified}(${callArgs.join(', ')});\nSELECT ${selects.join(', ')};`,
    }
  }

  // PG / GaussDB：CALL（PG11+）；OUT 位以 NULL 占位
  const callArgs = info.params.map((p, i) => (isOutParam(p) && p.mode === 'OUT' ? 'NULL' : lits[i]))
  return {
    sql: `CALL ${qualified}(${callArgs.join(', ')});`,
    outNoteKey: hasOut ? 'routineExec.outNoEcho' : undefined,
  }
}

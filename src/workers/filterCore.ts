/**
 * 结果集过滤核心（纯函数，主线程与 Worker 共用，保证两条路径语义一致）。
 * 约定：所有匹配均为「包含」（大小写不敏感）；NULL 文本化为 'NULL' 参与匹配。
 */

export interface FilterOptions {
  /** 全局关键字（调用方已 lowercase；空串 = 不过滤），扫描全部列 */
  search: string
  /** 列包含过滤（col 为矩阵列下标；q 已 lowercase） */
  colFilters: { col: number; q: string }[]
  /** 列精确值过滤（values 为 cellText 形式） */
  valueFilters: { col: number; values: string[] }[]
}

export function cellText(v: unknown): string {
  return v === null || v === undefined ? 'NULL' : String(v)
}

/** 返回升序匹配行下标（顺序与输入一致） */
export function filterMatrix(rows: unknown[][], opts: FilterOptions): number[] {
  const search = opts.search
  const colFilters = opts.colFilters
  const valueFilters = opts.valueFilters
    .filter((f) => f.values.length > 0)
    .map((f) => ({ col: f.col, set: new Set(f.values) }))
  const indices: number[] = []
  outer: for (let i = 0; i < rows.length; i++) {
    const row = rows[i]
    for (const f of colFilters) {
      if (!cellText(row[f.col]).toLowerCase().includes(f.q)) continue outer
    }
    for (const f of valueFilters) {
      if (!f.set.has(cellText(row[f.col]))) continue outer
    }
    if (search) {
      let hit = false
      for (let c = 0; c < row.length; c++) {
        if (cellText(row[c]).toLowerCase().includes(search)) {
          hit = true
          break
        }
      }
      if (!hit) continue
    }
    indices.push(i)
  }
  return indices
}

/**
 * 结果集过滤 Worker：大结果集（≥阈值）时把全表扫描移出主线程。
 * 协议：{ token, rows, ...FilterOptions } → { token, indices }（token 匹配才采纳）。
 */
import { filterMatrix, type FilterOptions } from './filterCore'

interface FilterRequest extends FilterOptions {
  token: number
  rows: unknown[][]
}

self.onmessage = (e: MessageEvent<FilterRequest>) => {
  const { token, rows, ...opts } = e.data
  const indices = filterMatrix(rows, opts)
  ;(self as unknown as Worker).postMessage({ token, indices })
}

/**
 * 结果集统一数据管线：矩阵 + prop 索引 + Worker 过滤 + 物化行。
 * 唯一事实源 filteredIdx：Worker 异步 / 同步回退共用同一 filterCore（≥3000 行移出主线程）。
 */
import { computed, ref, watch, onBeforeUnmount, toRaw } from 'vue'
import type { Ref } from 'vue'
import { filterMatrix, type FilterOptions } from '../workers/filterCore'
import type { QueryTab } from '../stores/tabs'
import type { CellValue } from '../types/db'

/** 小结果集直接同步计算（避免 postMessage 开销） */
const WORKER_THRESHOLD = 3000

export function useResultPipeline(options: {
  tab: () => QueryTab
  /** 列级设置（仅消费 filter 字段，结构化兼容完整 ColSetting） */
  colSettings: Ref<Record<string, { filter?: string }>>
  searchApplied: Ref<string>
  editMode: Ref<boolean>
}) {
  const { tab, colSettings, searchApplied, editMode } = options

  /** 结果矩阵（后端原始行，零拷贝引用） */
  const matrix = computed(() => tab().result?.rows ?? [])

  /** prop 键（c{count}_{name}）→ 矩阵列下标 */
  const propIndex = computed(() => {
    const result = tab().result
    const m = new Map<string, number>()
    if (!result) return m
    const seen = new Map<string, number>()
    result.columns.forEach((c, i) => {
      const count = seen.get(c.name) ?? 0
      seen.set(c.name, count + 1)
      m.set(`c${count}_${c.name}`, i)
    })
    return m
  })

  /** 过滤后的行下标（唯一事实源；Worker 异步 / 同步回退共用同一 filterCore） */
  const filteredIdx = ref<number[]>([])
  const filterSeq = { n: 0 }
  let filterWorker: Worker | null = null

  /** Excel 式列值筛选：prop → 选中的值列表（空数组=未启用） */
  const valueFilters = ref<Record<string, string[]>>({})
  const distinctCache = ref<Map<string, string[]>>(new Map())

watch(
  // 结果对象替换与分页续拉（原地 rows.push）都要使 distinct 缓存失效
  [() => tab().result, () => tab().result?.rows.length],
  () => {
    distinctCache.value = new Map()
  },
)

  function currentFilterOptions(): FilterOptions {
    const colFilters: { col: number; q: string }[] = []
    for (const [prop, s] of Object.entries(colSettings.value)) {
      if (!s.filter) continue
      const col = propIndex.value.get(prop)
      if (col === undefined) continue
      colFilters.push({ col, q: s.filter.toLowerCase() })
    }
    const vf: { col: number; values: string[] }[] = []
    for (const [prop, values] of Object.entries(valueFilters.value)) {
      if (!values || values.length === 0) continue
      const col = propIndex.value.get(prop)
      if (col === undefined) continue
      vf.push({ col, values })
    }
    return { search: searchApplied.value.trim().toLowerCase(), colFilters, valueFilters: vf }
  }

  function applyFilterSync(): void {
    filteredIdx.value = filterMatrix(matrix.value, currentFilterOptions())
  }

  function scheduleFilter(): void {
    // 编辑态走 editData 渲染；小结果集同步计算（避免 postMessage 开销）
    if (editMode.value || matrix.value.length < WORKER_THRESHOLD || typeof Worker === 'undefined') {
      applyFilterSync()
      return
    }
    if (!filterWorker) {
      try {
        filterWorker = new Worker(new URL('../workers/resultFilter.worker.ts', import.meta.url), {
          type: 'module',
        })
        filterWorker.onmessage = (e: MessageEvent<{ token: number; indices: number[] }>) => {
          if (e.data.token === filterSeq.n) filteredIdx.value = e.data.indices
        }
        filterWorker.onerror = () => {
          filterWorker?.terminate()
          filterWorker = null
          applyFilterSync()
        }
      } catch {
        filterWorker = null
        applyFilterSync()
        return
      }
    }
    const token = ++filterSeq.n
    try {
      // toRaw：reactive proxy 无法结构化克隆（DataCloneError），Worker 必须收原始数组
      filterWorker.postMessage({ token, rows: toRaw(matrix.value), ...currentFilterOptions() })
    } catch {
      filterWorker.terminate()
      filterWorker = null
      applyFilterSync()
    }
  }

  watch(
    // rows.length：loadMore 原地 push 不改变 result 身份，必须显式跟踪长度
    [
      searchApplied,
      valueFilters,
      colSettings,
      () => tab().result,
      () => tab().result?.rows.length,
      () => editMode.value,
    ],
    () => scheduleFilter(),
    { immediate: true },
  )

  onBeforeUnmount(() => {
    filterWorker?.terminate()
    filterWorker = null
  })

  /** 物化对象行（按需；仅 el-table 渲染 / 复制 / 导出 / 统计触达，虚拟路径零分配） */
  const resultRows = computed(() => {
    const keys = [...propIndex.value.keys()]
    const m = matrix.value
    return filteredIdx.value.map((i) => {
      const src = m[i]
      const o: Record<string, CellValue> = {}
      keys.forEach((k, c) => {
        o[k] = src[c]
      })
      return o
    })
  })

  return { matrix, propIndex, filteredIdx, valueFilters, distinctCache, resultRows }
}

import { describe, expect, it } from 'vitest'
import { cellText, filterMatrix } from '../src/workers/filterCore'

const rows: unknown[][] = [
  ['Alice', 30, null],
  ['Bob', 25, 'x@dev.com'],
  ['alice2', 41, 'DEV'],
  ['Carol', null, null],
]

describe('filterCore — cellText', () => {
  it('NULL 文本化（null/undefined → NULL）', () => {
    expect(cellText(null)).toBe('NULL')
    expect(cellText(undefined)).toBe('NULL')
    expect(cellText(0)).toBe('0')
    expect(cellText(false)).toBe('false')
  })
})

describe('filterCore — filterMatrix', () => {
  it('无过滤条件 → 全部行（升序原序）', () => {
    expect(filterMatrix(rows, { search: '', colFilters: [], valueFilters: [] })).toEqual([0, 1, 2, 3])
  })

  it('全局查找：跨列包含、大小写不敏感', () => {
    expect(filterMatrix(rows, { search: 'alice', colFilters: [], valueFilters: [] })).toEqual([0, 2])
    expect(filterMatrix(rows, { search: 'dev', colFilters: [], valueFilters: [] })).toEqual([1, 2])
  })

  it('NULL 文本可被全局查找命中', () => {
    expect(filterMatrix(rows, { search: 'null', colFilters: [], valueFilters: [] })).toEqual([0, 3])
  })

  it('列包含过滤（q 已 lowercase）', () => {
    expect(
      filterMatrix(rows, { search: '', colFilters: [{ col: 0, q: 'carol' }], valueFilters: [] }),
    ).toEqual([3])
    // 多列过滤为 AND 语义
    expect(
      filterMatrix(rows, {
        search: '',
        colFilters: [
          { col: 0, q: 'ali' },
          { col: 1, q: '4' },
        ],
        valueFilters: [],
      }),
    ).toEqual([2])
  })

  it('值集合精确过滤；空 values 数组视为未启用', () => {
    expect(
      filterMatrix(rows, { search: '', colFilters: [], valueFilters: [{ col: 0, values: ['Alice', 'Carol'] }] }),
    ).toEqual([0, 3])
    expect(
      filterMatrix(rows, { search: '', colFilters: [], valueFilters: [{ col: 0, values: [] }] }),
    ).toEqual([0, 1, 2, 3])
    // 值集合区分大小写（cellText 原样比较）
    expect(filterMatrix(rows, { search: '', colFilters: [], valueFilters: [{ col: 2, values: ['DEV'] }] })).toEqual([2])
  })

  it('search + colFilters + valueFilters 叠加（AND）', () => {
    expect(
      filterMatrix(rows, {
        search: 'e',
        colFilters: [{ col: 1, q: '2' }],
        valueFilters: [{ col: 2, values: ['x@dev.com'] }],
      }),
    ).toEqual([1])
  })
})

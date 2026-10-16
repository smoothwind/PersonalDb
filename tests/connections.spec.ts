import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import type { Pinia } from 'pinia'
import { useConnectionStore } from '../src/stores/connections'
import type { ConnectionConfig, ConnectionGroup } from '../src/types/db'

const api = vi.hoisted(() => ({
  configLoad: vi.fn(async () => [] as ConnectionConfig[]),
  configSave: vi.fn(async () => undefined),
  dbConnect: vi.fn(async () => ({ sessionId: 's' })),
  dbDisconnect: vi.fn(async () => undefined),
  dbListDatabases: vi.fn(async () => [] as string[]),
  groupLoad: vi.fn(async () => [] as ConnectionGroup[]),
  groupSave: vi.fn(async () => undefined),
  secretCopy: vi.fn(async () => undefined),
  secretDelete: vi.fn(async () => undefined),
  secretStore: vi.fn(async () => undefined),
}))

vi.mock('../src/api/db', () => api)

function cfg(id: string, groupId: string | null = null): ConnectionConfig {
  return {
    id,
    name: id,
    kind: 'mysql',
    host: 'localhost',
    port: 3306,
    username: 'u',
    password: '',
    database: '',
    groupId,
  }
}

function grp(id: string, parentId: string | null = null): ConnectionGroup {
  return { id, name: id, parentId }
}

let pinia: Pinia

beforeEach(() => {
  pinia = createPinia()
  setActivePinia(pinia)
})

describe('moveGroup — 文件夹移动/排序（含循环嵌套防护）', () => {
  it('拒绝移动到自身', async () => {
    const store = useConnectionStore()
    store.groups = [grp('A')]
    expect(await store.moveGroup('A', 'A', 'inner')).toBe(false)
  })

  it('拒绝移动到自己的后代（循环嵌套防护）', async () => {
    const store = useConnectionStore()
    store.groups = [grp('A'), grp('A1', 'A'), grp('A2', 'A1')]
    expect(await store.moveGroup('A', 'A1', 'inner')).toBe(false)
    expect(await store.moveGroup('A', 'A2', 'inner')).toBe(false)
    // 未被改动
    expect(store.groups.find((g) => g.id === 'A')?.parentId ?? null).toBe(null)
  })

  it('inner：嵌套为目标的子文件夹', async () => {
    const store = useConnectionStore()
    store.groups = [grp('A'), grp('B')]
    expect(await store.moveGroup('B', 'A', 'inner')).toBe(true)
    expect(store.groups.find((g) => g.id === 'B')?.parentId).toBe('A')
    expect(store.groupChildren('A').map((g) => g.id)).toContain('B')
    expect(store.groupChildren(null).map((g) => g.id)).not.toContain('B')
  })

  it('inner 到根（targetId=null）→ 移回根级', async () => {
    const store = useConnectionStore()
    store.groups = [grp('A'), grp('B', 'A')]
    expect(await store.moveGroup('B', null, 'inner')).toBe(true)
    expect(store.groups.find((g) => g.id === 'B')?.parentId ?? null).toBe(null)
  })

  it('before/after：同级排序，父级随目标', async () => {
    const store = useConnectionStore()
    store.groups = [grp('A'), grp('B'), grp('C')]
    expect(await store.moveGroup('C', 'A', 'before')).toBe(true)
    expect(store.groupChildren(null).map((g) => g.id)).toEqual(['C', 'A', 'B'])

    store.groups = [grp('A'), grp('B'), grp('C')]
    expect(await store.moveGroup('A', 'C', 'after')).toBe(true)
    expect(store.groupChildren(null).map((g) => g.id)).toEqual(['B', 'C', 'A'])
  })
})

describe('moveToGroup — 连接归属', () => {
  it('移入文件夹 / 移出分组', async () => {
    const store = useConnectionStore()
    store.saved = [cfg('c1'), cfg('c2', 'A')]
    await store.moveToGroup('c1', 'A')
    expect(store.configOf('c1')?.groupId).toBe('A')
    await store.moveToGroup('c2', null)
    expect(store.configOf('c2')?.groupId ?? null).toBe(null)
  })
})

describe('removeGroup — 三种删除策略', () => {
  it('promote：仅删本级，子文件夹/连接上移', async () => {
    const store = useConnectionStore()
    store.groups = [grp('A'), grp('A1', 'A')]
    store.saved = [cfg('c1', 'A'), cfg('c2', 'A1')]
    await store.removeGroup('A', 'promote')
    expect(store.groups.find((g) => g.id === 'A')).toBeUndefined()
    expect(store.groups.find((g) => g.id === 'A1')?.parentId ?? null).toBe(null)
    expect(store.configOf('c1')?.groupId ?? null).toBe(null)
    expect(store.configOf('c2')?.groupId).toBe('A1')
  })

  it('flatten：删本级+子文件夹，连接上移到父级', async () => {
    const store = useConnectionStore()
    store.groups = [grp('A'), grp('A1', 'A'), grp('A2', 'A1')]
    store.saved = [cfg('c1', 'A1')]
    await store.removeGroup('A', 'flatten')
    expect(store.groups.map((g) => g.id)).toEqual([])
    expect(store.configOf('c1')?.groupId ?? null).toBe(null)
  })

  it('purge：连带删除其中的连接（含凭据）', async () => {
    const store = useConnectionStore()
    store.groups = [grp('A'), grp('A1', 'A')]
    store.saved = [cfg('c1', 'A1'), cfg('c2', null)]
    await store.removeGroup('A', 'purge')
    expect(store.groups.map((g) => g.id)).toEqual([])
    expect(store.configOf('c1')).toBeUndefined()
    expect(store.configOf('c2')).toBeDefined()
    expect(api.secretDelete).toHaveBeenCalledWith('c1')
  })
})

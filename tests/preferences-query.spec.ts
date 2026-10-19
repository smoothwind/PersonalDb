import { beforeEach, describe, expect, it } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { usePreferencesStore } from '../src/stores/preferences'

beforeEach(() => {
  localStorage.clear()
  setActivePinia(createPinia())
})

describe('preferences — 查询执行超时', () => {
  it('默认 60 秒（兼容旧数据）', () => {
    const p = usePreferencesStore()
    expect(p.query.execTimeoutSec).toBe(60)
  })

  it('updateQuery 即时生效并持久化到 personaldb.query', () => {
    const p = usePreferencesStore()
    p.updateQuery({ execTimeoutSec: 0 })
    expect(p.query.execTimeoutSec).toBe(0)
    expect(JSON.parse(localStorage.getItem('personaldb.query') ?? '{}')).toEqual({
      execTimeoutSec: 0,
    })
  })

  it('旧数据缺字段时合并默认值', () => {
    localStorage.setItem('personaldb.query', '{}')
    setActivePinia(createPinia())
    const p = usePreferencesStore()
    expect(p.query.execTimeoutSec).toBe(60)
  })
})

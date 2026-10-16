import { beforeEach, describe, expect, it } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useLayoutStore } from '../src/stores/layout'

beforeEach(() => {
  localStorage.clear()
  localStorage.setItem('personaldb.layout', '')
  setActivePinia(createPinia())
})

describe('layout store — 停靠/交换/悬浮', () => {
  it('默认布局：conn 左 / tabs 中 / log 下', () => {
    const l = useLayoutStore()
    expect(l.leftPanel).toBe('conn')
    expect(l.centerPanel).toBe('tabs')
    expect(l.bottomPanel).toBe('log')
    expect(l.rightPanel).toBeNull()
  })

  it('move 到空区直接迁入；到占用区交换', () => {
    const l = useLayoutStore()
    // log → 右侧（空）
    l.move('log', 'right')
    expect(l.rightPanel).toBe('log')
    expect(l.bottomPanel).toBeNull()
    // conn → 右侧（log 占用）→ 交换：log 回 conn 原来的左侧
    l.move('conn', 'right')
    expect(l.rightPanel).toBe('conn')
    expect(l.leftPanel).toBe('log')
  })

  it('中心常驻：中心面板不允许隐藏/悬浮', () => {
    const l = useLayoutStore()
    l.hide('tabs')
    expect(l.centerPanel).toBe('tabs')
    l.float('tabs')
    expect(l.centerPanel).toBe('tabs')
    expect(l.panels.tabs.float).toBeUndefined()
  })

  it('hide / show：恢复到最近区，占用者转悬浮', () => {
    const l = useLayoutStore()
    l.hide('conn')
    expect(l.leftPanel).toBeNull()
    expect(l.panels.conn.zone).toBeNull()
    l.show('conn')
    expect(l.leftPanel).toBe('conn')
    // 左侧被 conn 占回？conn 的 lastZone=left 且左侧已空 → 直接回
    // 再测占用场景：log hide 后 show，lastZone=bottom（空）→ 直接回
    l.hide('log')
    l.show('log')
    expect(l.bottomPanel).toBe('log')
  })

  it('float：级联几何 + 焦点递增 + minimize/restore + dockBack', () => {
    const l = useLayoutStore()
    l.float('log')
    expect(l.panels.log.zone).toBeNull()
    expect(l.panels.log.float).toBeDefined()
    const z1 = l.zOf('log')
    l.float('conn')
    const z2 = l.zOf('conn')
    expect(z2).toBeGreaterThan(z1)
    // 最小化 → 层叠条
    l.minimize('log')
    expect(l.panels.log.minimized).toBe(true)
    expect(l.floatingPanels.filter((id) => l.panels[id].minimized)).toEqual(['log'])
    l.restore('log')
    expect(l.panels.log.minimized).toBe(false)
    // 回收停靠 → lastZone
    l.dockBack('log')
    expect(l.bottomPanel).toBe('log')
  })

  it('move 来源为悬浮时：目标占用者转悬浮', () => {
    const l = useLayoutStore()
    l.float('log') // log 悬浮，bottom 空
    l.move('conn', 'right') // conn → 右
    l.move('log', 'right') // 悬浮的 log → 右（conn 占用）→ conn 转悬浮
    expect(l.rightPanel).toBe('log')
    expect(l.panels.conn.zone).toBeNull()
    expect(l.panels.conn.float).toBeDefined()
  })

  it('resizeZone 夹取边界', () => {
    const l = useLayoutStore()
    l.resizeZone('left', 50)
    expect(l.sizes.left).toBe(200)
    l.resizeZone('left', 9999)
    expect(l.sizes.left).toBe(560)
    l.resizeZone('bottom', 100)
    expect(l.sizes.bottom).toBe(120)
  })

  it('persist：move 后写入 localStorage，重载恢复', async () => {
    const l = useLayoutStore()
    l.move('log', 'right')
    await new Promise((r) => setTimeout(r, 250)) // 等防抖落盘
    const raw = JSON.parse(localStorage.getItem('personaldb.layout')!)
    expect(raw.panels.log.zone).toBe('right')
    // 新 store 实例读到持久化值
    setActivePinia(createPinia())
    const l2 = useLayoutStore()
    expect(l2.rightPanel).toBe('log')
  })

  it('resetLayout 恢复默认', () => {
    const l = useLayoutStore()
    l.move('log', 'left')
    l.resetLayout()
    expect(l.leftPanel).toBe('conn')
    expect(l.bottomPanel).toBe('log')
  })
})

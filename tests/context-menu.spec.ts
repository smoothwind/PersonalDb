import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { nextTick } from 'vue'
import ElementPlus from 'element-plus'
import ContextMenu, { type CtxMenuItem } from '../src/components/ContextMenu.vue'

/** 扁平回归样例：普通项 + 分隔线 + 禁用项 */
const FLAT: CtxMenuItem[] = [
  { key: 'a', label: 'A' },
  { key: 'sep-0', label: '', sep: true },
  { key: 'b', label: 'B', disabled: true },
]

/** 级联样例：两个父项（含禁用子项）+ 一个普通项 */
const NESTED: CtxMenuItem[] = [
  { key: 'p1', label: 'P1', children: [{ key: 'c1', label: 'C1' }, { key: 'c2', label: 'C2', disabled: true }] },
  { key: 'f', label: 'F' },
  { key: 'p2', label: 'P2', children: [{ key: 'c3', label: 'C3' }] },
]

let wrapper: VueWrapper | null = null

function mountMenu(items: CtxMenuItem[]): VueWrapper {
  wrapper = mount(ContextMenu, {
    props: { visible: true, x: 10, y: 10, items },
    global: { plugins: [ElementPlus] },
    attachTo: document.body,
  })
  return wrapper
}

/** Teleport 到 body，统一走 document 查询 */
const q = (sel: string): Element | null => document.querySelector(sel)
const all = (sel: string): Element[] => [...document.querySelectorAll(sel)]
const topItems = (): Element[] => all('.pd-ctx > .pd-ctx-item')
const subItems = (): Element[] => all('.pd-ctx-sub .pd-ctx-item')
const click = (el: Element): void => el.dispatchEvent(new MouseEvent('click', { bubbles: true }))
const enter = (el: Element): void => el.dispatchEvent(new MouseEvent('mouseenter'))

async function pressEsc(): Promise<void> {
  document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
  await nextTick()
}

beforeEach(() => {
  document.body.innerHTML = ''
})

afterEach(() => {
  wrapper?.unmount()
  wrapper = null
  document.body.innerHTML = ''
  vi.restoreAllMocks()
})

describe('ContextMenu 扁平回归（既有 5 处调用零影响）', () => {
  it('渲染普通项/分隔线/禁用项', () => {
    mountMenu(FLAT)
    expect(topItems()).toHaveLength(2)
    expect(q('.pd-ctx-sep')).toBeTruthy()
  })

  it('点击叶子：emit select(key) + update:visible(false)', async () => {
    const w = mountMenu(FLAT)
    click(topItems()[0])
    await nextTick()
    expect(w.emitted('select')).toEqual([['a']])
    expect(w.emitted('update:visible')).toEqual([[false]])
  })

  it('点击禁用项：无 select 无关闭', async () => {
    const w = mountMenu(FLAT)
    click(topItems()[1])
    await nextTick()
    expect(w.emitted('select')).toBeUndefined()
    expect(w.emitted('update:visible')).toBeUndefined()
  })

  it('菜单外点击收起、Esc 收起', async () => {
    const w = mountMenu(FLAT)
    await flushPromises() // 等 immediate watcher 内 nextTick 后 attach 完成
    document.body.dispatchEvent(new MouseEvent('click', { bubbles: true }))
    await nextTick()
    expect(w.emitted('update:visible')).toEqual([[false]])
    w.setProps({ visible: true })
    await flushPromises()
    await pressEsc()
    expect(w.emitted('update:visible')).toEqual([[false], [false]])
  })
})

describe('ContextMenu 一级级联子菜单', () => {
  it('点击父项展开：不冒泡 select、不关闭菜单', async () => {
    const w = mountMenu(NESTED)
    click(topItems()[0])
    await flushPromises()
    expect(q('.pd-ctx-sub')).toBeTruthy()
    expect(w.emitted('select')).toBeUndefined()
    expect(w.emitted('update:visible')).toBeUndefined()
  })

  it('点击父项幂等：重复点击不收起已展开子菜单', async () => {
    mountMenu(NESTED)
    click(topItems()[0])
    await flushPromises()
    click(topItems()[0])
    await flushPromises()
    expect(q('.pd-ctx-sub')).toBeTruthy()
  })

  it('子项点击：emit 子项 key + 整菜单收起', async () => {
    const w = mountMenu(NESTED)
    click(topItems()[0])
    await flushPromises()
    click(subItems()[0])
    await nextTick()
    expect(w.emitted('select')).toEqual([['c1']])
    expect(w.emitted('update:visible')).toEqual([[false]])
  })

  it('禁用子项不可点击', async () => {
    const w = mountMenu(NESTED)
    click(topItems()[0])
    await flushPromises()
    click(subItems()[1])
    await nextTick()
    expect(w.emitted('select')).toBeUndefined()
    expect(w.emitted('update:visible')).toBeUndefined()
  })

  it('禁用父项不展开子菜单', async () => {
    mountMenu([
      { key: 'p', label: 'P', disabled: true, children: [{ key: 'c', label: 'C' }] },
    ])
    click(topItems()[0])
    enter(topItems()[0])
    await flushPromises()
    expect(q('.pd-ctx-sub')).toBeNull()
  })

  it('hover 展开、hover 其它项收起、hover 另一父项切换', async () => {
    mountMenu(NESTED)
    enter(topItems()[0])
    await flushPromises()
    expect(q('.pd-ctx-sub')).toBeTruthy()
    // 普通项收起
    enter(topItems()[1])
    await flushPromises()
    expect(q('.pd-ctx-sub')).toBeNull()
    // 切到另一父项
    enter(topItems()[2])
    await flushPromises()
    expect(q('.pd-ctx-sub')).toBeTruthy()
    expect(subItems()[0].textContent).toContain('C3')
  })

  it('Esc 两段关闭：先子菜单后主菜单', async () => {
    const w = mountMenu(NESTED)
    click(topItems()[0])
    await flushPromises()
    await pressEsc()
    expect(q('.pd-ctx-sub')).toBeNull()
    expect(w.emitted('update:visible')).toBeUndefined()
    await pressEsc()
    expect(w.emitted('update:visible')).toEqual([[false]])
  })

  it('父项高亮 is-open 随展开/收起', async () => {
    mountMenu(NESTED)
    click(topItems()[0])
    await flushPromises()
    expect(topItems()[0].classList.contains('is-open')).toBe(true)
    await pressEsc()
    expect(topItems()[0].classList.contains('is-open')).toBe(false)
  })

  it('视口右缘空间不足时子菜单向左弹出（is-flip）', async () => {
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
      x: 0, y: 0, top: 0, left: 0, bottom: 0, width: 100, height: 24,
      right: 900,
      toJSON: () => ({}),
    } as DOMRect)
    mountMenu(NESTED)
    click(topItems()[0])
    await flushPromises()
    // jsdom innerWidth=1024：900 + 200 > 1024 - 8 → 翻转
    expect(q('.pd-ctx-sub.is-flip')).toBeTruthy()
  })

  it('右缘空间充足时向右弹出（无 is-flip）', async () => {
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
      x: 0, y: 0, top: 0, left: 0, bottom: 0, width: 100, height: 24,
      right: 100,
      toJSON: () => ({}),
    } as DOMRect)
    mountMenu(NESTED)
    click(topItems()[0])
    await flushPromises()
    expect(q('.pd-ctx-sub')).toBeTruthy()
    expect(q('.pd-ctx-sub.is-flip')).toBeNull()
  })
})

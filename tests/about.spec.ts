import { beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { createI18n } from 'vue-i18n'
import { createPinia, setActivePinia } from 'pinia'
import { defineComponent, h, ref, type Ref } from 'vue'
import ElementPlus from 'element-plus'
import App from '../src/App.vue'
import AboutDialog from '../src/components/AboutDialog.vue'
import zhCn from '../src/i18n/locales/zh-CN'
import en from '../src/i18n/locales/en'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    switch (cmd) {
      case 'map_commands':
        return []
      case 'config_load':
      case 'group_load':
      case 'snippet_load':
        return []
      case 'workspace_load':
        return { currentId: '', list: [] }
      case 'bridge_status':
        return { oracleGo: true, obJdbc: true }
      default:
        return undefined
    }
  }),
}))
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ minimize: vi.fn(), close: vi.fn(), setFocus: vi.fn() }),
}))
vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(async () => null),
  save: vi.fn(async () => null),
}))
vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: vi.fn(async () => undefined),
  revealItemInDir: vi.fn(async () => undefined),
}))
vi.mock('../src/tours', () => ({
  maybeAutoTour: vi.fn(),
  startTour: vi.fn(),
  TOURS: [{ id: 'overview', labelKey: 'tour.overview' }],
}))

beforeEach(() => {
  document.body.innerHTML = ''
  setActivePinia(createPinia())
})

function makeI18n() {
  return createI18n({ legacy: false, locale: 'zh-CN', messages: { 'zh-CN': zhCn, en } })
}

function mountApp() {
  return mount(App, {
    global: {
      plugins: [ElementPlus, createPinia(), makeI18n()],
      config: { warnHandler: () => {} },
    },
    attachTo: document.body,
  })
}

function mountDialog(initial: boolean): { wrapper: ReturnType<typeof mount>; open: Ref<boolean> } {
  const open = ref(initial)
  const Host = defineComponent({
    setup() {
      return () =>
        h(AboutDialog, {
          modelValue: open.value,
          'onUpdate:modelValue': (v: boolean) => (open.value = v),
        })
    },
  })
  const wrapper = mount(Host, {
    global: {
      plugins: [ElementPlus, createPinia(), makeI18n()],
      config: { warnHandler: () => {} },
      stubs: { teleport: true },
    },
    attachTo: document.body,
  })
  return { wrapper, open }
}

const flush = async (wrapper: { vm: { $nextTick: () => Promise<void> } }) => {
  await wrapper.vm.$nextTick()
  await new Promise((r) => setTimeout(r, 120))
}

describe('关于按钮 — App 菜单链路', () => {
  it('帮助菜单选中 about 项时打开对话框，点确定后关闭，可再次打开', async () => {
    const wrapper = mountApp()
    await flush(wrapper)

    const menu = wrapper.findComponent({ name: 'ElMenu' })
    expect(menu.exists(), '存在顶部菜单').toBe(true)
    // 直接调用菜单绑定的 @select 处理函数（点菜单项等价路径，规避 EP 事件校验）
    const onSelect = (
      menu.vm.$ as unknown as { vnode: { props?: { onSelect?: (index: string) => void } } }
    ).vnode.props?.onSelect
    expect(typeof onSelect, '菜单绑定了 onMenuSelect').toBe('function')

    // 打开
    onSelect('about')
    await flush(wrapper)
    expect(wrapper.find('.about-logo').isVisible(), '菜单触发后对话框可见').toBe(true)

    // 点确定关闭
    const ok = wrapper
      .findAll('button')
      .find((b) => (b.text() ?? '').includes(zhCn.common.confirm))
    expect(ok, '存在确定按钮').toBeTruthy()
    await ok!.trigger('click')
    await flush(wrapper)
    expect(wrapper.find('.about-logo').isVisible(), '确定后对话框关闭').toBe(false)

    // 再次打开（回归）
    onSelect('about')
    await flush(wrapper)
    expect(wrapper.find('.about-logo').isVisible(), '可重新打开').toBe(true)

    wrapper.unmount()
  })
})

describe('关于对话框 — 内容与交互', () => {
  it('初始关闭时内容未渲染', async () => {
    const { wrapper } = mountDialog(false)
    await flush(wrapper)
    expect(wrapper.find('.about-logo').exists()).toBe(false)
    wrapper.unmount()
  })

  it('打开后展示品牌、版本号、技术栈、致谢文案', async () => {
    const { wrapper } = mountDialog(true)
    await flush(wrapper)
    const logo = wrapper.find('.about-logo')
    expect(logo.isVisible()).toBe(true)
    expect(logo.text()).toContain('PersonalDb')
    // 版本号随发版变化，断言格式而非具体值，避免每次发版都要改测试
    expect(wrapper.find('.about-ver').text()).toMatch(/^v\d+\.\d+\.\d+/)
    const stack = wrapper.find('.about-stack').text()
    expect(stack).toContain('Tauri 2')
    expect(stack).toContain('WebView2')
    expect(stack).toContain('Vue 3')
    expect(wrapper.find('.about-credits').text().length).toBeGreaterThan(0)
    wrapper.unmount()
  })

  it('点击「确定」触发 update:modelValue(false) 并关闭', async () => {
    const { wrapper, open } = mountDialog(true)
    await flush(wrapper)
    expect(open.value).toBe(true)

    const ok = wrapper.findAll('button').find((b) => (b.text() ?? '').includes(zhCn.common.confirm))
    expect(ok).toBeTruthy()
    await ok!.trigger('click')
    await flush(wrapper)

    expect(open.value, 'modelValue 更新为 false').toBe(false)
    expect(wrapper.find('.about-logo').isVisible()).toBe(false)
    wrapper.unmount()
  })

  it('重复打开/关闭可复用（回归）', async () => {
    const { wrapper, open } = mountDialog(false)
    await flush(wrapper)
    expect(wrapper.find('.about-logo').exists()).toBe(false)

    open.value = true
    await flush(wrapper)
    expect(wrapper.find('.about-logo').isVisible()).toBe(true)

    const ok = wrapper.findAll('button').find((b) => (b.text() ?? '').includes(zhCn.common.confirm))
    await ok!.trigger('click')
    await flush(wrapper)
    expect(wrapper.find('.about-logo').isVisible()).toBe(false)

    open.value = true
    await flush(wrapper)
    expect(wrapper.find('.about-logo').isVisible()).toBe(true)
    wrapper.unmount()
  })
})

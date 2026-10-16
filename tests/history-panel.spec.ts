import { beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { createI18n } from 'vue-i18n'
import { createPinia, setActivePinia } from 'pinia'
import ElementPlus from 'element-plus'
import HistoryPanel from '../src/components/HistoryPanel.vue'
import zhCn from '../src/i18n/locales/zh-CN'
import en from '../src/i18n/locales/en'

vi.mock('../src/api/db', () => ({
  historyList: vi.fn(async () => []),
  historyAdd: vi.fn(async () => undefined),
  historyClear: vi.fn(async () => undefined),
}))

vi.mock('../src/utils/clipboard', () => ({ copyText: vi.fn(async () => undefined) }))

beforeEach(() => {
  document.body.innerHTML = ''
  setActivePinia(createPinia())
})

const flush = async (wrapper: { vm: { $nextTick: () => Promise<void> } }) => {
  await wrapper.vm.$nextTick()
  await new Promise((r) => setTimeout(r, 60))
}

describe('HistoryPanel — 参考元素须为原生元素', () => {
  it('挂载不产生 Vue 警告（popover #reference 不再落到组件根）', async () => {
    const warns: string[] = []
    const wrapper = mount(HistoryPanel, {
      global: {
        plugins: [
          ElementPlus,
          createI18n({ legacy: false, locale: 'zh-CN', messages: { 'zh-CN': zhCn, en } }),
        ],
        config: {
          warnHandler: (msg) => {
            warns.push(String(msg))
          },
        },
        stubs: { teleport: true },
      },
      props: { connName: 'c' },
      attachTo: document.body,
    })
    await flush(wrapper)
    expect(warns).toEqual([])
    expect(wrapper.find('button').exists()).toBe(true)
    wrapper.unmount()
  })
})

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { createI18n } from 'vue-i18n'
import { createPinia, setActivePinia } from 'pinia'
import ElementPlus from 'element-plus'
import { useLogStore, sqlTitle } from '../src/stores/logs'
import LogPanel from '../src/components/LogPanel.vue'
import zhCn from '../src/i18n/locales/zh-CN'
import en from '../src/i18n/locales/en'

beforeEach(() => {
  document.body.innerHTML = ''
  localStorage.clear()
  setActivePinia(createPinia())
})

describe('logs store — 环形缓冲与未读计数', () => {
  it('push 追加、超上限丢弃最旧', () => {
    const logs = useLogStore()
    for (let i = 0; i < 1005; i++) {
      logs.push({ level: 'info', kind: 'sql', title: `stmt-${i}` })
    }
    expect(logs.entries.length).toBe(1000)
    expect(logs.entries[0].title).toBe('stmt-5')
    expect(logs.entries[999].title).toBe('stmt-1004')
  })

  it('未读错误计数：error 累加、markOpened 清零、clear 归零', () => {
    const logs = useLogStore()
    logs.push({ level: 'info', kind: 'sql', title: 'SELECT 1' })
    logs.push({ level: 'error', kind: 'sql', title: 'Bad SQL', detail: 'boom' })
    logs.push({ level: 'error', kind: 'conn', title: 'lost' })
    expect(logs.unreadErrors).toBe(2)
    logs.markOpened()
    expect(logs.unreadErrors).toBe(0)
    // 已读后再新增仍会计数
    logs.push({ level: 'error', kind: 'sql', title: 'x' })
    expect(logs.unreadErrors).toBe(1)
    logs.clear()
    expect(logs.entries.length).toBe(0)
    expect(logs.unreadErrors).toBe(0)
  })

  it('sqlTitle 取首个非空行并截断', () => {
    expect(sqlTitle('\n  SELECT 1 FROM t;  ')).toBe('SELECT 1 FROM t;')
    const long = 'SELECT ' + 'x'.repeat(300)
    expect(sqlTitle(long).length).toBeLessThanOrEqual(161)
    expect(sqlTitle(long).endsWith('…')).toBe(true)
  })
})

describe('LogPanel — 底部面板渲染', () => {
  const flush = async (wrapper: { vm: { $nextTick: () => Promise<void> } }) => {
    await wrapper.vm.$nextTick()
    await new Promise((r) => setTimeout(r, 60))
  }

  it('渲染条目 / 级别徽标 / 点击展开详情 / 清空', async () => {
    const logs = useLogStore()
    logs.push({ level: 'info', kind: 'sql', title: 'SELECT 1', conn: 'dev', db: 'db1', durationMs: 12, rows: 1 })
    logs.push({ level: 'error', kind: 'sql', title: 'DELETE FROM t', detail: 'ORA-00942: 表或视图不存在', conn: 'dev' })

    const wrapper = mount(LogPanel, {
      global: {
        plugins: [
          ElementPlus,
          createI18n({ legacy: false, locale: 'zh-CN', messages: { 'zh-CN': zhCn, en } }),
        ],
        config: { warnHandler: () => {} },
      },
      attachTo: document.body,
    })
    await flush(wrapper)

    const rows = wrapper.findAll('.log-row')
    expect(rows.length).toBe(2)
    expect(wrapper.find('.b-error').text()).toBe(zhCn.logs.level.error)
    expect(wrapper.find('.log-conn').text()).toContain('dev@db1')

    // 点击错误行展开 detail
    await rows[1].trigger('click')
    await flush(wrapper)
    expect(wrapper.find('.log-detail').text()).toContain('ORA-00942')

    // 清空（log-head 中第一个圆形按钮 = 清空）
    await wrapper.find('.log-head').findAll('button')[0].trigger('click')
    await flush(wrapper)
    expect(wrapper.find('.log-empty').exists()).toBe(true)
    expect(logs.entries.length).toBe(0)
    wrapper.unmount()
  })

  it('挂载不产生 Vue 警告', async () => {
    const warns: string[] = []
    const wrapper = mount(LogPanel, {
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
      },
      attachTo: document.body,
    })
    await flush(wrapper)
    expect(warns).toEqual([])
    wrapper.unmount()
  })
})

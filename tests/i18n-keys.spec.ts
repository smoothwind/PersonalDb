import { describe, expect, it } from 'vitest'
import { DB_KIND_LABELS } from '../src/types/db'
import zhCn from '../src/i18n/locales/zh-CN'
import en from '../src/i18n/locales/en'

type Bag = Record<string, unknown>
const zh = zhCn as unknown as Bag
const enMsg = en as unknown as Bag

describe('i18n — 与 DbKind 枚举同步', () => {
  it('每种 DbKind 在 zh-CN / en 均有 dbKind 标签', () => {
    const zhKind = zh.dbKind as Record<string, string>
    const enKind = enMsg.dbKind as Record<string, string>
    for (const kind of Object.keys(DB_KIND_LABELS)) {
      expect(zhKind[kind], `zh-CN dbKind.${kind}`).toBeTruthy()
      expect(enKind[kind], `en dbKind.${kind}`).toBeTruthy()
    }
  })

  it('结果集预览文案（cell.preview）双端存在', () => {
    expect((zh.cell as Record<string, string>).preview).toBeTruthy()
    expect((enMsg.cell as Record<string, string>).preview).toBeTruthy()
  })
})

import { describe, expect, it } from 'vitest'
import { compareVersions } from '../src/utils/version'

describe('compareVersions — 更新提示的版本比较（GitHub tag vs 当前版本）', () => {
  it('数字段逐位比较，忽略 v 前缀与缺失段', () => {
    expect(compareVersions('v0.2.0', '0.1.0')).toBe(1)
    expect(compareVersions('0.1.0', 'v0.2.0')).toBe(-1)
    expect(compareVersions('v0.1.0', '0.1.0')).toBe(0)
    expect(compareVersions('1.0', '1.0.0')).toBe(0)
    expect(compareVersions('1.10.0', '1.9.0')).toBe(1)
  })

  it('预发布版本低于同号正式版', () => {
    expect(compareVersions('1.2.0-beta.1', '1.2.0')).toBe(-1)
    expect(compareVersions('1.2.0', '1.2.0-rc.1')).toBe(1)
    expect(compareVersions('1.2.0-rc.1', '1.2.0-beta.1')).toBe(1)
  })

  it('脏 tag / 空串不抛错（非数字段按 0）', () => {
    expect(compareVersions('v1.0.0', 'release')).toBe(1)
    expect(compareVersions('', '0.1.0')).toBe(-1)
    expect(compareVersions('', '')).toBe(0)
  })
})

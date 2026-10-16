import { describe, expect, it } from 'vitest'
import { connErrorDetail, describeConnError } from '../src/utils/connError'
import { i18n } from '../src/i18n'

const t = (k: string) => i18n.global.t(k)

describe('describeConnError 连接失败友好化', () => {
  it('Windows 套接字码映射到可读建议', () => {
    expect(describeConnError(new Error('数据库错误: Input/output error: 由于目标计算机积极拒绝，无法连接。 (os error 10061)'))).toBe(
      t('connError.refused'),
    )
    expect(describeConnError('os error 10060')).toBe(t('connError.timeout'))
    expect(describeConnError('os error 11001')).toBe(t('connError.dns'))
    expect(describeConnError('os error 10065')).toBe(t('connError.unreachable'))
  })

  it('Oracle ORA 码映射（服务未注册 / 账号锁定 / 认证失败）', () => {
    expect(describeConnError('ORA-12514: TNS:listener does not currently know of service')).toBe(
      t('connError.serviceUnknown'),
    )
    expect(describeConnError('ORA-28000: the account is locked')).toBe(t('connError.accountLocked'))
    expect(describeConnError('ORA-01017: invalid username/password')).toBe(t('connError.auth'))
  })

  it('OceanBase Oracle 租户与内置驱动缺失走既有提示', () => {
    expect(describeConnError('ERROR 0A000 (1235): Oracle tenant for current client driver is not supported')).toBe(
      t('connError.obOracleTenant'),
    )
    expect(describeConnError('oracle-bridge not found')).toBe(t('connError.oracleFeatureHint'))
  })

  it('认证 / 库不存在的驱动文案映射', () => {
    expect(describeConnError('Access denied for user \'x\'@\'y\' (using password: YES)')).toBe(t('connError.auth'))
    expect(describeConnError('database "nope" does not exist')).toBe(t('connError.unknownDatabase'))
  })

  it('已友好化的文本再次传入不会二次包装', () => {
    expect(describeConnError(new Error(t('connError.refused')))).toBe(t('connError.refused'))
  })

  it('未命中场景：清理包装前缀与换行并截断', () => {
    expect(describeConnError('数据库错误: something\n  odd   happened')).toBe('something odd happened')
    expect(connErrorDetail(new Error('原始细节'))).toBe('原始细节')
    expect(describeConnError(`长文本${'x'.repeat(300)}`).length).toBe(201)
  })
})

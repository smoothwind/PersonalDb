import { i18n } from '../i18n'

/**
 * 连接失败友好化（SPEC-CONN-1）。
 *
 * - `describeConnError`：给用户看的一行摘要（toast / 日志标题），命中已知场景时给出可操作建议；
 * - `connErrorDetail`：原始错误文本（日志详情，便于排查），不做任何改写；
 * - `cleanRaw`：未命中时清理 Rust 端包装前缀与换行，避免 toast 出现多层嵌套文案。
 *
 * 覆盖：Windows 套接字错误码（10060/10061/10065/11001 等）、Oracle ORA 码
 * （12541/12514/12505/12170/01017/28000）、MySQL/PG/驱动常见文案。
 */
export function connErrorDetail(err: unknown): string {
  return err instanceof Error ? err.message : String(err)
}

interface Rule {
  key: string
  needles: string[]
}

/** 顺序敏感：先具体（租户 / 服务名 / SID / 账号状态），后泛化（拒绝 / 超时） */
const RULES: Rule[] = [
  {
    key: 'connError.obOracleTenant',
    needles: ['Oracle tenant for current client driver is not supported', 'ERROR 0A000 (1235)'],
  },
  { key: 'connError.oracleFeatureHint', needles: ['--features oracle', 'oracle-bridge'] },
  {
    key: 'connError.unsupportedAuth',
    needles: ['unsupported authentication method', 'unsupported authentication'],
  },
  { key: 'connError.accountLocked', needles: ['ORA-28000'] },
  { key: 'connError.serviceUnknown', needles: ['ORA-12514', 'ORA-12505', 'ORA-12154'] },
  {
    key: 'connError.pgHba',
    needles: ['no pg_hba.conf entry'],
  },
  {
    key: 'connError.gaussAuth',
    needles: ['Invalid username/password,login denied', 'login denied'],
  },
  {
    key: 'connError.protocol',
    needles: ['unsupported frontend protocol', 'expected authentication request'],
  },
  { key: 'connError.pgStarting', needles: ['the database system is starting up'] },
  { key: 'connError.tls', needles: ['tls handshake', 'TLS handshake', 'SSL error', 'ssl error'] },
  {
    key: 'connError.dns',
    needles: ['os error 11001', 'Name or service not known', 'no such host', 'failed to lookup address'],
  },
  {
    key: 'connError.unreachable',
    needles: ['os error 10065', 'os error 10051', 'No route to host', 'Network is unreachable'],
  },
  { key: 'connError.timeout', needles: ['os error 10060', 'ETIMEDOUT', 'timed out', 'ORA-12170'] },
  {
    key: 'connError.refused',
    needles: ['os error 10061', 'Connection refused', 'connection refused', 'ORA-12541'],
  },
  {
    key: 'connError.auth',
    needles: ['Access denied for user', 'password authentication failed', 'ORA-01017', 'authentication failed'],
  },
  { key: 'connError.unknownDatabase', needles: ['Unknown database', 'does not exist'] },
  // SQLite：文件库特有错误（无法打开/被锁/文件不是数据库）
  {
    key: 'connError.sqliteFile',
    needles: ['unable to open database file', 'Unable to open database file'],
  },
  { key: 'connError.sqliteLocked', needles: ['database is locked', 'database table is locked'] },
  { key: 'connError.sqliteNotDb', needles: ['file is not a database', 'malformed'] },
]

/** 一键友好化：命中已知场景返回可操作的一行摘要，否则返回清理后的原始文本 */
export function describeConnError(err: unknown): string {
  const msg = connErrorDetail(err)
  const rule = RULES.find((r) => r.needles.some((n) => msg.includes(n)))
  if (rule) return i18n.global.t(rule.key)
  return cleanRaw(msg)
}

const WRAPPER_PREFIXES = [
  '数据库错误: ',
  'Database error: ',
  'IO 错误: ',
  'Io 错误: ',
  'Input/output error: ',
  'Error: ',
]

/** 兜底清理：折叠空白为单行、剥离多层包装前缀、超长截断 */
function cleanRaw(msg: string): string {
  let out = msg.replace(/\s+/g, ' ').trim()
  let changed = true
  while (changed) {
    changed = false
    for (const p of WRAPPER_PREFIXES) {
      if (out.startsWith(p)) {
        out = out.slice(p.length).trim()
        changed = true
      }
    }
  }
  return out.length > 200 ? `${out.slice(0, 200)}…` : out
}

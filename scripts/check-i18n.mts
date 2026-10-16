/**
 * i18n 完整性校验：
 * 1. 对比 zh-CN 与 en 的 key 集合（递归叶子节点）；
 * 2. 对比每个 key 的插值占位符 {name} 集合（防运行时插值缺失）。
 * 用法：npm run check:i18n；有差异时输出明细并以非零码退出（CI 拦截）。
 */
import zh from '../src/i18n/locales/zh-CN.ts'
import en from '../src/i18n/locales/en.ts'

function leafKeys(obj: unknown, prefix = '', out: string[] = []): string[] {
  if (obj && typeof obj === 'object') {
    for (const [k, v] of Object.entries(obj as Record<string, unknown>)) {
      leafKeys(v, prefix ? `${prefix}.${k}` : k, out)
    }
  } else {
    out.push(prefix)
  }
  return out
}

function getAt(root: unknown, path: string): unknown {
  let cur: unknown = root
  for (const seg of path.split('.')) {
    if (cur && typeof cur === 'object') cur = (cur as Record<string, unknown>)[seg]
    else return undefined
  }
  return cur
}

/** 提取 {name} 插值占位符（vue-i18n 字面量 {'@'}/{'{'} 不含词字符，不会误报） */
function params(v: unknown): Set<string> {
  const out = new Set<string>()
  if (typeof v === 'string') {
    for (const m of v.matchAll(/\{([A-Za-z_]\w*)\}/g)) out.add(m[1])
  }
  return out
}

const zhKeys = new Set(leafKeys(zh))
const enKeys = new Set(leafKeys(en))

const missingInEn = [...zhKeys].filter((k) => !enKeys.has(k))
const missingInZh = [...enKeys].filter((k) => !zhKeys.has(k))

const paramMismatches: string[] = []
for (const k of zhKeys) {
  if (!enKeys.has(k)) continue
  const pz = params(getAt(zh, k))
  const pe = params(getAt(en, k))
  const diff = [...pz].filter((p) => !pe.has(p)).concat([...pe].filter((p) => !pz.has(p)))
  if (diff.length > 0) paramMismatches.push(`${k}: {${diff.join('},{')}}`)
}

if (missingInEn.length === 0 && missingInZh.length === 0 && paramMismatches.length === 0) {
  console.log(`i18n OK: ${zhKeys.size} keys, zh/en fully in sync`)
} else {
  if (missingInEn.length > 0) {
    console.error(`Missing in en.ts (${missingInEn.length}):`)
    for (const k of missingInEn) console.error(`  - ${k}`)
  }
  if (missingInZh.length > 0) {
    console.error(`Missing in zh-CN.ts (${missingInZh.length}):`)
    for (const k of missingInZh) console.error(`  - ${k}`)
  }
  if (paramMismatches.length > 0) {
    console.error(`Placeholder mismatch (${paramMismatches.length}):`)
    for (const k of paramMismatches) console.error(`  - ${k}`)
  }
  process.exit(1)
}

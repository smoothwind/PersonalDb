/**
 * 版本号比较（更新提示用）。
 *
 * 只面向形如 `v1.2.3` / `1.2.3` / `1.2.3-beta.1` 的语义化版本：
 * 前缀 `v` 忽略；数字段逐位比较（缺失段按 0 计）；数字相同时**带预发布后缀的更低**
 * （`1.2.0-beta` < `1.2.0`）。非数字段按 0 处理，避免脏 tag 抛错。
 */

/** 拆出版本的数字段与预发布后缀：`v1.2.3-beta.1` → nums=[1,2,3], pre='beta.1' */
function parseVersion(version: string): { nums: number[]; pre: string } {
  const clean = version.trim().replace(/^v/i, '')
  const [core, ...rest] = clean.split('-')
  const nums = core.split('.').map((seg) => {
    const n = Number.parseInt(seg, 10)
    return Number.isFinite(n) ? n : 0
  })
  return { nums, pre: rest.join('-') }
}

/**
 * 比较两个版本号：`a` 比 `b` 新返回 1，旧返回 -1，相同返回 0。
 * 用于判断「GitHub 最新 Release 的 tag 是否高于当前应用版本」。
 */
export function compareVersions(a: string, b: string): number {
  const va = parseVersion(a)
  const vb = parseVersion(b)
  const len = Math.max(va.nums.length, vb.nums.length)
  for (let i = 0; i < len; i += 1) {
    const diff = (va.nums[i] ?? 0) - (vb.nums[i] ?? 0)
    if (diff !== 0) return diff > 0 ? 1 : -1
  }
  if (va.pre === vb.pre) return 0
  // 正式版高于同号预发布版
  if (!va.pre) return 1
  if (!vb.pre) return -1
  return va.pre > vb.pre ? 1 : -1
}

/**
 * SQL 模板 store：启动时拉取后端全量生效模板（内置 + 用户覆盖），
 * 供分页续拉（`paged.*`）与双击预览（`preview.*`）运行时使用。
 *
 * 拉取失败不阻塞使用：utils/sqlTemplates.ts 保持内置兜底。
 */
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { sqlTemplatesOverrides } from '../api/db'
import { setResolvedTemplates } from '../utils/sqlTemplates'

export const useSqlTemplatesStore = defineStore('sqlTemplates', () => {
  const loaded = ref(false)

  /** 惰性加载（幂等）；设置页保存后调 `reload()` 强制刷新 */
  async function ensureLoaded(): Promise<void> {
    if (loaded.value) return
    await reload()
  }

  /** 强制从后端重拉用户覆盖表 */
  async function reload(): Promise<void> {
    try {
      setResolvedTemplates(await sqlTemplatesOverrides())
    } catch {
      // 后端不可达：保持内置兜底，不阻塞查询
    }
    loaded.value = true
  }

  return { loaded, ensureLoaded, reload }
})

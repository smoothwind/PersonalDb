import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { workspaceLoad, workspaceSave, workspaceSetDir } from '../api/db'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import { i18n } from '../i18n'
import { useTabsStore } from './tabs'
import type { Workspace } from '../types/db'

/**
 * 工作区：命名容器，管理工作区 Tab 集合与文件保存位置（导出目录）。
 * 数据持久化于存储体系 workspaces.json；Tab 变化自动快照（防抖）。
 */
export const useWorkspaceStore = defineStore('workspaces', () => {
  const list = ref<Workspace[]>([])
  const currentId = ref('')
  let loaded = false

  const tabsStore = useTabsStore()

  const current = computed(() => list.value.find((w) => w.id === currentId.value) ?? null)

  let persistTimer: number | undefined

  async function persist(): Promise<void> {
    await workspaceSave({ currentId: currentId.value, list: list.value })
  }

  function persistDebounced(): void {
    window.clearTimeout(persistTimer)
    persistTimer = window.setTimeout(() => void persist(), 500)
  }

  /** 加载工作区文件（首次无数据时创建默认工作区） */
  async function load(): Promise<void> {
    if (loaded) return
    loaded = true
    const data = await workspaceLoad().catch(() => null)
    list.value = data?.list ?? []
    currentId.value = data?.currentId ?? ''
    if (list.value.length === 0) {
      list.value = [
        { id: crypto.randomUUID(), name: i18n.global.t('workspace.defaultName'), tabs: [] },
      ]
      currentId.value = list.value[0].id
      await persist()
    } else if (!current.value) {
      currentId.value = list.value[0].id
    }
  }

  /** 启动时恢复当前工作区的页签 */
  function restoreTabs(): void {
    tabsStore.importSnapshot(current.value?.tabs ?? [])
  }

  /** 将当前页签快照写入当前工作区（防抖持久化） */
  function syncTabs(): void {
    const w = current.value
    if (!w) return
    w.tabs = tabsStore.exportSnapshot()
    persistDebounced()
  }

  /**
   * 新建工作区：**必须指定其在磁盘上的存储位置**（目录选择对话框，取消则不创建）。
   * dir 为空表示使用默认 Workspaces 存储类别目录（仅旧数据/默认工作区如此）。
   */
  async function create(name: string): Promise<void> {
    syncTabs()
    const dir = await openDialog({
      directory: true,
      title: i18n.global.t('workspace.selectDir', { name }),
    })
    if (!dir || Array.isArray(dir)) return
    const w: Workspace = { id: crypto.randomUUID(), name, tabs: [], dir }
    list.value = [...list.value, w]
    currentId.value = w.id
    tabsStore.importSnapshot([])
    await persist()
  }

  async function rename(id: string, name: string): Promise<void> {
    list.value = list.value.map((w) => (w.id === id ? { ...w, name } : w))
    await persist()
  }

  async function remove(id: string): Promise<void> {
    if (list.value.length <= 1) return
    list.value = list.value.filter((w) => w.id !== id)
    if (currentId.value === id) {
      currentId.value = list.value[0].id
      tabsStore.importSnapshot(current.value?.tabs ?? [])
    }
    await persist()
  }

  async function switchTo(id: string): Promise<void> {
    if (id === currentId.value) return
    syncTabs() // 保存当前工作区页签
    currentId.value = id
    tabsStore.importSnapshot(current.value?.tabs ?? [])
    await persist()
  }

  async function setExportDir(id: string, dir: string | null): Promise<void> {
    list.value = list.value.map((w) =>
      w.id === id ? { ...w, exportDir: dir ?? undefined } : w,
    )
    await persist()
  }

  /** 更改工作区磁盘存储位置（后端迁移数据文件并更新索引） */
  async function setDir(id: string, dir: string): Promise<void> {
    await workspaceSetDir(id, dir)
    const w = list.value.find((x) => x.id === id)
    if (w) w.dir = dir
    await persist()
  }

  return {
    list,
    currentId,
    current,
    load,
    restoreTabs,
    syncTabs,
    create,
    rename,
    remove,
    switchTo,
    setExportDir,
    setDir,
  }
})

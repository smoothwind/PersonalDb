import { defineComponent, h } from 'vue'
import type { Component } from 'vue'
import AppIcon from '../components/AppIcon.vue'
import { APP_ICONS } from './icons'
import type { DbObjectType } from '../types/db'

/**
 * 统一图标体系：对象类型 → 图标 key（utils/icons.ts 的 APP_ICONS）。
 * 连接树节点、查询 Tab 页签共用同一映射，保证同类对象图标一致。
 * 组件内禁止内联图标判断，一律经此映射取用。
 */
export const OBJECT_ICON_NAMES: Record<string, string> = {
  // 特殊节点
  query: 'obj_query',
  connection: 'obj_connection',
  database: 'obj_database',
  folder: 'obj_folder',
  // 对象类型
  table: 'obj_table',
  view: 'obj_view',
  index: 'obj_index',
  sequence: 'obj_sequence',
  function: 'obj_function',
  procedure: 'obj_procedure',
  trigger: 'obj_trigger',
  // 会话管理
  session: 'obj_session',
}

/** 图标 key（与图标映射键同名） */
export function iconKeyOfObjectType(t: DbObjectType): string {
  return t
}

const warned = new Set<string>()

/** 图标名称（APP_ICONS 键）
 *  解析优先级：APP_ICONS 直连 key（obj- / act- / st- 系列，例如 objectIcon('act_run')）
 *  > 对象类型别名（table/session/… → obj-*）> 兜底 obj_query（开发期告警，见图标红线）。
 */
export function iconNameOf(key?: string): string {
  if (key && APP_ICONS[key]) return key
  if (key && OBJECT_ICON_NAMES[key]) return OBJECT_ICON_NAMES[key]
  if (key && !warned.has(key)) {
    warned.add(key)
    if (import.meta.env.DEV) {
      console.warn(`[icons] 未登记的图标 key: "${key}"（将回退 obj_query），请检查事实来源或补登记`)
    }
  }
  return OBJECT_ICON_NAMES.query
}

const cache = new Map<string, Component>()

/** 兼容旧 API：返回可被 `component :is` 渲染的图标组件（1em，随 el-icon 缩放） */
export function objectIcon(key?: string): Component {
  const name = iconNameOf(key)
  let comp = cache.get(name)
  if (!comp) {
    comp = defineComponent({
      name: `AppIconProxy_${name}`,
      render: () => h(AppIcon, { name }),
    })
    cache.set(name, comp)
  }
  return comp
}

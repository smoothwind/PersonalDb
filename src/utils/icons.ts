/**
 * 全系统统一 SVG 图标库（24×24 线性风格，currentColor 适配明暗主题）。
 *
 * 命名规范：
 * - obj-*   对象类（表/视图/索引/…）
 * - act-*   动作类（运行/保存/导出/…）
 * - st-*    状态类（丢失/失效/分区）
 * 数据库品牌多色 Logo 不在此处（见 DbLogo.vue）。
 *
 * 值为 SVG 内部标记（inner markup），由 AppIcon.vue 以 v-html 渲染；
 * 组件内禁止内联图标判断，统一经 utils/objectIcons.ts 映射取用。
 */

export const APP_ICONS: Record<string, string> = {
  // ================= 对象类 =================
  obj_query:
    '<rect x="4" y="3" width="16" height="18" rx="2"/><path d="M8 8h8M8 12h8M8 16h5"/>',
  obj_connection:
    '<circle cx="12" cy="12" r="9"/><path d="M13 6.5 8.5 13h3l-1 4.5L15.5 11h-3z"/>',
  obj_database:
    '<ellipse cx="12" cy="5.5" rx="8" ry="3"/><path d="M4 5.5v6.5c0 1.66 3.58 3 8 3s8-1.34 8-3V5.5"/><path d="M4 12v6.5c0 1.66 3.58 3 8 3s8-1.34 8-3V12"/>',
  obj_folder:
    '<path d="M3 6.5A1.5 1.5 0 0 1 4.5 5h4.6c.5 0 1 .25 1.3.66L11.5 7h8A1.5 1.5 0 0 1 21 8.5v9A1.5 1.5 0 0 1 19.5 19h-15A1.5 1.5 0 0 1 3 17.5z"/>',
  obj_table:
    '<rect x="3.5" y="4.5" width="17" height="15" rx="1.8"/><path d="M3.5 9.5h17M9.5 9.5V19.5M15.5 9.5V19.5M3.5 14.5h17"/>',
  obj_view:
    '<path d="M2.5 12S6 5.8 12 5.8 21.5 12 21.5 12 18 18.2 12 18.2 2.5 12 2.5 12Z"/><circle cx="12" cy="12" r="2.6"/>',
  obj_index:
    '<circle cx="12" cy="5" r="2.2"/><circle cx="5.5" cy="18" r="2.2"/><circle cx="12" cy="18" r="2.2"/><circle cx="18.5" cy="18" r="2.2"/><path d="M12 7.2v3.6M12 10.8 6.3 16.2M12 10.8l5.7 5.4M12 10.8v5"/>',
  obj_sequence:
    '<path d="M5 7h6M5 12h6M5 17h6"/><path d="M15 5.5 18.5 9 15 12.5M18.5 9v5.5a3 3 0 0 1-3 3"/>',
  obj_function:
    '<path d="M10 4.5c-2.2 0-3.2 1.1-3.2 3v9c0 1.9-1 3-3.3 3"/><path d="M5.2 10.5h6"/><path d="m12.8 10.5 6.4 7M19.2 10.5l-6.4 7"/>',
  obj_procedure:
    '<rect x="4" y="4" width="16" height="16" rx="2.5"/><path d="M10 8.5c-1.4 0-2 .7-2 1.8v1c0 .8-.4 1.3-1.2 1.5v.4c.8.2 1.2.7 1.2 1.5v1c0 1.1.6 1.8 2 1.8M14 8.5c1.4 0 2 .7 2 1.8v1c0 .8.4 1.3 1.2 1.5v.4c-.8.2-1.2.7-1.2 1.5v1c0 1.1-.6 1.8-2 1.8"/>',
  obj_trigger:
    '<path d="M13 3 6.5 13.5h4L10 21l6.5-10.5h-4z"/><path d="M5 21h14"/>',
  obj_session:
    '<circle cx="9" cy="8" r="3.2"/><path d="M3.5 19c.6-3 2.9-4.6 5.5-4.6s4.9 1.6 5.5 4.6"/><circle cx="16.8" cy="9.2" r="2.4"/><path d="M15.6 14.6c2.3.1 4.2 1.5 4.8 4"/>',
  obj_key:
    '<circle cx="8" cy="14.5" r="4"/><path d="m11 11.5 8.5-8.5M15.5 7l2.5 2.5M18 4.5 20.5 7"/>',
  obj_column:
    '<rect x="4" y="4" width="16" height="16" rx="1.8"/><path d="M9.3 4v16M14.7 4v16"/>',
  obj_filter:
    '<path d="M4 5h16l-6.2 7.2v5.3L10.2 20v-7.8z"/>',

  // ================= 动作类 =================
  act_run: '<path d="M7 4.8v14.4L19.2 12z"/>',
  act_stop: '<rect x="6" y="6" width="12" height="12" rx="1.6"/>',
  act_commit: '<circle cx="12" cy="12" r="8.5"/><path d="m8 12.2 2.8 2.8L16.2 9.5"/>',
  act_rollback: '<path d="M4 5v5h5"/><path d="M4.6 10A8 8 0 1 1 12 20a8 8 0 0 1-7.4-4.9"/>',
  act_refresh:
    '<path d="M20 5.5V10h-4.5"/><path d="M4 18.5V14h4.5"/><path d="M5.6 9.5A7 7 0 0 1 17.8 8l2.2 2M18.4 14.5A7 7 0 0 1 6.2 16l-2.2-2"/>',
  act_save:
    '<path d="M5 3.5h11L20.5 8v11a1.5 1.5 0 0 1-1.5 1.5H5A1.5 1.5 0 0 1 3.5 19v-14A1.5 1.5 0 0 1 5 3.5Z"/><path d="M7.5 3.5V9h8V3.5M7.5 20.5v-6h9v6"/>',
  act_open:
    '<path d="M3 5.5A1.5 1.5 0 0 1 4.5 4h4.4c.5 0 1 .25 1.3.66L11.3 6h8.2A1.5 1.5 0 0 1 21 7.5V10"/><path d="m3 17.5 2.4-6.4A1.5 1.5 0 0 1 6.8 10h14.7a1 1 0 0 1 .94 1.34l-2.2 6.32a1.5 1.5 0 0 1-1.42 1.01H4.5A1.5 1.5 0 0 1 3 17.5Z"/>',
  act_export:
    '<path d="M14 3.5h6.5V10"/><path d="M20.5 3.5 12 12"/><path d="M19 14.5v4A1.5 1.5 0 0 1 17.5 20h-11A1.5 1.5 0 0 1 5 18.5v-11A1.5 1.5 0 0 1 6.5 6h4"/>',
  act_import:
    '<path d="M14 10.5h6.5V4"/><path d="M20.5 10.5 12 2.9M19 14.5v4A1.5 1.5 0 0 1 17.5 20h-11A1.5 1.5 0 0 1 5 18.5v-11A1.5 1.5 0 0 1 6.5 6h4"/>',
  act_copy:
    '<rect x="8.5" y="8.5" width="12" height="12" rx="2"/><path d="M15.5 5.5v-1a1.5 1.5 0 0 0-1.5-1.5H5A1.5 1.5 0 0 0 3.5 4.5v9A1.5 1.5 0 0 0 5 15h1"/>',
  act_edit:
    '<path d="m14.5 5 4.5 4.5L8.5 20H4v-4.5z"/><path d="m12.5 7 4.5 4.5"/>',
  act_delete:
    '<path d="M4 6h16M9.5 6V4.5A1.5 1.5 0 0 1 11 3h2a1.5 1.5 0 0 1 1.5 1.5V6"/><path d="M6.5 6 7.3 19a1.8 1.8 0 0 0 1.8 1.7h5.8a1.8 1.8 0 0 0 1.8-1.7L17.5 6"/><path d="M10 10.5v6M14 10.5v6"/>',
  act_plus: '<path d="M12 5v14M5 12h14"/>',
  act_close: '<path d="m6 6 12 12M18 6 6 18"/>',
  act_reconnect:
    '<path d="M20 5.5V10h-4.5"/><path d="M17.8 8A7 7 0 1 0 19 12"/><path d="M10.5 9.5 8 12l2.5 2.5M13.5 9.5 16 12l-2.5 2.5"/>',
  act_search:
    '<circle cx="11" cy="11" r="6.5"/><path d="m20.5 20.5-4.5-4.5"/>',
  act_settings:
    '<path d="M4 7h10M18 7h2M4 12h2M10 12h10M4 17h10M18 17h2"/><circle cx="16" cy="7" r="1.8"/><circle cx="8" cy="12" r="1.8"/><circle cx="16" cy="17" r="1.8"/>',
  act_help:
    '<circle cx="12" cy="12" r="9"/><path d="M9.5 9.2a2.6 2.6 0 0 1 5.1.7c0 1.8-2.6 2.1-2.6 3.6"/><circle cx="12" cy="17" r="0.4" fill="currentColor"/>',
  act_monitor:
    '<rect x="3" y="4" width="18" height="13" rx="2"/><path d="M12 17v3.5M8.5 20.5h7"/><path d="M6.5 11h2.4l1.6-3.5 2 5.5 1.5-2.7h3.5"/>',
  act_split:
    '<rect x="3.5" y="4.5" width="17" height="15" rx="1.8"/><path d="M12 4.5v15"/>',
  act_split_stack:
    '<rect x="3.5" y="4.5" width="17" height="15" rx="1.8"/><path d="M3.5 12h17"/>',
  act_unsplit:
    '<rect x="3.5" y="4.5" width="17" height="15" rx="1.8"/>',
  act_panel:
    '<rect x="3.5" y="4.5" width="17" height="15" rx="1.8"/><path d="M9.5 4.5v15"/>',
  act_workspace:
    '<rect x="3" y="4.5" width="18" height="15" rx="2"/><path d="M3 9h18"/><path d="M6 6.75h2.6"/>',
  act_expand:
    '<path d="M9 4H4v5M15 4h5v5M9 20H4v-5M15 20h5v-5"/>',
  act_fold:
    '<path d="M4 9h5V4M20 9h-5V4M4 15h5v5M20 15h-5v5"/>',
  act_beautify:
    '<path d="m6 18 9.5-9.5"/><path d="m4.8 19.2a2 2 0 0 1 0-2.8l.5-.5 2.8 2.8-.5.5a2 2 0 0 1-2.8 0Z"/><path d="M17.5 3.5 18.3 6l2.5.8-2.5.8-.8 2.5-.8-2.5-2.5-.8L16.7 6z"/>',
  act_history:
    '<circle cx="12" cy="12" r="8.5"/><path d="M12 7v5.2l3.5 2"/>',
  act_format:
    '<path d="M4 6h16M7 12h13M4 18h16"/><circle cx="4.8" cy="12" r="0.5" fill="currentColor"/>',
  act_upper:
    '<path d="m2.5 17 4-10 4 10M3.8 14h5.4"/><path d="m13.5 17 4-10 4 10M14.8 14h5.4"/>',
  act_lower:
    '<path d="M2.5 17c2.8 1 5.5-.3 5.5-3.2V8.6M8 13c-4.5 0-6 1.2-6 2.7 0 1.2 1.2 1.8 2.7 1.8 1.8 0 3.3-1 3.3-2.7"/><path d="M14.5 17c2.8 1 5.5-.3 5.5-3.2V8.6M20 13c-4.5 0-6 1.2-6 2.7 0 1.2 1.2 1.8 2.7 1.8 1.8 0 3.3-1 3.3-2.7"/>',
  act_plan:
    '<path d="M3 4.5h18"/><path d="M3 12h12M3 19.5h16"/><circle cx="18.8" cy="12" r="1.6"/>',
  act_tx:
    '<path d="M4 8h13M14 4.5 17.5 8 14 11.5"/><path d="M20 16H7M10 12.5 6.5 16l3.5 3.5"/>',
  act_float:
    '<rect x="3.5" y="9.5" width="11" height="11" rx="1.8"/><path d="M13.5 4H20v6.5"/><path d="M20 4 12 12"/>',
  act_hide:
    '<rect x="3.5" y="4" width="17" height="9.5" rx="1.8"/><path d="M12 16.5v-5"/><path d="m8.5 13.5 3.5 3.5 3.5-3.5"/>',
  act_dock:
    '<rect x="9.5" y="9.5" width="11" height="11" rx="1.8"/><path d="M10.5 4H5.5A1.5 1.5 0 0 0 4 5.5v5"/><path d="M4 4l6.5 6.5"/>',
  act_prev:
    '<path d="M12 20V7"/><path d="m6.5 12.5 5.5-5.5 5.5 5.5"/><path d="M5 21h14"/>',
  act_undo:
    '<path d="M7.5 8.5H15a4.5 4.5 0 0 1 0 9H8"/><path d="m10.5 5.5-3 3 3 3"/>',
  act_redo:
    '<path d="M16.5 8.5H9a4.5 4.5 0 0 0 0 9h7"/><path d="m13.5 5.5 3 3-3 3"/>',
  act_next:
    '<path d="M12 4v13"/><path d="m6.5 11.5 5.5 5.5 5.5-5.5"/><path d="M5 21h14"/>',
  // 主题切换（明暗对比圆）
  act_theme:
    '<circle cx="12" cy="12" r="8.5"/><path d="M12 3.5a8.5 8.5 0 0 1 0 17z" fill="currentColor" stroke="none"/>',
  // 驱动管理（连接器/插头）
  act_driver:
    '<path d="M9 3.5v4M15 3.5v4"/><path d="M7 7.5h10V11a5 5 0 0 1-10 0z"/><path d="M12 16v4.5"/>',
  // 检查更新（下载装入托盘）
  act_update:
    '<path d="M12 4v9.5"/><path d="m8.5 10.5 3.5 3.5 3.5-3.5"/><path d="M4.5 19.5h15"/>',
  // 表格视图（结果集网格）
  act_view_grid:
    '<rect x="3.5" y="4.5" width="17" height="15" rx="1.8"/><path d="M3.5 9.5h17M3.5 14.5h17M9.5 4.5v15"/>',
  // 关于 / 信息
  act_info:
    '<circle cx="12" cy="12" r="8.5"/><path d="M12 11v5.5"/><circle cx="12" cy="7.8" r="0.6" fill="currentColor"/>',
  // 代码 / 编辑器
  act_code:
    '<path d="m9 8-4.5 4L9 16"/><path d="m15 8 4.5 4L15 16"/>',
  // 只读代码视图（窗口 + 代码）
  act_code_view:
    '<rect x="3.5" y="4.5" width="17" height="15" rx="1.8"/><path d="m10 10-2.2 2 2.2 2"/><path d="m14 10 2.2 2-2.2 2"/>',
  // 键盘 / 快捷键
  act_keyboard:
    '<rect x="2.5" y="6.5" width="19" height="11" rx="2"/><path d="M6 10h.01M9.5 10h.01M13 10h.01M16.5 10h.01M7.5 13.5h9"/>',

  // ================= 状态类 =================
  st_lost:
    '<path d="M12.5 5.5 9 9.5H4.5v5H9l3.5 4z"/><path d="M17 9.5l5 5M22 9.5l-5 5"/>',
  st_invalid:
    '<path d="M12 3.5 22 20H2z"/><path d="M12 9.5v5"/><circle cx="12" cy="17.3" r="0.5" fill="currentColor"/>',
  st_partition:
    '<path d="m12 3.5 9 4.5-9 4.5L3 8z"/><path d="m5.2 11.5-2.2 1.1 9 4.5 9-4.5-2.2-1.1"/><path d="m5.2 16-2.2 1.1 9 4.5 9-4.5-2.2-1.1"/>',
}

/** 图标是否存在（用于防御性渲染与测试） */
export function hasIcon(name: string): boolean {
  return name in APP_ICONS
}

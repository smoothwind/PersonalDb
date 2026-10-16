import { defineStore } from 'pinia'
import { computed, reactive, ref } from 'vue'

/** 工作台面板 id：连接管理器 / 功能区页签容器 / 日志查看器 */
export type PanelId = 'conn' | 'tabs' | 'log'
/** 停靠区：左 / 右 / 下 / 中（中心常驻，不允许为空） */
export type ZoneId = 'left' | 'right' | 'bottom' | 'center'

/** 面板放置状态 */
export interface PanelState {
  /** 所在停靠区；null = 已隐藏（可通过菜单/状态栏恢复） */
  zone: ZoneId | null
  /** 悬浮窗几何（zone 为 null 且 float 存在 = 悬浮中） */
  float?: { x: number; y: number; w: number; h: number }
  /** 悬浮窗最小化（收进右下角层叠条） */
  minimized?: boolean
  /** 最近一次停靠区（悬浮/隐藏后「回收」目标） */
  lastZone: ZoneId
}

export interface FloatGeom {
  x: number
  y: number
  w: number
  h: number
}

const STORE_KEY = 'personaldb.layout'

type LayoutMap = Record<PanelId, PanelState>

function defaultLayout(): LayoutMap {
  return {
    conn: { zone: 'left', lastZone: 'left' },
    tabs: { zone: 'center', lastZone: 'center' },
    log: { zone: 'bottom', lastZone: 'bottom' },
  }
}

function defaultSizes(): Record<Exclude<ZoneId, 'center'>, number> {
  return { left: 260, right: 220, bottom: 200 }
}

function loadPersisted(): { panels: LayoutMap; sizes: Record<Exclude<ZoneId, 'center'>, number> } {
  try {
    const raw = localStorage.getItem(STORE_KEY)
    if (!raw) return { panels: defaultLayout(), sizes: defaultSizes() }
    const parsed = JSON.parse(raw) as {
      panels?: Partial<LayoutMap>
      sizes?: Partial<Record<Exclude<ZoneId, 'center'>, number>>
    }
    const base = defaultLayout()
    const panels = base
    for (const id of Object.keys(base) as PanelId[]) {
      const p = parsed.panels?.[id]
      if (p && typeof p === 'object') {
        panels[id] = {
          zone: p.zone ?? base[id].zone,
          lastZone: p.lastZone ?? base[id].lastZone,
          float: p.float,
          minimized: Boolean(p.minimized),
        }
      }
    }
    const sizes = { ...defaultSizes(), ...(parsed.sizes ?? {}) }
    return { panels, sizes }
  } catch {
    return { panels: defaultLayout(), sizes: defaultSizes() }
  }
}

/** 工作台布局：面板停靠 / 交换 / 悬浮 / 层叠 */
export const useLayoutStore = defineStore('layout', () => {
  const persisted = loadPersisted()

  const panels = reactive<LayoutMap>(persisted.panels)
  const sizes = reactive(persisted.sizes)
  /** 悬浮窗焦点层级（递增；Element Plus 弹层基线 2000，浮动层从 100 起步） */
  const zTop = ref(100)
  /** 当前拖拽中的面板（显示停靠投放遮罩） */
  const dragging = ref<PanelId | null>(null)
  /** 连接树刷新序号（保存连接后自增，各处 PanelBody 监听刷新） */
  const refreshSeq = ref(0)

  function bumpRefresh(): void {
    refreshSeq.value++
  }

  let persistTimer: number | undefined
  function persist(): void {
    window.clearTimeout(persistTimer)
    persistTimer = window.setTimeout(() => {
      try {
        localStorage.setItem(STORE_KEY, JSON.stringify({ panels, sizes }))
      } catch {
        /* 配额/隐私模式失败可忽略 */
      }
    }, 150)
  }

  const FLOAT_W = 480
  const FLOAT_H = 360
  const FLOAT_STEP = 28

  function panelOf(zone: ZoneId): PanelId | null {
    return (Object.keys(panels) as PanelId[]).find((id) => panels[id].zone === zone) ?? null
  }

  const leftPanel = computed(() => panelOf('left'))
  const rightPanel = computed(() => panelOf('right'))
  const bottomPanel = computed(() => panelOf('bottom'))
  const centerPanel = computed(() => panelOf('center'))

  /** 悬浮中的面板（含最小化） */
  const floatingPanels = computed(() =>
    (Object.keys(panels) as PanelId[]).filter((id) => panels[id].zone === null && panels[id].float),
  )

  function clampGeom(g: FloatGeom): FloatGeom {
    const vw = window.innerWidth
    const vh = window.innerHeight
    return {
      w: Math.min(1200, Math.max(280, g.w)),
      h: Math.min(1400, Math.max(180, g.h)),
      x: Math.min(vw - 120, Math.max(-40, g.x)),
      y: Math.min(vh - 60, Math.max(0, g.y)),
    }
  }

  function nextFloatGeom(): FloatGeom {
    const open = floatingPanels.value.filter((id) => !panels[id].minimized).length
    const offset = Math.min(open, 6) * FLOAT_STEP
    return clampGeom({
      x: 120 + offset,
      y: 90 + offset,
      w: FLOAT_W,
      h: FLOAT_H,
    })
  }

  /** 面板移动到目标区；目标区已有面板时交换（被换下面板回到来源；来源为悬浮/隐藏则转悬浮） */
  function move(id: PanelId, zone: ZoneId): void {
    if (panels[id].zone === zone) return
    const occupant = panelOf(zone)
    const from = panels[id]
    const fromZone = from.zone
    from.float = undefined
    from.minimized = false
    from.zone = zone
    from.lastZone = zone
    if (occupant && occupant !== id) {
      const occ = panels[occupant]
      if (fromZone !== null) {
        occ.zone = fromZone
        occ.lastZone = fromZone
        occ.float = undefined
      } else {
        // 来源是悬浮/隐藏：占用者转为悬浮
        occ.zone = null
        occ.float = nextFloatGeom()
      }
    }
    persist()
  }

  /** 隐藏面板（中心区面板不允许隐藏，保持中心常驻） */
  function hide(id: PanelId): void {
    if (panels[id].zone === 'center') return
    panels[id].zone = null
    persist()
  }

  /** 恢复显示：回最近停靠区；被占用时占用者转悬浮 */
  function show(id: PanelId): void {
    const target = panels[id].lastZone ?? 'bottom'
    const occupant = panelOf(target)
    if (occupant && occupant !== id) {
      float(occupant)
    }
    panels[id].zone = target
    panels[id].float = undefined
    panels[id].minimized = false
    persist()
  }

  /** 转悬浮（级联偏移 + 置焦） */
  function float(id: PanelId): void {
    const p = panels[id]
    if (p.zone === 'center') {
      // 中心面板悬浮会留空中心：拒绝（中心必须常驻内容）
      return
    }
    if (p.zone) p.lastZone = p.zone
    p.zone = null
    p.float = p.float ? clampGeom(p.float) : nextFloatGeom()
    p.minimized = false
    focusFloat(id)
    persist()
  }

  /** 悬浮窗回收停靠（回最近区） */
  function dockBack(id: PanelId): void {
    const p = panels[id]
    if (p.zone !== null) return
    show(id)
  }

  function focusFloat(id: PanelId): void {
    const p = panels[id]
    if (p.zone !== null || !p.float) return
    zTop.value += 1
    focusZ.set(id, zTop.value)
    persist()
  }

  /** 每个悬浮窗的 z（focusFloat 时递增） */
  const focusZ = reactive(new Map<PanelId, number>())
  function zOf(id: PanelId): number {
    return focusZ.get(id) ?? 100
  }

  function minimize(id: PanelId): void {
    const p = panels[id]
    if (p.zone !== null || !p.float) return
    p.minimized = true
    persist()
  }

  function restore(id: PanelId): void {
    const p = panels[id]
    if (p.zone !== null || !p.float) return
    p.minimized = false
    focusFloat(id)
    persist()
  }

  function setGeom(id: PanelId, patch: Partial<FloatGeom>): void {
    const p = panels[id]
    if (!p.float) return
    p.float = clampGeom({ ...p.float, ...patch })
    persist()
  }

  function resizeZone(zone: Exclude<ZoneId, 'center'>, size: number): void {
    const limits: Record<Exclude<ZoneId, 'center'>, [number, number]> = {
      left: [200, 560],
      right: [140, 420],
      bottom: [120, 480],
    }
    const [min, max] = limits[zone]
    sizes[zone] = Math.min(max, Math.max(min, Math.round(size)))
    persist()
  }

  /** 显隐切换：隐藏→恢复；悬浮→置焦；停靠→隐藏 */
  function toggle(id: PanelId): void {
    const p = panels[id]
    if (p.zone === null) {
      if (p.float && !p.minimized) {
        focusFloat(id)
        return
      }
      show(id)
      return
    }
    hide(id)
  }

  function beginDrag(id: PanelId): void {
    dragging.value = id
  }

  function endDrag(): void {
    dragging.value = null
  }

  function resetLayout(): void {
    Object.assign(panels, defaultLayout())
    Object.assign(sizes, defaultSizes())
    focusZ.clear()
    persist()
  }

  return {
    panels,
    sizes,
    dragging,
    refreshSeq,
    bumpRefresh,
    leftPanel,
    rightPanel,
    bottomPanel,
    centerPanel,
    floatingPanels,
    panelOf,
    move,
    hide,
    show,
    float,
    dockBack,
    focusFloat,
    minimize,
    restore,
    zOf,
    setGeom,
    resizeZone,
    toggle,
    beginDrag,
    endDrag,
    resetLayout,
  }
})

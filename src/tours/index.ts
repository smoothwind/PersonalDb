/**
 * 功能漫游指导（driver.js）。
 *
 * - 锚点：目标元素标 `data-tour="<key>"`，与 steps 中的 selector 对应；
 * - 文案：全部走 i18n（tour.*）；
 * - 入口：菜单「帮助 → 漫游指导」子项 + 首次启动自动引导（localStorage personaldb.tourDone）。
 */
import { driver, type Driver } from 'driver.js'
import 'driver.js/dist/driver.css'
import { i18n } from '../i18n'

export interface TourDef {
  id: string
  /** 菜单显示 label key */
  labelKey: string
  steps: { element: string; popover: { titleKey: string; descKey: string } }[]
}

const t = (key: string, vars?: Record<string, unknown>) => i18n.global.t(key, vars ?? {})

export const TOURS: TourDef[] = [
  {
    id: 'overview',
    labelKey: 'tour.overview',
    steps: [
      {
        element: '[data-tour="tree"]',
        popover: { titleKey: 'tour.treeTitle', descKey: 'tour.treeDesc' },
      },
      {
        element: '[data-tour="new-conn"]',
        popover: { titleKey: 'tour.newConnTitle', descKey: 'tour.newConnDesc' },
      },
      {
        element: '[data-tour="workspaces"]',
        popover: { titleKey: 'tour.wsTitle', descKey: 'tour.wsDesc' },
      },
      {
        element: '[data-tour="tabs"]',
        popover: { titleKey: 'tour.tabsTitle', descKey: 'tour.tabsDesc' },
      },
      {
        element: '[data-tour="settings"]',
        popover: { titleKey: 'tour.settingsTitle', descKey: 'tour.settingsDesc' },
      },
    ],
  },
  {
    id: 'query',
    labelKey: 'tour.query',
    steps: [
      {
        element: '[data-tour="query-toolbar"]',
        popover: { titleKey: 'tour.qToolbarTitle', descKey: 'tour.qToolbarDesc' },
      },
      {
        element: '[data-tour="result"]',
        popover: { titleKey: 'tour.resultTitle', descKey: 'tour.resultDesc' },
      },
    ],
  },
  {
    id: 'session',
    labelKey: 'tour.session',
    steps: [
      {
        element: '[data-tour="session-toolbar"]',
        popover: { titleKey: 'tour.sessTitle', descKey: 'tour.sessDesc' },
      },
    ],
  },
]

const instances = new Set<Driver>()

/** 是否存在 tour 对应的可见锚点（分屏/页签位置不同会导致部分元素隐藏） */
function elementVisible(selector: string): boolean {
  const el = document.querySelector(selector)
  if (!el) return false
  return el.getClientRects().length > 0
}

export function startTour(id: string): void {
  const def = TOURS.find((x) => x.id === id)
  if (!def) return
  const steps = def.steps
    .filter((s) => elementVisible(s.element))
    .map((s) => ({
      element: s.element,
      popover: { title: t(s.popover.titleKey), description: t(s.popover.descKey) },
    }))
  if (steps.length === 0) {
    ElMessageHint()
    return
  }
  const inst = driver({
    steps,
    showProgress: true,
    nextBtnText: t('tour.next'),
    prevBtnText: t('tour.prev'),
    doneBtnText: t('tour.done'),
    onDestroyed: () => instances.delete(inst),
  })
  instances.add(inst)
  inst.drive()
}

function ElMessageHint(): void {
  // 轻提示：避免循环依赖 element-plus 的全局提示在此处反复引入
  import('element-plus').then(({ ElMessage }) => {
    ElMessage.info(t('tour.unavailable'))
  })
}

const TOUR_DONE_KEY = 'personaldb.tourDone'

/** 首次启动自动引导（完成后不再触发） */
export function maybeAutoTour(): void {
  if (localStorage.getItem(TOUR_DONE_KEY)) return
  localStorage.setItem(TOUR_DONE_KEY, '1')
  window.setTimeout(() => startTour('overview'), 900)
}

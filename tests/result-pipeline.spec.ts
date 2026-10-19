import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import { defineComponent, ref } from 'vue'
import type { Ref } from 'vue'
import { useResultPipeline } from '../src/composables/useResultPipeline'
import type { QueryTab } from '../src/stores/tabs'
import type { CellValue } from '../src/types/db'

/** 最小 QueryTab 形状（管线只消费 result） */
function tabOf(columns: string[], rows: CellValue[][]): QueryTab {
  return {
    result: {
      columns: columns.map((name) => ({ name, typeLabel: 'TEXT', semantic: 'text' })),
      rows,
      affectedRows: null,
      durationMs: 0,
      truncated: false,
    },
  } as unknown as QueryTab
}

/** 宿主组件：在真实 setup 上下文里调用 composable（watch/onBeforeUnmount 生效） */
const Host = defineComponent({
  props: { tab: { type: Object, required: true } },
  setup(props) {
    const colSettings: Ref<Record<string, { filter?: string }>> = ref({})
    const searchApplied = ref('')
    const editMode = ref(false)
    const p = useResultPipeline({ tab: () => props.tab as QueryTab, colSettings, searchApplied, editMode })
    return { ...p, colSettings, searchApplied, editMode }
  },
  template: '<div/>',
})

describe('useResultPipeline — prop 索引与矩阵', () => {
  it('重复列名生成 c{count}_{name} prop 并映射到矩阵列下标', () => {
    const w = mount(Host, { props: { tab: tabOf(['id', 'name', 'id'], [[1, 'a', 2]]) } })
    const vm = w.vm as unknown as Record<string, unknown>
    expect(vm.matrix).toEqual([[1, 'a', 2]])
    expect([...(vm.propIndex as Map<string, number>).entries()]).toEqual([
      ['c0_id', 0],
      ['c0_name', 1],
      ['c1_id', 2],
    ])
  })

  it('无结果时矩阵为空数组', () => {
    const w = mount(Host, { props: { tab: { result: null } as unknown as QueryTab } })
    expect((w.vm as unknown as Record<string, unknown>).matrix).toEqual([])
  })
})

describe('useResultPipeline — 过滤（jsdom 无 Worker → 同步路径）', () => {
  it('全局查找过滤 filteredIdx 与物化行 resultRows', async () => {
    const w = mount(Host, {
      props: { tab: tabOf(['name', 'note'], [['Alice', 'x'], ['Bob', 'alice fan'], ['Carol', 'y']]) },
    })
    const vm = w.vm as unknown as Record<string, unknown>
    expect(vm.filteredIdx).toEqual([0, 1, 2])
    vm.searchApplied = 'alice'
    await w.vm.$nextTick()
    // watch 触发同步调度（微任务内）
    await Promise.resolve()
    expect(vm.filteredIdx).toEqual([0, 1])
    expect(vm.resultRows).toEqual([
      { c0_name: 'Alice', c0_note: 'x' },
      { c0_name: 'Bob', c0_note: 'alice fan' },
    ])
  })

  it('值集合筛选（空数组=未启用）与列筛选叠加', async () => {
    const w = mount(Host, {
      props: { tab: tabOf(['name'], [['A'], ['B'], ['C']]) },
    })
    const vm = w.vm as unknown as Record<string, unknown>
    vm.valueFilters = { c0_name: ['A', 'C'] }
    await w.vm.$nextTick()
    await Promise.resolve()
    expect(vm.filteredIdx).toEqual([0, 2])

    vm.colSettings = { c0_name: { filter: 'b' } }
    await w.vm.$nextTick()
    await Promise.resolve()
    expect(vm.filteredIdx).toEqual([])
  })

  it('编辑态强制同步路径（结果切换仍即时生效）', async () => {
    const w = mount(Host, {
      props: { tab: tabOf(['name'], [['A'], ['B']]) },
    })
    const vm = w.vm as unknown as Record<string, unknown>
    vm.editMode = true
    vm.searchApplied = 'b'
    await w.vm.$nextTick()
    await Promise.resolve()
    expect(vm.filteredIdx).toEqual([1])
  })
})

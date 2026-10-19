import { describe, expect, it } from 'vitest'
import { ref } from 'vue'
import { useEditUndo, type EditRow } from '../src/composables/useEditUndo'

function row(name: string, isNew = false): EditRow {
  return { orig: { id: '1' }, values: { name }, isNew, deleted: false }
}

function keydown(key: string, opts: { ctrl?: boolean; shift?: boolean } = {}): KeyboardEvent {
  return new KeyboardEvent('keydown', {
    key,
    ctrlKey: opts.ctrl ?? false,
    shiftKey: opts.shift ?? false,
    cancelable: true,
  })
}

describe('useEditUndo — 快照栈', () => {
  it('push → 修改 → undo 还原快照 → redo 重做', () => {
    const editRows = ref<EditRow[]>([row('a')])
    const u = useEditUndo(editRows, () => true)
    u.pushEditUndo()
    editRows.value = [row('b')]
    u.undoGridEdit()
    expect(editRows.value[0].values.name).toBe('a')
    u.redoGridEdit()
    expect(editRows.value[0].values.name).toBe('b')
  })

  it('快照为拷贝：push 后继续修改原行不影响栈内快照', () => {
    const r = row('a')
    const editRows = ref<EditRow[]>([r])
    const u = useEditUndo(editRows, () => true)
    u.pushEditUndo()
    r.values.name = 'mutated'
    editRows.value[0].values.extra = 'x'
    u.undoGridEdit()
    expect(editRows.value[0].values.name).toBe('a')
    expect(editRows.value[0].values.extra).toBeUndefined()
  })

  it('新 push 清空 redo 栈', () => {
    const editRows = ref<EditRow[]>([row('a')])
    const u = useEditUndo(editRows, () => true)
    u.pushEditUndo()
    editRows.value = [row('b')]
    u.undoGridEdit()
    expect(u.editRedoStack.value.length).toBe(1)
    u.pushEditUndo()
    expect(u.editRedoStack.value.length).toBe(0)
  })

  it('栈深上限 50：最旧快照被淘汰', () => {
    const editRows = ref<EditRow[]>([row('a')])
    const u = useEditUndo(editRows, () => true)
    for (let i = 0; i < 55; i++) u.pushEditUndo()
    expect(u.editUndoStack.value.length).toBe(50)
  })

  it('空栈 undo / redo 为无操作', () => {
    const editRows = ref<EditRow[]>([row('a')])
    const u = useEditUndo(editRows, () => true)
    u.undoGridEdit()
    u.redoGridEdit()
    expect(editRows.value[0].values.name).toBe('a')
    expect(u.editUndoStack.value.length).toBe(0)
  })
})

describe('useEditUndo — 快捷键判定（onEditKeydown）', () => {
  it('hotkeyActive=false 时不响应', () => {
    const editRows = ref<EditRow[]>([row('a')])
    const u = useEditUndo(editRows, () => false)
    u.pushEditUndo()
    editRows.value = [row('b')]
    const e = keydown('z', { ctrl: true })
    u.onEditKeydown(e)
    expect(e.defaultPrevented).toBe(false)
    expect(editRows.value[0].values.name).toBe('b')
  })

  it('Ctrl+Z 撤销 / Ctrl+Y 与 Ctrl+Shift+Z 重做 / 无修饰键不响应', () => {
    const editRows = ref<EditRow[]>([row('a')])
    const u = useEditUndo(editRows, () => true)
    u.pushEditUndo()
    editRows.value = [row('b')]

    const plain = keydown('z')
    u.onEditKeydown(plain)
    expect(plain.defaultPrevented).toBe(false)
    expect(editRows.value[0].values.name).toBe('b')

    u.onEditKeydown(keydown('z', { ctrl: true }))
    expect(editRows.value[0].values.name).toBe('a')

    u.onEditKeydown(keydown('y', { ctrl: true }))
    expect(editRows.value[0].values.name).toBe('b')

    // redo 后 redo 栈已空：再次 redo 为无操作
    u.onEditKeydown(keydown('z', { ctrl: true, shift: true }))
    expect(editRows.value[0].values.name).toBe('b')

    // 撤销后再 Ctrl+Shift+Z 重做
    u.onEditKeydown(keydown('z', { ctrl: true }))
    expect(editRows.value[0].values.name).toBe('a')
    u.onEditKeydown(keydown('z', { ctrl: true, shift: true }))
    expect(editRows.value[0].values.name).toBe('b')
  })
})

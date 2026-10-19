/**
 * 结果集编辑撤销/重做（Ctrl+Z / Ctrl+Y，全量快照栈深 50，见 AGENTS 编辑撤销约定）。
 * 快照为 EditRow 浅拷贝数组（orig/values 各复制一层）。
 */
import { ref } from 'vue'
import type { Ref } from 'vue'
import type { CellValue } from '../types/db'

export interface EditRow {
  orig: Record<string, CellValue>
  values: Record<string, CellValue>
  isNew: boolean
  deleted: boolean
}

const UNDO_LIMIT = 50

export function useEditUndo(editRows: Ref<EditRow[]>, hotkeyActive: () => boolean) {
  const editUndoStack = ref<EditRow[][]>([])
  const editRedoStack = ref<EditRow[][]>([])

  function snapshotEditRows(): EditRow[] {
    return editRows.value.map((r) => ({
      orig: { ...r.orig },
      values: { ...r.values },
      isNew: r.isNew,
      deleted: r.deleted,
    }))
  }

  function pushEditUndo(): void {
    editUndoStack.value.push(snapshotEditRows())
    if (editUndoStack.value.length > UNDO_LIMIT) editUndoStack.value.shift()
    editRedoStack.value = []
  }

  function undoGridEdit(): void {
    const snap = editUndoStack.value.pop()
    if (!snap) return
    editRedoStack.value.push(snapshotEditRows())
    editRows.value = snap
  }

  function redoGridEdit(): void {
    const snap = editRedoStack.value.pop()
    if (!snap) return
    editUndoStack.value.push(snapshotEditRows())
    editRows.value = snap
  }

  function onEditKeydown(e: KeyboardEvent): void {
    if (!hotkeyActive()) return
    if (e.ctrlKey && !e.shiftKey && e.key.toLowerCase() === 'z') {
      e.preventDefault()
      undoGridEdit()
    } else if ((e.ctrlKey && e.key.toLowerCase() === 'y') || (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === 'z')) {
      e.preventDefault()
      redoGridEdit()
    }
  }

  return { editUndoStack, editRedoStack, pushEditUndo, undoGridEdit, redoGridEdit, onEditKeydown }
}

/**
 * 鼠标拖拽手势统一封装：注册 window 级 mousemove/mouseup，
 * mouseup 时自清理并回调 onEnd（避免各处手写 add/removeEventListener 样板）
 */
export function startWindowDrag(
  onMove: (ev: MouseEvent) => void,
  onEnd?: () => void,
): void {
  const move = (ev: MouseEvent): void => onMove(ev)
  const up = (): void => {
    window.removeEventListener('mousemove', move)
    window.removeEventListener('mouseup', up)
    onEnd?.()
  }
  window.addEventListener('mousemove', move)
  window.addEventListener('mouseup', up)
}

/** 连接标识色：id 哈希 → HSL（饱和度/亮度固定，色相均布） */

export function connColor(id: string): string {
  let hash = 5381
  for (let i = 0; i < id.length; i++) {
    hash = ((hash << 5) + hash + id.charCodeAt(i)) | 0
  }
  const hue = Math.abs(hash) % 360
  return `hsl(${hue}, 62%, 52%)`
}

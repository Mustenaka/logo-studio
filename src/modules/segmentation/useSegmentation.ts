import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useCanvasStore } from '../../store/useCanvasStore'
import { useAppStore } from '../../store/useAppStore'

export function useSegmentation() {
  const canvasStore = useCanvasStore()
  const appStore = useAppStore()
  const isRunning = ref(false)

  /**
   * Run segmentation and store result as a **pending mask** for user confirmation.
   * Does NOT apply the mask directly — user must call confirmPendingMask().
   */
  async function runSegmentation(mode: 'auto' | 'point') {
    if (!canvasStore.imageLayer || isRunning.value) return

    isRunning.value = true
    appStore.setLoading(true, mode === 'auto' ? 'AI 智能抠图中，首次运行需要 1-2 分钟...' : 'AI 点选抠图中，首次运行需要 1-2 分钟...')

    try {
      const imageSrc = canvasStore.imageLayer.src
      const points = mode === 'point' ? canvasStore.segPoints : []
      // Map 0-100 slider → 0-150 internal tolerance
      const tolerance = Math.round(canvasStore.segTolerance / 100 * 150)

      const result = await invoke<{ mask: string; success: boolean; error?: string; method?: string }>('segment_image', {
        imageSrc,
        points,
        mode,
        tolerance,
        sam2Threshold: canvasStore.sam2Threshold,
        matteRadius: canvasStore.matteRadius,
      })

      if (result.success && result.mask) {
        canvasStore.setPendingMask(`data:image/png;base64,${result.mask}`)
        canvasStore.lastSegMethod = result.method ?? ''
        if (result.method?.startsWith('flood_fill') || result.method?.startsWith('color')) {
          appStore.showToast(`SAM2 未就绪，已使用降级方案: ${result.method}`, 'warn')
        }
      } else {
        console.error('Segmentation failed:', result.error)
        await fallbackSegment(tolerance)
      }
    } catch (e) {
      console.error('Segmentation invoke error:', e)
      await fallbackSegment(Math.round(canvasStore.segTolerance / 100 * 150))
    } finally {
      isRunning.value = false
      appStore.setLoading(false)
      canvasStore.clearSegPoints()
    }
  }

  /** Frontend fallback — also writes to pending mask */
  async function fallbackSegment(tolerance = 45) {
    if (!canvasStore.imageLayer) return
    try {
      const img = await loadImageEl(canvasStore.imageLayer.src)
      const off = document.createElement('canvas')
      off.width = img.naturalWidth
      off.height = img.naturalHeight
      const ctx = off.getContext('2d')!
      ctx.drawImage(img, 0, 0)
      const imageData = ctx.getImageData(0, 0, off.width, off.height)
      removeBackground(imageData.data, off.width, off.height, tolerance)
      ctx.putImageData(imageData, 0, 0)
      canvasStore.setPendingMask(off.toDataURL('image/png'))
    } catch (e) {
      console.error('Fallback segmentation error:', e)
    }
  }

  return { runSegmentation, isRunning }
}

// ─── helpers ──────────────────────────────────────────────────────────────────

function loadImageEl(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image()
    img.crossOrigin = 'anonymous'
    img.onload = () => resolve(img)
    img.onerror = reject
    img.src = src
  })
}

function removeBackground(data: Uint8ClampedArray, width: number, height: number, threshold = 30) {
  const visited = new Uint8Array(width * height)
  const queue: number[] = []

  function getIdx(x: number, y: number) { return y * width + x }
  function getRGBA(idx: number) {
    const i = idx * 4
    return [data[i], data[i + 1], data[i + 2], data[i + 3]] as const
  }
  function colorDist(r1: number, g1: number, b1: number, r2: number, g2: number, b2: number) {
    return Math.sqrt((r1 - r2) ** 2 + (g1 - g2) ** 2 + (b1 - b2) ** 2)
  }

  const cornerPixels = [
    [0, 0], [width - 1, 0], [0, height - 1], [width - 1, height - 1],
    [Math.floor(width / 2), 0], [Math.floor(width / 2), height - 1],
  ]

  const bgColors: [number, number, number][] = cornerPixels.map(([x, y]) => {
    const [r, g, b] = getRGBA(getIdx(x, y))
    return [r, g, b]
  })
  const avgBg = bgColors.reduce(
    (acc, [r, g, b]) => [acc[0] + r, acc[1] + g, acc[2] + b],
    [0, 0, 0],
  ).map(v => v / bgColors.length) as [number, number, number]

  for (const [x, y] of cornerPixels) {
    const idx = getIdx(x, y)
    if (!visited[idx]) { visited[idx] = 1; queue.push(idx) }
  }

  const [br, bg, bb] = avgBg
  const dirs = [-1, 1, -width, width]

  while (queue.length > 0) {
    const idx = queue.pop()!
    const [r, g, b, a] = getRGBA(idx)
    if (a < 10) continue
    if (colorDist(r, g, b, br, bg, bb) > threshold) continue
    data[idx * 4 + 3] = 0
    for (const d of dirs) {
      const nidx = idx + d
      if (nidx >= 0 && nidx < width * height && !visited[nidx]) {
        visited[nidx] = 1
        queue.push(nidx)
      }
    }
  }
}

import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

export interface ImageLayer {
  id: string
  name: string
  src: string
  x: number
  y: number
  width: number
  height: number
  naturalWidth: number
  naturalHeight: number
  opacity: number
  visible: boolean
  brightness: number  // -100 ~ 100
  contrast: number
  saturation: number
  maskDataUrl?: string
  hasMask: boolean
}

export const useCanvasStore = defineStore('canvas', () => {
  const canvasWidth = ref(800)
  const canvasHeight = ref(800)
  const zoom = ref(1)
  const panX = ref(0)
  const panY = ref(0)

  const imageLayer = ref<ImageLayer | null>(null)
  const activeLayerId = ref<string | null>(null)

  // segmentation state
  const segMode = ref<'auto' | 'point' | 'none'>('none')
  const segPoints = ref<{ x: number; y: number; label: number }[]>([])
  const isSegmenting = ref(false)
  /**
   * 0 = 最保守（只移除与背景极度相似的像素）
   * 100 = 最激进（大范围扩散，移除更多背景）
   * 内部映射到算法 tolerance: slider/100 * 150
   */
  const segTolerance = ref(40)

  // SAM2 debug parameters
  /** SAM2 蒙版阈值：越低保留越多（包含阴影/渐变），越高越干净。范围 0.05-0.95，默认 0.35 */
  const sam2Threshold = ref(0.50)
  /** Alpha Matting 边缘精修范围（像素），范围 1-30，默认 8 */
  const matteRadius = ref(8)
  /** 上次抠图使用的方法（调试用） */
  const lastSegMethod = ref<string>('')

  // ── Pending mask (抠图结果待确认) ─────────────────────────────────────────
  // When segmentation completes, the mask lands here first.
  // User must explicitly confirm or discard before it's applied to the layer.
  const pendingMaskDataUrl = ref<string | null>(null)

  const hasImage = computed(() => imageLayer.value !== null)
  const hasPendingMask = computed(() => pendingMaskDataUrl.value !== null)

  function loadImage(src: string, naturalWidth: number, naturalHeight: number) {
    const id = `img-${Date.now()}`
    const scale = Math.min(canvasWidth.value / naturalWidth, canvasHeight.value / naturalHeight, 1)
    const w = Math.round(naturalWidth * scale)
    const h = Math.round(naturalHeight * scale)
    imageLayer.value = {
      id,
      name: 'Logo Image',
      src,
      x: Math.round((canvasWidth.value - w) / 2),
      y: Math.round((canvasHeight.value - h) / 2),
      width: w,
      height: h,
      naturalWidth,
      naturalHeight,
      opacity: 1,
      visible: true,
      brightness: 0,
      contrast: 0,
      saturation: 0,
      hasMask: false,
    }
    activeLayerId.value = id
    segPoints.value = []
    segMode.value = 'none'
    pendingMaskDataUrl.value = null
  }

  function clearImage() {
    imageLayer.value = null
    activeLayerId.value = null
    segPoints.value = []
    segMode.value = 'none'
    pendingMaskDataUrl.value = null
  }

  // ── Pending mask lifecycle ─────────────────────────────────────────────────

  /** Called by useSegmentation — stores mask for user review */
  function setPendingMask(dataUrl: string) {
    pendingMaskDataUrl.value = dataUrl
  }

  /** User clicks "确认" — apply pending mask to the image layer */
  function confirmPendingMask() {
    if (!imageLayer.value || !pendingMaskDataUrl.value) return
    imageLayer.value.maskDataUrl = pendingMaskDataUrl.value
    imageLayer.value.hasMask = true
    pendingMaskDataUrl.value = null
  }

  /** User clicks "放弃" — discard pending mask */
  function discardPendingMask() {
    pendingMaskDataUrl.value = null
  }

  /** User clicks "反选" — invert the alpha channel of the pending mask */
  function invertPendingMask() {
    if (!pendingMaskDataUrl.value) return
    const img = new Image()
    img.onload = () => {
      const off = document.createElement('canvas')
      off.width = img.width
      off.height = img.height
      const ctx = off.getContext('2d')!
      ctx.drawImage(img, 0, 0)
      const imageData = ctx.getImageData(0, 0, off.width, off.height)
      const data = imageData.data
      for (let i = 3; i < data.length; i += 4) {
        data[i] = 255 - data[i]
      }
      ctx.putImageData(imageData, 0, 0)
      pendingMaskDataUrl.value = off.toDataURL('image/png')
    }
    img.src = pendingMaskDataUrl.value
  }

  // ── Confirmed mask helpers ─────────────────────────────────────────────────

  function setMask(maskDataUrl: string) {
    if (imageLayer.value) {
      imageLayer.value.maskDataUrl = maskDataUrl
      imageLayer.value.hasMask = true
    }
  }

  function clearMask() {
    if (imageLayer.value) {
      imageLayer.value.maskDataUrl = undefined
      imageLayer.value.hasMask = false
    }
    pendingMaskDataUrl.value = null
  }

  // ── Seg points ─────────────────────────────────────────────────────────────

  function addSegPoint(x: number, y: number, label: 1 | 0 = 1) {
    segPoints.value.push({ x, y, label })
  }

  function clearSegPoints() {
    segPoints.value = []
  }

  // ── Viewport ───────────────────────────────────────────────────────────────

  function setZoom(z: number) {
    zoom.value = Math.max(0.1, Math.min(4, z))
  }

  function setPan(x: number, y: number) {
    panX.value = x
    panY.value = y
  }

  function resetView() {
    zoom.value = 1
    panX.value = 0
    panY.value = 0
  }

  return {
    canvasWidth,
    canvasHeight,
    zoom,
    panX,
    panY,
    imageLayer,
    activeLayerId,
    segMode,
    segPoints,
    isSegmenting,
    segTolerance,
    sam2Threshold,
    matteRadius,
    lastSegMethod,
    pendingMaskDataUrl,
    hasImage,
    hasPendingMask,
    loadImage,
    clearImage,
    setPendingMask,
    confirmPendingMask,
    discardPendingMask,
    invertPendingMask,
    setMask,
    clearMask,
    addSegPoint,
    clearSegPoints,
    setZoom,
    setPan,
    resetView,
  }
})

import { ref } from 'vue'
import { useCanvasStore } from '../../store/useCanvasStore'
import { useBackgroundStore } from '../../store/useBackgroundStore'
import { useTypographyStore } from '../../store/useTypographyStore'
import type { ImageLayer } from '../../store/useCanvasStore'

// ─── Image Import ─────────────────────────────────────────────────────────────

export function useImageEditor() {
  const canvasStore = useCanvasStore()

  async function importImageFromFile(file?: File) {
    let targetFile = file
    if (!targetFile) {
      // Use native file picker via Tauri dialog or fallback HTML input
      targetFile = await pickFileViaInput()
    }
    if (!targetFile) return

    const dataUrl = await readFileAsDataUrl(targetFile)
    const { width, height } = await getImageDimensions(dataUrl)
    canvasStore.loadImage(dataUrl, width, height)
  }

  async function importImageFromDataUrl(dataUrl: string) {
    const { width, height } = await getImageDimensions(dataUrl)
    canvasStore.loadImage(dataUrl, width, height)
  }

  return { importImageFromFile, importImageFromDataUrl }
}

function pickFileViaInput(): Promise<File | undefined> {
  return new Promise((resolve) => {
    const input = document.createElement('input')
    input.type = 'file'
    input.accept = 'image/png,image/jpeg,image/webp,image/svg+xml'
    input.onchange = () => {
      resolve(input.files?.[0])
    }
    input.oncancel = () => resolve(undefined)
    input.click()
  })
}

function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(reader.result as string)
    reader.onerror = reject
    reader.readAsDataURL(file)
  })
}

function getImageDimensions(src: string): Promise<{ width: number; height: number }> {
  return new Promise((resolve, reject) => {
    const img = new Image()
    img.onload = () => resolve({ width: img.naturalWidth, height: img.naturalHeight })
    img.onerror = reject
    img.src = src
  })
}

// ─── Canvas Renderer ──────────────────────────────────────────────────────────

export const canvasEl = ref<HTMLCanvasElement | null>(null)
const imageCache = new Map<string, HTMLImageElement>()

async function loadImg(src: string): Promise<HTMLImageElement> {
  if (imageCache.has(src)) return imageCache.get(src)!
  return new Promise((resolve, reject) => {
    const img = new Image()
    img.onload = () => {
      imageCache.set(src, img)
      resolve(img)
    }
    img.onerror = reject
    img.src = src
  })
}

/**
 * Apply CSS filter string from image adjustments
 */
function buildFilter(layer: ImageLayer): string {
  const parts: string[] = []
  if (layer.brightness !== 0) parts.push(`brightness(${1 + layer.brightness / 100})`)
  if (layer.contrast !== 0) parts.push(`contrast(${1 + layer.contrast / 100})`)
  if (layer.saturation !== 0) parts.push(`saturate(${1 + layer.saturation / 100})`)
  return parts.join(' ') || 'none'
}

/**
 * Build gradient for canvas (matching CSS gradient store)
 */
function buildCanvasGradient(
  ctx: CanvasRenderingContext2D,
  bgStore: ReturnType<typeof useBackgroundStore>,
  x: number, y: number, w: number, h: number
): CanvasGradient | string {
  const { bgType, stops, angle } = bgStore
  if (bgType === 'solid') return bgStore.solidColor
  if (bgType === 'none') return 'transparent'

  const sorted = [...stops].sort((a, b) => a.position - b.position)

  let grad: CanvasGradient
  if (bgType === 'radial') {
    grad = ctx.createRadialGradient(x + w / 2, y + h / 2, 0, x + w / 2, y + h / 2, Math.max(w, h) / 2)
  } else {
    // linear gradient with angle
    const rad = (angle - 90) * (Math.PI / 180)
    const cos = Math.cos(rad)
    const sin = Math.sin(rad)
    const cx = x + w / 2, cy = y + h / 2
    const len = Math.sqrt(w * w + h * h) / 2
    grad = ctx.createLinearGradient(cx - cos * len, cy - sin * len, cx + cos * len, cy + sin * len)
  }

  sorted.forEach(s => grad.addColorStop(s.position / 100, s.color))
  return grad
}

/**
 * Main canvas render function — called reactively from CenterCanvas.vue
 */
export function useCanvasRenderer() {
  function render(
    canvas: HTMLCanvasElement,
    canvasStore: ReturnType<typeof useCanvasStore>,
    bgStore: ReturnType<typeof useBackgroundStore>,
    typoStore: ReturnType<typeof useTypographyStore>,
  ) {
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const W = canvas.width
    const H = canvas.height

    ctx.clearRect(0, 0, W, H)

    // ── 1. Background layer ──────────────────────────────────────────────────
    const { borderRadius, shadowEnabled, shadowBlur, shadowColor, shadowOffsetX, shadowOffsetY } = bgStore

    const drawBg = () => {
      const r = Math.min(borderRadius, W / 2, H / 2)

      // Shadow
      if (shadowEnabled) {
        ctx.save()
        ctx.shadowColor = shadowColor
        ctx.shadowBlur = shadowBlur
        ctx.shadowOffsetX = shadowOffsetX
        ctx.shadowOffsetY = shadowOffsetY
      }

      // Rounded rect path
      ctx.beginPath()
      ctx.roundRect(0, 0, W, H, r)
      ctx.closePath()

      if (bgStore.bgType === 'none') {
        // Transparent: no fill
      } else {
        const fill = buildCanvasGradient(ctx, bgStore, 0, 0, W, H)
        if (typeof fill === 'string') {
          ctx.fillStyle = fill
        } else {
          ctx.fillStyle = fill
        }
        ctx.fill()
      }

      if (shadowEnabled) ctx.restore()

      // Inner glow
      if (bgStore.innerGlow && bgStore.bgType !== 'none') {
        ctx.save()
        ctx.beginPath()
        ctx.roundRect(0, 0, W, H, r)
        ctx.closePath()
        const ig = ctx.createRadialGradient(W / 2, H / 2, Math.min(W, H) * 0.2, W / 2, H / 2, Math.max(W, H) * 0.7)
        ig.addColorStop(0, bgStore.innerGlowColor)
        ig.addColorStop(1, 'transparent')
        ctx.fillStyle = ig
        ctx.fill()
        ctx.restore()
      }
    }

    drawBg()

    // ── 2. Image layer ────────────────────────────────────────────────────────
    const imgLayer = canvasStore.imageLayer
    if (imgLayer && imgLayer.visible) {
      // Determine which mask to use for preview:
      // pendingMaskDataUrl takes priority (user is reviewing), then confirmed mask
      const activeMask = canvasStore.pendingMaskDataUrl ?? (imgLayer.hasMask ? imgLayer.maskDataUrl : undefined)
      const isPending = !!canvasStore.pendingMaskDataUrl

      ;(async () => {
        try {
          const img = await loadImg(imgLayer.src)
          const { x, y, width: w, height: h } = imgLayer

          ctx.save()
          ctx.globalAlpha = imgLayer.opacity
          const filterStr = buildFilter(imgLayer)
          if (filterStr !== 'none') ctx.filter = filterStr

          if (activeMask && !isPending) {
            // ── Confirmed mask: composite on offscreen to preserve background ─
            // NOTE: destination-in on the main canvas would erase the background
            // gradient. Instead, we blend on a temp canvas then composite onto main.
            const maskImg = await loadImg(activeMask)
            const off = document.createElement('canvas')
            off.width = W
            off.height = H
            const oc = off.getContext('2d')!
            oc.drawImage(img, x, y, w, h)
            oc.globalCompositeOperation = 'destination-in'
            oc.drawImage(maskImg, x, y, w, h)
            ctx.drawImage(off, 0, 0)
          } else if (activeMask && isPending) {
            // ── Pending preview: stylized glass overlay ──────────────────────
            // 1. Draw original image at full opacity (so user can see the full context)
            ctx.drawImage(img, x, y, w, h)
            ctx.restore()

            // 2. Build "removed area" overlay on an offscreen canvas
            const maskImg = await loadImg(activeMask)
            const off = document.createElement('canvas')
            off.width = W
            off.height = H
            const oc = off.getContext('2d')!

            // Fill the image bounds with the "removed" tint color
            oc.fillStyle = 'rgba(109, 40, 217, 0.55)'   // deep violet base
            oc.fillRect(x, y, w, h)

            // Add a subtle noise/grain texture via gradient
            const noisGrad = oc.createLinearGradient(x, y, x + w, y + h)
            noisGrad.addColorStop(0, 'rgba(139, 92, 246, 0.12)')
            noisGrad.addColorStop(0.5, 'rgba(59, 130, 246, 0.08)')
            noisGrad.addColorStop(1, 'rgba(139, 92, 246, 0.12)')
            oc.fillStyle = noisGrad
            oc.fillRect(x, y, w, h)

            // Punch out "kept" region using mask (destination-out removes where mask has alpha)
            oc.globalCompositeOperation = 'destination-out'
            oc.drawImage(maskImg, x, y, w, h)

            // Draw the overlay onto main canvas
            ctx.save()
            ctx.drawImage(off, 0, 0)
            ctx.restore()

            // 3. Edge highlight — soft glow along the mask boundary
            const edgeOff = document.createElement('canvas')
            edgeOff.width = W
            edgeOff.height = H
            const ec = edgeOff.getContext('2d')!
            ec.drawImage(maskImg, x, y, w, h)
            ec.globalCompositeOperation = 'source-in'
            const edgeGrad = ec.createRadialGradient(x + w / 2, y + h / 2, Math.min(w, h) * 0.3, x + w / 2, y + h / 2, Math.max(w, h) * 0.6)
            edgeGrad.addColorStop(0, 'rgba(255,255,255,0)')
            edgeGrad.addColorStop(0.85, 'rgba(255,255,255,0)')
            edgeGrad.addColorStop(1, 'rgba(99,102,241,0.7)')
            ec.fillStyle = edgeGrad
            ec.fillRect(x, y, w, h)
            ctx.save()
            ctx.drawImage(edgeOff, 0, 0)
            ctx.restore()

            // 4. Scanline pattern on removed area (subtle tech feel)
            ctx.save()
            const scanOff = document.createElement('canvas')
            scanOff.width = w
            scanOff.height = h
            const sc = scanOff.getContext('2d')!
            for (let sy = 0; sy < h; sy += 4) {
              sc.fillStyle = 'rgba(0,0,0,0.07)'
              sc.fillRect(0, sy, w, 1)
            }
            // Clip to removed area using mask (destination-in approach)
            sc.globalCompositeOperation = 'destination-out'
            sc.drawImage(maskImg, 0, 0, w, h)
            // We need the scanlines only where mask is transparent (removed area)
            // Flip: draw scanlines over the full area, remove where mask is opaque
            const scanFinal = document.createElement('canvas')
            scanFinal.width = w
            scanFinal.height = h
            const sf = scanFinal.getContext('2d')!
            for (let sy = 0; sy < h; sy += 4) {
              sf.fillStyle = 'rgba(0,0,0,0.07)'
              sf.fillRect(0, sy, w, 1)
            }
            sf.globalCompositeOperation = 'destination-out'
            sf.drawImage(maskImg, 0, 0, w, h)
            ctx.drawImage(scanFinal, x, y)
            ctx.restore()

            // 5. "Kept" region — draw the masked image cleanly on top
            ctx.save()
            ctx.globalAlpha = imgLayer.opacity
            const keptOff = document.createElement('canvas')
            keptOff.width = W
            keptOff.height = H
            const kc = keptOff.getContext('2d')!
            kc.drawImage(img, x, y, w, h)
            kc.globalCompositeOperation = 'destination-in'
            kc.drawImage(maskImg, x, y, w, h)
            ctx.drawImage(keptOff, 0, 0)
            ctx.restore()

            ctx.save()  // placeholder save for the ctx.restore() below
          } else {
            ctx.drawImage(img, x, y, w, h)
          }

          ctx.restore()

          renderTextLayers(ctx, typoStore)
          renderSegPoints(ctx, canvasStore, imgLayer)
        } catch (e) {
          console.error('Image render error:', e)
        }
      })()
    } else {
      renderTextLayers(ctx, typoStore)
    }
  }

  return { render, canvasEl }
}

function renderTextLayers(
  ctx: CanvasRenderingContext2D,
  typoStore: ReturnType<typeof useTypographyStore>,
) {
  for (const layer of typoStore.textLayers) {
    if (!layer.visible) continue
    ctx.save()
    ctx.globalAlpha = layer.opacity

    if (layer.shadow) {
      ctx.shadowColor = layer.shadowColor
      ctx.shadowBlur = layer.shadowBlur
      ctx.shadowOffsetX = layer.shadowOffsetX
      ctx.shadowOffsetY = layer.shadowOffsetY
    }

    ctx.font = `${layer.fontWeight} ${layer.fontSize}px "${layer.fontFamily}", Inter, sans-serif`
    ctx.fillStyle = layer.color
    ctx.textAlign = layer.align as CanvasTextAlign
    ctx.textBaseline = 'middle'

    // Letter spacing: draw char by char
    if (layer.letterSpacing !== 0) {
      drawTextWithSpacing(ctx, layer.text, layer.x, layer.y, layer.letterSpacing, layer.align)
    } else {
      ctx.fillText(layer.text, layer.x, layer.y)
    }

    ctx.restore()
  }
}

function drawTextWithSpacing(
  ctx: CanvasRenderingContext2D,
  text: string,
  x: number,
  y: number,
  spacing: number,
  align: string,
) {
  const chars = [...text]
  const widths = chars.map(c => ctx.measureText(c).width + spacing)
  const totalWidth = widths.reduce((a, b) => a + b, 0) - spacing

  let startX = x
  if (align === 'center') startX = x - totalWidth / 2
  else if (align === 'right') startX = x - totalWidth

  let curX = startX
  ctx.textAlign = 'left'
  for (let i = 0; i < chars.length; i++) {
    ctx.fillText(chars[i], curX, y)
    curX += widths[i]
  }
}

function renderSegPoints(
  ctx: CanvasRenderingContext2D,
  canvasStore: ReturnType<typeof useCanvasStore>,
  imgLayer: ImageLayer,
) {
  if (canvasStore.segMode !== 'point' || canvasStore.segPoints.length === 0) return

  for (const pt of canvasStore.segPoints) {
    // Convert from image-relative (1024 scale) back to canvas coords
    const cx = imgLayer.x + (pt.x / 1024) * imgLayer.width
    const cy = imgLayer.y + (pt.y / 1024) * imgLayer.height

    ctx.save()
    ctx.beginPath()
    ctx.arc(cx, cy, 6, 0, Math.PI * 2)
    ctx.fillStyle = pt.label === 1 ? '#22c55e' : '#ef4444'
    ctx.fill()
    ctx.strokeStyle = '#fff'
    ctx.lineWidth = 2
    ctx.stroke()
    ctx.restore()
  }
}

import { ref, watchEffect } from 'vue'
import { useCanvasStore, type ImageLayer } from '../../store/useCanvasStore'
import { useTypographyStore, type TextLayer } from '../../store/useTypographyStore'

const THUMB_SIZE = 56

/** Generates and keeps reactive thumbnail data URLs for all layers */
export function useThumbnail() {
  const canvasStore = useCanvasStore()
  const typoStore = useTypographyStore()

  const imageThumbnail = ref<string>('')
  // Map: layerId → data URL
  const textThumbnails = ref<Record<string, string>>({})

  // ── Image layer thumbnail ────────────────────────────────────────────────
  watchEffect(async () => {
    const layer = canvasStore.imageLayer
    if (!layer) {
      imageThumbnail.value = ''
      return
    }
    // Access mask fields so watchEffect re-runs after confirm/discard
    void (layer.hasMask, layer.maskDataUrl)
    imageThumbnail.value = await renderImageThumb(layer)
  })

  // ── Text layer thumbnails ────────────────────────────────────────────────
  watchEffect(() => {
    const result: Record<string, string> = {}
    for (const layer of typoStore.textLayers) {
      result[layer.id] = renderTextThumb(layer)
    }
    textThumbnails.value = result
  })

  return { imageThumbnail, textThumbnails }
}

// ─── Thumbnail generators ─────────────────────────────────────────────────────

async function renderImageThumb(layer: ImageLayer): Promise<string> {
  try {
    const off = document.createElement('canvas')
    off.width = THUMB_SIZE
    off.height = THUMB_SIZE
    const ctx = off.getContext('2d')!

    // Checkerboard background
    drawChecker(ctx, THUMB_SIZE, THUMB_SIZE)

    // Load image
    const img = await loadImgEl(layer.src)

    // Scale to fit with padding
    const pad = 2
    const scale = Math.min((THUMB_SIZE - pad * 2) / img.naturalWidth, (THUMB_SIZE - pad * 2) / img.naturalHeight)
    const w = img.naturalWidth * scale
    const h = img.naturalHeight * scale
    const ox = (THUMB_SIZE - w) / 2
    const oy = (THUMB_SIZE - h) / 2

    if (layer.hasMask && layer.maskDataUrl) {
      // Draw image + apply mask
      const offMask = document.createElement('canvas')
      offMask.width = THUMB_SIZE
      offMask.height = THUMB_SIZE
      const mctx = offMask.getContext('2d')!
      mctx.drawImage(img, ox, oy, w, h)
      const maskImg = await loadImgEl(layer.maskDataUrl)
      mctx.globalCompositeOperation = 'destination-in'
      mctx.drawImage(maskImg, ox, oy, w, h)
      ctx.drawImage(offMask, 0, 0)
    } else {
      ctx.drawImage(img, ox, oy, w, h)
    }

    return off.toDataURL('image/png')
  } catch {
    return ''
  }
}

function renderTextThumb(layer: TextLayer): string {
  const off = document.createElement('canvas')
  off.width = THUMB_SIZE
  off.height = THUMB_SIZE
  const ctx = off.getContext('2d')!

  // Subtle dark background
  ctx.fillStyle = 'rgba(0,0,0,0.35)'
  ctx.fillRect(0, 0, THUMB_SIZE, THUMB_SIZE)

  // Draw the first character(s) of the text
  const preview = layer.text.trim().slice(0, 2) || 'T'
  const fontSize = Math.min(28, THUMB_SIZE * 0.5)
  ctx.font = `${layer.fontWeight} ${fontSize}px "${layer.fontFamily}", Inter, sans-serif`
  ctx.fillStyle = layer.color || '#ffffff'
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  ctx.fillText(preview, THUMB_SIZE / 2, THUMB_SIZE / 2)

  return off.toDataURL('image/png')
}

function drawChecker(ctx: CanvasRenderingContext2D, w: number, h: number) {
  const size = 6
  for (let y = 0; y < h; y += size) {
    for (let x = 0; x < w; x += size) {
      ctx.fillStyle = (x / size + y / size) % 2 === 0 ? '#c0c0c0' : '#888'
      ctx.fillRect(x, y, size, size)
    }
  }
}

function loadImgEl(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image()
    img.onload = () => resolve(img)
    img.onerror = reject
    img.src = src
  })
}

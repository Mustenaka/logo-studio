<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { useCanvasStore } from '../../store/useCanvasStore'
import { useBackgroundStore } from '../../store/useBackgroundStore'
import { useTypographyStore } from '../../store/useTypographyStore'
import { useCanvasRenderer } from '../../modules/image-editor/useImageEditor'
import { useSegmentation } from '../../modules/segmentation/useSegmentation'

const canvasStore = useCanvasStore()
const bgStore = useBackgroundStore()
const typoStore = useTypographyStore()
const { runSegmentation, isRunning: isSegRunning } = useSegmentation()
const { t } = useI18n()

const canvasRef = ref<HTMLCanvasElement | null>(null)
const wrapperRef = ref<HTMLDivElement | null>(null)
const { render, canvasEl } = useCanvasRenderer()

// ── UI state ──────────────────────────────────────────────────────────────────
const showQuickBar = ref(true)
const showAdjust = ref(false)
const canvasCursor = ref('default')

watch(() => canvasStore.imageLayer?.id, () => {
  showQuickBar.value = true
  showAdjust.value = false
})

// ── Fine-tune brush ────────────────────────────────────────────────────────────
const viewportRef = ref<HTMLDivElement | null>(null)
const isFinetuning = ref(false)
const brushSize = ref(28)           // diameter in canvas-space pixels
const brushMode = ref<'add' | 'erase'>('erase')
const brushCursorX = ref(0)
const brushCursorY = ref(0)
const showBrushCursor = ref(false)
const brushDisplayRadius = computed(() => brushSize.value / 2 * canvasStore.zoom)

let maskCanvas: HTMLCanvasElement | null = null
let maskCtx: CanvasRenderingContext2D | null = null
let isPainting = false
let lastMaskX = 0
let lastMaskY = 0
let rafPending = false

function enterFinetune() {
  if (!canvasStore.pendingMaskDataUrl) return
  const img = new Image()
  img.onload = () => {
    maskCanvas = document.createElement('canvas')
    maskCanvas.width = img.width
    maskCanvas.height = img.height
    maskCtx = maskCanvas.getContext('2d')!
    maskCtx.drawImage(img, 0, 0)
    isFinetuning.value = true
  }
  img.src = canvasStore.pendingMaskDataUrl
}

function exitFinetune() {
  isFinetuning.value = false
  isPainting = false
  showBrushCursor.value = false
  maskCanvas = null
  maskCtx = null
}

function syncMaskToStore() {
  if (!maskCanvas || rafPending) return
  rafPending = true
  requestAnimationFrame(() => {
    rafPending = false
    if (maskCanvas) canvasStore.setPendingMask(maskCanvas.toDataURL('image/png'))
  })
}

function getMaskPos(e: MouseEvent): { mx: number; my: number; mr: number } | null {
  const layer = canvasStore.imageLayer
  if (!layer || !maskCanvas || !canvasRef.value) return null
  const rect = canvasRef.value.getBoundingClientRect()
  const cx = (e.clientX - rect.left) * (canvasStore.canvasWidth / rect.width)
  const cy = (e.clientY - rect.top) * (canvasStore.canvasHeight / rect.height)
  const mx = (cx - layer.x) / layer.width * maskCanvas.width
  const my = (cy - layer.y) / layer.height * maskCanvas.height
  const mr = Math.max(1, (brushSize.value / 2) * (maskCanvas.width / layer.width))
  return { mx, my, mr }
}

function doPaint(mx: number, my: number, mr: number) {
  if (!maskCtx) return
  if (brushMode.value === 'erase') {
    maskCtx.globalCompositeOperation = 'destination-out'
    maskCtx.strokeStyle = 'rgba(0,0,0,1)'
    maskCtx.fillStyle = 'rgba(0,0,0,1)'
  } else {
    maskCtx.globalCompositeOperation = 'source-over'
    maskCtx.strokeStyle = 'rgba(255,255,255,1)'
    maskCtx.fillStyle = 'rgba(255,255,255,1)'
  }
  maskCtx.lineWidth = mr * 2
  maskCtx.lineCap = 'round'
  maskCtx.lineJoin = 'round'
  maskCtx.beginPath()
  maskCtx.moveTo(lastMaskX, lastMaskY)
  maskCtx.lineTo(mx, my)
  maskCtx.stroke()
  maskCtx.beginPath()
  maskCtx.arc(mx, my, mr, 0, Math.PI * 2)
  maskCtx.fill()
  maskCtx.globalCompositeOperation = 'source-over'
}

watch(() => canvasStore.hasPendingMask, (val) => {
  if (!val) exitFinetune()
})

// ── Image scale ───────────────────────────────────────────────────────────────
const imageScalePct = computed(() => {
  const img = canvasStore.imageLayer
  if (!img) return 100
  const nw = img.naturalWidth || img.width
  return Math.round(img.width / nw * 100)
})

function adjustImageScale(delta: number) {
  const img = canvasStore.imageLayer
  if (!img) return
  const nw = img.naturalWidth || img.width
  const nh = img.naturalHeight || img.height
  const newPct = Math.max(10, Math.min(300, imageScalePct.value + delta))
  const cx = img.x + img.width / 2
  const cy = img.y + img.height / 2
  const newW = Math.round(nw * newPct / 100)
  const newH = Math.round(nh * newPct / 100)
  img.x = Math.round(cx - newW / 2)
  img.y = Math.round(cy - newH / 2)
  img.width = newW
  img.height = newH
}

function resetImagePosition() {
  const img = canvasStore.imageLayer
  if (!img) return
  img.x = Math.round((canvasStore.canvasWidth - img.width) / 2)
  img.y = Math.round((canvasStore.canvasHeight - img.height) / 2)
}

// ── Zoom (viewport) ───────────────────────────────────────────────────────────
function onWheel(e: WheelEvent) {
  e.preventDefault()
  const delta = e.deltaY > 0 ? 0.9 : 1.1
  canvasStore.setZoom(canvasStore.zoom * delta)
}

// ── Pan (middle-click / alt+drag) ─────────────────────────────────────────────
let isPanning = false
let lastPanX = 0, lastPanY = 0

// ── Image drag ────────────────────────────────────────────────────────────────
let isDraggingImage = false
let dragImgLastX = 0, dragImgLastY = 0

function getCanvasXY(e: MouseEvent) {
  const rect = canvasRef.value!.getBoundingClientRect()
  return {
    cx: (e.clientX - rect.left) * (canvasStore.canvasWidth / rect.width),
    cy: (e.clientY - rect.top) * (canvasStore.canvasHeight / rect.height),
  }
}

function isOverImage(cx: number, cy: number) {
  const img = canvasStore.imageLayer
  if (!img) return false
  return cx >= img.x && cx <= img.x + img.width && cy >= img.y && cy <= img.y + img.height
}

function onMouseDown(e: MouseEvent) {
  if (isFinetuning.value && e.button === 0) {
    isPainting = true
    const pos = getMaskPos(e)
    if (pos) {
      lastMaskX = pos.mx
      lastMaskY = pos.my
      doPaint(pos.mx, pos.my, pos.mr)
      syncMaskToStore()
    }
    e.preventDefault()
    return
  }
  if (e.button === 1 || (e.button === 0 && e.altKey)) {
    isPanning = true
    lastPanX = e.clientX
    lastPanY = e.clientY
    e.preventDefault()
  } else if (e.button === 0 && canvasStore.segMode === 'none' && canvasStore.imageLayer) {
    const { cx, cy } = getCanvasXY(e)
    if (isOverImage(cx, cy)) {
      isDraggingImage = true
      dragImgLastX = e.clientX
      dragImgLastY = e.clientY
      canvasCursor.value = 'grabbing'
      e.preventDefault()
    }
  }
}

function onMouseMove(e: MouseEvent) {
  if (isFinetuning.value) {
    if (viewportRef.value) {
      const rect = viewportRef.value.getBoundingClientRect()
      brushCursorX.value = e.clientX - rect.left
      brushCursorY.value = e.clientY - rect.top
    }
    if (isPainting) {
      const pos = getMaskPos(e)
      if (pos) {
        doPaint(pos.mx, pos.my, pos.mr)
        lastMaskX = pos.mx
        lastMaskY = pos.my
        syncMaskToStore()
      }
    }
    return
  }
  if (isPanning) {
    canvasStore.setPan(canvasStore.panX + e.clientX - lastPanX, canvasStore.panY + e.clientY - lastPanY)
    lastPanX = e.clientX
    lastPanY = e.clientY
  } else if (isDraggingImage && canvasStore.imageLayer) {
    const rect = canvasRef.value!.getBoundingClientRect()
    const sx = canvasStore.canvasWidth / rect.width
    const sy = canvasStore.canvasHeight / rect.height
    canvasStore.imageLayer.x += (e.clientX - dragImgLastX) * sx
    canvasStore.imageLayer.y += (e.clientY - dragImgLastY) * sy
    dragImgLastX = e.clientX
    dragImgLastY = e.clientY
  }
}

function onMouseUp() {
  if (isFinetuning.value && isPainting) {
    isPainting = false
    if (maskCanvas) canvasStore.setPendingMask(maskCanvas.toDataURL('image/png'))
    return
  }
  isPanning = false
  isDraggingImage = false
}

function onCanvasHover(e: MouseEvent) {
  if (isFinetuning.value) {
    canvasCursor.value = 'none'
    showBrushCursor.value = true
    if (viewportRef.value) {
      const rect = viewportRef.value.getBoundingClientRect()
      brushCursorX.value = e.clientX - rect.left
      brushCursorY.value = e.clientY - rect.top
    }
    return
  }
  if (canvasStore.segMode === 'point') { canvasCursor.value = 'crosshair'; return }
  if (isDraggingImage) { canvasCursor.value = 'grabbing'; return }
  const { cx, cy } = getCanvasXY(e)
  canvasCursor.value = canvasStore.imageLayer && isOverImage(cx, cy) ? 'grab' : 'default'
}

// ── Click → seg point ─────────────────────────────────────────────────────────
function onCanvasClick(e: MouseEvent) {
  if (isFinetuning.value) return
  if (canvasStore.segMode !== 'point' || !canvasStore.imageLayer) return
  const rect = canvasRef.value!.getBoundingClientRect()
  const scaleX = canvasStore.canvasWidth / rect.width
  const scaleY = canvasStore.canvasHeight / rect.height
  const x = (e.clientX - rect.left) * scaleX
  const y = (e.clientY - rect.top) * scaleY
  const img = canvasStore.imageLayer
  const ix = Math.round((x - img.x) / img.width * 1024)
  const iy = Math.round((y - img.y) / img.height * 1024)
  canvasStore.addSegPoint(ix, iy, e.shiftKey ? 0 : 1)
}

// ── Re-render trigger ─────────────────────────────────────────────────────────
function triggerRender() {
  nextTick(() => {
    if (canvasRef.value) render(canvasRef.value, canvasStore, bgStore, typoStore)
  })
}

watch(
  () => [
    canvasStore.imageLayer,
    canvasStore.pendingMaskDataUrl,
    canvasStore.segPoints,
    bgStore.cssGradient,
    bgStore.borderRadius,
    bgStore.shadowEnabled,
    bgStore.shadowBlur,
    bgStore.shadowOffsetY,
    bgStore.innerGlow,
    typoStore.textLayers.map(l => ({ ...l })),
  ],
  triggerRender,
  { deep: true },
)

onMounted(() => {
  if (canvasRef.value) {
    canvasEl.value = canvasRef.value
    render(canvasRef.value, canvasStore, bgStore, typoStore)
  }
  window.addEventListener('mouseup', onMouseUp)
  window.addEventListener('mousemove', onMouseMove)
})
onUnmounted(() => {
  window.removeEventListener('mouseup', onMouseUp)
  window.removeEventListener('mousemove', onMouseMove)
})
</script>

<template>
  <main class="canvas-area" ref="wrapperRef">
    <!-- Toolbar -->
    <div class="canvas-toolbar">
      <div class="toolbar-row">
        <!-- View controls -->
        <div class="toolbar-group">
          <button class="btn-ghost toolbar-btn" @click="canvasStore.resetView()">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z"/>
            </svg>
            {{ t('centerCanvas.toolbar.resetView') }}
          </button>
          <button class="btn-ghost toolbar-btn icon-only" :title="t('centerCanvas.toolbar.zoomIn')" @click="canvasStore.setZoom(canvasStore.zoom * 1.2)">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
              <line x1="11" y1="8" x2="11" y2="14"/><line x1="8" y1="11" x2="14" y2="11"/>
            </svg>
          </button>
          <button class="btn-ghost toolbar-btn icon-only" :title="t('centerCanvas.toolbar.zoomOut')" @click="canvasStore.setZoom(canvasStore.zoom * 0.8)">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
              <line x1="8" y1="11" x2="14" y2="11"/>
            </svg>
          </button>
          <span class="zoom-label">{{ Math.round(canvasStore.zoom * 100) }}%</span>
        </div>

        <!-- Image controls (only when image loaded) -->
        <template v-if="canvasStore.hasImage">
          <div class="toolbar-divider" />
          <div class="toolbar-group">
            <span class="toolbar-section-label">{{ t('centerCanvas.toolbar.image') }}</span>
            <!-- Scale controls -->
            <button class="btn-ghost toolbar-btn icon-only" :title="t('centerCanvas.toolbar.scaleDownImage')" @click="adjustImageScale(-10)">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                <line x1="5" y1="12" x2="19" y2="12"/>
              </svg>
            </button>
            <span class="scale-display" :title="t('centerCanvas.toolbar.currentImageScale')">{{ imageScalePct }}%</span>
            <button class="btn-ghost toolbar-btn icon-only" :title="t('centerCanvas.toolbar.scaleUpImage')" @click="adjustImageScale(10)">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
              </svg>
            </button>
            <button class="btn-ghost toolbar-btn" :title="t('centerCanvas.toolbar.centerImage')" @click="resetImagePosition()">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="3" y="3" width="18" height="18" rx="2"/>
                <line x1="12" y1="3" x2="12" y2="21"/><line x1="3" y1="12" x2="21" y2="12"/>
              </svg>
              {{ t('centerCanvas.toolbar.center') }}
            </button>
            <!-- Adjustments toggle -->
            <button
              class="btn-ghost toolbar-btn"
              :class="{ 'toolbar-btn--active': showAdjust }"
              @click="showAdjust = !showAdjust"
              :title="t('centerCanvas.toolbar.imageAdjustments')"
            >
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="4" y1="21" x2="4" y2="14"/><line x1="4" y1="10" x2="4" y2="3"/>
                <line x1="12" y1="21" x2="12" y2="12"/><line x1="12" y1="8" x2="12" y2="3"/>
                <line x1="20" y1="21" x2="20" y2="16"/><line x1="20" y1="12" x2="20" y2="3"/>
                <line x1="1" y1="14" x2="7" y2="14"/><line x1="9" y1="8" x2="15" y2="8"/>
                <line x1="17" y1="16" x2="23" y2="16"/>
              </svg>
              {{ t('centerCanvas.toolbar.adjustments') }}
              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"
                :style="{ transform: showAdjust ? 'rotate(180deg)' : '', transition: 'transform 0.2s' }">
                <polyline points="6 9 12 15 18 9"/>
              </svg>
            </button>
          </div>
          <div class="toolbar-divider" />
          <!-- Seg toggle -->
          <div class="toolbar-group">
            <button
              class="btn-ghost toolbar-btn"
              :class="{ 'toolbar-btn--active': showQuickBar }"
              :disabled="canvasStore.hasPendingMask"
              @click="showQuickBar = !showQuickBar"
              :title="t('centerCanvas.toolbar.segmentationTools')"
            >
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 2L2 7l10 5 10-5-10-5z"/>
                <path d="M2 17l10 5 10-5"/><path d="M2 12l10 5 10-5"/>
              </svg>
              {{ t('centerCanvas.toolbar.cutout') }}
            </button>
          </div>
        </template>
      </div>

      <!-- Adjust expand row -->
      <Transition name="expand">
        <div v-if="showAdjust && canvasStore.imageLayer" class="adjust-row">
          <div class="adjust-item">
            <label class="adjust-label">{{ t('centerCanvas.adjustments.brightness') }}</label>
            <input type="range" class="adjust-slider" min="-100" max="100"
              v-model.number="canvasStore.imageLayer.brightness" />
            <span class="adjust-val">{{ canvasStore.imageLayer.brightness }}</span>
          </div>
          <div class="adjust-item">
            <label class="adjust-label">{{ t('centerCanvas.adjustments.contrast') }}</label>
            <input type="range" class="adjust-slider" min="-100" max="100"
              v-model.number="canvasStore.imageLayer.contrast" />
            <span class="adjust-val">{{ canvasStore.imageLayer.contrast }}</span>
          </div>
          <div class="adjust-item">
            <label class="adjust-label">{{ t('centerCanvas.adjustments.saturation') }}</label>
            <input type="range" class="adjust-slider" min="-100" max="100"
              v-model.number="canvasStore.imageLayer.saturation" />
            <span class="adjust-val">{{ canvasStore.imageLayer.saturation }}</span>
          </div>
          <div class="adjust-item">
            <label class="adjust-label">{{ t('centerCanvas.adjustments.opacity') }}</label>
            <input type="range" class="adjust-slider" min="0" max="100"
              :value="Math.round(canvasStore.imageLayer.opacity * 100)"
              @input="(e) => canvasStore.imageLayer && (canvasStore.imageLayer.opacity = Number((e.target as HTMLInputElement).value) / 100)" />
            <span class="adjust-val">{{ Math.round(canvasStore.imageLayer.opacity * 100) }}%</span>
          </div>
        </div>
      </Transition>
    </div>

    <!-- Canvas viewport -->
    <div class="canvas-viewport" ref="viewportRef">
      <div
        class="canvas-container"
        :style="{ transform: `translate(${canvasStore.panX}px, ${canvasStore.panY}px) scale(${canvasStore.zoom})` }"
        @wheel.passive="onWheel"
        @mousedown="onMouseDown"
      >
        <canvas
          ref="canvasRef"
          :width="canvasStore.canvasWidth"
          :height="canvasStore.canvasHeight"
          class="main-canvas"
          :style="{ cursor: canvasCursor }"
          @click="onCanvasClick"
          @mousemove="onCanvasHover"
          @mouseleave="isFinetuning ? (showBrushCursor = false) : (canvasCursor = 'default')"
        />
      </div>

      <!-- Brush cursor overlay -->
      <div
        v-if="isFinetuning && showBrushCursor"
        class="brush-cursor"
        :class="{ 'brush-cursor--add': brushMode === 'add' }"
        :style="{
          left: `${brushCursorX - brushDisplayRadius}px`,
          top: `${brushCursorY - brushDisplayRadius}px`,
          width: `${brushDisplayRadius * 2}px`,
          height: `${brushDisplayRadius * 2}px`,
        }"
      />

      <!-- Empty state -->
      <div v-if="!canvasStore.hasImage" class="canvas-empty">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1">
          <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
          <circle cx="8.5" cy="8.5" r="1.5"/>
          <polyline points="21 15 16 10 5 21"/>
        </svg>
        <p>{{ t('centerCanvas.empty.title') }}</p>
      </div>

      <!-- ── Floating quick-action bar ── -->
      <Transition name="slide-up">
        <div
          v-if="canvasStore.hasImage && showQuickBar && !canvasStore.hasPendingMask && canvasStore.segMode === 'none'"
          class="quick-bar"
        >
          <div class="quick-bar__inner">
            <span class="quick-bar__label">{{ t('centerCanvas.quickBar.title') }}</span>
            <div class="quick-bar__divider" />
            <!-- Inline tolerance slider -->
            <div class="qb-tolerance" :title="t('centerCanvas.quickBar.strengthHint')">
              <span class="qb-tolerance__label">{{ t('centerCanvas.quickBar.strength') }}</span>
              <input
                type="range"
                class="qb-tolerance__track"
                min="0"
                max="100"
                v-model.number="canvasStore.segTolerance"
              />
              <span class="qb-tolerance__val">{{ canvasStore.segTolerance }}%</span>
            </div>
            <div class="quick-bar__divider" />
            <!-- SAM2 debug params -->
            <div class="qb-debug">
              <span class="qb-debug__label" :title="t('centerCanvas.quickBar.thresholdHint')">
                {{ t('centerCanvas.quickBar.threshold', { value: canvasStore.sam2Threshold.toFixed(2) }) }}
              </span>
              <input
                type="range" class="qb-tolerance__track"
                min="0.05" max="0.95" step="0.05"
                :value="canvasStore.sam2Threshold"
                @input="canvasStore.sam2Threshold = parseFloat(($event.target as HTMLInputElement).value)"
              />
              <span class="qb-debug__label qb-debug__gap" :title="t('centerCanvas.quickBar.edgeHint')">
                {{ t('centerCanvas.quickBar.edge', { value: canvasStore.matteRadius }) }}
              </span>
              <input
                type="range" class="qb-tolerance__track"
                min="1" max="20" step="1"
                :value="canvasStore.matteRadius"
                @input="canvasStore.matteRadius = parseInt(($event.target as HTMLInputElement).value)"
              />
              <span v-if="canvasStore.lastSegMethod" class="qb-debug__method">
                {{ canvasStore.lastSegMethod }}
              </span>
            </div>
            <div class="quick-bar__divider" />
            <button
              class="qb-btn qb-btn--primary"
              :disabled="isSegRunning"
              @click="runSegmentation('auto')"
            >
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>
              </svg>
              {{ isSegRunning ? t('app.loading.processing') : t('centerCanvas.quickBar.auto') }}
            </button>
            <button
              class="qb-btn"
              @click="canvasStore.segMode = 'point'"
            >
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="3"/><circle cx="12" cy="12" r="9"/>
              </svg>
              {{ t('centerCanvas.quickBar.point') }}
            </button>
            <template v-if="canvasStore.imageLayer?.hasMask">
              <div class="quick-bar__divider" />
              <button class="qb-btn qb-btn--danger" @click="canvasStore.clearMask()">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
                </svg>
                {{ t('centerCanvas.quickBar.clearMask') }}
              </button>
            </template>
            <!-- Close button -->
            <div class="quick-bar__divider" />
            <button class="qb-btn qb-btn--close" @click="showQuickBar = false" :title="t('centerCanvas.quickBar.close')">
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
              </svg>
            </button>
          </div>
        </div>
      </Transition>

      <!-- ── Point-select mode bar ── -->
      <Transition name="slide-up">
        <div v-if="canvasStore.segMode === 'point' && !canvasStore.hasPendingMask" class="point-bar">
          <div class="point-bar__inner">
            <div class="point-bar__hint">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="3"/><circle cx="12" cy="12" r="9"/>
              </svg>
              <span>{{ t('centerCanvas.pointBar.hint') }}</span>
              <span class="point-count">{{ t('centerCanvas.pointBar.count', { count: canvasStore.segPoints.length }) }}</span>
            </div>
            <div class="point-bar__actions">
              <button class="qb-btn" @click="canvasStore.segMode = 'none'; canvasStore.clearSegPoints()">
                {{ t('common.cancel') }}
              </button>
              <button
                class="qb-btn qb-btn--primary"
                :disabled="canvasStore.segPoints.length === 0 || isSegRunning"
                @click="async () => { await runSegmentation('point'); canvasStore.segMode = 'none' }"
              >
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>
                </svg>
                {{ isSegRunning ? t('app.loading.processing') : t('centerCanvas.pointBar.run', { count: canvasStore.segPoints.length }) }}
              </button>
            </div>
          </div>
        </div>
      </Transition>

      <!-- ── Pending mask confirmation bar ── -->
      <Transition name="slide-up">
        <div v-if="canvasStore.hasPendingMask" class="confirm-bar">
          <!-- Fine-tune brush row -->
          <Transition name="expand">
            <div v-if="isFinetuning" class="finetune-row">
              <div class="ft-mode">
                <button
                  class="ft-mode-btn"
                  :class="{ 'ft-mode-btn--active': brushMode === 'erase' }"
                  @click="brushMode = 'erase'"
                  :title="t('centerCanvas.finetune.eraseHint')"
                >
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M20 20H7L3 16l10-10 7 7-1.5 1.5"/><path d="M6.5 17.5l5-5"/>
                  </svg>
                  {{ t('centerCanvas.finetune.erase') }}
                </button>
                <button
                  class="ft-mode-btn"
                  :class="{ 'ft-mode-btn--active': brushMode === 'add' }"
                  @click="brushMode = 'add'"
                  :title="t('centerCanvas.finetune.brushHint')"
                >
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 013 3L7 19l-4 1 1-4L16.5 3.5z"/>
                  </svg>
                  {{ t('centerCanvas.finetune.brush') }}
                </button>
              </div>
              <div class="ft-divider" />
              <div class="ft-size">
                <span class="ft-label">{{ t('centerCanvas.finetune.brushSize') }}</span>
                <input
                  type="range" class="ft-slider"
                  min="5" max="120" step="1"
                  v-model.number="brushSize"
                />
                <span class="ft-val">{{ brushSize }}px</span>
              </div>
              <div class="ft-divider" />
              <span class="ft-hint">
                {{ brushMode === 'erase' ? t('centerCanvas.finetune.eraseGuide') : t('centerCanvas.finetune.brushGuide') }}
              </span>
            </div>
          </Transition>

          <div class="confirm-bar__inner">
            <div class="confirm-bar__icon" :class="{ 'confirm-bar__icon--finetune': isFinetuning }">
              <svg v-if="!isFinetuning" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/>
                <circle cx="12" cy="12" r="3"/>
              </svg>
              <svg v-else width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 013 3L7 19l-4 1 1-4L16.5 3.5z"/>
              </svg>
            </div>
            <div class="confirm-bar__text">
              <span class="confirm-bar__title">{{ isFinetuning ? t('centerCanvas.confirmBar.finetuneTitle') : t('centerCanvas.confirmBar.previewTitle') }}</span>
              <span class="confirm-bar__sub">{{ isFinetuning ? t('centerCanvas.confirmBar.finetuneSubtitle') : t('centerCanvas.confirmBar.previewSubtitle') }}</span>
            </div>
            <div class="confirm-bar__actions">
              <button class="cb-btn cb-btn--discard" @click="canvasStore.discardPendingMask()">
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
                </svg>
                {{ t('centerCanvas.confirmBar.discard') }}
              </button>
              <button
                class="cb-btn cb-btn--invert"
                @click="canvasStore.invertPendingMask()"
                :disabled="isFinetuning"
                :title="t('centerCanvas.confirmBar.invertHint')"
              >
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="10"/>
                  <path d="M12 2a10 10 0 0 1 0 20V2z" fill="currentColor" stroke="none"/>
                </svg>
                {{ t('centerCanvas.confirmBar.invert') }}
              </button>
              <button
                class="cb-btn"
                :class="isFinetuning ? 'cb-btn--finetune-active' : 'cb-btn--finetune'"
                @click="isFinetuning ? exitFinetune() : enterFinetune()"
                :title="t('centerCanvas.confirmBar.finetuneHint')"
              >
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 013 3L7 19l-4 1 1-4L16.5 3.5z"/>
                </svg>
                {{ isFinetuning ? t('centerCanvas.confirmBar.exitFinetune') : t('centerCanvas.confirmBar.finetune') }}
              </button>
              <button class="cb-btn cb-btn--confirm" @click="canvasStore.confirmPendingMask()">
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <polyline points="20 6 9 17 4 12"/>
                </svg>
                {{ t('centerCanvas.confirmBar.apply') }}
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </div>
  </main>
</template>

<style scoped>
.canvas-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  background: var(--bg-app);
  overflow: hidden;
}

/* ── Toolbar ─────────────────────────────────────────────────────── */
.canvas-toolbar {
  flex-shrink: 0;
  background: var(--bg-panel);
  border-bottom: 1px solid var(--border);
}

.toolbar-row {
  display: flex;
  align-items: center;
  padding: var(--space-2) var(--space-4);
  gap: var(--space-2);
  min-height: 40px;
}

.toolbar-group {
  display: flex;
  align-items: center;
  gap: var(--space-1);
}

.toolbar-divider {
  width: 1px;
  height: 18px;
  background: var(--border);
  margin: 0 var(--space-2);
  flex-shrink: 0;
}

.toolbar-section-label {
  font-size: 10px;
  font-weight: 600;
  color: var(--text-disabled);
  letter-spacing: 0.07em;
  text-transform: uppercase;
  padding: 0 4px;
  white-space: nowrap;
}

.toolbar-btn {
  font-size: var(--text-xs);
  padding: 4px var(--space-2);
  height: 26px;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  white-space: nowrap;
}
.toolbar-btn.icon-only {
  padding: 4px 6px;
  min-width: 26px;
  justify-content: center;
}
.toolbar-btn--active {
  background: rgba(99, 102, 241, 0.15) !important;
  color: var(--accent-hover) !important;
  border-color: rgba(99, 102, 241, 0.35) !important;
}

.zoom-label {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  font-variant-numeric: tabular-nums;
  min-width: 34px;
  text-align: center;
}

.scale-display {
  font-size: var(--text-xs);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
  min-width: 38px;
  text-align: center;
  background: var(--bg-input);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  padding: 3px 6px;
  line-height: 1;
}

/* ── Adjust expand row ───────────────────────────────────────────── */
.adjust-row {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  padding: 8px var(--space-4);
  border-top: 1px solid var(--border);
  background: var(--bg-input);
  flex-wrap: wrap;
}

.adjust-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: 1;
  min-width: 120px;
}

.adjust-label {
  font-size: 10px;
  color: var(--text-tertiary);
  white-space: nowrap;
  min-width: 30px;
}

.adjust-slider {
  flex: 1;
  -webkit-appearance: none;
  appearance: none;
  height: 3px;
  background: var(--border);
  border-radius: 2px;
  outline: none;
  cursor: pointer;
}
.adjust-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--accent);
  cursor: pointer;
  border: 2px solid var(--bg-panel);
}

.adjust-val {
  font-size: 10px;
  color: var(--text-tertiary);
  min-width: 30px;
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.expand-enter-active,
.expand-leave-active {
  transition: max-height 0.22s ease, opacity 0.2s ease;
  overflow: hidden;
  max-height: 80px;
}
.expand-enter-from,
.expand-leave-to {
  max-height: 0;
  opacity: 0;
}

/* ── Viewport ─────────────────────────────────────────────────────── */
.canvas-viewport {
  flex: 1;
  overflow: hidden;
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  background-image:
    linear-gradient(45deg, var(--canvas-checker-dark) 25%, transparent 25%),
    linear-gradient(-45deg, var(--canvas-checker-dark) 25%, transparent 25%),
    linear-gradient(45deg, transparent 75%, var(--canvas-checker-dark) 75%),
    linear-gradient(-45deg, transparent 75%, var(--canvas-checker-dark) 75%);
  background-size: 16px 16px;
  background-position: 0 0, 0 8px, 8px -8px, -8px 0px;
  background-color: var(--canvas-checker-light);
}

.canvas-container {
  transition: transform 0.05s ease;
  transform-origin: center center;
}

.main-canvas {
  display: block;
  box-shadow: var(--shadow-lg);
}

.canvas-empty {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-4);
  color: var(--text-disabled);
  font-size: var(--text-sm);
  pointer-events: none;
}

/* ── Quick-bar ────────────────────────────────────────────────────── */
.quick-bar {
  position: absolute;
  top: var(--space-4);
  left: 50%;
  transform: translateX(-50%);
  z-index: var(--z-overlay);
  pointer-events: auto;
}

.quick-bar__inner {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: 5px var(--space-3);
  background: var(--bg-panel);
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
  border: 1px solid var(--border);
  border-radius: var(--radius-full);
  box-shadow: var(--shadow-md);
}

.quick-bar__label {
  font-size: var(--text-xs);
  font-weight: 600;
  color: var(--text-tertiary);
  letter-spacing: 0.06em;
  text-transform: uppercase;
  padding: 0 2px;
  white-space: nowrap;
}

.quick-bar__divider {
  width: 1px;
  height: 16px;
  background: var(--border);
  flex-shrink: 0;
}

/* ── Tolerance inline ─────────────────────────────────────────────── */
.qb-tolerance {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 4px;
}
.qb-tolerance__label {
  font-size: 10px;
  color: var(--text-tertiary);
  white-space: nowrap;
}
.qb-tolerance__track {
  width: 72px;
  -webkit-appearance: none;
  appearance: none;
  height: 3px;
  background: var(--border);
  border-radius: 2px;
  outline: none;
  cursor: pointer;
}
.qb-tolerance__track::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: 11px;
  height: 11px;
  border-radius: 50%;
  background: var(--accent);
  cursor: pointer;
  border: 2px solid var(--bg-panel);
}
.qb-tolerance__val {
  font-size: 10px;
  color: var(--text-tertiary);
  min-width: 26px;
  font-variant-numeric: tabular-nums;
}

/* ── SAM2 debug params ───────────────────────────────────────────── */
.qb-debug {
  display: flex;
  align-items: center;
  gap: 5px;
}
.qb-debug__label {
  font-size: 10px;
  color: var(--text-tertiary);
  white-space: nowrap;
  cursor: help;
}
.qb-debug__gap {
  margin-left: 4px;
}
.qb-debug__method {
  font-size: 9px;
  color: var(--accent-primary, #6c63ff);
  background: color-mix(in srgb, var(--accent-primary, #6c63ff) 12%, transparent);
  border-radius: 3px;
  padding: 1px 5px;
  white-space: nowrap;
  margin-left: 2px;
}

/* ── Point-bar ────────────────────────────────────────────────────── */
.point-bar {
  position: absolute;
  top: var(--space-4);
  left: 50%;
  transform: translateX(-50%);
  z-index: var(--z-overlay);
  pointer-events: auto;
  min-width: 480px;
}

.point-bar__inner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  padding: 8px var(--space-4);
  background: var(--bg-panel);
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
  border: 1px solid rgba(99, 102, 241, 0.3);
  border-radius: var(--radius-full);
  box-shadow: var(--shadow-md), 0 0 0 1px rgba(99, 102, 241, 0.1);
}

.point-bar__hint {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--text-xs);
  color: var(--text-secondary);
  flex-shrink: 0;
}

.point-bar__hint kbd {
  display: inline-flex;
  align-items: center;
  padding: 1px 5px;
  background: var(--bg-button);
  border: 1px solid var(--border);
  border-radius: 4px;
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--text-primary);
}

.point-count { color: var(--text-tertiary); }
.point-count b { color: var(--accent-hover); }

.point-bar__actions {
  display: flex;
  gap: var(--space-2);
  flex-shrink: 0;
}

/* ── Confirm bar ──────────────────────────────────────────────────── */
.confirm-bar {
  position: absolute;
  bottom: 24px;
  left: 50%;
  transform: translateX(-50%);
  z-index: var(--z-overlay);
  pointer-events: auto;
  width: max-content;
  min-width: 540px;
  max-width: min(720px, calc(100vw - 48px));
  background: var(--bg-panel);
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
  border: 1px solid rgba(99, 102, 241, 0.35);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-lg), 0 0 0 1px rgba(99, 102, 241, 0.15), var(--shadow-glow);
  overflow: hidden;
}

.confirm-bar__inner {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: 10px var(--space-4);
}

.confirm-bar__icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: var(--radius-md);
  background: rgba(99, 102, 241, 0.15);
  color: var(--accent-hover);
  flex-shrink: 0;
}

.confirm-bar__text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex: 1;
  min-width: 100px;
}
.confirm-bar__title {
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--text-primary);
  white-space: nowrap;
}
.confirm-bar__sub {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  white-space: nowrap;
}

.confirm-bar__actions {
  display: flex;
  gap: var(--space-2);
  flex-shrink: 0;
  flex-shrink: 0;
}

.cb-btn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 6px 14px;
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  font-weight: 600;
  cursor: pointer;
  border: 1px solid transparent;
  transition: all var(--transition-fast);
}
.cb-btn--discard {
  background: var(--bg-button);
  color: var(--text-secondary);
  border-color: var(--border);
}
.cb-btn--discard:hover {
  background: rgba(239, 68, 68, 0.12);
  color: var(--danger);
  border-color: rgba(239, 68, 68, 0.3);
}
.cb-btn--invert {
  background: var(--bg-button);
  color: var(--text-secondary);
  border-color: var(--border);
}
.cb-btn--invert:hover {
  background: rgba(99, 102, 241, 0.12);
  color: var(--accent-hover);
  border-color: rgba(99, 102, 241, 0.35);
}
.cb-btn--confirm {
  background: var(--accent);
  color: #fff;
}
.cb-btn--confirm:hover {
  background: var(--accent-hover);
  box-shadow: var(--shadow-glow);
  transform: translateY(-1px);
}
.cb-btn--confirm:active { transform: translateY(0); }
.cb-btn--finetune {
  background: var(--bg-button);
  color: var(--text-secondary);
  border-color: var(--border);
}
.cb-btn--finetune:hover {
  background: rgba(99, 102, 241, 0.12);
  color: var(--accent-hover);
  border-color: rgba(99, 102, 241, 0.35);
}
.cb-btn--finetune-active {
  background: rgba(99, 102, 241, 0.18);
  color: var(--accent-hover);
  border-color: rgba(99, 102, 241, 0.5);
}
.cb-btn--finetune-active:hover {
  background: rgba(99, 102, 241, 0.25);
}
.cb-btn:disabled {
  opacity: 0.35;
  cursor: not-allowed;
  pointer-events: none;
}

/* ── Shared quick-button ─────────────────────────────────────────── */
.qb-btn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 4px 11px;
  background: var(--bg-button);
  color: var(--text-secondary);
  border: 1px solid var(--border);
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  font-weight: 500;
  cursor: pointer;
  white-space: nowrap;
  transition: all var(--transition-fast);
}
.qb-btn:hover {
  background: var(--bg-button-hover);
  color: var(--text-primary);
  border-color: var(--border-hover);
}
.qb-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
.qb-btn--primary {
  background: var(--accent);
  color: #fff;
  border-color: transparent;
}
.qb-btn--primary:hover:not(:disabled) {
  background: var(--accent-hover);
  box-shadow: 0 0 12px var(--accent-glow);
}
.qb-btn--danger:hover {
  color: var(--danger);
  background: rgba(239, 68, 68, 0.08);
  border-color: rgba(239, 68, 68, 0.2);
}
.qb-btn--close {
  padding: 4px 7px;
  color: var(--text-disabled);
}
.qb-btn--close:hover {
  color: var(--text-primary);
  background: var(--bg-button-hover);
}

/* ── Fine-tune row ───────────────────────────────────────────────────── */
.finetune-row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: 8px var(--space-4);
  border-bottom: 1px solid rgba(99, 102, 241, 0.2);
  flex-wrap: wrap;
}

.ft-mode {
  display: flex;
  gap: 4px;
  flex-shrink: 0;
}

.ft-mode-btn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 4px 10px;
  border-radius: var(--radius-md);
  font-size: var(--text-xs);
  font-weight: 500;
  cursor: pointer;
  border: 1px solid var(--border);
  background: var(--bg-button);
  color: var(--text-secondary);
  transition: all var(--transition-fast);
}
.ft-mode-btn:hover {
  background: var(--bg-button-hover);
  color: var(--text-primary);
}
.ft-mode-btn--active {
  background: rgba(99, 102, 241, 0.18);
  color: var(--accent-hover);
  border-color: rgba(99, 102, 241, 0.5);
}

.ft-divider {
  width: 1px;
  height: 16px;
  background: rgba(99, 102, 241, 0.2);
  flex-shrink: 0;
}

.ft-size {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.ft-label {
  font-size: 10px;
  color: var(--text-tertiary);
  white-space: nowrap;
}

.ft-slider {
  width: 90px;
  -webkit-appearance: none;
  appearance: none;
  height: 3px;
  background: var(--border);
  border-radius: 2px;
  outline: none;
  cursor: pointer;
}
.ft-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--accent);
  cursor: pointer;
  border: 2px solid var(--bg-panel);
}

.ft-val {
  font-size: 10px;
  color: var(--text-tertiary);
  min-width: 30px;
  font-variant-numeric: tabular-nums;
}

.ft-hint {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* ── Brush cursor overlay ────────────────────────────────────────────── */
.brush-cursor {
  position: absolute;
  border-radius: 50%;
  border: 1.5px solid rgba(239, 68, 68, 0.85);
  background: rgba(239, 68, 68, 0.08);
  pointer-events: none;
  z-index: calc(var(--z-overlay) + 1);
  box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.25);
}
.brush-cursor--add {
  border-color: rgba(99, 102, 241, 0.9);
  background: rgba(99, 102, 241, 0.1);
}

/* ── Confirm bar icon variant ────────────────────────────────────────── */
.confirm-bar__icon--finetune {
  background: rgba(99, 102, 241, 0.2);
}

/* ── Transition ──────────────────────────────────────────────────── */
.slide-up-enter-active,
.slide-up-leave-active {
  transition: opacity 0.2s ease, transform 0.25s cubic-bezier(0.34, 1.56, 0.64, 1);
}
.slide-up-enter-from,
.slide-up-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(16px);
}
</style>

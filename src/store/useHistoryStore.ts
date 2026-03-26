import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { ImageLayer } from './useCanvasStore'
import type { BgType, GradientStop } from './useBackgroundStore'
import type { TextLayer } from './useTypographyStore'

const MAX_HISTORY = 50

interface CanvasSnapshot {
  imageLayer: ImageLayer | null
}

interface BackgroundSnapshot {
  bgType: BgType
  stops: GradientStop[]
  angle: number
  solidColor: string
  borderRadius: number
  shadowEnabled: boolean
  shadowColor: string
  shadowBlur: number
  shadowOffsetX: number
  shadowOffsetY: number
  innerGlow: boolean
  innerGlowColor: string
  innerGlowBlur: number
  activePresetId: string | null
}

interface TypographySnapshot {
  textLayers: TextLayer[]
  selectedLayerId: string | null
}

export interface HistorySnapshot {
  canvas: CanvasSnapshot
  background: BackgroundSnapshot
  typography: TypographySnapshot
}

export const useHistoryStore = defineStore('history', () => {
  const undoStack = ref<HistorySnapshot[]>([])
  const redoStack = ref<HistorySnapshot[]>([])

  const canUndo = computed(() => undoStack.value.length > 1)
  const canRedo = computed(() => redoStack.value.length > 0)

  /**
   * Call this after every meaningful user action to push a snapshot.
   * Pass the current state from all three stores.
   */
  function push(snapshot: HistorySnapshot) {
    // Deep-clone to avoid shared references
    undoStack.value.push(deepClone(snapshot))
    if (undoStack.value.length > MAX_HISTORY) {
      undoStack.value.shift()
    }
    redoStack.value = []
  }

  /** Initialize with the very first state (no undo available yet). */
  function init(snapshot: HistorySnapshot) {
    undoStack.value = [deepClone(snapshot)]
    redoStack.value = []
  }

  /**
   * Returns the previous snapshot and moves current to redo stack.
   * Returns null if nothing to undo.
   */
  function undo(): HistorySnapshot | null {
    if (undoStack.value.length <= 1) return null
    const current = undoStack.value.pop()!
    redoStack.value.push(current)
    return deepClone(undoStack.value[undoStack.value.length - 1])
  }

  /**
   * Returns the next snapshot and moves it to undo stack.
   * Returns null if nothing to redo.
   */
  function redo(): HistorySnapshot | null {
    if (redoStack.value.length === 0) return null
    const next = redoStack.value.pop()!
    undoStack.value.push(next)
    return deepClone(next)
  }

  function clear() {
    undoStack.value = []
    redoStack.value = []
  }

  return { undoStack, redoStack, canUndo, canRedo, push, init, undo, redo, clear }
})

function deepClone<T>(val: T): T {
  return JSON.parse(JSON.stringify(val))
}

/**
 * Convenience composable: take a snapshot of the current store state
 * and restore it back when undo/redo is triggered.
 */
import { useHistoryStore, type HistorySnapshot } from '../../store/useHistoryStore'
import { useCanvasStore } from '../../store/useCanvasStore'
import { useBackgroundStore } from '../../store/useBackgroundStore'
import { useTypographyStore } from '../../store/useTypographyStore'

export function useHistory() {
  const history = useHistoryStore()
  const canvasStore = useCanvasStore()
  const bgStore = useBackgroundStore()
  const typoStore = useTypographyStore()

  function capture(): HistorySnapshot {
    return {
      canvas: {
        imageLayer: canvasStore.imageLayer ? { ...canvasStore.imageLayer } : null,
      },
      background: {
        bgType: bgStore.bgType,
        stops: bgStore.stops.map(s => ({ ...s })),
        angle: bgStore.angle,
        solidColor: bgStore.solidColor,
        borderRadius: bgStore.borderRadius,
        shadowEnabled: bgStore.shadowEnabled,
        shadowColor: bgStore.shadowColor,
        shadowBlur: bgStore.shadowBlur,
        shadowOffsetX: bgStore.shadowOffsetX,
        shadowOffsetY: bgStore.shadowOffsetY,
        innerGlow: bgStore.innerGlow,
        innerGlowColor: bgStore.innerGlowColor,
        innerGlowBlur: bgStore.innerGlowBlur,
        activePresetId: bgStore.activePresetId,
      },
      typography: {
        textLayers: typoStore.textLayers.map(l => ({ ...l })),
        selectedLayerId: typoStore.selectedLayerId,
      },
    }
  }

  function snapshot() {
    history.push(capture())
  }

  function initHistory() {
    history.init(capture())
  }

  function restore(snap: HistorySnapshot) {
    // Canvas
    canvasStore.imageLayer = snap.canvas.imageLayer ? { ...snap.canvas.imageLayer } : null
    if (!snap.canvas.imageLayer) {
      canvasStore.activeLayerId = null
    } else {
      canvasStore.activeLayerId = snap.canvas.imageLayer.id
    }

    // Background
    bgStore.bgType = snap.background.bgType
    bgStore.stops = snap.background.stops.map(s => ({ ...s }))
    bgStore.angle = snap.background.angle
    bgStore.solidColor = snap.background.solidColor
    bgStore.borderRadius = snap.background.borderRadius
    bgStore.shadowEnabled = snap.background.shadowEnabled
    bgStore.shadowColor = snap.background.shadowColor
    bgStore.shadowBlur = snap.background.shadowBlur
    bgStore.shadowOffsetX = snap.background.shadowOffsetX
    bgStore.shadowOffsetY = snap.background.shadowOffsetY
    bgStore.innerGlow = snap.background.innerGlow
    bgStore.innerGlowColor = snap.background.innerGlowColor
    bgStore.innerGlowBlur = snap.background.innerGlowBlur
    bgStore.activePresetId = snap.background.activePresetId

    // Typography
    typoStore.textLayers = snap.typography.textLayers.map(l => ({ ...l }))
    typoStore.selectedLayerId = snap.typography.selectedLayerId
  }

  function undo() {
    const snap = history.undo()
    if (snap) restore(snap)
  }

  function redo() {
    const snap = history.redo()
    if (snap) restore(snap)
  }

  return { snapshot, initHistory, undo, redo, capture, restore }
}

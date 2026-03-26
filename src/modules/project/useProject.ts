import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { save, open } from '@tauri-apps/plugin-dialog'
import { useCanvasStore } from '../../store/useCanvasStore'
import { useBackgroundStore } from '../../store/useBackgroundStore'
import { useTypographyStore } from '../../store/useTypographyStore'
import { useAppStore } from '../../store/useAppStore'
import { useHistoryStore } from '../../store/useHistoryStore'

/** Format version — bump when breaking changes are made to the schema */
const FORMAT_VERSION = 1

interface LspProject {
  _format: 'logo-studio-project'
  _version: number
  appVersion: string
  savedAt: string
  canvas: {
    imageLayer: ReturnType<typeof useCanvasStore>['imageLayer']
  }
  background: {
    bgType: ReturnType<typeof useBackgroundStore>['bgType']
    stops: ReturnType<typeof useBackgroundStore>['stops']
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
  typography: {
    textLayers: ReturnType<typeof useTypographyStore>['textLayers']
    selectedLayerId: string | null
  }
}

export function useProject() {
  const canvasStore = useCanvasStore()
  const bgStore = useBackgroundStore()
  const typoStore = useTypographyStore()
  const appStore = useAppStore()
  const historyStore = useHistoryStore()

  const currentFilePath = ref<string | null>(null)
  const isDirty = ref(false)

  function buildProject(): LspProject {
    return {
      _format: 'logo-studio-project',
      _version: FORMAT_VERSION,
      appVersion: __APP_VERSION__,
      savedAt: new Date().toISOString(),
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

  function applyProject(proj: LspProject) {
    // Canvas
    if (proj.canvas.imageLayer) {
      canvasStore.imageLayer = { ...proj.canvas.imageLayer }
      canvasStore.activeLayerId = proj.canvas.imageLayer.id
    } else {
      canvasStore.imageLayer = null
      canvasStore.activeLayerId = null
    }
    canvasStore.segMode = 'none'
    canvasStore.segPoints = []
    canvasStore.pendingMaskDataUrl = null

    // Background
    const bg = proj.background
    bgStore.bgType = bg.bgType
    bgStore.stops = bg.stops.map(s => ({ ...s }))
    bgStore.angle = bg.angle
    bgStore.solidColor = bg.solidColor
    bgStore.borderRadius = bg.borderRadius
    bgStore.shadowEnabled = bg.shadowEnabled
    bgStore.shadowColor = bg.shadowColor
    bgStore.shadowBlur = bg.shadowBlur
    bgStore.shadowOffsetX = bg.shadowOffsetX
    bgStore.shadowOffsetY = bg.shadowOffsetY
    bgStore.innerGlow = bg.innerGlow
    bgStore.innerGlowColor = bg.innerGlowColor
    bgStore.innerGlowBlur = bg.innerGlowBlur
    bgStore.activePresetId = bg.activePresetId

    // Typography
    typoStore.textLayers = proj.typography.textLayers.map(l => ({ ...l }))
    typoStore.selectedLayerId = proj.typography.selectedLayerId
  }

  async function saveProject(forceSaveAs = false) {
    let filePath = currentFilePath.value
    if (!filePath || forceSaveAs) {
      filePath = await save({
        title: '保存项目',
        defaultPath: 'untitled.lsp',
        filters: [{ name: 'Logo Studio 项目', extensions: ['lsp'] }],
      })
      if (!filePath) return
    }

    appStore.setLoading(true, '正在保存项目...')
    try {
      const data = JSON.stringify(buildProject(), null, 2)
      await invoke('write_text_file', { path: filePath, content: data })
      currentFilePath.value = filePath
      isDirty.value = false
      appStore.showToast(`已保存：${filePath}`, 'info')
    } catch (err) {
      appStore.showToast(`保存失败：${String(err)}`, 'error')
    } finally {
      appStore.setLoading(false)
    }
  }

  async function openProject() {
    const filePath = await open({
      title: '打开项目',
      multiple: false,
      filters: [{ name: 'Logo Studio 项目', extensions: ['lsp'] }],
    })
    if (!filePath || typeof filePath !== 'string') return

    appStore.setLoading(true, '正在加载项目...')
    try {
      const raw = await invoke<string>('read_text_file', { path: filePath })
      const proj = JSON.parse(raw) as LspProject
      if (proj._format !== 'logo-studio-project') {
        appStore.showToast('文件格式不正确', 'error')
        return
      }
      applyProject(proj)
      currentFilePath.value = filePath
      isDirty.value = false
      historyStore.clear()
      appStore.showToast(`已打开：${filePath}`, 'info')
    } catch (err) {
      appStore.showToast(`打开失败：${String(err)}`, 'error')
    } finally {
      appStore.setLoading(false)
    }
  }

  async function newProject() {
    canvasStore.clearImage()
    bgStore.applyPreset('ios')
    typoStore.textLayers = []
    typoStore.selectedLayerId = null
    currentFilePath.value = null
    isDirty.value = false
    historyStore.clear()
  }

  return {
    currentFilePath,
    isDirty,
    saveProject,
    openProject,
    newProject,
  }
}

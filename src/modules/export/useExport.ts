import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { save, open } from '@tauri-apps/plugin-dialog'
import { useCanvasStore } from '../../store/useCanvasStore'
import { useBackgroundStore } from '../../store/useBackgroundStore'
import { useTypographyStore } from '../../store/useTypographyStore'
import { useAppStore } from '../../store/useAppStore'
import { i18n } from '../../i18n'
import { useCanvasRenderer } from '../image-editor/useImageEditor'

interface IconEntry { size: number; relpath: string }

const WEB_ICONS: IconEntry[] = [
  { size: 16, relpath: 'favicon-16x16.png' },
  { size: 32, relpath: 'favicon-32x32.png' },
  { size: 48, relpath: 'favicon-48x48.png' },
  { size: 64, relpath: 'icon-64x64.png' },
  { size: 96, relpath: 'icon-96x96.png' },
  { size: 128, relpath: 'icon-128x128.png' },
  { size: 192, relpath: 'icon-192x192.png' },
  { size: 256, relpath: 'icon-256x256.png' },
  { size: 384, relpath: 'icon-384x384.png' },
  { size: 512, relpath: 'icon-512x512.png' },
]

const IOS_ICONS: IconEntry[] = [
  { size: 20, relpath: 'ios/AppIcon-20@1x.png' },
  { size: 40, relpath: 'ios/AppIcon-20@2x.png' },
  { size: 60, relpath: 'ios/AppIcon-20@3x.png' },
  { size: 29, relpath: 'ios/AppIcon-29@1x.png' },
  { size: 58, relpath: 'ios/AppIcon-29@2x.png' },
  { size: 87, relpath: 'ios/AppIcon-29@3x.png' },
  { size: 40, relpath: 'ios/AppIcon-40@1x.png' },
  { size: 80, relpath: 'ios/AppIcon-40@2x.png' },
  { size: 120, relpath: 'ios/AppIcon-40@3x.png' },
  { size: 60, relpath: 'ios/AppIcon-60@1x.png' },
  { size: 120, relpath: 'ios/AppIcon-60@2x.png' },
  { size: 180, relpath: 'ios/AppIcon-60@3x.png' },
  { size: 76, relpath: 'ios/AppIcon-76@1x.png' },
  { size: 152, relpath: 'ios/AppIcon-76@2x.png' },
  { size: 167, relpath: 'ios/AppIcon-83.5@2x.png' },
  { size: 1024, relpath: 'ios/AppIcon-1024x1024.png' },
]

const ANDROID_ICONS: IconEntry[] = [
  { size: 36, relpath: 'android/mipmap-ldpi/ic_launcher.png' },
  { size: 48, relpath: 'android/mipmap-mdpi/ic_launcher.png' },
  { size: 72, relpath: 'android/mipmap-hdpi/ic_launcher.png' },
  { size: 96, relpath: 'android/mipmap-xhdpi/ic_launcher.png' },
  { size: 144, relpath: 'android/mipmap-xxhdpi/ic_launcher.png' },
  { size: 192, relpath: 'android/mipmap-xxxhdpi/ic_launcher.png' },
  { size: 512, relpath: 'android/ic_launcher-playstore.png' },
]

const MACOS_ICONS: IconEntry[] = [
  { size: 16, relpath: 'macos/icon_16x16.png' },
  { size: 32, relpath: 'macos/icon_16x16@2x.png' },
  { size: 32, relpath: 'macos/icon_32x32.png' },
  { size: 64, relpath: 'macos/icon_32x32@2x.png' },
  { size: 128, relpath: 'macos/icon_128x128.png' },
  { size: 256, relpath: 'macos/icon_128x128@2x.png' },
  { size: 256, relpath: 'macos/icon_256x256.png' },
  { size: 512, relpath: 'macos/icon_256x256@2x.png' },
  { size: 512, relpath: 'macos/icon_512x512.png' },
  { size: 1024, relpath: 'macos/icon_512x512@2x.png' },
]

const ICON_PRESET_ENTRIES = {
  web: WEB_ICONS,
  ios: IOS_ICONS,
  android: ANDROID_ICONS,
  macos: MACOS_ICONS,
  full: [...WEB_ICONS, ...IOS_ICONS, ...ANDROID_ICONS, ...MACOS_ICONS],
} as const

export type IconPresetId = keyof typeof ICON_PRESET_ENTRIES

type TranslateFn = (key: string, params?: Record<string, unknown>) => string

export function getIconPresetOptions(t: TranslateFn) {
  return {
    web: {
      label: t('rightPanel.export.presets.web.label'),
      desc: t('rightPanel.export.presets.web.desc'),
      entries: WEB_ICONS,
    },
    ios: {
      label: t('rightPanel.export.presets.ios.label'),
      desc: t('rightPanel.export.presets.ios.desc'),
      entries: IOS_ICONS,
    },
    android: {
      label: t('rightPanel.export.presets.android.label'),
      desc: t('rightPanel.export.presets.android.desc'),
      entries: ANDROID_ICONS,
    },
    macos: {
      label: t('rightPanel.export.presets.macos.label'),
      desc: t('rightPanel.export.presets.macos.desc'),
      entries: MACOS_ICONS,
    },
    full: {
      label: t('rightPanel.export.presets.full.label'),
      desc: t('rightPanel.export.presets.full.desc'),
      entries: ICON_PRESET_ENTRIES.full,
    },
  } as const
}

export function useExport() {
  const canvasStore = useCanvasStore()
  const bgStore = useBackgroundStore()
  const typoStore = useTypographyStore()
  const appStore = useAppStore()
  const { render } = useCanvasRenderer()
  const isExporting = ref(false)
  const lastExportMsg = ref('')
  const isExportError = ref(false)

  function tr(key: string, params?: Record<string, unknown>) {
    return params ? i18n.global.t(key, params) : i18n.global.t(key)
  }

  async function renderAtSize(targetSize: number): Promise<string> {
    const exportCanvas = document.createElement('canvas')
    exportCanvas.width = targetSize
    exportCanvas.height = targetSize

    const scale = targetSize / canvasStore.canvasWidth
    const scaledCanvas = createScaledCanvasStore(canvasStore, scale)
    const scaledTypo = createScaledTypoStore(typoStore, scale)

    render(exportCanvas, scaledCanvas as never, bgStore, scaledTypo as never)
    await new Promise((resolve) => setTimeout(resolve, 400))
    render(exportCanvas, scaledCanvas as never, bgStore, scaledTypo as never)
    await new Promise((resolve) => setTimeout(resolve, 100))

    return exportCanvas.toDataURL('image/png', 1)
  }

  async function exportOriginalSize() {
    if (isExporting.value) return
    const layer = canvasStore.imageLayer
    if (!layer) return
    const targetSize = Math.max(layer.naturalWidth, layer.naturalHeight)
    isExporting.value = true
    isExportError.value = false
    appStore.setLoading(true, tr('exportModule.loading.exportOriginal', { size: targetSize }))

    try {
      const savePath = await save({
        title: tr('exportModule.dialog.savePngTitle'),
        defaultPath: `logo-original-${targetSize}px.png`,
        filters: [{ name: tr('exportModule.dialog.pngFilterName'), extensions: ['png'] }],
      })
      if (!savePath) return

      const dataUrl = await renderAtSize(targetSize)
      await invoke('save_image', { dataUrl, path: savePath })
      lastExportMsg.value = tr('exportModule.status.saved', { path: savePath })
    } catch (error) {
      console.error('Export error:', error)
      isExportError.value = true
      lastExportMsg.value = tr('exportModule.status.failed', { error: String(error) })
    } finally {
      isExporting.value = false
      appStore.setLoading(false)
    }
  }

  async function exportPng(targetSize = 1024) {
    if (isExporting.value) return
    isExporting.value = true
    isExportError.value = false
    appStore.setLoading(true, tr('exportModule.loading.exportPng', { size: targetSize }))

    try {
      const savePath = await save({
        title: tr('exportModule.dialog.savePngTitle'),
        defaultPath: `logo-${targetSize}px.png`,
        filters: [{ name: tr('exportModule.dialog.pngFilterName'), extensions: ['png'] }],
      })
      if (!savePath) return

      const dataUrl = await renderAtSize(targetSize)
      await invoke('save_image', { dataUrl, path: savePath })
      lastExportMsg.value = tr('exportModule.status.saved', { path: savePath })
    } catch (error) {
      console.error('Export error:', error)
      isExportError.value = true
      lastExportMsg.value = tr('exportModule.status.failed', { error: String(error) })
    } finally {
      isExporting.value = false
      appStore.setLoading(false)
    }
  }

  async function exportIconSet(presetId: IconPresetId) {
    if (isExporting.value) return
    isExporting.value = true
    isExportError.value = false

    const preset = getIconPresetOptions(i18n.global.t)[presetId]
    appStore.setLoading(true, tr('exportModule.loading.generateSet', { label: preset.label }))

    try {
      const dir = await open({
        title: tr('exportModule.dialog.chooseFolder', { label: preset.label }),
        directory: true,
        multiple: false,
      })
      if (!dir || typeof dir !== 'string') return

      appStore.setLoading(true, tr('exportModule.loading.rendering'))
      const dataUrl = await renderAtSize(1024)

      appStore.setLoading(true, tr('exportModule.loading.generateFiles', { count: preset.entries.length }))
      const count = await invoke<number>('export_icon_set', {
        dataUrl,
        outputDir: dir,
        entries: preset.entries,
      })

      lastExportMsg.value = tr('exportModule.status.generated', { count, dir })
    } catch (error) {
      console.error('Icon set export error:', error)
      isExportError.value = true
      lastExportMsg.value = tr('exportModule.status.failed', { error: String(error) })
    } finally {
      isExporting.value = false
      appStore.setLoading(false)
    }
  }

  return {
    exportPng,
    exportOriginalSize,
    exportIconSet,
    isExporting,
    lastExportMsg,
    isExportError: computed(() => isExportError.value),
  }
}

function createScaledCanvasStore(
  store: ReturnType<typeof useCanvasStore>,
  scale: number,
) {
  if (!store.imageLayer) return store
  return {
    ...store,
    canvasWidth: store.canvasWidth * scale,
    canvasHeight: store.canvasHeight * scale,
    zoom: 1,
    panX: 0,
    panY: 0,
    imageLayer: {
      ...store.imageLayer,
      x: store.imageLayer.x * scale,
      y: store.imageLayer.y * scale,
      width: store.imageLayer.width * scale,
      height: store.imageLayer.height * scale,
    },
    segMode: 'none',
    segPoints: [],
  }
}

function createScaledTypoStore(
  store: ReturnType<typeof useTypographyStore>,
  scale: number,
) {
  return {
    ...store,
    textLayers: store.textLayers.map((layer) => ({
      ...layer,
      x: layer.x * scale,
      y: layer.y * scale,
      fontSize: layer.fontSize * scale,
      letterSpacing: layer.letterSpacing * scale,
    })),
  }
}

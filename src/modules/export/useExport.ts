import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { save, open } from '@tauri-apps/plugin-dialog'
import { useCanvasStore } from '../../store/useCanvasStore'
import { useBackgroundStore } from '../../store/useBackgroundStore'
import { useTypographyStore } from '../../store/useTypographyStore'
import { useAppStore } from '../../store/useAppStore'
import { useCanvasRenderer } from '../image-editor/useImageEditor'

// ── Icon set presets ──────────────────────────────────────────────────────────

interface IconEntry { size: number; relpath: string }

const WEB_ICONS: IconEntry[] = [
  { size: 16,  relpath: 'favicon-16x16.png' },
  { size: 32,  relpath: 'favicon-32x32.png' },
  { size: 48,  relpath: 'favicon-48x48.png' },
  { size: 64,  relpath: 'icon-64x64.png' },
  { size: 96,  relpath: 'icon-96x96.png' },
  { size: 128, relpath: 'icon-128x128.png' },
  { size: 192, relpath: 'icon-192x192.png' },
  { size: 256, relpath: 'icon-256x256.png' },
  { size: 384, relpath: 'icon-384x384.png' },
  { size: 512, relpath: 'icon-512x512.png' },
]

const IOS_ICONS: IconEntry[] = [
  { size: 20,   relpath: 'ios/AppIcon-20@1x.png' },
  { size: 40,   relpath: 'ios/AppIcon-20@2x.png' },
  { size: 60,   relpath: 'ios/AppIcon-20@3x.png' },
  { size: 29,   relpath: 'ios/AppIcon-29@1x.png' },
  { size: 58,   relpath: 'ios/AppIcon-29@2x.png' },
  { size: 87,   relpath: 'ios/AppIcon-29@3x.png' },
  { size: 40,   relpath: 'ios/AppIcon-40@1x.png' },
  { size: 80,   relpath: 'ios/AppIcon-40@2x.png' },
  { size: 120,  relpath: 'ios/AppIcon-40@3x.png' },
  { size: 60,   relpath: 'ios/AppIcon-60@1x.png' },
  { size: 120,  relpath: 'ios/AppIcon-60@2x.png' },
  { size: 180,  relpath: 'ios/AppIcon-60@3x.png' },
  { size: 76,   relpath: 'ios/AppIcon-76@1x.png' },
  { size: 152,  relpath: 'ios/AppIcon-76@2x.png' },
  { size: 167,  relpath: 'ios/AppIcon-83.5@2x.png' },
  { size: 1024, relpath: 'ios/AppIcon-1024x1024.png' },
]

const ANDROID_ICONS: IconEntry[] = [
  { size: 36,  relpath: 'android/mipmap-ldpi/ic_launcher.png' },
  { size: 48,  relpath: 'android/mipmap-mdpi/ic_launcher.png' },
  { size: 72,  relpath: 'android/mipmap-hdpi/ic_launcher.png' },
  { size: 96,  relpath: 'android/mipmap-xhdpi/ic_launcher.png' },
  { size: 144, relpath: 'android/mipmap-xxhdpi/ic_launcher.png' },
  { size: 192, relpath: 'android/mipmap-xxxhdpi/ic_launcher.png' },
  { size: 512, relpath: 'android/ic_launcher-playstore.png' },
]

const MACOS_ICONS: IconEntry[] = [
  { size: 16,   relpath: 'macos/icon_16x16.png' },
  { size: 32,   relpath: 'macos/icon_16x16@2x.png' },
  { size: 32,   relpath: 'macos/icon_32x32.png' },
  { size: 64,   relpath: 'macos/icon_32x32@2x.png' },
  { size: 128,  relpath: 'macos/icon_128x128.png' },
  { size: 256,  relpath: 'macos/icon_128x128@2x.png' },
  { size: 256,  relpath: 'macos/icon_256x256.png' },
  { size: 512,  relpath: 'macos/icon_256x256@2x.png' },
  { size: 512,  relpath: 'macos/icon_512x512.png' },
  { size: 1024, relpath: 'macos/icon_512x512@2x.png' },
]

export const ICON_PRESETS = {
  web:     { label: 'Web 图标',    desc: '10 种尺寸 (16–512px)',            entries: WEB_ICONS },
  ios:     { label: 'iOS',         desc: '16 种规格 (iPhone + iPad)',        entries: IOS_ICONS },
  android: { label: 'Android',     desc: '7 种规格 (LDPI–XXXHDPI)',          entries: ANDROID_ICONS },
  macos:   { label: 'macOS',       desc: '10 种规格 (含 @2x)',               entries: MACOS_ICONS },
  full:    { label: '完整套装',     desc: '全平台，共 43 种规格',
             entries: [...WEB_ICONS, ...IOS_ICONS, ...ANDROID_ICONS, ...MACOS_ICONS] },
} as const

export type IconPresetId = keyof typeof ICON_PRESETS

// ── Composable ────────────────────────────────────────────────────────────────

export function useExport() {
  const canvasStore = useCanvasStore()
  const bgStore = useBackgroundStore()
  const typoStore = useTypographyStore()
  const appStore = useAppStore()
  const { render } = useCanvasRenderer()
  const isExporting = ref(false)
  const lastExportMsg = ref('')

  /** Render canvas at a target size and return base64 PNG data URL */
  async function renderAtSize(targetSize: number): Promise<string> {
    const exportCanvas = document.createElement('canvas')
    exportCanvas.width = targetSize
    exportCanvas.height = targetSize

    const scale = targetSize / canvasStore.canvasWidth
    const scaledCanvas = createScaledCanvasStore(canvasStore, scale)
    const scaledTypo = createScaledTypoStore(typoStore, scale)

    render(exportCanvas, scaledCanvas as any, bgStore, scaledTypo as any)
    await new Promise(r => setTimeout(r, 400))
    render(exportCanvas, scaledCanvas as any, bgStore, scaledTypo as any)
    await new Promise(r => setTimeout(r, 100))

    return exportCanvas.toDataURL('image/png', 1)
  }

  /** Export a single PNG — opens native save dialog */
  async function exportPng(targetSize: number = 1024) {
    if (isExporting.value) return
    isExporting.value = true
    appStore.setLoading(true, `导出 ${targetSize}px PNG…`)

    try {
      // Open native save dialog
      const savePath = await save({
        title: '保存 PNG',
        defaultPath: `logo-${targetSize}px.png`,
        filters: [{ name: 'PNG 图像', extensions: ['png'] }],
      })
      if (!savePath) return  // user cancelled

      const dataUrl = await renderAtSize(targetSize)
      await invoke('save_image', { dataUrl, path: savePath })
      lastExportMsg.value = `已保存：${savePath}`
    } catch (e) {
      console.error('Export error:', e)
      lastExportMsg.value = `导出失败：${e}`
    } finally {
      isExporting.value = false
      appStore.setLoading(false)
    }
  }

  /** Generate an icon set — renders at 1024px, then resizes via Rust */
  async function exportIconSet(presetId: IconPresetId) {
    if (isExporting.value) return
    isExporting.value = true

    const preset = ICON_PRESETS[presetId]
    appStore.setLoading(true, `生成 ${preset.label} 图标集…`)

    try {
      // Open folder picker
      const dir = await open({
        title: `选择 ${preset.label} 输出文件夹`,
        directory: true,
        multiple: false,
      })
      if (!dir || typeof dir !== 'string') return  // user cancelled

      // Render at 1024px (source for all resizes)
      appStore.setLoading(true, '渲染高清图…')
      const dataUrl = await renderAtSize(1024)

      appStore.setLoading(true, `生成 ${preset.entries.length} 个图标文件…`)
      const count = await invoke<number>('export_icon_set', {
        dataUrl,
        outputDir: dir,
        entries: preset.entries,
      })

      lastExportMsg.value = `✓ 已生成 ${count} 个图标文件至：${dir}`
    } catch (e) {
      console.error('Icon set export error:', e)
      lastExportMsg.value = `导出失败：${e}`
    } finally {
      isExporting.value = false
      appStore.setLoading(false)
    }
  }

  return { exportPng, exportIconSet, isExporting, lastExportMsg }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

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
    textLayers: store.textLayers.map(l => ({
      ...l,
      x: l.x * scale,
      y: l.y * scale,
      fontSize: l.fontSize * scale,
      letterSpacing: l.letterSpacing * scale,
    })),
  }
}

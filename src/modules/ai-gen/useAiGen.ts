/**
 * AI Logo Generation composable
 *
 * Wraps the four Tauri commands (ai_gen_*) and the two backend events:
 *   "ai-gen:download-progress"  — per-file download progress
 *   "ai-gen:step-progress"      — per-denoising-step progress
 *
 * Usage:
 *   const aiGen = useAiGen()
 *   await aiGen.loadModels()
 *   await aiGen.downloadModel('sd15-logo-redmond')
 *   await aiGen.generate({ modelId: '...', prompt: '...' })
 */

import { ref, computed, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useAppStore } from '../../store/useAppStore'
import { useImageEditor } from '../image-editor/useImageEditor'

// ── Types matching the Rust side ──────────────────────────────────────────────

export interface DeviceInfo {
  kind: 'cpu' | 'cuda'
  name: string
  vramMb: number | null
  isAccelerated: boolean
}

export interface ModelStatus {
  id: string
  name: string
  description: string
  base: 'sd15' | 'sdxl'
  sizeMb: number
  minRamMb: number
  defaultStepsCpu: number
  defaultStepsGpu: number
  defaultGuidance: number
  maxResolution: number
  examplePrompt: string
  hasLora: boolean
  loraTriggerWord: string | null
  isDownloaded: boolean
}

export interface DownloadProgress {
  modelId: string
  fileName: string
  bytesDone: number
  bytesTotal: number
  percent: number
}

export interface StepProgress {
  modelId: string
  step: number
  totalSteps: number
}

export interface GenerateOptions {
  modelId: string
  prompt: string
  negativePrompt?: string
  steps?: number
  guidance?: number
  width?: number
  height?: number
  seed?: number | null
}

// ── Composable ────────────────────────────────────────────────────────────────

export function useAiGen() {
  const appStore = useAppStore()
  const { importImageFromDataUrl } = useImageEditor()

  // State
  const device = ref<DeviceInfo | null>(null)
  const models = ref<ModelStatus[]>([])
  const selectedModelId = ref<string | null>(null)

  // Download state
  const downloadingModelId = ref<string | null>(null)
  const downloadProgress = ref<DownloadProgress | null>(null)

  // Generation state
  const isGenerating = ref(false)
  const generatingModelId = ref<string | null>(null)
  const stepProgress = ref<StepProgress | null>(null)

  // Event listener cleanup handles
  const unlisteners: Array<() => void> = []

  // ── Initialisation ──────────────────────────────────────────────────────────

  async function init() {
    await Promise.all([fetchDevice(), loadModels()])
    setupEventListeners()
  }

  async function fetchDevice() {
    try {
      device.value = await invoke<DeviceInfo>('ai_gen_device_info')
    } catch (e) {
      console.error('[AI-GEN] device info error:', e)
    }
  }

  async function loadModels() {
    try {
      models.value = await invoke<ModelStatus[]>('ai_gen_list_models')
      // Auto-select the first downloaded model, or the first model
      if (!selectedModelId.value) {
        const first = models.value.find(m => m.isDownloaded) ?? models.value[0]
        if (first) selectedModelId.value = first.id
      }
    } catch (e) {
      console.error('[AI-GEN] list models error:', e)
    }
  }

  function setupEventListeners() {
    listen<DownloadProgress>('ai-gen:download-progress', ({ payload }) => {
      downloadProgress.value = payload
      // Update isDownloaded flag reactively when a model finishes
      if (payload.percent >= 100) {
        refreshModel(payload.modelId)
      }
    }).then(unlisten => unlisteners.push(unlisten))

    listen<StepProgress>('ai-gen:step-progress', ({ payload }) => {
      stepProgress.value = payload
    }).then(unlisten => unlisteners.push(unlisten))
  }

  async function refreshModel(modelId: string) {
    try {
      const fresh = await invoke<ModelStatus[]>('ai_gen_list_models')
      const idx = models.value.findIndex(m => m.id === modelId)
      if (idx !== -1) {
        const updated = fresh.find(m => m.id === modelId)
        if (updated) models.value[idx] = updated
      }
    } catch { /* ignore */ }
  }

  onUnmounted(() => {
    unlisteners.forEach(fn => fn())
  })

  // ── Actions ─────────────────────────────────────────────────────────────────

  async function downloadModel(modelId: string) {
    if (downloadingModelId.value) return

    downloadingModelId.value = modelId
    downloadProgress.value = null

    try {
      await invoke('ai_gen_download', { modelId })
      await refreshModel(modelId)
      appStore.showToast('模型下载完成', 'info')
    } catch (e: any) {
      appStore.showToast(`下载失败: ${e}`, 'error')
    } finally {
      downloadingModelId.value = null
      downloadProgress.value = null
    }
  }

  async function generate(opts: GenerateOptions) {
    if (isGenerating.value) return

    isGenerating.value = true
    generatingModelId.value = opts.modelId
    stepProgress.value = null

    try {
      const result = await invoke<{
        success: boolean
        image: string | null
        error: string | null
        modelId: string
        deviceKind: string
        stepsTaken: number
      }>('ai_gen_generate', {
        modelId: opts.modelId,
        prompt: opts.prompt,
        negativePrompt: opts.negativePrompt ?? null,
        steps: opts.steps ?? null,
        guidance: opts.guidance ?? null,
        width: opts.width ?? null,
        height: opts.height ?? null,
        seed: opts.seed ?? null,
      })

      if (!result.success || !result.image) {
        throw new Error(result.error ?? '生成失败')
      }

      // Load the PNG into the canvas as a data URL
      const dataUrl = `data:image/png;base64,${result.image}`
      await importImageFromDataUrl(dataUrl)

      appStore.showToast(`生成完成（${result.stepsTaken} 步 · ${result.deviceKind.toUpperCase()}）`, 'info')
    } catch (e: any) {
      appStore.showToast(`生成失败: ${e}`, 'error')
    } finally {
      isGenerating.value = false
      generatingModelId.value = null
      stepProgress.value = null
    }
  }

  // ── Derived ─────────────────────────────────────────────────────────────────

  const selectedModel = computed(() =>
    models.value.find(m => m.id === selectedModelId.value) ?? null
  )

  const isDownloading = computed(() => downloadingModelId.value !== null)

  const downloadPercentForModel = computed(() => (modelId: string) => {
    if (downloadingModelId.value !== modelId) return null
    return downloadProgress.value?.percent ?? 0
  })

  const stepPercentage = computed(() => {
    if (!stepProgress.value) return 0
    const { step, totalSteps } = stepProgress.value
    return totalSteps > 0 ? Math.round((step / totalSteps) * 100) : 0
  })

  return {
    // State
    device,
    models,
    selectedModelId,
    selectedModel,
    downloadingModelId,
    downloadProgress,
    isDownloading,
    downloadPercentForModel,
    isGenerating,
    generatingModelId,
    stepProgress,
    stepPercentage,
    // Actions
    init,
    loadModels,
    downloadModel,
    generate,
  }
}

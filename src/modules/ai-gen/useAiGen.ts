/**
 * AI Logo Generation composable
 *
 * Tauri commands wrapped:
 *   ai_gen_device_info, ai_gen_list_models
 *   ai_gen_download      → emits "ai-gen:download-progress" events
 *   ai_gen_generate      → emits "ai-gen:step-progress" events
 *   ai_gen_get_hf_token, ai_gen_set_hf_token, ai_gen_delete_hf_token
 *
 * Download results carry a structured errorKind so the UI can show the
 * "Need HF Token" panel when the response is auth_required.
 */

import { ref, computed, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { openUrl } from '@tauri-apps/plugin-opener'
import { useAppStore } from '../../store/useAppStore'
import { useImageEditor } from '../image-editor/useImageEditor'

// ── Types ─────────────────────────────────────────────────────────────────────

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
  requiresToken: boolean
  isDownloaded: boolean
}

export interface DownloadResult {
  success: boolean
  errorKind: 'auth_required' | 'not_found' | 'error' | null
  errorMessage: string | null
  errorUrl: string | null
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

export interface HfTokenStatus {
  hasToken: boolean
  masked: string | null
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

  // Model & device state
  const device = ref<DeviceInfo | null>(null)
  const models = ref<ModelStatus[]>([])
  const selectedModelId = ref<string | null>(null)

  // Download state
  const downloadingModelId = ref<string | null>(null)
  const downloadProgress = ref<DownloadProgress | null>(null)
  /** Set when a download fails with auth_required — triggers the token panel */
  const authRequiredModelId = ref<string | null>(null)

  // Generation state
  const isGenerating = ref(false)
  const stepProgress = ref<StepProgress | null>(null)

  // HF Token state
  const hfTokenStatus = ref<HfTokenStatus>({ hasToken: false, masked: null })
  const showTokenPanel = ref(false)

  const unlisteners: Array<() => void> = []

  // ── Init ────────────────────────────────────────────────────────────────────

  async function init() {
    await Promise.all([fetchDevice(), loadModels(), fetchTokenStatus()])
    setupEventListeners()
  }

  async function fetchDevice() {
    try {
      device.value = await invoke<DeviceInfo>('ai_gen_device_info')
    } catch (e) {
      console.error('[AI-GEN] device info:', e)
    }
  }

  async function loadModels() {
    try {
      models.value = await invoke<ModelStatus[]>('ai_gen_list_models')
      if (!selectedModelId.value) {
        const first = models.value.find(m => m.isDownloaded) ?? models.value[0]
        if (first) selectedModelId.value = first.id
      }
    } catch (e) {
      console.error('[AI-GEN] list models:', e)
    }
  }

  async function fetchTokenStatus() {
    try {
      hfTokenStatus.value = await invoke<HfTokenStatus>('ai_gen_get_hf_token')
    } catch (e) {
      console.error('[AI-GEN] token status:', e)
    }
  }

  function setupEventListeners() {
    listen<DownloadProgress>('ai-gen:download-progress', ({ payload }) => {
      downloadProgress.value = payload
      if (payload.percent >= 100) refreshModel(payload.modelId)
    }).then(u => unlisteners.push(u))

    listen<StepProgress>('ai-gen:step-progress', ({ payload }) => {
      stepProgress.value = payload
    }).then(u => unlisteners.push(u))
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

  onUnmounted(() => unlisteners.forEach(fn => fn()))

  // ── Actions ─────────────────────────────────────────────────────────────────

  async function downloadModel(modelId: string) {
    if (downloadingModelId.value) return
    authRequiredModelId.value = null
    downloadingModelId.value = modelId
    downloadProgress.value = null

    try {
      const result = await invoke<DownloadResult>('ai_gen_download', { modelId })

      if (result.success) {
        await refreshModel(modelId)
        appStore.showToast('模型下载完成', 'info')
        return
      }

      if (result.errorKind === 'auth_required') {
        authRequiredModelId.value = modelId
        showTokenPanel.value = true
        appStore.showToast('需要 HuggingFace Token，请在设置中填写', 'warn')
      } else {
        appStore.showToast(result.errorMessage ?? '下载失败', 'error')
      }
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

      if (!result.success || !result.image) throw new Error(result.error ?? '生成失败')

      await importImageFromDataUrl(`data:image/png;base64,${result.image}`)
      appStore.showToast(`生成完成（${result.stepsTaken} 步 · ${result.deviceKind.toUpperCase()}）`, 'info')
    } catch (e: any) {
      appStore.showToast(`生成失败: ${e}`, 'error')
    } finally {
      isGenerating.value = false
      stepProgress.value = null
    }
  }

  // ── HF Token actions ────────────────────────────────────────────────────────

  async function saveToken(token: string) {
    try {
      await invoke('ai_gen_set_hf_token', { token })
      await fetchTokenStatus()
      appStore.showToast('Token 已保存', 'info')
      showTokenPanel.value = false
      // Retry the download that triggered auth if applicable
      if (authRequiredModelId.value) {
        const mid = authRequiredModelId.value
        authRequiredModelId.value = null
        await downloadModel(mid)
      }
    } catch (e: any) {
      appStore.showToast(`保存失败: ${e}`, 'error')
    }
  }

  async function deleteToken() {
    try {
      await invoke('ai_gen_delete_hf_token')
      await fetchTokenStatus()
      appStore.showToast('Token 已删除', 'info')
    } catch (e: any) {
      appStore.showToast(`删除失败: ${e}`, 'error')
    }
  }

  /** Open the HuggingFace token settings page in the system browser. */
  function openHfTokenPage() {
    openUrl('https://huggingface.co/settings/tokens').catch(console.error)
  }

  // ── Derived ─────────────────────────────────────────────────────────────────

  const selectedModel = computed(() =>
    models.value.find(m => m.id === selectedModelId.value) ?? null
  )

  const isDownloading = computed(() => downloadingModelId.value !== null)

  const stepPercentage = computed(() => {
    if (!stepProgress.value) return 0
    const { step, totalSteps } = stepProgress.value
    return totalSteps > 0 ? Math.round((step / totalSteps) * 100) : 0
  })

  return {
    device, models, selectedModelId, selectedModel,
    downloadingModelId, downloadProgress, isDownloading,
    authRequiredModelId,
    isGenerating, stepProgress, stepPercentage,
    hfTokenStatus, showTokenPanel,
    // Actions
    init, loadModels,
    downloadModel, generate,
    saveToken, deleteToken, openHfTokenPage,
  }
}

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAiGen } from '../../modules/ai-gen/useAiGen'
import { getGenerationProgressText } from '../../modules/ai-gen/progressText'
import { useAppStore } from '../../store/useAppStore'

const aiGen = useAiGen()
const { t } = useI18n()
const appStore = useAppStore()

// Local form state
const prompt = ref('')
const negativePrompt = ref('blurry, low quality, watermark, ugly, deformed, text, signature')
const showAdvanced = ref(false)
const customSteps = ref<number | null>(null)
const customGuidance = ref<number | null>(null)
// Default 256×256 — fast enough for CPU; user can raise it in advanced settings
const customWidth = ref<number>(256)
const customHeight = ref<number>(256)
const seed = ref<number | null>(null)
const randomSeed = ref(true)
// Sampler
const sampler = ref<'ddim' | 'euler_a' | 'dpm_pp_2m_karras'>('dpm_pp_2m_karras')
// Hires Fix
const hiresFixEnabled = ref(false)
const hiresFixWidth = ref<number>(512)
const hiresFixHeight = ref<number>(512)
const hiresFixStrength = ref<number>(0.5)
const hiresFixSteps = ref<number>(20)

// Token input state
const tokenInput = ref('')
const savingToken = ref(false)

onMounted(() => aiGen.init())

watch(aiGen.selectedModel, (m) => {
  if (!prompt.value && m) prompt.value = ''
})

const canGenerate = computed(() =>
  !!aiGen.selectedModel.value?.isDownloaded &&
  prompt.value.trim().length > 0 &&
  !aiGen.isGenerating.value &&
  !aiGen.isDownloading.value
)

const effectiveSteps = computed(() => {
  if (customSteps.value) return customSteps.value
  const m = aiGen.selectedModel.value
  if (!m) return 20
  return aiGen.device.value?.isAccelerated ? m.defaultStepsGpu : m.defaultStepsCpu
})

const effectiveGuidance = computed(() =>
  customGuidance.value ?? aiGen.selectedModel.value?.defaultGuidance ?? 7.5
)

const resolutionOptions = computed(() => {
  const base = aiGen.selectedModel.value?.base
  // 256 is the fast default; 512 is SD-native quality; SDXL can go to 1024
  if (base === 'sdxl') return [256, 512, 1024]
  return [256, 512, 768]
})

// customWidth/Height always have a value (default 256), so no fallback needed
const effectiveWidth = computed(() => customWidth.value)
const effectiveHeight = computed(() => customHeight.value)
const generationProgressText = computed(() =>
  getGenerationProgressText(
    t,
    aiGen.stepProgress.value,
    aiGen.generationStatus.value,
    effectiveSteps.value,
  ),
)

function formatBytes(b: number): string {
  if (b >= 1_073_741_824) return `${(b / 1_073_741_824).toFixed(1)} GB`
  if (b >= 1_048_576) return `${(b / 1_048_576).toFixed(0)} MB`
  return `${(b / 1024).toFixed(0)} KB`
}

async function copyGpuCommand() {
  try {
    await navigator.clipboard.writeText('npm run tauri:dev:gpu')
    appStore.showToast(t('aiGen.device.gpuCmdCopied'), 'info')
  } catch { /* clipboard not available */ }
}

async function handleSaveToken() {
  if (!tokenInput.value.trim()) return
  savingToken.value = true
  await aiGen.saveToken(tokenInput.value.trim())
  tokenInput.value = ''
  savingToken.value = false
}

async function handleGenerate() {
  if (!canGenerate.value || !aiGen.selectedModel.value) return
  await aiGen.generate({
    modelId: aiGen.selectedModel.value.id,
    prompt: prompt.value,
    negativePrompt: negativePrompt.value || undefined,
    steps: customSteps.value ?? undefined,
    guidance: customGuidance.value ?? undefined,
    width: customWidth.value,
    height: customHeight.value,
    seed: randomSeed.value ? null : seed.value,
    sampler: sampler.value,
    hiresFixEnabled: hiresFixEnabled.value,
    hiresFixWidth: hiresFixEnabled.value ? hiresFixWidth.value : undefined,
    hiresFixHeight: hiresFixEnabled.value ? hiresFixHeight.value : undefined,
    hiresFixStrength: hiresFixEnabled.value ? hiresFixStrength.value : undefined,
    hiresFixSteps: hiresFixEnabled.value ? hiresFixSteps.value : undefined,
  })
}

// Watch base resolution: keep hires defaults at 2× when user changes base
watch([customWidth, customHeight], ([w, h]) => {
  const model = aiGen.selectedModel.value
  const maxRes = model?.maxResolution ?? 1024
  hiresFixWidth.value  = Math.min(w * 2, maxRes)
  hiresFixHeight.value = Math.min(h * 2, maxRes)
})

function clampSteps(val: string) {
  const n = parseInt(val, 10)
  if (isNaN(n) || n < 1) return
  customSteps.value = n  // no upper limit
}

function clampGuidance(val: string) {
  const n = parseFloat(val)
  if (isNaN(n) || n < 0) return
  customGuidance.value = n  // no upper limit; 0 = disabled (SDXL Turbo)
}

function handleSelectModel(id: string) {
  if (!aiGen.isGenerating.value && !aiGen.isDownloading.value) {
    aiGen.selectedModelId.value = id
  }
}
</script>

<template>
  <div class="ai-panel">

    <!-- ── Device badge ──────────────────────────────────────────────── -->
    <div class="device-row">
      <div v-if="aiGen.device.value" class="device-badge" :class="aiGen.device.value.kind">
        <span class="device-badge__dot" />
        <span>{{ aiGen.device.value.isAccelerated ? 'GPU · ' : 'CPU · ' }}{{ aiGen.device.value.name }}</span>
      </div>
      <!-- GPU present but feature not compiled in -->
      <div
        v-if="aiGen.device.value?.gpuAvailableButDisabled"
        class="gpu-hint-badge"
        :title="t('aiGen.device.gpuHintDetail')"
        @click="copyGpuCommand"
      >
        <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>
        </svg>
        {{ t('aiGen.device.gpuHint') }}
        <code class="gpu-hint-cmd">npm run tauri:dev:gpu</code>
      </div>
    </div>

    <!-- ── HF Token panel ─────────────────────────────────────────────── -->
    <!-- Always visible as a collapsible settings section -->
    <div class="token-section">
      <div class="token-header" @click="aiGen.showTokenPanel.value = !aiGen.showTokenPanel.value">
        <div class="token-header__left">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
            <path d="M7 11V7a5 5 0 0110 0v4"/>
          </svg>
          <span>{{ t('aiGen.token.title') }}</span>
          <span v-if="aiGen.hfTokenStatus.value.hasToken" class="token-badge token-badge--set">{{ t('aiGen.token.set') }}</span>
          <span v-else class="token-badge token-badge--unset">{{ t('aiGen.token.unset') }}</span>
        </div>
        <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
          :style="{ transform: aiGen.showTokenPanel.value ? 'rotate(180deg)' : 'none', transition: 'transform 0.2s' }">
          <polyline points="6 9 12 15 18 9"/>
        </svg>
      </div>

      <!-- Auth-required alert (shown automatically after 401/403 error) -->
      <div v-if="aiGen.authRequiredModelId.value && !aiGen.showTokenPanel.value" class="auth-alert" @click="aiGen.showTokenPanel.value = true">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/>
        </svg>
        <span>{{ t('aiGen.token.authRequired') }}</span>
      </div>

      <Transition name="expand">
        <div v-if="aiGen.showTokenPanel.value" class="token-body">
          <!-- Current token display -->
          <div v-if="aiGen.hfTokenStatus.value.hasToken" class="token-current">
            <code class="token-masked">{{ aiGen.hfTokenStatus.value.masked }}</code>
            <button class="btn-token-del" @click="aiGen.deleteToken()" :title="t('aiGen.token.delete')">
              <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14H6L5 6"/>
              </svg>
            </button>
          </div>

          <p class="token-help">
            {{ t('aiGen.token.help') }}
            <button class="link-btn" @click="aiGen.openHfTokenPage()">
              {{ t('aiGen.token.getLink') }}
            </button>
          </p>

          <!-- Token input -->
          <div class="token-input-row">
            <input
              v-model="tokenInput"
              type="password"
              class="token-input"
              placeholder="hf_••••••••••••••••••••"
              autocomplete="off"
              @keydown.enter="handleSaveToken"
            />
            <button
              class="btn-token-save"
              :disabled="!tokenInput.trim() || savingToken"
              @click="handleSaveToken"
            >
              {{ savingToken ? t('aiGen.token.saving') : t('aiGen.token.save') }}
            </button>
          </div>
        </div>
      </Transition>
    </div>

    <!-- ── Model selector ────────────────────────────────────────────── -->
    <div class="section-label">{{ t('aiGen.sections.models') }}</div>
    <div class="model-list">
      <div
        v-for="model in aiGen.models.value"
        :key="model.id"
        class="model-card"
        :class="{
          'model-card--selected': aiGen.selectedModelId.value === model.id,
          'model-card--disabled': aiGen.isGenerating.value || (aiGen.isDownloading.value && aiGen.downloadingModelId.value !== model.id),
        }"
        @click="handleSelectModel(model.id)"
      >
        <!-- Header row -->
        <div class="model-card__head">
          <div class="model-card__name-row">
            <span class="model-card__name">{{ model.name }}</span>
            <span class="model-card__tag">{{ model.base === 'sd15' ? 'SD 1.5' : 'SDXL' }}</span>
          </div>
          <!-- Status indicator -->
          <span v-if="model.isDownloaded" class="status-dot status-dot--ready" :title="t('aiGen.models.downloaded')" />
          <span v-else class="status-dot status-dot--pending" :title="t('aiGen.models.notDownloaded')" />
        </div>

        <p class="model-card__desc">{{ model.description }}</p>

        <!-- Download progress (when downloading this model) -->
        <template v-if="aiGen.downloadingModelId.value === model.id">
          <div class="progress-wrap">
            <div class="progress-label">
              <span class="progress-file">{{ aiGen.downloadProgress.value?.fileName ?? t('common.preparing') }}</span>
              <span class="progress-pct">{{ Math.round(aiGen.downloadProgress.value?.percent ?? 0) }}%</span>
            </div>
            <div class="progress-bar">
              <div
                class="progress-bar__fill"
                :style="{ width: `${aiGen.downloadProgress.value?.percent ?? 0}%` }"
              />
            </div>
            <div v-if="aiGen.downloadProgress.value?.bytesTotal" class="progress-bytes">
              {{ formatBytes(aiGen.downloadProgress.value.bytesDone) }}
              / {{ formatBytes(aiGen.downloadProgress.value.bytesTotal) }}
            </div>
          </div>
        </template>

        <!-- Action button -->
        <div v-else class="model-card__footer">
          <button
            v-if="!model.isDownloaded"
            class="btn-model-action btn-model-action--download"
            :disabled="aiGen.isDownloading.value || aiGen.isGenerating.value"
            @click.stop="aiGen.downloadModel(model.id)"
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/>
              <polyline points="7 10 12 15 17 10"/>
              <line x1="12" y1="15" x2="12" y2="3"/>
            </svg>
            {{ t('aiGen.models.download', { size: (model.sizeMb / 1024).toFixed(1) }) }}
          </button>
          <div v-else class="model-card__ready">
            <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
              <polyline points="20 6 9 17 4 12"/>
            </svg>
            {{ t('aiGen.models.ready') }}
          </div>
        </div>
      </div>
    </div>

    <!-- ── Prompt ─────────────────────────────────────────────────────── -->
    <div class="section-label">{{ t('aiGen.sections.prompt') }}</div>
    <textarea
      v-model="prompt"
      class="prompt-input"
      :placeholder="aiGen.selectedModel.value?.examplePrompt ?? t('aiGen.promptPlaceholder')"
      :disabled="aiGen.isGenerating.value"
      rows="3"
    />

    <!-- ── Negative Prompt (always visible) ───────────────────────── -->
    <div class="section-label section-label--neg">
      <span>{{ t('aiGen.sections.negativePrompt') }}</span>
      <span class="label-hint">{{ t('aiGen.sections.negativePromptHint') }}</span>
    </div>
    <textarea
      v-model="negativePrompt"
      class="prompt-input prompt-input--neg"
      :placeholder="t('aiGen.negativePromptPlaceholder')"
      :disabled="aiGen.isGenerating.value"
      rows="2"
    />

    <!-- ── Advanced settings toggle ───────────────────────────────── -->
    <div class="advanced-toggle" @click="showAdvanced = !showAdvanced">
      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="12" cy="12" r="3"/><path d="M19.07 4.93a10 10 0 010 14.14M4.93 4.93a10 10 0 000 14.14"/>
      </svg>
      <span>{{ t('aiGen.sections.advanced') }}</span>
      <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
        :style="{ transform: showAdvanced ? 'rotate(180deg)' : 'none', transition: 'transform 0.2s' }">
        <polyline points="6 9 12 15 18 9"/>
      </svg>
    </div>

    <div v-if="showAdvanced" class="advanced-wrap">

      <!-- Sampler -->
      <div class="adv-row">
        <label class="adv-label">
          {{ t('aiGen.sections.sampler') }}
          <span class="adv-hint">{{ t('aiGen.sections.samplerHint') }}</span>
        </label>
        <div class="sampler-group">
          <button
            class="sampler-btn"
            :class="{ 'sampler-btn--active': sampler === 'dpm_pp_2m_karras' }"
            :disabled="aiGen.isGenerating.value"
            @click="sampler = 'dpm_pp_2m_karras'"
          >{{ t('aiGen.sampler.dpmPP2MKarras') }}</button>
          <button
            class="sampler-btn"
            :class="{ 'sampler-btn--active': sampler === 'ddim' }"
            :disabled="aiGen.isGenerating.value"
            @click="sampler = 'ddim'"
          >{{ t('aiGen.sampler.ddim') }}</button>
          <button
            class="sampler-btn"
            :class="{ 'sampler-btn--active': sampler === 'euler_a' }"
            :disabled="aiGen.isGenerating.value"
            @click="sampler = 'euler_a'"
          >{{ t('aiGen.sampler.eulerA') }}</button>
        </div>
      </div>

      <!-- Steps -->
      <div class="adv-row adv-row--inline">
        <label class="adv-label" style="flex:1;margin:0">
          {{ t('aiGen.sections.steps') }}
          <span class="adv-hint">{{ t('aiGen.sections.stepsHint') }}</span>
        </label>
        <input
          type="number"
          class="adv-num-input adv-num-input--wide"
          min="1"
          :value="effectiveSteps"
          :disabled="aiGen.isGenerating.value"
          @change="clampSteps(($event.target as HTMLInputElement).value)"
        />
      </div>

      <!-- CFG Scale (always visible) -->
      <div class="adv-row adv-row--inline">
        <label class="adv-label" style="flex:1;margin:0">
          {{ t('aiGen.sections.guidance') }}
          <span class="adv-hint">{{ t('aiGen.sections.guidanceHint') }}</span>
        </label>
        <input
          type="number"
          class="adv-num-input adv-num-input--wide"
          min="0" step="0.5"
          :value="effectiveGuidance"
          :disabled="aiGen.isGenerating.value"
          @change="clampGuidance(($event.target as HTMLInputElement).value)"
        />
      </div>

      <!-- Resolution -->
      <div class="adv-row adv-row--res">
        <label class="adv-label">
          {{ t('aiGen.sections.resolution') }}
        </label>
        <div class="adv-res-row">
          <!-- Quick presets -->
          <div class="res-presets">
            <button
              v-for="r in resolutionOptions"
              :key="r"
              class="res-preset-btn"
              :class="{ 'res-preset-btn--active': effectiveWidth === r && effectiveHeight === r }"
              :disabled="aiGen.isGenerating.value"
              @click="customWidth = r; customHeight = r"
            >{{ r }}</button>
          </div>
          <!-- W × H inputs -->
          <div class="res-inputs">
            <input
              type="number"
              class="adv-num-input adv-num-input--res"
              min="256" max="1024" step="64"
              :value="effectiveWidth"
              :disabled="aiGen.isGenerating.value"
              @change="customWidth = Math.round(Number(($event.target as HTMLInputElement).value) / 64) * 64"
            />
            <span class="res-x">×</span>
            <input
              type="number"
              class="adv-num-input adv-num-input--res"
              min="256" max="1024" step="64"
              :value="effectiveHeight"
              :disabled="aiGen.isGenerating.value"
              @change="customHeight = Math.round(Number(($event.target as HTMLInputElement).value) / 64) * 64"
            />
          </div>
        </div>
      </div>

      <!-- Seed -->
      <div class="adv-row">
        <label class="adv-label">
          {{ t('aiGen.sections.seed') }}
        </label>
        <div class="seed-row">
          <label class="toggle-label">
            <input type="checkbox" v-model="randomSeed" class="toggle-check" :disabled="aiGen.isGenerating.value" />
            <span>{{ t('common.random') }}</span>
          </label>
          <input
            v-if="!randomSeed"
            type="number"
            class="adv-num-input seed-num"
            v-model.number="seed"
            :placeholder="t('aiGen.seedPlaceholder')"
            :disabled="aiGen.isGenerating.value"
          />
        </div>
      </div>

      <!-- ── Hires Fix ──────────────────────────────────────────────── -->
      <div class="adv-row">
        <div class="hires-header">
          <label class="adv-label" style="margin:0">
            {{ t('aiGen.sections.hiresFix') }}
            <span class="adv-hint">{{ t('aiGen.sections.hiresFixHint') }}</span>
          </label>
          <label class="toggle-label">
            <input
              type="checkbox"
              v-model="hiresFixEnabled"
              class="toggle-check"
              :disabled="aiGen.isGenerating.value"
            />
            <span>{{ hiresFixEnabled ? t('common.on') : t('common.off') }}</span>
          </label>
        </div>

        <div v-if="hiresFixEnabled" class="hires-body">
          <!-- Target resolution -->
          <div class="hires-field">
            <span class="hires-field-label">{{ t('aiGen.sections.hiresTargetRes') }}</span>
            <div class="res-inputs">
              <input
                type="number"
                class="adv-num-input adv-num-input--res"
                min="256" :max="aiGen.selectedModel.value?.maxResolution ?? 2048" step="64"
                :value="hiresFixWidth"
                :disabled="aiGen.isGenerating.value"
                @change="hiresFixWidth = Math.round(Number(($event.target as HTMLInputElement).value) / 64) * 64"
              />
              <span class="res-x">×</span>
              <input
                type="number"
                class="adv-num-input adv-num-input--res"
                min="256" :max="aiGen.selectedModel.value?.maxResolution ?? 2048" step="64"
                :value="hiresFixHeight"
                :disabled="aiGen.isGenerating.value"
                @change="hiresFixHeight = Math.round(Number(($event.target as HTMLInputElement).value) / 64) * 64"
              />
            </div>
          </div>

          <!-- Denoising strength -->
          <div class="hires-field">
            <span class="hires-field-label">
              {{ t('aiGen.sections.hiresStrength') }}
              <span class="adv-hint">{{ t('aiGen.sections.hiresStrengthHint') }}</span>
            </span>
            <div class="adv-control">
              <input
                type="range"
                class="range-input"
                min="0.1" max="0.9" step="0.05"
                :value="hiresFixStrength"
                :disabled="aiGen.isGenerating.value"
                @input="hiresFixStrength = Number(($event.target as HTMLInputElement).value)"
              />
              <input
                type="number"
                class="adv-num-input"
                min="0.1" max="0.9" step="0.05"
                :value="hiresFixStrength"
                :disabled="aiGen.isGenerating.value"
                @change="hiresFixStrength = Math.min(0.9, Math.max(0.1, Number(($event.target as HTMLInputElement).value)))"
              />
            </div>
          </div>

          <!-- Hires steps -->
          <div class="hires-field">
            <span class="hires-field-label">{{ t('aiGen.sections.hiresSteps') }}</span>
            <div class="adv-control">
              <input
                type="range"
                class="range-input"
                min="5" max="50" step="1"
                :value="hiresFixSteps"
                :disabled="aiGen.isGenerating.value"
                @input="hiresFixSteps = Number(($event.target as HTMLInputElement).value)"
              />
              <input
                type="number"
                class="adv-num-input"
                min="5" max="50"
                :value="hiresFixSteps"
                :disabled="aiGen.isGenerating.value"
                @change="hiresFixSteps = Math.min(50, Math.max(5, Number(($event.target as HTMLInputElement).value)))"
              />
            </div>
          </div>
        </div>
      </div>

    </div>

    <!-- ── Generate button ────────────────────────────────────────────── -->
    <button
      class="btn-generate"
      :class="{ 'btn-generate--loading': aiGen.isGenerating.value }"
      :disabled="!canGenerate"
      @click="handleGenerate"
    >
      <template v-if="aiGen.isGenerating.value">
        <!-- Step progress inside the button -->
        <span class="gen-progress-text">
          {{ generationProgressText }}
        </span>
        <div class="gen-progress-bar">
          <div class="gen-progress-bar__fill" :style="{ width: `${aiGen.stepPercentage.value}%` }" />
        </div>
      </template>
      <template v-else-if="!aiGen.selectedModel.value?.isDownloaded">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
        </svg>
        {{ t('aiGen.actions.downloadModelFirst') }}
      </template>
      <template v-else-if="!prompt.trim()">
        {{ t('aiGen.actions.enterPromptFirst') }}
      </template>
      <template v-else>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polygon points="5 3 19 12 5 21 5 3"/>
        </svg>
        {{ t('aiGen.actions.generate') }}
      </template>
    </button>

    <!-- CPU warning (hide if we're already showing the GPU-hint badge) -->
    <p v-if="aiGen.device.value && !aiGen.device.value.isAccelerated && !aiGen.device.value.gpuAvailableButDisabled" class="cpu-warning">
      {{ t('aiGen.cpuWarning') }}
    </p>

  </div>
</template>

<style scoped>
.ai-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding-bottom: var(--space-4);
}

/* ── Device row ───────────────────────────────────────────────────── */
.device-row {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

/* ── Device badge ─────────────────────────────────────────────────── */
.device-badge {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  align-self: flex-start;
  padding: 3px 8px;
  border-radius: var(--radius-full);
  font-size: 10px;
  font-weight: 600;
  background: var(--bg-input);
  border: 1px solid var(--border);
  color: var(--text-tertiary);
  letter-spacing: 0.03em;
}
.device-badge.cuda {
  border-color: rgba(99, 102, 241, 0.4);
  color: var(--accent-hover);
}
.device-badge__dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--text-disabled);
  flex-shrink: 0;
}
.device-badge.cuda .device-badge__dot {
  background: var(--accent);
  box-shadow: 0 0 4px var(--accent-glow);
}
.device-badge.metal .device-badge__dot {
  background: #f97316;
  box-shadow: 0 0 4px rgba(249, 115, 22, 0.5);
}
.device-badge.metal {
  border-color: rgba(249, 115, 22, 0.4);
  color: #f97316;
}

/* GPU hint badge — shown when GPU detected but feature not compiled in */
.gpu-hint-badge {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 3px 8px;
  border-radius: var(--radius-full);
  font-size: 10px;
  font-weight: 600;
  background: rgba(234, 179, 8, 0.1);
  border: 1px solid rgba(234, 179, 8, 0.35);
  color: #ca8a04;
  cursor: pointer;
  letter-spacing: 0.02em;
  transition: background var(--transition-fast);
}
.gpu-hint-badge:hover {
  background: rgba(234, 179, 8, 0.2);
}
.gpu-hint-cmd {
  font-family: 'JetBrains Mono', 'Fira Code', ui-monospace, monospace;
  font-size: 9px;
  font-weight: 700;
  padding: 1px 4px;
  background: rgba(234, 179, 8, 0.15);
  border-radius: 3px;
  border: 1px solid rgba(234, 179, 8, 0.3);
  white-space: nowrap;
}

/* ── Section label ────────────────────────────────────────────────── */
.section-label {
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--text-disabled);
}

/* ── Model cards ──────────────────────────────────────────────────── */
.model-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.model-card {
  padding: var(--space-3);
  background: var(--bg-card);
  border: 1.5px solid var(--border);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: border-color var(--transition-fast), background var(--transition-fast);
}
.model-card:hover:not(.model-card--disabled) {
  border-color: rgba(99, 102, 241, 0.4);
  background: var(--bg-card-hover);
}
.model-card--selected {
  border-color: var(--accent) !important;
  background: rgba(99, 102, 241, 0.06) !important;
}
.model-card--disabled { opacity: 0.55; cursor: not-allowed; }

.model-card__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 4px;
}
.model-card__name-row {
  display: flex;
  align-items: center;
  gap: 6px;
}
.model-card__name {
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--text-primary);
}
.model-card__tag {
  font-size: 10px;
  padding: 1px 5px;
  border-radius: var(--radius-full);
  background: rgba(99,102,241,0.15);
  color: var(--accent-hover);
  font-weight: 600;
}
.model-card__desc {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  line-height: 1.5;
  margin: 0 0 var(--space-2);
}

/* Status dot */
.status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}
.status-dot--ready { background: var(--success); }
.status-dot--pending { background: var(--text-disabled); }

/* Download progress */
.progress-wrap {
  margin-top: var(--space-2);
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.progress-label {
  display: flex;
  justify-content: space-between;
  font-size: 10px;
  color: var(--text-tertiary);
}
.progress-file {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 140px;
}
.progress-pct { flex-shrink: 0; color: var(--accent-hover); font-weight: 600; }
.progress-bar {
  height: 3px;
  border-radius: 2px;
  background: var(--bg-input);
  overflow: hidden;
}
.progress-bar__fill {
  height: 100%;
  background: var(--accent);
  border-radius: 2px;
  transition: width 0.3s;
}
.progress-bytes {
  font-size: 10px;
  color: var(--text-disabled);
  text-align: right;
}

/* Model card footer */
.model-card__footer { margin-top: var(--space-2); }

.btn-model-action {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 10px;
  border-radius: var(--radius-sm);
  font-size: var(--text-xs);
  font-weight: 600;
  cursor: pointer;
  border: 1px solid transparent;
  transition: background var(--transition-fast), border-color var(--transition-fast);
}
.btn-model-action--download {
  background: rgba(99,102,241,0.1);
  border-color: rgba(99,102,241,0.35);
  color: var(--accent-hover);
}
.btn-model-action--download:hover:not(:disabled) {
  background: rgba(99,102,241,0.2);
  border-color: var(--accent);
}
.btn-model-action:disabled { opacity: 0.4; cursor: not-allowed; }

.model-card__ready {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: var(--text-xs);
  font-weight: 600;
  color: var(--success);
}

/* ── Prompt inputs ────────────────────────────────────────────────── */
.prompt-input {
  width: 100%;
  box-sizing: border-box;
  padding: var(--space-2) var(--space-3);
  background: var(--bg-input);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  color: var(--text-primary);
  font-size: var(--text-sm);
  line-height: 1.5;
  resize: vertical;
  outline: none;
  transition: border-color var(--transition-fast);
  font-family: inherit;
}
.prompt-input:focus { border-color: var(--accent); }
.prompt-input:disabled { opacity: 0.5; cursor: not-allowed; }
.prompt-input--neg {
  color: var(--text-secondary);
  border-color: rgba(239, 68, 68, 0.2);
}
.prompt-input--neg:focus { border-color: rgba(239, 68, 68, 0.5); }

/* section label with inline hint */
.section-label--neg {
  display: flex;
  align-items: baseline;
  gap: 6px;
}
.label-hint {
  font-size: 9px;
  font-weight: 400;
  color: var(--text-disabled);
  text-transform: none;
  letter-spacing: 0;
}

/* ── Advanced settings toggle ──────────────────────────────────────── */
.advanced-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  cursor: pointer;
  padding: 2px 0;
  user-select: none;
}
.advanced-toggle:hover { color: var(--text-secondary); }
/* push chevron to the right */
.advanced-toggle > svg:last-child { margin-left: auto; }

/* ── Advanced settings panel ───────────────────────────────────────── */
.advanced-wrap {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-3);
  background: var(--bg-input);
  border-radius: var(--radius-md);
  border: 1px solid var(--border);
}

/* each param row */
.adv-row {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.adv-label {
  display: flex;
  align-items: baseline;
  gap: 6px;
  font-size: 10px;
  font-weight: 700;
  color: var(--text-tertiary);
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.adv-hint {
  font-size: 9px;
  font-weight: 400;
  color: var(--text-disabled);
  text-transform: none;
  letter-spacing: 0;
}

/* slider + number input side by side */
.adv-control {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.range-input {
  flex: 1;
  accent-color: var(--accent);
  cursor: pointer;
  min-width: 0;
}
.range-input:disabled { opacity: 0.4; cursor: not-allowed; }

/* compact editable number box */
.adv-num-input {
  width: 52px;
  flex-shrink: 0;
  padding: 3px 6px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--accent-hover);
  font-size: var(--text-xs);
  font-weight: 700;
  text-align: center;
  outline: none;
  transition: border-color var(--transition-fast);
  /* hide number spinners */
  -moz-appearance: textfield;
}
.adv-num-input::-webkit-inner-spin-button,
.adv-num-input::-webkit-outer-spin-button { -webkit-appearance: none; }
.adv-num-input:focus { border-color: var(--accent); }
.adv-num-input:disabled { opacity: 0.4; cursor: not-allowed; }

/* resolution inputs */
.adv-res-row {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.res-presets {
  display: flex;
  gap: 4px;
}
.res-preset-btn {
  padding: 3px 8px;
  font-size: 10px;
  font-weight: 700;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: var(--bg-card);
  color: var(--text-tertiary);
  cursor: pointer;
  transition: background var(--transition-fast), border-color var(--transition-fast), color var(--transition-fast);
}
.res-preset-btn:hover:not(:disabled) {
  border-color: rgba(99,102,241,0.4);
  color: var(--accent-hover);
}
.res-preset-btn--active {
  background: rgba(99,102,241,0.12);
  border-color: var(--accent);
  color: var(--accent-hover);
}
.res-preset-btn:disabled { opacity: 0.4; cursor: not-allowed; }

.res-inputs {
  display: flex;
  align-items: center;
  gap: 6px;
}
.adv-num-input--res {
  width: 62px;
}
.res-x {
  font-size: var(--text-xs);
  color: var(--text-disabled);
}

/* seed row */
.seed-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}
.toggle-label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--text-xs);
  color: var(--text-secondary);
  cursor: pointer;
  white-space: nowrap;
}
.toggle-check { accent-color: var(--accent); cursor: pointer; }
.seed-num {
  width: 100px;
  text-align: left;
}

/* ── Generate button ──────────────────────────────────────────────── */
.btn-generate {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  width: 100%;
  padding: 12px var(--space-4);
  background: var(--accent);
  color: #fff;
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  font-weight: 700;
  cursor: pointer;
  transition: background var(--transition-fast), opacity var(--transition-fast);
  min-height: 48px;
}
.btn-generate:not(:disabled):hover { background: var(--accent-hover); }
.btn-generate:disabled {
  opacity: 0.45;
  cursor: not-allowed;
  background: var(--bg-button);
  color: var(--text-tertiary);
}
.btn-generate--loading {
  background: rgba(99,102,241,0.15) !important;
  border: 1px solid rgba(99,102,241,0.35) !important;
  color: var(--accent-hover) !important;
  cursor: default !important;
  opacity: 1 !important;
}

.gen-progress-text {
  font-size: var(--text-xs);
  font-weight: 600;
  color: var(--accent-hover);
}
.gen-progress-bar {
  width: 100%;
  height: 3px;
  border-radius: 2px;
  background: rgba(99,102,241,0.15);
  overflow: hidden;
}
.gen-progress-bar__fill {
  height: 100%;
  background: var(--accent);
  border-radius: 2px;
  transition: width 0.4s;
}

/* ── CPU warning ─────────────────────────────────────────────────── */
.cpu-warning {
  font-size: var(--text-xs);
  color: var(--text-disabled);
  text-align: center;
  line-height: 1.5;
}

/* ── HF Token section ─────────────────────────────────────────────── */
.token-section {
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--bg-card);
  overflow: hidden;
}

.token-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 10px;
  cursor: pointer;
  user-select: none;
  transition: background var(--transition-fast);
}
.token-header:hover { background: var(--bg-card-hover); }

.token-header__left {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--text-xs);
  font-weight: 600;
  color: var(--text-secondary);
}

.token-badge {
  font-size: 10px;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: var(--radius-full);
  letter-spacing: 0.03em;
}
.token-badge--set {
  background: rgba(34, 197, 94, 0.15);
  color: var(--success);
}
.token-badge--unset {
  background: var(--bg-input);
  color: var(--text-disabled);
}

.auth-alert {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  background: rgba(234, 179, 8, 0.1);
  border-top: 1px solid rgba(234, 179, 8, 0.25);
  color: #ca8a04;
  font-size: var(--text-xs);
  font-weight: 600;
  cursor: pointer;
  transition: background var(--transition-fast);
}
.auth-alert:hover { background: rgba(234, 179, 8, 0.18); }

.token-body {
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  border-top: 1px solid var(--border);
}

.token-current {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: 6px 8px;
  background: var(--bg-input);
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
}

.token-masked {
  flex: 1;
  font-size: 11px;
  font-family: 'JetBrains Mono', 'Fira Code', ui-monospace, monospace;
  color: var(--success);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  background: none;
}

.btn-token-del {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border-radius: var(--radius-sm);
  background: transparent;
  border: 1px solid transparent;
  color: var(--text-disabled);
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
}
.btn-token-del:hover {
  background: rgba(239, 68, 68, 0.12);
  border-color: rgba(239, 68, 68, 0.3);
  color: #ef4444;
}

.token-help {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  line-height: 1.5;
  margin: 0;
}

.link-btn {
  background: none;
  border: none;
  padding: 0;
  font-size: inherit;
  color: var(--accent-hover);
  cursor: pointer;
  text-decoration: underline;
  text-underline-offset: 2px;
  font-family: inherit;
}
.link-btn:hover { color: var(--accent); }

.token-input-row {
  display: flex;
  gap: var(--space-2);
}

.token-input {
  flex: 1;
  padding: 5px 8px;
  background: var(--bg-input);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-xs);
  font-family: 'JetBrains Mono', 'Fira Code', ui-monospace, monospace;
  outline: none;
  transition: border-color var(--transition-fast);
}
.token-input:focus { border-color: var(--accent); }

.btn-token-save {
  flex-shrink: 0;
  padding: 5px 12px;
  background: var(--accent);
  color: #fff;
  border: none;
  border-radius: var(--radius-sm);
  font-size: var(--text-xs);
  font-weight: 700;
  cursor: pointer;
  transition: background var(--transition-fast), opacity var(--transition-fast);
  white-space: nowrap;
}
.btn-token-save:hover:not(:disabled) { background: var(--accent-hover); }
.btn-token-save:disabled { opacity: 0.4; cursor: not-allowed; }

/* Expand transition */
.expand-enter-active,
.expand-leave-active {
  transition: max-height 0.22s ease, opacity 0.18s ease;
  max-height: 300px;
  overflow: hidden;
}
.expand-enter-from,
.expand-leave-to {
  max-height: 0;
  opacity: 0;
}

/* inline label + input row (Steps, CFG) */
.adv-row--inline {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}
/* wider number box without spinner arrows */
.adv-num-input--wide {
  width: 72px;
  flex-shrink: 0;
  padding: 4px 8px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--accent-hover);
  font-size: var(--text-sm);
  font-weight: 700;
  text-align: center;
  outline: none;
  transition: border-color var(--transition-fast);
  -moz-appearance: textfield;
}
.adv-num-input--wide::-webkit-inner-spin-button,
.adv-num-input--wide::-webkit-outer-spin-button { -webkit-appearance: none; }
.adv-num-input--wide:focus { border-color: var(--accent); }
.adv-num-input--wide:disabled { opacity: 0.4; cursor: not-allowed; }

/* ── Sampler toggle group ─────────────────────────────────────────── */
.sampler-group {
  display: flex;
  gap: 4px;
}
.sampler-btn {
  flex: 1;
  padding: 4px 8px;
  font-size: 10px;
  font-weight: 600;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: var(--bg-card);
  color: var(--text-tertiary);
  cursor: pointer;
  transition: background var(--transition-fast), border-color var(--transition-fast), color var(--transition-fast);
  white-space: nowrap;
  text-align: center;
}
.sampler-btn:hover:not(:disabled) {
  border-color: rgba(99,102,241,0.4);
  color: var(--accent-hover);
}
.sampler-btn--active {
  background: rgba(99,102,241,0.12);
  border-color: var(--accent);
  color: var(--accent-hover);
}
.sampler-btn:disabled { opacity: 0.4; cursor: not-allowed; }

/* ── Hires Fix ────────────────────────────────────────────────────── */
.hires-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.hires-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin-top: var(--space-2);
  padding: var(--space-2) var(--space-3);
  background: rgba(99,102,241,0.04);
  border: 1px solid rgba(99,102,241,0.15);
  border-radius: var(--radius-sm);
}
.hires-field {
  display: flex;
  flex-direction: column;
  gap: 5px;
}
.hires-field-label {
  display: flex;
  align-items: baseline;
  gap: 6px;
  font-size: 10px;
  font-weight: 600;
  color: var(--text-tertiary);
  letter-spacing: 0.04em;
  text-transform: uppercase;
}
</style>

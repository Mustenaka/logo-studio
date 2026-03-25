<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useAiGen } from '../../modules/ai-gen/useAiGen'

const aiGen = useAiGen()

// Local form state
const prompt = ref('')
const negativePrompt = ref('blurry, low quality, watermark, ugly, deformed, text, signature')
const showAdvanced = ref(false)
const customSteps = ref<number | null>(null)
const customGuidance = ref<number | null>(null)
const seed = ref<number | null>(null)
const randomSeed = ref(true)

onMounted(() => aiGen.init())

// When a model is selected, populate prompt placeholder
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

function formatBytes(b: number): string {
  if (b >= 1_073_741_824) return `${(b / 1_073_741_824).toFixed(1)} GB`
  if (b >= 1_048_576) return `${(b / 1_048_576).toFixed(0)} MB`
  return `${(b / 1024).toFixed(0)} KB`
}

async function handleGenerate() {
  if (!canGenerate.value || !aiGen.selectedModel.value) return
  await aiGen.generate({
    modelId: aiGen.selectedModel.value.id,
    prompt: prompt.value,
    negativePrompt: negativePrompt.value || undefined,
    steps: customSteps.value ?? undefined,
    guidance: customGuidance.value ?? undefined,
    seed: randomSeed.value ? null : seed.value,
  })
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
    <div v-if="aiGen.device.value" class="device-badge" :class="aiGen.device.value.kind">
      <span class="device-badge__dot" />
      <span>{{ aiGen.device.value.isAccelerated ? 'GPU · ' : 'CPU · ' }}{{ aiGen.device.value.name }}</span>
    </div>

    <!-- ── Model selector ────────────────────────────────────────────── -->
    <div class="section-label">选择模型</div>
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
          <span v-if="model.isDownloaded" class="status-dot status-dot--ready" title="已下载" />
          <span v-else class="status-dot status-dot--pending" title="未下载" />
        </div>

        <p class="model-card__desc">{{ model.description }}</p>

        <!-- Download progress (when downloading this model) -->
        <template v-if="aiGen.downloadingModelId.value === model.id">
          <div class="progress-wrap">
            <div class="progress-label">
              <span class="progress-file">{{ aiGen.downloadProgress.value?.fileName ?? '准备中...' }}</span>
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
            下载 (~{{ (model.sizeMb / 1024).toFixed(1) }} GB)
          </button>
          <div v-else class="model-card__ready">
            <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
              <polyline points="20 6 9 17 4 12"/>
            </svg>
            已就绪
          </div>
        </div>
      </div>
    </div>

    <!-- ── Prompt ─────────────────────────────────────────────────────── -->
    <div class="section-label">提示词</div>
    <textarea
      v-model="prompt"
      class="prompt-input"
      :placeholder="aiGen.selectedModel.value?.examplePrompt ?? '描述你想生成的 logo...'"
      :disabled="aiGen.isGenerating.value"
      rows="3"
    />

    <!-- Negative prompt -->
    <div class="neg-prompt-toggle" @click="showAdvanced = !showAdvanced">
      <span>高级设置</span>
      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
        :style="{ transform: showAdvanced ? 'rotate(180deg)' : 'none', transition: 'transform 0.2s' }">
        <polyline points="6 9 12 15 18 9"/>
      </svg>
    </div>

    <div v-if="showAdvanced" class="advanced-wrap">
      <!-- Negative prompt -->
      <div class="field-label">负面提示词</div>
      <textarea
        v-model="negativePrompt"
        class="prompt-input prompt-input--neg"
        rows="2"
        :disabled="aiGen.isGenerating.value"
      />

      <!-- Steps -->
      <div class="field-row">
        <div class="field-label">步数</div>
        <div class="field-hint">CPU 建议 ≤ 20</div>
      </div>
      <div class="steps-row">
        <input
          type="range"
          class="range-input"
          :min="aiGen.selectedModel.value?.base === 'sdxl' ? 1 : 10"
          :max="aiGen.selectedModel.value?.base === 'sdxl' ? 8 : 50"
          :value="effectiveSteps"
          :disabled="aiGen.isGenerating.value"
          @input="customSteps = Number(($event.target as HTMLInputElement).value)"
        />
        <span class="steps-val">{{ effectiveSteps }}</span>
      </div>

      <!-- Guidance -->
      <div v-if="(aiGen.selectedModel.value?.defaultGuidance ?? 0) > 0" class="field-row">
        <div class="field-label">引导强度</div>
        <div class="field-hint">{{ effectiveGuidance.toFixed(1) }}</div>
      </div>
      <input
        v-if="(aiGen.selectedModel.value?.defaultGuidance ?? 0) > 0"
        type="range"
        class="range-input"
        min="1" max="15" step="0.5"
        :value="effectiveGuidance"
        :disabled="aiGen.isGenerating.value"
        @input="customGuidance = Number(($event.target as HTMLInputElement).value)"
      />

      <!-- Seed -->
      <div class="field-label">随机种子</div>
      <div class="seed-row">
        <label class="toggle-label">
          <input type="checkbox" v-model="randomSeed" class="toggle-check" />
          <span>随机</span>
        </label>
        <input
          v-if="!randomSeed"
          type="number"
          class="seed-input"
          v-model.number="seed"
          placeholder="输入种子值"
          :disabled="aiGen.isGenerating.value"
        />
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
          生成中 {{ aiGen.stepProgress.value?.step ?? 0 }} / {{ aiGen.stepProgress.value?.totalSteps ?? effectiveSteps }} 步
        </span>
        <div class="gen-progress-bar">
          <div class="gen-progress-bar__fill" :style="{ width: `${aiGen.stepPercentage.value}%` }" />
        </div>
      </template>
      <template v-else-if="!aiGen.selectedModel.value?.isDownloaded">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
        </svg>
        请先下载模型
      </template>
      <template v-else-if="!prompt.trim()">
        输入提示词后生成
      </template>
      <template v-else>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polygon points="5 3 19 12 5 21 5 3"/>
        </svg>
        生成 Logo
      </template>
    </button>

    <!-- CPU warning -->
    <p v-if="aiGen.device.value && !aiGen.device.value.isAccelerated" class="cpu-warning">
      当前为 CPU 模式，生成约需 2–5 分钟，请耐心等待
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
.prompt-input--neg { color: var(--text-secondary); }

/* ── Advanced settings ─────────────────────────────────────────────── */
.neg-prompt-toggle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  cursor: pointer;
  padding: 2px 0;
  user-select: none;
}
.neg-prompt-toggle:hover { color: var(--text-secondary); }

.advanced-wrap {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3);
  background: var(--bg-input);
  border-radius: var(--radius-md);
  border: 1px solid var(--border);
}

.field-label {
  font-size: 10px;
  font-weight: 600;
  color: var(--text-tertiary);
  letter-spacing: 0.05em;
  text-transform: uppercase;
}
.field-hint {
  font-size: 10px;
  color: var(--text-disabled);
}
.field-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.range-input {
  width: 100%;
  accent-color: var(--accent);
  cursor: pointer;
}
.range-input:disabled { opacity: 0.4; cursor: not-allowed; }

.steps-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}
.steps-val {
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--accent-hover);
  min-width: 24px;
  text-align: right;
}

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
.seed-input {
  flex: 1;
  padding: 4px 8px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-xs);
  outline: none;
}
.seed-input:focus { border-color: var(--accent); }

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
</style>

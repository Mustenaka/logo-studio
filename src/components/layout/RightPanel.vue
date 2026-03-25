<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import GlassCard from '../ui/GlassCard.vue'
import SliderControl from '../ui/SliderControl.vue'
import { useBackgroundStore, PRESETS } from '../../store/useBackgroundStore'
import { useTypographyStore } from '../../store/useTypographyStore'
import { useCanvasStore } from '../../store/useCanvasStore'
import { useExport, getIconPresetOptions, type IconPresetId } from '../../modules/export/useExport'

const bgStore = useBackgroundStore()
const typoStore = useTypographyStore()
const canvasStore = useCanvasStore()
const { exportPng, exportOriginalSize, exportIconSet, isExporting, lastExportMsg, isExportError } = useExport()
const { t } = useI18n()

const activePanel = ref<'background' | 'typography' | 'export'>('background')

const selectedLayer = computed(() => typoStore.getSelectedLayer())
const iconPresetOptions = computed(() => getIconPresetOptions(t))
const iconPresetEntries = computed(() => Object.entries(iconPresetOptions.value) as Array<[IconPresetId, (typeof iconPresetOptions.value)[IconPresetId]]>)

const panels = computed(() => ([
  { id: 'background', label: t('rightPanel.nav.background') },
  { id: 'typography', label: t('rightPanel.nav.typography') },
  { id: 'export', label: t('rightPanel.nav.export') },
]) as const)

const bgPresetsCollapsed = ref(false)
const bgGradientCollapsed = ref(false)
const bgShapeCollapsed = ref(false)
const typoLayersCollapsed = ref(false)
const typoEditorCollapsed = ref(false)
const exportSingleCollapsed = ref(false)
const exportIconCollapsed = ref(false)
</script>

<template>
  <aside class="right-panel">
    <nav class="panel-nav">
      <button
        v-for="p in panels"
        :key="p.id"
        class="nav-btn"
        :class="{ active: activePanel === p.id }"
        @click="activePanel = p.id"
      >{{ p.label }}</button>
    </nav>

    <div class="panel-scroll-area panel-scroll">
      <div v-if="activePanel === 'background'" class="panel-section">
        <GlassCard :title="t('rightPanel.background.presets')" collapsible v-model:collapsed="bgPresetsCollapsed">
          <div class="preset-grid">
            <button
              v-for="preset in PRESETS"
              :key="preset.id"
              class="preset-item"
              :class="{ active: bgStore.activePresetId === preset.id }"
              @click="bgStore.applyPreset(preset.id)"
            >
              <div
                class="preset-thumb"
                :style="{
                  background: preset.type === 'none'
                    ? 'transparent'
                    : preset.type === 'solid'
                      ? preset.stops[0]?.color ?? '#000'
                      : preset.type === 'radial'
                        ? `radial-gradient(circle, ${preset.stops.map(s => `${s.color} ${s.position}%`).join(',')})`
                        : `linear-gradient(${preset.angle}deg, ${preset.stops.map(s => `${s.color} ${s.position}%`).join(',')})`,
                  border: preset.type === 'none' ? '1px dashed var(--border)' : 'none'
                }"
              />
              <span class="preset-name">{{ preset.name }}</span>
            </button>
          </div>
        </GlassCard>

        <GlassCard v-if="bgStore.bgType !== 'none'" :title="t('rightPanel.background.gradient')" collapsible v-model:collapsed="bgGradientCollapsed">
          <div class="gradient-stops">
            <div v-for="(stop, i) in bgStore.stops" :key="i" class="stop-row">
              <input type="color" class="color-pick" :value="stop.color" @input="(e) => bgStore.updateStop(i, { color: (e.target as HTMLInputElement).value })" />
              <input type="range" class="stop-pos" min="0" max="100" :value="stop.position" @input="(e) => bgStore.updateStop(i, { position: Number((e.target as HTMLInputElement).value) })" />
              <span class="stop-val">{{ stop.position }}%</span>
            </div>
          </div>
          <div v-if="bgStore.bgType === 'linear'" style="margin-top: 8px;">
            <SliderControl :label="t('rightPanel.background.angle')" v-model="bgStore.angle" :min="0" :max="360" unit="°" />
          </div>
        </GlassCard>

        <GlassCard :title="t('rightPanel.background.shape')" collapsible v-model:collapsed="bgShapeCollapsed">
          <SliderControl :label="t('rightPanel.background.radius')" v-model="bgStore.borderRadius" :min="0" :max="400" unit="px" />
          <div class="card-divider" />
          <div class="row-toggle">
            <span class="label-sm">{{ t('rightPanel.background.shadow') }}</span>
            <button class="toggle-btn" :class="{ on: bgStore.shadowEnabled }" @click="bgStore.shadowEnabled = !bgStore.shadowEnabled">
              {{ bgStore.shadowEnabled ? t('common.on') : t('common.off') }}
            </button>
          </div>
          <template v-if="bgStore.shadowEnabled">
            <SliderControl :label="t('rightPanel.background.blur')" v-model="bgStore.shadowBlur" :min="0" :max="80" unit="px" />
            <SliderControl :label="t('rightPanel.background.offsetY')" v-model="bgStore.shadowOffsetY" :min="-40" :max="40" unit="px" />
          </template>
        </GlassCard>
      </div>

      <div v-if="activePanel === 'typography'" class="panel-section">
        <GlassCard :title="t('rightPanel.typography.addText')" :padding="false">
          <div class="text-add-btns">
            <button class="btn-ghost text-add-btn" @click="typoStore.addTitleLayer()">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="4 7 4 4 20 4 20 7"/><line x1="9" y1="20" x2="15" y2="20"/>
                <line x1="12" y1="4" x2="12" y2="20"/>
              </svg>
              {{ t('rightPanel.typography.title') }}
            </button>
            <button class="btn-ghost text-add-btn" @click="typoStore.addSloganLayer()">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="21" y1="6" x2="3" y2="6"/><line x1="15" y1="12" x2="3" y2="12"/>
                <line x1="17" y1="18" x2="3" y2="18"/>
              </svg>
              {{ t('rightPanel.typography.slogan') }}
            </button>
          </div>
        </GlassCard>

        <GlassCard v-if="typoStore.textLayers.length > 0" :title="t('rightPanel.typography.textLayers')" :padding="false" collapsible v-model:collapsed="typoLayersCollapsed">
          <div
            v-for="layer in typoStore.textLayers"
            :key="layer.id"
            class="text-layer-item"
            :class="{ selected: typoStore.selectedLayerId === layer.id }"
            @click="typoStore.selectLayer(layer.id)"
          >
            <span class="tl-type">{{ layer.type === 'title' ? 'T' : 't' }}</span>
            <span class="tl-text">{{ layer.text }}</span>
            <button class="tl-delete" @click.stop="typoStore.removeLayer(layer.id)" :title="t('common.delete')">×</button>
          </div>
        </GlassCard>

        <GlassCard
          v-if="selectedLayer"
          :title="t('rightPanel.typography.editLayer', { type: selectedLayer.type === 'title' ? t('rightPanel.typography.title') : t('rightPanel.typography.slogan') })"
          collapsible
          v-model:collapsed="typoEditorCollapsed"
        >
          <div class="text-editor">
            <div class="field-group">
              <label class="field-label">{{ t('rightPanel.typography.fields.content') }}</label>
              <textarea
                class="text-input"
                :value="selectedLayer.text"
                @input="(e) => typoStore.updateLayer(selectedLayer!.id, { text: (e.target as HTMLTextAreaElement).value })"
                rows="2"
              />
            </div>
            <div class="field-group">
              <label class="field-label">{{ t('rightPanel.typography.fields.font') }}</label>
              <select
                class="select-input"
                :value="selectedLayer.fontFamily"
                @change="(e) => typoStore.updateLayer(selectedLayer!.id, { fontFamily: (e.target as HTMLSelectElement).value })"
              >
                <option v-for="f in typoStore.availableFonts" :key="f.family" :value="f.family">{{ f.family }}</option>
              </select>
            </div>
            <div class="field-group">
              <label class="field-label">{{ t('rightPanel.typography.fields.color') }}</label>
              <input
                type="color"
                class="color-pick"
                :value="selectedLayer.color"
                @input="(e) => typoStore.updateLayer(selectedLayer!.id, { color: (e.target as HTMLInputElement).value })"
              />
            </div>
            <SliderControl :label="t('rightPanel.typography.fields.fontSize')" v-model="selectedLayer.fontSize" :min="8" :max="200" unit="px"
              @update:model-value="(v) => typoStore.updateLayer(selectedLayer!.id, { fontSize: v })" />
            <SliderControl :label="t('rightPanel.typography.fields.fontWeight')" v-model="selectedLayer.fontWeight" :min="100" :max="900" :step="100"
              @update:model-value="(v) => typoStore.updateLayer(selectedLayer!.id, { fontWeight: v })" />
            <SliderControl :label="t('rightPanel.typography.fields.letterSpacing')" v-model="selectedLayer.letterSpacing" :min="-5" :max="30" unit="px"
              @update:model-value="(v) => typoStore.updateLayer(selectedLayer!.id, { letterSpacing: v })" />
            <SliderControl :label="t('rightPanel.typography.fields.yPosition')" v-model="selectedLayer.y" :min="0" :max="canvasStore.canvasHeight"
              @update:model-value="(v) => typoStore.updateLayer(selectedLayer!.id, { y: v })" />
          </div>
        </GlassCard>
      </div>

      <div v-if="activePanel === 'export'" class="panel-section">
        <GlassCard :title="t('rightPanel.export.singlePng')" collapsible v-model:collapsed="exportSingleCollapsed">
          <div class="export-info">
            <div class="info-row">
              <span>{{ t('rightPanel.export.canvasSize') }}</span>
              <span>{{ canvasStore.canvasWidth }} × {{ canvasStore.canvasHeight }}</span>
            </div>
            <div class="info-row">
              <span>{{ t('rightPanel.export.format') }}</span>
              <span>{{ t('rightPanel.export.pngAlpha') }}</span>
            </div>
          </div>
          <div class="export-sizes">
            <button class="btn-accent export-btn" :disabled="isExporting || !canvasStore.hasImage" @click="exportPng(1024)">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/>
                <polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>
              </svg>
              1024 × 1024 px
            </button>
            <button class="btn-ghost export-btn" :disabled="isExporting || !canvasStore.hasImage" @click="exportPng(512)">512 × 512 px</button>
            <button class="btn-ghost export-btn" :disabled="isExporting || !canvasStore.hasImage" @click="exportPng(256)">256 × 256 px</button>
            <button class="btn-ghost export-btn" :disabled="isExporting || !canvasStore.hasImage" @click="exportPng(128)">128 × 128 px</button>
            <button
              v-if="canvasStore.imageLayer"
              class="btn-ghost export-btn export-btn-original"
              :disabled="isExporting"
              :title="t('rightPanel.export.exportOriginalSizeHint', { w: canvasStore.imageLayer.naturalWidth, h: canvasStore.imageLayer.naturalHeight })"
              @click="exportOriginalSize()"
            >
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="3" y="3" width="18" height="18" rx="2"/>
                <path d="M9 3v18M15 3v18M3 9h18M3 15h18"/>
              </svg>
              {{ t('rightPanel.export.exportOriginalSize') }}
              <span class="original-size-tag">{{ canvasStore.imageLayer.naturalWidth }} × {{ canvasStore.imageLayer.naturalHeight }} px</span>
            </button>
          </div>
        </GlassCard>

        <GlassCard :title="t('rightPanel.export.iconSet')" collapsible v-model:collapsed="exportIconCollapsed">
          <p class="export-hint">{{ t('rightPanel.export.iconSetHint') }}</p>
          <div class="icon-presets">
            <button
              v-for="[id, preset] in iconPresetEntries"
              :key="id"
              class="icon-preset-btn"
              :disabled="isExporting || !canvasStore.hasImage"
              @click="exportIconSet(id as IconPresetId)"
            >
              <span class="preset-icon">
                <svg v-if="id === 'web'" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/>
                  <path d="M12 2a15.3 15.3 0 014 10 15.3 15.3 0 01-4 10 15.3 15.3 0 01-4-10 15.3 15.3 0 014-10z"/>
                </svg>
                <svg v-else-if="id === 'ios'" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <rect x="5" y="2" width="14" height="20" rx="2" ry="2"/>
                  <line x1="12" y1="18" x2="12.01" y2="18"/>
                </svg>
                <svg v-else-if="id === 'android'" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M17.6 9.5H6.4L5 17h14L17.6 9.5z"/><path d="M12 9.5V7m-4 2.5 -1.5-3m9 3 1.5-3"/>
                  <circle cx="9" cy="6" r="0.5" fill="currentColor"/><circle cx="15" cy="6" r="0.5" fill="currentColor"/>
                </svg>
                <svg v-else-if="id === 'macos'" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <rect x="2" y="4" width="20" height="14" rx="2"/><path d="M8 20h8m-4-2v2"/>
                </svg>
                <svg v-else width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z"/>
                </svg>
              </span>
              <span class="preset-texts">
                <span class="preset-name">{{ preset.label }}</span>
                <span class="preset-desc">{{ preset.desc }}</span>
              </span>
              <svg class="preset-arrow" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/>
                <polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>
              </svg>
            </button>
          </div>
        </GlassCard>

        <div v-if="lastExportMsg" class="export-status" :class="{ error: isExportError }">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline v-if="!isExportError" points="20 6 9 17 4 12"/>
            <circle v-else cx="12" cy="12" r="10"/><line v-if="isExportError" x1="12" y1="8" x2="12" y2="12"/>
          </svg>
          <span>{{ lastExportMsg }}</span>
        </div>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.right-panel {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--bg-panel);
  border-left: 1px solid var(--border);
  overflow: hidden;
}

.panel-nav {
  display: flex;
  gap: var(--space-1);
  background: var(--bg-input);
  border-radius: var(--radius-md);
  padding: 3px;
  flex-shrink: 0;
  margin: var(--space-3) var(--space-3) 0;
}

.panel-scroll-area {
  flex: 1;
  overflow-y: auto;
  padding: var(--space-3);
  display: flex;
  flex-direction: column;
  scroll-behavior: smooth;
}
.panel-scroll-area::-webkit-scrollbar {
  width: 4px;
}
.panel-scroll-area::-webkit-scrollbar-track {
  background: transparent;
}
.panel-scroll-area::-webkit-scrollbar-thumb {
  background: var(--border);
  border-radius: 2px;
}
.panel-scroll-area::-webkit-scrollbar-thumb:hover {
  background: var(--border-hover);
}

.nav-btn {
  flex: 1;
  padding: 5px 4px;
  background: transparent;
  color: var(--text-tertiary);
  border: none;
  border-radius: var(--radius-sm);
  font-size: 11px;
  font-weight: 500;
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
}
.nav-btn.active {
  background: var(--bg-card-hover);
  color: var(--text-primary);
}

.panel-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.preset-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: var(--space-2);
}

.preset-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  background: transparent;
  border: none;
  cursor: pointer;
  padding: 0;
}
.preset-thumb {
  width: 100%;
  height: 44px;
  border-radius: var(--radius-sm);
  border: 2px solid transparent;
  transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
}
.preset-item.active .preset-thumb {
  border-color: var(--accent);
  box-shadow: var(--shadow-glow);
}
.preset-item:hover .preset-thumb {
  border-color: var(--border-hover);
}
.preset-name {
  font-size: 10px;
  color: var(--text-tertiary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100%;
}

.gradient-stops {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}
.stop-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}
.color-pick {
  width: 28px;
  height: 28px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  cursor: pointer;
  padding: 0;
  background: none;
  flex-shrink: 0;
}
.stop-pos {
  flex: 1;
  -webkit-appearance: none;
  appearance: none;
  height: 4px;
  background: var(--border);
  border-radius: 2px;
  outline: none;
  cursor: pointer;
}
.stop-pos::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--accent);
  cursor: pointer;
}
.stop-val {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  min-width: 28px;
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.card-divider {
  height: 1px;
  background: var(--border);
  margin: var(--space-3) 0;
}

.row-toggle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-3);
}
.label-sm {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}
.toggle-btn {
  padding: 3px 10px;
  background: var(--bg-button);
  color: var(--text-secondary);
  border: 1px solid var(--border);
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: all var(--transition-fast);
}
.toggle-btn.on {
  background: rgba(99, 102, 241, 0.15);
  color: var(--accent-hover);
  border-color: rgba(99, 102, 241, 0.35);
}

.text-add-btns {
  display: flex;
  gap: var(--space-2);
  padding: var(--space-3);
}
.text-add-btn {
  flex: 1;
  justify-content: center;
}

.text-layer-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-4);
  cursor: pointer;
  transition: background var(--transition-fast);
}
.text-layer-item.selected {
  background: rgba(99, 102, 241, 0.08);
}
.text-layer-item:hover {
  background: var(--bg-card-hover);
}
.tl-type {
  font-weight: 700;
  font-size: var(--text-md);
  color: var(--text-tertiary);
  width: 16px;
}
.tl-text {
  flex: 1;
  font-size: var(--text-sm);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tl-delete {
  background: transparent;
  border: none;
  color: var(--text-tertiary);
  cursor: pointer;
  font-size: var(--text-md);
  line-height: 1;
  padding: 2px 4px;
  border-radius: 4px;
  transition: color var(--transition-fast), background var(--transition-fast);
}
.tl-delete:hover {
  color: var(--danger);
  background: rgba(239, 68, 68, 0.1);
}

.text-editor {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}
.field-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.field-label {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  font-weight: 500;
}
.text-input {
  background: var(--bg-input);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-sm);
  padding: var(--space-2);
  resize: none;
  font-family: var(--font-ui);
  transition: border-color var(--transition-fast);
  width: 100%;
}
.text-input:focus {
  outline: none;
  border-color: var(--border-focus);
}

.select-input {
  -webkit-appearance: none;
  appearance: none;
  background: var(--bg-input);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-sm);
  padding: var(--space-2) 28px var(--space-2) var(--space-2);
  width: 100%;
  cursor: pointer;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='rgba(255,255,255,0.35)' stroke-width='2.5'%3E%3Cpolyline points='6 9 12 15 18 9'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 8px center;
  transition: border-color var(--transition-fast);
}
.select-input:focus {
  outline: none;
  border-color: var(--border-focus);
}

:global(html[data-theme="dark"]) .select-input,
:global(html[data-theme="dark"]) .select-input option {
  background-color: #1e2130;
  color: rgba(255, 255, 255, 0.9);
}
:global(html[data-theme="light"]) .select-input,
:global(html[data-theme="light"]) .select-input option {
  background-color: #ffffff;
  color: #1a1a2e;
}

.export-info {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin-bottom: var(--space-3);
}
.info-row {
  display: flex;
  justify-content: space-between;
  font-size: var(--text-sm);
  color: var(--text-secondary);
}
.info-row span:last-child {
  color: var(--text-primary);
}
.export-sizes {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}
.export-btn {
  width: 100%;
  justify-content: center;
}
.export-btn-original {
  border-style: dashed;
  gap: var(--space-2);
}
.original-size-tag {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  margin-left: auto;
}

.export-hint {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  margin-bottom: var(--space-3);
  line-height: 1.5;
}

.icon-presets {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.icon-preset-btn {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  background: var(--bg-input);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  cursor: pointer;
  text-align: left;
  transition: all var(--transition-fast);
  width: 100%;
}
.icon-preset-btn:hover:not(:disabled) {
  background: var(--bg-card-hover);
  border-color: var(--border-hover);
}
.icon-preset-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.preset-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: var(--radius-sm);
  background: rgba(99, 102, 241, 0.12);
  color: var(--accent-hover);
  flex-shrink: 0;
}

.preset-texts {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.preset-desc {
  font-size: 10px;
  color: var(--text-tertiary);
}

.preset-arrow {
  color: var(--text-disabled);
  flex-shrink: 0;
}
.icon-preset-btn:hover:not(:disabled) .preset-arrow {
  color: var(--accent-hover);
}

.export-status {
  display: flex;
  align-items: flex-start;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  background: rgba(34, 197, 94, 0.08);
  border: 1px solid rgba(34, 197, 94, 0.2);
  border-radius: var(--radius-md);
  font-size: var(--text-xs);
  color: #4ade80;
  line-height: 1.4;
  word-break: break-all;
}
.export-status.error {
  background: rgba(239, 68, 68, 0.08);
  border-color: rgba(239, 68, 68, 0.2);
  color: var(--danger);
}
</style>

<script setup lang="ts">
import { ref } from 'vue'
import { useDropZone } from '@vueuse/core'
import GlassCard from '../ui/GlassCard.vue'
import AiGenPanel from '../ai-gen/AiGenPanel.vue'
import { useCanvasStore } from '../../store/useCanvasStore'
import { useTypographyStore } from '../../store/useTypographyStore'
import { useImageEditor } from '../../modules/image-editor/useImageEditor'
import { useThumbnail } from '../../modules/image-editor/useThumbnail'

const canvasStore = useCanvasStore()
const typoStore = useTypographyStore()
const { importImageFromFile } = useImageEditor()
const { imageThumbnail, textThumbnails } = useThumbnail()

const dropZoneRef = ref<HTMLDivElement | null>(null)
const isDragOver = ref(false)
const activeTab = ref<'import' | 'layers' | 'ai'>('import')

const { isOverDropZone } = useDropZone(dropZoneRef, {
  onOver: () => { isDragOver.value = true },
  onLeave: () => { isDragOver.value = false },
  onDrop: async (files) => {
    isDragOver.value = false
    if (files && files.length > 0) {
      await importImageFromFile(files[0])
      activeTab.value = 'layers'
    }
  },
  dataTypes: ['image/png', 'image/jpeg', 'image/webp', 'image/svg+xml'],
})

async function openFilePicker() {
  await importImageFromFile()
  if (canvasStore.hasImage) activeTab.value = 'layers'
}

function getLayerStatusLabel(layer: typeof canvasStore.imageLayer): string {
  if (!layer) return ''
  if (canvasStore.hasPendingMask) return '预览中'
  if (layer.hasMask) return '已抠图'
  return '原图'
}
</script>

<template>
  <aside class="left-panel panel-scroll">
    <!-- Header tabs -->
    <div class="panel-tabs">
      <button class="panel-tab" :class="{ active: activeTab === 'import' }" @click="activeTab = 'import'">导入</button>
      <button class="panel-tab" :class="{ active: activeTab === 'layers' }" @click="activeTab = 'layers'">
        图层
        <span v-if="canvasStore.hasImage || typoStore.textLayers.length > 0" class="tab-count">
          {{ (canvasStore.hasImage ? 1 : 0) + typoStore.textLayers.length }}
        </span>
      </button>
      <button class="panel-tab panel-tab--ai" :class="{ active: activeTab === 'ai' }" @click="activeTab = 'ai'">
        AI 生成
      </button>
    </div>

    <!-- ── Import Tab ─────────────────────────────────────────────── -->
    <div v-if="activeTab === 'import'" class="tab-content">
      <div
        ref="dropZoneRef"
        class="drop-zone"
        :class="{ 'drag-over': isDragOver || isOverDropZone }"
        @click="openFilePicker"
      >
        <div class="drop-zone__icon">
          <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/>
            <polyline points="17 8 12 3 7 8"/>
            <line x1="12" y1="3" x2="12" y2="15"/>
          </svg>
        </div>
        <p class="drop-zone__text">拖拽图片到此处</p>
        <p class="drop-zone__sub">或点击选择文件</p>
        <p class="drop-zone__formats">PNG · JPG · WebP · SVG</p>
      </div>

      <!-- Current image quick-info -->
      <div v-if="canvasStore.hasImage && canvasStore.imageLayer" class="image-preview">
        <div class="image-preview__thumb-wrap">
          <img
            v-if="imageThumbnail"
            :src="imageThumbnail"
            class="image-preview__thumb"
            alt="preview"
          />
          <div v-else class="image-preview__thumb-placeholder" />
        </div>
        <div class="image-preview__info">
          <span class="image-preview__name">{{ canvasStore.imageLayer.name }}</span>
          <span class="image-preview__size">{{ canvasStore.imageLayer.width }} × {{ canvasStore.imageLayer.height }}</span>
        </div>
        <button class="image-preview__remove" @click="canvasStore.clearImage()" title="移除">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
          </svg>
        </button>
      </div>
    </div>

    <!-- ── AI Generation Tab ──────────────────────────────────────── -->
    <div v-if="activeTab === 'ai'" class="tab-content">
      <AiGenPanel />
    </div>

    <!-- ── Layers Tab ─────────────────────────────────────────────── -->
    <div v-if="activeTab === 'layers'" class="tab-content">

      <!-- Empty state -->
      <div v-if="!canvasStore.hasImage && typoStore.textLayers.length === 0" class="layers-empty">
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2">
          <polygon points="12 2 2 7 12 12 22 7 12 2"/>
          <polyline points="2 17 12 22 22 17"/>
          <polyline points="2 12 12 17 22 12"/>
        </svg>
        <span>暂无图层</span>
        <span class="layers-empty__hint">从"导入"选项卡添加图片</span>
      </div>

      <!-- Image layer -->
      <GlassCard v-if="canvasStore.hasImage && canvasStore.imageLayer" :padding="false">
        <div
          class="layer-row"
          :class="{
            'layer-row--active': canvasStore.imageLayer?.id === canvasStore.activeLayerId,
            'layer-row--pending': canvasStore.hasPendingMask,
          }"
          @click="canvasStore.activeLayerId = canvasStore.imageLayer!.id"
        >
          <!-- Thumbnail -->
          <div class="layer-thumb-wrap">
            <img
              v-if="imageThumbnail"
              :src="imageThumbnail"
              class="layer-thumb"
              alt="thumb"
            />
            <div v-else class="layer-thumb layer-thumb--placeholder">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8.5" cy="8.5" r="1.5"/>
                <polyline points="21 15 16 10 5 21"/>
              </svg>
            </div>
            <!-- Pending pulse dot -->
            <div v-if="canvasStore.hasPendingMask" class="layer-thumb__dot layer-thumb__dot--pending" />
            <!-- Mask indicator dot -->
            <div v-else-if="canvasStore.imageLayer?.hasMask" class="layer-thumb__dot layer-thumb__dot--masked" />
          </div>

          <!-- Info -->
          <div class="layer-info">
            <span class="layer-name">{{ canvasStore.imageLayer.name }}</span>
            <span
              class="layer-status"
              :class="{
                'layer-status--pending': canvasStore.hasPendingMask,
                'layer-status--masked': !canvasStore.hasPendingMask && canvasStore.imageLayer.hasMask,
              }"
            >
              {{ getLayerStatusLabel(canvasStore.imageLayer) }}
            </span>
          </div>

          <!-- Actions -->
          <div class="layer-actions">
            <!-- Visibility toggle -->
            <button
              class="layer-action-btn"
              :title="canvasStore.imageLayer.visible ? '隐藏' : '显示'"
              @click.stop="canvasStore.imageLayer!.visible = !canvasStore.imageLayer!.visible"
            >
              <svg v-if="canvasStore.imageLayer.visible" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/>
              </svg>
              <svg v-else width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M17.94 17.94A10.07 10.07 0 0112 20c-7 0-11-8-11-8a18.45 18.45 0 015.06-5.94"/>
                <path d="M9.9 4.24A9.12 9.12 0 0112 4c7 0 11 8 11 8a18.5 18.5 0 01-2.16 3.19"/>
                <line x1="1" y1="1" x2="23" y2="23"/>
              </svg>
            </button>
            <!-- Delete -->
            <button class="layer-action-btn layer-action-btn--danger" title="删除" @click.stop="canvasStore.clearImage()">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14H6L5 6"/>
                <path d="M10 11v6"/><path d="M14 11v6"/><path d="M9 6V4h6v2"/>
              </svg>
            </button>
          </div>
        </div>
      </GlassCard>

      <!-- Text layers -->
      <GlassCard v-if="typoStore.textLayers.length > 0" title="文字图层" :padding="false">
        <div
          v-for="layer in typoStore.textLayers"
          :key="layer.id"
          class="layer-row"
          :class="{ 'layer-row--active': typoStore.selectedLayerId === layer.id }"
          @click="typoStore.selectLayer(layer.id)"
        >
          <!-- Thumbnail -->
          <div class="layer-thumb-wrap">
            <img
              v-if="textThumbnails[layer.id]"
              :src="textThumbnails[layer.id]"
              class="layer-thumb"
              alt="text thumb"
            />
            <div v-else class="layer-thumb layer-thumb--text">
              <span>{{ layer.type === 'title' ? 'T' : 't' }}</span>
            </div>
          </div>

          <!-- Info -->
          <div class="layer-info">
            <span class="layer-name" :style="{ fontFamily: layer.fontFamily }">
              {{ layer.text || '空文字' }}
            </span>
            <span class="layer-status">{{ layer.type === 'title' ? '标题' : 'Slogan' }} · {{ layer.fontFamily }}</span>
          </div>

          <!-- Actions -->
          <div class="layer-actions">
            <button
              class="layer-action-btn"
              :title="layer.visible ? '隐藏' : '显示'"
              @click.stop="typoStore.updateLayer(layer.id, { visible: !layer.visible })"
            >
              <svg v-if="layer.visible" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/>
              </svg>
              <svg v-else width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="1" y1="1" x2="23" y2="23"/>
                <path d="M17.94 17.94A10.07 10.07 0 0112 20c-7 0-11-8-11-8a18.45 18.45 0 015.06-5.94"/>
              </svg>
            </button>
            <button class="layer-action-btn layer-action-btn--danger" title="删除" @click.stop="typoStore.removeLayer(layer.id)">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14H6L5 6"/>
                <path d="M10 11v6"/><path d="M14 11v6"/><path d="M9 6V4h6v2"/>
              </svg>
            </button>
          </div>
        </div>
      </GlassCard>
    </div>
  </aside>
</template>

<style scoped>
.left-panel {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-3);
  background: var(--bg-panel);
  border-right: 1px solid var(--border);
}

/* ── Tabs ─────────────────────────────────────────────────────────── */
.panel-tabs {
  display: flex;
  gap: var(--space-1);
  background: var(--bg-input);
  border-radius: var(--radius-md);
  padding: 3px;
  flex-shrink: 0;
}

.panel-tab {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  padding: 6px;
  background: transparent;
  color: var(--text-tertiary);
  border: none;
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
  font-weight: 500;
  cursor: pointer;
  transition: background var(--transition-fast), color var(--transition-fast);
}
.panel-tab.active {
  background: var(--bg-card-hover);
  color: var(--text-primary);
}

.tab-count {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 16px;
  height: 16px;
  padding: 0 4px;
  background: rgba(99, 102, 241, 0.2);
  color: var(--accent-hover);
  border-radius: var(--radius-full);
  font-size: 10px;
  font-weight: 700;
}

.tab-content {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

/* ── Drop zone ────────────────────────────────────────────────────── */
.drop-zone {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  padding: var(--space-6) var(--space-4);
  background: var(--bg-card);
  border: 1.5px dashed var(--border);
  border-radius: var(--radius-lg);
  cursor: pointer;
  transition: border-color var(--transition-fast), background var(--transition-fast);
  min-height: 160px;
}
.drop-zone:hover, .drop-zone.drag-over {
  border-color: var(--accent);
  background: rgba(99, 102, 241, 0.06);
}
.drop-zone.drag-over { box-shadow: inset 0 0 0 1px var(--accent), var(--shadow-glow); }
.drop-zone__icon { color: var(--text-tertiary); transition: color var(--transition-fast); }
.drop-zone:hover .drop-zone__icon, .drop-zone.drag-over .drop-zone__icon { color: var(--accent); }
.drop-zone__text { font-size: var(--text-sm); font-weight: 500; color: var(--text-secondary); }
.drop-zone__sub { font-size: var(--text-xs); color: var(--text-tertiary); }
.drop-zone__formats { font-size: var(--text-xs); color: var(--text-disabled); letter-spacing: 0.05em; }

/* ── Import tab image-preview ─────────────────────────────────────── */
.image-preview {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-3);
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
}
.image-preview__thumb-wrap {
  width: 44px;
  height: 44px;
  border-radius: var(--radius-sm);
  overflow: hidden;
  flex-shrink: 0;
  background: #888;
}
.image-preview__thumb {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.image-preview__thumb-placeholder {
  width: 100%;
  height: 100%;
  background: var(--bg-input);
}
.image-preview__info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex: 1;
  min-width: 0;
}
.image-preview__name {
  font-size: var(--text-sm);
  color: var(--text-primary);
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.image-preview__size { font-size: var(--text-xs); color: var(--text-tertiary); }
.image-preview__remove {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  background: transparent;
  color: var(--text-tertiary);
  border: none;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: color var(--transition-fast), background var(--transition-fast);
}
.image-preview__remove:hover { color: var(--danger); background: rgba(239,68,68,0.1); }

/* ── Layers empty ─────────────────────────────────────────────────── */
.layers-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-8) var(--space-4);
  color: var(--text-disabled);
  font-size: var(--text-sm);
  text-align: center;
}
.layers-empty__hint { font-size: var(--text-xs); color: var(--text-disabled); }

/* ── Layer row ────────────────────────────────────────────────────── */
.layer-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  cursor: pointer;
  border-radius: var(--radius-sm);
  transition: background var(--transition-fast);
  position: relative;
}
.layer-row:hover { background: var(--bg-card-hover); }
.layer-row--active { background: rgba(99, 102, 241, 0.08); }
.layer-row--pending { background: rgba(99, 102, 241, 0.06); }

/* Thumbnail container */
.layer-thumb-wrap {
  position: relative;
  width: 40px;
  height: 40px;
  flex-shrink: 0;
}

.layer-thumb {
  width: 40px;
  height: 40px;
  border-radius: var(--radius-sm);
  object-fit: cover;
  border: 1px solid var(--border);
  display: block;
}

.layer-thumb--placeholder,
.layer-thumb--text {
  width: 40px;
  height: 40px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  background: var(--bg-input);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-tertiary);
  font-size: var(--text-md);
  font-weight: 700;
}

/* Status dot overlays on thumbnail */
.layer-thumb__dot {
  position: absolute;
  bottom: 2px;
  right: 2px;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  border: 1.5px solid var(--bg-panel);
}
.layer-thumb__dot--masked { background: var(--success); }
.layer-thumb__dot--pending {
  background: var(--accent);
  box-shadow: 0 0 4px var(--accent-glow);
  animation: pulse-dot 1.2s ease-in-out infinite;
}

@keyframes pulse-dot {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.6; transform: scale(0.8); }
}

/* Layer info */
.layer-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex: 1;
  min-width: 0;
}

.layer-name {
  font-size: var(--text-sm);
  color: var(--text-primary);
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.layer-status {
  font-size: 10px;
  color: var(--text-disabled);
}
.layer-status--pending { color: var(--accent-hover); }
.layer-status--masked { color: var(--success); }

/* Layer action buttons */
.layer-actions {
  display: flex;
  gap: 2px;
  opacity: 0;
  transition: opacity var(--transition-fast);
}
.layer-row:hover .layer-actions { opacity: 1; }

.layer-action-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  background: transparent;
  color: var(--text-tertiary);
  border: none;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: color var(--transition-fast), background var(--transition-fast);
}
.layer-action-btn:hover { color: var(--text-primary); background: var(--bg-button-hover); }
.layer-action-btn--danger:hover { color: var(--danger); background: rgba(239,68,68,0.1); }

/* AI tab accent */
.panel-tab--ai.active {
  background: rgba(99,102,241,0.15);
  color: var(--accent-hover);
}
</style>

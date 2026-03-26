<script setup lang="ts">
import { computed, ref, watch, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppStore } from './store/useAppStore'
import { localeOptions } from './i18n'
import ThemeToggle from './components/ui/ThemeToggle.vue'
import LeftPanel from './components/layout/LeftPanel.vue'
import CenterCanvas from './components/layout/CenterCanvas.vue'
import RightPanel from './components/layout/RightPanel.vue'
import ResizeHandle from './components/ui/ResizeHandle.vue'
import { useHistoryStore } from './store/useHistoryStore'
import { useHistory } from './modules/history/useHistory'
import { useCanvasStore } from './store/useCanvasStore'
import { useBackgroundStore } from './store/useBackgroundStore'
import { useTypographyStore } from './store/useTypographyStore'
import { useProject } from './modules/project/useProject'

const app = useAppStore()
const { t } = useI18n()

const nextLocale = computed(() => {
  const currentIndex = localeOptions.findIndex((option) => option.code === app.locale)
  return localeOptions[(currentIndex + 1) % localeOptions.length] ?? localeOptions[0]
})

// ── Undo / Redo ─────────────────────────────────────────────────────────────
const historyStore = useHistoryStore()
const { undo, redo, initHistory, snapshot } = useHistory()
const canvasStore = useCanvasStore()
const bgStore = useBackgroundStore()
const typoStore = useTypographyStore()

// Debounced auto-snapshot on any store change
let _snapTimer: ReturnType<typeof setTimeout> | null = null
let _isRestoring = false

function scheduleSnapshot() {
  if (_isRestoring) return
  if (_snapTimer) clearTimeout(_snapTimer)
  _snapTimer = setTimeout(() => {
    snapshot()
  }, 300)
}

onMounted(() => {
  initHistory()

  // Watch all relevant state for changes
  watch(
    () => JSON.stringify({
      img: canvasStore.imageLayer ? {
        id: canvasStore.imageLayer.id,
        x: canvasStore.imageLayer.x,
        y: canvasStore.imageLayer.y,
        width: canvasStore.imageLayer.width,
        height: canvasStore.imageLayer.height,
        opacity: canvasStore.imageLayer.opacity,
        brightness: canvasStore.imageLayer.brightness,
        contrast: canvasStore.imageLayer.contrast,
        saturation: canvasStore.imageLayer.saturation,
        hasMask: canvasStore.imageLayer.hasMask,
      } : null,
      bg: {
        bgType: bgStore.bgType,
        stops: bgStore.stops,
        angle: bgStore.angle,
        solidColor: bgStore.solidColor,
        borderRadius: bgStore.borderRadius,
        shadowEnabled: bgStore.shadowEnabled,
        shadowBlur: bgStore.shadowBlur,
        shadowOffsetY: bgStore.shadowOffsetY,
        innerGlow: bgStore.innerGlow,
      },
      ty: typoStore.textLayers,
    }),
    () => scheduleSnapshot(),
    { deep: false }
  )
})

function doUndo() {
  _isRestoring = true
  undo()
  setTimeout(() => { _isRestoring = false }, 50)
}

function doRedo() {
  _isRestoring = true
  redo()
  setTimeout(() => { _isRestoring = false }, 50)
}

function handleKeyDown(e: KeyboardEvent) {
  const ctrl = e.ctrlKey || e.metaKey
  if (!ctrl) return
  if (e.key === 'z' && !e.shiftKey) {
    e.preventDefault()
    doUndo()
  } else if (e.key === 'y' || (e.key === 'z' && e.shiftKey)) {
    e.preventDefault()
    doRedo()
  }
}

onMounted(() => window.addEventListener('keydown', handleKeyDown))
onUnmounted(() => window.removeEventListener('keydown', handleKeyDown))

// ── Project save/load ────────────────────────────────────────────────────────
const { saveProject, openProject, newProject, currentFilePath } = useProject()

// Ctrl+S / Ctrl+Shift+S / Ctrl+O
onMounted(() => window.addEventListener('keydown', handleProjectKeys))
onUnmounted(() => window.removeEventListener('keydown', handleProjectKeys))

function handleProjectKeys(e: KeyboardEvent) {
  const ctrl = e.ctrlKey || e.metaKey
  if (!ctrl) return
  if (e.key === 's') {
    e.preventDefault()
    saveProject(e.shiftKey)
  } else if (e.key === 'o') {
    e.preventDefault()
    openProject()
  }
}

// ── Panel resize ────────────────────────────────────────────────────────────
const LEFT_MIN = 180
const LEFT_MAX = 480
const RIGHT_MIN = 220
const RIGHT_MAX = 520

const leftWidth = ref(260)
const rightWidth = ref(300)

function onLeftDrag(delta: number) {
  leftWidth.value = Math.max(LEFT_MIN, Math.min(LEFT_MAX, leftWidth.value + delta))
}

function onRightDrag(delta: number) {
  rightWidth.value = Math.max(RIGHT_MIN, Math.min(RIGHT_MAX, rightWidth.value - delta))
}

// ── Toast 复制功能 ──────────────────────────────────────────
const toastCopied = ref(false)
let _copyResetTimer: ReturnType<typeof setTimeout> | null = null

function handleToastClick() {
  if (app.toastType === 'error' && app.toastMessage) {
    navigator.clipboard.writeText(app.toastMessage).then(() => {
      toastCopied.value = true
      if (_copyResetTimer) clearTimeout(_copyResetTimer)
      _copyResetTimer = setTimeout(() => {
        toastCopied.value = false
      }, 1500)
    })
  } else {
    app.dismissToast()
  }
}
</script>

<template>
  <div class="app-shell">
    <header class="title-bar" data-tauri-drag-region>
      <div class="title-bar__left">
        <div class="app-logo">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none">
            <defs>
              <linearGradient id="logoGrad" x1="0%" y1="0%" x2="100%" y2="100%">
                <stop offset="0%" style="stop-color:#6366f1"/>
                <stop offset="100%" style="stop-color:#8b5cf6"/>
              </linearGradient>
            </defs>
            <rect x="2" y="2" width="20" height="20" rx="6" fill="url(#logoGrad)"/>
            <path d="M7 16L12 8l5 8" stroke="white" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
            <path d="M9 13h6" stroke="white" stroke-width="1.5" stroke-linecap="round"/>
          </svg>
        </div>
        <span class="app-name">{{ t('app.title') }}</span>
        <span class="app-badge">v{{ __APP_VERSION__ }}</span>

        <div class="project-btns">
          <button class="btn-ghost proj-btn" title="新建项目" @click="newProject()">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/>
              <polyline points="14 2 14 8 20 8"/>
              <line x1="12" y1="18" x2="12" y2="12"/>
              <line x1="9" y1="15" x2="15" y2="15"/>
            </svg>
            新建
          </button>
          <button class="btn-ghost proj-btn" title="打开项目 (Ctrl+O)" @click="openProject()">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z"/>
            </svg>
            打开
          </button>
          <button class="btn-ghost proj-btn" title="保存项目 (Ctrl+S)" @click="saveProject(false)">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M19 21H5a2 2 0 01-2-2V5a2 2 0 012-2h11l5 5v11a2 2 0 01-2 2z"/>
              <polyline points="17 21 17 13 7 13 7 21"/>
              <polyline points="7 3 7 8 15 8"/>
            </svg>
            保存
          </button>
          <span v-if="currentFilePath" class="proj-filename" :title="currentFilePath">
            {{ currentFilePath.split(/[\\/]/).pop() }}
          </span>
        </div>
      </div>

      <div class="title-bar__actions">
        <div class="history-btns">
          <button
            class="btn-ghost icon-btn"
            :disabled="!historyStore.canUndo"
            title="撤销 (Ctrl+Z)"
            @click="doUndo()"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M3 7v6h6"/><path d="M21 17a9 9 0 00-9-9 9 9 0 00-6 2.3L3 13"/>
            </svg>
          </button>
          <button
            class="btn-ghost icon-btn"
            :disabled="!historyStore.canRedo"
            title="重做 (Ctrl+Y)"
            @click="doRedo()"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M21 7v6h-6"/><path d="M3 17a9 9 0 019-9 9 9 0 016 2.3l3 2.7"/>
            </svg>
          </button>
        </div>
        <ThemeToggle />
        <button
          class="btn-ghost lang-btn"
          :title="t('app.switchLanguageTo', { language: nextLocale.label })"
          @click="app.setLocale(nextLocale.code)"
          style="font-size: 11px; padding: 5px 8px; height: 32px;"
        >
          {{ nextLocale.code.toUpperCase() }}
        </button>
      </div>
    </header>

    <div class="workspace">
      <div class="panel-wrap" :style="{ width: leftWidth + 'px' }">
        <LeftPanel />
      </div>
      <ResizeHandle @drag="onLeftDrag" />
      <div class="panel-center">
        <CenterCanvas />
      </div>
      <ResizeHandle @drag="onRightDrag" />
      <div class="panel-wrap" :style="{ width: rightWidth + 'px' }">
        <RightPanel />
      </div>
    </div>

    <Transition name="fade">
      <div v-if="app.isLoading" class="loading-overlay">
        <div class="loading-spinner" />
        <p class="loading-text">{{ app.loadingText || t('app.loading.processing') }}</p>
      </div>
    </Transition>

    <Transition name="toast-slide">
      <div
        v-if="app.toastMessage"
        class="toast"
        :class="[`toast--${app.toastType}`, { 'toast--copied': toastCopied }]"
        :title="app.toastType === 'error' ? (toastCopied ? '已复制' : '点击复制错误信息') : '点击关闭'"
        @click="handleToastClick"
      >
        <span class="toast__icon">
          <template v-if="toastCopied">✓</template>
          <template v-else-if="app.toastType === 'warn'">!</template>
          <template v-else-if="app.toastType === 'error'">×</template>
          <template v-else>i</template>
        </span>
        <span class="toast__msg">{{ toastCopied ? '已复制到剪贴板' : app.toastMessage }}</span>
        <span v-if="app.toastType === 'error' && !toastCopied" class="toast__copy-hint">点击复制</span>
      </div>
    </Transition>
  </div>
</template>

<style>
.app-shell {
  width: 100vw;
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: var(--bg-app);
  overflow: hidden;
}

.title-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 44px;
  padding: 0 var(--space-4);
  background: var(--bg-panel);
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
  position: relative;
  z-index: var(--z-panel);
}

.title-bar__left {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.app-logo {
  display: flex;
  align-items: center;
  flex-shrink: 0;
}

.app-name {
  font-size: var(--text-sm);
  font-weight: 700;
  color: var(--text-primary);
  letter-spacing: 0.02em;
}

.app-badge {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  background: var(--bg-input);
  border: 1px solid var(--border);
  border-radius: var(--radius-full);
  padding: 1px 6px;
}

.title-bar__actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.history-btns {
  display: flex;
  align-items: center;
  gap: 2px;
  border-right: 1px solid var(--border);
  padding-right: var(--space-2);
  margin-right: 2px;
}

.icon-btn {
  width: 30px;
  height: 30px;
  padding: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}
.icon-btn:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}

.project-btns {
  display: flex;
  align-items: center;
  gap: 2px;
  border-left: 1px solid var(--border);
  padding-left: var(--space-2);
  margin-left: 4px;
}

.proj-btn {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  padding: 4px 8px;
  height: 28px;
  white-space: nowrap;
}

.proj-filename {
  font-size: 11px;
  color: var(--text-tertiary);
  max-width: 140px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  padding: 0 4px;
  border-left: 1px solid var(--border);
  margin-left: 4px;
}

.workspace {
  flex: 1;
  display: flex;
  flex-direction: row;
  overflow: hidden;
  min-height: 0;
}

.panel-wrap {
  flex-shrink: 0;
  height: 100%;
  overflow: hidden;
  min-width: 0;
}

.panel-center {
  flex: 1;
  height: 100%;
  overflow: hidden;
  min-width: 400px;
}

.loading-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-4);
  background: rgba(13, 15, 20, 0.8);
  backdrop-filter: blur(8px);
}

.loading-spinner {
  width: 36px;
  height: 36px;
  border: 3px solid var(--border);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

.loading-text {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity var(--transition-normal);
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.toast {
  position: fixed;
  bottom: 24px;
  left: 50%;
  transform: translateX(-50%);
  z-index: calc(var(--z-modal) + 1);
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 10px 18px;
  border-radius: var(--radius-lg);
  font-size: var(--text-sm);
  cursor: pointer;
  box-shadow: 0 4px 20px rgba(0,0,0,0.4);
  max-width: 560px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  transition: border-color 0.2s, background 0.2s;
}
/* error toast 允许多行展示完整错误 */
.toast--error {
  white-space: normal;
  word-break: break-all;
  align-items: flex-start;
}
.toast--info  { background: var(--bg-panel); border: 1px solid var(--border); color: var(--text-primary); }
.toast--warn  { background: #3d2e0e; border: 1px solid #a16207; color: #fbbf24; }
.toast--error { background: #2d1212; border: 1px solid #991b1b; color: #f87171; }
.toast--error:hover { background: #3a1515; border-color: #b91c1c; }
.toast--copied { background: #122d12 !important; border-color: #16a34a !important; color: #4ade80 !important; }

.toast__icon { font-size: 14px; flex-shrink: 0; margin-top: 1px; }
.toast__msg  { flex: 1; overflow: hidden; text-overflow: ellipsis; line-height: 1.5; }
.toast__copy-hint {
  flex-shrink: 0;
  font-size: 11px;
  opacity: 0.55;
  border: 1px solid currentColor;
  border-radius: 4px;
  padding: 1px 5px;
  margin-left: 4px;
  white-space: nowrap;
  align-self: flex-start;
  margin-top: 1px;
}

.toast-slide-enter-active,
.toast-slide-leave-active {
  transition: opacity 0.25s, transform 0.25s;
}
.toast-slide-enter-from,
.toast-slide-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(12px);
}
</style>

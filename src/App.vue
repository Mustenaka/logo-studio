<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppStore } from './store/useAppStore'
import { localeOptions } from './i18n'
import ThemeToggle from './components/ui/ThemeToggle.vue'
import LeftPanel from './components/layout/LeftPanel.vue'
import CenterCanvas from './components/layout/CenterCanvas.vue'
import RightPanel from './components/layout/RightPanel.vue'

const app = useAppStore()
const { t } = useI18n()

const nextLocale = computed(() => {
  const currentIndex = localeOptions.findIndex((option) => option.code === app.locale)
  return localeOptions[(currentIndex + 1) % localeOptions.length] ?? localeOptions[0]
})
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
        <span class="app-badge">v0.1</span>
      </div>

      <div class="title-bar__actions">
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
      <LeftPanel />
      <CenterCanvas />
      <RightPanel />
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
        :class="`toast--${app.toastType}`"
        @click="app.dismissToast()"
      >
        <span class="toast__icon">
          <template v-if="app.toastType === 'warn'">!</template>
          <template v-else-if="app.toastType === 'error'">×</template>
          <template v-else>i</template>
        </span>
        <span class="toast__msg">{{ app.toastMessage }}</span>
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

.workspace {
  flex: 1;
  display: grid;
  grid-template-columns: 260px 1fr 300px;
  overflow: hidden;
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
  align-items: center;
  gap: 8px;
  padding: 10px 18px;
  border-radius: var(--radius-lg);
  font-size: var(--text-sm);
  cursor: pointer;
  box-shadow: 0 4px 20px rgba(0,0,0,0.4);
  max-width: 480px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.toast--info  { background: var(--bg-panel); border: 1px solid var(--border); color: var(--text-primary); }
.toast--warn  { background: #3d2e0e; border: 1px solid #a16207; color: #fbbf24; }
.toast--error { background: #2d1212; border: 1px solid #991b1b; color: #f87171; }

.toast__icon { font-size: 14px; flex-shrink: 0; }
.toast__msg  { flex: 1; overflow: hidden; text-overflow: ellipsis; }

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

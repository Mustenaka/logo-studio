import { defineStore } from 'pinia'
import { ref, watch } from 'vue'

export type Theme = 'dark' | 'light'
export type Locale = 'zh' | 'en'

export const useAppStore = defineStore('app', () => {
  const theme = ref<Theme>('dark')
  const locale = ref<Locale>('zh')
  const isLoading = ref(false)
  const loadingText = ref('')
  const toastMessage = ref('')
  const toastType = ref<'info' | 'warn' | 'error'>('info')
  let _toastTimer: ReturnType<typeof setTimeout> | null = null

  function setTheme(t: Theme) {
    theme.value = t
  }

  function toggleTheme() {
    theme.value = theme.value === 'dark' ? 'light' : 'dark'
  }

  function setLocale(l: Locale) {
    locale.value = l
  }

  function setLoading(loading: boolean, text = '') {
    isLoading.value = loading
    loadingText.value = text
  }

  function showToast(message: string, type: 'info' | 'warn' | 'error' = 'info', duration = 4000) {
    if (_toastTimer) clearTimeout(_toastTimer)
    toastMessage.value = message
    toastType.value = type
    _toastTimer = setTimeout(() => { toastMessage.value = '' }, duration)
  }

  function dismissToast() {
    if (_toastTimer) clearTimeout(_toastTimer)
    toastMessage.value = ''
  }

  // Apply theme to <html>
  watch(theme, (t) => {
    document.documentElement.setAttribute('data-theme', t)
  }, { immediate: true })

  return {
    theme,
    locale,
    isLoading,
    loadingText,
    toastMessage,
    toastType,
    setTheme,
    toggleTheme,
    setLocale,
    setLoading,
    showToast,
    dismissToast,
  }
})

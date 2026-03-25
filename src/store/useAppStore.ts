import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import { defaultLocale, i18n, localeOptions, type LocaleCode } from '../i18n'

export type Theme = 'dark' | 'light'
export type Locale = LocaleCode

const localeSet = new Set(localeOptions.map((option) => option.code))

function getStoredTheme(): Theme {
  if (typeof window === 'undefined') return 'dark'
  const saved = window.localStorage.getItem('ls-theme')
  return saved === 'light' ? 'light' : 'dark'
}

function getStoredLocale(): Locale {
  if (typeof window === 'undefined') return defaultLocale
  const saved = window.localStorage.getItem('ls-locale')
  return saved && localeSet.has(saved as Locale) ? (saved as Locale) : defaultLocale
}

export const useAppStore = defineStore('app', () => {
  const theme = ref<Theme>(getStoredTheme())
  const locale = ref<Locale>(getStoredLocale())
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
    if (typeof document === 'undefined') return
    document.documentElement.setAttribute('data-theme', t)
    window.localStorage.setItem('ls-theme', t)
  }, { immediate: true })

  watch(locale, (value) => {
    i18n.global.locale.value = value
    if (typeof window !== 'undefined') {
      window.localStorage.setItem('ls-locale', value)
    }
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

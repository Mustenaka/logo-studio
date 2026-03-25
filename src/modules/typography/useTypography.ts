import { ref, onMounted } from 'vue'
import type { FontInfo } from '../../store/useTypographyStore'

/**
 * Manages dynamic font loading for the typography system.
 * Loads fonts from Google Fonts CDN and registers them for canvas use.
 */
export function useTypography() {
  const loadedFonts = ref<Set<string>>(new Set())
  const isLoading = ref(false)

  const GOOGLE_FONTS_TO_LOAD: string[] = [
    'Inter:wght@400;500;600;700;800',
    'Poppins:wght@400;600;700;800;900',
    'Montserrat:wght@400;600;700;800;900',
    'Space+Grotesk:wght@400;500;600;700',
    'Orbitron:wght@400;500;600;700;800;900',
    'Exo+2:wght@400;500;600;700;800',
  ]

  async function loadAllFonts() {
    isLoading.value = true
    try {
      await loadGoogleFonts(GOOGLE_FONTS_TO_LOAD)
      GOOGLE_FONTS_TO_LOAD.forEach(f => {
        const family = f.split(':')[0].replace('+', ' ')
        loadedFonts.value.add(family)
      })
    } catch (e) {
      console.warn('Font load error (offline?):', e)
    } finally {
      isLoading.value = false
    }
  }

  async function loadFont(fontInfo: FontInfo) {
    if (loadedFonts.value.has(fontInfo.family)) return
    try {
      const weights = fontInfo.variants.join(';')
      const family = fontInfo.family.replace(/ /g, '+')
      await loadGoogleFonts([`${family}:wght@${weights}`])
      loadedFonts.value.add(fontInfo.family)
    } catch (e) {
      console.warn(`Failed to load font ${fontInfo.family}:`, e)
    }
  }

  onMounted(() => {
    loadAllFonts()
  })

  return { loadedFonts, isLoading, loadAllFonts, loadFont }
}

function loadGoogleFonts(fonts: string[]): Promise<void> {
  return new Promise((resolve) => {
    const existing = document.getElementById('google-fonts-link')
    if (existing) {
      resolve()
      return
    }

    const families = fonts.map(f => `family=${f}`).join('&')
    const url = `https://fonts.googleapis.com/css2?${families}&display=swap`

    const link = document.createElement('link')
    link.id = 'google-fonts-link'
    link.rel = 'stylesheet'
    link.href = url
    link.onload = () => resolve()
    link.onerror = () => {
      console.warn('Google Fonts unavailable, using system fonts')
      resolve() // Don't fail — fallback to system fonts
    }
    document.head.appendChild(link)
  })
}

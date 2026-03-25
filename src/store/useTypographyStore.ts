import { defineStore } from 'pinia'
import { ref } from 'vue'

export interface TextLayer {
  id: string
  type: 'title' | 'slogan'
  text: string
  fontFamily: string
  fontSize: number
  fontWeight: number
  color: string
  opacity: number
  x: number
  y: number
  visible: boolean
  letterSpacing: number
  lineHeight: number
  align: 'left' | 'center' | 'right'
  shadow: boolean
  shadowColor: string
  shadowBlur: number
  shadowOffsetX: number
  shadowOffsetY: number
}

export interface FontInfo {
  family: string
  category: 'sans' | 'display' | 'tech' | 'serif'
  variants: string[]
}

const DEFAULT_FONTS: FontInfo[] = [
  { family: 'Inter', category: 'sans', variants: ['400', '500', '600', '700', '800'] },
  { family: 'Poppins', category: 'display', variants: ['400', '600', '700', '800', '900'] },
  { family: 'Montserrat', category: 'display', variants: ['400', '600', '700', '800', '900'] },
  { family: 'Space Grotesk', category: 'tech', variants: ['400', '500', '600', '700'] },
  { family: 'Orbitron', category: 'tech', variants: ['400', '500', '600', '700', '800', '900'] },
  { family: 'Exo 2', category: 'tech', variants: ['400', '500', '600', '700', '800'] },
]

export const useTypographyStore = defineStore('typography', () => {
  const textLayers = ref<TextLayer[]>([])
  const activeFontFamily = ref('Inter')
  const availableFonts = ref<FontInfo[]>(DEFAULT_FONTS)
  const selectedLayerId = ref<string | null>(null)

  function addTitleLayer() {
    const id = `title-${Date.now()}`
    textLayers.value.push({
      id,
      type: 'title',
      text: 'LOGO',
      fontFamily: 'Poppins',
      fontSize: 72,
      fontWeight: 800,
      color: '#ffffff',
      opacity: 1,
      x: 400,
      y: 520,
      visible: true,
      letterSpacing: 4,
      lineHeight: 1.2,
      align: 'center',
      shadow: true,
      shadowColor: 'rgba(0,0,0,0.4)',
      shadowBlur: 12,
      shadowOffsetX: 0,
      shadowOffsetY: 4,
    })
    selectedLayerId.value = id
    return id
  }

  function addSloganLayer() {
    const id = `slogan-${Date.now()}`
    textLayers.value.push({
      id,
      type: 'slogan',
      text: 'Your Tagline Here',
      fontFamily: 'Inter',
      fontSize: 22,
      fontWeight: 500,
      color: 'rgba(255,255,255,0.75)',
      opacity: 1,
      x: 400,
      y: 600,
      visible: true,
      letterSpacing: 2,
      lineHeight: 1.5,
      align: 'center',
      shadow: false,
      shadowColor: 'rgba(0,0,0,0.3)',
      shadowBlur: 8,
      shadowOffsetX: 0,
      shadowOffsetY: 2,
    })
    selectedLayerId.value = id
    return id
  }

  function updateLayer(id: string, updates: Partial<TextLayer>) {
    const idx = textLayers.value.findIndex(l => l.id === id)
    if (idx !== -1) {
      textLayers.value[idx] = { ...textLayers.value[idx], ...updates }
    }
  }

  function removeLayer(id: string) {
    textLayers.value = textLayers.value.filter(l => l.id !== id)
    if (selectedLayerId.value === id) selectedLayerId.value = null
  }

  function selectLayer(id: string | null) {
    selectedLayerId.value = id
  }

  function getSelectedLayer(): TextLayer | null {
    return textLayers.value.find(l => l.id === selectedLayerId.value) ?? null
  }

  return {
    textLayers,
    activeFontFamily,
    availableFonts,
    selectedLayerId,
    addTitleLayer,
    addSloganLayer,
    updateLayer,
    removeLayer,
    selectLayer,
    getSelectedLayer,
  }
})

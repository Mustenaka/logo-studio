import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

export type BgType = 'solid' | 'linear' | 'radial' | 'none'

export interface GradientStop {
  color: string
  position: number  // 0~100
}

export interface BackgroundPreset {
  id: string
  name: string
  type: BgType
  stops: GradientStop[]
  angle: number
}

export const PRESETS: BackgroundPreset[] = [
  // ── Dark / neutral ──────────────────────────────────────────────────────────
  {
    id: 'midnight',
    name: 'Midnight',
    type: 'linear',
    stops: [{ color: '#0F172A', position: 0 }, { color: '#1E293B', position: 100 }],
    angle: 160,
  },
  {
    id: 'charcoal',
    name: 'Charcoal',
    type: 'linear',
    stops: [{ color: '#111827', position: 0 }, { color: '#374151', position: 100 }],
    angle: 135,
  },
  {
    id: 'space',
    name: 'Deep Space',
    type: 'radial',
    stops: [{ color: '#1e1b4b', position: 0 }, { color: '#0c0a1a', position: 100 }],
    angle: 0,
  },
  {
    id: 'dark-mesh',
    name: 'Dark Mesh',
    type: 'linear',
    stops: [{ color: '#0f0c29', position: 0 }, { color: '#302b63', position: 50 }, { color: '#24243e', position: 100 }],
    angle: 135,
  },
  // ── Blue / cyan ─────────────────────────────────────────────────────────────
  {
    id: 'ios',
    name: 'iOS Blue',
    type: 'linear',
    stops: [{ color: '#7C3AED', position: 0 }, { color: '#3B82F6', position: 100 }],
    angle: 135,
  },
  {
    id: 'android',
    name: 'Android Teal',
    type: 'linear',
    stops: [{ color: '#2563EB', position: 0 }, { color: '#06B6D4', position: 100 }],
    angle: 135,
  },
  {
    id: 'ocean',
    name: 'Ocean',
    type: 'radial',
    stops: [{ color: '#0891B2', position: 0 }, { color: '#0D0F14', position: 100 }],
    angle: 0,
  },
  {
    id: 'arctic',
    name: 'Arctic',
    type: 'linear',
    stops: [{ color: '#e0f2fe', position: 0 }, { color: '#7dd3fc', position: 50 }, { color: '#38bdf8', position: 100 }],
    angle: 135,
  },
  // ── Purple / pink ───────────────────────────────────────────────────────────
  {
    id: 'neon',
    name: 'Neon Glow',
    type: 'linear',
    stops: [{ color: '#EC4899', position: 0 }, { color: '#8B5CF6', position: 100 }],
    angle: 135,
  },
  {
    id: 'aurora',
    name: 'Aurora',
    type: 'linear',
    stops: [{ color: '#6366f1', position: 0 }, { color: '#a855f7', position: 50 }, { color: '#ec4899', position: 100 }],
    angle: 120,
  },
  {
    id: 'lavender',
    name: 'Lavender',
    type: 'linear',
    stops: [{ color: '#c4b5fd', position: 0 }, { color: '#818cf8', position: 100 }],
    angle: 135,
  },
  {
    id: 'candy',
    name: 'Candy',
    type: 'linear',
    stops: [{ color: '#f9a8d4', position: 0 }, { color: '#fbcfe8', position: 50 }, { color: '#c084fc', position: 100 }],
    angle: 135,
  },
  // ── Warm / orange / red ─────────────────────────────────────────────────────
  {
    id: 'sunset',
    name: 'Sunset',
    type: 'linear',
    stops: [{ color: '#F97316', position: 0 }, { color: '#EF4444', position: 50 }, { color: '#EC4899', position: 100 }],
    angle: 135,
  },
  {
    id: 'fire',
    name: 'Fire',
    type: 'linear',
    stops: [{ color: '#fbbf24', position: 0 }, { color: '#f97316', position: 50 }, { color: '#dc2626', position: 100 }],
    angle: 135,
  },
  {
    id: 'rose',
    name: 'Rose',
    type: 'linear',
    stops: [{ color: '#fda4af', position: 0 }, { color: '#fb7185', position: 100 }],
    angle: 135,
  },
  {
    id: 'gold',
    name: 'Gold',
    type: 'linear',
    stops: [{ color: '#fbbf24', position: 0 }, { color: '#d97706', position: 100 }],
    angle: 135,
  },
  // ── Green / nature ──────────────────────────────────────────────────────────
  {
    id: 'emerald',
    name: 'Emerald',
    type: 'linear',
    stops: [{ color: '#059669', position: 0 }, { color: '#0D9488', position: 100 }],
    angle: 135,
  },
  {
    id: 'forest',
    name: 'Forest',
    type: 'linear',
    stops: [{ color: '#166534', position: 0 }, { color: '#15803d', position: 50 }, { color: '#4ade80', position: 100 }],
    angle: 135,
  },
  {
    id: 'mint',
    name: 'Mint',
    type: 'linear',
    stops: [{ color: '#6ee7b7', position: 0 }, { color: '#34d399', position: 100 }],
    angle: 135,
  },
  // ── Light / pastel ──────────────────────────────────────────────────────────
  {
    id: 'pearl',
    name: 'Pearl',
    type: 'linear',
    stops: [{ color: '#f8fafc', position: 0 }, { color: '#e2e8f0', position: 100 }],
    angle: 135,
  },
  {
    id: 'peach',
    name: 'Peach',
    type: 'linear',
    stops: [{ color: '#fff7ed', position: 0 }, { color: '#fed7aa', position: 100 }],
    angle: 135,
  },
  // ── Solid ───────────────────────────────────────────────────────────────────
  {
    id: 'pure-black',
    name: 'Black',
    type: 'solid' as BgType,
    stops: [{ color: '#000000', position: 0 }],
    angle: 0,
  },
  {
    id: 'pure-white',
    name: 'White',
    type: 'solid' as BgType,
    stops: [{ color: '#ffffff', position: 0 }],
    angle: 0,
  },
  // ── Special ─────────────────────────────────────────────────────────────────
  {
    id: 'transparent',
    name: 'Transparent',
    type: 'none',
    stops: [],
    angle: 0,
  },
]

export const useBackgroundStore = defineStore('background', () => {
  const bgType = ref<BgType>('linear')
  const stops = ref<GradientStop[]>([
    { color: '#7C3AED', position: 0 },
    { color: '#3B82F6', position: 100 },
  ])
  const angle = ref(135)
  const solidColor = ref('#1E293B')

  const borderRadius = ref(80)    // 0~512
  const shadowEnabled = ref(true)
  const shadowColor = ref('rgba(0,0,0,0.5)')
  const shadowBlur = ref(40)
  const shadowOffsetX = ref(0)
  const shadowOffsetY = ref(10)

  const innerGlow = ref(false)
  const innerGlowColor = ref('rgba(255,255,255,0.15)')
  const innerGlowBlur = ref(20)

  const activePresetId = ref<string | null>('ios')

  const cssGradient = computed(() => {
    if (bgType.value === 'none') return 'transparent'
    if (bgType.value === 'solid') return solidColor.value
    const stopStr = stops.value
      .slice()
      .sort((a, b) => a.position - b.position)
      .map(s => `${s.color} ${s.position}%`)
      .join(', ')
    if (bgType.value === 'radial') return `radial-gradient(circle, ${stopStr})`
    return `linear-gradient(${angle.value}deg, ${stopStr})`
  })

  function applyPreset(presetId: string) {
    const preset = PRESETS.find(p => p.id === presetId)
    if (!preset) return
    bgType.value = preset.type
    if (preset.type === 'solid' && preset.stops.length > 0) {
      solidColor.value = preset.stops[0].color
    } else {
      stops.value = preset.stops.map(s => ({ ...s }))
    }
    angle.value = preset.angle
    activePresetId.value = presetId
  }

  function setType(type: BgType) {
    bgType.value = type
    activePresetId.value = null
  }

  function updateStop(index: number, stop: Partial<GradientStop>) {
    if (stops.value[index]) {
      stops.value[index] = { ...stops.value[index], ...stop }
      activePresetId.value = null
    }
  }

  function addStop() {
    stops.value.push({ color: '#ffffff', position: 50 })
  }

  function removeStop(index: number) {
    if (stops.value.length > 2) {
      stops.value.splice(index, 1)
    }
  }

  return {
    bgType,
    stops,
    angle,
    solidColor,
    borderRadius,
    shadowEnabled,
    shadowColor,
    shadowBlur,
    shadowOffsetX,
    shadowOffsetY,
    innerGlow,
    innerGlowColor,
    innerGlowBlur,
    activePresetId,
    cssGradient,
    applyPreset,
    setType,
    updateStop,
    addStop,
    removeStop,
  }
})

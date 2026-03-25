import { computed } from 'vue'
import { useBackgroundStore } from '../../store/useBackgroundStore'

/**
 * Composable for background system utilities.
 * Business logic is in the store; this hook provides computed helpers.
 */
export function useBackground() {
  const bgStore = useBackgroundStore()

  const previewStyle = computed(() => ({
    background: bgStore.cssGradient,
    borderRadius: `${bgStore.borderRadius}px`,
    boxShadow: bgStore.shadowEnabled
      ? `${bgStore.shadowOffsetX}px ${bgStore.shadowOffsetY}px ${bgStore.shadowBlur}px ${bgStore.shadowColor}`
      : 'none',
  }))

  function randomizeGradient() {
    const hue1 = Math.floor(Math.random() * 360)
    const hue2 = (hue1 + 60 + Math.floor(Math.random() * 120)) % 360
    bgStore.stops[0].color = `hsl(${hue1}, 70%, 50%)`
    bgStore.stops[bgStore.stops.length - 1].color = `hsl(${hue2}, 70%, 50%)`
    bgStore.angle = Math.floor(Math.random() * 360)
  }

  return { previewStyle, randomizeGradient }
}

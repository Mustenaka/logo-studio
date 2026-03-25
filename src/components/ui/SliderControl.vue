<script setup lang="ts">
defineProps<{
  label: string
  min?: number
  max?: number
  step?: number
  unit?: string
}>()

const model = defineModel<number>({ required: true })
</script>

<template>
  <div class="slider-control">
    <div class="slider-control__header">
      <span class="slider-control__label">{{ label }}</span>
      <span class="slider-control__value">{{ model }}{{ unit ?? '' }}</span>
    </div>
    <input
      type="range"
      class="slider-control__track"
      :min="min ?? 0"
      :max="max ?? 100"
      :step="step ?? 1"
      v-model.number="model"
    />
  </div>
</template>

<style scoped>
.slider-control {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.slider-control__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.slider-control__label {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.slider-control__value {
  font-size: var(--text-sm);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
  min-width: 36px;
  text-align: right;
}

.slider-control__track {
  -webkit-appearance: none;
  appearance: none;
  width: 100%;
  height: 4px;
  background: var(--border);
  border-radius: var(--radius-full);
  outline: none;
  cursor: pointer;
}

.slider-control__track::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: var(--accent);
  box-shadow: 0 0 6px var(--accent-glow);
  cursor: pointer;
  transition: transform var(--transition-fast), box-shadow var(--transition-fast);
}

.slider-control__track::-webkit-slider-thumb:hover {
  transform: scale(1.2);
  box-shadow: 0 0 10px var(--accent-glow);
}
</style>

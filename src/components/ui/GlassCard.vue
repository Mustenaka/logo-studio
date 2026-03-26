<script setup lang="ts">
defineProps<{
  title?: string
  collapsible?: boolean
  padding?: boolean
}>()

const collapsed = defineModel<boolean>('collapsed', { default: false })
</script>

<template>
  <div class="glass-card" :class="{ 'no-pad': padding === false }">
    <div v-if="title" class="glass-card__header"
      :class="{ 'glass-card__header--collapsible': collapsible }"
      @click="collapsible && (collapsed = !collapsed)"
    >
      <span class="glass-card__title">{{ title }}</span>
      <svg v-if="collapsible" class="glass-card__chevron" :class="{ rotated: !collapsed }"
        width="14" height="14" viewBox="0 0 14 14" fill="none">
        <path d="M3 5l4 4 4-4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
    </div>
    <div v-if="!collapsed" class="glass-card__body">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.glass-card {
  background: var(--bg-card);
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.glass-card__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 9px var(--space-4);
  border-bottom: 1px solid var(--border);
  cursor: default;
  user-select: none;
}
.glass-card__header--collapsible {
  cursor: pointer;
}
.glass-card__header--collapsible:hover {
  background: var(--bg-card-hover);
}

.glass-card__title {
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.08em;
}

.glass-card__chevron {
  color: var(--text-tertiary);
  transition: transform var(--transition-fast);
}
.glass-card__chevron.rotated {
  transform: rotate(180deg);
}

.glass-card__body {
  padding: var(--space-4);
  /* Extra bottom padding so slider thumbs (14px, overflow 5px below track) clear the card edge */
  padding-bottom: 28px;
}

.no-pad .glass-card__body {
  padding: 0;
}
</style>

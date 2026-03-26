<script setup lang="ts">
const emit = defineEmits<{
  drag: [delta: number]
}>()

function onMouseDown(e: MouseEvent) {
  e.preventDefault()
  const startX = e.clientX

  function onMove(ev: MouseEvent) {
    emit('drag', ev.clientX - startX)
  }

  function onUp() {
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
  }

  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}
</script>

<template>
  <div class="resize-handle" @mousedown="onMouseDown">
    <div class="resize-handle__bar" />
  </div>
</template>

<style scoped>
.resize-handle {
  width: 6px;
  height: 100%;
  cursor: col-resize;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  background: transparent;
  position: relative;
  z-index: 10;
  transition: background var(--transition-fast);
}
.resize-handle:hover {
  background: rgba(99, 102, 241, 0.08);
}
.resize-handle__bar {
  width: 2px;
  height: 40px;
  border-radius: 1px;
  background: var(--border);
  transition: background var(--transition-fast), height var(--transition-fast);
}
.resize-handle:hover .resize-handle__bar {
  background: var(--accent);
  height: 60px;
}
</style>

<script setup lang="ts">
import { onBeforeUnmount, watch } from "vue";
import type { Notice } from "../stores/adb";

const props = withDefaults(
  defineProps<{
    notice: Notice | null;
    duration?: number;
  }>(),
  {
    duration: 2600,
  },
);

const emit = defineEmits<{
  close: [];
}>();

let timer: number | null = null;

function clearTimer() {
  if (timer !== null) {
    window.clearTimeout(timer);
    timer = null;
  }
}

watch(
  () => props.notice,
  (value) => {
    clearTimer();
    if (!value) {
      return;
    }
    timer = window.setTimeout(() => {
      emit("close");
    }, props.duration);
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  clearTimer();
});
</script>

<template>
  <Teleport to="body">
    <Transition name="toast-fade">
      <section v-if="notice" :class="['toast', `toast-${notice.type}`]" role="status" aria-live="polite">
        <span class="toast-message">{{ notice.message }}</span>
        <button class="toast-close" type="button" aria-label="关闭提示" @click="emit('close')">x</button>
      </section>
    </Transition>
  </Teleport>
</template>

<style scoped>
.toast {
  position: fixed;
  right: 14px;
  bottom: 14px;
  z-index: 999;
  min-width: 260px;
  max-width: min(88vw, 520px);
  border-radius: 8px;
  padding: 8px 10px;
  border: 1px solid;
  box-shadow: 0 10px 30px rgba(15, 23, 42, 0.16);
  display: flex;
  align-items: center;
  gap: 8px;
  justify-content: space-between;
  backdrop-filter: blur(8px);
}

.toast-message {
  overflow-wrap: anywhere;
}

.toast-info {
  background: rgba(239, 246, 255, 0.96);
  border-color: #bfdbfe;
  color: #1d4ed8;
}

.toast-success {
  background: rgba(236, 253, 245, 0.96);
  border-color: #bbf7d0;
  color: #047857;
}

.toast-error {
  background: rgba(254, 242, 242, 0.96);
  border-color: #fecaca;
  color: #b91c1c;
}

.toast-close {
  border: 0;
  border-radius: 6px;
  background: rgba(15, 23, 42, 0.08);
  color: inherit;
  padding: 5px 8px;
  cursor: pointer;
  flex-shrink: 0;
}

.toast-fade-enter-active,
.toast-fade-leave-active {
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}

.toast-fade-enter-from,
.toast-fade-leave-to {
  opacity: 0;
  transform: translateY(8px);
}
</style>

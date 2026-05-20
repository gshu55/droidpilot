<script setup lang="ts">
import { onBeforeUnmount, watch } from "vue";
import type { Notice } from "../stores/adb";
import { useI18n } from "../i18n";

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
const { t } = useI18n();

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
        <button class="toast-close" type="button" :aria-label="t('common.close')" @click="emit('close')">x</button>
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
  box-shadow: var(--shadow);
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
  background: var(--primary-soft);
  border-color: var(--primary-border);
  color: var(--primary);
}

.toast-success {
  background: rgba(236, 253, 245, 0.96);
  border-color: #bbf7d0;
  color: #047857;
}

.toast-error {
  background: var(--danger-soft);
  border-color: var(--danger-border);
  color: var(--danger-text);
}

.toast-close {
  border: 0;
  border-radius: 6px;
  background: color-mix(in srgb, currentColor 12%, transparent);
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

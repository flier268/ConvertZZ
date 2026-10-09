<script setup lang="ts">
import { onMounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

const message = ref("");
let hideTimer: ReturnType<typeof setTimeout> | undefined;

async function present(text: string): Promise<void> {
  message.value = text;
  const window = getCurrentWindow();
  await window.show();
  if (hideTimer) clearTimeout(hideTimer);
  hideTimer = setTimeout(() => {
    void window.hide();
    message.value = "";
  }, 2000);
}

async function dismiss(): Promise<void> {
  if (hideTimer) clearTimeout(hideTimer);
  message.value = "";
  await getCurrentWindow().hide();
}

onMounted(async () => {
  await listen<string>("app://toast", ({ payload }) => {
    void present(payload);
  });
});
</script>

<template>
  <button v-if="message" type="button" class="toast-shell" @click="dismiss">
    <div class="toast-card">{{ message }}</div>
  </button>
</template>

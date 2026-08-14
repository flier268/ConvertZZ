<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { isTauri } from "@tauri-apps/api/core";
import { LogicalPosition, type PhysicalPosition } from "@tauri-apps/api/dpi";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ElMessage } from "element-plus";
import { convertClipboard } from "./lib/actions";
import { executeLegacyAction } from "./lib/legacyActions";
import { loadSettings, saveSettings, zhConvertOptions } from "./lib/settings";

const busy = ref(false);
let floatingWindow: ReturnType<typeof getCurrentWindow> | undefined;
let unlistenMoved: UnlistenFn | undefined;
let savePositionTimer: ReturnType<typeof setTimeout> | undefined;
let lastDragButton: "left" | "right" = "left";

async function persistPosition(position: PhysicalPosition) {
  if (!floatingWindow) return;
  const scaleFactor = await floatingWindow.scaleFactor();
  const logical = position.toLogical(scaleFactor);
  const settings = await loadSettings();
  settings.floatingBall.x = Math.round(logical.x);
  settings.floatingBall.y = Math.round(logical.y);
  await saveSettings();
}

onMounted(async () => {
  if (!isTauri()) return;
  floatingWindow = getCurrentWindow();
  const settings = await loadSettings();
  const { x, y } = settings.floatingBall;
  if (Number.isFinite(x) && Number.isFinite(y) && (x !== -1 || y !== -1)) {
    await floatingWindow.setPosition(new LogicalPosition(x, y));
  }
  unlistenMoved = await floatingWindow.onMoved(({ payload }) => {
    if (savePositionTimer) clearTimeout(savePositionTimer);
    savePositionTimer = setTimeout(() => void persistPosition(payload), 180);
  });
});

onBeforeUnmount(() => {
  if (savePositionTimer) clearTimeout(savePositionTimer);
  unlistenMoved?.();
});

async function run(direction: "s2t" | "t2s") {
  if (busy.value) return;
  busy.value = true;
  try {
    const settings = await loadSettings();
    await convertClipboard(direction, settings.engine, undefined, settings.vocabularyCorrection, zhConvertOptions(settings, direction), settings.dictionaryPath);
    ElMessage.success(direction === "s2t" ? "已轉為繁體" : "已轉為簡體");
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    busy.value = false;
  }
}

async function startDrag(event: MouseEvent) {
  if (event.button !== 0 || event.ctrlKey || event.altKey || event.shiftKey || !isTauri()) return;
  event.preventDefault();
  try {
    await (floatingWindow ?? getCurrentWindow()).startDragging();
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  }
}

function modifier(event: MouseEvent | DragEvent): "Ctrl" | "Alt" | "Shift" | undefined {
  if (event.ctrlKey) return "Ctrl";
  if (event.altKey) return "Alt";
  if (event.shiftKey) return "Shift";
  return undefined;
}

async function runConfiguredClick(button: "left" | "right", event: MouseEvent) {
  const key = modifier(event);
  if (!key) return;
  event.preventDefault();
  event.stopPropagation();
  const settings = await loadSettings();
  const action = settings.quickActions[`${button}Click${key}` as keyof typeof settings.quickActions];
  try {
    await executeLegacyAction(action, settings);
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  }
}

function trackDragButton(event: DragEvent) {
  if (event.buttons & 2) lastDragButton = "right";
  else if (event.buttons & 1) lastDragButton = "left";
}

async function runConfiguredDrop(event: DragEvent) {
  const key = modifier(event);
  const text = event.dataTransfer?.getData("text/plain");
  if (!key || !text) return;
  const settings = await loadSettings();
  const action = settings.quickActions[`${lastDragButton}Drop${key}` as keyof typeof settings.quickActions];
  try {
    await executeLegacyAction(action, settings, text);
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  }
}

async function handleContextMenu(event: MouseEvent) {
  if (modifier(event)) await runConfiguredClick("right", event);
  else {
    event.preventDefault();
    await run("t2s");
  }
}
</script>

<template>
  <div class="floating-shell" title="拖曳移動 · 雙擊轉繁體 · 右鍵轉簡體" @mousedown="startDrag" @click="runConfiguredClick('left', $event)" @dragstart.prevent @dragover.prevent="trackDragButton" @drop.prevent="runConfiguredDrop" @dblclick="run('s2t')" @contextmenu="handleContextMenu">
    <div class="floating-orb" :class="{ busy }" aria-label="ConvertZZ 浮動球">
      <span class="floating-z-large">Z</span>
      <span class="floating-z-small">Z</span>
    </div>
  </div>
</template>

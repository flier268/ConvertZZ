<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { isTauri } from "@tauri-apps/api/core";
import type { PhysicalPosition } from "@tauri-apps/api/dpi";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { FLOATING_CONTEXT_MENU } from "./lib/appMenu";
import { popupAppMenu } from "./lib/appMenuPopup";
import { executeLegacyAction } from "./lib/legacyActions";
import {
  clickModifier,
  dropButton,
  mouseSide,
  pointerIntent,
  quickActionKey,
} from "./lib/floatingGestures";
import { applyFloatingBallWindow } from "./lib/desktop";
import { loadSettings, patchSavedSettings } from "./lib/settings";
import { showAppToast } from "./lib/toast";
import BrandMark from "./BrandMark.vue";

const busy = ref(false);
const htmlMenu = ref<{ x: number; y: number }>();
let floatingWindow: ReturnType<typeof getCurrentWindow> | undefined;
let unlistenMoved: UnlistenFn | undefined;
let savePositionTimer: ReturnType<typeof setTimeout> | undefined;
let lastDropButton: "left" | "right" = "left";

async function persistPosition(position: PhysicalPosition) {
  if (!floatingWindow) return;
  const scaleFactor = await floatingWindow.scaleFactor();
  const logical = position.toLogical(scaleFactor);
  await patchSavedSettings((settings) => {
    settings.floatingBall.x = Math.round(logical.x);
    settings.floatingBall.y = Math.round(logical.y);
  });
}

onMounted(async () => {
  if (!isTauri()) return;
  floatingWindow = getCurrentWindow();
  await applyFloatingBallWindow(await loadSettings());
  unlistenMoved = await floatingWindow.onMoved(({ payload }) => {
    if (savePositionTimer) clearTimeout(savePositionTimer);
    savePositionTimer = setTimeout(() => void persistPosition(payload), 180);
  });
});

onBeforeUnmount(() => {
  if (savePositionTimer) clearTimeout(savePositionTimer);
  unlistenMoved?.();
});

async function runAction(action: string, input?: string) {
  if (busy.value || !action || action === "0") return;
  busy.value = true;
  htmlMenu.value = undefined;
  try {
    await executeLegacyAction(action, await loadSettings(), input);
  } catch (error) {
    await showAppToast(error instanceof Error ? error.message : String(error));
  } finally {
    busy.value = false;
  }
}

async function handlePointerDown(event: MouseEvent) {
  if (
    htmlMenu.value &&
    !(event.target instanceof Element && event.target.closest(".floating-context-menu"))
  ) {
    htmlMenu.value = undefined;
  }
  const button = mouseSide(event.button);
  if (!button) return;
  const intent = pointerIntent(button, clickModifier(event), "down");
  if (intent.type !== "drag" || !isTauri()) return;
  event.preventDefault();
  try {
    await (floatingWindow ?? getCurrentWindow()).startDragging();
  } catch (error) {
    await showAppToast(error instanceof Error ? error.message : String(error));
  }
}

async function handlePointerUp(event: MouseEvent) {
  const button = mouseSide(event.button);
  if (!button) return;
  const intent = pointerIntent(button, clickModifier(event), "up");
  if (intent.type !== "quick-action") return;
  event.preventDefault();
  event.stopPropagation();
  const settings = await loadSettings();
  await runAction(settings.quickActions[quickActionKey(intent.button, "Click", intent.modifier)]);
}

async function handleContextMenu(event: MouseEvent) {
  event.preventDefault();
  if (clickModifier(event)) return;
  htmlMenu.value = undefined;
  if (isTauri()) {
    try {
      await popupAppMenu(FLOATING_CONTEXT_MENU, runAction);
    } catch (error) {
      await showAppToast(error instanceof Error ? error.message : String(error));
    }
    return;
  }
  htmlMenu.value = { x: event.clientX, y: event.clientY };
}

function trackDropButton(event: DragEvent) {
  lastDropButton = dropButton(event.buttons);
}

async function handleDrop(event: DragEvent) {
  const modifier = clickModifier(event);
  const text = event.dataTransfer?.getData("text/plain");
  if (!modifier || !text) return;
  const settings = await loadSettings();
  await runAction(settings.quickActions[quickActionKey(lastDropButton, "Drop", modifier)], text);
}
</script>

<template>
  <div
    class="floating-shell"
    title="拖曳移動 · 右鍵開啟選單 · 輔助鍵加點擊執行設定動作"
    @mousedown="handlePointerDown"
    @mouseup="handlePointerUp"
    @contextmenu="handleContextMenu"
    @selectstart.prevent
    @dragstart.prevent
    @dragover.prevent="trackDropButton"
    @drop.prevent="handleDrop"
  >
    <div class="floating-orb" :class="{ busy }" aria-label="ConvertZZ 浮動球">
      <BrandMark />
    </div>
    <nav
      v-if="htmlMenu"
      class="floating-context-menu"
      :style="{ left: `${htmlMenu.x}px`, top: `${htmlMenu.y}px` }"
      @mousedown.stop
    >
      <template v-for="(node, index) in FLOATING_CONTEXT_MENU" :key="index">
        <hr v-if="node.type === 'separator'" />
        <div v-else-if="node.type === 'submenu'" class="floating-context-submenu">
          <button type="button">{{ node.label }}</button>
          <div class="floating-context-submenu-items">
            <template v-for="(child, childIndex) in node.items" :key="childIndex">
              <hr v-if="child.type === 'separator'" />
              <button v-else-if="child.type === 'item'" type="button" @click="runAction(child.id)">
                {{ child.label }}
              </button>
            </template>
          </div>
        </div>
        <button v-else type="button" @click="runAction(node.id)">{{ node.label }}</button>
      </template>
    </nav>
  </div>
</template>

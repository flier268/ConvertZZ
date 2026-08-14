<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, provide, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import {
  Collection,
  Document,
  Files,
  Headset,
  InfoFilled,
  Memo,
  Operation,
  Setting,
  Switch,
} from "@element-plus/icons-vue";
import QuickPage from "./pages/QuickPage.vue";
import FilesPage from "./pages/FilesPage.vue";
import ClipboardPage from "./pages/ClipboardPage.vue";
import AudioPage from "./pages/AudioPage.vue";
import ToolsPage from "./pages/ToolsPage.vue";
import DictionaryPage from "./pages/DictionaryPage.vue";
import SettingsPage from "./pages/SettingsPage.vue";
import AboutPage from "./pages/AboutPage.vue";
import OnboardingTour from "./OnboardingTour.vue";
import BrandMark from "./BrandMark.vue";
import { loadSettings } from "./lib/settings";
import { sidecar } from "./lib/sidecar";
import { setCliInvocation } from "./lib/cli";
import type { ParsedCli } from "@shared/contracts";
import { applyDesktopSettings, applyStartupWindowVisibility } from "./lib/desktop";
import { executeLegacyAction } from "./lib/legacyActions";
import { showAppToast } from "./lib/toast";
import { promptForAppUpdate } from "./lib/appUpdate";

const active = ref("quick");
const ready = ref(false);
const health = ref<{ node: string; version: string }>();
const startupError = ref("");
const unlisten: Array<() => void> = [];

const pages = {
  quick: QuickPage,
  files: FilesPage,
  clipboard: ClipboardPage,
  audio: AudioPage,
  tools: ToolsPage,
  dictionary: DictionaryPage,
  settings: SettingsPage,
  about: AboutPage,
};
const currentPage = computed(() => pages[active.value as keyof typeof pages] ?? QuickPage);
const tour = ref<{ replay: () => Promise<void> } | null>(null);
const hasCliArgs = ref(false);

function onTourStarted(): void {
  void invoke("show_main_window");
}

provide("replayOnboarding", () => tour.value?.replay());

onMounted(async () => {
  try {
    const settings = await loadSettings();
    await applyDesktopSettings(settings, { revealFloating: false });
    const args = await invoke<string[]>("startup_args");
    hasCliArgs.value = args.length > 0;
    await applyStartupWindowVisibility(settings, args.length > 0);
    const savedApiKey = await invoke<string | null>("load_zhconvert_api_key").catch(() => null);
    if (savedApiKey) await sidecar.request("zhconvert.configure", { apiKey: savedApiKey });
    const currentHealth = await sidecar.request<{ node: string; version: string }>("health", {});
    health.value = currentHealth;
    if (settings.checkVersionOnStart) {
      void promptForAppUpdate({ silentWhenCurrent: true }).catch(() => undefined);
    }
    if (args.length) {
      const parsed = await sidecar.request<ParsedCli>("cli.parse", {
        args,
        defaultEngine: settings.engine,
      });
      setCliInvocation(parsed);
      if (parsed.mode === "audio") active.value = "audio";
      else if (parsed.mode === "file") active.value = "files";
    }
    unlisten.push(
      await listen<string>("app://navigate", ({ payload }) => {
        active.value = payload;
      }),
    );
    unlisten.push(
      await listen<string>("app://legacy-action", async ({ payload }) => {
        try {
          await executeLegacyAction(payload, await loadSettings());
        } catch (error) {
          await showAppToast(error instanceof Error ? error.message : String(error));
        }
      }),
    );
    unlisten.push(
      await listen<string[]>("app://second-instance", async ({ payload }) => {
        const currentSettings = await loadSettings();
        const parsed = await sidecar.request<ParsedCli>("cli.parse", {
          args: payload.slice(1),
          defaultEngine: currentSettings.engine,
        });
        setCliInvocation(parsed);
        active.value =
          parsed.mode === "audio" ? "audio" : parsed.mode === "file" ? "files" : "quick";
      }),
    );
    ready.value = true;
  } catch (error) {
    startupError.value = error instanceof Error ? error.message : String(error);
  }
});

onBeforeUnmount(() => unlisten.forEach((dispose) => dispose()));
</script>

<template>
  <el-container class="app-shell">
    <el-aside width="232px" class="sidebar">
      <div class="brand">
        <div id="tour-brand" class="brand-mark"><BrandMark /></div>
        <div>
          <strong>ConvertZZ</strong>
          <small>跨平台中文轉換</small>
        </div>
      </div>
      <el-menu :default-active="active" @select="active = $event">
        <el-menu-item id="tour-quick" index="quick"
          ><el-icon><Switch /></el-icon><span>快速轉換</span></el-menu-item
        >
        <el-menu-item id="tour-files" index="files"
          ><el-icon><Files /></el-icon><span>檔案與檔名</span></el-menu-item
        >
        <el-menu-item id="tour-clipboard" index="clipboard"
          ><el-icon><Memo /></el-icon><span>剪貼簿</span></el-menu-item
        >
        <el-menu-item id="tour-audio" index="audio"
          ><el-icon><Headset /></el-icon><span>音訊標籤</span></el-menu-item
        >
        <el-menu-item id="tour-tools" index="tools"
          ><el-icon><Operation /></el-icon><span>文字工具</span></el-menu-item
        >
        <el-menu-item id="tour-dictionary" index="dictionary"
          ><el-icon><Collection /></el-icon><span>舊版字典</span></el-menu-item
        >
        <el-menu-item id="tour-settings" index="settings"
          ><el-icon><Setting /></el-icon><span>設定</span></el-menu-item
        >
        <el-menu-item id="tour-about" index="about"
          ><el-icon><InfoFilled /></el-icon><span>關於與差異</span></el-menu-item
        >
      </el-menu>
      <div class="runtime-status">
        <span class="status-dot" :class="{ online: health }"></span>
        <span v-if="health">核心 {{ health.version }} · Node {{ health.node }}</span>
        <span v-else>核心啟動中</span>
      </div>
    </el-aside>
    <el-main class="content">
      <component :is="currentPage" v-if="ready" />
      <el-alert
        v-else-if="startupError"
        title="ConvertZZ 無法啟動"
        :description="startupError"
        type="error"
        :closable="false"
        show-icon
      />
      <div v-else class="loading-state">
        <el-icon class="is-loading"><Document /></el-icon><span>正在載入轉換核心</span>
      </div>
    </el-main>
    <OnboardingTour
      v-if="ready"
      ref="tour"
      :auto-start="!hasCliArgs"
      @navigate="active = $event"
      @started="onTourStarted"
    />
  </el-container>
</template>

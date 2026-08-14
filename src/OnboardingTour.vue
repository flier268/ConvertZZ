<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open as openFile } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import { ONBOARDING_STEPS, pageForOnboardingStep, type OnboardingPage } from "./lib/onboarding";
import { applyDesktopSettings } from "./lib/desktop";
import {
  clearOnboardingComplete,
  importLegacySettings,
  isOnboardingComplete,
  loadSettings,
  markOnboardingComplete,
} from "./lib/settings";
import { importFailureMessage } from "./lib/settingsApply";

const props = withDefaults(defineProps<{ autoStart?: boolean }>(), { autoStart: true });
const emit = defineEmits<{ navigate: [page: OnboardingPage]; started: [] }>();

const open = ref(false);
const current = ref(0);
const legacyPath = ref<string>();
const importing = ref(false);
const importResult = ref("");
const importError = ref("");

const targets: Record<string, string | undefined> = {
  welcome: undefined,
  quick: "#tour-quick",
  files: "#tour-files",
  clipboard: "#tour-clipboard",
  audio: "#tour-audio",
  tools: "#tour-tools",
  desktop: "#tour-brand",
  import: undefined,
  settings: "#tour-settings",
};

async function startTour(): Promise<void> {
  current.value = 0;
  importResult.value = "";
  importError.value = "";
  legacyPath.value =
    (await invoke<string | null>("legacy_settings_path").catch(() => null)) ?? undefined;
  emit("navigate", "quick");
  await invoke("show_main_window").catch(() => undefined);
  emit("started");
  open.value = true;
}

async function finishTour(): Promise<void> {
  open.value = false;
  await markOnboardingComplete();
}

function onStepChange(index: number): void {
  current.value = index;
  emit("navigate", pageForOnboardingStep(index));
}

async function importFrom(path: string): Promise<void> {
  importing.value = true;
  importError.value = "";
  try {
    const imported = await importLegacySettings(path, { confirmReplace: false });
    if (!imported) return;
    await applyDesktopSettings(imported.settings);
    importResult.value = `已匯入，備份位於 ${imported.backupPath}`;
    ElMessage.success(importResult.value);
  } catch (error) {
    importError.value = importFailureMessage(error);
    ElMessage.error(importError.value);
  } finally {
    importing.value = false;
  }
}

async function importDetected(): Promise<void> {
  if (legacyPath.value) await importFrom(legacyPath.value);
}

async function importPicked(): Promise<void> {
  const path = await openFile({
    multiple: false,
    filters: [{ name: "ConvertZZ 舊版設定", extensions: ["json"] }],
  });
  if (path) await importFrom(path as string);
}

defineExpose({
  start: startTour,
  async replay() {
    await clearOnboardingComplete();
    await startTour();
  },
});

onMounted(async () => {
  await loadSettings();
  if (props.autoStart && !(await isOnboardingComplete())) await startTour();
});
</script>

<template>
  <el-tour
    v-model="open"
    v-model:current="current"
    :z-index="5000"
    @change="onStepChange"
    @finish="finishTour"
    @close="finishTour"
  >
    <el-tour-step
      v-for="step in ONBOARDING_STEPS"
      :key="step.id"
      :title="step.title"
      :target="targets[step.id]"
      :next-button-props="step.id === 'import' ? { children: '略過匯入' } : undefined"
    >
      <template v-if="step.id === 'welcome'">
        <p>這是跨平台的中文轉換工具。接下來會帶你看主要畫面、浮動球，以及是否要匯入舊版設定。</p>
      </template>
      <template v-else-if="step.id === 'quick'">
        <p>在這裡貼上文字，選擇簡轉繁或繁轉簡，就能立即轉換。</p>
      </template>
      <template v-else-if="step.id === 'files'">
        <p>批次轉換檔案內容或檔名。執行前會先顯示預覽，確認後才寫入。</p>
      </template>
      <template v-else-if="step.id === 'clipboard'">
        <p>監看或讀取剪貼簿文字，轉換後可寫回剪貼簿。</p>
      </template>
      <template v-else-if="step.id === 'audio'">
        <p>轉換 MP3、APE、OGG 與 Opus 的文字標籤。未選欄位與封面不會被改動。</p>
      </template>
      <template v-else-if="step.id === 'tools'">
        <p>HTML 實體、全半形與 Unicode 跳脫等舊版文字工具都在這裡。</p>
      </template>
      <template v-else-if="step.id === 'desktop'">
        <p>
          程式預設留在托盤與浮動球。右鍵浮動球可開啟與舊版相同的轉換選單。托盤也可隨時打開主視窗。
        </p>
      </template>
      <template v-else-if="step.id === 'import'">
        <p>若你有 1.x 的 ConvertZZ.json，可先備份再匯入為 2.0 設定。</p>
        <p v-if="legacyPath" class="onboarding-path">已找到：{{ legacyPath }}</p>
        <p v-else>也可以稍後自行選擇檔案。</p>
        <el-alert
          v-if="importError"
          class="onboarding-import-error"
          :title="importError"
          type="error"
          :closable="false"
          show-icon
        />
        <p v-if="importResult" class="onboarding-result">{{ importResult }}</p>
        <div class="onboarding-import-actions">
          <el-button v-if="legacyPath" type="primary" :loading="importing" @click="importDetected"
            >匯入找到的設定</el-button
          >
          <el-button :loading="importing" @click="importPicked">選擇檔案</el-button>
        </div>
      </template>
      <template v-else>
        <p>可在這裡調整預設引擎、啟動時是否顯示主視窗，以及點選後按下組合鍵來設定全域快捷鍵。</p>
      </template>
    </el-tour-step>
  </el-tour>
</template>

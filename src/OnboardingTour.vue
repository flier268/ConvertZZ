<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open as openFile } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import { t } from "@/i18n";
import {
  importStepNextLabel,
  ONBOARDING_STEPS,
  onboardingStepTitle,
  pageForOnboardingStep,
  type OnboardingPage,
} from "./lib/onboarding";
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
const settingsImported = ref(false);

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
  settingsImported.value = false;
  legacyPath.value =
    (await invoke<string | null>("legacy_settings_path").catch(() => null)) ?? undefined;
  emit("navigate", "quick");
  await invoke("show_main_window").catch(() => undefined);
  emit("started");
  open.value = true;
  // 再開一次，蓋過啟動階段可能晚到的 hide，確保導覽不會開在已隱藏的主視窗上。
  await invoke("show_main_window").catch(() => undefined);
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
    await applyDesktopSettings(imported);
    settingsImported.value = true;
    importResult.value = t("app.onboarding.importSuccess");
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
    filters: [{ name: t("app.onboarding.legacyFilterName"), extensions: ["json"] }],
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
      :title="onboardingStepTitle(step.id)"
      :target="targets[step.id]"
      :next-button-props="
        step.id === 'import' ? { children: importStepNextLabel(settingsImported) } : undefined
      "
    >
      <template v-if="step.id === 'welcome'">
        <p>{{ t("app.onboarding.welcome") }}</p>
      </template>
      <template v-else-if="step.id === 'quick'">
        <p>{{ t("app.onboarding.quick") }}</p>
      </template>
      <template v-else-if="step.id === 'files'">
        <p>{{ t("app.onboarding.files") }}</p>
      </template>
      <template v-else-if="step.id === 'clipboard'">
        <p>{{ t("app.onboarding.clipboard") }}</p>
      </template>
      <template v-else-if="step.id === 'audio'">
        <p>{{ t("app.onboarding.audio") }}</p>
      </template>
      <template v-else-if="step.id === 'tools'">
        <p>{{ t("app.onboarding.tools") }}</p>
      </template>
      <template v-else-if="step.id === 'desktop'">
        <p>{{ t("app.onboarding.desktop") }}</p>
      </template>
      <template v-else-if="step.id === 'import'">
        <p>{{ t("app.onboarding.import") }}</p>
        <p v-if="legacyPath" class="onboarding-path">
          {{ t("app.onboarding.foundPath", { path: legacyPath }) }}
        </p>
        <p v-else>{{ t("app.onboarding.pickLater") }}</p>
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
          <el-button
            v-if="legacyPath"
            type="primary"
            :loading="importing"
            @click="importDetected"
            >{{ t("app.onboarding.importFound") }}</el-button
          >
          <el-button :loading="importing" @click="importPicked">{{
            t("app.onboarding.pickFile")
          }}</el-button>
        </div>
      </template>
      <template v-else>
        <p>{{ t("app.onboarding.settings") }}</p>
      </template>
    </el-tour-step>
  </el-tour>
</template>

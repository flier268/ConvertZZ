<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ElMessage } from "element-plus";
import { computed, inject, onMounted, ref } from "vue";
import type { PlatformCapabilities } from "@shared/contracts";
import BrandMark from "../BrandMark.vue";
import { isDialogCancelled, promptForAppUpdate } from "../lib/appUpdate";
import { loadSettings } from "../lib/settings";
import { t } from "@/i18n";

defineOptions({ name: "AboutPage" });

const capabilities = ref<PlatformCapabilities>();
const checkingUpdate = ref(false);
const replayOnboarding = inject<() => void>("replayOnboarding");
onMounted(async () => {
  capabilities.value = await invoke("platform_capabilities");
});

async function checkForUpdates(): Promise<void> {
  checkingUpdate.value = true;
  try {
    const settings = await loadSettings();
    await promptForAppUpdate({ includePreRelease: settings.checkPreReleaseUpdates });
  } catch (error) {
    if (!isDialogCancelled(error)) {
      ElMessage.error(error instanceof Error ? error.message : String(error));
    }
  } finally {
    checkingUpdate.value = false;
  }
}

type PlatformColumn = "windows" | "x11" | "wayland" | "macos";

const platformColumns: Array<{ key: PlatformColumn; label: string }> = [
  { key: "windows", label: "Windows" },
  { key: "x11", label: "Linux X11" },
  { key: "wayland", label: "Linux Wayland" },
  { key: "macos", label: "macOS" },
];

type Support =
  "full" | "disabledThisRelease" | "disabled" | "dependsOnCompositor" | "notApplicable";
type Detail =
  | "trayWindows"
  | "trayLinux"
  | "trayMacos"
  | "credentialsWindows"
  | "credentialsLinux"
  | "credentialsMacos"
  | "updateWindows"
  | "updateLinux"
  | "updateMacos";
type Cell = { support: Support } | { detail: Detail };
type Feature =
  | "textFiles"
  | "audioTags"
  | "globalShortcuts"
  | "autoCopyPaste"
  | "floatingBallOnTop"
  | "sendTo"
  | "tray"
  | "credentials"
  | "autoUpdate";

const full: Cell = { support: "full" };
const notApplicable: Cell = { support: "notApplicable" };

const differences: Array<{ feature: Feature } & Record<PlatformColumn, Cell>> = [
  { feature: "textFiles", windows: full, x11: full, wayland: full, macos: full },
  { feature: "audioTags", windows: full, x11: full, wayland: full, macos: full },
  {
    feature: "globalShortcuts",
    windows: full,
    x11: full,
    wayland: { support: "disabledThisRelease" },
    macos: full,
  },
  {
    feature: "autoCopyPaste",
    windows: full,
    x11: full,
    wayland: { support: "disabled" },
    macos: full,
  },
  {
    feature: "floatingBallOnTop",
    windows: full,
    x11: full,
    wayland: { support: "dependsOnCompositor" },
    macos: full,
  },
  {
    feature: "sendTo",
    windows: full,
    x11: notApplicable,
    wayland: notApplicable,
    macos: notApplicable,
  },
  {
    feature: "tray",
    windows: { detail: "trayWindows" },
    x11: { detail: "trayLinux" },
    wayland: { detail: "trayLinux" },
    macos: { detail: "trayMacos" },
  },
  {
    feature: "credentials",
    windows: { detail: "credentialsWindows" },
    x11: { detail: "credentialsLinux" },
    wayland: { detail: "credentialsLinux" },
    macos: { detail: "credentialsMacos" },
  },
  {
    feature: "autoUpdate",
    windows: { detail: "updateWindows" },
    x11: { detail: "updateLinux" },
    wayland: { detail: "updateLinux" },
    macos: { detail: "updateMacos" },
  },
];

const conversionDifferences = [
  "legacyEngine",
  "unmatched",
  "glyphs",
  "encoding",
  "zhconvert",
] as const;

const currentColumn = computed<PlatformColumn | null>(() => {
  const value = capabilities.value;
  if (!value) return null;
  if (value.platform === "windows") return "windows";
  if (value.platform === "macos") return "macos";
  if (value.displayServer === "x11") return "x11";
  if (value.displayServer === "wayland") return "wayland";
  return null;
});

function cellLabel(cell: Cell): string {
  return "support" in cell ? t(`about.support.${cell.support}`) : t(`about.details.${cell.detail}`);
}

function capabilityTone(cell: Cell): "success" | "warning" | "info" | undefined {
  if (!("support" in cell)) return undefined;
  if (cell.support === "full") return "success";
  if (cell.support === "notApplicable") return "info";
  return "warning";
}
</script>

<template>
  <section class="page-stack">
    <header class="page-header">
      <div>
        <p class="eyebrow">ABOUT</p>
        <h1>ConvertZZ 2.0</h1>
        <p>{{ t("about.subtitle") }}</p>
      </div>
      <div class="header-actions">
        <el-button @click="replayOnboarding?.()">{{ t("about.replayOnboarding") }}</el-button>
        <el-button :loading="checkingUpdate" @click="checkForUpdates">{{
          t("about.checkUpdates")
        }}</el-button>
        <el-button
          type="primary"
          @click="openUrl('https://github.com/flier268/ConvertZZ/issues')"
          >{{ t("about.reportIssue") }}</el-button
        >
      </div>
    </header>
    <div class="page-fill-main about-stack">
      <el-card shadow="never" class="about-hero">
        <div class="about-mark"><BrandMark /></div>
        <div>
          <h2>{{ t("about.hero.title") }}</h2>
          <p>{{ t("about.hero.body") }}</p>
        </div>
      </el-card>
      <div class="about-diff-grid">
        <el-card shadow="never" class="about-panel">
          <template #header>
            <div class="about-panel-header">
              <div class="section-title">{{ t("about.platform.title") }}</div>
              <p class="muted">{{ t("about.platform.subtitle") }}</p>
            </div>
          </template>
          <el-alert
            v-if="capabilities"
            class="capability-alert"
            :title="
              t('about.platform.currentEnvironment', {
                platform: capabilities.platform,
                displayServer: capabilities.displayServer,
              })
            "
            :description="
              capabilities?.limitations.length
                ? capabilities.limitations.join(' ')
                : t('about.platform.noLimitations')
            "
            :type="capabilities?.limitations.length ? 'warning' : 'info'"
            :closable="false"
            show-icon
          />
          <table class="capability-table">
            <thead>
              <tr>
                <th scope="col">{{ t("about.platform.feature") }}</th>
                <th
                  v-for="column in platformColumns"
                  :key="column.key"
                  scope="col"
                  :class="{ 'is-current': currentColumn === column.key }"
                >
                  {{ column.label
                  }}<small v-if="currentColumn === column.key">{{
                    t("about.platform.current")
                  }}</small>
                </th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="row in differences" :key="row.feature">
                <th scope="row">{{ t(`about.features.${row.feature}`) }}</th>
                <td
                  v-for="column in platformColumns"
                  :key="column.key"
                  :class="{ 'is-current': currentColumn === column.key }"
                >
                  <el-tag
                    v-if="capabilityTone(row[column.key])"
                    :type="capabilityTone(row[column.key])"
                    effect="light"
                    size="small"
                  >
                    {{ cellLabel(row[column.key]) }}
                  </el-tag>
                  <span v-else class="capability-detail">{{ cellLabel(row[column.key]) }}</span>
                </td>
              </tr>
            </tbody>
          </table>
        </el-card>
        <el-card shadow="never" class="about-panel">
          <template #header>
            <div class="about-panel-header">
              <div class="section-title">{{ t("about.conversion.title") }}</div>
              <p class="muted">{{ t("about.conversion.subtitle") }}</p>
            </div>
          </template>
          <ul class="conversion-diff-list">
            <li v-for="item in conversionDifferences" :key="item">
              <strong>{{ t(`about.conversion.${item}.title`) }}</strong>
              <p>{{ t(`about.conversion.${item}.body`) }}</p>
            </li>
          </ul>
        </el-card>
      </div>
    </div>
  </section>
</template>

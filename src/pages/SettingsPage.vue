<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { open as openFile } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import { computed, onMounted, onUnmounted, ref } from "vue";
import type {
  PlatformCapabilities,
  QuickActionSettings,
  SettingsV2,
  ShortcutSetting,
  UiLanguage,
} from "@shared/contracts";
import {
  getLoadedSettings,
  importLegacySettings,
  loadSettings,
  onSettingsReplaced,
  saveSettings,
} from "../lib/settings";
import { core } from "../lib/coreClient";
import { applyDesktopSettings } from "../lib/desktop";
import { acceleratorFromKeyboardEvent, assignShortcutAccelerator } from "../lib/hotkey";
import { legacyActions } from "../lib/legacyActions";
import { importFailureMessage } from "../lib/settingsApply";
import FilterProfilesPanel from "../components/FilterProfilesPanel.vue";
import { openUrl } from "@tauri-apps/plugin-opener";
import { t } from "@/i18n";
import { applyUiLanguage } from "@/lib/uiLanguage";

defineOptions({ name: "SettingsPage" });

const settings = ref<SettingsV2>();
const capabilities = ref<PlatformCapabilities>();
const apiKey = ref("");
const modulesJson = ref("{}");
const busy = ref(false);
const importing = ref(false);
const savedSnapshot = ref("");
const activeTab = ref("general");
const gestureModifiers = ["Ctrl", "Alt", "Shift"] as const;
type GestureModifier = (typeof gestureModifiers)[number];
const uiLanguages: UiLanguage[] = ["system", "zh-TW", "en"];
const gestureGroups = computed<
  Array<{
    id: string;
    title: string;
    hint: string;
    rows: Array<{
      id: string;
      label: string;
      keys: Record<GestureModifier, keyof QuickActionSettings>;
    }>;
  }>
>(() => [
  {
    id: "click",
    title: t("settings.floating.click"),
    hint: t("settings.floating.clickHint"),
    rows: [
      {
        id: "left",
        label: t("settings.floating.leftButton"),
        keys: { Ctrl: "leftClickCtrl", Alt: "leftClickAlt", Shift: "leftClickShift" },
      },
      {
        id: "right",
        label: t("settings.floating.rightButton"),
        keys: { Ctrl: "rightClickCtrl", Alt: "rightClickAlt", Shift: "rightClickShift" },
      },
    ],
  },
  {
    id: "drop",
    title: t("settings.floating.drop"),
    hint: t("settings.floating.dropHint"),
    rows: [
      {
        id: "left",
        label: t("settings.floating.leftButton"),
        keys: { Ctrl: "leftDropCtrl", Alt: "leftDropAlt", Shift: "leftDropShift" },
      },
      {
        id: "right",
        label: t("settings.floating.rightButton"),
        keys: { Ctrl: "rightDropCtrl", Alt: "rightDropAlt", Shift: "rightDropShift" },
      },
    ],
  },
]);

function quickActionValue(key: keyof QuickActionSettings): string {
  return settings.value?.quickActions[key] ?? "0";
}

function setQuickActionValue(key: keyof QuickActionSettings, value: string): void {
  if (settings.value) settings.value.quickActions[key] = value;
}

function settingsSnapshot(value: SettingsV2, modules: string): string {
  return JSON.stringify({ settings: value, modules });
}

function bindSettings(value: SettingsV2): void {
  settings.value = value;
  modulesJson.value = JSON.stringify(value.zhconvert.modules, null, 2);
  savedSnapshot.value = settingsSnapshot(value, modulesJson.value);
}

const dirty = computed(() => {
  const value = settings.value;
  if (!value) return false;
  return settingsSnapshot(value, modulesJson.value) !== savedSnapshot.value;
});

function captureAccelerator(shortcut: ShortcutSetting, event: KeyboardEvent): void {
  event.preventDefault();
  event.stopPropagation();
  assignShortcutAccelerator(shortcut, acceleratorFromKeyboardEvent(event));
}

const alreadyLoaded = getLoadedSettings();
if (alreadyLoaded) bindSettings(alreadyLoaded);

void invoke<PlatformCapabilities>("platform_capabilities").then((value) => {
  capabilities.value = value;
});

const stopReplacedListener = onSettingsReplaced(() => {
  const loaded = getLoadedSettings();
  if (loaded) bindSettings(loaded);
});

onMounted(async () => {
  if (!settings.value) bindSettings(await loadSettings());
});

onUnmounted(() => {
  stopReplacedListener();
});

async function save() {
  if (!settings.value) return;
  busy.value = true;
  try {
    const parsedModules = JSON.parse(modulesJson.value) as Record<string, unknown>;
    if (
      !parsedModules ||
      Array.isArray(parsedModules) ||
      Object.values(parsedModules).some((value) => ![-1, 0, 1].includes(Number(value)))
    ) {
      throw new Error(t("settings.modulesInvalid"));
    }
    settings.value.zhconvert.modules = Object.fromEntries(
      Object.entries(parsedModules).map(([key, value]) => [key, Number(value) as -1 | 0 | 1]),
    );
    await saveSettings();
    savedSnapshot.value = settingsSnapshot(settings.value, modulesJson.value);
    const warnings = await applyDesktopSettings(settings.value);
    ElMessage.success(t("settings.saved"));
    warnings.forEach((warning) => ElMessage.warning(warning));
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    busy.value = false;
  }
}

async function importLegacyJson() {
  const path = await openFile({
    multiple: false,
    filters: [{ name: t("settings.legacyFilterName"), extensions: ["json"] }],
  });
  if (!path) return;
  importing.value = true;
  try {
    const imported = await importLegacySettings(path as string);
    if (!imported) return;
    bindSettings(imported);
    await applyUiLanguage(imported.language);
    const warnings = await applyDesktopSettings(imported);
    ElMessage.success(t("settings.importedLegacy"));
    warnings.forEach((warning) => ElMessage.warning(warning));
  } catch (error) {
    ElMessage.error(importFailureMessage(error));
  } finally {
    importing.value = false;
  }
}

async function saveApiKey() {
  const persisted = await invoke<boolean>("save_zhconvert_api_key", { apiKey: apiKey.value }).catch(
    () => false,
  );
  await core.request("zhconvert.configure", { apiKey: apiKey.value });
  apiKey.value = "";
  ElMessage.success(
    persisted ? t("settings.zhconvert.keySaved") : t("settings.zhconvert.keySessionOnly"),
  );
}

async function changeLanguage(language: UiLanguage) {
  if (!settings.value) return;
  settings.value.language = language;
  await applyUiLanguage(language);
  await save();
}
</script>

<template>
  <section v-if="settings" class="page-stack">
    <header class="page-header">
      <div>
        <p class="eyebrow">PREFERENCES</p>
        <h1>{{ t("settings.title") }}</h1>
        <p>{{ t("settings.intro") }}</p>
      </div>
      <div class="header-actions">
        <el-button :loading="importing" @click="importLegacyJson">{{
          t("settings.importLegacy")
        }}</el-button>
      </div>
    </header>
    <el-card shadow="never" class="page-fill-main settings-main-card">
      <el-tabs v-model="activeTab" class="settings-tabs">
        <el-tab-pane :label="t('settings.tabs.general')" name="general" lazy>
          <el-form label-position="top" class="option-grid"
            ><el-form-item :label="t('settings.general.language')"
              ><el-select
                :model-value="settings.language"
                @update:model-value="changeLanguage($event as UiLanguage)"
                ><el-option
                  v-for="language in uiLanguages"
                  :key="language"
                  :label="t(`common.language.${language}`)"
                  :value="language"
              /></el-select>
              <p class="muted">{{ t("settings.general.languageHint") }}</p></el-form-item
            ><el-form-item :label="t('settings.general.engine')"
              ><el-select v-model="settings.engine"
                ><el-option
                  :label="t('settings.general.engineSegmented')"
                  value="segmented" /><el-option
                  label="ZhConvert"
                  value="zhconvert" /></el-select></el-form-item
            ><el-form-item :label="t('settings.general.direction')"
              ><el-select v-model="settings.direction"
                ><el-option :label="t('common.direction.s2t')" value="s2t" /><el-option
                  :label="t('common.direction.t2s')"
                  value="t2s" /></el-select></el-form-item
            ><el-form-item :label="t('settings.general.previewLimit')" class="field-with-suffix"
              ><div class="field-with-suffix-row">
                <el-input-number v-model="settings.previewMaxKb" :min="1" :max="1024" /><span
                  class="field-suffix"
                  >{{ t("settings.general.previewUnit") }}</span
                >
              </div></el-form-item
            ></el-form
          >
          <div class="settings-section">
            <div class="section-title">{{ t("settings.general.conversion") }}</div>
            <div class="settings-toggle-grid">
              <el-checkbox v-model="settings.vocabularyCorrection">{{
                t("settings.general.vocabularyCorrection")
              }}</el-checkbox>
              <el-checkbox v-model="settings.recognizeEncoding">{{
                t("settings.general.recognizeEncoding")
              }}</el-checkbox>
              <el-tooltip
                :content="t('settings.general.autoBackupTip')"
                placement="top"
                :show-after="300"
              >
                <el-checkbox v-model="settings.autoBackupBeforeConversion">{{
                  t("settings.general.autoBackup")
                }}</el-checkbox>
              </el-tooltip>
              <el-tooltip
                :content="t('settings.general.preserveModifiedTimeTip')"
                placement="top"
                :show-after="300"
              >
                <el-checkbox v-model="settings.preserveModifiedTime">{{
                  t("settings.general.preserveModifiedTime")
                }}</el-checkbox>
              </el-tooltip>
              <el-checkbox v-model="settings.promptAfterConversion">{{
                t("settings.general.promptAfterConversion")
              }}</el-checkbox>
            </div>
          </div>
          <div class="settings-section">
            <div class="section-title">{{ t("settings.general.startupAndUpdates") }}</div>
            <div class="settings-toggle-grid">
              <el-checkbox v-model="settings.showMainWindowOnStart">{{
                t("settings.general.showMainWindowOnStart")
              }}</el-checkbox>
              <el-checkbox v-model="settings.checkVersionOnStart">{{
                t("settings.general.checkVersionOnStart")
              }}</el-checkbox>
              <el-tooltip
                :content="t('settings.general.checkPreReleaseTip')"
                placement="top"
                :show-after="300"
              >
                <el-checkbox v-model="settings.checkPreReleaseUpdates">{{
                  t("settings.general.checkPreRelease")
                }}</el-checkbox>
              </el-tooltip>
            </div>
          </div>
          <p v-if="settings.skippedUpdateVersion" class="settings-note">
            {{ t("settings.general.skippedVersion", { version: settings.skippedUpdateVersion }) }}
            <el-button text type="primary" @click="settings.skippedUpdateVersion = ''">{{
              t("settings.general.clearSkipped")
            }}</el-button>
          </p>
          <p v-if="capabilities?.portable" class="settings-note muted">
            {{ t("settings.general.portableNote") }}
          </p>
          <div v-if="capabilities?.sendToShortcut" class="settings-windows">
            <div class="section-title">{{ t("settings.general.windowsIntegration") }}</div>
            <el-button
              @click="
                invoke('set_send_to_shortcut', { enabled: true }).then(() =>
                  ElMessage.success(t('settings.general.sendToCreated')),
                )
              "
              >{{ t("settings.general.createSendTo") }}</el-button
            ><el-button
              @click="
                invoke('set_send_to_shortcut', { enabled: false }).then(() =>
                  ElMessage.success(t('settings.general.sendToRemoved')),
                )
              "
              >{{ t("settings.general.removeSendTo") }}</el-button
            >
          </div>
        </el-tab-pane>
        <el-tab-pane :label="t('settings.tabs.files')" name="files" lazy>
          <FilterProfilesPanel :files="settings.files" />
          <el-form label-position="top"
            ><el-form-item :label="t('settings.files.defaultPath')"
              ><el-input
                v-model="settings.files.defaultPath"
                :placeholder="t('settings.files.defaultPathPlaceholder')" /></el-form-item
            ><el-form-item :label="t('settings.files.fixCharsetExtensions')"
              ><el-select
                v-model="settings.files.fixCharsetExtensions"
                multiple
                allow-create
                filterable
                default-first-option
                style="width: 100%" /></el-form-item></el-form
          ><el-checkbox v-model="settings.files.unicodeAddBom">{{
            t("settings.files.unicodeAddBom")
          }}</el-checkbox>
        </el-tab-pane>
        <el-tab-pane :label="t('settings.tabs.hotkeys')" name="hotkeys" lazy>
          <el-alert
            v-if="!capabilities?.globalShortcuts"
            :title="t('settings.hotkeys.noGlobalShortcuts')"
            type="warning"
            :closable="false"
          />
          <div class="switch-row">
            <el-checkbox v-model="settings.hotkeys.autoCopy">{{
              t("settings.hotkeys.autoCopy")
            }}</el-checkbox
            ><el-checkbox v-model="settings.hotkeys.autoPaste">{{
              t("settings.hotkeys.autoPaste")
            }}</el-checkbox>
          </div>
          <el-table :data="settings.hotkeys.shortcuts"
            ><el-table-column :label="t('settings.hotkeys.enabled')" width="70"
              ><template #default="scope"
                ><el-checkbox v-model="scope.row.enabled" /></template></el-table-column
            ><el-table-column :label="t('settings.hotkeys.action')" min-width="220"
              ><template #default="scope"
                ><el-select v-model="scope.row.action" filterable
                  ><el-option
                    v-for="action in legacyActions()"
                    :key="action.value"
                    :label="action.label"
                    :value="action.value" /></el-select></template></el-table-column
            ><el-table-column :label="t('settings.hotkeys.shortcut')" min-width="220"
              ><template #default="scope"
                ><el-input
                  :model-value="scope.row.accelerator"
                  readonly
                  :placeholder="t('settings.hotkeys.shortcutPlaceholder')"
                  @keydown="captureAccelerator(scope.row, $event)" /></template></el-table-column
          ></el-table>
          <p class="muted">{{ t("settings.hotkeys.hint") }}</p>
        </el-tab-pane>
        <el-tab-pane :label="t('settings.tabs.floating')" name="floating" lazy>
          <label class="floating-enable">
            <el-switch v-model="settings.floatingBall.enabled" />
            <span>
              <span class="section-title">{{ t("settings.floating.show") }}</span>
              <p class="muted">
                {{ t("settings.floating.description") }}
              </p>
            </span>
          </label>
          <el-alert
            v-if="capabilities && !capabilities.floatingAlwaysOnTop"
            :title="t('settings.floating.alwaysOnTopLimited')"
            type="info"
            :closable="false"
            show-icon
            class="floating-limitation"
          />
          <div class="gesture-groups">
            <section v-for="group in gestureGroups" :key="group.id" class="gesture-group">
              <div class="gesture-group-header">
                <div class="section-title">{{ group.title }}</div>
                <p class="muted">{{ group.hint }}</p>
              </div>
              <table class="gesture-matrix">
                <thead>
                  <tr>
                    <th scope="col">{{ t("settings.floating.button") }}</th>
                    <th v-for="modifier in gestureModifiers" :key="modifier" scope="col">
                      {{ modifier }}
                    </th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="row in group.rows" :key="row.id">
                    <th scope="row">{{ row.label }}</th>
                    <td v-for="modifier in gestureModifiers" :key="modifier">
                      <el-select
                        filterable
                        :model-value="quickActionValue(row.keys[modifier])"
                        :aria-label="`${group.title} ${row.label} ${modifier}`"
                        @update:model-value="setQuickActionValue(row.keys[modifier], $event)"
                      >
                        <el-option
                          v-for="action in legacyActions()"
                          :key="action.value"
                          :label="action.label"
                          :value="action.value"
                        />
                      </el-select>
                    </td>
                  </tr>
                </tbody>
              </table>
            </section>
          </div>
        </el-tab-pane>
        <el-tab-pane label="ZhConvert" name="zhconvert" lazy>
          <p class="muted">{{ t("settings.zhconvert.intro") }}</p>
          <div class="control-row">
            <el-input
              v-model="apiKey"
              type="password"
              show-password
              :placeholder="t('settings.zhconvert.apiKeyPlaceholder')"
            /><el-button :disabled="!apiKey" @click="saveApiKey">{{
              t("settings.zhconvert.saveKey")
            }}</el-button
            ><el-button link type="primary" @click="openUrl('https://zhconvert.org')">{{
              t("settings.zhconvert.website")
            }}</el-button>
          </div>
          <el-form label-position="top" class="option-grid"
            ><el-form-item :label="t('settings.zhconvert.converterS2T')"
              ><el-input v-model="settings.zhconvert.converterS2T" /></el-form-item
            ><el-form-item :label="t('settings.zhconvert.converterT2S')"
              ><el-input v-model="settings.zhconvert.converterT2S" /></el-form-item
            ><el-form-item :label="t('settings.zhconvert.jpTextStrategy')"
              ><el-select v-model="settings.zhconvert.jpTextConversionStrategy"
                ><el-option :label="t('settings.zhconvert.strategy.none')" value="none" /><el-option
                  :label="t('settings.zhconvert.strategy.protect')"
                  value="protect" /><el-option
                  :label="t('settings.zhconvert.strategy.protectOnlySameOrigin')"
                  value="protectOnlySameOrigin" /><el-option
                  :label="t('settings.zhconvert.strategy.fix')"
                  value="fix" /></el-select></el-form-item
            ><el-form-item :label="t('settings.zhconvert.jpStyleStrategy')"
              ><el-select v-model="settings.zhconvert.jpStyleConversionStrategy"
                ><el-option :label="t('settings.zhconvert.strategy.none')" value="none" /><el-option
                  :label="t('settings.zhconvert.strategy.protect')"
                  value="protect" /><el-option
                  :label="t('settings.zhconvert.strategy.protectOnlySameOrigin')"
                  value="protectOnlySameOrigin" /><el-option
                  :label="t('settings.zhconvert.strategy.fix')"
                  value="fix" /></el-select></el-form-item
            ><el-form-item :label="t('settings.zhconvert.tabsToSpaces')"
              ><el-input-number
                v-model="settings.zhconvert.translateTabsToSpaces"
                :min="-1"
                :max="16" /></el-form-item
            ><el-form-item :label="t('settings.zhconvert.ignoreTextStyles')"
              ><el-input v-model="settings.zhconvert.ignoreTextStyles" /></el-form-item
            ><el-form-item :label="t('settings.zhconvert.jpTextStyles')"
              ><el-input v-model="settings.zhconvert.jpTextStyles" /></el-form-item
          ></el-form>
          <el-form label-position="top"
            ><el-form-item :label="t('settings.zhconvert.modulesJson')"
              ><el-input
                v-model="modulesJson"
                type="textarea"
                :rows="4"
                placeholder='{"TaiwanPhrase": 1}' /></el-form-item
            ><el-form-item :label="t('settings.zhconvert.preReplace')"
              ><el-input
                v-model="settings.zhconvert.userPreReplace"
                type="textarea"
                :rows="3" /></el-form-item
            ><el-form-item :label="t('settings.zhconvert.postReplace')"
              ><el-input
                v-model="settings.zhconvert.userPostReplace"
                type="textarea"
                :rows="3" /></el-form-item
            ><el-form-item :label="t('settings.zhconvert.protectReplace')"
              ><el-input
                v-model="settings.zhconvert.userProtectReplace"
                type="textarea"
                :rows="3" /></el-form-item
          ></el-form>
          <div class="switch-row">
            <el-checkbox v-model="settings.zhconvert.cleanUpText">{{
              t("settings.zhconvert.cleanUpText")
            }}</el-checkbox
            ><el-checkbox v-model="settings.zhconvert.ensureNewlineAtEof">{{
              t("settings.zhconvert.ensureNewlineAtEof")
            }}</el-checkbox
            ><el-checkbox v-model="settings.zhconvert.trimTrailingWhiteSpaces">{{
              t("settings.zhconvert.trimTrailingWhiteSpaces")
            }}</el-checkbox
            ><el-checkbox v-model="settings.zhconvert.unifyLeadingHyphen">{{
              t("settings.zhconvert.unifyLeadingHyphen")
            }}</el-checkbox>
          </div>
          <p class="legal-note">{{ t("settings.zhconvert.legal") }}</p>
        </el-tab-pane>
      </el-tabs>
    </el-card>
    <div v-if="dirty" class="settings-save-bar">
      <span>{{ t("settings.unsaved") }}</span>
      <el-button type="primary" :loading="busy" @click="save">{{
        t("settings.saveSettings")
      }}</el-button>
    </div>
  </section>
</template>

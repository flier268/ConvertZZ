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
import { LEGACY_ACTIONS } from "../lib/legacyActions";
import { importFailureMessage } from "../lib/settingsApply";
import { openUrl } from "@tauri-apps/plugin-opener";

defineOptions({ name: "SettingsPage" });

const settings = ref<SettingsV2>();
const capabilities = ref<PlatformCapabilities>();
const apiKey = ref("");
const modulesJson = ref("{}");
const busy = ref(false);
const importing = ref(false);
const savedSnapshot = ref("");
const activeTab = ref("general");
const quickActionRows: Array<{ label: string; key: keyof QuickActionSettings }> = [
  { label: "左鍵 + Ctrl", key: "leftClickCtrl" },
  { label: "左鍵 + Alt", key: "leftClickAlt" },
  { label: "左鍵 + Shift", key: "leftClickShift" },
  { label: "右鍵 + Ctrl", key: "rightClickCtrl" },
  { label: "右鍵 + Alt", key: "rightClickAlt" },
  { label: "右鍵 + Shift", key: "rightClickShift" },
  { label: "左鍵拖入 + Ctrl", key: "leftDropCtrl" },
  { label: "左鍵拖入 + Alt", key: "leftDropAlt" },
  { label: "左鍵拖入 + Shift", key: "leftDropShift" },
  { label: "右鍵拖入 + Ctrl", key: "rightDropCtrl" },
  { label: "右鍵拖入 + Alt", key: "rightDropAlt" },
  { label: "右鍵拖入 + Shift", key: "rightDropShift" },
];

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
      throw new Error("ZhConvert 模組必須是值為 -1、0 或 1 的 JSON 物件。");
    }
    settings.value.zhconvert.modules = Object.fromEntries(
      Object.entries(parsedModules).map(([key, value]) => [key, Number(value) as -1 | 0 | 1]),
    );
    await saveSettings();
    savedSnapshot.value = settingsSnapshot(settings.value, modulesJson.value);
    const warnings = await applyDesktopSettings(settings.value);
    ElMessage.success("設定已儲存");
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
    filters: [{ name: "ConvertZZ 舊版設定", extensions: ["json"] }],
  });
  if (!path) return;
  importing.value = true;
  try {
    const imported = await importLegacySettings(path as string);
    if (!imported) return;
    bindSettings(imported);
    const warnings = await applyDesktopSettings(imported);
    ElMessage.success("已讀取舊設定，並另存為 2.0 設定。");
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
  ElMessage.success(persisted ? "API 金鑰已保存於系統憑證庫" : "API 金鑰只保留於目前工作階段");
}
</script>

<template>
  <section v-if="settings" class="page-stack">
    <header class="page-header">
      <div>
        <p class="eyebrow">PREFERENCES</p>
        <h1>設定</h1>
        <p>設定會保存於 Tauri 應用程式資料目錄。可從 1.x 的 ConvertZZ.json 匯入。</p>
      </div>
      <div class="header-actions">
        <el-button :loading="importing" @click="importLegacyJson">匯入 ConvertZZ.json</el-button>
      </div>
    </header>
    <el-card shadow="never">
      <el-tabs v-model="activeTab" class="settings-tabs">
        <el-tab-pane label="一般" name="general" lazy>
          <el-form label-position="top" class="option-grid"
            ><el-form-item label="預設引擎"
              ><el-select v-model="settings.engine"
                ><el-option label="新式分詞" value="segmented" /><el-option
                  label="舊版字典"
                  value="legacy" /><el-option
                  label="ZhConvert"
                  value="zhconvert" /></el-select></el-form-item
            ><el-form-item label="預設方向"
              ><el-select v-model="settings.direction"
                ><el-option label="簡轉繁" value="s2t" /><el-option
                  label="繁轉簡"
                  value="t2s" /></el-select></el-form-item
            ><el-form-item label="預覽上限" class="field-with-suffix"
              ><div class="field-with-suffix-row">
                <el-input-number v-model="settings.previewMaxKb" :min="1" :max="1024" /><span
                  class="field-suffix"
                  >KB</span
                >
              </div></el-form-item
            ></el-form
          >
          <div class="switch-row">
            <el-checkbox v-model="settings.vocabularyCorrection">詞彙修正</el-checkbox
            ><el-checkbox v-model="settings.recognizeEncoding">自動辨識編碼</el-checkbox
            ><el-tooltip
              content="檔案、檔名與音訊標籤寫入前會建立 .bak。選取資料夾時備份整份資料夾，不會對其中每個檔案分別加後綴。命令列可用 /b:f 關閉。"
              placement="top"
              :show-after="300"
            >
              <el-checkbox v-model="settings.autoBackupBeforeConversion"
                >轉換前自動備份</el-checkbox
              >
            </el-tooltip>
            <el-checkbox v-model="settings.promptAfterConversion">完成後提示</el-checkbox
            ><el-checkbox v-model="settings.showMainWindowOnStart">啟動時顯示主視窗</el-checkbox
            ><el-checkbox v-model="settings.checkVersionOnStart">啟動時檢查更新</el-checkbox>
            <el-tooltip
              content="正式版預設只提示正式版。目前執行的不是正式版時，會預設一併檢查開發／預發佈通道。開啟後會檢查 alpha、beta、rc 通道並可自動更新；若正式版較新則下載正式版。"
              placement="top"
              :show-after="300"
            >
              <el-checkbox v-model="settings.checkPreReleaseUpdates"
                >檢查開發／預發佈版本</el-checkbox
              >
            </el-tooltip>
          </div>
          <p v-if="settings.skippedUpdateVersion" class="settings-note">
            已略過 {{ settings.skippedUpdateVersion }}，啟動時不會再詢問此版本。
            <el-button text type="primary" @click="settings.skippedUpdateVersion = ''"
              >清除略過</el-button
            >
          </p>
          <p v-if="capabilities?.portable" class="settings-note muted">
            免安裝版：設定寫在程式目錄的 settings-v2.json，可整包帶走。ZhConvert
            金鑰仍存於系統憑證庫；檢查更新會開啟 GitHub Releases。
          </p>
          <div v-if="capabilities?.sendToShortcut" class="settings-windows">
            <div class="section-title">Windows 整合</div>
            <el-button
              @click="
                invoke('set_send_to_shortcut', { enabled: true }).then(() =>
                  ElMessage.success('SendTo 捷徑已建立'),
                )
              "
              >建立 SendTo 捷徑</el-button
            ><el-button
              @click="
                invoke('set_send_to_shortcut', { enabled: false }).then(() =>
                  ElMessage.success('SendTo 捷徑已移除'),
                )
              "
              >移除 SendTo 捷徑</el-button
            >
          </div>
        </el-tab-pane>
        <el-tab-pane label="檔案" name="files" lazy>
          <el-form label-position="top"
            ><el-form-item label="預設路徑"
              ><el-input
                v-model="settings.files.defaultPath"
                placeholder="留空時使用系統預設路徑" /></el-form-item
            ><el-form-item label="檔案篩選器"
              ><el-input
                v-model="settings.files.typeFilter"
                type="textarea"
                :rows="3"
                placeholder="<文字|*.txt;*.log>/<網頁|*.html;*.htm>" /></el-form-item
            ><el-form-item label="修正 charset 的副檔名"
              ><el-select
                v-model="settings.files.fixCharsetExtensions"
                multiple
                allow-create
                filterable
                default-first-option
                style="width: 100%" /></el-form-item></el-form
          ><el-checkbox v-model="settings.files.unicodeAddBom">Unicode 輸出加入 BOM</el-checkbox>
        </el-tab-pane>
        <el-tab-pane label="快捷鍵" name="hotkeys" lazy>
          <el-alert
            v-if="!capabilities?.globalShortcuts"
            title="目前顯示伺服器無法保證全域快捷鍵。"
            type="warning"
            :closable="false"
          />
          <div class="switch-row">
            <el-checkbox v-model="settings.hotkeys.autoCopy">快捷鍵讀取選取文字</el-checkbox
            ><el-checkbox v-model="settings.hotkeys.autoPaste">轉換後寫回選取文字</el-checkbox>
          </div>
          <el-table :data="settings.hotkeys.shortcuts"
            ><el-table-column label="啟用" width="70"
              ><template #default="scope"
                ><el-checkbox v-model="scope.row.enabled" /></template></el-table-column
            ><el-table-column label="動作" min-width="220"
              ><template #default="scope"
                ><el-select v-model="scope.row.action"
                  ><el-option
                    v-for="action in LEGACY_ACTIONS"
                    :key="action.value"
                    :label="action.label"
                    :value="action.value" /></el-select></template></el-table-column
            ><el-table-column label="快捷鍵" min-width="220"
              ><template #default="scope"
                ><el-input
                  :model-value="scope.row.accelerator"
                  readonly
                  placeholder="點選後按下組合鍵"
                  @keydown="captureAccelerator(scope.row, $event)" /></template></el-table-column
          ></el-table>
          <p class="muted">點選快捷鍵欄位後按下組合鍵，錄製後會自動啟用。Backspace 可清除。</p>
        </el-tab-pane>
        <el-tab-pane label="浮動球" name="floating" lazy>
          <div class="switch-row">
            <el-checkbox v-model="settings.floatingBall.enabled">顯示浮動球</el-checkbox>
          </div>
          <el-table :data="quickActionRows" max-height="360"
            ><el-table-column prop="label" label="手勢" width="170" /><el-table-column label="動作"
              ><template #default="scope"
                ><el-select
                  :model-value="quickActionValue(scope.row.key)"
                  @update:model-value="setQuickActionValue(scope.row.key, $event)"
                  ><el-option
                    v-for="action in LEGACY_ACTIONS"
                    :key="action.value"
                    :label="action.label"
                    :value="action.value" /></el-select></template></el-table-column
          ></el-table>
        </el-tab-pane>
        <el-tab-pane label="ZhConvert" name="zhconvert" lazy>
          <p class="muted">本程式使用繁化姬 API。金鑰不會寫入 settings-v2.json。</p>
          <div class="control-row">
            <el-input
              v-model="apiKey"
              type="password"
              show-password
              placeholder="選填的 ZhConvert API 金鑰"
            /><el-button :disabled="!apiKey" @click="saveApiKey">保存金鑰</el-button
            ><el-button link type="primary" @click="openUrl('https://zhconvert.org')"
              >繁化姬網站</el-button
            >
          </div>
          <el-form label-position="top" class="option-grid"
            ><el-form-item label="簡轉繁轉換器"
              ><el-input v-model="settings.zhconvert.converterS2T" /></el-form-item
            ><el-form-item label="繁轉簡轉換器"
              ><el-input v-model="settings.zhconvert.converterT2S" /></el-form-item
            ><el-form-item label="日文文字策略"
              ><el-select v-model="settings.zhconvert.jpTextConversionStrategy"
                ><el-option label="不處理" value="none" /><el-option
                  label="保護"
                  value="protect" /><el-option
                  label="僅同源保護"
                  value="protectOnlySameOrigin" /><el-option
                  label="修正"
                  value="fix" /></el-select></el-form-item
            ><el-form-item label="日文字形策略"
              ><el-select v-model="settings.zhconvert.jpStyleConversionStrategy"
                ><el-option label="不處理" value="none" /><el-option
                  label="保護"
                  value="protect" /><el-option
                  label="僅同源保護"
                  value="protectOnlySameOrigin" /><el-option
                  label="修正"
                  value="fix" /></el-select></el-form-item
            ><el-form-item label="Tab 轉空白數"
              ><el-input-number
                v-model="settings.zhconvert.translateTabsToSpaces"
                :min="-1"
                :max="16" /></el-form-item
            ><el-form-item label="忽略的字幕樣式"
              ><el-input v-model="settings.zhconvert.ignoreTextStyles" /></el-form-item
            ><el-form-item label="視為日文的字幕樣式"
              ><el-input v-model="settings.zhconvert.jpTextStyles" /></el-form-item
          ></el-form>
          <el-form label-position="top"
            ><el-form-item label="模組 JSON"
              ><el-input
                v-model="modulesJson"
                type="textarea"
                :rows="4"
                placeholder='{"TaiwanPhrase": 1}' /></el-form-item
            ><el-form-item label="轉換前取代"
              ><el-input
                v-model="settings.zhconvert.userPreReplace"
                type="textarea"
                :rows="3" /></el-form-item
            ><el-form-item label="轉換後取代"
              ><el-input
                v-model="settings.zhconvert.userPostReplace"
                type="textarea"
                :rows="3" /></el-form-item
            ><el-form-item label="保護取代"
              ><el-input
                v-model="settings.zhconvert.userProtectReplace"
                type="textarea"
                :rows="3" /></el-form-item
          ></el-form>
          <div class="switch-row">
            <el-checkbox v-model="settings.zhconvert.cleanUpText">清理文字</el-checkbox
            ><el-checkbox v-model="settings.zhconvert.ensureNewlineAtEof">檔尾換行</el-checkbox
            ><el-checkbox v-model="settings.zhconvert.trimTrailingWhiteSpaces"
              >移除行尾空白</el-checkbox
            ><el-checkbox v-model="settings.zhconvert.unifyLeadingHyphen"
              >統一行首連字號</el-checkbox
            >
          </div>
          <p class="legal-note">使用此服務時必須遵守繁化姬的署名與商業使用條款。</p>
        </el-tab-pane>
      </el-tabs>
    </el-card>
    <div v-if="dirty" class="settings-save-bar">
      <span>設定已變更，尚未儲存。</span>
      <el-button type="primary" :loading="busy" @click="save">儲存設定</el-button>
    </div>
  </section>
</template>

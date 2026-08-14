import { reactive, readonly } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { load, type Store } from "@tauri-apps/plugin-store";
import { confirm, message } from "@tauri-apps/plugin-dialog";
import type { Direction, SettingsV2, ZhConvertOptions } from "@shared/contracts";
import { sidecar } from "./sidecar";

const state = reactive<{ ready: boolean; value?: SettingsV2 }>({ ready: false });
let store: Store | undefined;

export async function loadSettings(): Promise<SettingsV2> {
  if (state.value) return state.value;
  store = await load("settings-v2.json", { autoSave: 250 });
  const saved = await store.get<SettingsV2>("settings");
  const legacyPath = saved ? undefined : await invoke<string | null>("legacy_settings_path");
  let value: SettingsV2;
  if (legacyPath && await confirm(
    "找到舊版 ConvertZZ.json。是否先建立備份，再匯入為 2.0 設定？",
    { title: "匯入舊版設定", kind: "warning" },
  )) {
    try {
      const backup = await sidecar.request<{ backupPath: string }>("settings.backup", { path: legacyPath });
      value = await sidecar.request<SettingsV2>("settings.migrate", { path: legacyPath });
      await message(`備份已建立於：\n${backup.backupPath}`, { title: "設定備份完成", kind: "info" });
    } catch (error) {
      await message(`備份失敗。舊版設定未匯入。\n${error instanceof Error ? error.message : String(error)}`, { title: "設定備份失敗", kind: "error" });
      value = await sidecar.request<SettingsV2>("settings.migrate", { input: undefined });
    }
  } else {
    value = await sidecar.request<SettingsV2>("settings.migrate", { input: saved });
  }
  state.value = reactive(value) as SettingsV2;
  state.ready = true;
  await saveSettings();
  return state.value;
}

export async function saveSettings(): Promise<void> {
  if (!state.value) return;
  store ??= await load("settings-v2.json", { autoSave: 250 });
  await store.set("settings", JSON.parse(JSON.stringify(state.value)));
}

export async function replaceSettings(input: unknown): Promise<SettingsV2> {
  const migrated = await sidecar.request<SettingsV2>("settings.migrate", { input });
  state.value = reactive(migrated) as SettingsV2;
  state.ready = true;
  await saveSettings();
  return state.value;
}

export async function importLegacySettings(path: string): Promise<{ settings: SettingsV2; backupPath: string } | undefined> {
  if (!await confirm(
    "匯入會取代目前的 2.0 設定。是否先備份舊版設定，再繼續？",
    { title: "確認匯入", kind: "warning" },
  )) return undefined;
  const backup = await sidecar.request<{ backupPath: string }>("settings.backup", { path });
  const migrated = await sidecar.request<SettingsV2>("settings.migrate", { path });
  return { settings: await replaceSettings(migrated), backupPath: backup.backupPath };
}

export function useSettingsState() {
  return readonly(state);
}

export function zhConvertOptions(settings: SettingsV2, direction: Direction): ZhConvertOptions {
  return {
    converter: direction === "t2s" ? settings.zhconvert.converterT2S : settings.zhconvert.converterS2T,
    modules: settings.zhconvert.modules,
    jpTextConversionStrategy: settings.zhconvert.jpTextConversionStrategy,
    jpStyleConversionStrategy: settings.zhconvert.jpStyleConversionStrategy,
    cleanUpText: settings.zhconvert.cleanUpText,
    userPreReplace: settings.zhconvert.userPreReplace,
    userPostReplace: settings.zhconvert.userPostReplace,
    userProtectReplace: settings.zhconvert.userProtectReplace,
    ensureNewlineAtEof: settings.zhconvert.ensureNewlineAtEof,
    translateTabsToSpaces: settings.zhconvert.translateTabsToSpaces,
    trimTrailingWhiteSpaces: settings.zhconvert.trimTrailingWhiteSpaces,
    unifyLeadingHyphen: settings.zhconvert.unifyLeadingHyphen,
    ignoreTextStyles: settings.zhconvert.ignoreTextStyles,
    jpTextStyles: settings.zhconvert.jpTextStyles,
  };
}

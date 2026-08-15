import { reactive, readonly } from "vue";
import { load, type Store } from "@tauri-apps/plugin-store";
import { confirm } from "@tauri-apps/plugin-dialog";
import type { Direction, SettingsV2, ZhConvertOptions } from "@shared/contracts";
import { ONBOARDING_STORE_KEY } from "./onboarding";
import { sidecar } from "./sidecar";

const state = reactive<{ ready: boolean; value?: SettingsV2 }>({ ready: false });
let store: Store | undefined;

async function settingsStore(): Promise<Store> {
  store ??= await load("settings-v2.json", { autoSave: false });
  return store;
}

function putSettings(value: SettingsV2): SettingsV2 {
  if (state.value) Object.assign(state.value, value);
  else state.value = reactive(value) as SettingsV2;
  state.ready = true;
  return state.value;
}

export async function loadSettings(): Promise<SettingsV2> {
  if (state.value) return state.value;
  return reloadSettings();
}

export async function reloadSettings(): Promise<SettingsV2> {
  const currentStore = await settingsStore();
  await currentStore.reload();
  const saved = await currentStore.get<SettingsV2>("settings");
  const value = await sidecar.request<SettingsV2>("settings.migrate", { input: saved });
  return putSettings(value);
}

export async function saveSettings(): Promise<void> {
  if (!state.value) return;
  const currentStore = await settingsStore();
  await currentStore.set("settings", JSON.parse(JSON.stringify(state.value)));
  await currentStore.save();
}

export async function patchSavedSettings(
  patch: (settings: SettingsV2) => void,
): Promise<SettingsV2> {
  const settings = await reloadSettings();
  patch(settings);
  await saveSettings();
  return settings;
}

export async function isOnboardingComplete(): Promise<boolean> {
  const currentStore = await settingsStore();
  return Boolean(await currentStore.get(ONBOARDING_STORE_KEY));
}

export async function markOnboardingComplete(): Promise<void> {
  const currentStore = await settingsStore();
  await currentStore.set(ONBOARDING_STORE_KEY, true);
  await currentStore.save();
}

export async function clearOnboardingComplete(): Promise<void> {
  const currentStore = await settingsStore();
  await currentStore.set(ONBOARDING_STORE_KEY, false);
  await currentStore.save();
}

export async function replaceSettings(input: unknown): Promise<SettingsV2> {
  const migrated = await sidecar.request<SettingsV2>("settings.migrate", { input });
  putSettings(migrated);
  await saveSettings();
  return state.value!;
}

export async function importLegacySettings(
  path: string,
  options: { confirmReplace?: boolean } = {},
): Promise<SettingsV2 | undefined> {
  if (
    options.confirmReplace !== false &&
    !(await confirm("匯入會取代目前的 2.0 設定。是否繼續？", {
      title: "確認匯入",
      kind: "warning",
    }))
  )
    return undefined;
  const migrated = await sidecar.request<SettingsV2>("settings.migrate", { path });
  return replaceSettings(migrated);
}

export function useSettingsState() {
  return readonly(state);
}

export function zhConvertOptions(settings: SettingsV2, direction: Direction): ZhConvertOptions {
  return {
    converter:
      direction === "t2s" ? settings.zhconvert.converterT2S : settings.zhconvert.converterS2T,
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

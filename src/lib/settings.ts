import { reactive, readonly } from "vue";
import { load, type Store } from "@tauri-apps/plugin-store";
import { confirm } from "@tauri-apps/plugin-dialog";
import type { Direction, SettingsV2, ZhConvertOptions } from "@shared/contracts";
import { ONBOARDING_STORE_KEY } from "./onboarding";
import { core } from "./coreClient";

const state = reactive<{ ready: boolean; value?: SettingsV2 }>({ ready: false });
let store: Store | undefined;
const replacedListeners = new Set<() => void>();

function notifySettingsReplaced(): void {
  for (const listener of replacedListeners) listener();
}

/** 當匯入或整份取代設定後通知畫面重綁（例如 SettingsPage 的 modulesJson 快照）。 */
export function onSettingsReplaced(listener: () => void): () => void {
  replacedListeners.add(listener);
  return () => {
    replacedListeners.delete(listener);
  };
}

async function settingsStore(): Promise<Store> {
  store ??= await load("settings-v2.json", { autoSave: false });
  return store;
}

function isMissingStoreFile(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return /os error 2|ENOENT|cannot find the file specified|no such file or directory|系統找不到指定的檔案/i.test(
    message,
  );
}

async function readStoredSettings(): Promise<SettingsV2 | undefined> {
  const currentStore = await settingsStore();
  try {
    await currentStore.reload();
  } catch (error) {
    // plugin-store 初次 load 會忽略缺檔，但 reload 會把 NotFound 丟回前端。
    if (!isMissingStoreFile(error)) throw error;
  }
  return currentStore.get<SettingsV2>("settings");
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
  const saved = await readStoredSettings();
  const value = await core.request<SettingsV2>("settings.migrate", { input: saved });
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
  const migrated = await core.request<SettingsV2>("settings.migrate", { input });
  putSettings(migrated);
  await saveSettings();
  notifySettingsReplaced();
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
  const migrated = await core.request<SettingsV2>("settings.migrate", { path });
  return replaceSettings(migrated);
}

export function useSettingsState() {
  return readonly(state);
}

export function getLoadedSettings(): SettingsV2 | undefined {
  return state.value;
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

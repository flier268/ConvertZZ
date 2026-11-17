import { reactive, readonly } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { load, type Store } from "@tauri-apps/plugin-store";
import { confirm } from "@tauri-apps/plugin-dialog";
import type {
  Direction,
  PlatformCapabilities,
  SettingsV2,
  ZhConvertOptions,
} from "@shared/contracts";
import { formatUnknownError } from "./errors";
import { ONBOARDING_STORE_KEY } from "./onboarding";
import { core } from "./coreClient";

const state = reactive<{ ready: boolean; value?: SettingsV2 }>({ ready: false });
let store: Store | undefined;
let portableMode: boolean | undefined;
let portableDocument: Record<string, unknown> | undefined;
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

async function isPortableMode(): Promise<boolean> {
  if (portableMode !== undefined) return portableMode;
  const capabilities = await invoke<PlatformCapabilities>("platform_capabilities");
  portableMode = Boolean(capabilities.portable);
  return portableMode;
}

async function settingsStore(): Promise<Store> {
  store ??= await load("settings-v2.json", { autoSave: false });
  return store;
}

function isMissingStoreFile(error: unknown): boolean {
  const message = formatUnknownError(error);
  // Windows 首次啟動常是 os error 3（AppData 目錄尚未建立），不是只有缺檔的 os error 2。
  return /os error [23]|ENOENT|cannot find the (file|path) specified|no such file or directory|系統找不到指定的(檔案|路徑)|系统找不到指定的(文件|路径)|找不到檔案|file not found|path not found/i.test(
    message,
  );
}

async function readPortableDocument(): Promise<Record<string, unknown>> {
  if (portableDocument) return portableDocument;
  const loaded = await invoke<Record<string, unknown> | null>("load_portable_settings_store");
  portableDocument = loaded ?? {};
  return portableDocument;
}

async function writePortableDocument(document: Record<string, unknown>): Promise<void> {
  portableDocument = document;
  await invoke("save_portable_settings_store", { document });
}

async function readStoreValue<T>(key: string): Promise<T | undefined> {
  if (await isPortableMode()) {
    const document = await readPortableDocument();
    return document[key] as T | undefined;
  }
  const currentStore = await settingsStore();
  try {
    await currentStore.reload();
  } catch (error) {
    // plugin-store 初次 load 會忽略缺檔，但 reload 會把 NotFound 丟回前端。
    if (!isMissingStoreFile(error)) throw error;
  }
  return currentStore.get<T>(key);
}

async function writeStoreValue(key: string, value: unknown): Promise<void> {
  if (await isPortableMode()) {
    const document = { ...(await readPortableDocument()), [key]: value };
    await writePortableDocument(document);
    return;
  }
  const currentStore = await settingsStore();
  await currentStore.set(key, value);
  await currentStore.save();
}

async function readStoredSettings(): Promise<SettingsV2 | undefined> {
  return readStoreValue<SettingsV2>("settings");
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
  await writeStoreValue("settings", JSON.parse(JSON.stringify(state.value)));
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
  return Boolean(await readStoreValue(ONBOARDING_STORE_KEY));
}

export async function markOnboardingComplete(): Promise<void> {
  await writeStoreValue(ONBOARDING_STORE_KEY, true);
}

export async function clearOnboardingComplete(): Promise<void> {
  await writeStoreValue(ONBOARDING_STORE_KEY, false);
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

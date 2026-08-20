import type { SettingsV2 } from "../../shared/contracts";

const defaultSettings: SettingsV2 = {
  version: 2,
  engine: "segmented",
  direction: "s2t",
  vocabularyCorrection: true,
  promptAfterConversion: true,
  autoBackupBeforeConversion: true,
  recognizeEncoding: true,
  previewMaxKb: 6,
  floatingBall: { enabled: false, x: -1, y: -1 },
  hotkeys: {
    autoCopy: true,
    autoPaste: true,
    shortcuts: Array.from({ length: 4 }, (_, index) => ({
      enabled: false,
      accelerator: "",
      action: `a${index + 1}`,
    })),
  },
  quickActions: {
    leftClickCtrl: "0",
    leftClickAlt: "0",
    leftClickShift: "0",
    rightClickCtrl: "0",
    rightClickAlt: "0",
    rightClickShift: "0",
    leftDropCtrl: "0",
    leftDropAlt: "0",
    leftDropShift: "0",
    rightDropCtrl: "0",
    rightDropAlt: "0",
    rightDropShift: "0",
  },
  files: {
    defaultPath: "!",
    typeFilter:
      "<常用文字檔案|*.txt;*.log;*.ini;*.inf;*.bat;*.cmd;*.srt;*.ass;*.lang>/<常用網頁文件|*.htm;*.html;*.php;*.asp;*.css;*.js>/<音訊文件|*.mp3;*.ape;*.ogg;*.oga;*.opus>",
    fixCharsetExtensions: [".htm", ".html", ".shtm", ".shtml", ".asp", ".aspx", ".php", ".css"],
    unicodeAddBom: false,
  },
  zhconvert: {
    converterS2T: "Taiwan",
    converterT2S: "Simplified",
    modules: {},
    jpTextConversionStrategy: "protectOnlySameOrigin",
    jpStyleConversionStrategy: "protectOnlySameOrigin",
    cleanUpText: false,
    userPreReplace: "",
    userPostReplace: "",
    userProtectReplace: "",
    ensureNewlineAtEof: false,
    translateTabsToSpaces: -1,
    trimTrailingWhiteSpaces: false,
    unifyLeadingHyphen: false,
    ignoreTextStyles: "",
    jpTextStyles: "",
  },
  checkVersionOnStart: false,
  checkPreReleaseUpdates: false,
  skippedUpdateVersion: "",
  showMainWindowOnStart: true,
  lastDropAction: {
    kind: "file",
    operation: "content",
    direction: "s2t",
  },
};

export interface ConvertzzE2eConfig {
  onboardingCompleted?: boolean;
  legacySettingsPath?: string | null;
  skippedUpdateVersion?: string;
  checkVersionOnStart?: boolean;
  selectedFiles?: string | string[];
  clipboardText?: string;
  confirmResult?: boolean;
  update?: "none" | "install";
  confirms?: string[];
  lastOpenedUrl?: string;
}

declare global {
  interface Window {
    __convertzzE2e?: ConvertzzE2eConfig;
    __convertzzEmit?: (event: string, payload?: unknown) => Promise<void>;
  }
}

function e2e(): ConvertzzE2eConfig {
  const host = globalThis as typeof globalThis & { __convertzzE2e?: ConvertzzE2eConfig };
  host.__convertzzE2e ??= { confirms: [] };
  host.__convertzzE2e.confirms ??= [];
  return host.__convertzzE2e;
}

function initialSettings(): SettingsV2 {
  const settings = structuredClone(defaultSettings);
  const config = e2e();
  if (config.skippedUpdateVersion !== undefined)
    settings.skippedUpdateVersion = config.skippedUpdateVersion;
  if (config.checkVersionOnStart !== undefined)
    settings.checkVersionOnStart = config.checkVersionOnStart;
  return settings;
}

const store = new Map<string, unknown>([
  ["onboardingCompleted", e2e().onboardingCompleted !== false],
  ["settings", initialSettings()],
]);
const listeners = new Map<string, Set<(event: { payload: unknown }) => void>>();
let clipboard = "";

export function isTauri(): boolean {
  return true;
}

export async function listen<T>(
  event: string,
  handler: (event: { payload: T }) => void,
): Promise<() => void> {
  const bucket = listeners.get(event) ?? new Set();
  const wrapped = handler as (event: { payload: unknown }) => void;
  bucket.add(wrapped);
  listeners.set(event, bucket);
  return () => bucket.delete(wrapped);
}

export async function emit(event: string, payload?: unknown): Promise<void> {
  for (const handler of listeners.get(event) ?? []) handler({ payload });
}

if (typeof window !== "undefined") {
  window.__convertzzEmit = emit;
}

export async function invoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  switch (command) {
    case "core_request":
      return mockCoreRequest(String(args.operation ?? ""), args.payload) as T;
    case "app_log":
      return undefined as T;
    case "app_log_path":
      return null as T;
    case "startup_args":
      return [] as T;
    case "legacy_settings_path":
      return (e2e().legacySettingsPath ?? null) as T;
    case "load_zhconvert_api_key":
      return null as T;
    case "platform_capabilities":
      return {
        platform: "linux",
        displayServer: "x11",
        globalShortcuts: true,
        automaticCopyPaste: true,
        floatingAlwaysOnTop: true,
        tray: true,
        sendToShortcut: false,
        credentialStorage: true,
        portable: false,
        automaticUpdates: true,
        limitations: [],
      } as T;
    case "load_portable_settings_store":
      return null as T;
    case "save_portable_settings_store":
      return undefined as T;
    case "capture_selection":
      return clipboard as T;
    case "show_main_window":
    case "show_toast":
    case "quit_app":
    case "save_zhconvert_api_key":
    case "set_send_to_shortcut":
    case "replace_selection":
      return undefined as T;
    default:
      throw new Error(`未模擬的 Tauri 指令：${command}`);
  }
}

function mockCoreRequest(operation: string, rawPayload: unknown): unknown {
  const payload = (rawPayload ?? {}) as Record<string, unknown>;
  if (operation === "health") return { engine: "rust", version: "2.0.0-beta5" };
  if (operation === "settings.migrate") {
    const input = payload.input as SettingsV2 | undefined;
    return input?.version === 2 ? input : structuredClone(defaultSettings);
  }
  if (operation === "files.plan") {
    const mode = String(payload.mode ?? "content");
    const rename = mode === "filename" || mode === "both";
    const content = mode === "content" || mode === "both";
    return {
      planId: "plan-1",
      createdAt: "2026-08-15T00:00:00.000Z",
      warnings: [],
      items: [
        {
          sourcePath: "/tmp/里面.txt",
          outputPath: rename ? "/tmp/裡面.txt" : "/tmp/里面.txt",
          kind: rename && !content ? "filename" : "file",
          selected: true,
          detectedEncoding: "utf8",
          sourcePreview: content ? "里面开发头发" : "里面.txt",
          outputPreview: content ? "裡面開發頭髮" : "裡面.txt",
          status: "ready",
        },
      ],
    };
  }
  if (operation === "files.apply") {
    return { succeeded: ["/tmp/里面.txt"], skipped: [], failed: [] };
  }
  if (operation === "files.cancel") {
    return { cancelled: true };
  }
  if (operation === "audio.scan") {
    return [
      {
        path: "/tmp/song.mp3",
        format: "mp3",
        selected: true,
        hasCoverArt: true,
        fields: [
          {
            key: "title",
            label: "標題",
            container: "id3v2",
            values: ["里面"],
            selected: true,
          },
          {
            key: "artist",
            label: "演出者",
            container: "id3v2",
            values: ["头发"],
            selected: true,
          },
          {
            key: "frame:TIT3",
            label: "TIT3",
            container: "id3v2",
            values: ["未知字幕"],
            selected: false,
          },
        ],
      },
    ];
  }
  if (operation === "audio.plan") {
    return {
      planId: "audio-plan-1",
      createdAt: "2026-08-15T00:00:00.000Z",
      warnings: [],
      files: [
        {
          path: "/tmp/song.mp3",
          format: "mp3",
          selected: true,
          hasCoverArt: true,
          fields: [
            {
              key: "title",
              label: "標題",
              container: "id3v2",
              values: ["里面"],
              convertedValues: ["裡面"],
              selected: true,
            },
            {
              key: "artist",
              label: "演出者",
              container: "id3v2",
              values: ["头发"],
              convertedValues: ["頭髮"],
              selected: true,
            },
            {
              key: "frame:TIT3",
              label: "TIT3",
              container: "id3v2",
              values: ["未知字幕"],
              selected: false,
            },
          ],
        },
      ],
    };
  }
  if (operation === "audio.apply") {
    return { succeeded: ["/tmp/song.mp3"], skipped: [], failed: [] };
  }
  if (operation === "audio.cancel") {
    return { cancelled: true };
  }
  if (operation === "dictionary.read") {
    return {
      path: String(payload.path ?? "/tmp/Dictionary.csv"),
      total: 1,
      entries: [
        {
          index: 0,
          enabled: true,
          type: "test",
          simplified: "里面",
          simplifiedPriority: 1,
          traditional: "裡面",
          traditionalPriority: 1,
        },
      ],
    };
  }
  if (operation === "dictionary.update") {
    return { updated: 1, backupPath: "/tmp/Dictionary.backup-20260817.csv" };
  }
  if (operation === "dictionary.preview") {
    return { text: "裡面" };
  }
  if (operation === "convert.preview") {
    return { text: "裡面開發頭髮", durationMs: 1 };
  }
  if (operation === "utility.convert") {
    return { text: String(payload.text ?? "") };
  }
  return {};
}

export async function getVersion(): Promise<string> {
  return "2.0.0-beta5";
}

export class LogicalPosition {
  constructor(
    public x: number,
    public y: number,
  ) {}
}

export function getCurrentWindow() {
  return {
    label: "main",
    hide: async () => undefined,
    show: async () => undefined,
    setFocus: async () => undefined,
    setPosition: async () => undefined,
    setSize: async () => undefined,
    outerPosition: async () => ({ x: 0, y: 0 }),
    listen: async () => () => undefined,
  };
}

export async function getAllWindows() {
  return [getCurrentWindow()];
}

export class Menu {}
export class MenuItem {}
export class PredefinedMenuItem {}
export class Submenu {}

export async function load() {
  return {
    get: async <T>(key: string) => store.get(key) as T | undefined,
    set: async (key: string, value: unknown) => {
      store.set(key, value);
    },
    save: async () => undefined,
    reload: async () => undefined,
  };
}

export async function confirm(message: string): Promise<boolean> {
  const config = e2e();
  config.confirms?.push(message);
  return config.confirmResult !== false;
}

export async function open(): Promise<string | string[] | null> {
  return e2e().selectedFiles ?? "/tmp/里面.txt";
}

export async function readText(): Promise<string> {
  const seeded = e2e().clipboardText;
  if (!clipboard && seeded) clipboard = seeded;
  return clipboard;
}

export async function writeText(value: string): Promise<void> {
  clipboard = value;
  e2e().clipboardText = value;
}

export async function openUrl(url: string): Promise<void> {
  e2e().lastOpenedUrl = url;
}

export async function relaunch(): Promise<void> {}

export async function check() {
  if (e2e().update !== "install") return null;
  return {
    currentVersion: "2.0.0-beta5",
    version: "2.1.0",
    body: "測試更新",
    close: async () => undefined,
    downloadAndInstall: async () => undefined,
  };
}

export async function register(): Promise<void> {}

export async function unregisterAll(): Promise<void> {}

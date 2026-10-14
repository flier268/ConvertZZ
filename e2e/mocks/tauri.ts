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
      "<常用文字檔案|*.txt;*.log;*.ini;*.inf;*.bat;*.cmd;*.srt;*.ass;*.lang>/<常用網頁文件|*.htm;*.html;*.php;*.asp;*.css;*.js>",
    filterProfiles: [
      {
        id: "builtin-text",
        name: "常用文字檔案",
        builtin: true,
        mode: "allow",
        extensions: [".txt", ".log", ".ini", ".inf", ".bat", ".cmd", ".srt", ".ass", ".lang"],
        includeNoExtension: false,
      },
      {
        id: "builtin-all",
        name: "所有檔案",
        builtin: true,
        mode: "all",
        extensions: [],
        includeNoExtension: true,
      },
      {
        id: "legacy-2",
        name: "常用網頁文件",
        builtin: false,
        mode: "allow",
        extensions: [".htm", ".html", ".php", ".asp", ".css", ".js"],
        includeNoExtension: false,
      },
    ],
    defaultFilterProfileId: "builtin-text",
    lastFilterProfileId: "",
    folderPickBehavior: "rememberLast",
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
  selectedDirectory?: string;
  clipboardText?: string;
  confirmResult?: boolean;
  update?: "none" | "install";
  confirms?: string[];
  lastOpenedUrl?: string;
  holdFileApply?: boolean;
  releaseFileApply?: () => void;
  failAudioScan?: boolean;
  failAudioPlan?: boolean;
  holdAudioRescan?: boolean;
  releaseAudioRescan?: () => void;
  holdAudioScan?: boolean;
  releaseAudioScan?: () => void;
  lastAudioApplyPaths?: string[];
  lastAudioApply?: {
    paths?: string[];
    selectedFields?: Record<string, string[]>;
  };
  lastAudioPlan?: {
    tagOperations?: Array<{ action?: string; source?: string }>;
  };
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
let lastFilePlanItem: Record<string, unknown> | undefined;
let lastCoreRequestId = "";

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
      lastCoreRequestId = String(args.id ?? "");
      return (await mockCoreRequest(String(args.operation ?? ""), args.payload)) as T;
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
    case "check_signed_update": {
      if (e2e().update !== "install") return null as T;
      const endpoint = String(args.endpoint ?? "");
      const version = /\/download\/v([^/]+)\/latest\.json/u.exec(endpoint)?.[1] ?? "2.1.0";
      return {
        rid: 1,
        currentVersion: "2.0.0-beta10",
        version,
        body: "測試更新",
        rawJson: {},
      } as T;
    }
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

async function mockCoreRequest(operation: string, rawPayload: unknown): Promise<unknown> {
  const payload = (rawPayload ?? {}) as Record<string, unknown>;
  if (operation === "health") return { engine: "rust", version: "2.0.0-beta10" };
  if (operation === "settings.migrate") {
    const input = payload.input as SettingsV2 | undefined;
    return input?.version === 2 ? input : structuredClone(defaultSettings);
  }
  if (operation === "files.plan") {
    const mode = String(payload.mode ?? "content");
    const rename = mode === "filename" || mode === "both";
    const content = mode === "content" || mode === "both";
    const paths = Array.isArray(payload.paths) ? payload.paths.map(String) : [];
    const directory = e2e().selectedDirectory;
    const scanningFolder = Boolean(directory && paths.includes(directory));
    const filter = payload.extensionFilter as
      { mode: string; extensions: string[]; includeNoExtension: boolean } | undefined;
    const candidates = scanningFolder
      ? [`${directory}/里面.txt`, `${directory}/DATA.BIN`, `${directory}/README`]
      : ["/tmp/里面.txt"];
    const visible = candidates.filter((path) => {
      if (!scanningFolder || !filter || filter.mode === "all") return true;
      const name = path.split("/").pop() ?? path;
      const dot = name.lastIndexOf(".");
      const extension = dot >= 0 ? name.slice(dot).toLowerCase() : "";
      const listed = extension ? filter.extensions.includes(extension) : filter.includeNoExtension;
      return filter.mode === "allow" ? listed : !listed;
    });
    const items = visible.map((sourcePath) => {
      const fileName = sourcePath.split("/").pop() ?? sourcePath;
      const outputName = fileName === "里面.txt" && rename ? "裡面.txt" : fileName;
      const outputPath = sourcePath.slice(0, sourcePath.length - fileName.length) + outputName;
      return {
        sourcePath,
        outputPath,
        kind: "file",
        selected: true,
        detectedEncoding: content ? undefined : "utf8",
        sourcePreview: content ? "" : fileName,
        outputPreview: content ? "" : outputName,
        previewLoaded: !content,
        status: "ready",
      };
    });
    lastFilePlanItem = items[0];
    return {
      planId: "plan-1",
      createdAt: "2026-08-15T00:00:00.000Z",
      warnings: [],
      ignoredInputs: [],
      items,
    };
  }
  if (operation === "files.scanExtensions") {
    return {
      extensions: [
        { extension: ".bin", count: 1 },
        { extension: ".txt", count: 1 },
      ],
      noExtensionCount: 1,
      totalFiles: 3,
      directoryCount: 0,
      warnings: [],
    };
  }
  if (operation === "files.preview") {
    const base = lastFilePlanItem ?? {
      sourcePath: "/tmp/里面.txt",
      outputPath: "/tmp/里面.txt",
      kind: "file",
      selected: true,
      status: "ready",
    };
    const pad = (ch: string, n: number) => ch.repeat(n);
    lastFilePlanItem = {
      ...base,
      detectedEncoding: "utf8",
      // 長文方便驗證全視窗分頁；仍保留既有短句供 e2e 斷言。
      sourcePreview: `${pad("甲", 1100)}里面开发头发${pad("乙", 1100)}`,
      outputPreview: `${pad("甲", 1100)}裡面開發頭髮${pad("乙", 1100)}`,
      previewLoaded: true,
    };
    return lastFilePlanItem;
  }
  if (operation === "files.apply") {
    if (e2e().holdFileApply) {
      await emit("core://progress", {
        id: lastCoreRequestId,
        current: 658,
        total: 947,
        message: "正在轉換並寫入：shard-000219.txt (658/947)",
      });
      await new Promise<void>((resolve) => {
        e2e().releaseFileApply = resolve;
      });
    }
    return { succeeded: ["/tmp/里面.txt"], skipped: [], failed: [] };
  }
  if (operation === "files.cancel") {
    return { cancelled: true };
  }
  if (operation === "core.cancel") {
    return { cancelled: true };
  }
  if (operation === "audio.prioritize") {
    return { ok: true };
  }
  if (operation === "audio.scan") {
    if (e2e().failAudioScan) throw new Error("讀不到音訊標籤");
    if (e2e().holdAudioScan) {
      const partial = Array.from({ length: 30 }, (_, index) => ({
        path: `/music/合輯/live-${String(index).padStart(2, "0")}.mp3`,
        format: "mp3",
        selected: true,
        fields: [
          {
            key: "title",
            label: "標題",
            container: "id3v2",
            values: [`即時${index}`],
            selected: true,
          },
        ],
      }));
      await emit("core://progress", {
        id: lastCoreRequestId,
        current: partial.length,
        total: 250,
        message: "正在掃描：live-29.mp3",
        batch: { phase: "scan", files: partial },
      });
      await new Promise<void>((resolve) => {
        e2e().releaseAudioScan = resolve;
      });
      return partial;
    }
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
    if (e2e().failAudioPlan) throw new Error("無法建立標籤預覽");
    const tagOperations = Array.isArray(payload.tagOperations) ? payload.tagOperations : [];
    e2e().lastAudioPlan = {
      tagOperations: tagOperations as Array<{ action?: string; source?: string }>,
    };
    if (tagOperations.length > 0) {
      const song = {
        path: "/music/專輯甲/song.mp3",
        format: "mp3" as const,
        selected: true,
        previewed: true,
        fields: [
          {
            key: "title",
            label: "標題",
            container: "id3v1" as const,
            values: ["裡面的很長標題不會完整寫進固定欄位🙂"],
            convertedValues: ["裡面的很長標題?"],
            selected: true,
            truncated: true,
            origin: "rule 1",
            truncation: "截斷 40→30 B",
            warnings: [
              { code: "truncated", message: "截斷 40→30 B", fromBytes: 40, toBytes: 30 },
              { code: "unrepresentable", message: "1 字 Big5 無法表示", count: 1 },
            ],
          },
        ],
        removals: [
          {
            container: "apev2" as const,
            hiddenFieldCount: 2,
            origin: "rule 2",
            selected: true,
          },
        ],
      };
      const apeOnly = {
        path: "/music/專輯甲/ape-only.mp3",
        format: "mp3" as const,
        selected: true,
        previewed: true,
        fields: [
          {
            key: "title",
            label: "標題",
            container: "apev2" as const,
            values: ["舊標題"],
            selected: false,
          },
        ],
        removals: [
          {
            container: "apev2" as const,
            hiddenFieldCount: 3,
            origin: "rule 2",
            selected: true,
          },
        ],
      };
      return {
        planId: "audio-plan-ops",
        createdAt: "2026-08-15T00:00:00.000Z",
        summary: { total: 2, changed: 2, unchanged: 0, errors: 0, selectedFields: 2 },
        files: [song, apeOnly],
      };
    }
    const song = {
      path: "/music/專輯甲/song.mp3",
      format: "mp3" as const,
      selected: true,
      hasCoverArt: true,
      fields: [
        {
          key: "title",
          label: "標題",
          container: "id3v2" as const,
          values: ["里面"],
          convertedValues: ["裡面"],
          selected: true,
        },
        {
          key: "artist",
          label: "演出者",
          container: "id3v2" as const,
          values: ["头发"],
          convertedValues: ["頭髮"],
          selected: true,
        },
        {
          key: "album",
          label: "專輯",
          container: "id3v2" as const,
          values: ["晨雾"],
          convertedValues: ["晨霧"],
          selected: true,
        },
        {
          key: "title",
          label: "標題",
          container: "id3v1" as const,
          values: ["裡面的很長標題不會完整寫進固定欄位"],
          convertedValues: ["裡面的很長標題"],
          selected: false,
          truncated: true,
          truncation: "「ID3v1 標題超過 30 位元組，已截斷為 裡面的很長標題」",
        },
        {
          key: "title",
          label: "標題",
          container: "apev2" as const,
          values: ["後來的我們"],
          selected: false,
        },
        {
          key: "replaygain",
          label: "ReplayGain",
          container: "apev2" as const,
          values: ["-1.5 dB"],
          selected: false,
        },
        {
          key: "frame:TIT3",
          label: "TIT3",
          container: "id3v2" as const,
          values: ["未知字幕"],
          selected: false,
        },
      ],
    };
    const unchanged = {
      path: "/music/專輯乙/same.ogg",
      format: "ogg" as const,
      selected: true,
      hasCoverArt: false,
      fields: [
        {
          key: "title",
          label: "標題",
          container: "vorbis-comment" as const,
          values: ["同名"],
          convertedValues: ["同名"],
          selected: true,
        },
      ],
    };
    const warning = {
      path: "/music/現場/broken.mp3",
      format: "mp3" as const,
      selected: false,
      hasCoverArt: false,
      warning: "無法讀取標籤",
      fields: [],
    };
    const batch = Array.from({ length: 247 }, (_, index) => {
      const same = index % 17 === 0;
      return {
        path: `${index % 2 === 0 ? "/music/合輯" : "/music/現場"}/track-${String(index).padStart(3, "0")}.mp3`,
        format: "mp3" as const,
        selected: true,
        hasCoverArt: false,
        fields: [
          {
            key: "title",
            label: "標題",
            container: "id3v2" as const,
            values: [`歌${index}`],
            convertedValues: [same ? `歌${index}` : `歌${index}繁`],
            selected: true,
          },
          {
            key: "album",
            label: "專輯",
            container: "id3v2" as const,
            values: ["晨雾"],
            convertedValues: [same ? "晨雾" : "晨霧"],
            selected: true,
          },
        ],
      };
    });
    const files = [song, unchanged, warning, ...batch];
    const errors = files.filter((file) => "warning" in file && file.warning).length;
    const changed = files.filter(
      (file) =>
        !("warning" in file && file.warning) &&
        file.fields.some(
          (field) =>
            field.convertedValues && field.convertedValues.join("\0") !== field.values.join("\0"),
        ),
    ).length;
    const slimFiles = files.map((file) => ({
      ...file,
      previewed: !("warning" in file && file.warning),
      hasCoverArt: file.hasCoverArt || undefined,
      fields: file.fields.map((field) => {
        if (
          !field.convertedValues ||
          field.convertedValues.join("\0") === field.values.join("\0")
        ) {
          const { convertedValues: _unused, ...rest } = field;
          return rest;
        }
        return field;
      }),
    }));
    return {
      planId: "audio-plan-1",
      createdAt: "2026-08-15T00:00:00.000Z",
      summary: {
        total: files.length,
        changed,
        unchanged: files.length - changed - errors,
        errors,
        selectedFields: files.reduce(
          (count, file) =>
            count + (file.selected ? file.fields.filter((field) => field.selected).length : 0),
          0,
        ),
      },
      files: slimFiles,
    };
  }
  if (operation === "audio.rescanFile") {
    if (e2e().holdAudioRescan) {
      await new Promise<void>((resolve) => {
        e2e().releaseAudioRescan = resolve;
      });
    }
    const path = String(payload.path ?? "/music/現場/broken.mp3");
    return {
      path,
      format: "mp3",
      selected: true,
      previewed: true,
      fields: [
        {
          key: "title",
          label: "標題",
          container: "id3v2",
          values: ["夜曲"],
          convertedValues: ["夜曲繁"],
          selected: true,
        },
        {
          key: "album",
          label: "專輯",
          container: "id3v2",
          values: ["北窗"],
          selected: true,
        },
      ],
    };
  }
  if (operation === "audio.apply") {
    const paths = Array.isArray(payload.paths) ? payload.paths.map((item) => String(item)) : [];
    const selectedFields =
      payload.selectedFields && typeof payload.selectedFields === "object"
        ? (payload.selectedFields as Record<string, string[]>)
        : {};
    e2e().lastAudioApplyPaths = paths;
    e2e().lastAudioApply = { paths, selectedFields };
    return { succeeded: paths.slice(0, 1), skipped: [], failed: [] };
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
  return "2.0.0-beta10";
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

export async function open(
  options: { directory?: boolean } = {},
): Promise<string | string[] | null> {
  if (options.directory && e2e().selectedDirectory) return e2e().selectedDirectory;
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

export class Update {
  currentVersion: string;
  version: string;
  body?: string;
  constructor(metadata: { currentVersion: string; version: string; body?: string }) {
    this.currentVersion = metadata.currentVersion;
    this.version = metadata.version;
    this.body = metadata.body;
  }
  async close(): Promise<void> {}
  async downloadAndInstall(): Promise<void> {}
}

export async function check() {
  if (e2e().update !== "install") return null;
  return new Update({
    currentVersion: "2.0.0-beta10",
    version: "2.1.0",
    body: "測試更新",
  });
}

export async function register(): Promise<void> {}

export async function unregisterAll(): Promise<void> {}

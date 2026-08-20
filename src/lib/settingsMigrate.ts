import type { EngineKind, SettingsV2 } from "@shared/contracts";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { DEFAULT_FILE_TYPE_FILTER } from "./fileFilters";

type LegacySettings = Record<string, unknown>;

export async function migrateSettingsFromPath(inputPath: string): Promise<SettingsV2> {
  const raw = await readFile(resolve(inputPath), "utf8");
  return migrateSettings(JSON.parse(raw.replace(/^\uFEFF/, "")));
}

export function defaultSettings(): SettingsV2 {
  return {
    version: 2,
    engine: "segmented",
    direction: "s2t",
    vocabularyCorrection: true,
    promptAfterConversion: true,
    autoBackupBeforeConversion: true,
    recognizeEncoding: true,
    previewMaxKb: 6,
    floatingBall: { enabled: true, x: -1, y: -1 },
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
      typeFilter: DEFAULT_FILE_TYPE_FILTER,
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
    checkVersionOnStart: true,
    checkPreReleaseUpdates: false,
    skippedUpdateVersion: "",
    showMainWindowOnStart: false,
    lastDropAction: {
      kind: "file",
      operation: "content",
      direction: "s2t",
    },
  };
}

export function migrateSettings(input: unknown): SettingsV2 {
  if (isSettingsV2(input)) {
    const defaults = defaultSettings();
    return {
      ...defaults,
      ...input,
      engine: input.engine ?? "segmented",
      // 舊版 2.0／匯入設定若尚無此欄，預設啟用轉換前備份。
      autoBackupBeforeConversion: booleanValue(input.autoBackupBeforeConversion, true),
      floatingBall: { ...defaults.floatingBall, ...input.floatingBall },
      hotkeys: { ...defaults.hotkeys, ...input.hotkeys },
      quickActions: { ...defaults.quickActions, ...input.quickActions },
      files: { ...defaults.files, ...input.files },
      zhconvert: { ...defaults.zhconvert, ...input.zhconvert },
      lastDropAction: { ...defaults.lastDropAction, ...input.lastDropAction },
    };
  }
  const legacy = (input && typeof input === "object" ? input : {}) as LegacySettings;
  const defaults = defaultSettings();
  const hotkey = objectValue(legacy.HotKey);
  const fileConvert = objectValue(legacy.FileConvert);
  const quickStart = objectValue(legacy.QuickStart);
  const fanhuaji = objectValue(legacy.Fanhuaji_Setting);
  const quickSettings = Array.from({ length: 4 }, (_, index) => {
    const feature = objectValue(hotkey[`Feature${index + 1}`]);
    const modifier = stringValue(feature.Modift);
    const key = stringValue(feature.Key);
    return {
      enabled: booleanValue(feature.Enable, false),
      accelerator: [normalizeModifier(modifier), key === "None" ? "" : key]
        .filter(Boolean)
        .join("+"),
      action: stringValue(feature.Action) || `a${index + 1}`,
    };
  });

  return {
    ...defaults,
    engine: engineValue(legacy.Engine),
    vocabularyCorrection: booleanValue(legacy["Vocabulary correction"], true),
    promptAfterConversion: booleanValue(legacy.Prompt, true),
    autoBackupBeforeConversion: true,
    recognizeEncoding: booleanValue(legacy.RecognitionEncoding, true),
    previewMaxKb: numberValue(legacy.MaxLengthPreview, 6),
    floatingBall: {
      enabled: booleanValue(legacy.AssistiveTouch, true),
      x: numberValue(legacy.PositionX, -1),
      y: numberValue(legacy.PositionY, -1),
    },
    hotkeys: {
      autoCopy: booleanValue(hotkey.AutoCopy, true),
      autoPaste: booleanValue(hotkey.AutoPaste, true),
      shortcuts: quickSettings,
    },
    quickActions: {
      leftClickCtrl: stringValue(quickStart.LeftClick_Ctrl) || "0",
      leftClickAlt: stringValue(quickStart.LeftClick_Alt) || "0",
      leftClickShift: stringValue(quickStart.LeftClick_Shift) || "0",
      rightClickCtrl: stringValue(quickStart.RightClick_Ctrl) || "0",
      rightClickAlt: stringValue(quickStart.RightClick_Alt) || "0",
      rightClickShift: stringValue(quickStart.RightClick_Shift) || "0",
      leftDropCtrl: stringValue(quickStart.LeftDrop_Ctrl) || "0",
      leftDropAlt: stringValue(quickStart.LeftDrop_Alt) || "0",
      leftDropShift: stringValue(quickStart.LeftDrop_Shift) || "0",
      rightDropCtrl: stringValue(quickStart.RightDrop_Ctrl) || "0",
      rightDropAlt: stringValue(quickStart.RightDrop_Alt) || "0",
      rightDropShift: stringValue(quickStart.RightDrop_Shift) || "0",
    },
    files: {
      defaultPath: stringValue(fileConvert.DefaultPath) || defaults.files.defaultPath,
      typeFilter: stringValue(fileConvert.TypeFilter) || defaults.files.typeFilter,
      fixCharsetExtensions: (
        stringValue(fileConvert.FixLabel) || defaults.files.fixCharsetExtensions.join("|")
      )
        .split("|")
        .filter(Boolean),
      unicodeAddBom: booleanValue(fileConvert.UnicodeAddBOM, false),
    },
    zhconvert: {
      ...defaults.zhconvert,
      converterS2T: converterValue(fanhuaji.Converter_S_to_T, defaults.zhconvert.converterS2T),
      converterT2S: converterValue(fanhuaji.Converter_T_to_S, defaults.zhconvert.converterT2S),
      cleanUpText: booleanValue(fanhuaji.CleanUpText, false),
      userPreReplace: replacementLines(fanhuaji.UserPreReplace),
      userPostReplace: replacementLines(fanhuaji.UserPostReplace),
      userProtectReplace: protectionLines(fanhuaji.UserProtectReplace),
      ensureNewlineAtEof: booleanValue(fanhuaji.EnsureNewlineAtEof, false),
      translateTabsToSpaces: numberValue(fanhuaji.TranslateTabsToSpaces, -1),
      trimTrailingWhiteSpaces: booleanValue(fanhuaji.TrimTrailingWhiteSpaces, false),
      unifyLeadingHyphen: booleanValue(fanhuaji.UnifyLeadingHyphen, false),
      jpTextConversionStrategy: strategyValue(fanhuaji.JpTextConversionStrategy),
      jpStyleConversionStrategy: strategyValue(fanhuaji.JpStyleConversionStrategy),
      modules: moduleValues(fanhuaji.Modules),
      ignoreTextStyles: stringValue(fanhuaji.IgnoreTextStyles),
      jpTextStyles: stringValue(fanhuaji.JpTextStyles),
    },
    checkVersionOnStart: booleanValue(legacy.CheckVersion, true),
  };
}

function isSettingsV2(value: unknown): value is SettingsV2 {
  return Boolean(
    value && typeof value === "object" && (value as { version?: unknown }).version === 2,
  );
}

function objectValue(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" ? (value as Record<string, unknown>) : {};
}

function stringValue(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function booleanValue(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

function numberValue(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function replacementLines(value: unknown): string {
  if (!Array.isArray(value)) return "";
  return value
    .flatMap((item) => {
      const entry = objectValue(item);
      const key = stringValue(entry.Key);
      return key ? [`${key}=${stringValue(entry.Value)}`] : [];
    })
    .join("\n");
}

function protectionLines(value: unknown): string {
  if (!Array.isArray(value)) return "";
  return value
    .flatMap((item) => {
      const entry = objectValue(item);
      const key = stringValue(entry.Key) || stringValue(entry.Value);
      return key ? [key] : [];
    })
    .join("\n");
}

function strategyValue(value: unknown): "none" | "protect" | "protectOnlySameOrigin" | "fix" {
  if (
    typeof value === "string" &&
    ["none", "protect", "protectOnlySameOrigin", "fix"].includes(value)
  ) {
    return value as "none" | "protect" | "protectOnlySameOrigin" | "fix";
  }
  if (typeof value === "number")
    return (
      (["protectOnlySameOrigin", "none", "protect", "fix"] as const)[value] ??
      "protectOnlySameOrigin"
    );
  return "protectOnlySameOrigin";
}

function normalizeModifier(value: string): string {
  if (!value || value === "None") return "";
  return value
    .split(/[,+]/u)
    .map((part) => part.trim())
    .filter((part) => part && part !== "None")
    .join("+");
}

function engineValue(value: unknown): EngineKind {
  if (value === 1 || value === "Fanhuaji" || value === "zhconvert") return "zhconvert";
  if (value === "legacy") return "legacy";
  return "segmented";
}

function converterValue(value: unknown, fallback: string): string {
  if (typeof value === "string" && value) return value;
  if (typeof value === "number") {
    return (
      (
        [
          "Simplified",
          "Traditional",
          "China",
          "Hongkong",
          "Taiwan",
          "Pinyin",
          "Bopomofo",
          "Mars",
          "WikiSimplified",
          "WikiTraditional",
        ] as const
      )[value] ?? fallback
    );
  }
  return fallback;
}

function moduleValues(value: unknown): Record<string, -1 | 0 | 1> {
  if (!Array.isArray(value)) return {};
  return Object.fromEntries(
    value.flatMap((item) => {
      const module = objectValue(item);
      const name = stringValue(module.ModuleName);
      if (!name) return [];
      const enabled = module.Enable;
      return [[name, enabled === true ? 1 : enabled === false ? 0 : -1] as const];
    }),
  );
}

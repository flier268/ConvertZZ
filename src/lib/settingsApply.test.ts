import { describe, expect, it, vi } from "vitest";
import type { SettingsV2 } from "@shared/contracts";
import { defaultSettings, migrateSettings } from "./settingsMigrate";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/dpi", () => ({ LogicalPosition: class {} }));
vi.mock("@tauri-apps/api/window", () => ({ getAllWindows: async () => [] }));
vi.mock("@tauri-apps/plugin-global-shortcut", () => ({
  register: vi.fn(),
  unregisterAll: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-store", () => ({ load: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ confirm: vi.fn() }));
vi.mock("./coreClient", () => ({ core: { request: vi.fn() } }));
vi.mock("./legacyActions", () => ({ executeLegacyAction: vi.fn() }));
vi.mock("element-plus", () => ({ ElMessage: { error: vi.fn() } }));

const { fileConversionDefaults, importedSettingsEffects, importFailureMessage } =
  await import("./settingsApply");

function withOverrides(overrides: (value: SettingsV2) => void): SettingsV2 {
  const settings = defaultSettings();
  overrides(settings);
  return settings;
}

describe("已匯入 SettingsV2 欄位會影響實際行為", () => {
  it("舊版欄位匯入後會改變對應行為", () => {
    const imported = migrateSettings({
      Engine: 1,
      "Vocabulary correction": false,
      Prompt: false,
      RecognitionEncoding: false,
      MaxLengthPreview: 12,
      AssistiveTouch: false,
      PositionX: 88,
      PositionY: 44,
      CheckVersion: false,
      HotKey: {
        AutoCopy: false,
        AutoPaste: false,
        Feature1: { Enable: true, Modift: "Control", Key: "F9", Action: "a3" },
      },
      QuickStart: { LeftClick_Ctrl: "a4", RightDrop_Shift: "ze2" },
      FileConvert: {
        DefaultPath: "/tmp/books",
        TypeFilter: "<文字|*.txt;*.log>",
        FixLabel: ".html|.php",
        UnicodeAddBOM: true,
      },
      Fanhuaji_Setting: {
        Converter_S_to_T: "Hongkong",
        Converter_T_to_S: "China",
        CleanUpText: true,
        UserPreReplace: [{ Key: "甲", Value: "乙" }],
        UserPostReplace: [{ Key: "丙", Value: "丁" }],
        UserProtectReplace: [{ Key: "戊" }],
        EnsureNewlineAtEof: true,
        TranslateTabsToSpaces: 4,
        TrimTrailingWhiteSpaces: true,
        UnifyLeadingHyphen: true,
        JpTextConversionStrategy: "fix",
        JpStyleConversionStrategy: "none",
        IgnoreTextStyles: "code",
        JpTextStyles: "jp",
        Modules: [{ ModuleName: "TaiwanPhrase", Enable: true }],
      },
    });
    imported.direction = "t2s";
    imported.dictionaryPath = "/tmp/Dictionary.csv";
    imported.showMainWindowOnStart = true;

    const effects = importedSettingsEffects(imported);
    expect(effects.files).toMatchObject({
      engine: "zhconvert",
      direction: "t2s",
      vocabularyCorrection: false,
      addBom: true,
      inputEncoding: "utf8",
      previewMaxBytes: 12 * 1024,
      fixCharsetExtensions: [".html", ".php"],
      defaultPath: "/tmp/books",
      fileFilters: [
        { name: "支援的檔案", extensions: ["txt", "log"] },
        { name: "文字", extensions: ["txt", "log"] },
      ],
      promptAfterConversion: false,
      dictionaryPath: "/tmp/Dictionary.csv",
    });
    expect(effects.zhconvert).toMatchObject({
      converter: "China",
      modules: { TaiwanPhrase: 1 },
      jpTextConversionStrategy: "fix",
      jpStyleConversionStrategy: "none",
      cleanUpText: true,
      userPreReplace: "甲=乙",
      userPostReplace: "丙=丁",
      userProtectReplace: "戊",
      ensureNewlineAtEof: true,
      translateTabsToSpaces: 4,
      trimTrailingWhiteSpaces: true,
      unifyLeadingHyphen: true,
      ignoreTextStyles: "code",
      jpTextStyles: "jp",
    });
    expect(effects.floatingBall).toEqual({ enabled: false, position: { x: 88, y: 44 } });
    expect(effects.showMainWindowOnStart).toBe(true);
    expect(effects.checkVersionOnStart).toBe(false);
    expect(effects.checkPreReleaseUpdates).toBe(false);
    expect(effects.hotkeys).toEqual({
      autoCopy: false,
      autoPaste: false,
      shortcuts: [{ enabled: true, accelerator: "Control+F9", action: "a3" }],
    });
    expect(effects.quickActions.leftClickCtrl).toBe("a4");
    expect(effects.quickActions.rightDropShift).toBe("ze2");
  });

  it("逐欄變更會改變對應行為", () => {
    const baseline = importedSettingsEffects(defaultSettings());
    const cases: Array<{ name: string; change: (value: SettingsV2) => void; path: string[] }> = [
      { name: "engine", change: (value) => (value.engine = "legacy"), path: ["files", "engine"] },
      {
        name: "direction",
        change: (value) => (value.direction = "t2s"),
        path: ["files", "direction"],
      },
      {
        name: "vocabularyCorrection",
        change: (value) => (value.vocabularyCorrection = false),
        path: ["files", "vocabularyCorrection"],
      },
      {
        name: "dictionaryPath",
        change: (value) => (value.dictionaryPath = "/tmp/d.csv"),
        path: ["files", "dictionaryPath"],
      },
      {
        name: "promptAfterConversion",
        change: (value) => (value.promptAfterConversion = false),
        path: ["files", "promptAfterConversion"],
      },
      {
        name: "recognizeEncoding",
        change: (value) => (value.recognizeEncoding = false),
        path: ["files", "inputEncoding"],
      },
      {
        name: "previewMaxKb",
        change: (value) => (value.previewMaxKb = 20),
        path: ["files", "previewMaxBytes"],
      },
      {
        name: "floatingBall.enabled",
        change: (value) => (value.floatingBall.enabled = false),
        path: ["floatingBall", "enabled"],
      },
      {
        name: "floatingBall.x/y",
        change: (value) => {
          value.floatingBall.x = 10;
          value.floatingBall.y = 20;
        },
        path: ["floatingBall", "position"],
      },
      {
        name: "hotkeys.autoCopy",
        change: (value) => (value.hotkeys.autoCopy = false),
        path: ["hotkeys", "autoCopy"],
      },
      {
        name: "hotkeys.autoPaste",
        change: (value) => (value.hotkeys.autoPaste = false),
        path: ["hotkeys", "autoPaste"],
      },
      {
        name: "hotkeys.shortcuts",
        change: (value) => {
          value.hotkeys.shortcuts[0] = { enabled: true, accelerator: "Alt+F8", action: "a1" };
        },
        path: ["hotkeys", "shortcuts"],
      },
      {
        name: "quickActions.leftClickCtrl",
        change: (value) => (value.quickActions.leftClickCtrl = "a1"),
        path: ["quickActions", "leftClickCtrl"],
      },
      {
        name: "files.defaultPath",
        change: (value) => (value.files.defaultPath = "/data"),
        path: ["files", "defaultPath"],
      },
      {
        name: "files.typeFilter",
        change: (value) => (value.files.typeFilter = "<日誌|*.log>"),
        path: ["files", "fileFilters"],
      },
      {
        name: "files.fixCharsetExtensions",
        change: (value) => (value.files.fixCharsetExtensions = [".asp"]),
        path: ["files", "fixCharsetExtensions"],
      },
      {
        name: "files.unicodeAddBom",
        change: (value) => (value.files.unicodeAddBom = true),
        path: ["files", "addBom"],
      },
      {
        name: "zhconvert.converterS2T",
        change: (value) => (value.zhconvert.converterS2T = "Hongkong"),
        path: ["zhconvert", "converter"],
      },
      {
        name: "zhconvert.modules",
        change: (value) => (value.zhconvert.modules = { ProperNoun: 0 }),
        path: ["zhconvert", "modules"],
      },
      {
        name: "zhconvert.jpTextConversionStrategy",
        change: (value) => (value.zhconvert.jpTextConversionStrategy = "fix"),
        path: ["zhconvert", "jpTextConversionStrategy"],
      },
      {
        name: "zhconvert.cleanUpText",
        change: (value) => (value.zhconvert.cleanUpText = true),
        path: ["zhconvert", "cleanUpText"],
      },
      {
        name: "zhconvert.userPreReplace",
        change: (value) => (value.zhconvert.userPreReplace = "a=b"),
        path: ["zhconvert", "userPreReplace"],
      },
      {
        name: "checkVersionOnStart",
        change: (value) => (value.checkVersionOnStart = false),
        path: ["checkVersionOnStart"],
      },
      {
        name: "checkPreReleaseUpdates",
        change: (value) => (value.checkPreReleaseUpdates = true),
        path: ["checkPreReleaseUpdates"],
      },
      {
        name: "showMainWindowOnStart",
        change: (value) => (value.showMainWindowOnStart = true),
        path: ["showMainWindowOnStart"],
      },
    ];

    for (const sample of cases) {
      const changed = importedSettingsEffects(withOverrides(sample.change));
      const read = (value: unknown, path: string[]) =>
        path.reduce<unknown>((current, key) => (current as Record<string, unknown>)[key], value);
      expect(read(changed, sample.path), sample.name).not.toEqual(read(baseline, sample.path));
    }
  });

  it("匯入失敗訊息可供畫面顯示", () => {
    expect(importFailureMessage(new Error("ENOENT"))).toBe("匯入失敗。目前設定未變更。ENOENT");
  });

  it("fileConversionDefaults 會套用預覽上限與 BOM", () => {
    const defaults = fileConversionDefaults(
      withOverrides((value) => {
        value.previewMaxKb = 8;
        value.files.unicodeAddBom = true;
        value.recognizeEncoding = false;
      }),
    );
    expect(defaults.previewMaxBytes).toBe(8192);
    expect(defaults.addBom).toBe(true);
    expect(defaults.inputEncoding).toBe("utf8");
  });
});

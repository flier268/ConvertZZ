import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SettingsV2 } from "@shared/contracts";

const invoke = vi.fn();
const emit = vi.fn();
const readText = vi.fn();
const writeText = vi.fn();
const openUrl = vi.fn();
const convertText = vi.fn();
const showAppToast = vi.fn();
const getAllWindows = vi.fn();
const coreRequest = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  emit: (...args: unknown[]) => emit(...args),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getAllWindows: (...args: unknown[]) => getAllWindows(...args),
}));
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  readText: (...args: unknown[]) => readText(...args),
  writeText: (...args: unknown[]) => writeText(...args),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: (...args: unknown[]) => openUrl(...args),
}));
vi.mock("./actions", () => ({
  convertText: (...args: unknown[]) => convertText(...args),
}));
vi.mock("./toast", () => ({
  showAppToast: (...args: unknown[]) => showAppToast(...args),
}));
vi.mock("./coreClient", () => ({
  core: { request: (...args: unknown[]) => coreRequest(...args) },
}));
vi.mock("./settings", () => ({
  zhConvertOptions: () => undefined,
}));

const { executeLegacyAction } = await import("./legacyActions");

function settingsFixture(overrides: Partial<SettingsV2> = {}): SettingsV2 {
  return {
    version: 2,
    engine: "segmented",
    direction: "s2t",
    vocabularyCorrection: true,
    promptAfterConversion: false,
    autoBackupBeforeConversion: true,
    recognizeEncoding: true,
    previewMaxKb: 6,
    floatingBall: { enabled: true, x: -1, y: -1 },
    hotkeys: {
      autoCopy: true,
      autoPaste: true,
      shortcuts: [],
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
      typeFilter: "",
      fixCharsetExtensions: [],
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
    ...overrides,
  };
}

describe("executeLegacyAction", () => {
  beforeEach(() => {
    invoke.mockReset();
    emit.mockReset();
    readText.mockReset();
    writeText.mockReset();
    openUrl.mockReset();
    convertText.mockReset();
    showAppToast.mockReset();
    getAllWindows.mockReset();
    coreRequest.mockReset();
    readText.mockResolvedValue("里面");
    writeText.mockResolvedValue(undefined);
    convertText.mockResolvedValue({ text: "裡面", durationMs: 1 });
    coreRequest.mockResolvedValue({ text: "結果" });
  });

  it("G-11 一般剪貼簿動作只走 clipboard 外掛讀寫", async () => {
    const result = await executeLegacyAction("a3", settingsFixture());
    expect(readText).toHaveBeenCalledOnce();
    expect(invoke).not.toHaveBeenCalledWith("capture_selection");
    expect(convertText).toHaveBeenCalledWith(
      "里面",
      "s2t",
      "segmented",
      true,
      undefined,
      undefined,
    );
    expect(writeText).toHaveBeenCalledWith("裡面");
    expect(invoke).not.toHaveBeenCalledWith("replace_selection", expect.anything());
    expect(result.text).toBe("裡面");
  });

  it("G-11／I 快捷鍵自動化會走 selection 指令而非剪貼簿外掛", async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === "capture_selection") return "头发";
      return undefined;
    });
    convertText.mockResolvedValue({ text: "頭髮", durationMs: 1 });

    const result = await executeLegacyAction("a3", settingsFixture(), undefined, {
      copy: true,
      paste: true,
    });

    expect(invoke).toHaveBeenCalledWith("capture_selection");
    expect(readText).not.toHaveBeenCalled();
    expect(writeText).toHaveBeenCalledWith("頭髮");
    expect(invoke).toHaveBeenCalledWith("replace_selection", { text: "頭髮" });
    expect(result.text).toBe("頭髮");
  });

  it("G-12 殼層動作會顯示主視窗並導向對應頁面", async () => {
    await executeLegacyAction("b1", settingsFixture());
    expect(invoke).toHaveBeenCalledWith("show_main_window");
    expect(emit).toHaveBeenCalledWith("app://navigate", "files");
    expect(convertText).not.toHaveBeenCalled();
  });

  it("G-14 回報問題會開啟 Issues", async () => {
    await executeLegacyAction("report", settingsFixture());
    expect(openUrl).toHaveBeenCalledWith("https://github.com/flier268/ConvertZZ/issues");
  });
});

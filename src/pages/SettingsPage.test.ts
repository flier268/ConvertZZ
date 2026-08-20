/** @vitest-environment jsdom */
import { flushPromises, mount } from "@vue/test-utils";
import ElementPlus from "element-plus";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PlatformCapabilities, SettingsV2 } from "@shared/contracts";

const invoke = vi.fn();
const getLoadedSettings = vi.fn();
const loadSettings = vi.fn();
const importLegacySettings = vi.fn();
const openFile = vi.fn();
const applyDesktopSettings = vi.fn();
const onSettingsReplaced = vi.fn((_listener?: () => void) => () => undefined);

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => openFile(...args),
}));
vi.mock("../lib/coreClient", () => ({ core: { request: vi.fn() } }));
vi.mock("../lib/desktop", () => ({
  applyDesktopSettings: (...args: unknown[]) => applyDesktopSettings(...args),
}));
vi.mock("../lib/settings", () => ({
  getLoadedSettings: () => getLoadedSettings(),
  loadSettings: (...args: unknown[]) => loadSettings(...args),
  saveSettings: vi.fn(),
  importLegacySettings: (...args: unknown[]) => importLegacySettings(...args),
  onSettingsReplaced: (listener: () => void) => onSettingsReplaced(listener),
}));

import SettingsPage from "./SettingsPage.vue";

function settingsFixture(): SettingsV2 {
  return {
    version: 2,
    engine: "segmented",
    direction: "s2t",
    vocabularyCorrection: true,
    promptAfterConversion: true,
    autoBackupBeforeConversion: true,
    recognizeEncoding: true,
    previewMaxKb: 6,
    dictionaryPath: "",
    floatingBall: { enabled: true, x: -1, y: -1 },
    hotkeys: {
      autoCopy: true,
      autoPaste: true,
      shortcuts: [{ enabled: false, accelerator: "", action: "a1" }],
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
      typeFilter: "<文字|*.txt>",
      fixCharsetExtensions: [".html"],
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
  };
}

const linuxCapabilities: PlatformCapabilities = {
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
};

async function mountPage() {
  const wrapper = mount(SettingsPage, {
    global: { plugins: [ElementPlus] },
  });
  await flushPromises();
  return wrapper;
}

describe("設定分頁", () => {
  beforeEach(() => {
    invoke.mockReset();
    getLoadedSettings.mockReset();
    loadSettings.mockReset();
    importLegacySettings.mockReset();
    openFile.mockReset();
    applyDesktopSettings.mockReset();
    onSettingsReplaced.mockReset();
    onSettingsReplaced.mockReturnValue(() => undefined);
    getLoadedSettings.mockReturnValue(settingsFixture());
    loadSettings.mockResolvedValue(settingsFixture());
    applyDesktopSettings.mockResolvedValue([]);
    invoke.mockImplementation(async (command: string) => {
      if (command === "platform_capabilities") return linuxCapabilities;
      return null;
    });
  });

  it("第一次只掛一般分頁，切換後才出現快捷鍵與 ZhConvert", async () => {
    const wrapper = await mountPage();
    expect(wrapper.text()).toContain("預設引擎");
    expect(wrapper.text()).toContain("啟動時顯示主視窗");
    expect(wrapper.text()).not.toContain("點選快捷鍵欄位後按下組合鍵");
    expect(wrapper.text()).not.toContain("顯示浮動球");
    expect(wrapper.text()).not.toContain("繁化姬的署名與商業使用條款");

    await wrapper.get('[id^="tab-hotkeys"]').trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("點選快捷鍵欄位後按下組合鍵");

    await wrapper.get('[id^="tab-floating"]').trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("顯示浮動球");

    await wrapper.get('[id^="tab-zhconvert"]').trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("使用此服務時必須遵守繁化姬的署名與商業使用條款。");
    wrapper.unmount();
  });

  it("Windows 整合只出現在一般分頁", async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === "platform_capabilities") {
        return { ...linuxCapabilities, platform: "windows", sendToShortcut: true };
      }
      return null;
    });
    const wrapper = await mountPage();
    expect(wrapper.get("#pane-general").text()).toContain("SendTo 捷徑");
    await wrapper.get('[id^="tab-files"]').trigger("click");
    await flushPromises();
    expect(wrapper.get("#pane-files").text()).toContain("檔案篩選器");
    expect(wrapper.get("#pane-files").text()).not.toContain("SendTo 捷徑");
    wrapper.unmount();
  });

  it("H-06 Linux 一般分頁不顯示 SendTo", async () => {
    const wrapper = await mountPage();
    expect(wrapper.get("#pane-general").text()).not.toContain("SendTo 捷徑");
    expect(wrapper.get("#pane-general").text()).not.toContain("建立 SendTo 捷徑");
    wrapper.unmount();
  });

  it("I-04 無全域快捷鍵時顯示停用提示", async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === "platform_capabilities") {
        return {
          ...linuxCapabilities,
          displayServer: "wayland",
          globalShortcuts: false,
          automaticCopyPaste: false,
          floatingAlwaysOnTop: false,
          limitations: ["本版停用 Wayland 全域快捷鍵。"],
        };
      }
      return null;
    });
    const wrapper = await mountPage();
    await wrapper.get('[id^="tab-hotkeys"]').trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("目前顯示伺服器無法保證全域快捷鍵。");
    wrapper.unmount();
  });

  it("有略過版本時可從一般分頁清除", async () => {
    const settings = settingsFixture();
    settings.skippedUpdateVersion = "2.1.0";
    getLoadedSettings.mockReturnValue(settings);
    loadSettings.mockResolvedValue(settings);
    const wrapper = await mountPage();
    expect(wrapper.text()).toContain("已略過 2.1.0");
    await wrapper.get(".settings-note button").trigger("click");
    expect(settings.skippedUpdateVersion).toBe("");
    wrapper.unmount();
  });

  it("可從設定頁匯入 ConvertZZ.json 並套用桌面設定", async () => {
    const imported = settingsFixture();
    imported.engine = "legacy";
    imported.previewMaxKb = 12;
    openFile.mockResolvedValue("/tmp/ConvertZZ.json");
    importLegacySettings.mockResolvedValue(imported);
    const wrapper = await mountPage();
    expect(wrapper.text()).toContain("匯入 ConvertZZ.json");
    await wrapper.get(".header-actions button").trigger("click");
    await flushPromises();
    expect(openFile).toHaveBeenCalledWith(
      expect.objectContaining({
        filters: [expect.objectContaining({ extensions: ["json"] })],
      }),
    );
    expect(importLegacySettings).toHaveBeenCalledWith("/tmp/ConvertZZ.json");
    expect(applyDesktopSettings).toHaveBeenCalledWith(imported);
    const previewInput = wrapper.get(".field-with-suffix-row input");
    expect((previewInput.element as HTMLInputElement).value).toBe("12");
    wrapper.unmount();
  });
});

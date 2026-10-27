/** @vitest-environment jsdom */
import { flushPromises, mount } from "@vue/test-utils";
import ElementPlus from "element-plus";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ExtensionStats, SettingsV2 } from "@shared/contracts";
import { withFilterProfiles } from "../lib/filterProfiles";

const openDialog = vi.fn();
const loadSettings = vi.fn();
const saveSettings = vi.fn();
const patchSavedSettings = vi.fn();
const coreRequest = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => undefined) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => openDialog(...args),
  confirm: vi.fn(async () => true),
}));
vi.mock("../lib/coreClient", () => ({
  core: { request: (...args: unknown[]) => coreRequest(...args), cancel: vi.fn() },
  isCancellationError: () => false,
}));
vi.mock("../lib/settings", () => ({
  loadSettings: (...args: unknown[]) => loadSettings(...args),
  saveSettings: (...args: unknown[]) => saveSettings(...args),
  patchSavedSettings: (...args: unknown[]) => patchSavedSettings(...args),
  zhConvertOptions: () => ({}),
}));

import FilesPage from "./FilesPage.vue";

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
    floatingBall: { enabled: false, x: -1, y: -1 },
    hotkeys: { autoCopy: true, autoPaste: true, shortcuts: [] },
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
    files: withFilterProfiles({
      defaultPath: "!",
      typeFilter: "<文字|*.txt>",
      fixCharsetExtensions: [],
      unicodeAddBom: false,
    }),
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
    lastDropAction: { kind: "file", operation: "content", direction: "s2t" },
    audio: {
      backup: true,
      conflictPolicy: "skip",
      recursive: true,
      direction: "s2t",
      engine: "segmented",
      vocabularyCorrection: true,
      id3v1Enabled: true,
      id3v1Direction: "s2t",
      id3v1SourceEncoding: "gbk",
      id3v1OutputEncoding: "big5",
      id3v2Enabled: true,
      id3v2Direction: "s2t",
      id3v2SourceEncoding: "gbk",
      id3v2RepairSourceEncoding: true,
      id3v2Version: 4,
      id3v2Encoding: "utf8",
      tagOperations: [],
      convertFilename: false,
      filenameInputEncoding: "auto",
      filenameOutputEncoding: "utf8",
    },
  };
}

async function mountPage() {
  const wrapper = mount(FilesPage, {
    global: { plugins: [ElementPlus] },
    attachTo: document.body,
  });
  await flushPromises();
  return wrapper;
}

function bodyButton(text: string): HTMLButtonElement | undefined {
  return Array.from(document.body.querySelectorAll("button")).find(
    (button) => button.textContent?.replace(/\s+/gu, "") === text.replace(/\s+/gu, ""),
  ) as HTMLButtonElement | undefined;
}

async function click(text: string): Promise<void> {
  const button = bodyButton(text);
  expect(button, `找不到按鈕：${text}`).toBeTruthy();
  button!.click();
  await flushPromises();
}

const statsFixture: ExtensionStats = {
  extensions: [
    { extension: ".txt", count: 5 },
    { extension: ".png", count: 3 },
  ],
  noExtensionCount: 2,
  totalFiles: 10,
  directoryCount: 4,
  warnings: [],
};

function stubCore(stats: ExtensionStats = statsFixture) {
  coreRequest.mockImplementation(async (operation: string) => {
    if (operation === "files.scanExtensions") return stats;
    return {
      planId: "plan-1",
      createdAt: "2026-08-15T00:00:00.000Z",
      warnings: [],
      ignoredInputs: [],
      items: [],
    };
  });
}

async function setInput(selector: string, value: string): Promise<void> {
  const input = document.body.querySelector(selector) as HTMLInputElement;
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  await flushPromises();
}

describe("檔案頁資料夾篩選流程", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
    openDialog.mockReset();
    saveSettings.mockReset();
    patchSavedSettings.mockReset();
    coreRequest.mockReset();
    loadSettings.mockResolvedValue(settingsFixture());
    saveSettings.mockResolvedValue(undefined);
    patchSavedSettings.mockImplementation(async (patch: (settings: SettingsV2) => void) => {
      const fresh = settingsFixture();
      patch(fresh);
      return fresh;
    });
    stubCore();
  });

  it("完整流程：資料夾 → profile → 預覽統計 → 加入，並以 extensionFilter 建立計畫", async () => {
    const wrapper = await mountPage();
    expect(wrapper.find(".folder-filter").exists()).toBe(false);
    await click("選取來源資料夾");
    expect(openDialog).not.toHaveBeenCalled();
    expect(document.body.textContent).toContain("包含子資料夾");
    await setInput('input[aria-label="來源資料夾路徑"]', "/tmp/samples");
    await click("下一步");
    const rows = document.body.querySelectorAll('[data-testid="profile-row"]');
    expect(Array.from(rows).map((row) => row.textContent)).toEqual(
      expect.arrayContaining([expect.stringContaining("常用文字檔案")]),
    );
    await click("下一步");
    expect(coreRequest).toHaveBeenCalledWith(
      "files.scanExtensions",
      { path: "/tmp/samples", recursive: true },
      expect.anything(),
    );
    const table = document.body.querySelector('table[aria-label="副檔名統計"]')!.textContent!;
    expect(table).toContain(".txt");
    expect(table).toContain("（無副檔名）");
    expect(bodyButton("加入 5 個檔案")).toBeTruthy();
    await click("加入 5 個檔案");

    const planCall = coreRequest.mock.calls.find((call) => call[0] === "files.plan")!;
    const payload = planCall[1] as { extensionFilter?: unknown; allowedExtensions?: unknown };
    expect(payload.extensionFilter).toMatchObject({ mode: "allow", includeNoExtension: false });
    expect(payload.allowedExtensions).toBeUndefined();
    expect(patchSavedSettings).toHaveBeenCalledTimes(1);
    const patched = settingsFixture();
    (patchSavedSettings.mock.calls[0]?.[0] as (settings: SettingsV2) => void)(patched);
    expect(patched.files.lastFilterProfileId).toBe("legacy-1");
    expect(wrapper.find(".folder-filter").exists()).toBe(true);
    wrapper.unmount();
  });

  it("不勾選記住時不寫入 lastFilterProfileId", async () => {
    const wrapper = await mountPage();
    await click("選取來源資料夾");
    await setInput('input[aria-label="來源資料夾路徑"]', "/tmp/samples");
    await click("下一步");
    (document.body.querySelector(".folder-wizard .el-checkbox input") as HTMLInputElement).click();
    await flushPromises();
    await click("下一步");
    await click("加入 5 個檔案");
    expect(patchSavedSettings).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it("快速路徑：記住上次 profile 時選完資料夾直接到預覽，可更換 profile", async () => {
    const settings = settingsFixture();
    settings.files.lastFilterProfileId = "builtin-all";
    loadSettings.mockResolvedValue(settings);
    openDialog.mockResolvedValue("/tmp/samples");
    const wrapper = await mountPage();
    await click("選取來源資料夾");
    expect(openDialog).toHaveBeenCalledTimes(1);
    expect(document.body.textContent).toContain("已套用上次的 profile「所有檔案」");
    expect(bodyButton("加入 10 個檔案")).toBeTruthy();
    await click("更換 profile");
    expect(document.body.querySelectorAll('[data-testid="profile-row"]').length).toBeGreaterThan(1);
    await click("上一步");
    expect(document.body.querySelector('input[aria-label="來源資料夾路徑"]')).toBeTruthy();
    wrapper.unmount();
  });

  it("設定為每次都走完整流程時，即使有上次 profile 也從步驟 1 開始", async () => {
    const settings = settingsFixture();
    settings.files.lastFilterProfileId = "builtin-all";
    settings.files.folderPickBehavior = "alwaysWizard";
    loadSettings.mockResolvedValue(settings);
    const wrapper = await mountPage();
    await click("選取來源資料夾");
    expect(openDialog).not.toHaveBeenCalled();
    expect(document.body.querySelector('input[aria-label="來源資料夾路徑"]')).toBeTruthy();
    wrapper.unmount();
  });

  it("沒有符合的檔案時主按鈕停用並提示換 profile", async () => {
    stubCore({
      ...statsFixture,
      extensions: [{ extension: ".png", count: 3 }],
      noExtensionCount: 0,
    });
    const settings = settingsFixture();
    settings.files.lastFilterProfileId = "builtin-text";
    loadSettings.mockResolvedValue(settings);
    openDialog.mockResolvedValue("/tmp/samples");
    const wrapper = await mountPage();
    await click("選取來源資料夾");
    const button = bodyButton("加入 0 個檔案")!;
    expect(button.disabled).toBe(true);
    expect(document.body.textContent).toContain("沒有符合的檔案");
    wrapper.unmount();
  });

  it("檔案頁顯示目前篩選並可編輯；編輯內建 profile 另存為自訂", async () => {
    const settings = settingsFixture();
    settings.files.lastFilterProfileId = "builtin-text";
    loadSettings.mockResolvedValue(settings);
    openDialog.mockResolvedValue("/tmp/samples");
    const wrapper = await mountPage();
    await click("選取來源資料夾");
    await click("加入 5 個檔案");
    expect(wrapper.get(".folder-filter").text()).toContain("副檔名篩選");
    expect(wrapper.find(".folder-filter textarea").exists()).toBe(false);
    patchSavedSettings.mockClear();
    await click("編輯篩選");
    expect(document.body.textContent).toContain("另存「常用文字檔案」為自訂 profile");
    await setInput('input[aria-label="新增副檔名"]', "*.md;.MD ");
    await click("儲存");
    expect(patchSavedSettings).toHaveBeenCalledTimes(1);
    const patched = settingsFixture();
    (patchSavedSettings.mock.calls[0]?.[0] as (settings: SettingsV2) => void)(patched);
    const added = patched.files.filterProfiles.find(
      (item) => !item.builtin && item.name === "常用文字檔案 副本",
    )!;
    expect(added.extensions).toContain(".md");
    expect(added.extensions.filter((item) => item === ".md")).toHaveLength(1);
    expect(
      patched.files.filterProfiles.find((item) => item.id === "builtin-text")!.extensions,
    ).not.toContain(".md");
    wrapper.unmount();
  });

  it("刪除只更新磁碟上的 files 欄位", async () => {
    const onDisk = settingsFixture();
    onDisk.checkVersionOnStart = false;
    const cached = settingsFixture();
    cached.checkVersionOnStart = true;
    cached.files.lastFilterProfileId = "legacy-1";
    loadSettings.mockResolvedValue(cached);
    let written: SettingsV2 | undefined;
    patchSavedSettings.mockImplementation(async (patch: (settings: SettingsV2) => void) => {
      const fresh = settingsFixture();
      fresh.checkVersionOnStart = onDisk.checkVersionOnStart;
      patch(fresh);
      written = JSON.parse(JSON.stringify(fresh)) as SettingsV2;
      return fresh;
    });
    openDialog.mockResolvedValue("/tmp/samples");
    const wrapper = await mountPage();
    await click("選取來源資料夾");
    await click("加入 5 個檔案");
    await click("刪除");
    expect(written!.checkVersionOnStart).toBe(false);
    expect(written!.files.filterProfiles.map((item) => item.id)).not.toContain("legacy-1");
    expect(saveSettings).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it("只選檔案時不顯示資料夾篩選", async () => {
    openDialog.mockResolvedValue("/tmp/里面.txt");
    const wrapper = await mountPage();
    const fileButton = wrapper.findAll("button").find((button) => button.text() === "選取檔案");
    await fileButton!.trigger("click");
    await flushPromises();
    expect(wrapper.find(".folder-filter").exists()).toBe(false);
    const filters = (openDialog.mock.calls[0]?.[0] as { filters: Array<{ name: string }> }).filters;
    expect(filters.map((item) => item.name)).toEqual(["支援的檔案", "所有檔案", "文字"]);
    wrapper.unmount();
  });
});

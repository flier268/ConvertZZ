import { beforeEach, describe, expect, it, vi } from "vitest";

const storeGet = vi.fn();
const storeSet = vi.fn();
const storeReload = vi.fn();
const storeSave = vi.fn();
const confirm = vi.fn();
const request = vi.fn();
const invoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));
vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({
    get: storeGet,
    set: storeSet,
    reload: storeReload,
    save: storeSave,
  })),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: (...args: unknown[]) => confirm(...args),
}));
vi.mock("./coreClient", () => ({
  core: { request: (...args: unknown[]) => request(...args) },
}));

function mockInstalledMode() {
  invoke.mockImplementation(async (command: string) => {
    if (command === "platform_capabilities") {
      return {
        platform: "windows",
        portable: false,
        automaticUpdates: true,
        limitations: [],
      };
    }
    throw new Error(`未預期的 invoke：${command}`);
  });
}

const { importLegacySettings } = await import("./settings");

describe("匯入舊版設定", () => {
  beforeEach(() => {
    confirm.mockReset();
    request.mockReset();
    storeGet.mockReset();
    storeSet.mockReset();
    storeReload.mockReset();
    storeSave.mockReset();
    invoke.mockReset();
    mockInstalledMode();
  });

  it("只讀取舊設定並另存為 2.0", async () => {
    confirm.mockResolvedValue(true);
    const migrated = { version: 2, engine: "segmented" };
    request.mockResolvedValue(migrated);
    const onReplaced = vi.fn();
    const { onSettingsReplaced } = await import("./settings");
    const stop = onSettingsReplaced(onReplaced);
    await expect(importLegacySettings("/tmp/ConvertZZ.json")).resolves.toMatchObject(migrated);
    expect(request).toHaveBeenNthCalledWith(1, "settings.migrate", { path: "/tmp/ConvertZZ.json" });
    expect(request.mock.calls.every(([operation]) => operation === "settings.migrate")).toBe(true);
    expect(storeSet).toHaveBeenCalledWith("settings", expect.objectContaining(migrated));
    expect(storeSave).toHaveBeenCalled();
    expect(onReplaced).toHaveBeenCalledTimes(1);
    stop();
  });

  it("讀取或轉換失敗時不覆寫目前設定", async () => {
    confirm.mockResolvedValue(true);
    request.mockRejectedValueOnce(new Error("ENOENT"));
    await expect(importLegacySettings("/tmp/ConvertZZ.json")).rejects.toThrow("ENOENT");
    expect(request).toHaveBeenCalledTimes(1);
    expect(request).toHaveBeenCalledWith("settings.migrate", { path: "/tmp/ConvertZZ.json" });
    expect(storeSet).not.toHaveBeenCalled();
  });

  it("使用者取消時不讀取也不覆寫", async () => {
    confirm.mockResolvedValue(false);
    await expect(importLegacySettings("/tmp/ConvertZZ.json")).resolves.toBeUndefined();
    expect(request).not.toHaveBeenCalled();
    expect(storeSet).not.toHaveBeenCalled();
  });
});

describe("設定持久化", () => {
  beforeEach(() => {
    storeGet.mockReset();
    storeSet.mockReset();
    storeReload.mockReset();
    storeSave.mockReset();
    request.mockReset();
    invoke.mockReset();
    mockInstalledMode();
  });

  it("設定檔不存在時仍可載入遷移後的預設值，且不寫回磁碟", async () => {
    vi.resetModules();
    const { loadSettings } = await import("./settings");
    storeGet.mockResolvedValue(undefined);
    storeReload.mockRejectedValue(new Error("系統找不到指定的檔案。 (os error 2)"));
    const migrated = { version: 2, engine: "segmented" };
    request.mockResolvedValue(migrated);
    await expect(loadSettings()).resolves.toMatchObject(migrated);
    expect(request).toHaveBeenCalledWith("settings.migrate", { input: undefined });
    expect(storeSet).not.toHaveBeenCalled();
    expect(storeSave).not.toHaveBeenCalled();
  });

  it("AppData 目錄尚未建立時（Windows os error 3）仍可載入預設值", async () => {
    vi.resetModules();
    const { loadSettings } = await import("./settings");
    storeGet.mockResolvedValue(undefined);
    storeReload.mockRejectedValue(new Error("系统找不到指定的路径。 (os error 3)"));
    const migrated = { version: 2, engine: "segmented" };
    request.mockResolvedValue(migrated);
    await expect(loadSettings()).resolves.toMatchObject(migrated);
    expect(request).toHaveBeenCalledWith("settings.migrate", { input: undefined });
    expect(storeSet).not.toHaveBeenCalled();
    expect(storeSave).not.toHaveBeenCalled();
  });

  it("設定檔讀取失敗且不是缺檔時仍要中止啟動", async () => {
    vi.resetModules();
    const { loadSettings } = await import("./settings");
    storeReload.mockRejectedValue(new Error("Failed to deserialize store. invalid JSON"));
    await expect(loadSettings()).rejects.toThrow("Failed to deserialize store. invalid JSON");
    expect(request).not.toHaveBeenCalled();
  });

  it("載入設定時不把記憶體內容寫回磁碟", async () => {
    vi.resetModules();
    const { loadSettings } = await import("./settings");
    const saved = {
      version: 2,
      hotkeys: { autoCopy: true, autoPaste: true, shortcuts: [] },
    };
    storeGet.mockResolvedValue(saved);
    request.mockResolvedValue(saved);
    await loadSettings();
    expect(storeReload).toHaveBeenCalled();
    expect(storeSet).not.toHaveBeenCalled();
    expect(storeSave).not.toHaveBeenCalled();
  });

  it("已載入的設定可同步讀取", async () => {
    vi.resetModules();
    const { getLoadedSettings, loadSettings } = await import("./settings");
    const saved = {
      version: 2,
      hotkeys: { autoCopy: true, autoPaste: true, shortcuts: [] },
    };
    storeGet.mockResolvedValue(saved);
    request.mockResolvedValue(saved);
    expect(getLoadedSettings()).toBeUndefined();
    await loadSettings();
    expect(getLoadedSettings()).toMatchObject(saved);
  });

  it("修補浮動球位置前會先重讀磁碟上的快捷鍵", async () => {
    vi.resetModules();
    const { patchSavedSettings } = await import("./settings");
    const saved = {
      version: 2,
      floatingBall: { enabled: true, x: -1, y: -1 },
      hotkeys: {
        autoCopy: true,
        autoPaste: true,
        shortcuts: [{ enabled: true, accelerator: "Alt+U", action: "a4" }],
      },
    };
    storeGet.mockResolvedValue(saved);
    request.mockImplementation(
      async (_operation: string, payload: { input?: unknown }) => payload.input,
    );
    const result = await patchSavedSettings((settings) => {
      settings.floatingBall.x = 10;
      settings.floatingBall.y = 20;
    });
    expect(storeReload).toHaveBeenCalled();
    expect(result.hotkeys.shortcuts[0]).toMatchObject({ accelerator: "Alt+U", enabled: true });
    expect(result.floatingBall).toMatchObject({ x: 10, y: 20 });
    expect(storeSet).toHaveBeenCalledWith(
      "settings",
      expect.objectContaining({
        hotkeys: expect.objectContaining({
          shortcuts: [expect.objectContaining({ accelerator: "Alt+U", enabled: true })],
        }),
        floatingBall: expect.objectContaining({ x: 10, y: 20 }),
      }),
    );
    expect(storeSave).toHaveBeenCalled();
  });
});

describe("可攜模式設定", () => {
  beforeEach(() => {
    storeGet.mockReset();
    storeSet.mockReset();
    storeReload.mockReset();
    storeSave.mockReset();
    request.mockReset();
    invoke.mockReset();
  });

  it("讀寫 settings-v2.json 走執行檔旁，不經 plugin-store", async () => {
    vi.resetModules();
    let document: Record<string, unknown> = {
      settings: {
        version: 2,
        hotkeys: { autoCopy: true, autoPaste: true, shortcuts: [] },
      },
    };
    invoke.mockImplementation(async (command: string, args?: { document?: unknown }) => {
      if (command === "platform_capabilities") {
        return { platform: "windows", portable: true, automaticUpdates: false, limitations: [] };
      }
      if (command === "load_portable_settings_store") return document;
      if (command === "save_portable_settings_store") {
        document = args?.document as Record<string, unknown>;
        return undefined;
      }
      throw new Error(`未預期的 invoke：${command}`);
    });
    const { loadSettings, saveSettings } = await import("./settings");
    request.mockImplementation(
      async (_operation: string, payload: { input?: unknown }) => payload.input,
    );
    const settings = await loadSettings();
    expect(settings).toMatchObject({ version: 2 });
    expect(storeGet).not.toHaveBeenCalled();
    settings.floatingBall = { enabled: true, x: 1, y: 2 };
    await saveSettings();
    expect(storeSet).not.toHaveBeenCalled();
    expect(storeSave).not.toHaveBeenCalled();
    expect(invoke).toHaveBeenCalledWith(
      "save_portable_settings_store",
      expect.objectContaining({
        document: expect.objectContaining({
          settings: expect.objectContaining({
            floatingBall: { enabled: true, x: 1, y: 2 },
          }),
        }),
      }),
    );
  });

  it("可攜模式缺檔時仍可載入預設且不寫回", async () => {
    vi.resetModules();
    invoke.mockImplementation(async (command: string) => {
      if (command === "platform_capabilities") {
        return { platform: "windows", portable: true, automaticUpdates: false, limitations: [] };
      }
      if (command === "load_portable_settings_store") return null;
      throw new Error(`未預期的 invoke：${command}`);
    });
    const { loadSettings } = await import("./settings");
    const migrated = { version: 2, engine: "segmented" };
    request.mockResolvedValue(migrated);
    await expect(loadSettings()).resolves.toMatchObject(migrated);
    expect(request).toHaveBeenCalledWith("settings.migrate", { input: undefined });
    expect(invoke).not.toHaveBeenCalledWith("save_portable_settings_store", expect.anything());
  });
});

import { beforeEach, describe, expect, it, vi } from "vitest";

const storeGet = vi.fn();
const storeSet = vi.fn();
const storeReload = vi.fn();
const storeSave = vi.fn();
const confirm = vi.fn();
const request = vi.fn();

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
vi.mock("./sidecar", () => ({
  sidecar: { request: (...args: unknown[]) => request(...args) },
}));

const { importLegacySettings } = await import("./settings");

describe("匯入舊版設定", () => {
  beforeEach(() => {
    confirm.mockReset();
    request.mockReset();
    storeGet.mockReset();
    storeSet.mockReset();
    storeReload.mockReset();
    storeSave.mockReset();
  });

  it("只讀取舊設定並另存為 2.0", async () => {
    confirm.mockResolvedValue(true);
    const migrated = { version: 2, engine: "segmented" };
    request.mockResolvedValue(migrated);
    await expect(importLegacySettings("/tmp/ConvertZZ.json")).resolves.toMatchObject(migrated);
    expect(request).toHaveBeenNthCalledWith(1, "settings.migrate", { path: "/tmp/ConvertZZ.json" });
    expect(request.mock.calls.every(([operation]) => operation === "settings.migrate")).toBe(true);
    expect(storeSet).toHaveBeenCalled();
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

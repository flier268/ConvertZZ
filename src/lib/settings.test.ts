import { beforeEach, describe, expect, it, vi } from "vitest";

const storeGet = vi.fn();
const storeSet = vi.fn();
const confirm = vi.fn();
const request = vi.fn();

vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(async () => ({ get: storeGet, set: storeSet })),
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

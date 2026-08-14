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

  it("備份失敗時不遷移也不覆寫目前設定", async () => {
    confirm.mockResolvedValue(true);
    request.mockRejectedValueOnce(new Error("EACCES"));
    await expect(importLegacySettings("/tmp/ConvertZZ.json")).rejects.toThrow("EACCES");
    expect(request).toHaveBeenCalledTimes(1);
    expect(request).toHaveBeenCalledWith("settings.backup", { path: "/tmp/ConvertZZ.json" });
    expect(storeSet).not.toHaveBeenCalled();
  });
});

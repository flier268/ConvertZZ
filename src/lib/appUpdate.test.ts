import { beforeEach, describe, expect, it, vi } from "vitest";

const getVersion = vi.fn();
const invoke = vi.fn();
const openUrl = vi.fn();
const relaunch = vi.fn();
const check = vi.fn();
const confirm = vi.fn();
const success = vi.fn();
const patchSavedSettings = vi.fn();
const resolveUpdate = vi.fn();

vi.mock("@tauri-apps/api/app", () => ({
  getVersion: (...args: unknown[]) => getVersion(...args),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: (...args: unknown[]) => openUrl(...args),
}));
vi.mock("@tauri-apps/plugin-process", () => ({
  relaunch: (...args: unknown[]) => relaunch(...args),
}));
vi.mock("@tauri-apps/plugin-updater", () => ({
  check: (...args: unknown[]) => check(...args),
}));
vi.mock("element-plus", () => ({
  ElLoading: { service: () => ({ close: vi.fn() }) },
  ElMessage: { success: (...args: unknown[]) => success(...args) },
  ElMessageBox: { confirm: (...args: unknown[]) => confirm(...args) },
}));
vi.mock("./settings", () => ({
  patchSavedSettings: (...args: unknown[]) => patchSavedSettings(...args),
}));
vi.mock("./update", async () => {
  const actual = await vi.importActual<typeof import("./update")>("./update");
  return {
    ...actual,
    resolveUpdate: (...args: unknown[]) => resolveUpdate(...args),
  };
});

const { isDialogCancelled, promptForAppUpdate, rememberSkippedUpdateVersion } =
  await import("./appUpdate");

describe("更新對話框", () => {
  beforeEach(() => {
    getVersion.mockReset().mockResolvedValue("2.0.0");
    invoke.mockReset().mockResolvedValue(undefined);
    openUrl.mockReset();
    relaunch.mockReset();
    check.mockReset();
    confirm.mockReset();
    success.mockReset();
    patchSavedSettings
      .mockReset()
      .mockImplementation(async (patch: (settings: { skippedUpdateVersion: string }) => void) => {
        patch({ skippedUpdateVersion: "" });
      });
    resolveUpdate.mockReset();
  });

  it("辨識使用者取消", () => {
    expect(isDialogCancelled("cancel")).toBe(true);
    expect(isDialogCancelled("close")).toBe(true);
    expect(isDialogCancelled(new Error("網路失敗"))).toBe(false);
  });

  it("把略過版本寫入設定", async () => {
    const settings = { skippedUpdateVersion: "" };
    patchSavedSettings.mockImplementation(async (patch: (value: typeof settings) => void) => {
      patch(settings);
    });
    await rememberSkippedUpdateVersion("2.1.0");
    expect(settings.skippedUpdateVersion).toBe("2.1.0");
  });

  it("啟動檢查遇到較新版本時仍會詢問", async () => {
    resolveUpdate.mockResolvedValue({
      kind: "install",
      currentVersion: "2.0.0",
      latestVersion: "2.2.0",
    });
    confirm.mockRejectedValue("close");
    await promptForAppUpdate({ silentWhenCurrent: true, skippedVersion: "2.1.0" });
    expect(confirm).toHaveBeenCalledOnce();
  });

  it("啟動檢查略過已記錄版本時不跳出對話框", async () => {
    resolveUpdate.mockResolvedValue({
      kind: "install",
      currentVersion: "2.0.0",
      latestVersion: "2.1.0",
    });
    await promptForAppUpdate({ silentWhenCurrent: true, skippedVersion: "2.1.0" });
    expect(confirm).not.toHaveBeenCalled();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("手動檢查仍會詢問已略過的版本", async () => {
    resolveUpdate.mockResolvedValue({
      kind: "open",
      currentVersion: "2.0.0",
      latestVersion: "2.1.0",
      url: "https://github.com/flier268/ConvertZZ/releases/tag/v2.1.0",
    });
    confirm.mockResolvedValue(undefined);
    await promptForAppUpdate({ skippedVersion: "2.1.0" });
    expect(confirm).toHaveBeenCalledOnce();
    expect(openUrl).toHaveBeenCalledOnce();
  });

  it("勾選不再詢問後取消會記住該版本", async () => {
    resolveUpdate.mockResolvedValue({
      kind: "install",
      currentVersion: "2.0.0",
      latestVersion: "2.1.0",
    });
    confirm.mockImplementation(async (message: { children?: unknown[] }) => {
      const label = Array.isArray(message.children) ? message.children[1] : undefined;
      const input =
        label && typeof label === "object" && "children" in label
          ? (label as { children?: unknown[] }).children?.[0]
          : undefined;
      const onChange =
        input && typeof input === "object" && "props" in input
          ? (input as { props?: { onChange?: (event: Event) => void } }).props?.onChange
          : undefined;
      onChange?.({ target: { checked: true } } as unknown as Event);
      throw "cancel";
    });
    await promptForAppUpdate();
    expect(patchSavedSettings).toHaveBeenCalledOnce();
    expect(success).toHaveBeenCalledWith("已略過 2.1.0。啟動時不會再詢問此版本。");
  });

  it("未勾選不再詢問時取消不會記住版本", async () => {
    resolveUpdate.mockResolvedValue({
      kind: "open",
      currentVersion: "2.0.0",
      latestVersion: "2.1.0",
      url: "https://github.com/flier268/ConvertZZ/releases/tag/v2.1.0",
    });
    confirm.mockRejectedValue("close");
    await promptForAppUpdate();
    expect(patchSavedSettings).not.toHaveBeenCalled();
    expect(openUrl).not.toHaveBeenCalled();
  });
});

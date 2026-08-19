import { describe, expect, it, vi } from "vitest";
import type { SettingsV2 } from "@shared/contracts";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/dpi", () => ({ LogicalPosition: class {} }));
vi.mock("@tauri-apps/api/window", () => ({ getAllWindows: vi.fn(async () => []) }));
vi.mock("@tauri-apps/plugin-global-shortcut", () => ({
  register: vi.fn(),
  unregisterAll: vi.fn(),
}));
vi.mock("./legacyActions", () => ({ executeLegacyAction: vi.fn() }));
vi.mock("./toast", () => ({ showAppToast: vi.fn() }));

const { invoke } = await import("@tauri-apps/api/core");
const { register, unregisterAll } = await import("@tauri-apps/plugin-global-shortcut");
const { executeLegacyAction } = await import("./legacyActions");
const { applyDesktopSettings, floatingBallPosition } = await import("./desktop");

function settingsWithBall(x: number, y: number): SettingsV2 {
  return { floatingBall: { enabled: true, x, y } } as SettingsV2;
}

describe("floatingBallPosition", () => {
  it("uses a saved logical position", () => {
    expect(floatingBallPosition(settingsWithBall(120, 80))).toEqual({ x: 120, y: 80 });
  });

  it("ignores the unset default coordinates", () => {
    expect(floatingBallPosition(settingsWithBall(-1, -1))).toBeUndefined();
  });
});

describe("applyDesktopSettings", () => {
  it("registers enabled shortcuts and runs the action on press", async () => {
    vi.mocked(invoke).mockResolvedValue({ globalShortcuts: true });
    vi.mocked(unregisterAll).mockResolvedValue(undefined);
    const settings = {
      floatingBall: { enabled: false, x: -1, y: -1 },
      hotkeys: {
        autoCopy: true,
        autoPaste: true,
        shortcuts: [
          { enabled: true, accelerator: "Alt+U", action: "a4" },
          { enabled: false, accelerator: "Alt+I", action: "a3" },
        ],
      },
    } as SettingsV2;

    const warnings = await applyDesktopSettings(settings);
    expect(unregisterAll).toHaveBeenCalled();
    expect(register).toHaveBeenCalledTimes(1);
    expect(register).toHaveBeenCalledWith("Alt+U", expect.any(Function));
    expect(warnings).toEqual(["快捷鍵 Alt+I 已設定但未啟用，因此尚未註冊。"]);

    const handler = vi.mocked(register).mock.calls[0]?.[1] as (event: {
      state: "Pressed" | "Released";
    }) => Promise<void>;
    await handler({ state: "Released" });
    expect(executeLegacyAction).not.toHaveBeenCalled();
    await handler({ state: "Pressed" });
    expect(executeLegacyAction).toHaveBeenCalledWith("a4", settings, undefined, {
      copy: true,
      paste: true,
    });
  });

  it("I-04 平台不支援全域快捷鍵時不註冊", async () => {
    vi.mocked(invoke).mockResolvedValue({ globalShortcuts: false });
    vi.mocked(register).mockClear();
    vi.mocked(unregisterAll).mockClear();
    const settings = {
      floatingBall: { enabled: false, x: -1, y: -1 },
      hotkeys: {
        autoCopy: true,
        autoPaste: true,
        shortcuts: [{ enabled: true, accelerator: "Alt+U", action: "a4" }],
      },
    } as SettingsV2;

    const warnings = await applyDesktopSettings(settings);
    expect(warnings).toEqual([]);
    expect(unregisterAll).not.toHaveBeenCalled();
    expect(register).not.toHaveBeenCalled();
  });

  it("treats a missing shortcut plugin as a warning instead of aborting", async () => {
    vi.mocked(invoke).mockResolvedValue({ globalShortcuts: true });
    vi.mocked(unregisterAll).mockRejectedValueOnce(new Error("plugin global-shortcut not found"));
    vi.mocked(register).mockClear();
    const settings = {
      floatingBall: { enabled: false, x: -1, y: -1 },
      hotkeys: {
        autoCopy: true,
        autoPaste: true,
        shortcuts: [{ enabled: true, accelerator: "Alt+U", action: "a4" }],
      },
    } as SettingsV2;

    const warnings = await applyDesktopSettings(settings);
    expect(warnings).toEqual(["無法註冊全域快捷鍵：plugin global-shortcut not found"]);
    expect(register).not.toHaveBeenCalled();
  });

  it("treats floating ball setup failure as a warning instead of aborting", async () => {
    const { getAllWindows } = await import("@tauri-apps/api/window");
    vi.mocked(getAllWindows).mockRejectedValueOnce("");
    vi.mocked(invoke).mockResolvedValue({ globalShortcuts: false });
    const settings = {
      floatingBall: { enabled: true, x: -1, y: -1 },
      hotkeys: {
        autoCopy: true,
        autoPaste: true,
        shortcuts: [] as SettingsV2["hotkeys"]["shortcuts"],
      },
    } as SettingsV2;

    const warnings = await applyDesktopSettings(settings);
    expect(warnings).toEqual(["無法套用浮動球：未知錯誤（空字串）"]);
  });
});

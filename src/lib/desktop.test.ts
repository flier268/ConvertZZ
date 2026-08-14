import { describe, expect, it, vi } from "vitest";
import type { SettingsV2 } from "@shared/contracts";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/dpi", () => ({ LogicalPosition: class {} }));
vi.mock("@tauri-apps/api/window", () => ({ getAllWindows: async () => [] }));
vi.mock("@tauri-apps/plugin-global-shortcut", () => ({
  register: vi.fn(),
  unregisterAll: vi.fn(),
}));
vi.mock("./legacyActions", () => ({ executeLegacyAction: vi.fn() }));
vi.mock("element-plus", () => ({ ElMessage: { error: vi.fn() } }));

const { floatingBallPosition } = await import("./desktop");

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

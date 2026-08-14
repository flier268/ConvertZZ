/** @vitest-environment jsdom */
import { mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import FloatingBall from "./FloatingBall.vue";

const executeLegacyAction = vi.fn();
const loadSettings = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => false, invoke: vi.fn() }));
vi.mock("@tauri-apps/api/dpi", () => ({ LogicalPosition: class {} }));
vi.mock("@tauri-apps/api/event", () => ({}));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({}) }));
vi.mock("./lib/appMenuPopup", () => ({ popupAppMenu: vi.fn() }));
vi.mock("./lib/legacyActions", () => ({ executeLegacyAction: (...args: unknown[]) => executeLegacyAction(...args) }));
vi.mock("./lib/settings", () => ({
  loadSettings: (...args: unknown[]) => loadSettings(...args),
  saveSettings: vi.fn(),
}));
vi.mock("element-plus", () => ({ ElMessage: { error: vi.fn(), success: vi.fn() } }));

describe("浮動球左右鍵", () => {
  beforeEach(() => {
    executeLegacyAction.mockReset();
    loadSettings.mockReset();
    loadSettings.mockResolvedValue({
      quickActions: {
        leftClickCtrl: "a3",
        rightClickAlt: "a4",
        leftDropCtrl: "0",
      },
    });
  });

  it("左鍵沒有輔助鍵時不轉換也不開選單", async () => {
    const wrapper = mount(FloatingBall);
    await wrapper.get(".floating-shell").trigger("mousedown", { button: 0 });
    await wrapper.get(".floating-shell").trigger("mouseup", { button: 0 });
    await wrapper.get(".floating-shell").trigger("dblclick");
    expect(wrapper.find(".floating-context-menu").exists()).toBe(false);
    expect(executeLegacyAction).not.toHaveBeenCalled();
  });

  it("右鍵沒有輔助鍵時開啟與舊版相同的選單", async () => {
    const wrapper = mount(FloatingBall);
    await wrapper.get(".floating-shell").trigger("contextmenu", { button: 2 });
    const menu = wrapper.get(".floating-context-menu");
    expect(menu.text()).toContain("Unicode 簡 → Unicode 繁");
    expect(menu.text()).toContain("Unicode 繁 → Unicode 簡");
    expect(menu.text()).toContain("文件/檔名轉換");
    expect(menu.text()).toContain("結束 ConvertZZ");
    expect(executeLegacyAction).not.toHaveBeenCalled();
  });

  it("輔助鍵加左鍵會執行設定的快速動作", async () => {
    const wrapper = mount(FloatingBall);
    await wrapper.get(".floating-shell").trigger("mouseup", { button: 0, ctrlKey: true });
    expect(executeLegacyAction).toHaveBeenCalledWith(
      "a3",
      expect.objectContaining({ quickActions: expect.any(Object) }),
      undefined,
    );
  });
});

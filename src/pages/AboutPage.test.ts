/** @vitest-environment jsdom */
import { flushPromises, mount } from "@vue/test-utils";
import ElementPlus from "element-plus";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PlatformCapabilities } from "@shared/contracts";

const invoke = vi.fn();
const loadSettings = vi.fn();
const promptForAppUpdate = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));
vi.mock("../lib/settings", () => ({
  loadSettings: (...args: unknown[]) => loadSettings(...args),
}));
vi.mock("../lib/appUpdate", () => ({
  isDialogCancelled: () => false,
  promptForAppUpdate: (...args: unknown[]) => promptForAppUpdate(...args),
}));

import AboutPage from "./AboutPage.vue";

const linuxX11: PlatformCapabilities = {
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

async function mountPage(capabilities: PlatformCapabilities = linuxX11) {
  invoke.mockImplementation(async (command: string) => {
    if (command === "platform_capabilities") return capabilities;
    return null;
  });
  const wrapper = mount(AboutPage, {
    global: { plugins: [ElementPlus] },
  });
  await flushPromises();
  return wrapper;
}

describe("關於與差異", () => {
  beforeEach(() => {
    invoke.mockReset();
    loadSettings.mockReset();
    promptForAppUpdate.mockReset();
    loadSettings.mockResolvedValue({ checkPreReleaseUpdates: false });
  });

  it("顯示平台差異表並標示目前環境欄", async () => {
    const wrapper = await mountPage();
    expect(wrapper.text()).toContain("平台差異");
    expect(wrapper.text()).toContain("全域快捷鍵");
    expect(wrapper.text()).toContain("SendTo 捷徑");
    expect(wrapper.text()).toContain("需 AppIndicator；使用選單開啟");
    expect(wrapper.text()).toContain("目前環境：linux / x11");
    expect(wrapper.get("th.is-current").text()).toContain("Linux X11");
    expect(wrapper.get("th.is-current").text()).toContain("目前");
    wrapper.unmount();
  });

  it("I-04／I-05 Wayland 限制出現在目前環境說明與狀態標籤", async () => {
    const wrapper = await mountPage({
      ...linuxX11,
      displayServer: "wayland",
      globalShortcuts: false,
      automaticCopyPaste: false,
      floatingAlwaysOnTop: false,
      limitations: ["本版停用 Wayland 全域快捷鍵。", "浮動球置頂能力取決於合成器。"],
    });
    expect(wrapper.text()).toContain("目前環境：linux / wayland");
    expect(wrapper.text()).toContain("本版停用 Wayland 全域快捷鍵。");
    expect(wrapper.text()).toContain("浮動球置頂能力取決於合成器。");
    expect(wrapper.get("th.is-current").text()).toContain("Linux Wayland");
    expect(wrapper.text()).toContain("本版停用");
    expect(wrapper.text()).toContain("依合成器");
    wrapper.unmount();
  });

  it("轉換差異以條目呈現且保留原說明", async () => {
    const wrapper = await mountPage();
    expect(wrapper.text()).toContain("轉換差異");
    expect(wrapper.text()).toContain("舊版字典的優先權、長詞與保護詞規則保持不變。");
    expect(wrapper.text()).toContain("未命中字元改由 cjk-convert-rs 處理。");
    expect(wrapper.text()).toContain("ZhConvert 是選用的網路服務。");
    wrapper.unmount();
  });
});

/** @vitest-environment jsdom */
import { flushPromises, mount } from "@vue/test-utils";
import ElementPlus from "element-plus";
import { describe, expect, it, vi } from "vitest";

const openDialog = vi.fn();
const coreRequest = vi.fn();

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => openDialog(...args),
  confirm: vi.fn(async () => true),
}));
vi.mock("../lib/coreClient", () => ({
  core: { request: (...args: unknown[]) => coreRequest(...args), cancel: vi.fn() },
  isCancellationError: () => false,
}));
vi.mock("../lib/settings", () => ({
  loadSettings: async () => ({
    direction: "s2t",
    engine: "segmented",
    vocabularyCorrection: true,
    promptAfterConversion: true,
    autoBackupBeforeConversion: true,
  }),
  patchSavedSettings: vi.fn(async () => ({})),
  zhConvertOptions: () => ({}),
}));

import AudioPage from "./AudioPage.vue";

function warningFile(warning: string) {
  return {
    path: "/tmp/broken.mp3",
    format: "mp3",
    selected: true,
    hasCoverArt: false,
    warning,
    fields: [],
  };
}

async function openWarning(warning: string) {
  openDialog.mockResolvedValueOnce("/tmp/broken.mp3");
  coreRequest.mockResolvedValueOnce([warningFile(warning)]);
  const wrapper = mount(AudioPage, {
    global: { plugins: [ElementPlus], stubs: { transition: false } },
  });
  await flushPromises();
  const pick = wrapper.findAll("button").find((button) => button.text().includes("選取音訊檔案"));
  await pick!.trigger("click");
  await flushPromises();
  const errors = wrapper
    .findAll(".el-radio-button")
    .find((button) => button.text().includes("錯誤"));
  const radio = errors!.get("input");
  (radio.element as HTMLInputElement).checked = true;
  await radio.trigger("change");
  await flushPromises();
  await wrapper.find(".audio-name").trigger("click");
  await flushPromises();
  return wrapper;
}

describe("音訊錯誤檔畫面", () => {
  it("錯誤檔核取方塊停用且未勾選，全選與反選也不會勾上", async () => {
    const wrapper = await openWarning("無法讀取標籤");
    const input = wrapper.get(".audio-row input").element as HTMLInputElement;
    expect(input.disabled).toBe(true);
    expect(input.checked).toBe(false);
    const bulk = (name: string) =>
      wrapper.findAll("button").find((button) => button.text() === name)!;
    await bulk("全選").trigger("click");
    await bulk("反選目前篩選").trigger("click");
    expect((wrapper.get(".audio-row input").element as HTMLInputElement).checked).toBe(false);
    expect(wrapper.text()).toContain("已勾選 0 個檔案");
  });

  it("警告標題只出現一次，不同原因才另外顯示", async () => {
    const same = await openWarning("無法讀取標籤");
    expect(same.get(".audio-detail").text().split("無法讀取標籤")).toHaveLength(2);
    expect(same.find(".audio-error-reason").exists()).toBe(false);
    same.unmount();

    const other = await openWarning("檔案已截斷");
    const detail = other.get(".audio-detail").text();
    expect(detail.split("無法讀取標籤")).toHaveLength(2);
    expect(other.get(".audio-error-reason").text()).toBe("檔案已截斷");
  });

  it("選中且有錯誤時，錯誤區段帶上填滿樣式", async () => {
    const wrapper = await openWarning("無法讀取標籤");
    const segment = wrapper.find(".el-radio-button.is-filled");
    expect(segment.exists()).toBe(true);
    expect(segment.classes()).toContain("is-active");
    expect(segment.classes()).toContain("is-alert");
    expect(segment.text()).toContain("錯誤 1");
  });

  it("錯誤篩選裡重掃成功後改到新分桶並維持選取", async () => {
    const wrapper = await openWarning("無法讀取標籤");
    coreRequest.mockResolvedValueOnce([
      {
        path: "/tmp/broken.mp3",
        format: "mp3",
        selected: true,
        previewed: true,
        hasCoverArt: false,
        fields: [
          {
            key: "title",
            label: "標題",
            container: "id3v2",
            values: ["夜曲"],
            convertedValues: ["夜曲繁"],
            selected: true,
          },
        ],
      },
    ]);
    await wrapper.get(".audio-rescan").trigger("click");
    await flushPromises();
    const changed = wrapper
      .findAll(".el-radio-button")
      .find((button) => button.text().includes("有變更"));
    expect(changed!.classes()).toContain("is-active");
    expect(wrapper.get(".audio-detail").text()).toContain("夜曲");
    expect(wrapper.get(".audio-detail").text()).not.toContain("不能勾選");
    expect(wrapper.get(".audio-detail").text()).toContain("/tmp/broken.mp3");
  });

  it("列核取方塊不是上下鍵的攔截目標", async () => {
    openDialog.mockResolvedValueOnce(["/tmp/a.mp3", "/tmp/b.mp3"]);
    coreRequest.mockResolvedValueOnce([
      {
        path: "/tmp/a.mp3",
        format: "mp3",
        selected: true,
        hasCoverArt: false,
        fields: [
          {
            key: "title",
            label: "標題",
            container: "id3v2",
            values: ["甲"],
            selected: true,
          },
        ],
      },
      {
        path: "/tmp/b.mp3",
        format: "mp3",
        selected: true,
        hasCoverArt: false,
        fields: [
          {
            key: "title",
            label: "標題",
            container: "id3v2",
            values: ["乙"],
            selected: true,
          },
        ],
      },
    ]);
    const wrapper = mount(AudioPage, {
      global: { plugins: [ElementPlus], stubs: { transition: false } },
    });
    await flushPromises();
    await wrapper
      .findAll("button")
      .find((button) => button.text().includes("選取音訊檔案"))!
      .trigger("click");
    await flushPromises();
    const box = wrapper.get(".audio-row input");
    expect(box.attributes("tabindex")).toBe("-1");
    await box.trigger("keydown", { key: "ArrowDown" });
    expect(wrapper.get(".audio-row.active .audio-name").text()).toContain("b.mp3");
    const hide = wrapper.get(".audio-hide-unchanged");
    expect(hide.attributes("aria-pressed")).toBe("true");
    expect(hide.element.tagName).toBe("BUTTON");
  });
});

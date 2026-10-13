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
  zhConvertOptions: () => ({}),
}));

import AudioPage from "./AudioPage.vue";

const scanned = {
  path: "/tmp/enc.mp3",
  format: "mp3",
  selected: true,
  hasCoverArt: false,
  fields: [
    { key: "title", label: "標題", container: "id3v2", values: ["里面"], selected: true },
    { key: "comment", label: "備註", container: "id3v2", values: ["Hello"], selected: true },
  ],
};

const planned = {
  ...scanned,
  previewed: true,
  rewriteContainers: ["id3v2"],
  fields: [
    {
      key: "title",
      label: "標題",
      container: "id3v2",
      values: ["里面"],
      convertedValues: ["裡面"],
      selected: true,
    },
    {
      key: "comment",
      label: "備註",
      container: "id3v2",
      values: ["Hello"],
      convertedValues: ["Hello"],
      selected: true,
    },
  ],
};

describe("隱藏不變欄位", () => {
  it("文字相同的欄位在開啟隱藏後不出現，即使容器要重寫編碼", async () => {
    openDialog.mockResolvedValueOnce("/tmp/enc.mp3");
    coreRequest.mockResolvedValueOnce([scanned]);
    coreRequest.mockResolvedValueOnce({
      planId: "p1",
      createdAt: "2026-08-15T00:00:00.000Z",
      files: [planned],
    });
    const wrapper = mount(AudioPage, {
      global: { plugins: [ElementPlus], stubs: { transition: false } },
    });
    await flushPromises();
    await wrapper
      .findAll("button")
      .find((button) => button.text().includes("選取音訊檔案"))!
      .trigger("click");
    await flushPromises();
    await wrapper
      .findAll("button")
      .find((button) => button.text().includes("建立標籤預覽"))!
      .trigger("click");
    await flushPromises();

    const hide = wrapper.get(".audio-hide-unchanged");
    expect(hide.attributes("aria-pressed")).toBe("true");
    expect(wrapper.get(".audio-detail").text()).toContain("標題");
    expect(wrapper.get(".audio-detail").text()).not.toContain("備註");

    await hide.trigger("click");
    expect(hide.attributes("aria-pressed")).toBe("false");
    expect(wrapper.get(".audio-detail").text()).toContain("備註");
    expect(wrapper.get(".audio-detail").text()).toContain("不變");
  });
});

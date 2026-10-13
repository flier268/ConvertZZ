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

const notice = "「ID3v1 標題超過 30 位元組，已截斷為 測測測」";

describe("ID3v1 截斷警告", () => {
  it("詳細面板顯示欄位截斷說明", async () => {
    openDialog.mockResolvedValueOnce("/tmp/long.mp3");
    const scanned = {
      path: "/tmp/long.mp3",
      format: "mp3",
      selected: true,
      fields: [
        {
          key: "title",
          label: "標題",
          container: "id3v1",
          values: ["測".repeat(15)],
          selected: true,
        },
      ],
    };
    const planned = {
      planId: "p-trunc",
      createdAt: "2026-08-15T00:00:00.000Z",
      warnings: [`/tmp/long.mp3：${notice}`],
      files: [
        {
          path: "/tmp/long.mp3",
          format: "mp3",
          selected: true,
          previewed: true,
          fields: [
            {
              key: "title",
              label: "標題",
              container: "id3v1",
              values: ["測".repeat(15)],
              convertedValues: ["測測測"],
              selected: true,
              truncated: true,
              truncation: notice,
            },
          ],
        },
      ],
    };
    coreRequest.mockImplementation(async (operation: string) => {
      if (operation === "audio.scan") return [scanned];
      if (operation === "audio.plan") return planned;
      return undefined;
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
    const alert = wrapper.get(".audio-id3v1-truncation");
    expect(alert.text()).toContain("ID3v1 標題超過 30 位元組，已截斷為 測測測");
    expect(wrapper.get(".audio-detail").text()).not.toContain("不能勾選");
  });
});

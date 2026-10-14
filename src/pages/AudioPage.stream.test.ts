/** @vitest-environment jsdom */
import { flushPromises, mount } from "@vue/test-utils";
import ElementPlus from "element-plus";
import { describe, expect, it, vi } from "vitest";
import type { Progress } from "../lib/coreClient";

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

describe("音訊掃描增量更新", () => {
  it("進度批次先進入清單，完成後仍保留", async () => {
    openDialog.mockResolvedValueOnce("/tmp/a.mp3");
    coreRequest.mockImplementation(
      async (
        operation: string,
        _payload: unknown,
        options?: { onProgress?: (value: Progress) => void },
      ) => {
        if (operation !== "audio.scan") return undefined;
        options?.onProgress?.({
          current: 1,
          total: 2,
          message: "正在掃描",
          batch: {
            phase: "scan",
            files: [
              {
                path: "/tmp/early.mp3",
                format: "mp3",
                selected: true,
                fields: [
                  {
                    key: "title",
                    label: "標題",
                    container: "id3v2",
                    values: ["先到"],
                    selected: true,
                  },
                ],
              },
            ],
          },
        });
        return [
          {
            path: "/tmp/early.mp3",
            format: "mp3",
            selected: true,
            fields: [
              { key: "title", label: "標題", container: "id3v2", values: ["先到"], selected: true },
            ],
          },
          {
            path: "/tmp/late.mp3",
            format: "mp3",
            selected: true,
            fields: [
              { key: "title", label: "標題", container: "id3v2", values: ["後到"], selected: true },
            ],
          },
        ];
      },
    );
    const wrapper = mount(AudioPage, {
      global: { plugins: [ElementPlus], stubs: { transition: false } },
    });
    await flushPromises();
    await wrapper
      .findAll("button")
      .find((button) => button.text().includes("選取音訊檔案"))!
      .trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("early.mp3");
    expect(wrapper.text()).toContain("late.mp3");
    expect(wrapper.findAll("button").some((button) => button.text().includes("建立預覽"))).toBe(
      false,
    );
  });
});

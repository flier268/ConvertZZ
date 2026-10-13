/** @vitest-environment jsdom */
import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import ElementPlus from "element-plus";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AUDIO_PREVIEW_DEBOUNCE_MS } from "../lib/audioReview";

const openDialog = vi.fn();
const confirmDialog = vi.fn<(...args: unknown[]) => Promise<boolean>>(async () => true);
const coreRequest = vi.fn();
const coreCancel = vi.fn(async (..._args: unknown[]) => undefined);

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => openDialog(...args),
  confirm: (...args: unknown[]) => confirmDialog(...args),
}));
vi.mock("../lib/coreClient", () => ({
  core: {
    request: (...args: unknown[]) => coreRequest(...args),
    cancel: (...args: unknown[]) => coreCancel(...args),
  },
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

function file(path: string, selected = true) {
  return {
    path,
    format: "mp3",
    selected,
    hasCoverArt: false,
    fields: [
      {
        key: "title",
        label: "標題",
        container: "id3v2",
        values: ["夜曲"],
        selected: true,
      },
    ],
  };
}

function buttonByText(wrapper: VueWrapper, text: string) {
  return wrapper.findAll("button").find((button) => button.text().includes(text));
}

describe("完整掃描的勾選快照", () => {
  beforeEach(() => {
    openDialog.mockReset();
    confirmDialog.mockReset();
    confirmDialog.mockResolvedValue(true);
    coreRequest.mockReset();
    coreCancel.mockReset();
    vi.useRealTimers();
  });

  it("完整掃描被預覽取代後，下一次完整掃描不會還原更早的勾選", async () => {
    vi.useFakeTimers();
    openDialog.mockResolvedValueOnce("/tmp/a.mp3");
    let releaseSuperseded: (value: unknown) => void = () => undefined;
    let scanRound = 0;
    coreRequest.mockImplementation(
      async (
        operation: string,
        _payload: unknown,
        options?: { onProgress?: (value: unknown) => void },
      ) => {
        if (operation === "audio.scan") {
          scanRound += 1;
          if (scanRound === 1) return [file("/tmp/a.mp3", true)];
          if (scanRound === 2) {
            options?.onProgress?.({
              current: 1,
              total: 1,
              message: "掃描中",
              batch: { phase: "scan", files: [file("/tmp/a.mp3", true)] },
            });
            return new Promise((resolve) => {
              releaseSuperseded = resolve;
            });
          }
          return [file("/tmp/a.mp3", true)];
        }
        if (operation === "audio.plan") throw new Error("預覽失敗");
        return undefined;
      },
    );

    const wrapper = mount(AudioPage, {
      global: { plugins: [ElementPlus], stubs: { transition: false } },
    });
    await flushPromises();
    await buttonByText(wrapper, "選取音訊檔案")!.trigger("click");
    await flushPromises();

    const fileBox = wrapper.get(".audio-row input");
    await fileBox.setValue(false);
    await fileBox.trigger("change");
    expect((wrapper.get(".audio-row input").element as HTMLInputElement).checked).toBe(false);

    await buttonByText(wrapper, "編輯設定")!.trigger("click");
    await flushPromises();
    await buttonByText(wrapper, "重新掃描")!.trigger("click");
    await flushPromises();

    const direction = wrapper
      .findAllComponents({ name: "ElSelect" })
      .find((select) => select.props("modelValue") === "s2t");
    direction!.vm.$emit("update:modelValue", "t2s");
    await flushPromises();
    await vi.advanceTimersByTimeAsync(AUDIO_PREVIEW_DEBOUNCE_MS);
    await flushPromises();

    releaseSuperseded([file("/tmp/a.mp3", false)]);
    await flushPromises();

    await buttonByText(wrapper, "重新掃描")!.trigger("click");
    await flushPromises();

    expect((wrapper.get(".audio-row input").element as HTMLInputElement).checked).toBe(true);
    wrapper.unmount();
  });
});

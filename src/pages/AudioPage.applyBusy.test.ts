/** @vitest-environment jsdom */
import { flushPromises, mount } from "@vue/test-utils";
import ElementPlus from "element-plus";
import { describe, expect, it, vi } from "vitest";

const openDialog = vi.fn();
const confirmDialog = vi.fn(async () => true);
const coreRequest = vi.fn();

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => openDialog(...args),
  confirm: (...args: unknown[]) => confirmDialog(...args),
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

const scannedFile = {
  path: "/tmp/song.mp3",
  format: "mp3",
  selected: true,
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

function plannedFile() {
  return {
    ...scannedFile,
    previewed: true,
    fields: [
      {
        ...scannedFile.fields[0],
        convertedValues: ["夜曲繁"],
      },
    ],
  };
}

describe("確認寫入與忙碌狀態", () => {
  it("確認框回來時若已在忙碌，就中止、不呼叫 audio.apply", async () => {
    let planCalls = 0;
    let releaseCancel: () => void = () => undefined;
    const hungCancel = new Promise<void>((resolve) => {
      releaseCancel = resolve;
    });
    openDialog.mockResolvedValueOnce("/tmp/song.mp3");
    coreRequest.mockImplementation(async (operation: string) => {
      if (operation === "audio.scan") return [scannedFile];
      if (operation === "audio.plan") {
        planCalls += 1;
        if (planCalls === 1) return { planId: "plan-1", files: [plannedFile()] };
        return new Promise(() => undefined);
      }
      // 重跑預覽會先把 busy 升起；清掉計畫要等這次取消回來。
      if (operation === "audio.cancel") return hungCancel.then(() => undefined);
      if (operation === "audio.apply") return { succeeded: ["/tmp/song.mp3"], failed: [] };
      return undefined;
    });
    confirmDialog.mockImplementation(async () => {
      const rebuild = wrapper
        .findAll("button")
        .find((button) => button.text().includes("重新建立預覽"));
      void rebuild!.trigger("click");
      await flushPromises();
      return true;
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

    await wrapper
      .findAll("button")
      .find((button) => button.text().includes("確認寫入"))!
      .trigger("click");
    await flushPromises();

    const operations = coreRequest.mock.calls.map((call) => call[0]);
    expect(operations).not.toContain("audio.apply");
    expect(wrapper.text()).toContain("停止作業");
    releaseCancel();
    await flushPromises();
    wrapper.unmount();
  });
});

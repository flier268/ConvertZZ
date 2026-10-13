/** @vitest-environment jsdom */
import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import ElementPlus from "element-plus";
import { describe, expect, it, vi } from "vitest";

const openDialog = vi.fn();
const confirmDialog = vi.fn<(...args: unknown[]) => Promise<boolean>>(async () => true);
const coreRequest = vi.fn();
const coreCancel = vi.fn(async () => undefined);

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

function file(path: string, selected = true, fieldSelected = true) {
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
        selected: fieldSelected,
      },
    ],
  };
}

function planned(path: string, selected = true, fieldSelected = true) {
  const base = file(path, selected, fieldSelected);
  return {
    ...base,
    previewed: true,
    fields: [{ ...base.fields[0], convertedValues: ["夜曲繁"], selected: fieldSelected }],
  };
}

function buttonByText(wrapper: VueWrapper, text: string) {
  return wrapper.findAll("button").find((button) => button.text().includes(text));
}

function previewButtons(wrapper: VueWrapper) {
  return wrapper
    .findAll("button")
    .map((button) => button.text().trim())
    .filter((text) => text.includes("建立預覽") || text.includes("建立標籤預覽"));
}

describe("音訊選取後自動預覽", () => {
  it("選取檔案後立即建立預覽，且任何時候都沒有建立預覽按鈕", async () => {
    openDialog.mockResolvedValueOnce("/tmp/a.mp3");
    coreRequest.mockImplementation(async (operation: string) => {
      if (operation === "audio.scan") return [file("/tmp/a.mp3")];
      if (operation === "audio.plan") return { planId: "plan-1", files: [planned("/tmp/a.mp3")] };
      return undefined;
    });
    const wrapper = mount(AudioPage, {
      global: { plugins: [ElementPlus], stubs: { transition: false } },
    });
    await flushPromises();
    expect(previewButtons(wrapper)).toEqual([]);

    await buttonByText(wrapper, "選取音訊檔案")!.trigger("click");
    await flushPromises();

    const operations = coreRequest.mock.calls.map((call) => call[0]);
    expect(operations).toContain("audio.scan");
    expect(operations).toContain("audio.plan");
    expect(previewButtons(wrapper)).toEqual([]);
    expect(buttonByText(wrapper, "重新載入")).toBeTruthy();
    expect(buttonByText(wrapper, "重新建立預覽")).toBeFalsy();
    expect(buttonByText(wrapper, "選取音訊檔案")!.attributes("disabled")).toBeUndefined();
    expect(buttonByText(wrapper, "選取資料夾")!.attributes("disabled")).toBeUndefined();
    wrapper.unmount();
  });

  it("預覽失敗或停止後也不出現建立預覽按鈕", async () => {
    openDialog.mockResolvedValueOnce("/tmp/a.mp3");
    coreRequest.mockImplementation(async (operation: string) => {
      if (operation === "audio.scan") return [file("/tmp/a.mp3")];
      if (operation === "audio.plan") throw new Error("預覽失敗");
      return undefined;
    });
    const wrapper = mount(AudioPage, {
      global: { plugins: [ElementPlus], stubs: { transition: false } },
    });
    await flushPromises();
    await buttonByText(wrapper, "選取音訊檔案")!.trigger("click");
    await flushPromises();
    expect(previewButtons(wrapper)).toEqual([]);

    await buttonByText(wrapper, "停止作業")?.trigger("click");
    await flushPromises();
    expect(previewButtons(wrapper)).toEqual([]);
    wrapper.unmount();
  });

  it("再加入檔案時去重合併，並保留既有勾選與欄位勾選", async () => {
    openDialog.mockResolvedValueOnce(["/tmp/a.mp3"]);
    openDialog.mockResolvedValueOnce(["/tmp/a.mp3", "/tmp/b.mp3"]);
    coreRequest.mockImplementation(async (operation: string, payload: { paths?: string[] }) => {
      if (operation === "audio.scan") {
        return (payload.paths ?? []).map((path) => file(path));
      }
      if (operation === "audio.plan") {
        return {
          planId: "plan-merge",
          files: (payload.paths ?? []).map((path) => planned(path)),
        };
      }
      return undefined;
    });
    const wrapper = mount(AudioPage, {
      global: { plugins: [ElementPlus], stubs: { transition: false } },
    });
    await flushPromises();
    await buttonByText(wrapper, "選取音訊檔案")!.trigger("click");
    await flushPromises();

    const fileBox = wrapper.get(".audio-row input");
    await fileBox.setValue(false);
    await fileBox.trigger("change");
    const fieldBox = wrapper.get(".audio-detail .el-checkbox input");
    await fieldBox.setValue(false);
    await fieldBox.trigger("change");

    await buttonByText(wrapper, "選取音訊檔案")!.trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain("a.mp3");
    expect(wrapper.text()).toContain("b.mp3");
    const scanPayloads = coreRequest.mock.calls
      .filter((call) => call[0] === "audio.scan")
      .map((call) => call[1] as { paths: string[] });
    expect(scanPayloads[0]?.paths).toEqual(["/tmp/a.mp3"]);
    expect(scanPayloads[1]?.paths).toEqual(["/tmp/b.mp3"]);
    const planPayload = coreRequest.mock.calls
      .filter((call) => call[0] === "audio.plan")
      .at(-1)?.[1] as { paths: string[]; selectedPaths?: string[] };
    expect(planPayload.paths).toEqual(["/tmp/a.mp3", "/tmp/b.mp3"]);
    expect(planPayload.selectedPaths ?? []).not.toContain("/tmp/a.mp3");
    const rows = wrapper.findAll(".audio-row");
    const kept = rows.find((row) => row.text().includes("a.mp3"));
    expect((kept!.get("input").element as HTMLInputElement).checked).toBe(false);
    wrapper.unmount();
  });

  it("套用中停用選取按鈕，且加入檔案不會取消 audio.apply", async () => {
    openDialog.mockResolvedValue("/tmp/a.mp3");
    let releaseApply: (value: { succeeded: string[]; failed: unknown[] }) => void = () => undefined;
    coreRequest.mockImplementation(async (operation: string) => {
      if (operation === "audio.scan") return [file("/tmp/a.mp3")];
      if (operation === "audio.plan")
        return { planId: "plan-apply", files: [planned("/tmp/a.mp3")] };
      if (operation === "audio.apply") {
        return new Promise((resolve) => {
          releaseApply = resolve;
        });
      }
      if (operation === "audio.cancel") return undefined;
      return undefined;
    });
    const wrapper = mount(AudioPage, {
      global: { plugins: [ElementPlus], stubs: { transition: false } },
    });
    await flushPromises();
    await buttonByText(wrapper, "選取音訊檔案")!.trigger("click");
    await flushPromises();
    await buttonByText(wrapper, "確認寫入")!.trigger("click");
    await flushPromises();

    expect(buttonByText(wrapper, "選取音訊檔案")!.attributes("disabled")).toBeDefined();
    expect(buttonByText(wrapper, "選取資料夾")!.attributes("disabled")).toBeDefined();
    const callsBefore = coreRequest.mock.calls.length;
    await buttonByText(wrapper, "選取音訊檔案")!.trigger("click");
    await buttonByText(wrapper, "選取資料夾")!.trigger("click");
    await flushPromises();
    const later = coreRequest.mock.calls.slice(callsBefore).map((call) => call[0]);
    expect(later).not.toContain("audio.cancel");
    expect(later).not.toContain("audio.scan");
    expect(coreCancel).not.toHaveBeenCalled();
    expect(openDialog).toHaveBeenCalledTimes(1);

    releaseApply({ succeeded: ["/tmp/a.mp3"], failed: [] });
    await flushPromises();
    wrapper.unmount();
  });
});

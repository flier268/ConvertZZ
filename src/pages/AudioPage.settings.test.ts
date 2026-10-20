/** @vitest-environment jsdom */
import { flushPromises, mount } from "@vue/test-utils";
import ElementPlus, { ElMessage } from "element-plus";
import { describe, expect, it, vi } from "vitest";
import type { SettingsV2 } from "@shared/contracts";

const openDialog = vi.fn();
const confirmDialog = vi.fn<(...args: unknown[]) => Promise<boolean>>(async () => true);
const coreRequest = vi.fn();
const patchSavedSettings = vi.fn();

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
    audio: {
      backup: false,
      conflictPolicy: "skip",
      recursive: true,
      direction: "t2s",
      engine: "legacy",
      vocabularyCorrection: false,
      id3v1Enabled: true,
      id3v1Direction: "s2t",
      id3v1SourceEncoding: "big5",
      id3v1OutputEncoding: "utf8",
      id3v2Enabled: true,
      id3v2Direction: "s2t",
      id3v2SourceEncoding: "gbk",
      id3v2RepairSourceEncoding: true,
      id3v2Version: 4,
      id3v2Encoding: "utf16",
      tagOperations: [],
    },
  }),
  patchSavedSettings: (...args: unknown[]) => patchSavedSettings(...args),
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
      values: ["里面"],
      selected: true,
    },
  ],
};

function plannedFile() {
  return {
    ...scannedFile,
    previewed: true,
    fields: [{ ...scannedFile.fields[0], convertedValues: ["裡面"] }],
  };
}

describe("音訊編輯設定", () => {
  it("關閉抽屜時若存檔失敗會提示", async () => {
    patchSavedSettings.mockRejectedValueOnce(new Error("無法寫入設定"));
    const error = vi.spyOn(ElMessage, "error").mockImplementation(() => undefined as never);
    const wrapper = mount(AudioPage, {
      attachTo: document.body,
      global: { plugins: [ElementPlus], stubs: { transition: false } },
    });
    await flushPromises();
    await wrapper
      .findAll("button")
      .find((button) => button.text().includes("編輯設定"))!
      .trigger("click");
    await flushPromises();
    const done = Array.from(document.body.querySelectorAll("button")).find(
      (button) => button.textContent?.replace(/\s+/gu, "") === "完成",
    );
    done!.click();
    await flushPromises();
    expect(error).toHaveBeenCalledWith("無法寫入設定");
    error.mockRestore();
    wrapper.unmount();
  });

  it("啟動時套用已存的編碼與備份，關閉抽屜時寫回設定", async () => {
    patchSavedSettings.mockImplementation(async (patch: (settings: SettingsV2) => void) => {
      const settings = { audio: {} } as SettingsV2;
      patch(settings);
      return settings;
    });
    const wrapper = mount(AudioPage, {
      attachTo: document.body,
      global: { plugins: [ElementPlus], stubs: { transition: false } },
    });
    await flushPromises();
    expect(wrapper.text()).toContain("不備份");
    expect(wrapper.text()).toContain("UTF-16");

    await wrapper
      .findAll("button")
      .find((button) => button.text().includes("編輯設定"))!
      .trigger("click");
    await flushPromises();
    const done = Array.from(document.body.querySelectorAll("button")).find(
      (button) => button.textContent?.replace(/\s+/gu, "") === "完成",
    );
    expect(done).toBeTruthy();
    done!.click();
    await flushPromises();

    expect(patchSavedSettings).toHaveBeenCalled();
    const written = {
      direction: "t2s",
      engine: "opencc",
      vocabularyCorrection: false,
      audio: {},
    } as unknown as SettingsV2;
    (patchSavedSettings.mock.calls[0]?.[0] as (settings: SettingsV2) => void)(written);
    expect(written.direction).toBe("t2s");
    expect(written.engine).toBe("opencc");
    expect(written.vocabularyCorrection).toBe(false);
    expect(written.audio.direction).toBe("t2s");
    expect(written.audio.engine).toBe("legacy");
    expect(written.audio.vocabularyCorrection).toBe(false);
    expect(written.audio.backup).toBe(false);
    expect(written.audio.id3v1SourceEncoding).toBe("big5");
    expect(written.audio.id3v2Encoding).toBe("utf16");
    await wrapper
      .findAll("button")
      .find((button) => button.text().includes("編輯設定"))!
      .trigger("click");
    await flushPromises();
    expect(document.body.textContent).toContain("MP3／ID3v1");
    expect(document.body.textContent).toContain("MP3／ID3v2");
    wrapper.unmount();
  });

  it("確認寫入若全部略過，會提示沒有寫入", async () => {
    const warning = vi.spyOn(ElMessage, "warning").mockImplementation(() => undefined as never);
    openDialog.mockResolvedValueOnce("/tmp/song.mp3");
    coreRequest.mockImplementation(async (operation: string) => {
      if (operation === "audio.scan") return [scannedFile];
      if (operation === "audio.plan") return { planId: "plan-1", files: [plannedFile()] };
      if (operation === "audio.apply")
        return { succeeded: [], skipped: ["/tmp/song.mp3"], failed: [] };
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
    await wrapper
      .findAll("button")
      .find((button) => button.text().includes("確認寫入"))!
      .trigger("click");
    await flushPromises();
    expect(warning).toHaveBeenCalledWith(expect.stringContaining("沒有寫入任何檔案"));
    const operations = coreRequest.mock.calls.map((call) => call[0] as string);
    expect(operations.filter((operation) => operation === "audio.scan")).toHaveLength(1);
    expect(wrapper.text()).toContain("轉換預覽");
    warning.mockRestore();
    wrapper.unmount();
  });

  it("寫入成功後重掃磁碟、不再自動預覽", async () => {
    openDialog.mockResolvedValueOnce("/tmp/song.mp3");
    coreRequest.mockImplementation(async (operation: string) => {
      if (operation === "audio.scan")
        return [{ ...scannedFile, fields: [{ ...scannedFile.fields[0], values: ["裡面"] }] }];
      if (operation === "audio.plan") return { planId: "plan-1", files: [plannedFile()] };
      if (operation === "audio.apply")
        return { succeeded: ["/tmp/song.mp3"], skipped: [], failed: [] };
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
    expect(wrapper.text()).toContain("轉換預覽");
    await wrapper
      .findAll("button")
      .find((button) => button.text().includes("確認寫入"))!
      .trigger("click");
    await flushPromises();
    const operations = coreRequest.mock.calls.map((call) => call[0] as string);
    const afterApply = operations.slice(operations.lastIndexOf("audio.apply"));
    expect(afterApply).toContain("audio.scan");
    expect(afterApply.filter((operation) => operation === "audio.plan")).toEqual([]);
    expect(wrapper.text()).toContain("裡面");
    expect(wrapper.text()).not.toContain("轉換預覽");
    expect(wrapper.findAll("button").some((button) => button.text().includes("確認寫入"))).toBe(
      false,
    );
    wrapper.unmount();
  });

  it("寫入中途失敗會清掉預覽並重掃磁碟", async () => {
    openDialog.mockResolvedValueOnce("/tmp/song.mp3");
    coreRequest.mockImplementation(async (operation: string) => {
      if (operation === "audio.scan") return [scannedFile];
      if (operation === "audio.plan") return { planId: "plan-1", files: [plannedFile()] };
      if (operation === "audio.apply") throw new Error("磁碟已滿");
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
    await wrapper
      .findAll("button")
      .find((button) => button.text().includes("確認寫入"))!
      .trigger("click");
    await flushPromises();
    const operations = coreRequest.mock.calls.map((call) => call[0] as string);
    const afterApply = operations.slice(operations.lastIndexOf("audio.apply"));
    expect(afterApply).toContain("audio.scan");
    expect(wrapper.text()).not.toContain("轉換預覽");
    wrapper.unmount();
  });

  it("部分寫入失敗時保留預覽並只重掃已寫入檔案", async () => {
    const second = {
      ...scannedFile,
      path: "/tmp/other.mp3",
      fields: [{ ...scannedFile.fields[0], values: ["里面"] }],
    };
    openDialog.mockResolvedValueOnce(["/tmp/song.mp3", "/tmp/other.mp3"]);
    coreRequest.mockImplementation(async (operation: string, payload?: { paths?: string[] }) => {
      if (operation === "audio.scan") {
        const paths = payload?.paths ?? [];
        if (paths.length === 1 && paths[0] === "/tmp/song.mp3")
          return [{ ...scannedFile, fields: [{ ...scannedFile.fields[0], values: ["裡面"] }] }];
        return [scannedFile, second];
      }
      if (operation === "audio.plan")
        return {
          planId: "plan-1",
          files: [
            plannedFile(),
            {
              ...second,
              previewed: true,
              fields: [{ ...second.fields[0], convertedValues: ["裡面"] }],
            },
          ],
        };
      if (operation === "audio.apply")
        return {
          succeeded: ["/tmp/song.mp3"],
          skipped: [],
          failed: [{ path: "/tmp/other.mp3", message: "權限不足" }],
        };
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
    await wrapper
      .findAll("button")
      .find((button) => button.text().includes("確認寫入"))!
      .trigger("click");
    await flushPromises();
    const scanCalls = coreRequest.mock.calls.filter((call) => call[0] === "audio.scan");
    const lastScan = scanCalls.at(-1)?.[1] as { paths?: string[] };
    expect(lastScan.paths).toEqual(["/tmp/song.mp3"]);
    expect(wrapper.text()).toContain("轉換預覽");
    wrapper.unmount();
  });

  it("寫入後完整重掃失敗時保留原清單", async () => {
    openDialog.mockResolvedValueOnce("/tmp/song.mp3");
    let scans = 0;
    coreRequest.mockImplementation(async (operation: string) => {
      if (operation === "audio.scan") {
        scans += 1;
        if (scans > 1) throw new Error("掃描失敗");
        return [scannedFile];
      }
      if (operation === "audio.plan") return { planId: "plan-1", files: [plannedFile()] };
      if (operation === "audio.apply")
        return { succeeded: ["/tmp/song.mp3"], skipped: [], failed: [] };
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
    await wrapper
      .findAll("button")
      .find((button) => button.text().includes("確認寫入"))!
      .trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("song.mp3");
    expect(wrapper.text()).toContain("里面");
    wrapper.unmount();
  });
});

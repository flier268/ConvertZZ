/** @vitest-environment jsdom */
import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import ElementPlus, { ElMessage } from "element-plus";
import { beforeEach, describe, expect, it, vi } from "vitest";

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
  isCancellationError: (error: unknown) =>
    Boolean(
      error && typeof error === "object" && (error as { code?: string }).code === "PLAN_CANCELLED",
    ),
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

function file(path: string, selected = true, fieldSelected = true, fieldKey = "title") {
  return {
    path,
    format: "mp3",
    selected,
    hasCoverArt: false,
    fields: [
      {
        key: fieldKey,
        label: "標題",
        container: "id3v2",
        values: ["夜曲"],
        selected: fieldSelected,
      },
    ],
  };
}

function planned(path: string, fieldKey = "title") {
  const base = file(path, true, true, fieldKey);
  return {
    ...base,
    previewed: true,
    fields: [{ ...base.fields[0], convertedValues: ["夜曲繁"], selected: true }],
  };
}

function buttonByText(wrapper: VueWrapper, text: string) {
  return wrapper.findAll("button").find((button) => button.text().includes(text));
}

function scanPayloads(): { paths: string[] }[] {
  return coreRequest.mock.calls
    .filter((call) => call[0] === "audio.scan")
    .map((call) => call[1] as { paths: string[] });
}

function mountPage() {
  return mount(AudioPage, {
    global: { plugins: [ElementPlus], stubs: { transition: false } },
  });
}

describe("音訊掃描修正", () => {
  beforeEach(() => {
    openDialog.mockReset();
    confirmDialog.mockReset();
    confirmDialog.mockResolvedValue(true);
    coreRequest.mockReset();
    coreCancel.mockReset();
  });

  it("再試一次與重新掃描都對既有路徑做完整掃描", async () => {
    openDialog.mockResolvedValueOnce("/tmp/a.mp3");
    coreRequest.mockImplementation(async (operation: string) => {
      if (operation === "audio.scan") throw new Error("讀不到標籤");
      return undefined;
    });
    const wrapper = mountPage();
    await flushPromises();
    await buttonByText(wrapper, "選取音訊檔案")!.trigger("click");
    await flushPromises();

    expect(buttonByText(wrapper, "再試一次")).toBeTruthy();
    await buttonByText(wrapper, "再試一次")!.trigger("click");
    await flushPromises();

    coreRequest.mockImplementation(async (operation: string) => {
      if (operation === "audio.scan") return [file("/tmp/a.mp3")];
      if (operation === "audio.plan") throw new Error("預覽失敗");
      return undefined;
    });
    await buttonByText(wrapper, "再試一次")!.trigger("click");
    await flushPromises();

    expect(buttonByText(wrapper, "重新掃描")).toBeTruthy();
    await buttonByText(wrapper, "重新掃描")!.trigger("click");
    await flushPromises();

    for (const payload of scanPayloads()) {
      expect(Array.isArray(payload.paths)).toBe(true);
      expect(payload.paths).toEqual(["/tmp/a.mp3"]);
    }
    expect(scanPayloads().length).toBeGreaterThanOrEqual(3);
    wrapper.unmount();
  });

  it("掃描進行中再加入路徑時改掃全部路徑", async () => {
    openDialog.mockResolvedValueOnce("/tmp/a.mp3");
    openDialog.mockResolvedValueOnce("/tmp/b.mp3");
    let releaseFirst: (files: ReturnType<typeof file>[]) => void = () => undefined;
    coreRequest.mockImplementation(async (operation: string) => {
      if (operation === "audio.scan") {
        const calls = coreRequest.mock.calls.filter((call) => call[0] === "audio.scan");
        if (calls.length === 1) {
          return new Promise((resolve) => {
            releaseFirst = resolve;
          });
        }
        return [file("/tmp/a.mp3"), file("/tmp/b.mp3")];
      }
      if (operation === "audio.plan") {
        return { planId: "plan-both", files: [planned("/tmp/a.mp3"), planned("/tmp/b.mp3")] };
      }
      return undefined;
    });
    const wrapper = mountPage();
    await flushPromises();
    await buttonByText(wrapper, "選取音訊檔案")!.trigger("click");
    await flushPromises();
    await buttonByText(wrapper, "選取音訊檔案")!.trigger("click");
    await flushPromises();

    const second = scanPayloads()[1];
    expect(second?.paths).toEqual(["/tmp/a.mp3", "/tmp/b.mp3"]);
    releaseFirst([]);
    await flushPromises();
    expect(wrapper.text()).toContain("a.mp3");
    expect(wrapper.text()).toContain("b.mp3");
    wrapper.unmount();
  });

  it("停止掃描或掃描失敗後不建立預覽", async () => {
    openDialog.mockResolvedValue("/tmp/a.mp3");
    let rejectScan: (error: unknown) => void = () => undefined;
    coreRequest.mockImplementation(
      async (
        operation: string,
        _payload: unknown,
        options?: { onProgress?: (value: unknown) => void; onRequestId?: (id: string) => void },
      ) => {
        if (operation === "audio.scan") {
          options?.onRequestId?.("scan-1");
          options?.onProgress?.({
            current: 0,
            total: 1,
            message: "掃描中",
            batch: { phase: "scan", files: [file("/tmp/a.mp3")] },
          });
          return new Promise((_resolve, reject) => {
            rejectScan = reject;
          });
        }
        if (operation === "audio.plan") return { planId: "should-not", files: [] };
        return undefined;
      },
    );
    coreCancel.mockImplementation(async () => {
      rejectScan(Object.assign(new Error("cancelled"), { code: "PLAN_CANCELLED" }));
    });
    const wrapper = mountPage();
    await flushPromises();
    await buttonByText(wrapper, "選取音訊檔案")!.trigger("click");
    await flushPromises();
    await buttonByText(wrapper, "停止作業")!.trigger("click");
    await flushPromises();
    expect(coreRequest.mock.calls.some((call) => call[0] === "audio.plan")).toBe(false);

    coreRequest.mockImplementation(
      async (
        operation: string,
        _payload: unknown,
        options?: { onProgress?: (value: unknown) => void },
      ) => {
        if (operation === "audio.scan") {
          options?.onProgress?.({
            current: 0,
            total: 1,
            message: "掃描中",
            batch: { phase: "scan", files: [file("/tmp/a.mp3")] },
          });
          throw new Error("讀取失敗");
        }
        if (operation === "audio.plan") return { planId: "should-not", files: [] };
        return undefined;
      },
    );
    await buttonByText(wrapper, "重新掃描")!.trigger("click");
    await flushPromises();
    expect(coreRequest.mock.calls.filter((call) => call[0] === "audio.plan")).toHaveLength(0);
    wrapper.unmount();
  });

  it("未聚焦資料夾時顯示路徑數與全部檔案數，聚焦時含子資料夾", async () => {
    openDialog.mockResolvedValueOnce("/music");
    coreRequest.mockImplementation(async (operation: string) => {
      if (operation === "audio.scan") {
        return [file("/music/a.mp3"), file("/music/sub/b.mp3")];
      }
      if (operation === "audio.plan") throw new Error("先看副標");
      return undefined;
    });
    const wrapper = mountPage();
    await flushPromises();
    expect(wrapper.text()).toContain("ID3、APEv2 與 Vorbis Comment 都會保留未選欄位與封面。");
    await buttonByText(wrapper, "選取資料夾")!.trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("1 個路徑 · 全部 2 個檔案");

    const folder = wrapper
      .findAll(".audio-folder")
      .find((row) => row.text().includes("/music") && !row.text().includes("/music/sub"));
    expect(folder).toBeTruthy();
    await folder!.trigger("click");
    expect(wrapper.text()).toContain("/music · 2 個檔案");
    wrapper.unmount();
  });

  it("路徑寫法不同仍去重，欄位鍵重產後仍保留勾選", async () => {
    openDialog.mockResolvedValueOnce(["C:\\Music\\a.mp3"]);
    openDialog.mockResolvedValueOnce(["c:/music/a.mp3/"]);
    let planRound = 0;
    coreRequest.mockImplementation(async (operation: string) => {
      if (operation === "audio.scan") return [file("C:\\Music\\a.mp3")];
      if (operation === "audio.plan") {
        planRound += 1;
        if (planRound === 1) return { planId: "plan-1", files: [planned("C:\\Music\\a.mp3")] };
        return { planId: "plan-2", files: [planned("c:/music/a.mp3", "TIT2")] };
      }
      return undefined;
    });
    const wrapper = mountPage();
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
    await buttonByText(wrapper, "重新載入")!.trigger("click");
    await flushPromises();

    expect(scanPayloads()).toHaveLength(1);
    const rows = wrapper.findAll(".audio-row");
    expect(rows).toHaveLength(1);
    expect((rows[0]!.get("input").element as HTMLInputElement).checked).toBe(false);
    const field = wrapper.get(".audio-detail .el-checkbox input").element as HTMLInputElement;
    expect(field.checked).toBe(false);
    wrapper.unmount();
  });

  it("完整重新掃描後仍保留已取消的欄位勾選", async () => {
    openDialog.mockResolvedValueOnce("/tmp/a.mp3");
    coreRequest.mockImplementation(async (operation: string) => {
      if (operation === "audio.scan") return [file("/tmp/a.mp3")];
      if (operation === "audio.plan") throw new Error("先看勾選");
      return undefined;
    });
    const wrapper = mountPage();
    await flushPromises();
    await buttonByText(wrapper, "選取音訊檔案")!.trigger("click");
    await flushPromises();
    const fieldBox = wrapper.get(".audio-detail .el-checkbox input");
    await fieldBox.setValue(false);
    await fieldBox.trigger("change");

    await buttonByText(wrapper, "重新掃描")!.trigger("click");
    await flushPromises();

    const field = wrapper.get(".audio-detail .el-checkbox input").element as HTMLInputElement;
    expect(field.checked).toBe(false);
    wrapper.unmount();
  });

  it("掃描進行中加入路徑後仍保留已取消的欄位勾選", async () => {
    openDialog.mockResolvedValueOnce("/tmp/a.mp3");
    openDialog.mockResolvedValueOnce("/tmp/b.mp3");
    let releaseRescan: (files: ReturnType<typeof file>[]) => void = () => undefined;
    let scanCount = 0;
    coreRequest.mockImplementation(async (operation: string) => {
      if (operation === "audio.scan") {
        scanCount += 1;
        if (scanCount === 1) return [file("/tmp/a.mp3")];
        if (scanCount === 2) {
          return new Promise((resolve) => {
            releaseRescan = resolve;
          });
        }
        return [file("/tmp/a.mp3", true, true), file("/tmp/b.mp3")];
      }
      if (operation === "audio.plan") throw new Error("先看勾選");
      return undefined;
    });
    const wrapper = mountPage();
    await flushPromises();
    await buttonByText(wrapper, "選取音訊檔案")!.trigger("click");
    await flushPromises();
    const fieldBox = wrapper.get(".audio-detail .el-checkbox input");
    await fieldBox.setValue(false);
    await fieldBox.trigger("change");

    await buttonByText(wrapper, "重新掃描")!.trigger("click");
    await flushPromises();
    await buttonByText(wrapper, "選取音訊檔案")!.trigger("click");
    await flushPromises();
    releaseRescan([file("/tmp/a.mp3", true, true)]);
    await flushPromises();

    const rows = wrapper.findAll(".audio-row");
    expect(rows.some((row) => row.text().includes("a.mp3"))).toBe(true);
    expect(rows.some((row) => row.text().includes("b.mp3"))).toBe(true);
    await rows.find((row) => row.text().includes("a.mp3"))!.trigger("click");
    const field = wrapper.get(".audio-detail .el-checkbox input").element as HTMLInputElement;
    expect(field.checked).toBe(false);
    wrapper.unmount();
  });

  it("後續掃描取消前一輪時不顯示已取消掃描，作業也不會中途變閒置", async () => {
    openDialog.mockResolvedValueOnce("/tmp/a.mp3");
    openDialog.mockResolvedValueOnce("/tmp/b.mp3");
    const info = vi.spyOn(ElMessage, "info").mockImplementation(() => ({ close: () => undefined }));
    let rejectFirst: (error: unknown) => void = () => undefined;
    let scanCount = 0;
    coreRequest.mockImplementation(
      async (
        operation: string,
        _payload: unknown,
        options?: { onRequestId?: (id: string) => void },
      ) => {
        if (operation === "audio.scan") {
          scanCount += 1;
          if (scanCount === 1) {
            options?.onRequestId?.("scan-1");
            return new Promise((_resolve, reject) => {
              rejectFirst = reject;
            });
          }
          return [file("/tmp/a.mp3"), file("/tmp/b.mp3")];
        }
        if (operation === "audio.plan") throw new Error("先看取消");
        return undefined;
      },
    );
    coreCancel.mockImplementation(async () => {
      rejectFirst(Object.assign(new Error("cancelled"), { code: "PLAN_CANCELLED" }));
      await flushPromises();
      expect(info).not.toHaveBeenCalled();
      expect(wrapper.text()).toContain("停止作業");
    });
    const wrapper = mountPage();
    await flushPromises();
    await buttonByText(wrapper, "選取音訊檔案")!.trigger("click");
    await flushPromises();
    await buttonByText(wrapper, "選取音訊檔案")!.trigger("click");
    await flushPromises();

    expect(info).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain("b.mp3");
    info.mockRestore();
    wrapper.unmount();
  });

  it("同檔重複標籤名稱不會用標籤名互相覆蓋勾選", async () => {
    const tagged = (key: string, selected: boolean) => ({
      key,
      label: "備註",
      container: "id3v2",
      values: [key],
      selected,
    });
    openDialog.mockResolvedValueOnce("/tmp/a.mp3");
    let planCount = 0;
    coreRequest.mockImplementation(async (operation: string) => {
      if (operation === "audio.scan") {
        return [
          {
            ...file("/tmp/a.mp3"),
            fields: [tagged("TXXX:a", true), tagged("TXXX:b", true)],
          },
        ];
      }
      if (operation === "audio.plan") {
        planCount += 1;
        const keyA = planCount === 1 ? "TXXX:a" : "TXXX:a2";
        const keyB = planCount === 1 ? "TXXX:b" : "TXXX:b2";
        return {
          planId: `plan-${planCount}`,
          files: [
            {
              ...file("/tmp/a.mp3"),
              previewed: true,
              fields: [
                { ...tagged(keyA, true), convertedValues: ["甲"] },
                { ...tagged(keyB, true), convertedValues: ["乙"] },
              ],
            },
          ],
        };
      }
      return undefined;
    });
    const wrapper = mountPage();
    await flushPromises();
    await buttonByText(wrapper, "選取音訊檔案")!.trigger("click");
    await flushPromises();
    const boxes = wrapper.findAll(".audio-detail .el-checkbox input");
    expect(boxes).toHaveLength(2);
    await boxes[1]!.setValue(false);
    await boxes[1]!.trigger("change");
    expect((boxes[1]!.element as HTMLInputElement).checked).toBe(false);

    await buttonByText(wrapper, "重新載入")!.trigger("click");
    await flushPromises();

    const after = wrapper.findAll(".audio-detail .el-checkbox input");
    expect((after[0]!.element as HTMLInputElement).checked).toBe(true);
    expect((after[1]!.element as HTMLInputElement).checked).toBe(true);
    wrapper.unmount();
  });

  it("聚焦資料夾時用路徑鍵計算副標筆數", async () => {
    openDialog.mockResolvedValueOnce("/music");
    coreRequest.mockImplementation(async (operation: string) => {
      if (operation === "audio.scan") {
        return [file("C:\\Music\\a.mp3"), file("c:/music/b.mp3")];
      }
      if (operation === "audio.plan") throw new Error("先看副標");
      return undefined;
    });
    const wrapper = mountPage();
    await flushPromises();
    await buttonByText(wrapper, "選取資料夾")!.trigger("click");
    await flushPromises();
    const folder = wrapper.findAll(".audio-folder").find((row) => row.text().includes("C:/Music"));
    expect(folder).toBeTruthy();
    await folder!.trigger("click");
    expect(wrapper.text()).toContain("C:/Music · 2 個檔案");
    wrapper.unmount();
  });

  it("預覽計畫路徑寫法不同時仍留在原位", async () => {
    openDialog.mockResolvedValueOnce(["C:\\Music\\a.mp3", "/tmp/b.mp3"]);
    coreRequest.mockImplementation(async (operation: string) => {
      if (operation === "audio.scan") {
        return [file("C:\\Music\\a.mp3"), file("/tmp/b.mp3")];
      }
      if (operation === "audio.plan") {
        return {
          planId: "plan-case",
          files: [planned("c:/music/a.mp3"), planned("/tmp/b.mp3")],
        };
      }
      return undefined;
    });
    const wrapper = mountPage();
    await flushPromises();
    await buttonByText(wrapper, "選取音訊檔案")!.trigger("click");
    await flushPromises();
    const folders = wrapper.findAll(".audio-folder-path").map((row) => row.text());
    expect(folders[0]?.toLowerCase()).toBe("c:/music");
    expect(folders[1]).toBe("/tmp");
    wrapper.unmount();
  });

  it("可從列表移除檔案，預覽不會把篩選切回有變更", async () => {
    openDialog.mockResolvedValueOnce(["/tmp/a.mp3", "/tmp/b.mp3"]);
    coreRequest.mockImplementation(async (operation: string) => {
      if (operation === "audio.scan") return [file("/tmp/a.mp3"), file("/tmp/b.mp3")];
      if (operation === "audio.plan") {
        return { planId: "plan-rm", files: [planned("/tmp/a.mp3"), planned("/tmp/b.mp3")] };
      }
      return undefined;
    });
    const wrapper = mountPage();
    await flushPromises();
    await buttonByText(wrapper, "選取音訊檔案")!.trigger("click");
    await flushPromises();
    await wrapper.get('[aria-label="從列表移除 a.mp3"]').trigger("click");
    await flushPromises();
    expect(wrapper.text()).not.toContain("a.mp3");
    expect(wrapper.text()).toContain("b.mp3");
    wrapper.unmount();
  });
});

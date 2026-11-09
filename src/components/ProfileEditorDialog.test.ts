/** @vitest-environment jsdom */
import { flushPromises, mount } from "@vue/test-utils";
import ElementPlus from "element-plus";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { FileFilterProfile } from "@shared/contracts";

const openDialog = vi.fn();
const coreRequest = vi.fn();

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: (...args: unknown[]) => openDialog(...args),
}));
vi.mock("../lib/coreClient", () => ({
  core: { request: (...args: unknown[]) => coreRequest(...args), cancel: vi.fn() },
  isCancellationError: () => false,
}));

import { builtinAllProfile, builtinTextProfile } from "../lib/filterProfiles";
import ProfileEditorDialog from "./ProfileEditorDialog.vue";

const blank: FileFilterProfile = {
  id: "u-new",
  name: "我的篩選",
  builtin: false,
  mode: "allow",
  extensions: [],
  includeNoExtension: false,
};

function mountDialog(profile: FileFilterProfile = blank) {
  const wrapper = mount(ProfileEditorDialog, {
    props: {
      modelValue: true,
      profile,
      existing: [builtinTextProfile(), builtinAllProfile()],
    },
    global: { plugins: [ElementPlus] },
    attachTo: document.body,
  });
  return wrapper;
}

function type(text: string): void {
  const input = document.body.querySelector('input[aria-label="新增副檔名"]') as HTMLInputElement;
  input.value = text;
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

function button(text: string): HTMLButtonElement {
  return Array.from(document.body.querySelectorAll("button")).find(
    (item) => item.textContent?.trim() === text,
  ) as HTMLButtonElement;
}

describe("ProfileEditorDialog", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
    openDialog.mockReset();
    coreRequest.mockReset();
  });

  it("空白名單不能儲存；輸入後正規化、去重並可儲存", async () => {
    const wrapper = mountDialog();
    await flushPromises();
    expect(button("儲存").disabled).toBe(true);
    expect(document.body.textContent).toContain("白名單至少要有一個副檔名");

    type("*.SRT;*.ass .srt");
    await flushPromises();
    const tags = Array.from(document.body.querySelectorAll(".ext-tags .el-tag")).map((tag) =>
      tag.textContent?.trim(),
    );
    expect(tags).toEqual([".srt", ".ass"]);
    expect(document.body.textContent).toContain("已存在，略過：.srt");
    expect(button("儲存").disabled).toBe(false);
    button("儲存").click();
    await flushPromises();
    const saved = wrapper.emitted("save")![0][0] as FileFilterProfile;
    expect(saved).toMatchObject({
      id: "u-new",
      name: "我的篩選",
      mode: "allow",
      extensions: [".srt", ".ass"],
      builtin: false,
    });
    wrapper.unmount();
  });

  it("貼上 * 不當副檔名並提示改選全部", async () => {
    const wrapper = mountDialog();
    await flushPromises();
    type("*.* ");
    await flushPromises();
    expect(document.body.textContent).toContain("* 與 *.* 不是副檔名");
    expect(document.body.querySelectorAll(".ext-tags .el-tag")).toHaveLength(0);
    wrapper.unmount();
  });

  it("空黑名單提示等同所有檔案但仍可儲存", async () => {
    const wrapper = mountDialog({ ...blank, mode: "deny" });
    await flushPromises();
    expect(document.body.textContent).toContain("等同所有檔案");
    expect(button("儲存").disabled).toBe(false);
    wrapper.unmount();
  });

  it("編輯內建 profile 儲存為新的自訂 profile，內建本身不變", async () => {
    const builtin = builtinTextProfile();
    const wrapper = mountDialog(builtin);
    await flushPromises();
    expect(document.body.textContent).toContain("另存「常用文字檔案」為自訂 profile");
    type(".md ");
    await flushPromises();
    button("儲存").click();
    await flushPromises();
    const saved = wrapper.emitted("save")![0][0] as FileFilterProfile;
    expect(saved.id).not.toBe("builtin-text");
    expect(saved.builtin).toBe(false);
    expect(saved.name).toBe("常用文字檔案 副本");
    expect(saved.extensions).toContain(".md");
    expect(builtin.extensions).not.toContain(".md");
    wrapper.unmount();
  });

  it("掃描目錄後可勾選副檔名與無副檔名，並與標籤共用同一份清單", async () => {
    openDialog.mockResolvedValue("/tmp/samples");
    coreRequest.mockResolvedValue({
      extensions: [
        { extension: ".srt", count: 4 },
        { extension: ".png", count: 2 },
      ],
      noExtensionCount: 1,
      totalFiles: 7,
      directoryCount: 0,
      warnings: [],
    });
    const wrapper = mountDialog();
    await flushPromises();
    (
      Array.from(document.body.querySelectorAll(".el-tabs__item")).find(
        (item) => item.textContent === "掃描目錄勾選",
      ) as HTMLElement
    ).click();
    await flushPromises();
    button("選擇目錄…").click();
    await flushPromises();
    expect(coreRequest).toHaveBeenCalledWith(
      "files.scanExtensions",
      { path: "/tmp/samples", recursive: true },
      expect.anything(),
    );
    const rows = Array.from(document.body.querySelectorAll(".scan-list li"));
    expect(rows.map((row) => row.textContent?.replace(/\s+/gu, ""))).toEqual([
      ".srt4",
      ".png2",
      "（無副檔名）1",
    ]);
    button("全選").click();
    await flushPromises();
    button("反選").click();
    await flushPromises();
    expect(document.body.querySelectorAll(".scan-list input:checked")).toHaveLength(0);
    (rows[0].querySelector("input") as HTMLInputElement).click();
    (rows[2].querySelector("input") as HTMLInputElement).click();
    await flushPromises();
    button("儲存").click();
    await flushPromises();
    expect(wrapper.emitted("save")![0][0]).toMatchObject({
      extensions: [".srt"],
      includeNoExtension: true,
    });
    wrapper.unmount();
  });
});

/** @vitest-environment jsdom */
import { mount } from "@vue/test-utils";
import ElementPlus from "element-plus";
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("../lib/coreClient", () => ({
  core: { request: vi.fn(), cancel: vi.fn() },
  isCancellationError: () => false,
}));

import { builtinTextProfile } from "../lib/filterProfiles";
import FolderPickWizard from "./FolderPickWizard.vue";
import ProfileEditorDialog from "./ProfileEditorDialog.vue";

describe("FolderPickWizard", () => {
  it("編輯 profile 對話框與流程對話框同層，且點遮罩不關閉", () => {
    const wrapper = mount(FolderPickWizard, {
      props: {
        modelValue: true,
        profiles: [builtinTextProfile()],
        initialPath: "",
        recursive: true,
        profileId: "builtin-text",
        quick: false,
      },
      global: { plugins: [ElementPlus] },
    });
    const wizard = wrapper.findComponent({ name: "ElDialog" });
    expect(wizard.props("closeOnClickModal")).toBe(false);
    expect(wizard.findComponent(ProfileEditorDialog).exists()).toBe(false);
    const editor = wrapper.findComponent(ProfileEditorDialog);
    expect(editor.exists()).toBe(true);
    expect(editor.findComponent({ name: "ElDialog" }).props("closeOnClickModal")).toBe(false);
  });
});

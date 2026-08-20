/** @vitest-environment jsdom */
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import SideBySideDiffView from "./SideBySideDiffView.vue";
import { buildInterleavedDiffPair } from "../lib/textDiff.fixtures";

describe("SideBySideDiffView 渲染效能", () => {
  it("掛載 3000 個獨立差異點的耗時", { timeout: 15_000 }, async () => {
    const { source, output } = buildInterleavedDiffPair(3000);
    const started = performance.now();
    const wrapper = mount(SideBySideDiffView, {
      props: {
        source,
        output,
        sourceLabel: "來源",
        outputLabel: "輸出",
      },
    });
    await wrapper.vm.$nextTick();
    const elapsedMs = performance.now() - started;
    const marks = wrapper.findAll("mark.diff-change");

    console.info(
      JSON.stringify({
        case: "SideBySideDiffView-mount-3000",
        note: "v-html 單次注入，避免數千 VNode",
        elapsedMs: Math.round(elapsedMs * 100) / 100,
        markCount: marks.length,
        removeMarks: wrapper.findAll("mark.diff-remove").length,
        addMarks: wrapper.findAll("mark.diff-add").length,
      }),
    );

    expect(marks.length).toBe(6000);
    expect(elapsedMs).toBeLessThan(2_000);
    wrapper.unmount();
  });
});

/** @vitest-environment jsdom */
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import SideBySideDiffView from "./SideBySideDiffView.vue";

const navStubs = {
  "el-splitter": { template: "<div><slot /></div>" },
  "el-splitter-panel": { template: "<div><slot /></div>" },
  "el-pagination": {
    props: ["total", "currentPage", "pageSize"],
    template: `<div class="el-pagination-stub">{{ total }}</div>`,
  },
  // 宣告 emits，避免 Vue 3 把 @click 當 fallthrough 又再 $emit 造成連跳兩次。
  "el-button": {
    emits: ["click"],
    template: `<button type="button" @click="$emit('click')"><slot /></button>`,
  },
  "el-empty": true,
  "el-icon": true,
};

function mockPaneLayout(pane: HTMLElement, markTops: number[]) {
  Object.defineProperty(pane, "clientHeight", { configurable: true, value: 300 });
  Object.defineProperty(pane, "scrollHeight", { configurable: true, value: 2000 });
  let scrollTop = 0;
  Object.defineProperty(pane, "scrollTop", {
    configurable: true,
    get: () => scrollTop,
    set: (value: number) => {
      scrollTop = value;
    },
  });
  const marks = pane.querySelectorAll<HTMLElement>("mark.diff-change");
  marks.forEach((mark, index) => {
    Object.defineProperty(mark, "offsetTop", {
      configurable: true,
      value: markTops[index] ?? 100 + index * 40,
    });
  });
}

describe("SideBySideDiffView 分頁與差異導航", () => {
  it("長文分頁時顯示分頁與上下差異按鈕", async () => {
    const source = `${"甲".repeat(60)}简体${"乙".repeat(60)}开发`;
    const output = `${"甲".repeat(60)}簡體${"乙".repeat(60)}開發`;
    const wrapper = mount(SideBySideDiffView, {
      props: {
        source,
        output,
        paginated: true,
        showNav: true,
        pageSize: 50,
      },
      global: { stubs: navStubs },
    });
    await wrapper.vm.$nextTick();

    expect(wrapper.text()).toContain("上一個差異");
    expect(wrapper.text()).toContain("下一個差異");
    expect(wrapper.text()).toMatch(/\d+ \/ \d+ 頁/);

    const nextButton = wrapper
      .findAll("button")
      .find((button) => button.text().includes("下一個差異"));
    expect(nextButton).toBeTruthy();
    await nextButton!.trigger("click");
    await wrapper.vm.$nextTick();
    // 下一個差異會跨頁跳到含變更的頁，並標出目前位置。
    expect(wrapper.findAll("mark.diff-change").length).toBeGreaterThan(0);
    expect(wrapper.findAll("mark.diff-change.is-active").length).toBeGreaterThan(0);

    wrapper.unmount();
  });

  it("跳到下一個差異時左右窗格會同步捲動", async () => {
    const source = `${"前".repeat(80)}简体${"中".repeat(80)}开发${"後".repeat(80)}`;
    const output = `${"前".repeat(80)}簡體${"中".repeat(80)}開發${"後".repeat(80)}`;
    const wrapper = mount(SideBySideDiffView, {
      props: {
        source,
        output,
        showNav: true,
        paginated: false,
      },
      global: { stubs: navStubs },
    });
    await wrapper.vm.$nextTick();

    const panes = wrapper.findAll("pre.preview-diff-body");
    expect(panes).toHaveLength(2);
    mockPaneLayout(panes[0]!.element as HTMLElement, [400, 900]);
    mockPaneLayout(panes[1]!.element as HTMLElement, [404, 908]);

    const nextButton = wrapper
      .findAll("button")
      .find((button) => button.text().includes("下一個差異"));
    await nextButton!.trigger("click");
    await wrapper.vm.$nextTick();

    expect((panes[0]!.element as HTMLElement).scrollTop).toBe(300);
    expect((panes[1]!.element as HTMLElement).scrollTop).toBe(304);
    expect(wrapper.findAll("mark.diff-change.is-active")).toHaveLength(2);

    await nextButton!.trigger("click");
    await wrapper.vm.$nextTick();
    expect((panes[0]!.element as HTMLElement).scrollTop).toBe(800);
    expect((panes[1]!.element as HTMLElement).scrollTop).toBe(808);

    wrapper.unmount();
  });
});

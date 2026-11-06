/** @vitest-environment jsdom */
import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import VirtualList from "./VirtualList.vue";

describe("VirtualList", () => {
  it("固定列高只渲染可視範圍與少量緩衝", () => {
    const wrapper = mount(VirtualList, {
      props: { count: 1000, itemHeight: 40, overscan: 2, viewportHeight: 200 },
      slots: { default: ({ index }: { index: number }) => `檔案-${index}` },
    });
    const rows = wrapper.findAll(".virtual-list-row");
    expect(rows.length).toBeGreaterThan(0);
    expect(rows.length).toBeLessThan(30);
    expect(wrapper.text()).toContain("檔案-0");
    expect(wrapper.text()).not.toContain("檔案-80");
    expect(wrapper.find(".virtual-list-spacer").attributes("style")).toContain("40000px");
  });

  it("捲動後改渲染後面的列", async () => {
    const wrapper = mount(VirtualList, {
      props: { count: 500, itemHeight: 40, overscan: 1, viewportHeight: 80 },
      slots: { default: ({ index }: { index: number }) => `列${index}` },
    });
    const host = wrapper.get(".virtual-list").element as HTMLElement;
    host.scrollTop = 4000;
    await wrapper.get(".virtual-list").trigger("scroll");
    expect(wrapper.text()).toContain("列100");
    expect(wrapper.text()).not.toContain("列0");
  });

  it("清單變短後把捲動位置夾回可捲範圍", async () => {
    const wrapper = mount(VirtualList, {
      props: { count: 100, itemHeight: 40, overscan: 1, viewportHeight: 80 },
      slots: { default: ({ index }: { index: number }) => `列${index}` },
    });
    const host = wrapper.get(".virtual-list").element as HTMLElement;
    host.scrollTop = 3000;
    await wrapper.get(".virtual-list").trigger("scroll");
    expect(wrapper.text()).toContain("列75");
    await wrapper.setProps({ count: 3 });
    expect(host.scrollTop).toBe(40);
    expect(wrapper.text()).toContain("列0");
    expect(wrapper.text()).not.toContain("列75");
  });

  it("區段標題捲出視窗後黏在上方", async () => {
    const wrapper = mount(VirtualList, {
      props: {
        count: 50,
        itemHeight: 40,
        overscan: 1,
        viewportHeight: 80,
        sectionOf: (index: number) => Math.floor(index / 10) * 10,
      },
      slots: {
        default: ({ index }: { index: number }) => `列${index}`,
        sticky: ({ index }: { index: number }) => `黏住${index}`,
      },
    });
    expect(wrapper.find(".virtual-list-sticky").exists()).toBe(false);
    const host = wrapper.get(".virtual-list").element as HTMLElement;
    host.scrollTop = 80;
    await wrapper.get(".virtual-list").trigger("scroll");
    expect(wrapper.get(".virtual-list-sticky").text()).toBe("黏住0");
    const sticky = wrapper.get(".virtual-list-sticky").element;
    expect(sticky.parentElement?.classList.contains("virtual-list-frame")).toBe(true);
    expect(wrapper.get(".virtual-list").element.contains(sticky)).toBe(false);
    expect(sticky.getAttribute("style") ?? "").not.toContain("margin-bottom");
  });
});

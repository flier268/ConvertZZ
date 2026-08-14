import { describe, expect, it } from "vitest";
import {
  clickModifier,
  dropButton,
  mouseSide,
  pointerIntent,
  quickActionKey,
} from "./floatingGestures";

describe("浮動球左右鍵意圖", () => {
  it("左鍵按下且沒有輔助鍵時只拖曳", () => {
    expect(pointerIntent("left", undefined, "down")).toEqual({ type: "drag" });
  });

  it("左鍵放開且沒有輔助鍵時不轉換", () => {
    expect(pointerIntent("left", undefined, "up")).toEqual({ type: "ignore" });
  });

  it("右鍵放開且沒有輔助鍵時開啟選單", () => {
    expect(pointerIntent("right", undefined, "up")).toEqual({ type: "context-menu" });
  });

  it("輔助鍵加左鍵或右鍵會對應設定中的快速動作", () => {
    expect(pointerIntent("left", "Ctrl", "up")).toEqual({
      type: "quick-action",
      button: "left",
      modifier: "Ctrl",
    });
    expect(pointerIntent("right", "Shift", "up")).toEqual({
      type: "quick-action",
      button: "right",
      modifier: "Shift",
    });
    expect(quickActionKey("left", "Click", "Ctrl")).toBe("leftClickCtrl");
    expect(quickActionKey("right", "Drop", "Alt")).toBe("rightDropAlt");
  });

  it("輔助鍵優先順序與舊版相同：Ctrl、Alt、Shift", () => {
    expect(clickModifier({ ctrlKey: true, altKey: true, shiftKey: true })).toBe("Ctrl");
    expect(clickModifier({ ctrlKey: false, altKey: true, shiftKey: true })).toBe("Alt");
    expect(clickModifier({ ctrlKey: false, altKey: false, shiftKey: true })).toBe("Shift");
  });

  it("滑鼠按鍵對應左鍵與右鍵", () => {
    expect(mouseSide(0)).toBe("left");
    expect(mouseSide(2)).toBe("right");
    expect(dropButton(2)).toBe("right");
    expect(dropButton(1)).toBe("left");
  });
});

import { describe, expect, it } from "vitest";
import { acceleratorFromKeyboardEvent } from "./hotkey";

function key(
  partial: Partial<{
    key: string;
    code: string;
    ctrlKey: boolean;
    metaKey: boolean;
    altKey: boolean;
    shiftKey: boolean;
  }>,
) {
  return {
    key: "a",
    code: "KeyA",
    ctrlKey: false,
    metaKey: false,
    altKey: false,
    shiftKey: false,
    ...partial,
  };
}

describe("快捷鍵錄製", () => {
  it("把組合鍵轉成 Tauri 加速鍵字串", () => {
    expect(
      acceleratorFromKeyboardEvent(key({ ctrlKey: true, shiftKey: true, key: "T", code: "KeyT" })),
    ).toBe("CommandOrControl+Shift+T");
    expect(acceleratorFromKeyboardEvent(key({ altKey: true, key: "F8", code: "F8" }))).toBe(
      "Alt+F8",
    );
    expect(acceleratorFromKeyboardEvent(key({ metaKey: true, key: "1", code: "Digit1" }))).toBe(
      "CommandOrControl+1",
    );
    expect(acceleratorFromKeyboardEvent(key({ key: "F2", code: "F2" }))).toBe("F2");
  });

  it("忽略單獨的修飾鍵與一般字母", () => {
    expect(
      acceleratorFromKeyboardEvent(key({ key: "Control", code: "ControlLeft", ctrlKey: true })),
    ).toBeUndefined();
    expect(acceleratorFromKeyboardEvent(key({ key: "t", code: "KeyT" }))).toBeUndefined();
  });

  it("Backspace 或 Delete 會清除快捷鍵", () => {
    expect(acceleratorFromKeyboardEvent(key({ key: "Backspace", code: "Backspace" }))).toBe(
      "clear",
    );
    expect(acceleratorFromKeyboardEvent(key({ key: "Delete", code: "Delete" }))).toBe("clear");
    expect(
      acceleratorFromKeyboardEvent(key({ key: "Backspace", code: "Backspace", ctrlKey: true })),
    ).not.toBe("clear");
  });
});

import { describe, expect, it } from "vitest";
import {
  acceleratorFromKeyboardEvent,
  acceleratorMainKey,
  assignShortcutAccelerator,
  registrableShortcuts,
  unregisteredAcceleratorWarnings,
} from "./hotkey";

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

  it("錄製組合鍵時自動啟用，清除時停用", () => {
    const shortcut = { accelerator: "", enabled: false };
    assignShortcutAccelerator(shortcut, "Alt+U");
    expect(shortcut).toEqual({ accelerator: "Alt+U", enabled: true });
    assignShortcutAccelerator(shortcut, "clear");
    expect(shortcut).toEqual({ accelerator: "", enabled: false });
    assignShortcutAccelerator(shortcut, undefined);
    expect(shortcut).toEqual({ accelerator: "", enabled: false });
  });

  it("只註冊已啟用且有按鍵的快捷鍵", () => {
    const shortcuts = [
      { enabled: true, accelerator: "Alt+U", action: "a4" },
      { enabled: false, accelerator: "Alt+I", action: "a3" },
      { enabled: true, accelerator: "", action: "a1" },
    ];
    expect(registrableShortcuts(shortcuts)).toEqual([
      { enabled: true, accelerator: "Alt+U", action: "a4" },
    ]);
    expect(unregisteredAcceleratorWarnings(shortcuts)).toEqual([
      "快捷鍵 Alt+I 已設定但未啟用，因此尚未註冊。",
    ]);
    expect(acceleratorMainKey("Alt+U")).toBe("U");
    expect(acceleratorMainKey("CommandOrControl+Shift+T")).toBe("T");
    expect(acceleratorMainKey("Alt")).toBeUndefined();
  });
});

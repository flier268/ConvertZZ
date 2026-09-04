import { describe, expect, it } from "vitest";
import { collectMenuActionIds, FLOATING_CONTEXT_MENU, resolveShellAction } from "./appMenu";

describe("浮動球右鍵選單", () => {
  it("包含舊版懸浮球右鍵的轉換與視窗動作", () => {
    expect(collectMenuActionIds(FLOATING_CONTEXT_MENU)).toEqual(
      expect.arrayContaining([
        "a1",
        "a2",
        "a3",
        "a4",
        "b1",
        "b2",
        "c1",
        "c2",
        "c3",
        "za1",
        "za2",
        "za3",
        "zb1",
        "zb2",
        "zb3",
        "zb4",
        "zb5",
        "zb6",
        "zc1",
        "zc2",
        "zc3",
        "zc4",
        "zd1",
        "zd2",
        "zd3",
        "zd4",
        "ze1",
        "ze2",
        "1",
        "settings",
        "about",
        "report",
        "quit",
      ]),
    );
  });

  it("文件、剪貼簿與音訊項目會開啟主視窗對應頁面", () => {
    expect(resolveShellAction("b1")).toEqual({ type: "navigate", page: "files" });
    expect(resolveShellAction("b2")).toEqual({ type: "navigate", page: "clipboard" });
    expect(resolveShellAction("c1")).toEqual({ type: "navigate", page: "audio" });
    expect(resolveShellAction("settings")).toEqual({ type: "navigate", page: "settings" });
    expect(resolveShellAction("about")).toEqual({ type: "navigate", page: "about" });
    expect(resolveShellAction("report")).toEqual({
      type: "open-url",
      url: "https://github.com/flier268/ConvertZZ/issues",
    });
    expect(resolveShellAction("quit")).toEqual({ type: "quit" });
    expect(resolveShellAction("a3")).toBeUndefined();
  });
});

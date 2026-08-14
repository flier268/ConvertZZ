import { describe, expect, it } from "vitest";
import { parseLegacyFileFilters } from "./fileFilters";

describe("舊版檔案篩選器", () => {
  it("將六欄設定中的篩選格式轉成 Tauri 選擇器格式", () => {
    expect(parseLegacyFileFilters("<文字|*.txt;*.log>/<網頁|*.html;*.htm>")).toEqual([
      { name: "文字", extensions: ["txt", "log"] },
      { name: "網頁", extensions: ["html", "htm"] },
    ]);
  });

  it("忽略無效與任意檔案片段", () => {
    expect(parseLegacyFileFilters("任意檔案(*.*)|*.*<圖片|.png;*.png>")).toEqual([
      { name: "圖片", extensions: ["png"] },
    ]);
  });
});

import { describe, expect, it } from "vitest";
import {
  DEFAULT_FILE_TYPE_FILTER,
  ensureSupportedFilesFilter,
  parseLegacyFileFilters,
  SUPPORTED_FILES_FILTER_NAME,
} from "./fileFilters";

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

  it("預設篩選字串只含分類，不含支援的檔案", () => {
    const filters = parseLegacyFileFilters(DEFAULT_FILE_TYPE_FILTER);
    expect(filters.map((filter) => filter.name)).toEqual([
      "常用文字檔案",
      "常用網頁文件",
      "音訊文件",
    ]);
    expect(ensureSupportedFilesFilter(filters)[0]?.name).toBe(SUPPORTED_FILES_FILTER_NAME);
  });

  it("在既有分類前插入支援的檔案聯集作為預設", () => {
    expect(
      ensureSupportedFilesFilter([
        { name: "文字", extensions: ["txt", "log"] },
        { name: "網頁", extensions: ["html", "HTML"] },
      ]),
    ).toEqual([
      { name: SUPPORTED_FILES_FILTER_NAME, extensions: ["txt", "log", "html"] },
      { name: "文字", extensions: ["txt", "log"] },
      { name: "網頁", extensions: ["html", "HTML"] },
    ]);
  });

  it("已有支援的檔案時以分類副檔名重建並維持在最前", () => {
    expect(
      ensureSupportedFilesFilter([
        { name: SUPPORTED_FILES_FILTER_NAME, extensions: ["txt"] },
        { name: "文字", extensions: ["txt", "log"] },
      ]),
    ).toEqual([
      { name: SUPPORTED_FILES_FILTER_NAME, extensions: ["txt", "log"] },
      { name: "文字", extensions: ["txt", "log"] },
    ]);
  });

  it("僅有支援的檔案時保留原清單", () => {
    const onlySupported = [{ name: SUPPORTED_FILES_FILTER_NAME, extensions: ["txt", "md"] }];
    expect(ensureSupportedFilesFilter(onlySupported)).toEqual(onlySupported);
  });
});

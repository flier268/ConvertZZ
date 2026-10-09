import { describe, expect, it } from "vitest";
import {
  ALL_FILES_EXTENSION,
  ALL_FILES_FILTER_NAME,
  ALL_FILES_TYPE_FILTER_GROUP,
  appendAllFilesTypeFilter,
  DEFAULT_FILE_TYPE_FILTER,
  dialogFileFilters,
  ensureSupportedFilesFilter,
  folderScanExtensions,
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

  it("保留自訂的 *.* 與 *，代表所有檔案", () => {
    expect(parseLegacyFileFilters("<所有檔案|*.*>")).toEqual([
      { name: "所有檔案", extensions: [ALL_FILES_EXTENSION] },
    ]);
    expect(parseLegacyFileFilters("<全部|*>")).toEqual([
      { name: "全部", extensions: [ALL_FILES_EXTENSION] },
    ]);
    expect(parseLegacyFileFilters("<混合|*.txt;*.*;*>")).toEqual([
      { name: "混合", extensions: ["txt", ALL_FILES_EXTENSION] },
    ]);
    expect(parseLegacyFileFilters("不是篩選")).toEqual([]);
  });

  it("篩選器字串可加入所有檔案，且不重複", () => {
    expect(appendAllFilesTypeFilter("<文字|*.txt>")).toBe(
      `<文字|*.txt>/${ALL_FILES_TYPE_FILTER_GROUP}`,
    );
    expect(appendAllFilesTypeFilter("")).toBe(ALL_FILES_TYPE_FILTER_GROUP);
    expect(appendAllFilesTypeFilter("<所有檔案|*.*>")).toBe("<所有檔案|*.*>");
    expect(appendAllFilesTypeFilter("<全部|*>")).toBe("<全部|*>");
  });

  it("預設篩選字串只含分類，不含支援的檔案", () => {
    const filters = parseLegacyFileFilters(DEFAULT_FILE_TYPE_FILTER);
    expect(filters.map((filter) => filter.name)).toEqual(["常用文字檔案", "常用網頁文件"]);
    expect(DEFAULT_FILE_TYPE_FILTER).not.toMatch(/mp3|flac|ogg|wav|m4a|ape|opus/iu);
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

  it("支援的檔案聯集不含所有檔案標記", () => {
    expect(
      ensureSupportedFilesFilter([
        { name: "文字", extensions: ["txt", ALL_FILES_EXTENSION] },
        { name: "全部", extensions: [ALL_FILES_EXTENSION] },
      ]),
    ).toEqual([
      { name: SUPPORTED_FILES_FILTER_NAME, extensions: ["txt"] },
      { name: "文字", extensions: ["txt", ALL_FILES_EXTENSION] },
      { name: "全部", extensions: [ALL_FILES_EXTENSION] },
    ]);
  });

  it("對話框在支援的檔案之後固定附上所有檔案", () => {
    const dialog = dialogFileFilters(parseLegacyFileFilters(DEFAULT_FILE_TYPE_FILTER));
    const supportedAt = dialog.findIndex((filter) => filter.name === SUPPORTED_FILES_FILTER_NAME);
    const allAt = dialog.findIndex((filter) => filter.name === ALL_FILES_FILTER_NAME);
    expect(supportedAt).toBeGreaterThanOrEqual(0);
    expect(allAt).toBeGreaterThan(supportedAt);
    expect(dialog[allAt]?.extensions).toEqual([ALL_FILES_EXTENSION]);
    expect(dialog[supportedAt]?.extensions).not.toContain(ALL_FILES_EXTENSION);
    expect(DEFAULT_FILE_TYPE_FILTER.includes("*.*")).toBe(false);
    expect(
      parseLegacyFileFilters(DEFAULT_FILE_TYPE_FILTER).every(
        (filter) => !filter.extensions.includes(ALL_FILES_EXTENSION),
      ),
    ).toBe(true);
  });

  it("自訂篩選已有所有檔案時不重複插入", () => {
    const dialog = dialogFileFilters([
      { name: "文字", extensions: ["txt"] },
      { name: ALL_FILES_FILTER_NAME, extensions: [ALL_FILES_EXTENSION] },
    ]);
    expect(dialog.filter((filter) => filter.name === ALL_FILES_FILTER_NAME)).toHaveLength(1);
    const supportedAt = dialog.findIndex((filter) => filter.name === SUPPORTED_FILES_FILTER_NAME);
    const allAt = dialog.findIndex((filter) => filter.name === ALL_FILES_FILTER_NAME);
    expect(allAt).toBeGreaterThan(supportedAt);
  });

  it("資料夾掃描把 * 當成所有檔案，空結果不是所有檔案", () => {
    expect(
      folderScanExtensions([
        { name: SUPPORTED_FILES_FILTER_NAME, extensions: ["txt", "log"] },
        { name: "文字", extensions: ["TXT"] },
      ]),
    ).toEqual({ kind: "list", extensions: [".txt", ".log"] });
    expect(
      folderScanExtensions([
        { name: SUPPORTED_FILES_FILTER_NAME, extensions: ["txt"] },
        { name: "全部", extensions: [ALL_FILES_EXTENSION] },
      ]),
    ).toEqual({ kind: "all" });
    expect(folderScanExtensions(parseLegacyFileFilters("<所有檔案|*.*>"))).toEqual({ kind: "all" });
    expect(folderScanExtensions(parseLegacyFileFilters("<全部|*>"))).toEqual({ kind: "all" });
    expect(folderScanExtensions(parseLegacyFileFilters("不是篩選"))).toEqual({
      kind: "list",
      extensions: [],
    });
    expect(folderScanExtensions([])).toEqual({ kind: "list", extensions: [] });
    expect(folderScanExtensions([{ name: "空", extensions: [] }])).toEqual({
      kind: "list",
      extensions: [],
    });
  });
});

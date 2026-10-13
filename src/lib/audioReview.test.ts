import { describe, expect, it } from "vitest";
import type { AudioTagFile } from "@shared/contracts";
import {
  applyBulkSelection,
  audioParentDir,
  audioWarningDetail,
  clearWarningSelection,
  audioReviewStats,
  audioSegmentCounts,
  replaceAudioFile,
  audioRowSection,
  buildAudioListRows,
  buildFieldDiffs,
  charDiff,
  sortAudioFilesWithinFolders,
  filterAudioFiles,
  selectionTotals,
  stepAudioRow,
  toggleFolderCollapsed,
  audioApplySelection,
  writableAudioPaths,
  settingsSummaryChips,
} from "./audioReview";

function file(partial: Partial<AudioTagFile> & Pick<AudioTagFile, "path">): AudioTagFile {
  return {
    format: "mp3",
    selected: true,
    hasCoverArt: false,
    fields: [],
    ...partial,
  };
}

const sample: AudioTagFile[] = [
  file({
    path: "/tmp/晨雾/风.mp3",
    fields: [
      {
        key: "title",
        label: "標題",
        container: "id3v2",
        values: ["风的方向"],
        convertedValues: ["風的方向"],
        selected: true,
      },
      {
        key: "album",
        label: "專輯",
        container: "id3v2",
        values: ["晨雾"],
        convertedValues: ["晨霧"],
        selected: true,
      },
      {
        key: "track",
        label: "曲目",
        container: "id3v2",
        values: ["1"],
        convertedValues: ["1"],
        selected: false,
      },
    ],
  }),
  file({
    path: "/tmp/same.ogg",
    format: "ogg",
    fields: [
      {
        key: "title",
        label: "標題",
        container: "vorbis-comment",
        values: ["同"],
        convertedValues: ["同"],
        selected: true,
      },
    ],
  }),
  file({
    path: "/tmp/broken.mp3",
    selected: false,
    warning: "無法讀取標籤",
    fields: [],
  }),
];

describe("audioReview", () => {
  it("統計有變更、不變與錯誤", () => {
    expect(audioReviewStats(sample)).toMatchObject({
      total: 3,
      changed: 1,
      unchanged: 1,
      errors: 1,
      listedChanged: 1,
    });
  });

  it("同一個篩選值決定區段筆數，搜尋再和它取交集", () => {
    const counts = audioSegmentCounts(sample);
    expect(counts).toEqual({ changed: 1, unchanged: 1, errors: 1, all: 3 });
    for (const filter of ["changed", "unchanged", "errors", "all"] as const) {
      expect(filterAudioFiles(sample, filter, "")).toHaveLength(counts[filter]);
    }
    expect(filterAudioFiles(sample, "changed", "晨霧").map((item) => item.path)).toEqual([
      "/tmp/晨雾/风.mp3",
    ]);
    expect(filterAudioFiles(sample, "unchanged", "晨霧")).toHaveLength(0);
    expect(filterAudioFiles(sample, "errors", "broken")).toHaveLength(1);
    expect(filterAudioFiles(sample, "errors", "晨霧")).toHaveLength(0);
  });

  it("重新掃描用同一個路徑換掉那一筆，區段筆數跟著變", () => {
    const updated = file({
      path: "/tmp/broken.mp3",
      selected: true,
      fields: [
        {
          key: "title",
          label: "標題",
          container: "id3v2",
          values: ["夜曲"],
          convertedValues: ["夜曲"],
          selected: true,
        },
      ],
    });
    const next = replaceAudioFile(sample, updated);
    expect(next).toHaveLength(3);
    expect(next[2].warning).toBeUndefined();
    expect(next[0].path).toBe("/tmp/晨雾/风.mp3");
    expect(audioSegmentCounts(next)).toEqual({ changed: 1, unchanged: 2, errors: 0, all: 3 });
    const added = replaceAudioFile(next, file({ path: "/tmp/new.mp3" }));
    expect(added.map((item) => item.path)).toContain("/tmp/new.mp3");
  });

  it("尚未建立預覽時，只看有變更仍列出可審檔案", () => {
    const pending = [
      file({
        path: "/tmp/song.mp3",
        fields: [
          { key: "title", label: "標題", container: "id3v2", values: ["里面"], selected: true },
        ],
      }),
      file({ path: "/tmp/bad.mp3", warning: "損壞", selected: false, fields: [] }),
    ];
    expect(audioReviewStats(pending).listedChanged).toBe(1);
    expect(filterAudioFiles(pending, "changed", "").map((item) => item.path)).toEqual([
      "/tmp/song.mp3",
    ]);
    expect(filterAudioFiles(pending, "errors", "")).toHaveLength(1);
    const counts = audioSegmentCounts(pending);
    expect(counts).toEqual({ changed: 1, unchanged: 0, errors: 1, all: 2 });
    for (const filter of ["changed", "unchanged", "errors", "all"] as const) {
      expect(filterAudioFiles(pending, filter, "")).toHaveLength(counts[filter]);
    }
    expect(audioReviewStats(pending)).toMatchObject({
      changed: counts.changed,
      unchanged: counts.unchanged,
      errors: counts.errors,
      listedChanged: counts.changed,
    });
  });

  it("搜尋檔名、專輯與標題，批次勾選只動到篩選結果", () => {
    expect(filterAudioFiles(sample, "all", "晨霧").map((item) => item.path)).toEqual([
      "/tmp/晨雾/风.mp3",
    ]);
    expect(filterAudioFiles(sample, "unchanged", "同")).toHaveLength(1);
    expect(filterAudioFiles(sample, "changed", "broken")).toHaveLength(0);

    applyBulkSelection(sample, filterAudioFiles(sample, "changed", ""), "none");
    expect(sample[0].selected).toBe(false);
    expect(sample[1].selected).toBe(true);
    applyBulkSelection(sample, filterAudioFiles(sample, "all", ""), "invert");
    expect(sample[0].selected).toBe(true);
    expect(sample[1].selected).toBe(false);
    expect(sample[2].selected).toBe(false);
    applyBulkSelection(sample, sample, "all");
    expect(sample[2].selected).toBe(false);
    expect(sample[1].selected).toBe(true);

    const forced = file({ path: "/tmp/broken.mp3", selected: true, warning: "截斷" });
    clearWarningSelection([forced]);
    expect(forced.selected).toBe(false);
    applyBulkSelection([forced], [forced], "all");
    expect(forced.selected).toBe(false);
    forced.selected = true;
    applyBulkSelection([forced], [forced], "invert");
    expect(forced.selected).toBe(false);
  });

  it("錯誤原因與標題相同時不重複", () => {
    expect(audioWarningDetail("無法讀取標籤")).toBeUndefined();
    expect(audioWarningDetail("  無法讀取標籤  ")).toBeUndefined();
    expect(audioWarningDetail("檔案已截斷")).toBe("檔案已截斷");
  });

  it("勾選計數只算會變更的欄位，並略過錯誤檔與未啟用的 ID3 容器", () => {
    sample[0].selected = true;
    sample[0].fields[0].selected = true;
    sample[0].fields[1].selected = true;
    sample[0].fields.push({
      key: "title",
      label: "標題",
      container: "id3v1",
      values: ["风"],
      convertedValues: ["風"],
      selected: true,
    });
    expect(selectionTotals(sample, { id3v1: true, id3v2: true })).toEqual({ files: 1, fields: 3 });
    expect(selectionTotals(sample, { id3v1: true, id3v2: false })).toEqual({ files: 1, fields: 1 });
    expect(writableAudioPaths(sample, { id3v1: true, id3v2: true })).toEqual(["/tmp/晨雾/风.mp3"]);
    expect(audioApplySelection(sample, { id3v1: true, id3v2: true })).toEqual({
      paths: ["/tmp/晨雾/风.mp3"],
      selectedFields: {
        "/tmp/晨雾/风.mp3": ["id3v2:title", "id3v2:album", "id3v1:title"],
      },
    });
    sample[0].fields[1].selected = false;
    expect(audioApplySelection(sample, { id3v1: true, id3v2: true }).selectedFields).toEqual({
      "/tmp/晨雾/风.mp3": ["id3v2:title", "id3v1:title"],
    });
    sample[0].selected = false;
    expect(audioApplySelection(sample, { id3v1: true, id3v2: true })).toEqual({
      paths: [],
      selectedFields: {},
    });
    const pending = [
      file({
        path: "/tmp/song.mp3",
        fields: [
          { key: "title", label: "標題", container: "id3v2", values: ["里面"], selected: true },
        ],
      }),
    ];
    expect(selectionTotals(pending, { id3v1: true, id3v2: true })).toEqual({ files: 1, fields: 1 });
  });

  it("只有編碼或 ID3 版本會變的檔案算進同一個分桶與確認筆數", () => {
    const encodingOnly = file({
      path: "/tmp/enc.mp3",
      rewriteContainers: ["id3v2"],
      fields: [
        {
          key: "title",
          label: "標題",
          container: "id3v2",
          values: ["Hello"],
          convertedValues: ["Hello"],
          selected: true,
        },
      ],
    });
    const id3v1 = file({
      path: "/tmp/v1.mp3",
      rewriteContainers: ["id3v1"],
      fields: [
        {
          key: "title",
          label: "標題",
          container: "id3v1",
          values: ["裡面"],
          convertedValues: ["裡面"],
          selected: true,
        },
      ],
    });
    const files = [encodingOnly, id3v1];
    expect(audioSegmentCounts(files)).toEqual({ changed: 2, unchanged: 0, errors: 0, all: 2 });
    expect(audioReviewStats(files)).toMatchObject({
      changed: 2,
      unchanged: 0,
      listedChanged: 2,
    });
    expect(filterAudioFiles(files, "changed", "")).toHaveLength(2);
    expect(selectionTotals(files, { id3v1: true, id3v2: true })).toEqual({ files: 2, fields: 2 });
    expect(audioApplySelection(files, { id3v1: true, id3v2: true }).paths).toEqual([
      "/tmp/enc.mp3",
      "/tmp/v1.mp3",
    ]);
    const rows = buildAudioListRows(files, new Set());
    expect(rows[0]).toMatchObject({ type: "folder", counts: { changed: 2 } });
    encodingOnly.fields[0].selected = false;
    expect(selectionTotals([encodingOnly], { id3v1: true, id3v2: true })).toEqual({
      files: 0,
      fields: 0,
    });
    expect(audioSegmentCounts([encodingOnly]).changed).toBe(1);
  });

  it("資料夾內排序不打亂資料夾出現順序", () => {
    const files = [
      file({ path: "/music/乙/b.mp3", format: "mp3" }),
      file({ path: "/music/甲/z.ogg", format: "ogg" }),
      file({
        path: "/music/乙/a.ape",
        format: "ape",
        fields: [
          {
            key: "title",
            label: "標題",
            container: "apev2",
            values: ["里"],
            convertedValues: ["裡"],
            selected: true,
          },
        ],
      }),
    ];
    expect(sortAudioFilesWithinFolders(files, "name").map((item) => item.path)).toEqual([
      "/music/乙/a.ape",
      "/music/乙/b.mp3",
      "/music/甲/z.ogg",
    ]);
    expect(sortAudioFilesWithinFolders(files, "format").map((item) => item.path)).toEqual([
      "/music/乙/a.ape",
      "/music/乙/b.mp3",
      "/music/甲/z.ogg",
    ]);
    expect(sortAudioFilesWithinFolders(files, "changed").map((item) => item.path)).toEqual([
      "/music/乙/a.ape",
      "/music/乙/b.mp3",
      "/music/甲/z.ogg",
    ]);
  });

  it("選取檔案的每個欄位只建立一份字元差異", () => {
    const diffs = buildFieldDiffs(
      file({
        path: "/tmp/one.mp3",
        fields: [
          {
            key: "title",
            label: "標題",
            container: "id3v2",
            values: ["风的方向"],
            convertedValues: ["風的方向"],
            selected: true,
          },
          {
            key: "track",
            label: "曲目",
            container: "id3v2",
            values: ["1"],
            selected: false,
          },
        ],
      }),
    );
    expect(diffs.size).toBe(1);
    const title = diffs.get("id3v2:title");
    expect(title?.after.map((piece) => piece.text).join("")).toBe("風的方向");
    expect(diffs.get("id3v2:title")).toBe(title);
    expect(buildFieldDiffs(undefined).size).toBe(0);
  });

  it("字元差異把舊字與新字分開", () => {
    const diff = charDiff("后来的我们", "後來的我們");
    expect(
      diff.before.filter((piece) => piece.kind === "remove").map((piece) => piece.text),
    ).toEqual(["后来", "们"]);
    expect(diff.after.filter((piece) => piece.kind === "add").map((piece) => piece.text)).toEqual([
      "後來",
      "們",
    ]);
    expect(charDiff("同", "同").before).toEqual([{ text: "同", kind: "same" }]);
  });

  it("依資料夾收成單一扁平列，摺疊時拿掉檔案列", () => {
    expect(audioParentDir("/music/專輯甲/song.mp3")).toBe("/music/專輯甲");
    expect(audioParentDir("C:\\music\\a.mp3")).toBe("C:/music");
    expect(audioParentDir("song.mp3")).toBe(".");
    const rows = buildAudioListRows(sample, new Set());
    expect(rows.map((row) => (row.type === "folder" ? row.path : row.file.path))).toEqual([
      "/tmp/晨雾",
      "/tmp/晨雾/风.mp3",
      "/tmp",
      "/tmp/same.ogg",
      "/tmp/broken.mp3",
    ]);
    const folder = rows[0];
    expect(folder).toMatchObject({
      type: "folder",
      counts: { files: 1, changed: 1, errors: 0 },
      collapsed: false,
    });
    const errors = rows[2];
    expect(errors).toMatchObject({
      type: "folder",
      path: "/tmp",
      counts: { files: 2, changed: 0, errors: 1 },
    });
    const collapsed = toggleFolderCollapsed(new Set(), "/tmp");
    const hidden = buildAudioListRows(sample, collapsed);
    expect(hidden.map((row) => row.type)).toEqual(["folder", "file", "folder"]);
    expect(audioRowSection(hidden, 1)).toBe(0);
    expect(audioRowSection(hidden, 2)).toBe(2);
    expect(stepAudioRow(hidden.length, 0, 1)).toBe(1);
    expect(stepAudioRow(hidden.length, -1, 1)).toBe(0);
    expect(stepAudioRow(hidden.length, -1, -1)).toBe(2);
    expect(stepAudioRow(0, -1, 1)).toBe(-1);
  });

  it("全選作用於全部篩選檔，不因資料夾摺疊而略過", () => {
    const files = sample.map((item) => ({ ...item, selected: false }));
    const filtered = filterAudioFiles(files, "all", "");
    const collapsed = buildAudioListRows(filtered, new Set(["/tmp"]));
    expect(collapsed.some((row) => row.type === "file" && row.file.path === "/tmp/same.ogg")).toBe(
      false,
    );
    applyBulkSelection(files, filtered, "all");
    expect(files.find((item) => item.path === "/tmp/same.ogg")?.selected).toBe(true);
    expect(files.find((item) => item.path === "/tmp/broken.mp3")?.selected).toBe(false);
  });

  it("設定摘要含 ID3、錯碼修復與備份", () => {
    expect(
      settingsSummaryChips({
        direction: "s2t",
        vocabularyCorrection: true,
        backup: true,
        hasMp3: true,
        id3v1Enabled: true,
        id3v1SourceEncoding: "gbk",
        id3v1OutputEncoding: "big5",
        id3v2Enabled: true,
        id3v2Direction: "s2t",
        id3v2RepairSourceEncoding: true,
        id3v2SourceEncoding: "gbk",
        id3v2Version: 4,
        id3v2Encoding: "utf8",
      }),
    ).toEqual([
      "ID3v2 簡→繁",
      "GBK 錯碼修復",
      "ID3v1 GBK→Big5",
      "APE/Vorbis 簡→繁",
      "詞彙修正",
      "備份 .bak",
      "ID3v2.4・UTF-8",
    ]);
  });
});

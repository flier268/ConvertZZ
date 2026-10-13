import { describe, expect, it } from "vitest";
import type { AudioTagFile } from "@shared/contracts";
import {
  applyBulkSelection,
  audioReviewStats,
  charDiff,
  filterAudioFiles,
  selectionTotals,
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
  });

  it("勾選計數略過錯誤檔與未啟用的 ID3 容器", () => {
    sample[0].selected = true;
    sample[0].fields[0].selected = true;
    sample[0].fields[1].selected = true;
    expect(selectionTotals(sample, { id3v1: true, id3v2: true })).toEqual({ files: 2, fields: 3 });
    expect(selectionTotals(sample, { id3v1: true, id3v2: false })).toEqual({ files: 1, fields: 1 });
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

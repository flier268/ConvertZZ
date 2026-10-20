import { describe, expect, it } from "vitest";
import { mergeAudioFiles } from "./audioReview";
import type { AudioTagFile } from "@shared/contracts";

function audio(path: string, title: string): AudioTagFile {
  return {
    path,
    format: "mp3",
    selected: true,
    fields: [{ key: "title", label: "標題", container: "id3v2", values: [title], selected: true }],
  };
}

describe("音訊清單增量合併", () => {
  it("後來的批次更新同一路徑，新路徑加在後面", () => {
    const first = mergeAudioFiles([], [audio("/a.mp3", "甲"), audio("/b.mp3", "乙")]);
    const next = mergeAudioFiles(first, [audio("/b.mp3", "乙繁"), audio("/c.mp3", "丙")]);
    expect(next.map((file) => file.path)).toEqual(["/a.mp3", "/b.mp3", "/c.mp3"]);
    expect(next[1]?.fields[0]?.values).toEqual(["乙繁"]);
  });
});

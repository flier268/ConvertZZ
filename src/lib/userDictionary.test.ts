import { describe, expect, it } from "vitest";
import type { LegacyImportRow, UserDictionaryEntry } from "@shared/contracts";
import { findConflicts, mergeLegacyRows } from "./userDictionary";

function row(
  simplified: string,
  traditional: string,
  direction: LegacyImportRow["direction"],
  status: LegacyImportRow["status"] = "importable",
): LegacyImportRow {
  return { line: 1, simplified, traditional, direction, status };
}

describe("使用者詞典匯入合併", () => {
  it("只併入可匯入的列，並略過表格已有的相同詞條", () => {
    const current: UserDictionaryEntry[] = [
      { simplified: "优盘", traditional: "隨身碟", direction: "s2t" },
    ];
    const result = mergeLegacyRows(current, [
      row("优盘", "隨身碟", "s2t"),
      row("信息", "資訊", "both"),
      row("里面", "裡面", "both", "invalid"),
    ]);
    expect(result.added).toBe(1);
    expect(result.existing).toBe(1);
    expect(result.entries).toEqual([
      { simplified: "优盘", traditional: "隨身碟", direction: "s2t" },
      { simplified: "信息", traditional: "資訊", direction: "both" },
    ]);
  });

  it("同方向同來源詞衝突時保留表格原有詞條", () => {
    const current: UserDictionaryEntry[] = [
      { simplified: "信息", traditional: "訊息", direction: "s2t" },
    ];
    const result = mergeLegacyRows(current, [row("信息", "資訊", "both")]);
    expect(result.conflicts).toBe(1);
    expect(result.added).toBe(0);
    expect(result.entries).toEqual(current);
  });

  it("不同方向的同字詞可以並存", () => {
    const current: UserDictionaryEntry[] = [
      { simplified: "信息", traditional: "訊息", direction: "s2t" },
    ];
    const result = mergeLegacyRows(current, [row("讯息", "訊息", "t2s")]);
    expect(result.added).toBe(1);
    expect(findConflicts(result.entries)).toEqual([]);
  });

  it("找出同方向同來源詞的衝突", () => {
    expect(
      findConflicts([
        { simplified: "信息", traditional: "訊息", direction: "both" },
        { simplified: "信息", traditional: "資訊", direction: "s2t" },
      ]),
    ).toEqual(["信息"]);
  });
});

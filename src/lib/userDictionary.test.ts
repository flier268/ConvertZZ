import { describe, expect, it } from "vitest";
import type { DictionaryImportRow, UserDictionaryEntry } from "@shared/contracts";
import { findConflicts, MANUAL_SOURCE, matchesFilter, mergeImport } from "./userDictionary";

function row(
  simplified: string,
  traditional: string,
  direction: DictionaryImportRow["direction"],
  status: DictionaryImportRow["status"] = "importable",
): DictionaryImportRow {
  return { line: 1, simplified, traditional, direction, status };
}

const stamp = { sourceFile: "新字典.txt", importedAt: "2026-11-17T12:00:00+08:00" };

describe("使用者詞典匯入合併", () => {
  it("只併入可匯入的列並標上匯入檔名與時間；表格已有的相同詞條改標成這次匯入", () => {
    const current: UserDictionaryEntry[] = [
      { simplified: "优盘", traditional: "隨身碟", direction: "s2t" },
      { simplified: "鼠标", traditional: "滑鼠", direction: "both" },
    ];
    const result = mergeImport(current, {
      ...stamp,
      rows: [
        row("优盘", "隨身碟", "s2t"),
        row("信息", "資訊", "both"),
        row("里面", "裡面", "both", "invalid"),
      ],
    });
    expect(result.added).toBe(1);
    expect(result.existing).toBe(1);
    expect(result.entries).toEqual([
      { simplified: "优盘", traditional: "隨身碟", direction: "s2t", ...stamp },
      { simplified: "鼠标", traditional: "滑鼠", direction: "both" },
      { simplified: "信息", traditional: "資訊", direction: "both", ...stamp },
    ]);
    expect(current[0]).not.toHaveProperty("sourceFile");
  });

  it("表格雙向詞條已涵蓋的單向列不重複加入", () => {
    const current: UserDictionaryEntry[] = [
      { simplified: "信息", traditional: "資訊", direction: "both" },
    ];
    const result = mergeImport(current, { ...stamp, rows: [row("信息", "資訊", "s2t")] });
    expect(result.added).toBe(0);
    expect(result.existing).toBe(1);
    expect(result.entries).toEqual(current);
  });

  it("同方向同來源詞衝突時保留表格原有詞條", () => {
    const current: UserDictionaryEntry[] = [
      { simplified: "信息", traditional: "訊息", direction: "s2t" },
    ];
    const result = mergeImport(current, { ...stamp, rows: [row("信息", "資訊", "both")] });
    expect(result.conflicts).toBe(1);
    expect(result.added).toBe(0);
    expect(result.entries).toEqual(current);
  });

  it("不同方向的同字詞可以並存", () => {
    const current: UserDictionaryEntry[] = [
      { simplified: "信息", traditional: "訊息", direction: "s2t" },
    ];
    const result = mergeImport(current, { ...stamp, rows: [row("讯息", "訊息", "t2s")] });
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

describe("使用者詞典篩選", () => {
  const manual: UserDictionaryEntry = {
    simplified: "软件",
    traditional: "軟體",
    direction: "both",
  };
  const imported: UserDictionaryEntry = {
    simplified: "信息",
    traditional: "資訊",
    direction: "s2t",
    ...stamp,
  };

  it("依關鍵字、方向與來源篩選", () => {
    const all = { keyword: "", direction: "", source: "" } as const;
    expect(matchesFilter(manual, { ...all, keyword: "軟" })).toBe(true);
    expect(matchesFilter(manual, { ...all, keyword: "資" })).toBe(false);
    expect(matchesFilter(imported, { ...all, direction: "s2t" })).toBe(true);
    expect(matchesFilter(manual, { ...all, direction: "s2t" })).toBe(false);
    expect(matchesFilter(manual, { ...all, source: MANUAL_SOURCE })).toBe(true);
    expect(matchesFilter(imported, { ...all, source: MANUAL_SOURCE })).toBe(false);
    expect(matchesFilter(imported, { ...all, source: "新字典.txt" })).toBe(true);
    expect(matchesFilter(manual, { ...all, source: "新字典.txt" })).toBe(false);
  });
});

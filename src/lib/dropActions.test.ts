import { describe, expect, it } from "vitest";
import {
  buildDropCliInvocation,
  dropTargetPage,
  normalizeDropActionChoice,
  summarizeDropPaths,
} from "./dropActions";

describe("dropActions", () => {
  it("正規化並保留上次拖放選項", () => {
    expect(
      normalizeDropActionChoice({
        kind: "audio",
        operation: "both",
        direction: "t2s",
      }),
    ).toEqual({ kind: "audio", operation: "both", direction: "t2s" });
    expect(normalizeDropActionChoice(undefined, "t2s")).toEqual({
      kind: "file",
      operation: "content",
      direction: "t2s",
    });
    expect(normalizeDropActionChoice({ kind: "nope" as "file" }, "none")).toEqual({
      kind: "file",
      operation: "content",
      direction: "s2t",
    });
  });

  it("組出檔案轉換的 ParsedCli", () => {
    expect(
      buildDropCliInvocation(
        ["/tmp/a.txt", "/tmp/b.txt"],
        { kind: "file", operation: "filename", direction: "t2s" },
        { engine: "legacy", autoBackupBeforeConversion: false },
      ),
    ).toEqual({
      mode: "file",
      paths: ["/tmp/a.txt", "/tmp/b.txt"],
      inputEncoding: "auto",
      outputEncoding: "auto",
      direction: "t2s",
      engine: "legacy",
      operation: "filename",
      vocabularyCorrection: "settings",
      backup: false,
      headless: false,
      confirmWrite: false,
      outputEncodingExplicit: false,
      inputEncodingExplicit: false,
      directionExplicit: true,
      engineExplicit: false,
      vocabularyExplicit: false,
      backupExplicit: false,
      useGlobalConfig: false,
      parseErrors: [],
    });
  });

  it("組出音訊標籤的 ParsedCli", () => {
    const parsed = buildDropCliInvocation(
      ["/music/song.mp3"],
      { kind: "audio", operation: "content", direction: "s2t" },
      { engine: "segmented", autoBackupBeforeConversion: true },
    );
    expect(parsed.mode).toBe("audio");
    expect(parsed.paths).toEqual(["/music/song.mp3"]);
    expect(parsed.direction).toBe("s2t");
    expect(parsed.backup).toBe(true);
    expect(dropTargetPage("audio")).toBe("audio");
    expect(dropTargetPage("file")).toBe("files");
  });

  it("摘要過多路徑時會截斷", () => {
    expect(summarizeDropPaths([])).toBe("未選取檔案");
    expect(summarizeDropPaths(["/a/一.txt", "/b/二.txt"])).toBe("一.txt、二.txt");
    expect(summarizeDropPaths(["/a/1.txt", "/b/2.txt", "/c/3.txt", "/d/4.txt"])).toBe(
      "1.txt、2.txt、3.txt 等 4 項",
    );
  });
});

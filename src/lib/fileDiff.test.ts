import { describe, expect, it } from "vitest";
import type { FilePlanItem } from "@shared/contracts";
import { baseName, buildFileDiffSections } from "./fileDiff";

function item(partial: Partial<FilePlanItem>): FilePlanItem {
  return {
    sourcePath: "/tmp/里面.txt",
    outputPath: "/tmp/里面.txt",
    kind: "file",
    selected: true,
    sourcePreview: "",
    outputPreview: "",
    status: "ready",
    ...partial,
  };
}

describe("fileDiff", () => {
  it("取出路徑檔名", () => {
    expect(baseName("/tmp/里面.txt")).toBe("里面.txt");
    expect(baseName(String.raw`C:\Docs\頭髮.mp3`)).toBe("頭髮.mp3");
  });

  it("純檔名作業只顯示檔名差異", () => {
    expect(
      buildFileDiffSections(
        item({
          outputPath: "/tmp/裡面.txt",
          kind: "file",
          sourcePreview: "里面.txt",
          outputPreview: "裡面.txt",
        }),
      ),
    ).toEqual([
      {
        title: "檔名",
        sourceLabel: "來源檔名",
        outputLabel: "輸出檔名",
        source: "里面.txt",
        output: "裡面.txt",
      },
    ]);
  });

  it("純內容作業只顯示內容差異", () => {
    expect(
      buildFileDiffSections(
        item({
          sourcePreview: "里面开发",
          outputPreview: "裡面開發",
        }),
      ),
    ).toEqual([
      {
        title: "內容",
        sourceLabel: "來源預覽",
        outputLabel: "輸出預覽",
        source: "里面开发",
        output: "裡面開發",
      },
    ]);
  });

  it("內容與檔名作業同時顯示兩段差異", () => {
    expect(
      buildFileDiffSections(
        item({
          outputPath: "/tmp/裡面.txt",
          sourcePreview: "里面开发",
          outputPreview: "裡面開發",
        }),
      ),
    ).toEqual([
      {
        title: "檔名",
        sourceLabel: "來源檔名",
        outputLabel: "輸出檔名",
        source: "里面.txt",
        output: "裡面.txt",
      },
      {
        title: "內容",
        sourceLabel: "來源預覽",
        outputLabel: "輸出預覽",
        source: "里面开发",
        output: "裡面開發",
      },
    ]);
  });
});

import { describe, expect, it } from "vitest";
import { parseLegacyCli } from "./cli.js";

describe("舊版命令列", () => {
  it("保留既有參數並加入新引擎", () => {
    expect(
      parseLegacyCli(["/file", "/i:gbk", "/o:big5", "/f:t", "/d:t", "/e:n", "book.txt"]),
    ).toMatchObject({
      mode: "file",
      inputEncoding: "gbk",
      outputEncoding: "big5",
      operation: "content",
      direction: "s2t",
      engine: "segmented",
      vocabularyCorrection: "enabled",
      paths: ["book.txt"],
    });
  });

  it("辨識音訊與舊版引擎", () => {
    expect(parseLegacyCli(["/audio", "/e:l", "song.ape"])).toMatchObject({
      mode: "audio",
      engine: "legacy",
      paths: ["song.ape"],
    });
    expect(parseLegacyCli(["/audio", "a.mp3", "b.ape", "c.ogg", "d.opus"]).paths).toEqual([
      "a.mp3",
      "b.ape",
      "c.ogg",
      "d.opus",
    ]);
  });

  it.each([
    ["/e:l", "legacy"],
    ["/e:f", "zhconvert"],
    ["/e:n", "segmented"],
  ] as const)("把 %s 對應至 %s 引擎", (argument, engine) => {
    expect(parseLegacyCli([argument, "book.txt"]).engine).toBe(engine);
  });

  it("未指定引擎時沿用保存的引擎", () => {
    expect(parseLegacyCli(["book.txt"], "legacy").engine).toBe("legacy");
  });

  it("保留繁轉簡與停用字典參數", () => {
    expect(parseLegacyCli(["/f:s", "/d:f", "book.txt"])).toMatchObject({
      mode: "file",
      direction: "t2s",
      vocabularyCorrection: "disabled",
      paths: ["book.txt"],
    });
  });

  it("保留舊版的輸入與輸出路徑語意", () => {
    expect(parseLegacyCli(["/f:t", "books/*.txt", "converted/*.txt"])).toMatchObject({
      mode: "file",
      paths: ["books/*.txt"],
      outputPath: "converted/*.txt",
    });
  });

  it("明確的檔案模式會接受多個來源路徑", () => {
    expect(parseLegacyCli(["/file", "a.txt", "b.txt"])).toMatchObject({
      mode: "file",
      paths: ["a.txt", "b.txt"],
    });
  });

  it("命令列路徑保留原始字串供檔案作業使用", () => {
    expect(
      parseLegacyCli([
        "/file",
        "\\\\?\\C:\\Temp\\里面.txt",
        "\\\\?\\UNC\\server\\share\\converted.txt",
      ]),
    ).toMatchObject({
      mode: "file",
      paths: ["\\\\?\\C:\\Temp\\里面.txt", "\\\\?\\UNC\\server\\share\\converted.txt"],
    });
  });
});

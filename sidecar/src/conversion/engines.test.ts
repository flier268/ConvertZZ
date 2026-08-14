import { beforeAll, describe, expect, it } from "vitest";
import { resolve } from "node:path";
import { ConversionService } from "./engines.js";

describe("ConversionService", () => {
  let service: ConversionService;

  beforeAll(() => {
    service = new ConversionService(resolve("ConvertZZ/Dictionary.csv"));
  });

  it.each([
    ["里面", "裡面"],
    ["皇后", "皇后"],
    ["头发", "頭髮"],
    ["开发", "開發"],
    ["面对表面", "面對表面"],
  ])("以語意模式把 %s 轉為 %s", async (source, expected) => {
    const result = await service.convert({ text: source, direction: "s2t", engine: "segmented" });
    expect(result.text).toBe(expected);
  });

  it.each([
    ["裡面", "里面"],
    ["皇后", "皇后"],
    ["頭髮", "头发"],
    ["開發", "开发"],
  ])("以語意模式把 %s 轉為 %s", async (source, expected) => {
    const result = await service.convert({ text: source, direction: "t2s", engine: "segmented" });
    expect(result.text).toBe(expected);
  });

  it("保留換行與空白", async () => {
    const source = "里面  开发\n头发";
    const result = await service.convert({ text: source, direction: "s2t", engine: "segmented" });
    expect(result.text).toBe("裡面  開發\n頭髮");
  });

  it("保留標點與不可辨識片段", async () => {
    const result = await service.convert({
      text: "里面  😀\n《A》",
      direction: "s2t",
      engine: "segmented",
    });
    expect(result.text).toBe("裡面  😀\n《A》");
  });

  it("長文字分段不會切斷代理字元", async () => {
    const source = `${"里".repeat(131_071)}😀里面`;
    const result = await service.convert({ text: source, direction: "s2t", engine: "segmented" });
    expect(result.text.endsWith("😀裡面")).toBe(true);
    expect(result.text).not.toContain("�");
  }, 15_000);

  it("沿用舊版字典", async () => {
    const result = await service.convert({
      text: "软件和头发",
      direction: "s2t",
      engine: "legacy",
    });
    expect(result.text).toContain("軟體");
    expect(result.text).toContain("頭髮");
  });

  it("停用詞彙修正時只執行字形轉換", async () => {
    const result = await service.convert({
      text: "里面",
      direction: "s2t",
      engine: "segmented",
      vocabularyCorrection: false,
    });
    expect(result.text).toBe("里麵");
    expect(result.warnings[0]).toContain("詞彙修正已停用");
  });
});

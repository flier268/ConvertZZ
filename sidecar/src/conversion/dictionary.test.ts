import { afterEach, describe, expect, it } from "vitest";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { LegacyDictionary, readDictionaryEntries } from "./dictionary.js";

const temporary: string[] = [];

afterEach(async () =>
  Promise.all(temporary.splice(0).map((path) => rm(path, { recursive: true, force: true }))),
);

describe("舊版字典規則", () => {
  it("同優先權使用長詞", async () => {
    const dictionary = await load([row("开发", 10, "短詞", 10), row("开发者", 10, "長詞", 10)]);
    expect(dictionary.replace("开发者", "s2t", (value) => value)).toBe("長詞");
  });

  it("較高優先權先於較長詞", async () => {
    const dictionary = await load([row("开发", 100, "優先", 100), row("开发者", 10, "長詞", 10)]);
    expect(dictionary.replace("开发者", "s2t", (value) => value)).toBe("優先者");
  });

  it("9999 保護詞不交給字形轉換", async () => {
    const dictionary = await load([row("皇后", 9999, "皇后", 9999)]);
    expect(dictionary.replace("皇后", "s2t", (value) => value.replaceAll("后", "後"))).toBe("皇后");
  });

  it("停用項目會由基礎轉換處理", async () => {
    const dictionary = await load([row("软件", 100, "軟體", 100, false)]);
    expect(dictionary.replace("软件", "s2t", () => "軟件")).toBe("軟件");
  });

  it("解析時保留空白行後方項目的原始索引", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-dictionary-index-"));
    temporary.push(directory);
    const path = join(directory, "Dictionary.csv");
    await writeFile(path, `\uFEFF${row("一", 1, "壹", 1)}\n\n${row("二", 2, "貳", 2)}\n`, "utf8");

    const entries = await readDictionaryEntries(path);
    expect(entries.map((entry) => entry.index)).toEqual([0, 2]);
  });
});

async function load(rows: string[]): Promise<LegacyDictionary> {
  const directory = await mkdtemp(join(tmpdir(), "convertzz-dictionary-"));
  temporary.push(directory);
  const path = join(directory, "Dictionary.csv");
  await writeFile(path, `\uFEFF${rows.join("\r\n")}\r\n`, "utf8");
  return LegacyDictionary.load(path);
}

function row(
  simplified: string,
  simplifiedPriority: number,
  traditional: string,
  traditionalPriority: number,
  enabled = true,
): string {
  return `${enabled ? "True" : "False"}\tTest\t${simplified}\t${simplifiedPriority}\t${traditional}\t${traditionalPriority}`;
}

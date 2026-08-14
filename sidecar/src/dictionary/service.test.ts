import { afterEach, describe, expect, it } from "vitest";
import { mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { ConversionService } from "../conversion/engines.js";
import { DictionaryService } from "./service.js";

const temporary: string[] = [];
afterEach(async () => Promise.all(temporary.splice(0).map((path) => rm(path, { recursive: true, force: true }))));

describe("字典管理", () => {
  it("連續儲存會各自建立不覆蓋的備份", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-dictionary-"));
    temporary.push(directory);
    const path = join(directory, "Dictionary.csv");
    const original = "\uFEFFtrue\t一般\t里面\t1\t裡面\t1\n";
    await writeFile(path, original);
    const service = new DictionaryService(path);
    const conversion = new ConversionService(path);
    expect((await conversion.convert({ text: "里面", direction: "s2t", engine: "legacy" })).text).toBe("裡面");
    const first = await service.update({
      path,
      updates: [{
        index: 0,
        entry: {
          enabled: true,
          type: "一般",
          simplified: "里面",
          simplifiedPriority: 2,
          traditional: "內部",
          traditionalPriority: 2,
        },
      }],
    });
    const afterFirstUpdate = await readFile(path, "utf8");
    const second = await service.update({
      path,
      updates: [{
        index: 0,
        entry: {
          enabled: true,
          type: "一般",
          simplified: "里面",
          simplifiedPriority: 3,
          traditional: "裏邊",
          traditionalPriority: 3,
        },
      }],
    });

    expect(first.backupPath).not.toBe(second.backupPath);
    expect(await readFile(first.backupPath, "utf8")).toBe(original);
    expect(await readFile(second.backupPath, "utf8")).toBe(afterFirstUpdate);
    expect((await readdir(directory)).filter((name) => name.includes(".backup-"))).toHaveLength(2);
    expect(await readFile(path, "utf8")).toContain("\t3\t裏邊\t3");
    expect((await conversion.convert({ text: "里面", direction: "s2t", engine: "legacy" })).text).toBe("裏邊");
    expect((await readdir(directory)).filter((name) => name.startsWith(".convertzz-"))).toEqual([]);
  });

  it("支援新增、刪除、排序與尚未儲存的預覽", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-dictionary-changes-"));
    temporary.push(directory);
    const path = join(directory, "Dictionary.csv");
    await writeFile(path, [
      "\uFEFFtrue\t一般\t专案\t10\t舊專案\t10",
      "true\t一般\t开发\t20\t舊開發\t20",
    ].join("\n"));
    const service = new DictionaryService(path);
    const inserted = {
      enabled: true,
      type: "新增",
      simplified: "开发者",
      simplifiedPriority: 100,
      traditional: "新開發者",
      traditionalPriority: 100,
    };

    const sorted = await service.read({ sort: "s2t" });
    expect(sorted.entries.map((entry) => entry.simplified)).toEqual(["开发", "专案"]);
    const preview = await service.preview({
      text: "专案开发者",
      direction: "s2t",
      deletes: [0],
      inserts: [inserted],
    });
    expect(preview.text).toBe("專案新開發者");

    const updated = await service.update({ path, deletes: [0], inserts: [inserted] });
    expect(updated.updated).toBe(2);
    const saved = await service.read({ sort: "s2t" });
    expect(saved.entries.map((entry) => entry.simplified)).toEqual(["开发者", "开发"]);
    expect(await readFile(path, "utf8")).not.toContain("舊專案");
    expect(await readFile(path, "utf8")).toContain("新開發者");
  });

  it("以原始行號更新空白行後方的項目", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-dictionary-index-"));
    temporary.push(directory);
    const path = join(directory, "Dictionary.csv");
    await writeFile(path, "\uFEFFtrue\t一般\t一\t1\t壹\t1\n\ntrue\t一般\t二\t2\t貳\t2\n");
    const service = new DictionaryService(path);
    const before = await service.read({});
    expect(before.entries.map((entry) => entry.index)).toEqual([0, 2]);

    await service.update({
      path,
      updates: [{
        index: 2,
        entry: {
          enabled: true,
          type: "一般",
          simplified: "二",
          simplifiedPriority: 3,
          traditional: "兩",
          traditionalPriority: 3,
        },
      }],
    });
    const lines = (await readFile(path, "utf8")).replace(/^\uFEFF/u, "").split("\n");
    expect(lines[1]).toBe("");
    expect(lines[2]).toContain("\t二\t3\t兩\t3");
  });
});

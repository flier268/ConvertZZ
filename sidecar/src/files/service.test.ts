import { afterEach, describe, expect, it } from "vitest";
import { readdirSync, rmSync } from "node:fs";
import { chmod, mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { ConversionService } from "../conversion/engines.js";
import { FileService } from "./service.js";
import { encodeText } from "../encoding/codecs.js";

const temporary: string[] = [];
afterEach(async () => Promise.all(temporary.splice(0).map((path) => rm(path, { recursive: true, force: true }))));

describe("檔案轉換", () => {
  it("先預覽再安全寫入並修正 charset", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-files-"));
    temporary.push(directory);
    const path = join(directory, "里面.html");
    await writeFile(path, encodeText('<meta charset="gbk">里面开发', "gbk"));
    const service = new FileService(new ConversionService(resolve("ConvertZZ/Dictionary.csv")));
    const plan = await service.plan({
      paths: [path],
      mode: "content",
      recursive: false,
      inputEncoding: "auto",
      outputEncoding: "utf8",
      addBom: false,
      fixCharsetDeclaration: true,
      conflictPolicy: "skip",
      conversion: { direction: "s2t", engine: "segmented" },
    });
    expect(plan.items[0].sourcePreview).toContain("里面开发");
    expect(plan.items[0].outputPreview).toContain("裡面開發");
    const result = await service.apply(plan.planId);
    expect(result.failed).toEqual([]);
    expect(await readFile(path, "utf8")).toContain('<meta charset="utf-8">裡面開發');
  });

  it("預設略過同名衝突", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-conflict-"));
    temporary.push(directory);
    const source = join(directory, "里面.txt");
    const output = join(directory, "裡面.txt");
    await writeFile(source, "來源");
    await writeFile(output, "既有目標");
    const service = new FileService(new ConversionService(resolve("ConvertZZ/Dictionary.csv")));
    const plan = await service.plan(fileNameRequest(source, "skip"));
    expect(plan.items[0].status).toBe("conflict");
    const result = await service.apply(plan.planId);
    expect(result.skipped).toEqual([source]);
    expect(await readFile(source, "utf8")).toBe("來源");
    expect(await readFile(output, "utf8")).toBe("既有目標");
  });

  it("取消後不允許執行舊計畫", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-cancel-"));
    temporary.push(directory);
    const source = join(directory, "里面.txt");
    await writeFile(source, "來源");
    const service = new FileService(new ConversionService(resolve("ConvertZZ/Dictionary.csv")));
    const plan = await service.plan(fileNameRequest(source, "skip"));
    expect(service.cancel(plan.planId)).toEqual({ cancelled: true });
    await expect(service.apply(plan.planId)).rejects.toMatchObject({ code: "PLAN_NOT_FOUND" });
    expect(await readFile(source, "utf8")).toBe("來源");
  });

  it("覆寫同名目標後清除交易暫存檔", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-overwrite-"));
    temporary.push(directory);
    const source = join(directory, "里面.txt");
    const output = join(directory, "裡面.txt");
    await writeFile(source, "來源");
    await writeFile(output, "既有目標");
    const service = new FileService(new ConversionService(resolve("ConvertZZ/Dictionary.csv")));
    const plan = await service.plan(fileNameRequest(source, "overwrite"));
    const result = await service.apply(plan.planId);
    expect(result.failed).toEqual([]);
    expect(result.succeeded).toEqual([output]);
    expect(await readFile(output, "utf8")).toBe("來源");
    expect((await readdir(directory)).filter((name) => name.startsWith(".convertzz-"))).toEqual([]);
  });

  it("展開舊版萬用字元並套用輸出路徑", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-wildcard-"));
    temporary.push(directory);
    const sourceDirectory = join(directory, "source");
    const outputDirectory = join(directory, "output");
    await mkdir(sourceDirectory);
    await writeFile(join(sourceDirectory, "one.txt"), "里面开发");
    await writeFile(join(sourceDirectory, "two.log"), "不会选取");
    const service = new FileService(new ConversionService(resolve("ConvertZZ/Dictionary.csv")));
    const plan = await service.plan({
      paths: [join(sourceDirectory, "*.txt")],
      outputPath: join(outputDirectory, "*.txt"),
      mode: "content",
      recursive: false,
      inputEncoding: "utf8",
      outputEncoding: "utf8",
      addBom: false,
      fixCharsetDeclaration: false,
      conflictPolicy: "skip",
      conversion: { direction: "s2t", engine: "segmented" },
    });
    expect(plan.items).toHaveLength(1);
    expect(plan.items[0].outputPath).toBe(join(outputDirectory, "one.txt"));
    const result = await service.apply(plan.planId);
    expect(result.failed).toEqual([]);
    expect(await readFile(join(outputDirectory, "one.txt"), "utf8")).toBe("裡面開發");
  });

  it("拒絕數量不同的舊版萬用字元", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-wildcard-error-"));
    temporary.push(directory);
    const service = new FileService(new ConversionService(resolve("ConvertZZ/Dictionary.csv")));
    await expect(service.plan({
      paths: [join(directory, "*.txt")],
      outputPath: join(directory, "*.*.txt"),
      mode: "content",
      recursive: false,
      inputEncoding: "utf8",
      outputEncoding: "utf8",
      addBom: false,
      fixCharsetDeclaration: false,
      conflictPolicy: "skip",
      conversion: { direction: "s2t", engine: "segmented" },
    })).rejects.toMatchObject({ code: "CLI_WILDCARD" });
  });

  it("遞迴轉換時會重新命名資料夾並保留其中檔案", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-directory-rename-"));
    temporary.push(directory);
    const sourceDirectory = join(directory, "里面资料");
    const sourceFile = join(sourceDirectory, "开发.txt");
    await mkdir(sourceDirectory);
    await writeFile(sourceFile, "內容");
    const service = new FileService(new ConversionService(resolve("ConvertZZ/Dictionary.csv")));
    const plan = await service.plan({
      ...fileNameRequest(directory, "skip"),
      recursive: true,
      allowedExtensions: ["txt"],
    });

    expect(plan.items).toEqual(expect.arrayContaining([
      expect.objectContaining({ sourcePath: sourceDirectory, outputPath: join(directory, "裡面資料"), kind: "directory" }),
      expect.objectContaining({ sourcePath: sourceFile, outputPath: join(sourceDirectory, "開發.txt"), kind: "file" }),
    ]));

    const result = await service.apply(plan.planId);
    expect(result.failed).toEqual([]);
    expect(result.succeeded).toContain(join(directory, "裡面資料", "開發.txt"));
    expect(result.succeeded).toContain(join(directory, "裡面資料"));
    expect(await readFile(join(directory, "裡面資料", "開發.txt"), "utf8")).toBe("內容");
    expect(await readdir(directory)).toEqual(["裡面資料"]);
  });

  it("資料夾輸入只收集篩選器允許的副檔名", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-directory-filter-"));
    temporary.push(directory);
    const nested = join(directory, "nested");
    await mkdir(nested);
    await Promise.all([
      writeFile(join(directory, "one.txt"), "一"),
      writeFile(join(directory, "two.log"), "二"),
      writeFile(join(nested, "three.TXT"), "三"),
      writeFile(join(nested, "four.md"), "四"),
    ]);
    const service = new FileService(new ConversionService(resolve("ConvertZZ/Dictionary.csv")));
    const plan = await service.plan({
      paths: [directory],
      mode: "content",
      recursive: true,
      allowedExtensions: [".txt"],
      inputEncoding: "utf8",
      outputEncoding: "utf8",
      addBom: false,
      fixCharsetDeclaration: false,
      conflictPolicy: "skip",
      conversion: { direction: "none", engine: "segmented" },
    });

    expect(plan.items.map((item) => item.sourcePath)).toEqual([
      join(directory, "one.txt"),
      join(nested, "three.TXT"),
    ].sort());
  });

  it.runIf(process.platform !== "win32")("遞迴處理不跟隨符號連結", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-symlink-root-"));
    const outside = await mkdtemp(join(tmpdir(), "convertzz-symlink-outside-"));
    temporary.push(directory, outside);
    await writeFile(join(directory, "inside.txt"), "內部");
    await writeFile(join(outside, "outside.txt"), "外部");
    await symlink(outside, join(directory, "linked-directory"), "dir");
    const service = new FileService(new ConversionService(resolve("ConvertZZ/Dictionary.csv")));
    const plan = await service.plan({
      ...fileNameRequest(directory, "skip"),
      recursive: true,
      allowedExtensions: ["txt"],
    });

    expect(plan.items.map((item) => item.sourcePath)).toEqual([join(directory, "inside.txt")]);
    expect(plan.items.some((item) => item.sourcePath.startsWith(outside))).toBe(false);
  });

  it("第二階段失敗時回復全部來源與已提交項目", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-two-phase-rollback-"));
    temporary.push(directory);
    const sources = [join(directory, "里面一.txt"), join(directory, "里面二.txt")];
    await writeFile(sources[0], "來源一");
    await writeFile(sources[1], "來源二");
    const service = new FileService(new ConversionService(resolve("ConvertZZ/Dictionary.csv")));
    const plan = await service.plan({
      ...fileNameRequest(directory, "overwrite"),
      recursive: false,
    });
    const outputs = plan.items.map((item) => item.outputPath);
    let removedRemainingStage = false;

    const result = await service.apply(plan.planId, (progress) => {
      if (removedRemainingStage || !progress.message.startsWith("正在寫入：")) return;
      const remainingStage = readdirSync(directory).find((name) => name.startsWith(".convertzz-stage-"));
      if (!remainingStage) return;
      rmSync(join(directory, remainingStage));
      removedRemainingStage = true;
    });

    expect(removedRemainingStage).toBe(true);
    expect(result.failed).toHaveLength(1);
    expect(await readFile(sources[0], "utf8")).toBe("來源一");
    expect(await readFile(sources[1], "utf8")).toBe("來源二");
    expect(await readdir(directory)).toEqual(["里面一.txt", "里面二.txt"]);
    expect(outputs.every((path) => !readdirSync(directory).includes(path.split(/[\\/]/u).at(-1) ?? ""))).toBe(true);
  });

  it("名稱互換時以兩階段重新命名保留兩份內容", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-swap-"));
    temporary.push(directory);
    const first = join(directory, "甲.txt");
    const second = join(directory, "乙.txt");
    await writeFile(first, "甲的內容");
    await writeFile(second, "乙的內容");
    const service = new FileService({
      convert: async (request) => ({
        text: ({ "甲.txt": "乙.txt", "乙.txt": "甲.txt" }[request.text] ?? request.text),
        engine: request.engine,
        direction: request.direction,
        warnings: [],
        durationMs: 0,
      }),
    } as ConversionService);
    const plan = await service.plan(fileNameRequest(directory, "overwrite"));

    const result = await service.apply(plan.planId);

    expect(result.failed).toEqual([]);
    expect(await readFile(first, "utf8")).toBe("乙的內容");
    expect(await readFile(second, "utf8")).toBe("甲的內容");
    expect(new Set(await readdir(directory))).toEqual(new Set(["甲.txt", "乙.txt"]));
  });

  it("暫存檔驗證失敗時不會取代原檔", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-verify-failure-"));
    temporary.push(directory);
    const source = join(directory, "里面.txt");
    await writeFile(source, "來源內容");
    const service = new FileService(
      new ConversionService(resolve("ConvertZZ/Dictionary.csv")),
      async () => { throw new Error("受控驗證失敗"); },
    );
    const plan = await service.plan(fileNameRequest(source, "overwrite"));

    const result = await service.apply(plan.planId);

    expect(result.failed).toEqual([expect.objectContaining({ path: "批次作業", message: "受控驗證失敗" })]);
    expect(await readFile(source, "utf8")).toBe("來源內容");
    expect((await readdir(directory)).filter((name) => name.startsWith(".convertzz-"))).toEqual([]);
  });

  it.runIf(process.platform !== "win32")("唯讀檔案在預覽時回報可辨識錯誤", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-readonly-file-plan-"));
    temporary.push(directory);
    const source = join(directory, "里面.txt");
    await writeFile(source, "來源");
    await chmod(source, 0o444);
    const service = new FileService(new ConversionService(resolve("ConvertZZ/Dictionary.csv")));

    const plan = await service.plan(fileNameRequest(source, "overwrite"));

    expect(plan.items).toEqual([expect.objectContaining({ status: "error", warning: "來源檔案為唯讀，無法安全取代。" })]);
    expect(await readFile(source, "utf8")).toBe("來源");
  });

  it.runIf(process.platform !== "win32")("執行前變成唯讀的檔案不會被原子取代", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-readonly-file-apply-"));
    temporary.push(directory);
    const source = join(directory, "里面.txt");
    await writeFile(source, "來源");
    const service = new FileService(new ConversionService(resolve("ConvertZZ/Dictionary.csv")));
    const plan = await service.plan(fileNameRequest(source, "overwrite"));
    await chmod(source, 0o444);
    try {
      const result = await service.apply(plan.planId);
      expect(result.failed).toEqual([expect.objectContaining({ path: "批次作業", message: "來源檔案為唯讀，無法安全取代。" })]);
      expect(await readFile(source, "utf8")).toBe("來源");
      expect(await readdir(directory)).toEqual(["里面.txt"]);
    } finally {
      await chmod(source, 0o644);
    }
  });

  it.runIf(process.platform !== "win32")("唯讀目錄失敗時保持原檔", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-readonly-"));
    temporary.push(directory);
    const source = join(directory, "里面.txt");
    await writeFile(source, "來源");
    const service = new FileService(new ConversionService(resolve("ConvertZZ/Dictionary.csv")));
    const plan = await service.plan(fileNameRequest(source, "overwrite"));
    await chmod(directory, 0o555);
    try {
      const result = await service.apply(plan.planId);
      expect(result.failed).toHaveLength(1);
      expect(await readFile(source, "utf8")).toBe("來源");
    } finally {
      await chmod(directory, 0o755);
    }
  });
});

function fileNameRequest(path: string, conflictPolicy: "skip" | "overwrite") {
  return {
    paths: [path],
    mode: "filename" as const,
    recursive: false,
    inputEncoding: "auto" as const,
    outputEncoding: "auto" as const,
    addBom: false,
    fixCharsetDeclaration: false,
    conflictPolicy,
    conversion: { direction: "s2t" as const, engine: "segmented" as const },
  };
}

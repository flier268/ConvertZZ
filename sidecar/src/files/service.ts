import { randomUUID } from "node:crypto";
import {
  chmod,
  copyFile,
  lstat,
  mkdir,
  readFile,
  readdir,
  rename,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { basename, dirname, extname, isAbsolute, join, relative, resolve } from "node:path";
import type {
  ApplyResult,
  FileConversionPlan,
  FilePlanItem,
  FilePlanRequest,
  TextEncoding,
} from "../../../shared/contracts.js";
import { createUserBackups, resolveBackupRoots, type BackupRoot } from "../backup.js";
import type { ConversionService } from "../conversion/engines.js";
import { decodeText, encodeText } from "../encoding/codecs.js";
import { ConvertZZError } from "../errors.js";
import { cjk2zht } from "cjk-conv";

interface PreparedFile extends FilePlanItem {
  content?: Buffer;
  conflictPolicy: "skip" | "overwrite";
}

interface StoredPlan {
  publicPlan: FileConversionPlan;
  files: PreparedFile[];
  backup: boolean;
  backupRoots: BackupRoot[];
}

interface TransactionEntry {
  file: PreparedFile;
  stagePath: string;
  originalBackup?: string;
  conflictBackup?: string;
  committed: boolean;
}

interface DirectoryTransactionEntry {
  item: PreparedFile;
  temporaryPath: string;
  conflictBackup?: string;
  committed: boolean;
}

type ProgressReporter = (progress: { current: number; total: number; message: string }) => void;
type StageValidator = (
  path: string,
  expected: Buffer | undefined,
  sourcePath: string,
) => Promise<void>;

export class FileService {
  private readonly plans = new Map<string, StoredPlan>();
  private readonly cancelledPlans = new Set<string>();

  constructor(
    private readonly conversion: ConversionService,
    private readonly stageValidator?: StageValidator,
  ) {}

  cancel(planId: string): { cancelled: boolean } {
    const cancelled = this.plans.has(planId);
    if (cancelled) {
      this.cancelledPlans.add(planId);
      this.plans.delete(planId);
    }
    return { cancelled };
  }

  async plan(request: FilePlanRequest, report?: ProgressReporter): Promise<FileConversionPlan> {
    await validateOutputPattern(request.paths[0], request.outputPath);
    const paths = await collectFiles(request.paths, request.recursive, request.allowedExtensions);
    const directories =
      request.mode === "content" ? [] : await collectDirectories(request.paths, request.recursive);
    const files: PreparedFile[] = [];
    const warnings: string[] = [];
    const previewMaxBytes = Math.min(
      1024 * 1024,
      Math.max(1024, request.previewMaxBytes ?? 6 * 1024),
    );

    for (const [index, sourcePath] of paths.entries()) {
      try {
        await assertSourceWritable(sourcePath);
        const sourceBuffer = request.mode === "filename" ? undefined : await readFile(sourcePath);
        const decoded = sourceBuffer ? decodeText(sourceBuffer, request.inputEncoding) : undefined;
        const convertedContent = decoded
          ? await this.conversion.convert({ text: decoded.text, ...request.conversion })
          : undefined;
        const convertedName =
          request.mode === "content"
            ? basename(sourcePath)
            : (await this.conversion.convert({ text: basename(sourcePath), ...request.conversion }))
                .text;
        const defaultOutputPath = join(dirname(sourcePath), convertedName);
        const outputPath = request.outputDirectory
          ? await resolveOutputDirectoryPath(
              sourcePath,
              request.paths,
              request.outputDirectory,
              convertedName,
              request.mode,
              this.conversion,
              request.conversion,
            )
          : request.outputPath
            ? resolveRequestedOutputPath(
                sourcePath,
                request.paths[0] ?? sourcePath,
                request.outputPath,
                convertedName,
                request.mode,
              )
            : defaultOutputPath;
        const outputEncoding = resolveOutputEncoding(request.outputEncoding, decoded?.encoding);
        let outputText = convertedContent
          ? request.fixCharsetDeclaration
            ? fixCharsetDeclaration(
                convertedContent.text,
                outputEncoding,
                extname(sourcePath),
                request.fixCharsetExtensions,
              )
            : convertedContent.text
          : "";
        if (
          convertedContent &&
          request.conversion.direction === "none" &&
          outputEncoding === "big5"
        ) {
          outputText = repairUnrepresentableBig5(outputText);
        }
        const conflict = outputPath !== sourcePath && (await exists(outputPath));
        files.push({
          sourcePath,
          outputPath,
          kind: "file",
          selected: true,
          detectedEncoding: decoded?.encoding,
          sourcePreview: decoded?.text.slice(0, previewMaxBytes) ?? basename(sourcePath),
          outputPreview: convertedContent ? outputText.slice(0, previewMaxBytes) : convertedName,
          status: conflict && request.conflictPolicy === "skip" ? "conflict" : "ready",
          warning: conflict ? "輸出路徑已存在。" : undefined,
          content: convertedContent
            ? encodeText(outputText, outputEncoding, request.addBom)
            : undefined,
          conflictPolicy: request.conflictPolicy,
        });
        if (convertedContent) warnings.push(...convertedContent.warnings);
      } catch (error) {
        files.push({
          sourcePath,
          outputPath: sourcePath,
          kind: "file",
          selected: false,
          sourcePreview: "",
          outputPreview: "",
          status: "error",
          warning: error instanceof Error ? error.message : String(error),
          conflictPolicy: request.conflictPolicy,
        });
      }
      report?.({
        current: index + 1,
        total: paths.length,
        message: `正在建立預覽：${basename(sourcePath)}`,
      });
    }

    if (!request.outputDirectory && !request.outputPath) {
      for (const sourcePath of directories.sort(
        (left, right) => pathDepth(right) - pathDepth(left),
      )) {
        const convertedName = (
          await this.conversion.convert({ text: basename(sourcePath), ...request.conversion })
        ).text;
        const outputPath = join(dirname(sourcePath), convertedName);
        const conflict = outputPath !== sourcePath && (await exists(outputPath));
        files.push({
          sourcePath,
          outputPath,
          kind: "directory",
          selected: true,
          sourcePreview: basename(sourcePath),
          outputPreview: convertedName,
          status: conflict && request.conflictPolicy === "skip" ? "conflict" : "ready",
          warning: conflict ? "輸出資料夾已存在。" : undefined,
          conflictPolicy: request.conflictPolicy,
        });
      }
    }

    const planId = randomUUID();
    const publicPlan: FileConversionPlan = {
      planId,
      createdAt: new Date().toISOString(),
      items: files.map(({ content: _content, conflictPolicy: _policy, ...item }) => item),
      warnings: Array.from(new Set(warnings)),
    };
    this.plans.set(planId, {
      publicPlan,
      files,
      backup: request.backup !== false,
      backupRoots: await resolveBackupRoots(request.paths),
    });
    return publicPlan;
  }

  async apply(
    planId: string,
    report?: ProgressReporter,
    selectedPaths?: string[],
  ): Promise<ApplyResult> {
    const plan = this.plans.get(planId);
    if (!plan) throw new ConvertZZError("PLAN_NOT_FOUND", "檔案轉換計畫已失效。請重新預覽。");
    const result: ApplyResult = { succeeded: [], skipped: [], failed: [] };
    const transaction: TransactionEntry[] = [];
    const directoryTransaction: DirectoryTransactionEntry[] = [];
    const selection = selectedPaths
      ? new Set(selectedPaths.map((value) => resolve(value)))
      : undefined;

    try {
      const readyFiles = plan.files.filter(
        (file) =>
          file.status === "ready" && (!selection || selection.has(resolve(file.sourcePath))),
      );
      if (plan.backup && readyFiles.length) {
        report?.({
          current: 0,
          total: Math.max(1, readyFiles.length * 2 + 1),
          message: "正在建立備份…",
        });
        await createUserBackups(
          plan.backupRoots,
          readyFiles.map((file) => file.sourcePath),
        );
      }
      const total = Math.max(1, readyFiles.length * 2);
      let current = 0;
      for (const file of plan.files) {
        this.throwIfCancelled(planId);
        if (file.kind === "directory") continue;
        if (file.status !== "ready" || (selection && !selection.has(resolve(file.sourcePath)))) {
          result.skipped.push(file.sourcePath);
          continue;
        }
        if (file.outputPath === file.sourcePath && !file.content) {
          result.skipped.push(file.sourcePath);
          continue;
        }
        await assertSourceWritable(file.sourcePath);
        const stagePath = transactionPath(file.outputPath, "stage");
        await mkdir(dirname(stagePath), { recursive: true });
        if (file.content) await writeStage(stagePath, file.content, file.sourcePath);
        else await copyFile(file.sourcePath, stagePath, 1);
        await this.verifyStage(stagePath, file.content, file.sourcePath);
        transaction.push({ file, stagePath, committed: false });
        current += 1;
        report?.({ current, total, message: `正在準備：${basename(file.sourcePath)}` });
      }

      for (const entry of transaction) {
        this.throwIfCancelled(planId);
        entry.originalBackup = transactionPath(entry.file.sourcePath, "original");
        await rename(entry.file.sourcePath, entry.originalBackup);
      }

      for (const entry of transaction) {
        this.throwIfCancelled(planId);
        const { file } = entry;
        if (file.outputPath !== file.sourcePath && (await exists(file.outputPath))) {
          if (entry.file.conflictPolicy === "skip") {
            await rm(entry.stagePath);
            if (entry.originalBackup) await rename(entry.originalBackup, file.sourcePath);
            entry.originalBackup = undefined;
            result.skipped.push(file.sourcePath);
            continue;
          }
          entry.conflictBackup = transactionPath(file.outputPath, "conflict");
          await rename(file.outputPath, entry.conflictBackup);
        }
        await rename(entry.stagePath, file.outputPath);
        entry.committed = true;
        current += 1;
        report?.({ current, total, message: `正在寫入：${basename(file.outputPath)}` });
      }

      const directoryItems = plan.files
        .filter((item) => item.kind === "directory")
        .sort((left, right) => pathDepth(right.sourcePath) - pathDepth(left.sourcePath));
      for (const item of directoryItems) {
        this.throwIfCancelled(planId);
        if (
          item.status !== "ready" ||
          (selection && !selection.has(resolve(item.sourcePath))) ||
          item.outputPath === item.sourcePath
        ) {
          result.skipped.push(item.sourcePath);
          continue;
        }
        const entry: DirectoryTransactionEntry = {
          item,
          temporaryPath: transactionPath(item.sourcePath, "directory"),
          committed: false,
        };
        directoryTransaction.push(entry);
        await rename(item.sourcePath, entry.temporaryPath);
        if (await exists(item.outputPath)) {
          if (item.conflictPolicy === "skip") {
            await rename(entry.temporaryPath, item.sourcePath);
            result.skipped.push(item.sourcePath);
            continue;
          }
          entry.conflictBackup = transactionPath(item.outputPath, "conflict");
          await rename(item.outputPath, entry.conflictBackup);
        }
        await rename(entry.temporaryPath, item.outputPath);
        entry.committed = true;
        current += 2;
        report?.({ current, total, message: `正在重新命名資料夾：${basename(item.outputPath)}` });
      }
    } catch (error) {
      await rollbackDirectories(directoryTransaction);
      await rollbackTransaction(transaction);
      this.plans.delete(planId);
      this.cancelledPlans.delete(planId);
      if (error instanceof ConvertZZError && error.code === "PLAN_CANCELLED") throw error;
      result.failed.push({
        path: "批次作業",
        message: error instanceof Error ? error.message : String(error),
      });
      return result;
    }

    for (const entry of transaction) {
      if (!entry.committed) continue;
      result.succeeded.push(
        resolveCommittedDirectoryPath(entry.file.outputPath, directoryTransaction),
      );
      for (const backup of [entry.originalBackup, entry.conflictBackup]) {
        if (!backup) continue;
        const effectiveBackup = resolveCommittedDirectoryPath(backup, directoryTransaction);
        try {
          await rm(effectiveBackup);
        } catch (error) {
          result.failed.push({
            path: effectiveBackup,
            message: `已完成轉換，但無法清除復原暫存檔。${error instanceof Error ? error.message : String(error)}`,
          });
        }
      }
    }
    for (const entry of directoryTransaction) {
      if (entry.committed) result.succeeded.push(entry.item.outputPath);
      if (entry.conflictBackup) {
        const effectiveBackup = resolveCommittedDirectoryPath(
          entry.conflictBackup,
          directoryTransaction,
        );
        try {
          await rm(effectiveBackup, { recursive: true });
        } catch (error) {
          result.failed.push({
            path: effectiveBackup,
            message: `已完成轉換，但無法清除復原暫存資料夾。${error instanceof Error ? error.message : String(error)}`,
          });
        }
      }
    }
    this.plans.delete(planId);
    this.cancelledPlans.delete(planId);
    return result;
  }

  private throwIfCancelled(planId: string): void {
    if (this.cancelledPlans.has(planId))
      throw new ConvertZZError("PLAN_CANCELLED", "檔案作業已由使用者取消。");
  }

  private async verifyStage(
    path: string,
    expected: Buffer | undefined,
    sourcePath: string,
  ): Promise<void> {
    try {
      await verifyStage(path, expected, sourcePath);
      await this.stageValidator?.(path, expected, sourcePath);
    } catch (error) {
      await rm(path, { force: true });
      throw error;
    }
  }
}

async function collectFiles(
  inputs: string[],
  recursive: boolean,
  allowedExtensions?: string[],
): Promise<string[]> {
  const collected = new Set<string>();
  const allowed = new Set(
    (allowedExtensions ?? []).map((extension) =>
      extension.toLowerCase().startsWith(".")
        ? extension.toLowerCase()
        : `.${extension.toLowerCase()}`,
    ),
  );
  const visit = async (path: string, discovered = false): Promise<void> => {
    const absolute = resolve(path);
    if (/[*?]/u.test(absolute)) {
      const directory = dirname(absolute);
      const matcher = wildcardMatcher(basename(absolute));
      const entries = await readdir(directory, { withFileTypes: true });
      for (const entry of entries) {
        if (entry.isFile() && matcher.test(entry.name)) collected.add(join(directory, entry.name));
      }
      return;
    }
    const info = await lstat(absolute);
    if (info.isSymbolicLink()) return;
    if (info.isFile()) {
      if (!discovered || !allowed.size || allowed.has(extname(absolute).toLowerCase()))
        collected.add(absolute);
      return;
    }
    if (!info.isDirectory()) return;
    const entries = await readdir(absolute, { withFileTypes: true });
    for (const entry of entries) {
      if (entry.isSymbolicLink()) continue;
      if (entry.isFile() && (!allowed.size || allowed.has(extname(entry.name).toLowerCase())))
        collected.add(join(absolute, entry.name));
      else if (recursive && entry.isDirectory()) await visit(join(absolute, entry.name), true);
    }
  };
  for (const path of inputs) await visit(path);
  return Array.from(collected).sort();
}

async function collectDirectories(inputs: string[], recursive: boolean): Promise<string[]> {
  if (!recursive) return [];
  const collected = new Set<string>();
  const visit = async (path: string): Promise<void> => {
    const absolute = resolve(path);
    const info = await lstat(absolute);
    if (info.isSymbolicLink() || !info.isDirectory()) return;
    for (const entry of await readdir(absolute, { withFileTypes: true })) {
      if (entry.isSymbolicLink() || !entry.isDirectory()) continue;
      const child = join(absolute, entry.name);
      collected.add(child);
      await visit(child);
    }
  };
  for (const path of inputs) {
    if (!/[*?]/u.test(path)) await visit(path);
  }
  return Array.from(collected);
}

async function validateOutputPattern(inputPattern?: string, outputPattern?: string): Promise<void> {
  if (!inputPattern || !outputPattern || !inputPattern.includes("*")) return;
  const inputWildcards = inputPattern.match(/\*/gu)?.length ?? 0;
  const outputWildcards = outputPattern.match(/\*/gu)?.length ?? 0;
  if (outputWildcards && outputWildcards !== inputWildcards) {
    throw new ConvertZZError("CLI_WILDCARD", "輸入與輸出路徑的萬用字元數量不同。");
  }
  if (
    !outputWildcards &&
    (await exists(resolve(outputPattern))) &&
    (await stat(resolve(outputPattern))).isFile()
  ) {
    throw new ConvertZZError("CLI_OUTPUT", "多檔輸入的輸出路徑不能是既有檔案。");
  }
}

function resolveRequestedOutputPath(
  sourcePath: string,
  inputPattern: string,
  outputPattern: string,
  convertedName: string,
  mode: FilePlanRequest["mode"],
): string {
  const absoluteOutput = resolve(outputPattern);
  if (!/[*?]/u.test(inputPattern)) {
    if (mode === "content") return absoluteOutput;
    return join(dirname(absoluteOutput), convertedName);
  }

  const match = wildcardMatcher(basename(resolve(inputPattern))).exec(basename(sourcePath));
  if (/\*/u.test(outputPattern)) {
    let capture = 1;
    return resolve(outputPattern.replace(/\*/gu, () => match?.[capture++] ?? ""));
  }
  return join(absoluteOutput, mode === "content" ? basename(sourcePath) : convertedName);
}

async function resolveOutputDirectoryPath(
  sourcePath: string,
  inputs: string[],
  outputDirectory: string,
  convertedName: string,
  mode: FilePlanRequest["mode"],
  conversion: ConversionService,
  conversionRequest: FilePlanRequest["conversion"],
): Promise<string> {
  const firstInput = resolve(inputs[0] ?? sourcePath);
  const base = inputs.length === 1 && !/[*?]/u.test(firstInput) ? firstInput : dirname(firstInput);
  const relativePath = relative(base, sourcePath);
  const safeRelative = relativePath.startsWith("..") ? basename(sourcePath) : relativePath;
  const relativeDirectory =
    dirname(safeRelative) === "." ? [] : dirname(safeRelative).split(/[\\/]/u);
  const convertedDirectories =
    mode === "content"
      ? relativeDirectory
      : await Promise.all(
          relativeDirectory.map(
            async (part) => (await conversion.convert({ text: part, ...conversionRequest })).text,
          ),
        );
  return join(resolve(outputDirectory), ...convertedDirectories, convertedName);
}

function wildcardMatcher(pattern: string): RegExp {
  const source = Array.from(pattern, (character) => {
    if (character === "*") return "(.*?)";
    if (character === "?") return "(.)";
    return character.replace(/[\\^$.*+?()[\]{}|]/gu, "\\$&");
  }).join("");
  return new RegExp(`^${source}$`, process.platform === "win32" ? "iu" : "u");
}

function resolveOutputEncoding(
  requested: TextEncoding,
  detected?: TextEncoding,
): Exclude<TextEncoding, "auto"> {
  if (requested !== "auto") return requested;
  return detected && detected !== "auto" ? detected : "utf8";
}

function repairUnrepresentableBig5(text: string): string {
  return Array.from(text, (character) => {
    const encoded = encodeText(character, "big5");
    return decodeText(encoded, "big5").text === character ? character : cjk2zht(character);
  }).join("");
}

function fixCharsetDeclaration(
  text: string,
  encoding: TextEncoding,
  extension: string,
  configured?: string[],
): string {
  const extensions = configured?.length
    ? new Set(
        configured
          .map((value) => value.trim().toLowerCase())
          .filter(Boolean)
          .map((value) => (value.startsWith(".") ? value : `.${value}`)),
      )
    : new Set([".htm", ".html", ".shtm", ".shtml", ".asp", ".aspx", ".php", ".css"]);
  if (!extensions.has(extension.toLowerCase())) return text;
  const charset = (
    {
      utf8: "utf-8",
      "utf8-bom": "utf-8",
      utf16le: "utf-16le",
      utf16be: "utf-16be",
      big5: "big5",
      gbk: "gbk",
      "shift-jis": "shift_jis",
      "euc-jp": "euc-jp",
      "iso-2022-jp": "iso-2022-jp",
      "hz-gb-2312": "hz-gb-2312",
      auto: "utf-8",
    } satisfies Record<TextEncoding, string>
  )[encoding];
  return text
    .replace(/(<meta\s+[^>]*charset\s*=\s*["']?)[^\s"'/>]+/gi, `$1${charset}`)
    .replace(/(@charset\s+["'])[^"']+(["'])/gi, `$1${charset}$2`)
    .replace(/(content\s*=\s*["'][^"']*charset\s*=\s*)[^\s"';]+/gi, `$1${charset}`);
}

async function writeStage(path: string, content: Buffer, sourcePath: string): Promise<void> {
  try {
    await writeFile(path, content, { flag: "wx" });
    const source = await stat(sourcePath);
    await chmod(path, source.mode);
  } catch (error) {
    await rm(path, { force: true });
    throw error;
  }
}

async function assertSourceWritable(path: string): Promise<void> {
  const source = await stat(path);
  if ((source.mode & 0o222) === 0) {
    throw new ConvertZZError("FILE_READONLY", "來源檔案為唯讀，無法安全取代。", { path });
  }
}

async function verifyStage(
  path: string,
  expected: Buffer | undefined,
  sourcePath: string,
): Promise<void> {
  const staged = await readFile(path);
  const comparison = expected ?? (await readFile(sourcePath));
  if (!staged.equals(comparison)) {
    await rm(path, { force: true });
    throw new ConvertZZError("FILE_VERIFY", "暫存檔寫入驗證失敗。", { path });
  }
}

async function rollbackTransaction(transaction: TransactionEntry[]): Promise<void> {
  const reversed = [...transaction].reverse();
  for (const entry of reversed) {
    try {
      if (entry.committed && (await exists(entry.file.outputPath))) await rm(entry.file.outputPath);
    } catch {
      // The original failure is returned while recoverable temporary files remain in place.
    }
  }
  for (const entry of reversed) {
    try {
      if (
        entry.originalBackup &&
        (await exists(entry.originalBackup)) &&
        !(await exists(entry.file.sourcePath))
      ) {
        await rename(entry.originalBackup, entry.file.sourcePath);
      }
    } catch {
      // The original failure is returned while recoverable temporary files remain in place.
    }
  }
  for (const entry of reversed) {
    try {
      if (
        entry.conflictBackup &&
        (await exists(entry.conflictBackup)) &&
        !(await exists(entry.file.outputPath))
      ) {
        await rename(entry.conflictBackup, entry.file.outputPath);
      }
      if (await exists(entry.stagePath)) await rm(entry.stagePath);
    } catch {
      // The original failure is returned while recoverable temporary files remain in place.
    }
  }
}

async function rollbackDirectories(transaction: DirectoryTransactionEntry[]): Promise<void> {
  for (const entry of [...transaction].reverse()) {
    try {
      if (
        entry.committed &&
        (await exists(entry.item.outputPath)) &&
        !(await exists(entry.item.sourcePath))
      ) {
        await rename(entry.item.outputPath, entry.item.sourcePath);
      } else if (
        !entry.committed &&
        (await exists(entry.temporaryPath)) &&
        !(await exists(entry.item.sourcePath))
      ) {
        await rename(entry.temporaryPath, entry.item.sourcePath);
      }
      if (
        entry.conflictBackup &&
        (await exists(entry.conflictBackup)) &&
        !(await exists(entry.item.outputPath))
      ) {
        await rename(entry.conflictBackup, entry.item.outputPath);
      }
    } catch {
      // Leave recoverable temporary directories in place when restoration is blocked.
    }
  }
}

function transactionPath(
  path: string,
  kind: "stage" | "original" | "conflict" | "directory",
): string {
  return join(dirname(path), `.convertzz-${kind}-${randomUUID()}${extname(path)}`);
}

function pathDepth(path: string): number {
  return resolve(path).split(/[\\/]/u).filter(Boolean).length;
}

function resolveCommittedDirectoryPath(
  path: string,
  transaction: DirectoryTransactionEntry[],
): string {
  return transaction.reduce((current, entry) => {
    if (!entry.committed) return current;
    const suffix = relative(entry.item.sourcePath, current);
    if (suffix.startsWith("..") || isAbsolute(suffix)) return current;
    return join(entry.item.outputPath, suffix);
  }, path);
}

async function exists(path: string): Promise<boolean> {
  try {
    await stat(path);
    return true;
  } catch {
    return false;
  }
}

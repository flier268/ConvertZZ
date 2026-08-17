import { cp, copyFile, lstat, readdir, rm, stat } from "node:fs/promises";
import { basename, dirname, isAbsolute, join, relative, resolve } from "node:path";
import { ConvertZZError } from "./errors.js";

export interface BackupRoot {
  path: string;
  kind: "file" | "directory";
}

/** 來源路徑對應的使用者備份路徑：`path.bak`。 */
export function userBackupPath(sourcePath: string): string {
  return `${sourcePath}.bak`;
}

/**
 * 依計畫輸入路徑決定備份單位。
 * 選取資料夾時整份備份為 `資料夾.bak`；選取檔案或萬用字元展開的檔案則逐檔 `.bak`。
 */
export async function resolveBackupRoots(paths: string[]): Promise<BackupRoot[]> {
  const roots: BackupRoot[] = [];
  for (const path of paths) {
    if (/[*?]/u.test(path)) {
      for (const match of await expandWildcardFiles(path)) {
        roots.push({ path: match, kind: "file" });
      }
      continue;
    }
    const absolute = resolve(path);
    try {
      const info = await lstat(absolute);
      if (info.isSymbolicLink()) continue;
      if (info.isDirectory()) roots.push({ path: absolute, kind: "directory" });
      else if (info.isFile()) roots.push({ path: absolute, kind: "file" });
    } catch {
      // 計畫階段可能含尚未驗證的路徑，略過無法備份的項目。
    }
  }
  return pruneNestedBackupRoots(roots);
}

/** 只保留最外層資料夾與不在其內的檔案，避免同一樹重複備份。 */
export function pruneNestedBackupRoots(roots: BackupRoot[]): BackupRoot[] {
  const unique = new Map<string, BackupRoot>();
  for (const root of roots) unique.set(root.path, root);
  const list = Array.from(unique.values());
  const directories = list
    .filter((root) => root.kind === "directory")
    .map((root) => root.path)
    .sort((left, right) => left.length - right.length);

  return list.filter((root) => {
    const coveredByOuterDirectory = directories.some(
      (directory) => directory !== root.path && pathIsInside(root.path, directory),
    );
    return !coveredByOuterDirectory;
  });
}

/**
 * 對會被修改的來源建立 `.bak`。
 * 資料夾根：整份複製為 `folder.bak`；檔案根：`file.ext.bak`。
 */
export async function createUserBackups(
  roots: BackupRoot[],
  affectedPaths: string[],
): Promise<string[]> {
  if (!roots.length || !affectedPaths.length) return [];
  const affected = affectedPaths.map((path) => resolve(path));
  const created: string[] = [];

  for (const root of roots) {
    const covers = affected.some((path) =>
      root.kind === "directory" ? pathIsInside(path, root.path) : path === root.path,
    );
    if (!covers) continue;
    created.push(await createUserBackup(root.path));
  }
  return created;
}

export async function createUserBackup(sourcePath: string): Promise<string> {
  const absolute = resolve(sourcePath);
  const target = userBackupPath(absolute);
  try {
    const info = await lstat(absolute);
    if (info.isSymbolicLink()) {
      throw new ConvertZZError("BACKUP_SYMLINK", "不備份符號連結來源。", { path: absolute });
    }
    await rm(target, { recursive: true, force: true });
    if (info.isDirectory()) {
      await cp(absolute, target, {
        recursive: true,
        force: true,
        errorOnExist: false,
        verbatimSymlinks: true,
      });
    } else if (info.isFile()) {
      await copyFile(absolute, target);
    } else {
      throw new ConvertZZError("BACKUP_UNSUPPORTED", "不支援的備份來源類型。", { path: absolute });
    }
    return target;
  } catch (error) {
    if (error instanceof ConvertZZError) throw error;
    throw new ConvertZZError(
      "BACKUP_FAILED",
      `建立備份失敗：${error instanceof Error ? error.message : String(error)}`,
      { path: absolute, backupPath: target },
    );
  }
}

export function pathIsInside(path: string, directory: string): boolean {
  const relativePath = relative(resolve(directory), resolve(path));
  return relativePath === "" || (!relativePath.startsWith("..") && !isAbsolute(relativePath));
}

async function expandWildcardFiles(pattern: string): Promise<string[]> {
  const absolute = resolve(pattern);
  const directory = dirname(absolute);
  const matcher = wildcardMatcher(basename(absolute));
  try {
    await stat(directory);
  } catch {
    return [];
  }
  const entries = await readdir(directory, { withFileTypes: true });
  return entries
    .filter((entry) => entry.isFile() && matcher.test(entry.name))
    .map((entry) => join(directory, entry.name))
    .sort();
}

function wildcardMatcher(pattern: string): RegExp {
  const source = Array.from(pattern, (character) => {
    if (character === "*") return "(.*?)";
    if (character === "?") return "(.)";
    return character.replace(/[\\^$.*+?()[\]{}|]/gu, "\\$&");
  }).join("");
  return new RegExp(`^${source}$`, process.platform === "win32" ? "iu" : "u");
}

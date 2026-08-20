import {
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

/** 執行檔旁此標記代表可攜模式：設定寫在同目錄。 */
export const PORTABLE_MARKER = "portable";

/**
 * @param {string} releaseDir
 * @returns {string[]}
 */
export function listPortablePayload(releaseDir) {
  const required = ["ConvertZZ.exe", "Dictionary.csv", "segment-dict"];
  for (const name of required) {
    const path = join(releaseDir, name);
    if (!existsSync(path)) {
      throw new Error(`免安裝版缺少 ${name}（目錄：${releaseDir}）。`);
    }
  }
  /** @type {string[]} */
  const entries = ["ConvertZZ.exe", "Dictionary.csv", "segment-dict"];
  const licenses = join(releaseDir, "licenses");
  if (existsSync(licenses) && statSync(licenses).isDirectory()) {
    entries.push("licenses");
  }
  for (const entry of readdirSync(releaseDir)) {
    if (entry.toLowerCase().endsWith(".dll")) {
      entries.push(entry);
    }
  }
  return entries;
}

/**
 * @param {{ releaseDir: string; version: string; outDir: string }} options
 * @returns {{ zipPath: string; zipName: string; stagedRoot: string }}
 */
export function packageWindowsPortable({ releaseDir, version, outDir }) {
  if (!version) throw new Error("缺少版本字串。");
  const normalizedVersion = version.replace(/^v/iu, "");
  if (!normalizedVersion) throw new Error("版本字串無效。");

  const zipName = `ConvertZZ_${normalizedVersion}_x64-portable.zip`;
  const zipPath = join(outDir, zipName);
  const stageDirName = `.portable-stage-${normalizedVersion}`;
  const stageRoot = join(outDir, stageDirName);
  const stagedApp = join(stageRoot, "ConvertZZ");

  rmSync(stageRoot, { recursive: true, force: true });
  mkdirSync(stagedApp, { recursive: true });
  mkdirSync(outDir, { recursive: true });

  for (const entry of listPortablePayload(releaseDir)) {
    cpSync(join(releaseDir, entry), join(stagedApp, entry), { recursive: true });
  }
  writeFileSync(join(stagedApp, PORTABLE_MARKER), "");

  rmSync(zipPath, { force: true });
  // Windows tar 會把絕對路徑的磁碟代號（如 D:）當成遠端主機，改在 outDir 用相對路徑。
  const archive = spawnSync("tar", ["-a", "-cf", zipName, "-C", stageDirName, "ConvertZZ"], {
    cwd: outDir,
    encoding: "utf8",
  });
  if (archive.status !== 0) {
    throw new Error(
      `建立免安裝 zip 失敗：${archive.stderr || archive.stdout || `exit ${archive.status}`}`,
    );
  }
  if (!existsSync(zipPath)) {
    throw new Error(`未產生免安裝 zip：${zipPath}`);
  }

  rmSync(stageRoot, { recursive: true, force: true });
  return { zipPath, zipName, stagedRoot: stagedApp };
}

function readArg(name) {
  const index = process.argv.indexOf(`--${name}`);
  if (index === -1) return "";
  return process.argv[index + 1] ?? "";
}

const entry = process.argv[1];
if (entry && fileURLToPath(import.meta.url) === resolve(entry)) {
  const releaseDir = readArg("release-dir");
  const version = readArg("version");
  const outDir = readArg("out-dir");
  if (!releaseDir || !version || !outDir) {
    process.stderr.write(
      "用法：node scripts/package-windows-portable.mjs --release-dir <目錄> --version <版本> --out-dir <目錄>\n",
    );
    process.exit(1);
  }
  const result = packageWindowsPortable({
    releaseDir: resolve(releaseDir),
    version,
    outDir: resolve(outDir),
  });
  process.stdout.write(`${basename(result.zipPath)}\n`);
}

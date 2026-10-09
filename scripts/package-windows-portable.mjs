import {
  cpSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, join, posix, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { crc32, deflateRawSync } from "node:zlib";

/** 執行檔旁此標記代表可攜模式：設定寫在同目錄。 */
export const PORTABLE_MARKER = "portable";

const ZIP_LOCAL_HEADER = 0x04034b50;
const ZIP_CENTRAL_HEADER = 0x02014b50;
const ZIP_EOCD = 0x06054b50;
const ZIP_UTF8_FLAG = 0x0800;
const ZIP_VERSION = 20;
const ZIP_MAX_UINT16 = 0xffff;
const ZIP_MAX_UINT32 = 0xffffffff;

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
 * GNU tar 的 `-a` 只依後綴選擇 gzip/xz 等壓縮程式，`.zip` 仍會寫成 tar。
 * Windows bsdtar 才會把 `.zip` 寫成 PKZIP，因此改用 Node 直接寫 zip。
 *
 * @param {string} zipPath
 * @returns {string[]}
 */
export function listZipEntries(zipPath) {
  const bytes = readFileSync(zipPath);
  if (bytes.length < 22 || bytes.readUInt32LE(0) !== ZIP_LOCAL_HEADER) {
    throw new Error(`不是 PKZIP：${zipPath}`);
  }
  const eocdOffset = bytes.length - 22;
  if (bytes.readUInt32LE(eocdOffset) !== ZIP_EOCD) {
    throw new Error(`不是 PKZIP（缺少 EOCD）：${zipPath}`);
  }
  const entryCount = bytes.readUInt16LE(eocdOffset + 10);
  let offset = bytes.readUInt32LE(eocdOffset + 16);
  /** @type {string[]} */
  const names = [];
  for (let index = 0; index < entryCount; index += 1) {
    if (offset + 46 > bytes.length || bytes.readUInt32LE(offset) !== ZIP_CENTRAL_HEADER) {
      throw new Error(`zip 中央目錄損毀：${zipPath}`);
    }
    const nameLength = bytes.readUInt16LE(offset + 28);
    const extraLength = bytes.readUInt16LE(offset + 30);
    const commentLength = bytes.readUInt16LE(offset + 32);
    const nameStart = offset + 46;
    const nameEnd = nameStart + nameLength;
    if (nameEnd > bytes.length) {
      throw new Error(`zip 檔名損毀：${zipPath}`);
    }
    names.push(bytes.subarray(nameStart, nameEnd).toString("utf8"));
    offset = nameEnd + extraLength + commentLength;
  }
  return names;
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
  writeZipDirectory(zipPath, stagedApp, "ConvertZZ");
  if (!existsSync(zipPath)) {
    throw new Error(`未產生免安裝 zip：${zipPath}`);
  }
  const magic = readFileSync(zipPath).subarray(0, 4);
  if (!magic.equals(Buffer.from([0x50, 0x4b, 0x03, 0x04]))) {
    throw new Error(`免安裝檔不是 PKZIP：${zipPath}`);
  }

  rmSync(stageRoot, { recursive: true, force: true });
  return { zipPath, zipName, stagedRoot: stagedApp };
}

/**
 * @param {string} zipPath
 * @param {string} sourceDir
 * @param {string} archiveRootName
 */
function writeZipDirectory(zipPath, sourceDir, archiveRootName) {
  const records = collectZipRecords(sourceDir, archiveRootName);
  if (records.length > ZIP_MAX_UINT16) {
    throw new Error("免安裝 zip 條目過多，無法寫入標準 zip。");
  }
  writeFileSync(zipPath, buildZip(records));
}

/**
 * @typedef {{ name: string; data: Buffer; mtime: Date; isDir: boolean }} ZipRecord
 */

/**
 * @param {string} sourceDir
 * @param {string} archiveRootName
 * @returns {ZipRecord[]}
 */
function collectZipRecords(sourceDir, archiveRootName) {
  /** @type {ZipRecord[]} */
  const records = [];

  /**
   * @param {string} absoluteDir
   * @param {string} zipDirName
   */
  function addDirectory(absoluteDir, zipDirName) {
    const stat = lstatSync(absoluteDir);
    if (stat.isSymbolicLink()) return;
    records.push({
      name: `${zipDirName}/`,
      data: Buffer.alloc(0),
      mtime: stat.mtime,
      isDir: true,
    });
    for (const name of readdirSync(absoluteDir).sort()) {
      const child = join(absoluteDir, name);
      const childStat = lstatSync(child);
      if (childStat.isSymbolicLink()) continue;
      const childZipName = posix.join(zipDirName, name);
      if (childStat.isDirectory()) {
        addDirectory(child, childZipName);
      } else if (childStat.isFile()) {
        records.push({
          name: childZipName,
          data: readFileSync(child),
          mtime: childStat.mtime,
          isDir: false,
        });
      }
    }
  }

  addDirectory(sourceDir, archiveRootName);
  return records;
}

/**
 * @param {ZipRecord[]} records
 * @returns {Buffer}
 */
function buildZip(records) {
  /** @type {Buffer[]} */
  const localParts = [];
  /** @type {Buffer[]} */
  const centralParts = [];
  let offset = 0;

  for (const record of records) {
    const nameBuf = Buffer.from(record.name, "utf8");
    const { dosTime, dosDate } = dosDateTime(record.mtime);
    const uncompressed = record.data;
    let method = 0;
    let compressed = uncompressed;
    if (!record.isDir && uncompressed.length > 0) {
      const deflated = deflateRawSync(uncompressed, { level: 9 });
      if (deflated.length < uncompressed.length) {
        method = 8;
        compressed = deflated;
      }
    }
    if (
      compressed.length > ZIP_MAX_UINT32 ||
      uncompressed.length > ZIP_MAX_UINT32 ||
      offset > ZIP_MAX_UINT32
    ) {
      throw new Error("免安裝 zip 過大，無法寫入標準 zip。");
    }

    const crc = crc32(uncompressed);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(ZIP_LOCAL_HEADER, 0);
    local.writeUInt16LE(ZIP_VERSION, 4);
    local.writeUInt16LE(ZIP_UTF8_FLAG, 6);
    local.writeUInt16LE(method, 8);
    local.writeUInt16LE(dosTime, 10);
    local.writeUInt16LE(dosDate, 12);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(compressed.length, 18);
    local.writeUInt32LE(uncompressed.length, 22);
    local.writeUInt16LE(nameBuf.length, 26);
    local.writeUInt16LE(0, 28);

    const central = Buffer.alloc(46);
    central.writeUInt32LE(ZIP_CENTRAL_HEADER, 0);
    central.writeUInt16LE(ZIP_VERSION, 4);
    central.writeUInt16LE(ZIP_VERSION, 6);
    central.writeUInt16LE(ZIP_UTF8_FLAG, 8);
    central.writeUInt16LE(method, 10);
    central.writeUInt16LE(dosTime, 12);
    central.writeUInt16LE(dosDate, 14);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(compressed.length, 20);
    central.writeUInt32LE(uncompressed.length, 24);
    central.writeUInt16LE(nameBuf.length, 28);
    central.writeUInt16LE(0, 30);
    central.writeUInt16LE(0, 32);
    central.writeUInt16LE(0, 34);
    central.writeUInt16LE(0, 36);
    central.writeUInt32LE(record.isDir ? 0x10 : 0, 38);
    central.writeUInt32LE(offset, 42);

    localParts.push(local, nameBuf, compressed);
    centralParts.push(central, nameBuf);
    offset += 30 + nameBuf.length + compressed.length;
  }

  const centralDirectory = Buffer.concat(centralParts);
  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(ZIP_EOCD, 0);
  eocd.writeUInt16LE(0, 4);
  eocd.writeUInt16LE(0, 6);
  eocd.writeUInt16LE(records.length, 8);
  eocd.writeUInt16LE(records.length, 10);
  eocd.writeUInt32LE(centralDirectory.length, 12);
  eocd.writeUInt32LE(offset, 16);
  eocd.writeUInt16LE(0, 20);
  return Buffer.concat([...localParts, centralDirectory, eocd]);
}

/**
 * @param {Date} date
 * @returns {{ dosTime: number; dosDate: number }}
 */
function dosDateTime(date) {
  const year = Math.min(2107, Math.max(1980, date.getFullYear()));
  const dosTime =
    (date.getHours() << 11) | (date.getMinutes() << 5) | Math.floor(date.getSeconds() / 2);
  const dosDate = ((year - 1980) << 9) | ((date.getMonth() + 1) << 5) | date.getDate();
  return { dosTime, dosDate };
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

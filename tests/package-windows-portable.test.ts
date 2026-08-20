import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  listPortablePayload,
  listZipEntries,
  packageWindowsPortable,
} from "../scripts/package-windows-portable.mjs";

function makeReleaseDir() {
  const root = mkdtempSync(join(tmpdir(), "convertzz-portable-"));
  writeFileSync(join(root, "ConvertZZ.exe"), "exe");
  writeFileSync(join(root, "Dictionary.csv"), "csv");
  mkdirSync(join(root, "segment-dict", "segment"), { recursive: true });
  writeFileSync(join(root, "segment-dict", "segment", "dict.txt"), "dict");
  mkdirSync(join(root, "licenses"), { recursive: true });
  writeFileSync(join(root, "licenses", "THIRD_PARTY_NOTICES.md"), "notice");
  writeFileSync(join(root, "helper.dll"), "dll");
  return root;
}

describe("Windows 免安裝 zip 打包", () => {
  it("會收集 exe、字典、分詞資源、授權與 DLL", () => {
    const releaseDir = makeReleaseDir();
    try {
      expect(listPortablePayload(releaseDir)).toEqual([
        "ConvertZZ.exe",
        "Dictionary.csv",
        "segment-dict",
        "licenses",
        "helper.dll",
      ]);
    } finally {
      rmSync(releaseDir, { recursive: true, force: true });
    }
  });

  it("缺少必要檔案時會失敗", () => {
    const releaseDir = mkdtempSync(join(tmpdir(), "convertzz-portable-missing-"));
    try {
      writeFileSync(join(releaseDir, "ConvertZZ.exe"), "exe");
      expect(() => listPortablePayload(releaseDir)).toThrow(/Dictionary\.csv/);
    } finally {
      rmSync(releaseDir, { recursive: true, force: true });
    }
  });

  it("會產生 ConvertZZ_<version>_x64-portable.zip，內容在 ConvertZZ/ 目錄", () => {
    const releaseDir = makeReleaseDir();
    const outDir = mkdtempSync(join(tmpdir(), "convertzz-portable-out-"));
    try {
      const result = packageWindowsPortable({
        releaseDir,
        version: "v2.0.0-beta6",
        outDir,
      });
      expect(result.zipName).toBe("ConvertZZ_2.0.0-beta6_x64-portable.zip");
      expect(result.zipPath).toBe(join(outDir, result.zipName));

      const bytes = readFileSync(result.zipPath);
      expect([...bytes.subarray(0, 4)]).toEqual([0x50, 0x4b, 0x03, 0x04]);
      expect(bytes.subarray(257, 262).toString("utf8")).not.toBe("ustar");

      const entries = listZipEntries(result.zipPath);
      expect(entries).toContain("ConvertZZ/ConvertZZ.exe");
      expect(entries).toContain("ConvertZZ/Dictionary.csv");
      expect(entries).toContain("ConvertZZ/helper.dll");
      expect(entries).toContain("ConvertZZ/portable");
      expect(entries.some((entry) => entry.includes("segment-dict/segment/dict.txt"))).toBe(true);
      expect(entries.some((entry) => entry.includes("licenses/THIRD_PARTY_NOTICES.md"))).toBe(true);
      expect(bytes.byteLength).toBeGreaterThan(0);
    } finally {
      rmSync(releaseDir, { recursive: true, force: true });
      rmSync(outDir, { recursive: true, force: true });
    }
  });
});

import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import {
  CSHARP_FILE_TYPE_FILTER_WITH_AUDIO,
  DEFAULT_FILE_TYPE_FILTER,
  LEGACY_FILE_TYPE_FILTER_WITH_AUDIO,
} from "./fileFilters";
import { defaultCheckPreReleaseUpdates, defaultSettings, migrateSettings } from "./settingsMigrate";

interface TypeFilterMigrationCase {
  name: string;
  source: string;
  input: string;
  expected: "default" | "keep";
}

interface TypeFilterMigrationFixture {
  defaultTypeFilter: string;
  cases: TypeFilterMigrationCase[];
}

const typeFilterVectors = JSON.parse(
  readFileSync(
    resolve(
      dirname(fileURLToPath(import.meta.url)),
      "../../tests/fixtures/type-filter-migration.json",
    ),
    "utf8",
  ),
) as TypeFilterMigrationFixture;

describe("開發／預發佈更新預設", () => {
  it("正式版預設不檢查開發通道，非正式版預設檢查", () => {
    expect(defaultCheckPreReleaseUpdates("2.0.0")).toBe(false);
    expect(defaultCheckPreReleaseUpdates("v2.0.0")).toBe(false);
    expect(defaultCheckPreReleaseUpdates("2.0.0-beta5")).toBe(true);
    expect(defaultCheckPreReleaseUpdates("v2.0.0-rc.1")).toBe(true);
    expect(defaultSettings("2.0.0").checkPreReleaseUpdates).toBe(false);
    expect(defaultSettings("2.0.0-beta5").checkPreReleaseUpdates).toBe(true);
  });

  it("缺少欄位時依目前版本補預設，已寫入的選擇維持不變", () => {
    expect(migrateSettings({ version: 2, engine: "legacy" }, "2.0.0").checkPreReleaseUpdates).toBe(
      false,
    );
    expect(
      migrateSettings({ version: 2, engine: "legacy" }, "2.0.0-beta5").checkPreReleaseUpdates,
    ).toBe(true);
    expect(
      migrateSettings({ version: 2, checkPreReleaseUpdates: false }, "2.0.0-beta5")
        .checkPreReleaseUpdates,
    ).toBe(false);
    expect(
      migrateSettings({ version: 2, checkPreReleaseUpdates: true }, "2.0.0").checkPreReleaseUpdates,
    ).toBe(true);
    expect(migrateSettings({ CheckVersion: false }, "2.0.0-rc.1").checkPreReleaseUpdates).toBe(
      true,
    );
  });

  it("空字串（beta1–8 清空篩選器）與舊版缺值都改回預設", () => {
    const defaults = defaultSettings("2.0.0");
    expect(defaults.files.typeFilter).toBe(DEFAULT_FILE_TYPE_FILTER);
    expect(defaults.files.typeFilter).not.toMatch(/mp3|flac|ogg|wav|m4a/iu);
    expect(
      migrateSettings({ version: 2, files: { typeFilter: "" } }, "2.0.0").files.typeFilter,
    ).toBe(DEFAULT_FILE_TYPE_FILTER);
    expect(migrateSettings({ FileConvert: { TypeFilter: "" } }, "2.0.0").files.typeFilter).toBe(
      defaults.files.typeFilter,
    );
  });

  it("共用向量與程式內建字串一致", () => {
    // 舊格式字串來源寫在 tests/fixtures/type-filter-migration.json 的 source：
    // e7c3aeb ConvertZZ/Settings.cs:112、03204e6 ConvertZZ/Settings.cs:109。
    expect(typeFilterVectors.defaultTypeFilter).toBe(DEFAULT_FILE_TYPE_FILTER);
    const byName = new Map(typeFilterVectors.cases.map((item) => [item.name, item]));
    expect(byName.get("2.0 音訊版預設")?.input).toBe(LEGACY_FILE_TYPE_FILTER_WITH_AUDIO);
    expect(byName.get("C# 音頻版預設")?.input).toBe(CSHARP_FILE_TYPE_FILTER_WITH_AUDIO);
    expect(byName.get("新預設")?.input).toBe(DEFAULT_FILE_TYPE_FILTER);
    expect(byName.get("e7c3aeb 舊格式")?.source).toContain("e7c3aeb");
    expect(byName.get("e7c3aeb 舊格式")?.source).toContain("Settings.cs:112");
    expect(byName.get("03204e6 舊格式")?.source).toContain("03204e6");
    expect(byName.get("03204e6 舊格式")?.source).toContain("Settings.cs:109");
  });

  it.each(typeFilterVectors.cases)("$name", (item) => {
    const expected = item.expected === "default" ? typeFilterVectors.defaultTypeFilter : item.input;
    expect(item.source.length).toBeGreaterThan(0);
    expect(
      migrateSettings({ version: 2, files: { typeFilter: item.input } }, "2.0.0").files.typeFilter,
    ).toBe(expected);
    expect(
      migrateSettings({ FileConvert: { TypeFilter: item.input } }, "2.0.0").files.typeFilter,
    ).toBe(expected);
  });
});

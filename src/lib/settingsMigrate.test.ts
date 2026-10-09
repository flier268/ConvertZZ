import { describe, expect, it } from "vitest";
import {
  CSHARP_FILE_TYPE_FILTER_WITH_AUDIO,
  DEFAULT_FILE_TYPE_FILTER,
  LEGACY_FILE_TYPE_FILTER_WITH_AUDIO,
} from "./fileFilters";
import { defaultCheckPreReleaseUpdates, defaultSettings, migrateSettings } from "./settingsMigrate";

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

  it("早期內建音訊篩選會改成不含音訊的預設，自訂篩選保留", () => {
    const defaults = defaultSettings("2.0.0");
    expect(defaults.files.typeFilter).not.toMatch(/mp3|flac|ogg|wav|m4a/iu);
    expect(
      migrateSettings(
        { version: 2, files: { typeFilter: LEGACY_FILE_TYPE_FILTER_WITH_AUDIO } },
        "2.0.0",
      ).files.typeFilter,
    ).toBe(defaults.files.typeFilter);
    expect(
      migrateSettings({ FileConvert: { TypeFilter: LEGACY_FILE_TYPE_FILTER_WITH_AUDIO } }, "2.0.0")
        .files.typeFilter,
    ).toBe(defaults.files.typeFilter);
    expect(
      migrateSettings({ version: 2, files: { typeFilter: "<日誌|*.log>" } }, "2.0.0").files
        .typeFilter,
    ).toBe("<日誌|*.log>");
    expect(
      migrateSettings({ version: 2, files: { typeFilter: "" } }, "2.0.0").files.typeFilter,
    ).toBe("");
  });

  it("C# 預設音頻篩選會改成不含音訊的預設，自訂音頻群組保留", () => {
    const csharp =
      "<常用文字檔案|*.txt;*.log;*.ini;*.inf;*.bat;*.cmd;*.srt;*.ass;*.lang>/<常用網頁文件|*.htm;*.html;*.php;*.asp;*.css;*.js>/<音頻文件|*.mp3>";
    expect(CSHARP_FILE_TYPE_FILTER_WITH_AUDIO).toBe(csharp);
    const defaults = defaultSettings("2.0.0");
    expect(
      migrateSettings({ version: 2, files: { typeFilter: csharp } }, "2.0.0").files.typeFilter,
    ).toBe(defaults.files.typeFilter);
    expect(migrateSettings({ FileConvert: { TypeFilter: csharp } }, "2.0.0").files.typeFilter).toBe(
      defaults.files.typeFilter,
    );
    const audioInTheMiddle = `${DEFAULT_FILE_TYPE_FILTER.split("/").join("/<音訊文件|*.wav>/")}`;
    expect(audioInTheMiddle).toContain("音訊文件");
    expect(
      migrateSettings({ version: 2, files: { typeFilter: audioInTheMiddle } }, "2.0.0").files
        .typeFilter,
    ).toBe(defaults.files.typeFilter);
    expect(
      migrateSettings(
        { version: 2, files: { typeFilter: "<日誌|*.log>/<音頻文件|*.mp3>" } },
        "2.0.0",
      ).files.typeFilter,
    ).toBe("<日誌|*.log>/<音頻文件|*.mp3>");
  });
});

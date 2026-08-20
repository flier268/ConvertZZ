import { describe, expect, it } from "vitest";
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
});

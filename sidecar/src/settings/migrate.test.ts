import { afterEach, describe, expect, it } from "vitest";
import { mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { backupLegacySettings, migrateSettings } from "./migrate.js";

const temporary: string[] = [];
afterEach(async () => Promise.all(temporary.splice(0).map((path) => rm(path, { recursive: true, force: true }))));

describe("設定遷移", () => {
  it("把舊版設定轉為 SettingsV2", () => {
    const result = migrateSettings({
      RecognitionEncoding: false,
      Prompt: false,
      MaxLengthPreview: 12,
      AssistiveTouch: false,
      PositionX: 100,
      PositionY: 200,
      HotKey: { AutoCopy: false, AutoPaste: true, Feature1: { Enable: true, Modift: "Control, Shift", Key: "F8", Action: "a1" } },
      QuickStart: { LeftClick_Ctrl: "a3", RightDrop_Shift: "ze2" },
      FileConvert: { DefaultPath: "D:\\Text", TypeFilter: "<文字|*.txt>", FixLabel: ".html|.php", UnicodeAddBOM: true },
      Fanhuaji_Setting: {
        Converter_S_to_T: 4,
        Converter_T_to_S: "Simplified",
        JpTextConversionStrategy: 0,
        JpStyleConversionStrategy: 1,
        IgnoreTextStyles: "code",
        JpTextStyles: "jp",
        CleanUpText: true,
        UserPreReplace: [{ Key: "甲", Value: "乙" }],
        Modules: [{ ModuleName: "TaiwanPhrase", Enable: true }],
      },
    });
    expect(result.version).toBe(2);
    expect(result.engine).toBe("segmented");
    expect(result.showMainWindowOnStart).toBe(false);
    expect(result.recognizeEncoding).toBe(false);
    expect(result.floatingBall).toEqual({ enabled: false, x: 100, y: 200 });
    expect(result.hotkeys.shortcuts[0]).toMatchObject({ enabled: true, accelerator: "Control+Shift+F8", action: "a1" });
    expect(result.quickActions.leftClickCtrl).toBe("a3");
    expect(result.quickActions.rightDropShift).toBe("ze2");
    expect(result.files.unicodeAddBom).toBe(true);
    expect(result.zhconvert).toMatchObject({
      converterS2T: "Taiwan",
      converterT2S: "Simplified",
      jpTextConversionStrategy: "protectOnlySameOrigin",
      jpStyleConversionStrategy: "none",
      ignoreTextStyles: "code",
      jpTextStyles: "jp",
      cleanUpText: true,
      userPreReplace: "甲=乙",
      modules: { TaiwanPhrase: 1 },
    });
  });

  it("匯入前建立不覆寫的時間戳備份", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-settings-"));
    temporary.push(directory);
    const source = join(directory, "ConvertZZ.json");
    await writeFile(source, "{\"Prompt\":false}", "utf8");
    const first = await backupLegacySettings(source);
    const second = await backupLegacySettings(source);
    expect(first).not.toBe(second);
    expect(await readFile(first, "utf8")).toBe("{\"Prompt\":false}");
    expect(await readFile(second, "utf8")).toBe("{\"Prompt\":false}");
    expect((await readdir(directory)).filter((name) => name.startsWith("ConvertZZ.backup-"))).toHaveLength(2);
  });

  it("舊版與缺少欄位的 2.0 設定預設不啟動主視窗", () => {
    expect(migrateSettings(undefined).showMainWindowOnStart).toBe(false);
    expect(migrateSettings({ version: 2, engine: "legacy" }).showMainWindowOnStart).toBe(false);
    expect(migrateSettings({ version: 2, showMainWindowOnStart: true }).showMainWindowOnStart).toBe(true);
  });
});

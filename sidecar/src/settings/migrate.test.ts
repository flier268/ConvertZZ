import { afterEach, describe, expect, it } from "vitest";
import { mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { migrateSettings, migrateSettingsFromPath } from "./migrate.js";

const temporary: string[] = [];
afterEach(async () =>
  Promise.all(temporary.splice(0).map((path) => rm(path, { recursive: true, force: true }))),
);

describe("設定遷移", () => {
  it("把舊版設定轉為 SettingsV2", () => {
    const result = migrateSettings({
      Engine: 1,
      RecognitionEncoding: false,
      Prompt: false,
      MaxLengthPreview: 12,
      AssistiveTouch: false,
      PositionX: 100,
      PositionY: 200,
      HotKey: {
        AutoCopy: false,
        AutoPaste: true,
        Feature1: { Enable: true, Modift: "Control, Shift", Key: "F8", Action: "a1" },
      },
      QuickStart: { LeftClick_Ctrl: "a3", RightDrop_Shift: "ze2" },
      FileConvert: {
        DefaultPath: "D:\\Text",
        TypeFilter: "<文字|*.txt>",
        FixLabel: ".html|.php",
        UnicodeAddBOM: true,
      },
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
    expect(result.engine).toBe("zhconvert");
    expect(result.showMainWindowOnStart).toBe(false);
    expect(result.recognizeEncoding).toBe(false);
    expect(result.floatingBall).toEqual({ enabled: false, x: 100, y: 200 });
    expect(result.hotkeys.shortcuts[0]).toMatchObject({
      enabled: true,
      accelerator: "Control+Shift+F8",
      action: "a1",
    });
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

  it("從路徑匯入只讀取來源，不修改也不另建備份", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-settings-"));
    temporary.push(directory);
    const source = join(directory, "ConvertZZ.json");
    const original = '{"Prompt":false}';
    await writeFile(source, original, "utf8");
    const result = await migrateSettingsFromPath(source);
    expect(result.promptAfterConversion).toBe(false);
    expect(await readFile(source, "utf8")).toBe(original);
    expect(await readdir(directory)).toEqual(["ConvertZZ.json"]);
  });

  it("讀取失敗時不寫入也不改變來源目錄", async () => {
    const directory = await mkdtemp(join(tmpdir(), "convertzz-settings-missing-"));
    temporary.push(directory);
    const source = join(directory, "ConvertZZ.json");
    await expect(migrateSettingsFromPath(source)).rejects.toBeTruthy();
    expect(await readdir(directory)).toEqual([]);
  });

  it("舊版 Local 引擎匯入為新式分詞，Fanhuaji 匯入為 ZhConvert", () => {
    expect(migrateSettings({ Engine: 0 }).engine).toBe("segmented");
    expect(migrateSettings({ Engine: "Local" }).engine).toBe("segmented");
    expect(migrateSettings({ Engine: "Fanhuaji" }).engine).toBe("zhconvert");
  });

  it("舊版與缺少欄位的 2.0 設定預設不啟動主視窗", () => {
    expect(migrateSettings(undefined).showMainWindowOnStart).toBe(false);
    expect(migrateSettings({ version: 2, engine: "legacy" }).showMainWindowOnStart).toBe(false);
    expect(migrateSettings({ version: 2, showMainWindowOnStart: true }).showMainWindowOnStart).toBe(
      true,
    );
  });

  it("缺少欄位的 2.0 設定預設不略過任何更新版本", () => {
    expect(migrateSettings(undefined).skippedUpdateVersion).toBe("");
    expect(migrateSettings({ version: 2, engine: "legacy" }).skippedUpdateVersion).toBe("");
    expect(
      migrateSettings({ version: 2, skippedUpdateVersion: "2.1.0" }).skippedUpdateVersion,
    ).toBe("2.1.0");
  });
});

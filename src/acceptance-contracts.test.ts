import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

function readProjectFile(path: string): string {
  return readFileSync(fileURLToPath(new URL(`../${path}`, import.meta.url)), "utf8");
}

describe("待人工驗收項目的畫面契約", () => {
  it("D-07 首次匯入前會詢問", () => {
    const settings = readProjectFile("src/lib/settings.ts");
    const tour = readProjectFile("src/OnboardingTour.vue");
    expect(settings).toContain("匯入會取代目前的 2.0 設定。是否繼續？");
    expect(settings).toContain('title: "確認匯入"');
    expect(tour).toContain("importLegacySettings");
    expect(tour).toContain("匯入找到的設定");
  });

  it("D-08／D-09 匯入只讀取舊 JSON，失敗時不覆寫目前設定", () => {
    const tour = readProjectFile("src/OnboardingTour.vue");
    const settings = readProjectFile("src/lib/settings.ts");
    expect(tour).toContain("來源檔不會被修改");
    expect(tour).toContain("onboarding-import-error");
    expect(tour).toContain("importFailureMessage");
    const importer = settings.slice(settings.indexOf("export async function importLegacySettings"));
    expect(importer).toContain("settings.migrate");
    expect(importer).not.toContain("settings.backup");
  });

  it("D-11 儲存字典前會詢問並備份", () => {
    const page = readProjectFile("src/pages/DictionaryPage.vue");
    expect(page).toContain("將先備份字典，再寫入");
    expect(page).toContain('title: "確認字典備份"');
    expect(page).toContain("dictionary.update");
  });

  it("E-01／E-02 預覽會顯示來源、輸出、編碼與差異", () => {
    const page = readProjectFile("src/pages/FilesPage.vue");
    expect(page).toContain('label="來源"');
    expect(page).toContain('label="輸出"');
    expect(page).toContain('label="編碼"');
    expect(page).toContain('prop="sourcePreview"');
    expect(page).toContain('prop="outputPreview"');
    expect(page).toContain('label="轉換檔名"');
  });

  it("E-05 覆寫需要額外確認", () => {
    const page = readProjectFile("src/pages/FilesPage.vue");
    expect(page).toContain('conflictPolicy === "overwrite"');
    expect(page).toContain("覆寫會取代既有的同名檔案。是否確定繼續？");
    expect(page).toContain('title: "確認覆寫"');
    expect(page).toContain('label="略過"');
  });

  it("F-07 APEv2 與 Vorbis 不顯示 ID3 編碼選項", () => {
    const page = readProjectFile("src/pages/AudioPage.vue");
    const id3Block = page.slice(page.indexOf('v-if="hasMp3"'));
    expect(id3Block).toContain("ID3v2 版本");
    expect(id3Block).toContain("ID3v2 編碼");
    expect(page.indexOf("ID3v2 編碼")).toBeGreaterThan(page.indexOf('v-if="hasMp3"'));
    expect(page).toContain("APE");
    expect(page).toContain("Vorbis");
  });

  it("F-08 預覽列出所有可轉換字串欄位", () => {
    const page = readProjectFile("src/pages/AudioPage.vue");
    expect(page).toContain('v-for="file in files"');
    expect(page).toContain(':data="file.fields"');
    expect(page).toContain("scope.row.values.join");
    expect(page).toContain("convertedValues");
  });

  it("G-12 快捷鍵、托盤與浮動球走同一動作路由", () => {
    const desktop = readProjectFile("src/lib/desktop.ts");
    const app = readProjectFile("src/App.vue");
    const floating = readProjectFile("src/FloatingBall.vue");
    expect(desktop).toContain("executeLegacyAction(shortcut.action, settings");
    expect(app).toContain('listen<string>("app://legacy-action"');
    expect(app).toContain("executeLegacyAction(payload, await loadSettings())");
    expect(floating).toContain("executeLegacyAction(action, await loadSettings(), input)");
  });

  it("H-03 /file 會自動建立檔案預覽", () => {
    const page = readProjectFile("src/pages/FilesPage.vue");
    expect(page).toContain('invocation?.options.mode !== "file"');
    expect(page).toContain("await createPlan()");
  });

  it("H-06 SendTo 只在 Windows 出現", () => {
    const rust = readProjectFile("src-tauri/src/lib.rs");
    const settings = readProjectFile("src/pages/SettingsPage.vue");
    expect(rust).toContain("send_to_shortcut: true");
    expect(rust).toContain("send_to_shortcut: false");
    expect(rust).toContain('Err("SendTo 只支援 Windows。".into())');
    expect(settings).toContain("capabilities?.sendToShortcut");
  });

  it("I-01 Windows 快捷鍵會先放開修飾鍵再送 Ctrl+C／V", () => {
    const rust = readProjectFile("src-tauri/src/selection.rs");
    expect(rust).toContain("prepare_keyboard");
    expect(rust).toContain("release_held_modifiers");
    expect(rust).toContain("send_shortcut(Key::Control, Key::C)");
    expect(rust).toContain("send_shortcut(Key::Control, Key::V)");
    expect(rust).toContain("GetClipboardSequenceNumber");
    expect(rust).not.toContain("WM_COPY");
    expect(rust).not.toContain("WM_PASTE");
  });

  it("I-03 Wayland 不會嘗試鍵盤注入", () => {
    const rust = readProjectFile("src-tauri/src/lib.rs");
    expect(rust).toContain("automatic_copy_paste: !wayland");
    expect(rust).toContain("if !platform_capabilities().automatic_copy_paste");
    expect(rust).toContain("目前顯示伺服器不允許自動讀寫選取文字。");
    expect(rust).not.toContain("simulate_copy_paste");
    expect(rust).not.toContain("Key::Control");
    expect(readProjectFile("src/lib/legacyActions.ts")).toContain(
      'invoke<string>("capture_selection")',
    );
    expect(readProjectFile("src/lib/legacyActions.ts")).toContain('invoke("replace_selection"');
  });

  it("I-04／I-05 Wayland 限制會顯示在關於頁", () => {
    const about = readProjectFile("src/pages/AboutPage.vue");
    const rust = readProjectFile("src-tauri/src/lib.rs");
    expect(about).toContain("本版停用");
    expect(about).toContain("依合成器");
    expect(rust).toContain("本版停用 Wayland 全域快捷鍵。");
    expect(rust).toContain("浮動球置頂能力取決於合成器。");
  });
});

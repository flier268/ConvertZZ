import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { collectMenuActionIds, FLOATING_CONTEXT_MENU } from "./lib/appMenu";

function readProjectFile(path: string): string {
  return readFileSync(fileURLToPath(new URL(`../${path}`, import.meta.url)), "utf8");
}

function trayMenuIds(rust: string): string[] {
  const start = rust.indexOf("fn install_tray");
  const end = rust.indexOf("fn hide_startup_windows", start);
  const body = rust.slice(start, end);
  return [...body.matchAll(/\.text\("([^"]+)"/g)].map((match) => match[1]!);
}

describe("驗收項目的自動化契約", () => {
  it("D-07 首次匯入前會詢問", () => {
    const settings = readProjectFile("src/lib/settings.ts");
    const tour = readProjectFile("src/OnboardingTour.vue");
    const page = readProjectFile("src/pages/SettingsPage.vue");
    expect(settings).toContain("匯入會取代目前的 2.0 設定。是否繼續？");
    expect(settings).toContain('title: "確認匯入"');
    expect(tour).toContain("importLegacySettings");
    expect(tour).toContain("匯入找到的設定");
    expect(page).toContain("匯入 ConvertZZ.json");
    expect(page).toContain("importLegacySettings");
  });

  it("D-08／D-09 匯入只讀取舊 JSON，失敗時不覆寫目前設定", () => {
    const tour = readProjectFile("src/OnboardingTour.vue");
    const settings = readProjectFile("src/lib/settings.ts");
    const page = readProjectFile("src/pages/SettingsPage.vue");
    expect(tour).toContain("來源檔不會被修改");
    expect(tour).toContain("onboarding-import-error");
    expect(tour).toContain("importFailureMessage");
    expect(page).toContain("importFailureMessage");
    const importer = settings.slice(settings.indexOf("export async function importLegacySettings"));
    expect(importer).toContain("settings.migrate");
    expect(importer).not.toContain("settings.backup");
    expect(settings).toContain("notifySettingsReplaced");
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

  it("音訊備份衝突預設略過，覆寫需額外確認", () => {
    const page = readProjectFile("src/pages/AudioPage.vue");
    expect(page).toContain('ref<AudioTagPlanRequest["conflictPolicy"]>("skip")');
    expect(page).toContain('label="備份衝突"');
    expect(page).toContain('label="略過"');
    expect(page).toContain('value="overwrite"');
    expect(page).toContain('conflictPolicy.value === "overwrite"');
    expect(page).toContain("覆寫會取代既有的 .bak 備份。是否確定繼續？");
    expect(page).toContain('title: "確認覆寫備份"');
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

  it("G-02／G-05 浮動球為獨立透明視窗且無白底外框", () => {
    const config = JSON.parse(readProjectFile("src-tauri/tauri.conf.json")) as {
      app?: {
        windows?: Array<{
          label?: string;
          transparent?: boolean;
          decorations?: boolean;
          shadow?: boolean;
          backgroundColor?: number[];
          width?: number;
          height?: number;
        }>;
      };
    };
    const floating = config.app?.windows?.find((window) => window.label === "floating");
    const styles = readProjectFile("src/styles.css");
    const ball = readProjectFile("src/FloatingBall.vue");
    expect(floating?.transparent).toBe(true);
    expect(floating?.decorations).toBe(false);
    expect(floating?.shadow).toBe(false);
    expect(floating?.backgroundColor).toEqual([0, 0, 0, 0]);
    expect(floating?.width).toBe(72);
    expect(floating?.height).toBe(72);
    expect(styles).toContain("html.floating-window");
    expect(styles).toContain("background: transparent !important");
    expect(styles).toContain(".floating-orb");
    expect(styles).toContain("background: transparent");
    expect(ball).toContain("BrandMark");
    expect(ball).not.toContain("drop-shadow");
  });

  it("G-04 浮動球位置會寫回設定並在套用時還原", () => {
    const ball = readProjectFile("src/FloatingBall.vue");
    const desktop = readProjectFile("src/lib/desktop.ts");
    expect(ball).toContain("onMoved");
    expect(ball).toContain("settings.floatingBall.x");
    expect(ball).toContain("settings.floatingBall.y");
    expect(ball).toContain("patchSavedSettings");
    expect(desktop).toContain("floatingBallPosition");
    expect(desktop).toContain("setPosition");
    expect(desktop).toContain("LogicalPosition");
  });

  it("G-07／G-08／G-09 托盤會裝圖示、左鍵開主視窗、右鍵走選單", () => {
    const rust = readProjectFile("src-tauri/src/lib.rs");
    expect(rust).toContain('TrayIconBuilder::with_id("main-tray")');
    expect(rust).toContain("default_window_icon");
    expect(rust).toContain('.tooltip("ConvertZZ 2.0")');
    expect(rust).toContain("show_menu_on_left_click(false)");
    expect(rust).toContain("MouseButton::Left");
    expect(rust).toContain("show_main(tray.app_handle())");
    expect(rust).toContain('.text("show", "開啟 ConvertZZ")');
    expect(rust).toContain('"show" => show_main(app)');
    expect(rust).toContain(".menu(&menu)");
  });

  it("G-10 關閉主視窗只會隱藏，不會結束程序", () => {
    const rust = readProjectFile("src-tauri/src/lib.rs");
    expect(rust).toContain("WindowEvent::CloseRequested");
    expect(rust).toContain("api.prevent_close()");
    expect(rust).toContain("window_to_hide.hide()");
  });

  it("G-11 主視窗與浮動球皆有剪貼簿 capability", () => {
    const main = JSON.parse(readProjectFile("src-tauri/capabilities/default.json")) as {
      permissions?: string[];
    };
    const floating = JSON.parse(readProjectFile("src-tauri/capabilities/floating.json")) as {
      permissions?: string[];
    };
    const required = ["clipboard-manager:allow-read-text", "clipboard-manager:allow-write-text"];
    expect(main.permissions).toEqual(expect.arrayContaining(required));
    expect(floating.permissions).toEqual(expect.arrayContaining(required));
    expect(readProjectFile("src/lib/legacyActions.ts")).toContain("readText");
    expect(readProjectFile("src/lib/legacyActions.ts")).toContain("writeText");
  });

  it("G-12 快捷鍵、托盤與浮動球走同一動作路由", () => {
    const desktop = readProjectFile("src/lib/desktop.ts");
    const app = readProjectFile("src/App.vue");
    const floating = readProjectFile("src/FloatingBall.vue");
    const rust = readProjectFile("src-tauri/src/lib.rs");
    const menuIds = collectMenuActionIds(FLOATING_CONTEXT_MENU);
    const trayIds = trayMenuIds(rust).filter((id) => id !== "show");
    expect(desktop).toContain("executeLegacyAction(shortcut.action, settings");
    expect(app).toContain('listen<string>("app://legacy-action"');
    expect(app).toContain("executeLegacyAction(payload, await loadSettings())");
    expect(floating).toContain("executeLegacyAction(action, await loadSettings(), input)");
    expect(rust).toContain('app.emit("app://legacy-action", id)');
    expect(new Set(trayIds)).toEqual(new Set(menuIds));
  });

  it("G-13 第二個程序會顯示既有視窗並轉交參數", () => {
    const rust = readProjectFile("src-tauri/src/lib.rs");
    expect(rust).toContain("tauri_plugin_single_instance::init");
    expect(rust).toContain("show_main(app)");
    expect(rust).toContain("startup_args");
  });

  it("G-15／G-16 更新先確認；簽署通道不可用時改開 Releases", () => {
    const update = readProjectFile("src/lib/update.ts");
    const appUpdate = readProjectFile("src/lib/appUpdate.ts");
    const settings = readProjectFile("src/pages/SettingsPage.vue");
    const contracts = readProjectFile("shared/contracts.ts");
    expect(update).toContain('kind: "install"');
    expect(update).toContain('kind: "open"');
    expect(update).toContain("includePreRelease");
    expect(update).toContain("簽署更新通道不可用時改走 GitHub Release");
    expect(appUpdate).toContain("是否下載並安裝");
    expect(appUpdate).toContain("此安裝方式無法自動更新");
    expect(appUpdate).toContain("downloadAndInstall");
    expect(appUpdate).toContain("openUrl(resolved.url)");
    expect(appUpdate).toContain("includePreRelease");
    expect(contracts).toContain("checkPreReleaseUpdates");
    expect(settings).toContain("checkPreReleaseUpdates");
    expect(settings).toContain("檢查開發／預發佈版本");
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
    expect(rust).toContain("GetFolderPath('SendTo')");
    expect(settings).toContain("capabilities?.sendToShortcut");
    expect(settings).toContain("set_send_to_shortcut");
  });

  it("I-01／I-02 支援平台會開啟快捷鍵與自動複製貼上能力", () => {
    const rust = readProjectFile("src-tauri/src/lib.rs");
    expect(rust).toMatch(/#\[cfg\(target_os = "windows"\)\][\s\S]*global_shortcuts: true/);
    expect(rust).toMatch(/#\[cfg\(target_os = "windows"\)\][\s\S]*automatic_copy_paste: true/);
    expect(rust).toContain("global_shortcuts: !wayland");
    expect(rust).toContain("automatic_copy_paste: !wayland");
    expect(readProjectFile("src/lib/desktop.ts")).toContain("if (!capabilities.globalShortcuts)");
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

  it("I-04／I-05／I-08 關於頁差異表與能力旗標一致", () => {
    const about = readProjectFile("src/pages/AboutPage.vue");
    const rust = readProjectFile("src-tauri/src/lib.rs");
    expect(about).toContain("本版停用");
    expect(about).toContain("依合成器");
    expect(about).toContain("SendTo 捷徑");
    expect(about).toContain("不適用");
    expect(about).toContain("需 AppIndicator");
    expect(about).toContain("Secret Service");
    expect(about).toContain("capabilities?.limitations");
    expect(rust).toContain("本版停用 Wayland 全域快捷鍵。");
    expect(rust).toContain("浮動球置頂能力取決於合成器。");
  });

  it("I-07 缺少 Secret Service 時只保留工作階段金鑰提示", () => {
    const rust = readProjectFile("src-tauri/src/lib.rs");
    expect(rust).toContain("DBUS_SESSION_BUS_ADDRESS");
    expect(rust).toContain('credential_storage: std::env::var_os("DBUS_SESSION_BUS_ADDRESS")');
    expect(rust).toContain("缺少 Secret Service 時 ZhConvert API 金鑰只保留於目前工作階段。");
  });
});

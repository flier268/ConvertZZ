mod core;
mod selection;

use core::{CoreError, CoreState, ProgressEvent};
use enigo::{Enigo, Mouse, Settings};
use keyring::Entry;
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{
    menu::{MenuBuilder, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_updater::UpdaterExt;

const PORTABLE_MARKER: &str = "portable";
const PORTABLE_SETTINGS_FILE: &str = "settings-v2.json";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlatformCapabilities {
    platform: &'static str,
    display_server: &'static str,
    global_shortcuts: bool,
    automatic_copy_paste: bool,
    floating_always_on_top: bool,
    tray: bool,
    send_to_shortcut: bool,
    credential_storage: bool,
    portable: bool,
    automatic_updates: bool,
    limitations: Vec<&'static str>,
}

fn executable_directory() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
}

fn directory_is_portable(directory: &Path) -> bool {
    directory.join(PORTABLE_MARKER).is_file()
}

fn is_portable_mode() -> bool {
    executable_directory()
        .map(|directory| directory_is_portable(&directory))
        .unwrap_or(false)
}

fn portable_settings_path() -> Result<PathBuf, String> {
    let directory = executable_directory().ok_or_else(|| "找不到執行檔目錄。".to_string())?;
    if !directory_is_portable(&directory) {
        return Err("目前不是免安裝可攜模式。".into());
    }
    Ok(directory.join(PORTABLE_SETTINGS_FILE))
}

fn write_portable_settings_document(path: &Path, document: &Value) -> Result<(), String> {
    let payload =
        serde_json::to_vec_pretty(document).map_err(|error| format!("序列化設定失敗：{error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, &payload).map_err(|error| format!("寫入暫存設定失敗：{error}"))?;
    if path.exists() {
        let backup = path.with_extension("json.bak");
        let _ = fs::remove_file(&backup);
        fs::rename(path, &backup).map_err(|error| format!("備份舊設定失敗：{error}"))?;
        if let Err(error) = fs::rename(&temporary, path) {
            let _ = fs::rename(&backup, path);
            let _ = fs::remove_file(&temporary);
            return Err(format!("取代設定檔失敗：{error}"));
        }
        let _ = fs::remove_file(&backup);
    } else if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!("寫入設定檔失敗：{error}"));
    }
    Ok(())
}

#[tauri::command]
async fn core_request(
    app: AppHandle,
    state: State<'_, Arc<CoreState>>,
    id: String,
    operation: String,
    payload: Value,
) -> Result<Value, CoreError> {
    let handle = app.clone();
    let request_id = id.clone();
    let progress = Arc::new(move |event: ProgressEvent| {
        let _ = handle.emit(
            "core://progress",
            serde_json::json!({
                "id": request_id,
                "current": event.current,
                "total": event.total,
                "message": event.message,
            }),
        );
    });
    match core::dispatch(Arc::clone(&state), &operation, payload, progress).await {
        Ok(result) => Ok(result),
        Err(error) => {
            append_log(&app, "core", &error.message);
            Err(error)
        }
    }
}

#[tauri::command]
fn app_log_path(app: AppHandle) -> Option<String> {
    log_file_path(&app).map(|path| path.display().to_string())
}

#[tauri::command]
fn app_log(app: AppHandle, source: String, message: String) {
    append_log(&app, &source, &message);
}

#[tauri::command]
fn startup_args() -> Vec<String> {
    std::env::args().skip(1).collect()
}

#[tauri::command]
fn legacy_settings_path() -> Option<String> {
    let mut candidates = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            candidates.push(directory.join("ConvertZZ.json"));
        }
    }
    if let Ok(directory) = std::env::current_dir() {
        candidates.push(directory.join("ConvertZZ.json"));
    }
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .map(|path| path.to_string_lossy().into_owned())
}

#[tauri::command]
fn platform_capabilities() -> PlatformCapabilities {
    let portable = is_portable_mode();
    let automatic_updates = !portable;
    #[cfg(target_os = "windows")]
    {
        let mut limitations = Vec::new();
        if portable {
            limitations.push("免安裝版設定寫在程式目錄，可整包帶走。");
            limitations.push("免安裝版不支援應用程式內自動更新，請改從 GitHub Releases 下載。");
        }
        return PlatformCapabilities {
            platform: "windows",
            display_server: "windows",
            global_shortcuts: true,
            automatic_copy_paste: true,
            floating_always_on_top: true,
            tray: true,
            send_to_shortcut: true,
            credential_storage: true,
            portable,
            automatic_updates,
            limitations,
        };
    }

    #[cfg(target_os = "linux")]
    {
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        return PlatformCapabilities {
            platform: "linux",
            display_server: if wayland { "wayland" } else { "x11" },
            global_shortcuts: !wayland,
            automatic_copy_paste: !wayland,
            floating_always_on_top: !wayland,
            tray: true,
            send_to_shortcut: false,
            credential_storage: std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some(),
            portable,
            automatic_updates,
            limitations: {
                let mut items = if wayland {
                    vec![
                        "Wayland 不允許一般應用程式注入鍵盤事件。",
                        "本版停用 Wayland 全域快捷鍵。",
                        "浮動球置頂能力取決於合成器。",
                    ]
                } else {
                    vec!["系統托盤需要 AppIndicator 支援。"]
                };
                if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
                    items.push("缺少 Secret Service 時 ZhConvert API 金鑰只保留於目前工作階段。");
                }
                if portable {
                    items.push("免安裝版設定寫在程式目錄，可整包帶走。");
                    items.push("免安裝版不支援應用程式內自動更新，請改從 GitHub Releases 下載。");
                }
                items
            },
        };
    }

    #[cfg(target_os = "macos")]
    {
        return PlatformCapabilities {
            platform: "macos",
            display_server: "macos",
            global_shortcuts: true,
            automatic_copy_paste: true,
            floating_always_on_top: true,
            tray: true,
            send_to_shortcut: false,
            credential_storage: true,
            portable,
            automatic_updates,
            limitations: vec!["自動複製貼上需要輔助使用（Accessibility）權限。"],
        };
    }

    #[allow(unreachable_code)]
    PlatformCapabilities {
        platform: "unknown",
        display_server: "unknown",
        global_shortcuts: false,
        automatic_copy_paste: false,
        floating_always_on_top: false,
        tray: false,
        send_to_shortcut: false,
        credential_storage: false,
        portable,
        automatic_updates,
        limitations: vec!["目前平台不在正式支援範圍。"],
    }
}

#[tauri::command]
fn load_portable_settings_store() -> Result<Option<Value>, String> {
    let path = portable_settings_path()?;
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(&path).map_err(|error| format!("讀取可攜設定失敗：{error}"))?;
    let document: Value =
        serde_json::from_slice(&bytes).map_err(|error| format!("可攜設定格式無效：{error}"))?;
    if !document.is_object() {
        return Err("可攜設定必須是 JSON 物件。".into());
    }
    Ok(Some(document))
}

#[tauri::command]
fn save_portable_settings_store(document: Value) -> Result<(), String> {
    if !document.is_object() {
        return Err("可攜設定必須是 JSON 物件。".into());
    }
    let path = portable_settings_path()?;
    write_portable_settings_document(&path, &document)
}

#[tauri::command]
fn capture_selection() -> Result<String, String> {
    if !platform_capabilities().automatic_copy_paste {
        return Err("目前顯示伺服器不允許自動讀寫選取文字。".into());
    }
    selection::capture()
}

#[tauri::command]
fn replace_selection(text: String) -> Result<(), String> {
    if !platform_capabilities().automatic_copy_paste {
        return Err("目前顯示伺服器不允許自動讀寫選取文字。".into());
    }
    selection::replace(text)
}

#[tauri::command]
fn save_zhconvert_api_key(api_key: String) -> Result<bool, String> {
    let entry = Entry::new("dev.flier268.convertzz", "zhconvert-api-key")
        .map_err(|error| error.to_string())?;
    match entry.set_password(&api_key) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[tauri::command]
fn load_zhconvert_api_key() -> Result<Option<String>, String> {
    let entry = Entry::new("dev.flier268.convertzz", "zhconvert-api-key")
        .map_err(|error| error.to_string())?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

#[tauri::command]
fn set_send_to_shortcut(enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let executable = std::env::current_exe()
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .replace('\'', "''");
        let script = if enabled {
            format!(
                "$p=[Environment]::GetFolderPath('SendTo');$w=New-Object -ComObject WScript.Shell;$s=$w.CreateShortcut((Join-Path $p 'ConvertZZ 文件.lnk'));$s.TargetPath='{executable}';$s.Arguments='/file';$s.Save();$a=$w.CreateShortcut((Join-Path $p 'ConvertZZ 音訊標籤.lnk'));$a.TargetPath='{executable}';$a.Arguments='/audio';$a.Save()"
            )
        } else {
            "$p=[Environment]::GetFolderPath('SendTo');Remove-Item -LiteralPath (Join-Path $p 'ConvertZZ 文件.lnk') -ErrorAction SilentlyContinue;Remove-Item -LiteralPath (Join-Path $p 'ConvertZZ 音訊標籤.lnk') -ErrorAction SilentlyContinue".into()
        };
        let status = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .status()
            .map_err(|error| error.to_string())?;
        return if status.success() {
            Ok(())
        } else {
            Err("無法更新 SendTo 捷徑。".into())
        };
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = enabled;
        Err("SendTo 只支援 Windows。".into())
    }
}

fn discover_dictionary(app: Option<&AppHandle>) -> Option<std::path::PathBuf> {
    let mut candidates = Vec::new();
    if let Some(app) = app {
        if let Ok(resource) = app.path().resource_dir() {
            candidates.push(resource.join("Dictionary.csv"));
        }
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            candidates.push(directory.join("Dictionary.csv"));
        }
    }
    candidates.push(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ConvertZZ/Dictionary.csv"),
    );
    candidates.push(std::path::PathBuf::from("ConvertZZ/Dictionary.csv"));
    candidates.into_iter().find(|path| path.is_file())
}

fn log_file_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_log_dir()
        .ok()
        .map(|dir| dir.join("convertzz.log"))
}

fn append_log(app: &AppHandle, source: &str, message: &str) {
    // 空訊息仍要落盤，否則首次啟動失敗時 log 會完全沒有線索。
    let message = if message.trim().is_empty() {
        "(empty message)"
    } else {
        message
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let line = format!(
        "{}.{:03} [{source}] {message}",
        now.as_secs(),
        now.subsec_millis()
    );
    eprintln!("{line}");
    let Some(path) = log_file_path(app) else {
        return;
    };
    if let Some(directory) = path.parent() {
        let _ = std::fs::create_dir_all(directory);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        use std::io::Write;
        let _ = writeln!(file, "{line}");
    }
}

fn create_configured_windows(app: &tauri::App) -> tauri::Result<()> {
    let configs = app.config().app.windows.clone();
    for config in &configs {
        WebviewWindowBuilder::from_config(app.handle(), config)?.build()?;
    }
    Ok(())
}

fn install_tray(app: &tauri::App) -> tauri::Result<()> {
    let audio = SubmenuBuilder::new(app, "Audio 標籤轉換")
        .text("c1", "ID3")
        .text("c2", "APE")
        .text("c3", "OGG")
        .build()?;
    let others = SubmenuBuilder::new(app, "其他")
        .text("za1", "Unicode → HTML 十進位")
        .text("za2", "Unicode → HTML 十六進位")
        .text("za3", "HTML → Unicode")
        .separator()
        .text("zb1", "Unicode → GBK")
        .text("zb2", "Unicode → Big5")
        .text("zb3", "Unicode → Shift-JIS")
        .text("zb4", "GBK → Unicode")
        .text("zb5", "Big5 → Unicode")
        .text("zb6", "Shift-JIS → Unicode")
        .separator()
        .text("zc1", "Shift-JIS → GBK")
        .text("zc2", "Shift-JIS → Big5")
        .text("zc3", "GBK → Shift-JIS")
        .text("zc4", "Big5 → Shift-JIS")
        .separator()
        .text("zd1", "HZ → GBK")
        .text("zd2", "HZ → Big5")
        .text("zd3", "GBK → HZ")
        .text("zd4", "Big5 → HZ")
        .separator()
        .text("ze1", "半形 → 全形")
        .text("ze2", "全形 → 半形")
        .build()?;
    let help = SubmenuBuilder::new(app, "說明")
        .text("about", "關於 ConvertZZ")
        .text("report", "回報問題")
        .build()?;
    let menu = MenuBuilder::new(app)
        .text("show", "開啟 ConvertZZ")
        .separator()
        .text("a1", "GBK → Big5")
        .text("a2", "Big5 → GBK")
        .text("a3", "Unicode 簡 → Unicode 繁")
        .text("a4", "Unicode 繁 → Unicode 簡")
        .separator()
        .text("b1", "文件/檔名轉換")
        .text("b2", "剪貼簿轉換")
        .separator()
        .item(&audio)
        .separator()
        .item(&others)
        .text("1", "隱藏或顯示浮動球")
        .text("settings", "設定")
        .item(&help)
        .text("quit", "結束 ConvertZZ")
        .build()?;
    let tray = TrayIconBuilder::with_id("main-tray")
        .tooltip("ConvertZZ 2.0")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => app.exit(0),
            "show" => show_main(app),
            id => {
                let _ = app.emit("app://legacy-action", id);
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    let tray = if let Some(icon) = app.default_window_icon() {
        tray.icon(icon.clone())
    } else {
        tray
    };
    tray.build(app)?;
    Ok(())
}

fn hide_startup_windows(app: &tauri::App) {
    // main 在設定裡已是 visible:false。啟動時再 hide 一次，會與前端首次導覽的
    // show_main_window 在 Windows 上競態，造成主視窗閃過後被藏起、只剩懸浮球。
    for label in ["floating", "toast"] {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.hide();
        }
    }
    if let Some(floating) = app.get_webview_window("floating") {
        let _ = floating.set_size(tauri::LogicalSize::new(72.0, 72.0));
    }
}

fn clear_overlay_window_backgrounds(app: &tauri::App) {
    for label in ["floating", "toast"] {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
        }
    }
}

fn keep_main_available_from_tray(app: &tauri::App) {
    if let Some(window) = app.get_webview_window("main") {
        let window_to_hide = window.clone();
        window.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window_to_hide.hide();
            }
        });
    }
}

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        // 僅 show() 無法還原已最小化的視窗（Linux／Windows）。
        if window.is_minimized().unwrap_or(false) {
            let _ = window.unminimize();
        }
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
fn show_main_window(app: AppHandle) {
    show_main(&app);
}

const TOAST_OFFSET_PX: i32 = 16;
const TOAST_WIDTH: i32 = 280;
const TOAST_HEIGHT: i32 = 140;

fn cursor_position() -> Option<(i32, i32)> {
    Enigo::new(&Settings::default()).ok()?.location().ok()
}

fn clamp_to_monitor(app: &AppHandle, x: i32, y: i32, width: i32, height: i32) -> (i32, i32) {
    let monitors = app.available_monitors().unwrap_or_default();
    let monitor = monitors
        .iter()
        .find(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            x >= position.x
                && y >= position.y
                && x < position.x + size.width as i32
                && y < position.y + size.height as i32
        })
        .or_else(|| monitors.first());
    let Some(monitor) = monitor else {
        return (x, y);
    };
    let position = monitor.position();
    let size = monitor.size();
    let max_x = position.x + size.width as i32 - width;
    let max_y = position.y + size.height as i32 - height;
    (x.min(max_x).max(position.x), y.min(max_y).max(position.y))
}

fn place_toast_near_cursor(app: &AppHandle, window: &tauri::WebviewWindow) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let width = (f64::from(TOAST_WIDTH) * scale).round() as i32;
    let height = (f64::from(TOAST_HEIGHT) * scale).round() as i32;
    let offset = (f64::from(TOAST_OFFSET_PX) * scale).round() as i32;
    let (x, y) = if let Some((cursor_x, cursor_y)) = cursor_position() {
        (cursor_x + offset, cursor_y + offset)
    } else if let Some(floating) = app.get_webview_window("floating") {
        match floating.outer_position() {
            Ok(position) => (position.x + offset, position.y + offset),
            Err(_) => return,
        }
    } else {
        return;
    };
    let (x, y) = clamp_to_monitor(app, x, y, width, height);
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

#[tauri::command]
fn show_toast(app: AppHandle, message: String) {
    if let Some(window) = app.get_webview_window("toast") {
        place_toast_near_cursor(&app, &window);
    }
    let _ = app.emit("app://toast", message);
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SignedUpdateMetadata {
    rid: tauri::ResourceId,
    current_version: String,
    version: String,
    body: Option<String>,
    raw_json: Value,
}

#[tauri::command]
async fn check_signed_update(
    webview: tauri::Webview,
    endpoint: Option<String>,
) -> Result<Option<SignedUpdateMetadata>, String> {
    let mut builder = webview.updater_builder();
    if let Some(endpoint) = endpoint {
        let url = url::Url::parse(&endpoint).map_err(|error| format!("更新端點無效：{error}"))?;
        builder = builder
            .endpoints(vec![url])
            .map_err(|error| error.to_string())?;
    }
    let updater = builder.build().map_err(|error| error.to_string())?;
    let Some(update) = updater.check().await.map_err(|error| error.to_string())? else {
        return Ok(None);
    };
    Ok(Some(SignedUpdateMetadata {
        current_version: update.current_version.clone(),
        version: update.version.clone(),
        body: update.body.clone(),
        raw_json: update.raw_json.clone(),
        rid: webview.resources_table().add(update),
    }))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Arc::new(
            CoreState::new(discover_dictionary(None)).expect("無法初始化轉換核心"),
        ))
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            show_main(app);
            let _ = app.emit("app://second-instance", args);
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // Release webviews load local files immediately; this plugin must exist first.
            if let Err(error) = app
                .handle()
                .plugin(tauri_plugin_global_shortcut::Builder::new().build())
            {
                append_log(app.handle(), "global-shortcut", &error.to_string());
            }
            create_configured_windows(app)?;
            hide_startup_windows(app);
            clear_overlay_window_backgrounds(app);
            keep_main_available_from_tray(app);
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| install_tray(app))) {
                Err(error) => append_log(app.handle(), "tray", &format!("panic: {error:?}")),
                Ok(Err(error)) => append_log(app.handle(), "tray", &error.to_string()),
                Ok(Ok(())) => {}
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            core_request,
            app_log_path,
            app_log,
            startup_args,
            legacy_settings_path,
            platform_capabilities,
            load_portable_settings_store,
            save_portable_settings_store,
            capture_selection,
            replace_selection,
            save_zhconvert_api_key,
            load_zhconvert_api_key,
            set_send_to_shortcut,
            show_main_window,
            show_toast,
            quit_app,
            check_signed_update,
        ])
        .run(tauri::generate_context!())
        .expect("ConvertZZ failed to start");
}

#[cfg(test)]
mod portable_settings_tests {
    use super::{directory_is_portable, write_portable_settings_document, PORTABLE_MARKER};
    use serde_json::json;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("convertzz-portable-{label}-{nanos}"));
        fs::create_dir_all(&path).expect("temp dir");
        path
    }

    #[test]
    fn detects_portable_marker_beside_executable_dir() {
        let directory = temp_dir("marker");
        assert!(!directory_is_portable(&directory));
        fs::write(directory.join(PORTABLE_MARKER), "").expect("marker");
        assert!(directory_is_portable(&directory));
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn writes_settings_document_atomically() {
        let directory = temp_dir("write");
        let path = directory.join("settings-v2.json");
        write_portable_settings_document(&path, &json!({ "settings": { "version": 2 } }))
            .expect("first write");
        write_portable_settings_document(
            &path,
            &json!({ "settings": { "version": 2 }, "onboardingCompleted": true }),
        )
        .expect("second write");
        let text = fs::read_to_string(&path).expect("read");
        assert!(text.contains("onboardingCompleted"));
        assert!(!directory.join("settings-v2.json.tmp").exists());
        assert!(!directory.join("settings-v2.json.bak").exists());
        let _ = fs::remove_dir_all(directory);
    }
}

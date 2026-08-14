use enigo::{Direction, Enigo, Key, Keyboard, Mouse, Settings};
use keyring::Entry;
use serde::Serialize;
use std::sync::Arc;
use tauri::{
    menu::{MenuBuilder, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, State, WindowEvent,
};
use tauri_plugin_shell::{
    process::{CommandChild, CommandEvent},
    ShellExt,
};
use tokio::sync::Mutex;

struct SidecarState {
    child: Arc<Mutex<CommandChild>>,
}

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
    limitations: Vec<&'static str>,
}

#[tauri::command]
async fn sidecar_send(state: State<'_, SidecarState>, request: String) -> Result<(), String> {
    let mut child = state.child.lock().await;
    child
        .write(format!("{request}\n").as_bytes())
        .map_err(|error| error.to_string())
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
    #[cfg(target_os = "windows")]
    {
        return PlatformCapabilities {
            platform: "windows",
            display_server: "windows",
            global_shortcuts: true,
            automatic_copy_paste: true,
            floating_always_on_top: true,
            tray: true,
            send_to_shortcut: true,
            credential_storage: true,
            limitations: vec![],
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
                items
            },
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
        limitations: vec!["目前平台不在正式支援範圍。"],
    }
}

#[tauri::command]
fn simulate_copy_paste(action: String) -> Result<(), String> {
    let capabilities = platform_capabilities();
    if !capabilities.automatic_copy_paste {
        return Err("目前顯示伺服器不允許自動鍵盤操作。".into());
    }
    let character = match action.as_str() {
        "copy" => 'c',
        "paste" => 'v',
        _ => return Err("不支援的鍵盤動作。".into()),
    };
    let mut enigo = Enigo::new(&Settings::default()).map_err(|error| error.to_string())?;
    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|error| error.to_string())?;
    let action_result = enigo
        .key(Key::Unicode(character), Direction::Click)
        .map_err(|error| error.to_string());
    let release_result = enigo
        .key(Key::Control, Direction::Release)
        .map_err(|error| error.to_string());
    action_result?;
    release_result?;
    Ok(())
}

#[tauri::command]
fn save_zhconvert_api_key(api_key: String) -> Result<bool, String> {
    let entry =
        Entry::new("org.convertzz.app", "zhconvert-api-key").map_err(|error| error.to_string())?;
    match entry.set_password(&api_key) {
        Ok(()) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[tauri::command]
fn load_zhconvert_api_key() -> Result<Option<String>, String> {
    let entry =
        Entry::new("org.convertzz.app", "zhconvert-api-key").map_err(|error| error.to_string())?;
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
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let executable = executable.to_string_lossy().replace('\'', "''");
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

fn start_sidecar(app: &AppHandle) -> Result<SidecarState, Box<dyn std::error::Error>> {
    let resource_dir = app.path().resource_dir()?;
    let dictionary = resource_dir.join("Dictionary.csv");
    let wasm = resource_dir.join("taglib-wasi.wasm");
    let arguments = vec![
        "--dictionary".to_string(),
        dictionary.to_string_lossy().into_owned(),
        "--wasm".to_string(),
        wasm.to_string_lossy().into_owned(),
    ];
    #[cfg(target_os = "linux")]
    let command = {
        let source = resource_dir.join("convertzz-sidecar.gz");
        let checksum = resource_dir.join("convertzz-sidecar.sha256");
        let cache_dir = app.path().app_cache_dir()?.join("sidecar");
        app.shell()
            .command(prepare_linux_sidecar(&source, &checksum, &cache_dir)?)
            .args(arguments)
    };
    #[cfg(not(target_os = "linux"))]
    let command = app.shell().sidecar("convertzz-sidecar")?.args(arguments);
    let (mut events, child) = command.spawn()?;
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut pending = String::new();
        while let Some(event) = events.recv().await {
            match event {
                CommandEvent::Stdout(bytes) => {
                    pending.push_str(&String::from_utf8_lossy(&bytes));
                    while let Some(position) = pending.find('\n') {
                        let line = pending[..position].trim().to_string();
                        pending.drain(..=position);
                        if !line.is_empty() {
                            let _ = handle.emit("sidecar://message", line);
                        }
                    }
                }
                CommandEvent::Stderr(bytes) => {
                    eprintln!("[convertzz-sidecar] {}", String::from_utf8_lossy(&bytes))
                }
                CommandEvent::Error(error) => {
                    let _ = handle.emit("sidecar://error", error);
                }
                CommandEvent::Terminated(payload) => {
                    let _ = handle.emit("sidecar://terminated", payload.code);
                }
                _ => {}
            }
        }
    });
    Ok(SidecarState {
        child: Arc::new(Mutex::new(child)),
    })
}

#[cfg(target_os = "linux")]
fn prepare_linux_sidecar(
    source: &std::path::Path,
    checksum: &std::path::Path,
    cache_dir: &std::path::Path,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    use flate2::read::GzDecoder;
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};
    use std::os::unix::fs::PermissionsExt;

    let expected_hash = std::fs::read_to_string(checksum)?
        .trim()
        .to_ascii_lowercase();
    if expected_hash.len() != 64 || !expected_hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Linux sidecar SHA-256 格式無效。".into());
    }
    std::fs::create_dir_all(&cache_dir)?;

    let destination = cache_dir.join(format!("convertzz-sidecar-{expected_hash}"));
    let cache_is_valid = destination
        .symlink_metadata()
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
        && matches!(sha256_file(&destination), Ok(actual_hash) if actual_hash == expected_hash);
    if cache_is_valid {
        let mut permissions = std::fs::metadata(&destination)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&destination, permissions)?;
        return Ok(destination);
    }

    let temporary = cache_dir.join(format!(".convertzz-sidecar-{}.tmp", uuid::Uuid::new_v4()));
    let prepared = (|| -> Result<(), Box<dyn std::error::Error>> {
        let input = std::fs::File::open(source)?;
        let mut decoder = GzDecoder::new(input);
        let mut output = std::fs::File::create(&temporary)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = decoder.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            output.write_all(&buffer[..count])?;
            hasher.update(&buffer[..count]);
        }
        output.sync_all()?;
        if format!("{:x}", hasher.finalize()) != expected_hash {
            return Err("Linux sidecar 解壓後的 SHA-256 不符。".into());
        }
        let mut permissions = std::fs::metadata(&temporary)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&temporary, permissions)?;
        std::fs::rename(&temporary, &destination)?;
        Ok(())
    })();
    if prepared.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    prepared?;
    Ok(destination)
}

#[cfg(target_os = "linux")]
fn sha256_file(path: &std::path::Path) -> Result<String, Box<dyn std::error::Error>> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let mut input = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(all(test, target_os = "linux"))]
mod linux_sidecar_tests {
    use super::prepare_linux_sidecar;
    use flate2::{write::GzEncoder, Compression};
    use sha2::{Digest, Sha256};
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn rebuilds_a_damaged_content_addressed_cache() {
        let root =
            std::env::temp_dir().join(format!("convertzz-sidecar-test-{}", uuid::Uuid::new_v4()));
        let source = root.join("convertzz-sidecar.gz");
        let checksum = root.join("convertzz-sidecar.sha256");
        let cache = root.join("cache");
        let fixture = b"ConvertZZ sidecar fixture";
        std::fs::create_dir(&root).expect("create test root");

        let compressed = std::fs::File::create(&source).expect("create gzip resource");
        let mut encoder = GzEncoder::new(compressed, Compression::best());
        encoder.write_all(fixture).expect("compress fixture");
        encoder.finish().expect("finish gzip resource");
        std::fs::write(&checksum, format!("{:x}\n", Sha256::digest(fixture)))
            .expect("write checksum");

        let destination =
            prepare_linux_sidecar(&source, &checksum, &cache).expect("prepare sidecar");
        assert_eq!(std::fs::read(&destination).expect("read cache"), fixture);
        assert_eq!(
            std::fs::metadata(&destination)
                .expect("read permissions")
                .permissions()
                .mode()
                & 0o777,
            0o755
        );

        std::fs::write(&destination, b"damaged").expect("damage cache");
        let rebuilt = prepare_linux_sidecar(&source, &checksum, &cache).expect("rebuild cache");
        assert_eq!(rebuilt, destination);
        assert_eq!(
            std::fs::read(&rebuilt).expect("read rebuilt cache"),
            fixture
        );

        std::fs::remove_dir_all(&root).expect("remove test root");
    }
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
    for label in ["main", "floating", "toast"] {
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            show_main(app);
            let _ = app.emit("app://second-instance", args);
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            app.handle()
                .plugin(tauri_plugin_global_shortcut::Builder::new().build())?;
            hide_startup_windows(app);
            clear_overlay_window_backgrounds(app);
            keep_main_available_from_tray(app);
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| install_tray(app))) {
                Err(error) => eprintln!("[convertzz-tray] 無法載入系統托盤：{error:?}"),
                Ok(Err(error)) => eprintln!("[convertzz-tray] 無法建立系統托盤：{error}"),
                Ok(Ok(())) => {}
            }
            let sidecar = start_sidecar(app.handle()).map_err(|error| error.to_string())?;
            app.manage(sidecar);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            sidecar_send,
            startup_args,
            legacy_settings_path,
            platform_capabilities,
            simulate_copy_paste,
            save_zhconvert_api_key,
            load_zhconvert_api_key,
            set_send_to_shortcut,
            show_main_window,
            show_toast,
            quit_app,
        ])
        .run(tauri::generate_context!())
        .expect("ConvertZZ failed to start");
}

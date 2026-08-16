use arboard::Clipboard;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
#[cfg(any(test, target_os = "linux", target_os = "windows"))]
use std::time::Instant;

#[cfg(any(target_os = "linux", target_os = "macos"))]
const CLIPBOARD_TIMEOUT: Duration = Duration::from_millis(300);
#[cfg(target_os = "windows")]
const CLIPBOARD_TIMEOUT: Duration = Duration::from_millis(500);
#[cfg(target_os = "linux")]
const MODIFIER_WAIT: Duration = Duration::from_millis(2000);
#[cfg(target_os = "windows")]
const MODIFIER_WAIT: Duration = Duration::from_millis(300);
#[cfg(any(target_os = "linux", target_os = "macos"))]
const PASTE_TIMEOUT: Duration = Duration::from_millis(2500);
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
const KEY_PRESS_TIME: Duration = Duration::from_millis(20);
#[cfg(any(target_os = "linux", target_os = "windows"))]
const MODIFIER_POLL: Duration = Duration::from_millis(20);

/// Shift / Control / Alt / Hyper / Super / ISO-Level3. CapsLock 與 NumLock 不算。
#[cfg(any(test, target_os = "linux"))]
const LEFTOVER_MODIFIER_BITS: u16 = (1 << 0) | (1 << 2) | (1 << 3) | (1 << 5) | (1 << 6) | (1 << 7);

#[cfg(any(test, target_os = "linux"))]
fn leftover_modifiers(mask: u16) -> bool {
    mask & LEFTOVER_MODIFIER_BITS != 0
}

pub fn capture() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        return windows::capture();
    }
    #[cfg(target_os = "linux")]
    {
        return linux::capture();
    }
    #[cfg(target_os = "macos")]
    {
        return macos::capture();
    }
    #[allow(unreachable_code)]
    Err("目前平台不支援讀取選取文字。".into())
}

pub fn replace(text: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        return windows::replace(&text);
    }
    #[cfg(target_os = "linux")]
    {
        return linux::replace(&text);
    }
    #[cfg(target_os = "macos")]
    {
        return macos::replace(&text);
    }
    #[allow(unreachable_code)]
    Err("目前平台不支援寫回選取文字。".into())
}

fn with_clipboard<T>(
    operation: impl FnOnce(&mut Clipboard) -> Result<T, String>,
) -> Result<T, String> {
    static CLIPBOARD: OnceLock<Mutex<Option<Clipboard>>> = OnceLock::new();
    let slot = CLIPBOARD.get_or_init(|| Mutex::new(None));
    let mut guard = slot.lock().map_err(|_| "無法鎖定剪貼簿。".to_string())?;
    if guard.is_none() {
        *guard = Some(Clipboard::new().map_err(|error| error.to_string())?);
    }
    operation(
        guard
            .as_mut()
            .ok_or_else(|| "剪貼簿尚未初始化。".to_string())?,
    )
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn run_with_timeout<T: Send + 'static>(
    timeout: Duration,
    timeout_message: impl Into<String>,
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("convertzz-selection".into())
        .spawn(move || {
            let _ = sender.send(work());
        })
        .map_err(|error| error.to_string())?;
    receiver
        .recv_timeout(timeout)
        .map_err(|_| timeout_message.into())
}

#[cfg(any(test, target_os = "linux", target_os = "windows"))]
fn wait_while(
    timeout: Duration,
    interval: Duration,
    mut still_waiting: impl FnMut() -> Result<bool, String>,
    timeout_message: &str,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        if !still_waiting()? {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(timeout_message.into());
        }
        std::thread::sleep(interval);
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn send_shortcut(modifier: enigo::Key, key: enigo::Key) -> Result<(), String> {
    use enigo::{Direction, Enigo, Keyboard, Settings};

    let mut enigo = Enigo::new(&Settings::default()).map_err(|error| {
        let message = error.to_string();
        if message.contains("permission") {
            "自動複製貼上需要輔助使用（Accessibility）權限。".into()
        } else {
            message
        }
    })?;
    enigo
        .key(modifier, Direction::Press)
        .map_err(|error| error.to_string())?;
    enigo
        .key(key, Direction::Press)
        .map_err(|error| error.to_string())?;
    std::thread::sleep(KEY_PRESS_TIME);
    enigo
        .key(key, Direction::Release)
        .map_err(|error| error.to_string())?;
    enigo
        .key(modifier, Direction::Release)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{
        leftover_modifiers, run_with_timeout, send_shortcut, wait_while, with_clipboard,
        CLIPBOARD_TIMEOUT, MODIFIER_POLL, MODIFIER_WAIT, PASTE_TIMEOUT,
    };
    use arboard::{Clipboard, GetExtLinux, LinuxClipboardKind, SetExtLinux};
    use enigo::Key;
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::ConnectionExt;

    pub(super) fn capture() -> Result<String, String> {
        let primary = read_kind(LinuxClipboardKind::Primary)?;
        if !primary.is_empty() {
            return Ok(primary);
        }
        let clipboard = read_kind(LinuxClipboardKind::Clipboard)?;
        if clipboard.is_empty() {
            return Err("沒有可轉換的選取文字。".into());
        }
        Ok(clipboard)
    }

    pub(super) fn replace(text: &str) -> Result<(), String> {
        let owned = text.to_string();
        with_clipboard(move |clipboard| {
            clipboard
                .set()
                .clipboard(LinuxClipboardKind::Clipboard)
                .text(owned)
                .map_err(|error| error.to_string())
        })?;
        run_with_timeout(PASTE_TIMEOUT, "貼上選取文字逾時。", || {
            wait_for_modifiers_released()?;
            send_shortcut(Key::Shift, Key::Insert)
        })?
    }

    fn read_kind(kind: LinuxClipboardKind) -> Result<String, String> {
        run_with_timeout(
            CLIPBOARD_TIMEOUT,
            "讀取選取文字逾時。",
            move || {
                Clipboard::new()
                    .ok()
                    .and_then(|mut clipboard| clipboard.get().clipboard(kind).text().ok())
                    .unwrap_or_default()
            },
        )
    }

    fn wait_for_modifiers_released() -> Result<(), String> {
        let (connection, screen) = x11rb::connect(None).map_err(|error| error.to_string())?;
        let root = connection.setup().roots[screen].root;
        wait_while(
            MODIFIER_WAIT,
            MODIFIER_POLL,
            || {
                let reply = connection
                    .query_pointer(root)
                    .map_err(|error| error.to_string())?
                    .reply()
                    .map_err(|error| error.to_string())?;
                Ok(leftover_modifiers(u16::from(reply.mask)))
            },
            "修飾鍵尚未放開，無法貼上。",
        )
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{
        run_with_timeout, send_shortcut, with_clipboard, CLIPBOARD_TIMEOUT, PASTE_TIMEOUT,
    };
    use enigo::Key;

    pub(super) fn capture() -> Result<String, String> {
        run_with_timeout(PASTE_TIMEOUT, "讀取選取文字逾時。", || {
            send_shortcut(Key::Meta, Key::Unicode('c'))?;
            std::thread::sleep(CLIPBOARD_TIMEOUT);
            let text = with_clipboard(|clipboard| {
                clipboard.get_text().map_err(|error| error.to_string())
            })?;
            if text.is_empty() {
                return Err("沒有可轉換的選取文字。".into());
            }
            Ok(text)
        })?
    }

    pub(super) fn replace(text: &str) -> Result<(), String> {
        with_clipboard(|clipboard| clipboard.set_text(text).map_err(|error| error.to_string()))?;
        run_with_timeout(PASTE_TIMEOUT, "貼上選取文字逾時。", || {
            send_shortcut(Key::Meta, Key::Unicode('v'))
        })?
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::{
        run_with_timeout, send_shortcut, wait_while, with_clipboard, CLIPBOARD_TIMEOUT,
        KEY_PRESS_TIME, MODIFIER_POLL, MODIFIER_WAIT,
    };
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    use std::time::Duration;
    use windows::Win32::System::DataExchange::GetClipboardSequenceNumber;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    };

    pub(super) fn capture() -> Result<String, String> {
        prepare_keyboard()?;
        let before = clipboard_sequence();
        send_shortcut(Key::Control, Key::C)?;
        wait_for_copied_text(before)
    }

    pub(super) fn replace(text: &str) -> Result<(), String> {
        prepare_keyboard()?;
        let owned = text.to_string();
        run_with_timeout(CLIPBOARD_TIMEOUT, "寫入剪貼簿逾時。", move || {
            with_clipboard(|clipboard| clipboard.set_text(owned).map_err(|error| error.to_string()))
        })??;
        send_shortcut(Key::Control, Key::V)
    }

    fn prepare_keyboard() -> Result<(), String> {
        if wait_while(
            MODIFIER_WAIT,
            MODIFIER_POLL,
            || Ok(any_modifier_down()),
            "修飾鍵尚未放開，無法複製貼上。",
        )
        .is_err()
        {
            release_held_modifiers()?;
        }
        std::thread::sleep(KEY_PRESS_TIME);
        Ok(())
    }

    fn wait_for_copied_text(before: u32) -> Result<String, String> {
        let mut text = String::new();
        wait_while(
            CLIPBOARD_TIMEOUT,
            Duration::from_millis(20),
            || {
                if clipboard_sequence() == before {
                    return Ok(true);
                }
                text = read_clipboard_text()?;
                Ok(text.is_empty())
            },
            "沒有可轉換的選取文字。",
        )?;
        if text.is_empty() {
            return Err("沒有可轉換的選取文字。".into());
        }
        Ok(text)
    }

    fn read_clipboard_text() -> Result<String, String> {
        run_with_timeout(CLIPBOARD_TIMEOUT, "讀取選取文字逾時。", || {
            with_clipboard(|clipboard| match clipboard.get().text() {
                Ok(value) => Ok(value),
                Err(_) => Ok(String::new()),
            })
        })?
    }

    fn clipboard_sequence() -> u32 {
        unsafe { GetClipboardSequenceNumber() }
    }

    fn any_modifier_down() -> bool {
        [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
            .into_iter()
            .any(virtual_key_down)
    }

    fn virtual_key_down(key: VIRTUAL_KEY) -> bool {
        virtual_key_held(unsafe { GetAsyncKeyState(i32::from(key.0)) })
    }

    pub(super) fn virtual_key_held(state: i16) -> bool {
        (state as u16) & 0x8000 != 0
    }

    fn release_held_modifiers() -> Result<(), String> {
        let mut enigo = Enigo::new(&Settings::default()).map_err(|error| error.to_string())?;
        for key in held_modifier_keys() {
            enigo
                .key(key, Direction::Release)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn held_modifier_keys() -> Vec<Key> {
        let mut keys = Vec::new();
        if virtual_key_down(VK_SHIFT) {
            keys.push(Key::Shift);
        }
        if virtual_key_down(VK_CONTROL) {
            keys.push(Key::Control);
        }
        if virtual_key_down(VK_MENU) {
            keys.push(Key::Alt);
        }
        if virtual_key_down(VK_LWIN) {
            keys.push(Key::LWin);
        }
        if virtual_key_down(VK_RWIN) {
            keys.push(Key::RWin);
        }
        keys
    }
}

#[cfg(test)]
mod tests {
    use super::{leftover_modifiers, wait_while};
    use std::time::{Duration, Instant};

    #[test]
    fn leftover_modifiers_ignore_lock_and_numlock() {
        assert!(!leftover_modifiers(0));
        assert!(!leftover_modifiers(1 << 1));
        assert!(!leftover_modifiers(1 << 4));
        assert!(leftover_modifiers(1 << 3));
    }

    #[test]
    fn modifier_wait_times_out_when_still_held() {
        let started = Instant::now();
        let result = wait_while(
            Duration::from_millis(50),
            Duration::from_millis(10),
            || Ok(true),
            "修飾鍵尚未放開，無法貼上。",
        );
        assert_eq!(result, Err("修飾鍵尚未放開，無法貼上。".into()));
        assert!(started.elapsed() < Duration::from_millis(400));
    }

    #[test]
    fn modifier_wait_returns_when_released() {
        let started = Instant::now();
        wait_while(
            Duration::from_secs(2),
            Duration::from_millis(10),
            || Ok(false),
            "修飾鍵尚未放開，無法貼上。",
        )
        .expect("already released");
        assert!(started.elapsed() < Duration::from_millis(200));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn virtual_key_held_uses_high_bit() {
        assert!(!super::windows::virtual_key_held(0));
        assert!(!super::windows::virtual_key_held(1));
        assert!(super::windows::virtual_key_held(i16::MIN));
        assert!(super::windows::virtual_key_held(-32767));
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    #[test]
    fn clipboard_timeout_does_not_block_forever() {
        use super::{run_with_timeout, CLIPBOARD_TIMEOUT};

        let started = Instant::now();
        let result = run_with_timeout(
            Duration::from_millis(50),
            "讀取選取文字逾時。",
            || {
                std::thread::sleep(Duration::from_secs(2));
                "late"
            },
        );
        assert_eq!(result, Err("讀取選取文字逾時。".into()));
        assert!(started.elapsed() < Duration::from_millis(400));
        let _ = CLIPBOARD_TIMEOUT;
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn live_primary_selection_is_readable() {
        use super::with_clipboard;
        use arboard::{LinuxClipboardKind, SetExtLinux};

        if std::env::var_os("DISPLAY").is_none() {
            return;
        }
        let marker = format!("CONVERTZZ-PRIMARY-{}", std::process::id());
        with_clipboard(|clipboard| {
            clipboard
                .set()
                .clipboard(LinuxClipboardKind::Primary)
                .text(marker.clone())
                .map_err(|error| error.to_string())
        })
        .expect("set PRIMARY");
        std::thread::sleep(Duration::from_millis(150));
        let captured = super::capture().expect("capture PRIMARY");
        assert!(
            captured.contains(&marker),
            "expected PRIMARY marker, got {captured:?}"
        );
    }
}

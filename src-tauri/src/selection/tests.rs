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

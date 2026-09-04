use super::*;
use crate::core::cli::parse_cli;
use crate::core::settings::default_settings;
use crate::core::types::{Direction, TextEncoding, ZhConvertModules};
use serde_json::{json, Value};
use std::fs;
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("convertzz-headless-{label}-{nanos}"));
    fs::create_dir_all(&path).expect("temp dir");
    path
}

#[test]
fn rejects_write_without_yes_in_non_tty_path() {
    // confirm_write 對非 TTY 的契約：未帶 --yes 時 confirm_write() 回 Err。
    let mut parsed = parse_cli(&args(&["--output", "utf8", "a.txt"]), None);
    assert!(parsed.headless);
    assert!(!parsed.confirm_write);
    // 直接測 helper：非互動時缺 --yes 必須失敗（stdin 在測試中通常非 TTY）。
    let result = confirm_write(&mut parsed, 1, "個檔案");
    if !io::stdin().is_terminal() {
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("--yes"));
    }
}

#[test]
fn rejects_headless_without_direction_when_no_config() {
    let dir = temp_dir("no-dir");
    let input = dir.join("in.txt");
    fs::write(&input, "測試").expect("write");

    let code = run(
        &args(&[
            "--output",
            "utf8",
            "--vocabulary",
            "off",
            "--no-backup",
            "--yes",
            input.to_str().unwrap(),
        ]),
        None,
    );
    assert_eq!(code, 1);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rejects_globalconfig_with_config_path() {
    let code = run(
        &args(&[
            "--headless",
            "--file",
            "--globalconfig",
            "--config",
            "/tmp/missing-settings.json",
            "--yes",
            "a.txt",
        ]),
        None,
    );
    assert_eq!(code, 1);
}

#[test]
fn rejects_explicit_vocabulary_settings_without_config() {
    let dir = temp_dir("vocab-settings");
    let input = dir.join("in.txt");
    fs::write(&input, "測試").expect("write");

    let code = run(
        &args(&[
            "--output",
            "utf8",
            "--direction",
            "none",
            "--vocabulary",
            "settings",
            "--no-backup",
            "--yes",
            input.to_str().unwrap(),
        ]),
        None,
    );
    assert_eq!(code, 1);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn headless_file_convert_with_yes() {
    let dir = temp_dir("file");
    let input = dir.join("in.txt");
    let output = dir.join("out.txt");
    fs::write(&input, "简体字测试").expect("write input");

    let code = run(
        &args(&[
            "--output",
            "utf8",
            "--direction",
            "s2t",
            "--vocabulary",
            "off",
            "--engine",
            "segmented",
            "--no-backup",
            "--yes",
            input.to_str().unwrap(),
            output.to_str().unwrap(),
        ]),
        None,
    );
    assert_eq!(code, 0, "headless file convert should succeed");
    let text = fs::read_to_string(&output).expect("read output");
    assert_eq!(text.trim(), "簡體字測試");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn headless_flag_file_without_output_encoding() {
    let dir = temp_dir("flag");
    let input = dir.join("sample.txt");
    fs::write(&input, "測試文字").expect("write");

    let code = run(
        &args(&[
            "--headless",
            "--file",
            "--direction",
            "none",
            "--vocabulary",
            "off",
            "--no-backup",
            "-y",
            input.to_str().unwrap(),
        ]),
        None,
    );
    assert_eq!(code, 0);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn headless_config_path_supplies_direction() {
    let dir = temp_dir("config");
    let input = dir.join("in.txt");
    let output = dir.join("out.txt");
    let config = dir.join("settings.json");
    fs::write(&input, "简体字测试").expect("write input");

    let mut settings = default_settings();
    settings["direction"] = json!("s2t");
    settings["engine"] = json!("segmented");
    settings["vocabularyCorrection"] = json!(false);
    fs::write(
        &config,
        serde_json::to_string_pretty(&settings).expect("serialize"),
    )
    .expect("write config");

    let code = run(
        &args(&[
            "--output",
            "utf8",
            "--config",
            config.to_str().unwrap(),
            "--no-backup",
            "--yes",
            input.to_str().unwrap(),
            output.to_str().unwrap(),
        ]),
        None,
    );
    assert_eq!(code, 0);
    let text = fs::read_to_string(&output).expect("read output");
    assert_eq!(text.trim(), "簡體字測試");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn headless_config_accepts_store_document_shape() {
    let dir = temp_dir("store-shape");
    let input = dir.join("in.txt");
    let output = dir.join("out.txt");
    let config = dir.join("settings-v2.json");
    fs::write(&input, "简体字测试").expect("write input");

    let mut settings = default_settings();
    settings["direction"] = json!("s2t");
    settings["vocabularyCorrection"] = json!(false);
    let document = json!({ "settings": settings });
    fs::write(
        &config,
        serde_json::to_string_pretty(&document).expect("serialize"),
    )
    .expect("write config");

    let code = run(
        &args(&[
            "--output",
            "utf8",
            "--config",
            config.to_str().unwrap(),
            "--no-backup",
            "--yes",
            input.to_str().unwrap(),
            output.to_str().unwrap(),
        ]),
        None,
    );
    assert_eq!(code, 0);
    let text = fs::read_to_string(&output).expect("read output");
    assert_eq!(text.trim(), "簡體字測試");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn normalize_settings_document_extracts_store_key() {
    let mut settings = default_settings();
    settings["direction"] = json!("t2s");
    let normalized = normalize_settings_document(json!({ "settings": settings }));
    assert_eq!(
        normalized.get("direction").and_then(Value::as_str),
        Some("t2s")
    );
}

#[test]
fn rejects_invalid_direction_value() {
    let code = run(
        &args(&[
            "--output",
            "utf8",
            "--direction",
            "sideways",
            "--yes",
            "a.txt",
        ]),
        None,
    );
    assert_eq!(code, 1);
}

#[test]
fn config_applies_backup_and_recognize_encoding() {
    let dir = temp_dir("settings-defaults");
    let input = dir.join("in.txt");
    let config = dir.join("settings.json");
    fs::write(&input, "測試").expect("write");

    let mut settings = default_settings();
    settings["direction"] = json!("none");
    settings["autoBackupBeforeConversion"] = json!(false);
    settings["recognizeEncoding"] = json!(false);
    settings["vocabularyCorrection"] = json!(false);
    fs::write(
        &config,
        serde_json::to_string_pretty(&settings).expect("serialize"),
    )
    .expect("write config");

    let mut parsed = parse_cli(
        &args(&[
            "--headless",
            "--file",
            "--config",
            config.to_str().unwrap(),
            "--yes",
            input.to_str().unwrap(),
        ]),
        None,
    );
    let loaded = load_cli_settings(&parsed).expect("load");
    apply_settings_defaults(&mut parsed, loaded.as_ref()).expect("defaults");
    assert!(!parsed.backup);
    assert_eq!(parsed.input_encoding, TextEncoding::Utf8);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn zhconvert_options_keeps_modules_from_settings() {
    let settings = json!({
        "zhconvert": {
            "converterS2T": "Taiwan",
            "converterT2S": "Simplified",
            "modules": { "ChineseVariant": 1 }
        }
    });
    let options = zhconvert_options(Some(&settings), Direction::S2t);
    match options.modules {
        Some(ZhConvertModules::Map(map)) => {
            assert_eq!(map.get("ChineseVariant"), Some(&1));
        }
        other => panic!("expected modules map, got {other:?}"),
    }
}

#[test]
fn merge_exit_codes_prefers_cancel_then_partial() {
    assert_eq!(merge_exit_codes(1, 2), 2);
    assert_eq!(merge_exit_codes(2, 3), 2);
    assert_eq!(merge_exit_codes(1, 3), 3);
    assert_eq!(merge_exit_codes(0, 3), 3);
    assert_eq!(merge_exit_codes(1, 0), 1);
    assert_eq!(merge_exit_codes(0, 0), 0);
}

#[test]
fn confirm_write_yes_flag_is_sticky() {
    let mut parsed = parse_cli(&args(&["--output", "utf8", "--yes", "a.txt"]), None);
    assert!(confirm_write(&mut parsed, 1, "個檔案").expect("yes"));
    assert!(parsed.confirm_write);
    assert!(confirm_write(&mut parsed, 3, "個音訊檔案").expect("still yes"));
}

#[test]
fn yes_answer_accepts_chinese_and_latin() {
    assert!(is_yes_answer("是"));
    assert!(is_yes_answer(" y "));
    assert!(is_yes_answer("YES"));
    assert!(!is_yes_answer("否"));
    assert!(!is_yes_answer("n"));
    assert!(!is_yes_answer(""));
}

#[test]
fn confirm_tags_then_rename_yes_flag() {
    let mut parsed = parse_cli(
        &args(&["--headless", "--audio", "--filename", "--yes", "a.mp3"]),
        None,
    );
    assert!(confirm_tags_then_rename(&mut parsed, 2, 2).expect("yes"));
}

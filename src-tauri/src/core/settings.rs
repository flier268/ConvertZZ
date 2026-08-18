use super::error::CoreError;
use serde_json::{json, Map, Value};

pub fn migrate_from_path(path: &str) -> Result<Value, CoreError> {
    let raw = std::fs::read_to_string(path)?
        .trim_start_matches('\u{feff}')
        .to_string();
    let parsed = serde_json::from_str::<Value>(&raw)?;
    Ok(migrate(parsed))
}

pub fn default_settings() -> Value {
    json!({
        "version": 2,
        "engine": "segmented",
        "direction": "s2t",
        "vocabularyCorrection": true,
        "promptAfterConversion": true,
        "autoBackupBeforeConversion": true,
        "recognizeEncoding": true,
        "previewMaxKb": 6,
        "floatingBall": { "enabled": true, "x": -1, "y": -1 },
        "hotkeys": {
            "autoCopy": true,
            "autoPaste": true,
            "shortcuts": (0..4).map(|index| json!({
                "enabled": false,
                "accelerator": "",
                "action": format!("a{}", index + 1)
            })).collect::<Vec<_>>()
        },
        "quickActions": {
            "leftClickCtrl": "0",
            "leftClickAlt": "0",
            "leftClickShift": "0",
            "rightClickCtrl": "0",
            "rightClickAlt": "0",
            "rightClickShift": "0",
            "leftDropCtrl": "0",
            "leftDropAlt": "0",
            "leftDropShift": "0",
            "rightDropCtrl": "0",
            "rightDropAlt": "0",
            "rightDropShift": "0"
        },
        "files": {
            "defaultPath": "!",
            "typeFilter": "<常用文字檔案|*.txt;*.log;*.ini;*.inf;*.bat;*.cmd;*.srt;*.ass;*.lang>/<常用網頁文件|*.htm;*.html;*.php;*.asp;*.css;*.js>/<音訊文件|*.mp3;*.ape;*.ogg;*.oga;*.opus>",
            "fixCharsetExtensions": [".htm", ".html", ".shtm", ".shtml", ".asp", ".aspx", ".php", ".css"],
            "unicodeAddBom": false
        },
        "zhconvert": {
            "converterS2T": "Taiwan",
            "converterT2S": "Simplified",
            "modules": {},
            "jpTextConversionStrategy": "protectOnlySameOrigin",
            "jpStyleConversionStrategy": "protectOnlySameOrigin",
            "cleanUpText": false,
            "userPreReplace": "",
            "userPostReplace": "",
            "userProtectReplace": "",
            "ensureNewlineAtEof": false,
            "translateTabsToSpaces": -1,
            "trimTrailingWhiteSpaces": false,
            "unifyLeadingHyphen": false,
            "ignoreTextStyles": "",
            "jpTextStyles": ""
        },
        "checkVersionOnStart": true,
        "checkPreReleaseUpdates": false,
        "skippedUpdateVersion": "",
        "showMainWindowOnStart": false
    })
}

pub fn migrate(input: Value) -> Value {
    if input.get("version").and_then(Value::as_u64) == Some(2) {
        return merge_v2(input);
    }
    let defaults = default_settings();
    let hotkey = object_value(input.get("HotKey"));
    let file_convert = object_value(input.get("FileConvert"));
    let quick_start = object_value(input.get("QuickStart"));
    let fanhuaji = object_value(input.get("Fanhuaji_Setting"));
    let shortcuts = (0..4)
        .map(|index| {
            let feature = object_value(hotkey.get(&format!("Feature{}", index + 1)));
            let modifier = normalize_modifier(&string_value(feature.get("Modift")));
            let key = string_value(feature.get("Key"));
            let action = string_value(feature.get("Action"));
            let key = if key == "None" { String::new() } else { key };
            let accelerator = [modifier, key]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("+");
            let action = if action.is_empty() {
                format!("a{}", index + 1)
            } else {
                action
            };
            json!({
                "enabled": boolean_value(feature.get("Enable"), false),
                "accelerator": accelerator,
                "action": action
            })
        })
        .collect::<Vec<_>>();

    json!({
        "version": 2,
        "engine": engine_value(input.get("Engine")),
        "direction": defaults["direction"],
        "vocabularyCorrection": boolean_value(input.get("Vocabulary correction"), true),
        "promptAfterConversion": boolean_value(input.get("Prompt"), true),
        "autoBackupBeforeConversion": true,
        "recognizeEncoding": boolean_value(input.get("RecognitionEncoding"), true),
        "previewMaxKb": number_value(input.get("MaxLengthPreview"), 6),
        "floatingBall": {
            "enabled": boolean_value(input.get("AssistiveTouch"), true),
            "x": number_value(input.get("PositionX"), -1),
            "y": number_value(input.get("PositionY"), -1)
        },
        "hotkeys": {
            "autoCopy": boolean_value(hotkey.get("AutoCopy"), true),
            "autoPaste": boolean_value(hotkey.get("AutoPaste"), true),
            "shortcuts": shortcuts
        },
        "quickActions": {
            "leftClickCtrl": nonempty(string_value(quick_start.get("LeftClick_Ctrl")), "0"),
            "leftClickAlt": nonempty(string_value(quick_start.get("LeftClick_Alt")), "0"),
            "leftClickShift": nonempty(string_value(quick_start.get("LeftClick_Shift")), "0"),
            "rightClickCtrl": nonempty(string_value(quick_start.get("RightClick_Ctrl")), "0"),
            "rightClickAlt": nonempty(string_value(quick_start.get("RightClick_Alt")), "0"),
            "rightClickShift": nonempty(string_value(quick_start.get("RightClick_Shift")), "0"),
            "leftDropCtrl": nonempty(string_value(quick_start.get("LeftDrop_Ctrl")), "0"),
            "leftDropAlt": nonempty(string_value(quick_start.get("LeftDrop_Alt")), "0"),
            "leftDropShift": nonempty(string_value(quick_start.get("LeftDrop_Shift")), "0"),
            "rightDropCtrl": nonempty(string_value(quick_start.get("RightDrop_Ctrl")), "0"),
            "rightDropAlt": nonempty(string_value(quick_start.get("RightDrop_Alt")), "0"),
            "rightDropShift": nonempty(string_value(quick_start.get("RightDrop_Shift")), "0")
        },
        "files": {
            "defaultPath": nonempty(string_value(file_convert.get("DefaultPath")), defaults["files"]["defaultPath"].as_str().unwrap_or("!")),
            "typeFilter": nonempty(string_value(file_convert.get("TypeFilter")), defaults["files"]["typeFilter"].as_str().unwrap_or("")),
            "fixCharsetExtensions": fix_label(file_convert.get("FixLabel"), &defaults),
            "unicodeAddBom": boolean_value(file_convert.get("UnicodeAddBOM"), false)
        },
        "zhconvert": {
            "converterS2T": converter_value(fanhuaji.get("Converter_S_to_T"), "Taiwan"),
            "converterT2S": converter_value(fanhuaji.get("Converter_T_to_S"), "Simplified"),
            "modules": module_values(fanhuaji.get("Modules")),
            "jpTextConversionStrategy": strategy_value(fanhuaji.get("JpTextConversionStrategy")),
            "jpStyleConversionStrategy": strategy_value(fanhuaji.get("JpStyleConversionStrategy")),
            "cleanUpText": boolean_value(fanhuaji.get("CleanUpText"), false),
            "userPreReplace": replacement_lines(fanhuaji.get("UserPreReplace")),
            "userPostReplace": replacement_lines(fanhuaji.get("UserPostReplace")),
            "userProtectReplace": protection_lines(fanhuaji.get("UserProtectReplace")),
            "ensureNewlineAtEof": boolean_value(fanhuaji.get("EnsureNewlineAtEof"), false),
            "translateTabsToSpaces": number_value(fanhuaji.get("TranslateTabsToSpaces"), -1),
            "trimTrailingWhiteSpaces": boolean_value(fanhuaji.get("TrimTrailingWhiteSpaces"), false),
            "unifyLeadingHyphen": boolean_value(fanhuaji.get("UnifyLeadingHyphen"), false),
            "ignoreTextStyles": string_value(fanhuaji.get("IgnoreTextStyles")),
            "jpTextStyles": string_value(fanhuaji.get("JpTextStyles"))
        },
        "checkVersionOnStart": boolean_value(input.get("CheckVersion"), true),
        "checkPreReleaseUpdates": false,
        "skippedUpdateVersion": "",
        "showMainWindowOnStart": false
    })
}

fn merge_v2(input: Value) -> Value {
    let defaults = default_settings();
    let mut merged = defaults.as_object().cloned().unwrap_or_default();
    if let Some(object) = input.as_object() {
        for (key, value) in object {
            match key.as_str() {
                "floatingBall" | "hotkeys" | "quickActions" | "files" | "zhconvert" => {
                    merged.insert(key.clone(), merge_object(defaults.get(key), value));
                }
                "autoBackupBeforeConversion" => {
                    merged.insert(key.clone(), json!(value.as_bool().unwrap_or(true)));
                }
                "engine" => {
                    merged.insert(key.clone(), value.clone());
                }
                _ => {
                    merged.insert(key.clone(), value.clone());
                }
            }
        }
    }
    if !merged.contains_key("engine") {
        merged.insert("engine".into(), json!("segmented"));
    }
    Value::Object(merged)
}

fn merge_object(defaults: Option<&Value>, value: &Value) -> Value {
    let mut merged = defaults
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(object) = value.as_object() {
        for (key, item) in object {
            merged.insert(key.clone(), item.clone());
        }
    }
    Value::Object(merged)
}

fn object_value(value: Option<&Value>) -> Map<String, Value> {
    value
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn string_value(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn boolean_value(value: Option<&Value>, fallback: bool) -> bool {
    value.and_then(Value::as_bool).unwrap_or(fallback)
}

fn number_value(value: Option<&Value>, fallback: i64) -> i64 {
    value.and_then(Value::as_i64).unwrap_or(fallback)
}

fn nonempty(value: String, fallback: &str) -> String {
    if value.is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

fn fix_label(value: Option<&Value>, defaults: &Value) -> Value {
    let fallback = defaults["files"]["fixCharsetExtensions"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| item.as_str().map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    let raw = string_value(value);
    if raw.is_empty() {
        return json!(fallback);
    }
    json!(raw
        .split('|')
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>())
}

fn replacement_lines(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let key = string_value(item.get("Key"));
                    if key.is_empty() {
                        None
                    } else {
                        Some(format!("{key}={}", string_value(item.get("Value"))))
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn protection_lines(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let key = string_value(item.get("Key"));
                    let fallback = string_value(item.get("Value"));
                    let value = if key.is_empty() { fallback } else { key };
                    if value.is_empty() {
                        None
                    } else {
                        Some(value)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn strategy_value(value: Option<&Value>) -> &'static str {
    if let Some(text) = value.and_then(Value::as_str) {
        return match text {
            "none" | "protect" | "protectOnlySameOrigin" | "fix" => match text {
                "none" => "none",
                "protect" => "protect",
                "fix" => "fix",
                _ => "protectOnlySameOrigin",
            },
            _ => "protectOnlySameOrigin",
        };
    }
    match value.and_then(Value::as_u64) {
        Some(0) => "protectOnlySameOrigin",
        Some(1) => "none",
        Some(2) => "protect",
        Some(3) => "fix",
        _ => "protectOnlySameOrigin",
    }
}

fn normalize_modifier(value: &str) -> String {
    if value.is_empty() || value == "None" {
        return String::new();
    }
    value
        .split([',', '+'])
        .map(str::trim)
        .filter(|part| !part.is_empty() && *part != "None")
        .collect::<Vec<_>>()
        .join("+")
}

fn engine_value(value: Option<&Value>) -> &'static str {
    match value {
        Some(Value::Number(number)) if number.as_i64() == Some(1) => "zhconvert",
        Some(Value::String(text)) if text == "Fanhuaji" || text == "zhconvert" => "zhconvert",
        Some(Value::String(text)) if text == "legacy" => "legacy",
        _ => "segmented",
    }
}

fn converter_value(value: Option<&Value>, fallback: &str) -> String {
    if let Some(text) = value
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
    {
        return text.to_string();
    }
    if let Some(index) = value.and_then(Value::as_u64) {
        return [
            "Simplified",
            "Traditional",
            "China",
            "Hongkong",
            "Taiwan",
            "Pinyin",
            "Bopomofo",
            "Mars",
            "WikiSimplified",
            "WikiTraditional",
        ]
        .get(index as usize)
        .unwrap_or(&fallback)
        .to_string();
    }
    fallback.to_string()
}

fn module_values(value: Option<&Value>) -> Value {
    let Some(items) = value.and_then(Value::as_array) else {
        return json!({});
    };
    let mut modules = Map::new();
    for item in items {
        let name = string_value(item.get("ModuleName"));
        if name.is_empty() {
            continue;
        }
        let enabled = item.get("Enable").and_then(Value::as_bool);
        modules.insert(
            name,
            json!(match enabled {
                Some(true) => 1,
                Some(false) => 0,
                None => -1,
            }),
        );
    }
    Value::Object(modules)
}

#[cfg(test)]
mod tests;

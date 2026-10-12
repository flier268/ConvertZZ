use super::error::CoreError;
use serde_json::{json, Map, Value};

/// 與 `src/lib/fileFilters.ts` 的 `DEFAULT_FILE_TYPE_FILTER` 保持一致。不含音訊副檔名。
const DEFAULT_TYPE_FILTER: &str = "<常用文字檔案|*.txt;*.log;*.ini;*.inf;*.bat;*.cmd;*.srt;*.ass;*.lang>/<常用網頁文件|*.htm;*.html;*.php;*.asp;*.css;*.js>";

/// 2.0 早期預設。載入時改為 `DEFAULT_TYPE_FILTER`，避免檔案轉換對話框再帶入音訊。
const LEGACY_TYPE_FILTER_WITH_AUDIO: &str = "<常用文字檔案|*.txt;*.log;*.ini;*.inf;*.bat;*.cmd;*.srt;*.ass;*.lang>/<常用網頁文件|*.htm;*.html;*.php;*.asp;*.css;*.js>/<音訊文件|*.mp3;*.ape;*.ogg;*.oga;*.opus>";

/// C# `ConvertZZ/Settings.cs` 的 `FileConvert` 預設（`origin/master` 第 141 行，commit `00c2e902`）。
/// 群組名是「音頻」，而且只列 `*.mp3`，與 2.0 的「音訊」字串不同。
const CSHARP_TYPE_FILTER_WITH_AUDIO: &str = "<常用文字檔案|*.txt;*.log;*.ini;*.inf;*.bat;*.cmd;*.srt;*.ass;*.lang>/<常用網頁文件|*.htm;*.html;*.php;*.asp;*.css;*.js>/<音頻文件|*.mp3>";

pub fn migrate_from_path(path: &str) -> Result<Value, CoreError> {
    let raw = std::fs::read_to_string(path)?
        .trim_start_matches('\u{feff}')
        .to_string();
    let parsed = serde_json::from_str::<Value>(&raw)?;
    Ok(migrate(parsed))
}

pub fn default_settings() -> Value {
    let mut settings = base_default_settings();
    ensure_filter_profiles(&mut settings);
    settings
}

fn base_default_settings() -> Value {
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
            "typeFilter": DEFAULT_TYPE_FILTER,
            "folderPickBehavior": "rememberLast",
            "lastFilterProfileId": "",
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
        "checkPreReleaseUpdates": default_check_pre_release_updates(),
        "skippedUpdateVersion": "",
        "showMainWindowOnStart": false,
        "lastDropAction": {
            "kind": "file",
            "operation": "content",
            "direction": "s2t"
        }
    })
}

pub fn migrate(input: Value) -> Value {
    let mut migrated = migrate_document(input);
    ensure_filter_profiles(&mut migrated);
    migrated
}

fn migrate_document(input: Value) -> Value {
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
            "typeFilter": normalize_type_filter(&string_value(file_convert.get("TypeFilter")), defaults["files"]["typeFilter"].as_str().unwrap_or(DEFAULT_TYPE_FILTER)),
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
        "checkPreReleaseUpdates": default_check_pre_release_updates(),
        "skippedUpdateVersion": "",
        "showMainWindowOnStart": false,
        "lastDropAction": defaults["lastDropAction"].clone()
    })
}

fn merge_v2(input: Value) -> Value {
    // 不含預設 profile：缺欄位時要由輸入自己的 `typeFilter` 遷移，而不是套用預設清單。
    let defaults = base_default_settings();
    let mut merged = defaults.as_object().cloned().unwrap_or_default();
    if let Some(object) = input.as_object() {
        for (key, value) in object {
            match key.as_str() {
                "floatingBall" | "hotkeys" | "quickActions" | "files" | "zhconvert"
                | "lastDropAction" => {
                    merged.insert(key.clone(), merge_object(defaults.get(key), value));
                }
                "autoBackupBeforeConversion" => {
                    merged.insert(key.clone(), json!(value.as_bool().unwrap_or(true)));
                }
                "checkPreReleaseUpdates" => {
                    merged.insert(
                        key.clone(),
                        json!(value
                            .as_bool()
                            .unwrap_or_else(default_check_pre_release_updates)),
                    );
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
    if let Some(filter) = merged
        .get("files")
        .and_then(|files| files.get("typeFilter"))
        .and_then(Value::as_str)
        .map(str::to_string)
    {
        let normalized = normalize_saved_type_filter(&filter);
        if normalized != filter {
            if let Some(files) = merged.get_mut("files").and_then(Value::as_object_mut) {
                files.insert("typeFilter".into(), json!(normalized));
            }
        }
    }
    Value::Object(merged)
}

const BUILTIN_TEXT_ID: &str = "builtin-text";
const BUILTIN_ALL_ID: &str = "builtin-all";
const BUILTIN_TEXT_NAME: &str = "常用文字檔案";
const BUILTIN_ALL_NAME: &str = "所有檔案";
const BUILTIN_TEXT_EXTENSIONS: &[&str] = &[
    ".txt", ".log", ".ini", ".inf", ".bat", ".cmd", ".srt", ".ass", ".lang",
];

fn builtin_text_profile() -> Value {
    json!({
        "id": BUILTIN_TEXT_ID,
        "name": BUILTIN_TEXT_NAME,
        "builtin": true,
        "mode": "allow",
        "extensions": BUILTIN_TEXT_EXTENSIONS,
        "includeNoExtension": false
    })
}

fn builtin_all_profile() -> Value {
    json!({
        "id": BUILTIN_ALL_ID,
        "name": BUILTIN_ALL_NAME,
        "builtin": true,
        "mode": "all",
        "extensions": [],
        "includeNoExtension": true
    })
}

/// 舊篩選器單一樣式轉副檔名（小寫、含前導點）。多段（`*.tar.gz`）只取最後一段，
/// 與核心 `extension_of` 一致；`*` 與空字串不是副檔名。
pub(crate) fn legacy_pattern_extension(pattern: &str) -> Option<String> {
    let cleaned = pattern.trim().trim_start_matches('*');
    let last = cleaned.rsplit('.').next().unwrap_or_default();
    if last.is_empty() || last.contains('*') {
        return None;
    }
    Some(format!(".{}", last.to_lowercase()))
}

/// 舊 `<名稱|*.a;*.b>/<...>` 字串逐群組解析。含 `*`／`*.*` 的群組是所有檔案。
/// 回傳 `(名稱, 副檔名（小寫、含前導點）, 是否為所有檔案)`。
fn parse_legacy_groups(value: &str) -> Vec<(String, Vec<String>, bool)> {
    let mut groups = Vec::new();
    let mut rest = value;
    while let Some(start) = rest.find('<') {
        let Some(relative_end) = rest[start..].find('>') else {
            break;
        };
        let end = start + relative_end;
        let inner = &rest[start + 1..end];
        rest = &rest[end + 1..];
        let Some((name, patterns)) = inner.split_once('|') else {
            continue;
        };
        if name.contains('<') || patterns.contains('<') {
            continue;
        }
        let mut all_files = false;
        let mut extensions = Vec::new();
        for pattern in patterns.split(';') {
            let trimmed = pattern.trim();
            if trimmed == "*" || trimmed.eq_ignore_ascii_case("*.*") {
                all_files = true;
                continue;
            }
            let Some(extension) = legacy_pattern_extension(trimmed) else {
                continue;
            };
            if !extensions.contains(&extension) {
                extensions.push(extension);
            }
        }
        if all_files || !extensions.is_empty() {
            groups.push((name.trim().to_string(), extensions, all_files));
        }
    }
    groups
}

/// 舊篩選器字串每個群組轉成一個白名單 profile；所有檔案群組與完全相同的「常用文字檔案」
/// 對應內建 profile。回傳 `(自訂 profile 以外的完整清單, 預設 profile id)`。
fn profiles_from_type_filter(type_filter: &str) -> (Vec<Value>, String) {
    let mut profiles = vec![builtin_text_profile(), builtin_all_profile()];
    let mut default_id: Option<String> = None;
    for (index, (name, extensions, all_files)) in
        parse_legacy_groups(type_filter).into_iter().enumerate()
    {
        let id = if all_files {
            BUILTIN_ALL_ID.to_string()
        } else if name == BUILTIN_TEXT_NAME && extensions == BUILTIN_TEXT_EXTENSIONS {
            BUILTIN_TEXT_ID.to_string()
        } else {
            let id = format!("legacy-{}", index + 1);
            let name = if name.is_empty() || name == BUILTIN_TEXT_NAME || name == BUILTIN_ALL_NAME {
                format!("{}（舊版）", if name.is_empty() { "篩選器" } else { &name })
            } else {
                name
            };
            profiles.push(json!({
                "id": id,
                "name": name,
                "builtin": false,
                "mode": "allow",
                "extensions": extensions,
                "includeNoExtension": false
            }));
            id
        };
        default_id.get_or_insert(id);
    }
    (
        profiles,
        default_id.unwrap_or_else(|| BUILTIN_TEXT_ID.to_string()),
    )
}

/// 丟棄格式錯誤的 profile：id 須為非空字串且不重複、name 須為字串、mode 只能是
/// allow／deny／all、extensions 須為字串陣列、includeNoExtension 若存在須為布林。
/// 副檔名以 `legacy_pattern_extension` 正規化並去重。
pub(crate) fn sanitize_filter_profiles(items: &[Value]) -> Vec<Value> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for item in items {
        let Some(id) = item
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
        else {
            continue;
        };
        let Some(name) = item.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(mode) = item
            .get("mode")
            .and_then(Value::as_str)
            .filter(|mode| matches!(*mode, "allow" | "deny" | "all"))
        else {
            continue;
        };
        let Some(raw_extensions) = item.get("extensions").and_then(Value::as_array) else {
            continue;
        };
        let Some(raw_extensions) = raw_extensions
            .iter()
            .map(Value::as_str)
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let include_no_extension = match item.get("includeNoExtension") {
            None => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => continue,
        };
        if !seen.insert(id.to_string()) {
            continue;
        }
        let mut extensions: Vec<String> = Vec::new();
        for raw in raw_extensions {
            if let Some(extension) = legacy_pattern_extension(raw) {
                if !extensions.contains(&extension) {
                    extensions.push(extension);
                }
            }
        }
        result.push(json!({
            "id": id,
            "name": name,
            "builtin": item.get("builtin").and_then(Value::as_bool).unwrap_or(false),
            "mode": mode,
            "extensions": extensions,
            "includeNoExtension": include_no_extension
        }));
    }
    result
}

/// 補齊 `files.filterProfiles` 等欄位。profile 不存在時由舊 `typeFilter` 遷移產生；
/// `typeFilter` 字串本身保留，供回滾。
pub(crate) fn ensure_filter_profiles(settings: &mut Value) {
    let Some(files) = settings.get_mut("files").and_then(Value::as_object_mut) else {
        return;
    };
    let existing = files
        .get("filterProfiles")
        .and_then(Value::as_array)
        .map(|items| sanitize_filter_profiles(items))
        .filter(|items| !items.is_empty());
    let type_filter = files
        .get("typeFilter")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_TYPE_FILTER)
        .to_string();
    let (mut profiles, derived_default) = match existing {
        Some(profiles) => (profiles, BUILTIN_TEXT_ID.to_string()),
        None => profiles_from_type_filter(&type_filter),
    };
    for builtin in [builtin_all_profile(), builtin_text_profile()] {
        let id = builtin["id"].as_str().unwrap_or_default().to_string();
        if !profiles
            .iter()
            .any(|item| item.get("id").and_then(Value::as_str) == Some(id.as_str()))
        {
            profiles.insert(0, builtin);
        }
    }
    let has = |id: &str| {
        profiles
            .iter()
            .any(|item| item.get("id").and_then(Value::as_str) == Some(id))
    };
    let default_id = files
        .get("defaultFilterProfileId")
        .and_then(Value::as_str)
        .filter(|id| has(id))
        .map(str::to_string)
        .unwrap_or(derived_default);
    let last_id = files
        .get("lastFilterProfileId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let behavior = match files.get("folderPickBehavior").and_then(Value::as_str) {
        Some("alwaysWizard") => "alwaysWizard",
        _ => "rememberLast",
    };
    files.insert("filterProfiles".into(), Value::Array(profiles));
    files.insert("defaultFilterProfileId".into(), json!(default_id));
    files.insert("lastFilterProfileId".into(), json!(last_id));
    files.insert("folderPickBehavior".into(), json!(behavior));
}

fn normalize_type_filter(value: &str, fallback: &str) -> String {
    if value.is_empty() {
        return fallback.to_string();
    }
    normalize_saved_type_filter(value)
}

/// 自訂篩選保留。空字串（beta1–8 把篩選器清空時存成 `""`，當時代表所有檔案；
/// 現在空清單代表資料夾掃描不收任何檔案）、非空且不含 `<`（C# 1.0.0.0–1.0.0.3，
/// 例如 e7c3aeb、03204e6）、已知舊內建預設，或拿掉音訊／音頻群組後與新預設相同，
/// 才改成不含音訊的預設。
fn normalize_saved_type_filter(value: &str) -> String {
    if is_legacy_builtin_type_filter(value) {
        DEFAULT_TYPE_FILTER.to_string()
    } else {
        value.to_string()
    }
}

fn is_legacy_builtin_type_filter(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    // 只看 `<`，與 C# `Contains("<")` 相同；不含這個字元的非空字串一律視為舊內建格式。
    if !value.contains('<') {
        return true;
    }
    value == LEGACY_TYPE_FILTER_WITH_AUDIO
        || value == CSHARP_TYPE_FILTER_WITH_AUDIO
        || without_audio_groups(value) == DEFAULT_TYPE_FILTER
}

/// 只移除格式完整、且名稱含「音訊」或「音頻」的 `<名稱|樣式>`。
/// 其餘 `<...>`（沒有 `|`、未閉合）留在比較字串。未閉合時從該 `<` 留到結尾。
/// 群組之間的其他文字不參加比較，剩下的片段用 `/` 接回。
fn without_audio_groups(value: &str) -> String {
    let mut groups = Vec::new();
    let mut rest = value;
    while let Some(start) = rest.find('<') {
        let Some(relative_end) = rest[start..].find('>') else {
            groups.push(&rest[start..]);
            break;
        };
        let end = start + relative_end;
        let group = &rest[start..=end];
        if !is_removable_audio_group(group) {
            groups.push(group);
        }
        rest = &rest[end + 1..];
    }
    groups.join("/")
}

fn is_removable_audio_group(group: &str) -> bool {
    let Some(inner) = group.get(1..group.len().saturating_sub(1)) else {
        return false;
    };
    let Some((name, _)) = inner.split_once('|') else {
        return false;
    };
    name.contains("音訊") || name.contains("音頻")
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

fn default_check_pre_release_updates() -> bool {
    is_pre_release_version(env!("CARGO_PKG_VERSION"))
}

fn is_pre_release_version(version: &str) -> bool {
    let trimmed = version.trim();
    let without_prefix = trimmed
        .strip_prefix('v')
        .or_else(|| trimmed.strip_prefix('V'))
        .unwrap_or(trimmed);
    let normalized = without_prefix
        .split_once('+')
        .map(|(base, _)| base)
        .unwrap_or(without_prefix);
    normalized.contains('-')
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

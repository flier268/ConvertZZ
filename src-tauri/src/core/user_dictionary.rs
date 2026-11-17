//! 使用者詞典：讓使用者指定特定整詞的簡繁對應。
//!
//! 詞條載入後加入分詞表，轉換時只對分出的整詞套用，不做全文取代。
//! 舊版 `Dictionary.csv` 只能唯讀匯入成本格式；匯入時略過 1.x 內建列與停用列。

use super::error::CoreError;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const USER_DICTIONARY_FILE: &str = "user-dictionary.txt";
const MAX_WORD_CHARS: usize = 64;
const LEGACY_DEFAULTS: &str = include_str!("user_dictionary/legacy-defaults.tsv");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UserDictionaryDirection {
    Both,
    S2t,
    T2s,
}

impl UserDictionaryDirection {
    pub fn s2t(self) -> bool {
        matches!(self, Self::Both | Self::S2t)
    }

    pub fn t2s(self) -> bool {
        matches!(self, Self::Both | Self::T2s)
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Both => "both",
            Self::S2t => "s2t",
            Self::T2s => "t2s",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "both" => Some(Self::Both),
            "s2t" => Some(Self::S2t),
            "t2s" => Some(Self::T2s),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDictionaryEntry {
    pub simplified: String,
    pub traditional: String,
    pub direction: UserDictionaryDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LegacyImportStatus {
    Importable,
    BuiltinDefault,
    Disabled,
    Invalid,
    Duplicate,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyImportRow {
    pub line: usize,
    pub simplified: String,
    pub traditional: String,
    pub status: LegacyImportStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<UserDictionaryDirection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyImportPreview {
    pub path: String,
    pub total: usize,
    pub importable: usize,
    pub builtin_default: usize,
    pub disabled: usize,
    pub invalid: usize,
    pub duplicate: usize,
    /// 可匯入的列與略過原因需要說明的列；內建預設列只計數，不逐列列出。
    pub rows: Vec<LegacyImportRow>,
}

/// 詞條檢查錯誤訊息；`None` 表示可用。
pub fn entry_problem(entry: &UserDictionaryEntry) -> Option<String> {
    for (label, word) in [("簡體", &entry.simplified), ("繁體", &entry.traditional)] {
        if word.is_empty() {
            return Some(format!("{label}詞不可空白。"));
        }
        if word.trim() != word.as_str() {
            return Some(format!("{label}詞前後不可有空白。"));
        }
        if word.chars().count() > MAX_WORD_CHARS {
            return Some(format!("{label}詞超過 {MAX_WORD_CHARS} 字。"));
        }
        if word.chars().any(|ch| ch.is_control() || ch == '|') {
            return Some(format!("{label}詞不可含定位字元、換行或「|」。"));
        }
    }
    if entry.direction.s2t() && !matchable(&entry.simplified) {
        return Some("簡轉繁的來源詞只能包含中文字（轉換時只在中文片段內分詞比對）。".into());
    }
    if entry.direction.t2s() && !matchable(&entry.traditional) {
        return Some("繁轉簡的來源詞只能包含中文字（轉換時只在中文片段內分詞比對）。".into());
    }
    None
}

fn matchable(word: &str) -> bool {
    word.chars().all(super::conversion::is_cjk_char)
}

/// 同一方向不得有同一來源詞對到不同結果。
pub fn check_conflicts(entries: &[UserDictionaryEntry]) -> Result<(), CoreError> {
    let mut s2t: HashMap<&str, &str> = HashMap::new();
    let mut t2s: HashMap<&str, &str> = HashMap::new();
    for entry in entries {
        if let Some(problem) = entry_problem(entry) {
            return Err(CoreError::new(
                "USER_DICTIONARY_INVALID",
                format!("{}／{}：{problem}", entry.simplified, entry.traditional),
            ));
        }
        if entry.direction.s2t() {
            if let Some(existing) = s2t.insert(&entry.simplified, &entry.traditional) {
                if existing != entry.traditional {
                    return Err(CoreError::new(
                        "USER_DICTIONARY_CONFLICT",
                        format!(
                            "簡轉繁「{}」同時對到「{existing}」與「{}」。",
                            entry.simplified, entry.traditional
                        ),
                    ));
                }
            }
        }
        if entry.direction.t2s() {
            if let Some(existing) = t2s.insert(&entry.traditional, &entry.simplified) {
                if existing != entry.simplified {
                    return Err(CoreError::new(
                        "USER_DICTIONARY_CONFLICT",
                        format!(
                            "繁轉簡「{}」同時對到「{existing}」與「{}」。",
                            entry.traditional, entry.simplified
                        ),
                    ));
                }
            }
        }
    }
    Ok(())
}

pub fn parse(text: &str) -> Result<Vec<UserDictionaryEntry>, CoreError> {
    let mut entries = Vec::new();
    for (index, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let columns: Vec<&str> = line.split('\t').collect();
        let direction = match columns.get(2) {
            None => Some(UserDictionaryDirection::Both),
            Some(value) => UserDictionaryDirection::parse(value),
        };
        let (Some(direction), true) = (direction, (2..=3).contains(&columns.len())) else {
            return Err(CoreError::new(
                "USER_DICTIONARY_INVALID",
                format!("使用者詞典第 {} 行格式錯誤。", index + 1),
            ));
        };
        entries.push(UserDictionaryEntry {
            simplified: columns[0].to_string(),
            traditional: columns[1].to_string(),
            direction,
        });
    }
    check_conflicts(&entries)?;
    Ok(entries)
}

pub fn serialize(entries: &[UserDictionaryEntry]) -> String {
    let mut text =
        String::from("# ConvertZZ 使用者詞典：簡體詞\\t繁體詞\\t方向（both｜s2t｜t2s）\n");
    for entry in entries {
        text.push_str(&entry.simplified);
        text.push('\t');
        text.push_str(&entry.traditional);
        text.push('\t');
        text.push_str(entry.direction.as_str());
        text.push('\n');
    }
    text
}

pub fn load(path: &Path) -> Result<Vec<UserDictionaryEntry>, CoreError> {
    match fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error.into()),
    }
}

/// 先寫同目錄暫存檔並重讀驗證，成功後才取代正式檔。
pub fn save(path: &Path, entries: &[UserDictionaryEntry]) -> Result<(), CoreError> {
    check_conflicts(entries)?;
    let directory = path
        .parent()
        .ok_or_else(|| CoreError::new("USER_DICTIONARY_PATH", "使用者詞典路徑無效。"))?;
    fs::create_dir_all(directory)?;
    let text = serialize(entries);
    let stage = directory.join(format!(
        ".{USER_DICTIONARY_FILE}.{}.stage",
        uuid::Uuid::new_v4().simple()
    ));
    let result = (|| {
        let mut file = fs::File::create(&stage)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        if parse(&fs::read_to_string(&stage)?)? != entries {
            return Err(CoreError::new(
                "USER_DICTIONARY_VERIFY",
                "使用者詞典暫存檔驗證失敗。",
            ));
        }
        fs::rename(&stage, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&stage);
    }
    result
}

/// 與設定檔同目錄：可攜模式在執行檔旁，安裝版在應用程式資料目錄。
pub fn default_path() -> Option<PathBuf> {
    super::headless::settings_store_path().and_then(|settings| {
        settings
            .parent()
            .map(|directory| directory.join(USER_DICTIONARY_FILE))
    })
}

fn legacy_defaults() -> &'static HashSet<(&'static str, &'static str)> {
    static DEFAULTS: std::sync::OnceLock<HashSet<(&'static str, &'static str)>> =
        std::sync::OnceLock::new();
    DEFAULTS.get_or_init(|| {
        LEGACY_DEFAULTS
            .lines()
            .filter(|line| !line.starts_with('#'))
            .filter_map(|line| line.split_once('\t'))
            .collect()
    })
}

fn clean_legacy_word(raw: &str) -> String {
    raw.trim_matches(|ch: char| ch.is_whitespace()).to_string()
}

/// 唯讀解析舊版六欄 `Dictionary.csv`（啟用、類型、簡體、優先權、繁體、優先權）。
/// 優先權不再使用：新引擎依分詞結果套用整詞。
pub fn preview_legacy_import(path: &Path) -> Result<LegacyImportPreview, CoreError> {
    let bytes = fs::read(path)?;
    let text = String::from_utf8(bytes).map_err(|_| {
        CoreError::new(
            "LEGACY_DICTIONARY_ENCODING",
            "舊版字典不是 UTF-8 編碼；請以 UTF-8 另存後再匯入。",
        )
    })?;
    let defaults = legacy_defaults();
    let mut preview = LegacyImportPreview {
        path: path.to_string_lossy().into_owned(),
        total: 0,
        importable: 0,
        builtin_default: 0,
        disabled: 0,
        invalid: 0,
        duplicate: 0,
        rows: Vec::new(),
    };
    let mut s2t_sources: HashMap<String, String> = HashMap::new();
    let mut t2s_sources: HashMap<String, String> = HashMap::new();
    for (index, line) in text.trim_start_matches('\u{feff}').split('\n').enumerate() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        preview.total += 1;
        let columns: Vec<&str> = line.split('\t').collect();
        let mut row = LegacyImportRow {
            line: index + 1,
            simplified: columns
                .get(2)
                .map(|value| clean_legacy_word(value))
                .unwrap_or_default(),
            traditional: columns
                .get(4)
                .map(|value| clean_legacy_word(value))
                .unwrap_or_default(),
            status: LegacyImportStatus::Invalid,
            direction: None,
            reason: None,
        };
        if columns.len() < 6 {
            row.reason = Some("不是六欄格式。".into());
            preview.invalid += 1;
            preview.rows.push(row);
            continue;
        }
        if defaults.contains(&(columns[2], columns[4])) {
            preview.builtin_default += 1;
            continue;
        }
        if !matches!(columns[0].to_ascii_lowercase().as_str(), "true" | "1") {
            row.status = LegacyImportStatus::Disabled;
            row.reason = Some("詞條已停用。".into());
            preview.disabled += 1;
            preview.rows.push(row);
            continue;
        }
        let usable = match (matchable(&row.simplified), matchable(&row.traditional)) {
            (true, true) => Some(UserDictionaryDirection::Both),
            (true, false) => Some(UserDictionaryDirection::S2t),
            (false, true) => Some(UserDictionaryDirection::T2s),
            (false, false) => None,
        };
        let candidate = UserDictionaryEntry {
            simplified: row.simplified.clone(),
            traditional: row.traditional.clone(),
            direction: usable.unwrap_or(UserDictionaryDirection::Both),
        };
        if let Some(problem) = entry_problem(&candidate) {
            row.reason = Some(problem);
            preview.invalid += 1;
            preview.rows.push(row);
            continue;
        }
        // 舊版允許多列共用來源詞並以優先權取捨；這裡保留先出現的對應，後者只保留另一方向。
        let usable = candidate.direction;
        let s2t_taken = !usable.s2t()
            || s2t_sources
                .get(&row.simplified)
                .is_some_and(|target| *target != row.traditional);
        let t2s_taken = !usable.t2s()
            || t2s_sources
                .get(&row.traditional)
                .is_some_and(|target| *target != row.simplified);
        let direction = match (s2t_taken, t2s_taken) {
            (false, false) => UserDictionaryDirection::Both,
            (false, true) => UserDictionaryDirection::S2t,
            (true, false) => UserDictionaryDirection::T2s,
            (true, true) => {
                row.status = LegacyImportStatus::Duplicate;
                row.reason = Some("兩個方向都已由前面的列使用。".into());
                preview.duplicate += 1;
                preview.rows.push(row);
                continue;
            }
        };
        if s2t_sources.get(&row.simplified) == Some(&row.traditional)
            && t2s_sources.get(&row.traditional) == Some(&row.simplified)
        {
            row.status = LegacyImportStatus::Duplicate;
            row.reason = Some("與前面的列相同。".into());
            preview.duplicate += 1;
            preview.rows.push(row);
            continue;
        }
        if direction.s2t() {
            s2t_sources.insert(row.simplified.clone(), row.traditional.clone());
        }
        if direction.t2s() {
            t2s_sources.insert(row.traditional.clone(), row.simplified.clone());
        }
        if direction != UserDictionaryDirection::Both {
            let other_is_text = if direction == UserDictionaryDirection::S2t {
                !matchable(&row.traditional)
            } else {
                !matchable(&row.simplified)
            };
            row.reason = Some(match (direction, other_is_text) {
                (UserDictionaryDirection::S2t, true) => "繁體詞含非中文字，只匯入簡轉繁。".into(),
                (UserDictionaryDirection::S2t, false) => {
                    "繁體詞已由前面的列使用，只匯入簡轉繁。".into()
                }
                (_, true) => "簡體詞含非中文字，只匯入繁轉簡。".into(),
                (_, false) => "簡體詞已由前面的列使用，只匯入繁轉簡。".into(),
            });
        }
        row.status = LegacyImportStatus::Importable;
        row.direction = Some(direction);
        preview.importable += 1;
        preview.rows.push(row);
    }
    Ok(preview)
}

#[cfg(test)]
mod tests;

//! 使用者詞典：讓使用者指定特定整詞的簡繁對應。
//!
//! 詞條載入後加入分詞表，轉換時只對分出的整詞套用，不做全文取代。
//! 匯入只依副檔名接受檔案，依內容判斷是本格式或 1.x 六欄字典；兩者都唯讀解析，
//! 轉成本格式存進 `user-dictionary.txt`，之後不再讀取來源檔。

use super::error::CoreError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const USER_DICTIONARY_FILE: &str = "user-dictionary.txt";
const MAX_WORD_CHARS: usize = 64;
const MAX_LABEL_CHARS: usize = 255;
/// 匯入與匯出接受的副檔名；檔名本身不限制。
pub const DICTIONARY_EXTENSIONS: [&str; 3] = ["txt", "tsv", "csv"];

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
    /// 匯入時間（RFC 3339）；手動加入的詞沒有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_at: Option<String>,
    /// 匯入來源的檔名，只作標籤，不會再讀取原檔。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_file: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportFormat {
    /// 本程式的使用者詞典格式（含匯出檔）。
    Native,
    /// 1.x 的六欄 `Dictionary.csv`。
    Legacy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportStatus {
    Importable,
    Disabled,
    Invalid,
    Duplicate,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRow {
    pub line: usize,
    pub simplified: String,
    pub traditional: String,
    pub status: ImportStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<UserDictionaryDirection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub path: String,
    pub format: ImportFormat,
    /// 寫入匯入詞條的來源檔名標籤。
    pub source_file: String,
    /// 寫入匯入詞條的匯入時間。
    pub imported_at: String,
    pub total: usize,
    pub importable: usize,
    pub disabled: usize,
    pub invalid: usize,
    pub duplicate: usize,
    pub rows: Vec<ImportRow>,
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
    for (label, value) in [
        ("匯入時間", &entry.imported_at),
        ("來源檔名", &entry.source_file),
    ] {
        if let Some(value) = value {
            if value.is_empty() || value.chars().count() > MAX_LABEL_CHARS {
                return Some(format!("{label}不可空白或超過 {MAX_LABEL_CHARS} 字。"));
            }
            if value.chars().any(char::is_control) {
                return Some(format!("{label}不可含定位字元或換行。"));
            }
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
        let (Some(direction), true) = (direction, (2..=5).contains(&columns.len())) else {
            return Err(CoreError::new(
                "USER_DICTIONARY_INVALID",
                format!("使用者詞典第 {} 行格式錯誤。", index + 1),
            ));
        };
        entries.push(UserDictionaryEntry {
            simplified: columns[0].to_string(),
            traditional: columns[1].to_string(),
            direction,
            imported_at: label_column(&columns, 3),
            source_file: label_column(&columns, 4),
        });
    }
    check_conflicts(&entries)?;
    Ok(entries)
}

fn label_column(columns: &[&str], index: usize) -> Option<String> {
    columns
        .get(index)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

pub fn serialize(entries: &[UserDictionaryEntry]) -> String {
    let mut text = String::from(NATIVE_HEADER);
    text.push('\n');
    for entry in entries {
        text.push_str(&entry.simplified);
        text.push('\t');
        text.push_str(&entry.traditional);
        text.push('\t');
        text.push_str(entry.direction.as_str());
        if entry.imported_at.is_some() || entry.source_file.is_some() {
            text.push('\t');
            text.push_str(entry.imported_at.as_deref().unwrap_or_default());
            text.push('\t');
            text.push_str(entry.source_file.as_deref().unwrap_or_default());
        }
        text.push('\n');
    }
    text
}

const NATIVE_HEADER: &str =
    "# ConvertZZ 使用者詞典：簡體詞\\t繁體詞\\t方向（both｜s2t｜t2s）\\t匯入時間\\t來源檔名";

pub fn load(path: &Path) -> Result<Vec<UserDictionaryEntry>, CoreError> {
    match fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error.into()),
    }
}

/// 先寫同目錄暫存檔並重讀驗證，成功後才取代正式檔。
pub fn save(path: &Path, entries: &[UserDictionaryEntry]) -> Result<(), CoreError> {
    write_verified(path, entries)
}

/// 以本格式匯出到使用者在存檔對話框選的路徑；同樣暫存驗證後才取代。
pub fn export(path: &Path, entries: &[UserDictionaryEntry]) -> Result<(), CoreError> {
    check_extension(path)?;
    write_verified(path, entries)
}

fn write_verified(path: &Path, entries: &[UserDictionaryEntry]) -> Result<(), CoreError> {
    check_conflicts(entries)?;
    let (Some(directory), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(CoreError::new(
            "USER_DICTIONARY_PATH",
            "使用者詞典路徑無效。",
        ));
    };
    fs::create_dir_all(directory)?;
    let text = serialize(entries);
    let stage = directory.join(format!(
        ".{}.{}.stage",
        name.to_string_lossy(),
        uuid::Uuid::new_v4().simple()
    ));
    let stage_guard = super::remove_file_on_drop(&stage);
    let mut file = fs::File::create_new(&stage)?;
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
    std::mem::DropGuard::dismiss(stage_guard);
    Ok(())
}

fn check_extension(path: &Path) -> Result<(), CoreError> {
    let extension = path
        .extension()
        .map(|value| value.to_string_lossy().to_ascii_lowercase());
    if extension.is_some_and(|value| DICTIONARY_EXTENSIONS.contains(&value.as_str())) {
        Ok(())
    } else {
        Err(CoreError::new(
            "USER_DICTIONARY_EXTENSION",
            "字典檔副檔名須為 .txt、.tsv 或 .csv。",
        ))
    }
}

/// 與設定檔同目錄：可攜模式在執行檔旁，安裝版在應用程式資料目錄。
pub fn default_path() -> Option<PathBuf> {
    super::headless::settings_store_path().and_then(|settings| {
        settings
            .parent()
            .map(|directory| directory.join(USER_DICTIONARY_FILE))
    })
}

fn clean_legacy_word(raw: &str) -> String {
    raw.trim_matches(|ch: char| ch.is_whitespace()).to_string()
}

/// 唯讀解析要匯入的字典檔並列出每列結果；不修改來源，也不記住來源路徑。
pub fn preview_import(path: &Path) -> Result<ImportPreview, CoreError> {
    check_extension(path)?;
    let bytes = fs::read(path)?;
    let text = String::from_utf8(bytes).map_err(|_| {
        CoreError::new(
            "USER_DICTIONARY_ENCODING",
            "字典檔不是 UTF-8 編碼；請以 UTF-8 另存後再匯入。",
        )
    })?;
    let text = text.trim_start_matches('\u{feff}');
    let mut preview = ImportPreview {
        path: path.to_string_lossy().into_owned(),
        format: detect_format(text),
        source_file: source_label(path),
        imported_at: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        total: 0,
        importable: 0,
        disabled: 0,
        invalid: 0,
        duplicate: 0,
        rows: Vec::new(),
    };
    match preview.format {
        ImportFormat::Native => preview_native_rows(text, &mut preview),
        ImportFormat::Legacy => preview_legacy_rows(text, &mut preview),
    }
    Ok(preview)
}

/// 第一個非空白列是六欄且首欄為啟用旗標時視為 1.x 字典；其餘一律依本格式解析。
fn detect_format(text: &str) -> ImportFormat {
    let first = text
        .lines()
        .map(|line| line.trim_end_matches('\r'))
        .find(|line| !line.trim().is_empty());
    let Some(line) = first.filter(|line| !line.starts_with('#')) else {
        return ImportFormat::Native;
    };
    let columns: Vec<&str> = line.split('\t').collect();
    if columns.len() >= 6
        && matches!(
            columns[0].trim().to_ascii_lowercase().as_str(),
            "true" | "false" | "1" | "0"
        )
    {
        ImportFormat::Legacy
    } else {
        ImportFormat::Native
    }
}

fn source_label(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let label: String = name
        .chars()
        .map(|ch| if ch.is_control() { '_' } else { ch })
        .take(MAX_LABEL_CHARS)
        .collect();
    if label.is_empty() {
        "未命名".into()
    } else {
        label
    }
}

/// 本格式匯入不沿用檔案內的匯入時間與來源，一律標成這次匯入的檔名與時間。
fn preview_native_rows(text: &str, preview: &mut ImportPreview) {
    let mut s2t: HashMap<String, String> = HashMap::new();
    let mut t2s: HashMap<String, String> = HashMap::new();
    for (index, line) in text.split('\n').enumerate() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        preview.total += 1;
        let columns: Vec<&str> = line.split('\t').collect();
        let mut row = ImportRow {
            line: index + 1,
            simplified: columns[0].to_string(),
            traditional: columns
                .get(1)
                .map(|value| value.to_string())
                .unwrap_or_default(),
            status: ImportStatus::Invalid,
            direction: None,
            reason: None,
        };
        let direction = match columns.get(2) {
            None => Some(UserDictionaryDirection::Both),
            Some(value) => UserDictionaryDirection::parse(value),
        };
        let Some(direction) = direction.filter(|_| (2..=5).contains(&columns.len())) else {
            row.reason = Some("格式錯誤：應為「簡體詞、繁體詞、方向」以定位字元分隔。".into());
            preview.invalid += 1;
            preview.rows.push(row);
            continue;
        };
        let candidate = UserDictionaryEntry {
            simplified: row.simplified.clone(),
            traditional: row.traditional.clone(),
            direction,
            imported_at: None,
            source_file: None,
        };
        if let Some(problem) = entry_problem(&candidate) {
            row.reason = Some(problem);
            preview.invalid += 1;
            preview.rows.push(row);
            continue;
        }
        let s2t_known = direction.s2t().then(|| {
            s2t.get(&row.simplified)
                .map(|target| target == &row.traditional)
        });
        let t2s_known = direction.t2s().then(|| {
            t2s.get(&row.traditional)
                .map(|target| target == &row.simplified)
        });
        let known = [s2t_known, t2s_known];
        if known.iter().flatten().any(|same| *same == Some(false)) {
            row.reason = Some("同方向的來源詞已由前面的列對應到別的詞。".into());
            preview.invalid += 1;
            preview.rows.push(row);
            continue;
        }
        if known.iter().flatten().all(|same| *same == Some(true)) {
            row.status = ImportStatus::Duplicate;
            row.reason = Some("與前面的列相同。".into());
            preview.duplicate += 1;
            preview.rows.push(row);
            continue;
        }
        if direction.s2t() {
            s2t.insert(row.simplified.clone(), row.traditional.clone());
        }
        if direction.t2s() {
            t2s.insert(row.traditional.clone(), row.simplified.clone());
        }
        row.status = ImportStatus::Importable;
        row.direction = Some(direction);
        preview.importable += 1;
        preview.rows.push(row);
    }
}

/// 1.x 六欄（啟用、類型、簡體、優先權、繁體、優先權）。優先權不再使用：新引擎依分詞結果套用整詞。
fn preview_legacy_rows(text: &str, preview: &mut ImportPreview) {
    let mut s2t_sources: HashMap<String, String> = HashMap::new();
    let mut t2s_sources: HashMap<String, String> = HashMap::new();
    for (index, line) in text.split('\n').enumerate() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        preview.total += 1;
        let columns: Vec<&str> = line.split('\t').collect();
        let mut row = ImportRow {
            line: index + 1,
            simplified: columns
                .get(2)
                .map(|value| clean_legacy_word(value))
                .unwrap_or_default(),
            traditional: columns
                .get(4)
                .map(|value| clean_legacy_word(value))
                .unwrap_or_default(),
            status: ImportStatus::Invalid,
            direction: None,
            reason: None,
        };
        if columns.len() < 6 {
            row.reason = Some("不是六欄格式。".into());
            preview.invalid += 1;
            preview.rows.push(row);
            continue;
        }
        if !matches!(columns[0].to_ascii_lowercase().as_str(), "true" | "1") {
            row.status = ImportStatus::Disabled;
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
            imported_at: None,
            source_file: None,
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
                row.status = ImportStatus::Duplicate;
                row.reason = Some("兩個方向都已由前面的列使用。".into());
                preview.duplicate += 1;
                preview.rows.push(row);
                continue;
            }
        };
        if s2t_sources.get(&row.simplified) == Some(&row.traditional)
            && t2s_sources.get(&row.traditional) == Some(&row.simplified)
        {
            row.status = ImportStatus::Duplicate;
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
        row.status = ImportStatus::Importable;
        row.direction = Some(direction);
        preview.importable += 1;
        preview.rows.push(row);
    }
}

#[cfg(test)]
mod tests;

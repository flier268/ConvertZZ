use super::conversion::base_convert;
use super::dictionary::{read_dictionary_entries, LegacyDictionary};
use super::error::CoreError;
use super::types::{
    DictionaryEntry, DictionaryPreviewRequest, DictionaryReadRequest, DictionaryUpdateRequest,
};
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct DictionaryService {
    default_path: Option<PathBuf>,
}

impl DictionaryService {
    pub fn new(default_path: Option<PathBuf>) -> Self {
        Self { default_path }
    }

    pub fn read(&self, request: DictionaryReadRequest) -> Result<serde_json::Value, CoreError> {
        let path = self.resolve_path(request.path.as_deref())?;
        let entries = read_dictionary_entries(&path)?;
        let query = request.query.unwrap_or_default().trim().to_lowercase();
        let mut filtered = if query.is_empty() {
            entries
        } else {
            entries
                .into_iter()
                .filter(|entry| {
                    format!(
                        "{}\t{}\t{}",
                        entry.entry_type, entry.simplified, entry.traditional
                    )
                    .to_lowercase()
                    .contains(&query)
                })
                .collect()
        };
        match request.sort.as_deref() {
            Some("s2t") => filtered.sort_by(|left, right| {
                right
                    .simplified_priority
                    .cmp(&left.simplified_priority)
                    .then_with(|| {
                        right
                            .simplified
                            .chars()
                            .count()
                            .cmp(&left.simplified.chars().count())
                    })
                    .then_with(|| left.index.cmp(&right.index))
            }),
            Some("t2s") => filtered.sort_by(|left, right| {
                right
                    .traditional_priority
                    .cmp(&left.traditional_priority)
                    .then_with(|| {
                        right
                            .traditional
                            .chars()
                            .count()
                            .cmp(&left.traditional.chars().count())
                    })
                    .then_with(|| left.index.cmp(&right.index))
            }),
            _ => filtered.sort_by_key(|entry| entry.index),
        }
        let offset = request.offset.unwrap_or(0);
        let limit = request.limit.unwrap_or(100).clamp(1, 500);
        let total = filtered.len();
        let page = filtered
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect::<Vec<_>>();
        Ok(serde_json::json!({
            "path": path,
            "total": total,
            "offset": offset,
            "entries": page,
        }))
    }

    pub fn update(&self, request: DictionaryUpdateRequest) -> Result<serde_json::Value, CoreError> {
        if request.path.is_empty() {
            return Err(CoreError::new(
                "DICTIONARY_PATH",
                "儲存字典前必須選取可寫入檔案。",
            ));
        }
        let path = PathBuf::from(&request.path);
        let updated = request.updates.as_ref().map(Vec::len).unwrap_or(0)
            + request.inserts.as_ref().map(Vec::len).unwrap_or(0)
            + request.deletes.as_ref().map(Vec::len).unwrap_or(0);
        let raw = fs::read_to_string(&path)?
            .trim_start_matches('\u{feff}')
            .to_string();
        let mut lines = raw.split('\n').map(ToOwned::to_owned).collect::<Vec<_>>();
        for update in request.updates.unwrap_or_default() {
            if update.index >= lines.len() {
                return Err(CoreError::new(
                    "DICTIONARY_INDEX",
                    "字典項目索引已失效。請重新載入。",
                ));
            }
            lines[update.index] = serialize_entry(&update.entry)?;
        }
        let mut deletes = request.deletes.unwrap_or_default();
        deletes.sort_unstable();
        deletes.dedup();
        for index in deletes.into_iter().rev() {
            if index >= lines.len() {
                return Err(CoreError::new(
                    "DICTIONARY_INDEX",
                    "字典項目索引已失效。請重新載入。",
                ));
            }
            lines.remove(index);
        }
        for entry in request.inserts.unwrap_or_default() {
            lines.push(serialize_entry(&entry)?);
        }
        let temporary =
            path.with_file_name(format!(".convertzz-dictionary-{}.csv", Uuid::new_v4()));
        let backup = dictionary_backup_path(&path);
        let transaction_backup = path.with_file_name(format!(
            ".convertzz-dictionary-original-{}.csv",
            Uuid::new_v4()
        ));
        if backup.exists() {
            return Err(CoreError::new(
                "DICTIONARY_BACKUP",
                "字典備份檔已存在，拒絕覆寫。",
            ));
        }
        fs::copy(&path, &backup)?;
        let content = format!("\u{feff}{}", lines.join("\n"));
        fs::write(&temporary, content)?;
        fs::rename(&path, &transaction_backup)?;
        if let Err(error) = fs::rename(&temporary, &path) {
            let _ = fs::rename(&transaction_backup, &path);
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }
        let _ = fs::remove_file(&transaction_backup);
        Ok(serde_json::json!({
            "updated": updated,
            "backupPath": backup,
        }))
    }

    pub fn preview(
        &self,
        request: DictionaryPreviewRequest,
    ) -> Result<serde_json::Value, CoreError> {
        let path = self.resolve_path(request.path.as_deref())?;
        let deleted = request
            .deletes
            .unwrap_or_default()
            .into_iter()
            .collect::<HashSet<_>>();
        let updates = request
            .updates
            .unwrap_or_default()
            .into_iter()
            .map(|update| (update.index, update.entry))
            .collect::<HashMap<_, _>>();
        let mut entries = read_dictionary_entries(&path)?
            .into_iter()
            .filter(|entry| !deleted.contains(&entry.index))
            .map(|entry| {
                updates
                    .get(&entry.index)
                    .cloned()
                    .unwrap_or_else(|| entry.into())
            })
            .collect::<Vec<_>>();
        entries.extend(request.inserts.unwrap_or_default());
        let dictionary = LegacyDictionary::from_entries(entries);
        let text = dictionary.replace(&request.text, request.direction, |value| {
            base_convert(value, request.direction)
        });
        Ok(serde_json::json!({ "text": text }))
    }

    fn resolve_path(&self, path: Option<&str>) -> Result<PathBuf, CoreError> {
        path.filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| self.default_path.clone())
            .ok_or_else(|| CoreError::new("DICTIONARY_MISSING", "找不到字典路徑。"))
    }
}

fn serialize_entry(entry: &DictionaryEntry) -> Result<String, CoreError> {
    if entry.simplified.is_empty() || entry.traditional.is_empty() {
        return Err(CoreError::new(
            "DICTIONARY_ENTRY",
            "簡體與繁體欄位不能留空。",
        ));
    }
    Ok(format!(
        "{}\t{}\t{}\t{}\t{}\t{}",
        entry.enabled,
        entry.entry_type,
        entry.simplified,
        entry.simplified_priority,
        entry.traditional,
        entry.traditional_priority
    ))
}

fn dictionary_backup_path(path: &Path) -> PathBuf {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("csv");
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Dictionary");
    let timestamp = Utc::now().format("%Y%m%d%H%M%S");
    let id = &Uuid::new_v4().to_string()[..8];
    path.with_file_name(format!("{stem}.backup-{timestamp}-{id}.{extension}"))
}

#[cfg(test)]
mod tests;

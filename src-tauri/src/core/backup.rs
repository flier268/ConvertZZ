use super::error::CoreError;
use super::types::ConflictPolicy;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupRoot {
    pub path: PathBuf,
    pub kind: BackupKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupKind {
    File,
    Directory,
}

/// 結尾分隔符號或 `.` 會先正規化，否則 `docs/` 的備份會落在來源目錄內；沒有檔名的路徑（`..`、根目錄）不備份。
pub fn user_backup_path(source: &Path) -> Result<PathBuf, CoreError> {
    let normalized: PathBuf = source.components().collect();
    if normalized.file_name().is_none() {
        return Err(CoreError::with_details(
            "BACKUP_PATH",
            "無法為沒有檔名的路徑建立備份。",
            serde_json::json!({ "path": source }),
        ));
    }
    Ok(normalized.with_added_extension("bak"))
}

pub fn resolve_backup_roots(paths: &[String]) -> Result<Vec<BackupRoot>, CoreError> {
    let mut roots = Vec::new();
    for path in paths {
        if path.contains(['*', '?']) {
            roots.extend(
                expand_wildcard_files(path)?
                    .into_iter()
                    .map(|item| BackupRoot {
                        path: item,
                        kind: BackupKind::File,
                    }),
            );
            continue;
        }
        let absolute = resolve_path(path);
        let Ok(metadata) = fs::symlink_metadata(&absolute) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            roots.push(BackupRoot {
                path: absolute,
                kind: BackupKind::Directory,
            });
        } else if metadata.is_file() {
            roots.push(BackupRoot {
                path: absolute,
                kind: BackupKind::File,
            });
        }
    }
    Ok(prune_nested_backup_roots(roots))
}

pub fn prune_nested_backup_roots(roots: Vec<BackupRoot>) -> Vec<BackupRoot> {
    let mut unique = Vec::new();
    for root in roots {
        if !unique
            .iter()
            .any(|item: &BackupRoot| item.path == root.path)
        {
            unique.push(root);
        }
    }
    let list = unique;
    let mut directories = list
        .iter()
        .filter(|root| root.kind == BackupKind::Directory)
        .map(|root| root.path.clone())
        .collect::<Vec<_>>();
    directories.sort_by_key(|path| path.as_os_str().len());
    list.into_iter()
        .filter(|root| {
            !directories
                .iter()
                .any(|directory| directory != &root.path && path_is_inside(&root.path, directory))
        })
        .collect()
}

pub fn create_user_backups(
    roots: &[BackupRoot],
    affected_paths: &[PathBuf],
    policy: ConflictPolicy,
) -> Result<Vec<PathBuf>, CoreError> {
    if roots.is_empty() || affected_paths.is_empty() {
        return Ok(Vec::new());
    }
    let affected = affected_paths
        .iter()
        .map(|path| resolve_path(&path.to_string_lossy()))
        .collect::<Vec<_>>();
    let mut created = Vec::new();
    for root in roots {
        let covers = affected.iter().any(|path| match root.kind {
            BackupKind::Directory => path_is_inside(path, &root.path),
            BackupKind::File => path == &root.path,
        });
        if covers {
            created.push(create_user_backup(&root.path, policy)?);
        }
    }
    Ok(created)
}

pub fn create_user_backup(
    source_path: &Path,
    policy: ConflictPolicy,
) -> Result<PathBuf, CoreError> {
    let absolute = resolve_path(&source_path.to_string_lossy());
    let target = user_backup_path(&absolute)?;
    let metadata = fs::symlink_metadata(&absolute)?;
    if metadata.file_type().is_symlink() {
        return Err(CoreError::with_details(
            "BACKUP_SYMLINK",
            "不備份符號連結來源。",
            serde_json::json!({ "path": absolute }),
        ));
    }
    let exists = fs::symlink_metadata(&target).is_ok();
    if exists && policy == ConflictPolicy::Skip {
        return Ok(target);
    }
    if !metadata.is_dir() && !metadata.is_file() {
        return Err(CoreError::with_details(
            "BACKUP_UNSUPPORTED",
            "不支援的備份來源類型。",
            serde_json::json!({ "path": absolute }),
        ));
    }
    // 先完整複製到同目錄暫存名稱，成功後才換掉既有備份；複製失敗時既有備份保持不變。
    let staged = sibling_temporary(&target, "convertzz-backup");
    let copied = if metadata.is_dir() {
        copy_dir(&absolute, &staged)
    } else {
        fs::copy(&absolute, &staged).map(|_| ()).map_err(Into::into)
    };
    if let Err(error) = copied {
        remove_any(&staged);
        return Err(error);
    }
    if !exists {
        if let Err(error) = fs::rename(&staged, &target) {
            remove_any(&staged);
            return Err(error.into());
        }
        return Ok(target);
    }
    let previous = sibling_temporary(&target, "convertzz-old-backup");
    if let Err(error) = fs::rename(&target, &previous) {
        remove_any(&staged);
        return Err(error.into());
    }
    if let Err(error) = fs::rename(&staged, &target) {
        let _ = fs::rename(&previous, &target);
        remove_any(&staged);
        return Err(error.into());
    }
    remove_any(&previous);
    Ok(target)
}

pub fn path_is_inside(path: &Path, directory: &Path) -> bool {
    let path = resolve_path(&path.to_string_lossy());
    let directory = resolve_path(&directory.to_string_lossy());
    path == directory || path.starts_with(&directory)
}

fn sibling_temporary(target: &Path, label: &str) -> PathBuf {
    target.with_file_name(format!(".{label}-{}", uuid::Uuid::new_v4()))
}

fn remove_any(path: &Path) {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            let _ = fs::remove_dir_all(path);
        }
        Ok(_) => {
            let _ = fs::remove_file(path);
        }
        Err(_) => {}
    }
}

fn copy_dir(source: &Path, destination: &Path) -> Result<(), CoreError> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let from = entry.path();
        let to = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&from)?;
        if metadata.file_type().is_symlink() {
            #[cfg(unix)]
            {
                let target = fs::read_link(&from)?;
                let _ = fs::remove_file(&to);
                std::os::unix::fs::symlink(target, &to)?;
            }
        } else if metadata.is_dir() {
            copy_dir(&from, &to)?;
        } else if metadata.is_file() {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

fn expand_wildcard_files(pattern: &str) -> Result<Vec<PathBuf>, CoreError> {
    let absolute = resolve_path(pattern);
    let directory = absolute.parent().unwrap_or(Path::new("."));
    let Some(name) = absolute.file_name().and_then(|name| name.to_str()) else {
        return Ok(Vec::new());
    };
    if fs::metadata(directory).is_err() {
        return Ok(Vec::new());
    }
    let matcher = wildcard_matcher(name);
    let mut matches = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            if let Some(file_name) = entry.file_name().to_str() {
                if matcher.is_match(file_name) {
                    matches.push(entry.path());
                }
            }
        }
    }
    matches.sort();
    Ok(matches)
}

pub fn wildcard_matcher(pattern: &str) -> regex::Regex {
    let mut source = String::from("^");
    for character in pattern.chars() {
        match character {
            '*' => source.push_str("(.*?)"),
            '?' => source.push_str("(.)"),
            other => source.push_str(&regex::escape(&other.to_string())),
        }
    }
    source.push('$');
    let flags = if cfg!(windows) { "(?iu)" } else { "(?u)" };
    regex::Regex::new(&format!("{flags}{source}")).expect("wildcard")
}

pub fn resolve_path(path: &str) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        candidate
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(candidate)
    }
}

#[cfg(test)]
mod tests;

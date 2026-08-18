use super::error::CoreError;
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

pub fn user_backup_path(source: &Path) -> PathBuf {
    let mut path = source.as_os_str().to_os_string();
    path.push(".bak");
    PathBuf::from(path)
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
            created.push(create_user_backup(&root.path)?);
        }
    }
    Ok(created)
}

pub fn create_user_backup(source_path: &Path) -> Result<PathBuf, CoreError> {
    let absolute = resolve_path(&source_path.to_string_lossy());
    let target = user_backup_path(&absolute);
    let metadata = fs::symlink_metadata(&absolute)?;
    if metadata.file_type().is_symlink() {
        return Err(CoreError::with_details(
            "BACKUP_SYMLINK",
            "不備份符號連結來源。",
            serde_json::json!({ "path": absolute }),
        ));
    }
    let _ = fs::remove_dir_all(&target);
    let _ = fs::remove_file(&target);
    if metadata.is_dir() {
        copy_dir(&absolute, &target)?;
    } else if metadata.is_file() {
        fs::copy(&absolute, &target)?;
    } else {
        return Err(CoreError::with_details(
            "BACKUP_UNSUPPORTED",
            "不支援的備份來源類型。",
            serde_json::json!({ "path": absolute }),
        ));
    }
    Ok(target)
}

pub fn path_is_inside(path: &Path, directory: &Path) -> bool {
    let path = resolve_path(&path.to_string_lossy());
    let directory = resolve_path(&directory.to_string_lossy());
    path == directory || path.starts_with(&directory)
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

use super::backup::{
    create_user_backups, resolve_backup_roots, resolve_path, wildcard_matcher, BackupRoot,
};
use super::conversion::ConversionService;
use super::encoding::{can_roundtrip, decode_text, encode_text};
use super::error::CoreError;
use super::types::{
    ApplyFailure, ApplyResult, ConflictPolicy, ConversionOptions, FileConversionPlan, FileItemKind,
    FileMode, FilePlanItem, FilePlanRequest, PlanStatus, ProgressReporter, TextEncoding,
};
use chrono::Utc;
use cjk_convert_rs::cjk2zht;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

type ConvertHook = Arc<dyn Fn(&str) -> String + Send + Sync>;
type StageValidator =
    Arc<dyn Fn(&Path, Option<&[u8]>, &Path) -> Result<(), CoreError> + Send + Sync>;

struct PreparedFile {
    item: FilePlanItem,
    content: Option<Vec<u8>>,
    conflict_policy: ConflictPolicy,
}

struct StoredPlan {
    files: Vec<PreparedFile>,
    backup: bool,
    backup_roots: Vec<BackupRoot>,
}

struct TransactionEntry {
    file: PreparedFile,
    stage_path: PathBuf,
    original_backup: Option<PathBuf>,
    conflict_backup: Option<PathBuf>,
    committed: bool,
}

struct DirectoryTransactionEntry {
    item: FilePlanItem,
    temporary_path: PathBuf,
    conflict_backup: Option<PathBuf>,
    committed: bool,
    conflict_policy: ConflictPolicy,
}

pub struct FileService {
    plans: Mutex<std::collections::HashMap<String, StoredPlan>>,
    cancelled: Mutex<HashSet<String>>,
    convert_hook: Option<ConvertHook>,
    stage_validator: Option<StageValidator>,
}

impl FileService {
    pub fn new() -> Self {
        Self {
            plans: Mutex::new(std::collections::HashMap::new()),
            cancelled: Mutex::new(HashSet::new()),
            convert_hook: None,
            stage_validator: None,
        }
    }

    #[cfg(test)]
    pub fn with_convert_hook(
        mut self,
        hook: impl Fn(&str) -> String + Send + Sync + 'static,
    ) -> Self {
        self.convert_hook = Some(Arc::new(hook));
        self
    }

    #[cfg(test)]
    pub fn with_stage_validator(
        mut self,
        validator: impl Fn(&Path, Option<&[u8]>, &Path) -> Result<(), CoreError> + Send + Sync + 'static,
    ) -> Self {
        self.stage_validator = Some(Arc::new(validator));
        self
    }

    async fn convert_text(
        &self,
        conversion: &ConversionService,
        options: &ConversionOptions,
        text: impl Into<String>,
    ) -> Result<super::types::ConversionResult, CoreError> {
        let text = text.into();
        if let Some(hook) = &self.convert_hook {
            return Ok(super::types::ConversionResult {
                text: hook(&text),
                engine: options.engine,
                direction: options.direction,
                warnings: Vec::new(),
                duration_ms: 0.0,
            });
        }
        conversion.convert(options.with_text(text)).await
    }

    pub fn cancel(&self, plan_id: &str) -> serde_json::Value {
        let cancelled = self
            .plans
            .lock()
            .ok()
            .is_some_and(|mut plans| plans.remove(plan_id).is_some());
        if cancelled {
            if let Ok(mut set) = self.cancelled.lock() {
                set.insert(plan_id.to_string());
            }
        }
        serde_json::json!({ "cancelled": cancelled })
    }

    pub async fn plan(
        &self,
        conversion: &ConversionService,
        request: FilePlanRequest,
        progress: ProgressReporter,
    ) -> Result<FileConversionPlan, CoreError> {
        validate_output_pattern(
            request.paths.first().map(String::as_str),
            request.output_path.as_deref(),
        )?;
        let paths = collect_files(
            &request.paths,
            request.recursive,
            request.allowed_extensions.as_deref(),
        )?;
        let directories = if request.mode == FileMode::Content {
            Vec::new()
        } else {
            collect_directories(&request.paths, request.recursive)?
        };
        let mut files = Vec::new();
        let mut warnings = Vec::new();
        let preview_max_bytes = request
            .preview_max_bytes
            .unwrap_or(6 * 1024)
            .clamp(1024, 1024 * 1024) as usize;

        for (index, source_path) in paths.iter().enumerate() {
            match self
                .prepare_file(conversion, &request, source_path, preview_max_bytes)
                .await
            {
                Ok((file, extra_warnings)) => {
                    warnings.extend(extra_warnings);
                    files.push(file);
                }
                Err(error) => files.push(PreparedFile {
                    item: FilePlanItem {
                        source_path: source_path.to_string_lossy().into_owned(),
                        output_path: source_path.to_string_lossy().into_owned(),
                        kind: FileItemKind::File,
                        selected: false,
                        detected_encoding: None,
                        source_preview: String::new(),
                        output_preview: String::new(),
                        status: PlanStatus::Error,
                        warning: Some(error.message),
                    },
                    content: None,
                    conflict_policy: request.conflict_policy,
                }),
            }
            progress(super::types::ProgressEvent {
                current: (index + 1) as u64,
                total: paths.len() as u64,
                message: format!("正在建立預覽：{}", file_name(source_path)),
            });
        }

        if request.output_directory.is_none() && request.output_path.is_none() {
            let mut directories = directories;
            directories.sort_by_key(|path| std::cmp::Reverse(path_depth(path)));
            for source_path in directories {
                let converted_name = self
                    .convert_text(conversion, &request.conversion, file_name(&source_path))
                    .await?
                    .text;
                let output_path = source_path
                    .parent()
                    .unwrap_or(Path::new("."))
                    .join(&converted_name);
                let conflict = output_path != source_path && output_path.exists();
                files.push(PreparedFile {
                    item: FilePlanItem {
                        source_path: source_path.to_string_lossy().into_owned(),
                        output_path: output_path.to_string_lossy().into_owned(),
                        kind: FileItemKind::Directory,
                        selected: true,
                        detected_encoding: None,
                        source_preview: file_name(&source_path),
                        output_preview: converted_name,
                        status: if conflict && request.conflict_policy == ConflictPolicy::Skip {
                            PlanStatus::Conflict
                        } else {
                            PlanStatus::Ready
                        },
                        warning: conflict.then(|| "輸出資料夾已存在。".into()),
                    },
                    content: None,
                    conflict_policy: request.conflict_policy,
                });
            }
        }

        let plan_id = Uuid::new_v4().to_string();
        let public = FileConversionPlan {
            plan_id: plan_id.clone(),
            created_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            items: files.iter().map(|file| file.item.clone()).collect(),
            warnings: unique(warnings),
        };
        if let Ok(mut plans) = self.plans.lock() {
            plans.insert(
                plan_id,
                StoredPlan {
                    files,
                    backup: request.backup != Some(false),
                    backup_roots: resolve_backup_roots(&request.paths)?,
                },
            );
        }
        Ok(public)
    }

    async fn prepare_file(
        &self,
        conversion: &ConversionService,
        request: &FilePlanRequest,
        source_path: &Path,
        preview_max_bytes: usize,
    ) -> Result<(PreparedFile, Vec<String>), CoreError> {
        assert_source_writable(source_path)?;
        let source_buffer = if request.mode == FileMode::Filename {
            None
        } else {
            Some(fs::read(source_path)?)
        };
        let decoded = source_buffer
            .as_deref()
            .map(|buffer| decode_text(buffer, request.input_encoding))
            .transpose()?;
        let converted_content = if let Some((text, _)) = &decoded {
            Some(
                self.convert_text(conversion, &request.conversion, text)
                    .await?,
            )
        } else {
            None
        };
        let converted_name = if request.mode == FileMode::Content {
            file_name(source_path)
        } else {
            self.convert_text(conversion, &request.conversion, file_name(source_path))
                .await?
                .text
        };
        let default_output = source_path
            .parent()
            .unwrap_or(Path::new("."))
            .join(&converted_name);
        let output_path = if let Some(directory) = &request.output_directory {
            resolve_output_directory_path(
                self,
                conversion,
                source_path,
                &request.paths,
                directory,
                &converted_name,
                request.mode,
                &request.conversion,
            )
            .await?
        } else if let Some(pattern) = &request.output_path {
            resolve_requested_output_path(
                source_path,
                request.paths.first().map(String::as_str).unwrap_or(""),
                pattern,
                &converted_name,
                request.mode,
            )
        } else {
            default_output
        };
        let output_encoding =
            resolve_output_encoding(request.output_encoding, decoded.as_ref().map(|item| item.1));
        let mut output_text = converted_content
            .as_ref()
            .map(|item| item.text.clone())
            .unwrap_or_default();
        if converted_content.is_some() && request.fix_charset_declaration {
            output_text = fix_charset_declaration(
                &output_text,
                output_encoding,
                source_path
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or(""),
                request.fix_charset_extensions.as_deref(),
            );
        }
        if converted_content.is_some()
            && request.conversion.direction == super::types::Direction::None
            && output_encoding == TextEncoding::Big5
        {
            output_text = repair_unrepresentable_big5(&output_text);
        }
        let conflict = output_path != source_path && output_path.exists();
        let warnings = converted_content
            .as_ref()
            .map(|item| item.warnings.clone())
            .unwrap_or_default();
        let content = if converted_content.is_some() {
            Some(encode_text(&output_text, output_encoding, request.add_bom)?)
        } else {
            None
        };
        let preview_source = decoded
            .as_ref()
            .map(|(text, _)| truncate(text, preview_max_bytes))
            .unwrap_or_else(|| file_name(source_path));
        let preview_output = if converted_content.is_some() {
            truncate(&output_text, preview_max_bytes)
        } else {
            converted_name
        };
        Ok((
            PreparedFile {
                item: FilePlanItem {
                    source_path: source_path.to_string_lossy().into_owned(),
                    output_path: output_path.to_string_lossy().into_owned(),
                    kind: FileItemKind::File,
                    selected: true,
                    detected_encoding: decoded.map(|item| item.1),
                    source_preview: preview_source,
                    output_preview: preview_output,
                    status: if conflict && request.conflict_policy == ConflictPolicy::Skip {
                        PlanStatus::Conflict
                    } else {
                        PlanStatus::Ready
                    },
                    warning: conflict.then(|| "輸出路徑已存在。".into()),
                },
                content,
                conflict_policy: request.conflict_policy,
            },
            warnings,
        ))
    }

    pub async fn apply(
        &self,
        plan_id: &str,
        selected_paths: Option<&[String]>,
        progress: ProgressReporter,
    ) -> Result<ApplyResult, CoreError> {
        let mut plan = self
            .plans
            .lock()
            .ok()
            .and_then(|mut plans| plans.remove(plan_id))
            .ok_or_else(|| CoreError::new("PLAN_NOT_FOUND", "檔案轉換計畫已失效。請重新預覽。"))?;
        let mut result = ApplyResult {
            succeeded: Vec::new(),
            skipped: Vec::new(),
            failed: Vec::new(),
        };
        let selection = selected_paths.map(|paths| {
            paths
                .iter()
                .map(|path| resolve_path(path))
                .collect::<HashSet<_>>()
        });
        let mut transaction = Vec::new();
        let mut directory_transaction = Vec::new();
        let apply_result = (|| -> Result<(), CoreError> {
            let ready_files = plan
                .files
                .iter()
                .filter(|file| {
                    file.item.status == PlanStatus::Ready
                        && selection
                            .as_ref()
                            .is_none_or(|set| set.contains(&resolve_path(&file.item.source_path)))
                })
                .collect::<Vec<_>>();
            if plan.backup && !ready_files.is_empty() {
                progress(super::types::ProgressEvent {
                    current: 0,
                    total: (ready_files.len() * 2 + 1).max(1) as u64,
                    message: "正在建立備份…".into(),
                });
                create_user_backups(
                    &plan.backup_roots,
                    &ready_files
                        .iter()
                        .map(|file| PathBuf::from(&file.item.source_path))
                        .collect::<Vec<_>>(),
                )?;
            }
            let total = (ready_files.len() * 2).max(1) as u64;
            let mut current = 0_u64;
            let mut remaining = Vec::new();
            for file in std::mem::take(&mut plan.files) {
                self.throw_if_cancelled(plan_id)?;
                if file.item.kind == FileItemKind::Directory {
                    remaining.push(file);
                    continue;
                }
                if file.item.status != PlanStatus::Ready
                    || selection
                        .as_ref()
                        .is_some_and(|set| !set.contains(&resolve_path(&file.item.source_path)))
                {
                    result.skipped.push(file.item.source_path);
                    continue;
                }
                if file.item.output_path == file.item.source_path && file.content.is_none() {
                    result.skipped.push(file.item.source_path);
                    continue;
                }
                let source = PathBuf::from(&file.item.source_path);
                assert_source_writable(&source)?;
                let output = PathBuf::from(&file.item.output_path);
                let stage_path = transaction_path(&output, "stage");
                if let Some(parent) = stage_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                if let Some(content) = &file.content {
                    write_stage(&stage_path, content, &source)?;
                } else {
                    fs::copy(&source, &stage_path)?;
                }
                verify_stage(&stage_path, file.content.as_deref(), &source)?;
                if let Some(validator) = &self.stage_validator {
                    if let Err(error) = validator(&stage_path, file.content.as_deref(), &source) {
                        let _ = fs::remove_file(&stage_path);
                        return Err(error);
                    }
                }
                transaction.push(TransactionEntry {
                    file,
                    stage_path,
                    original_backup: None,
                    conflict_backup: None,
                    committed: false,
                });
                current += 1;
                progress(super::types::ProgressEvent {
                    current,
                    total,
                    message: format!("正在準備：{}", file_name(&source)),
                });
            }

            for entry in &mut transaction {
                self.throw_if_cancelled(plan_id)?;
                let original =
                    transaction_path(Path::new(&entry.file.item.source_path), "original");
                fs::rename(&entry.file.item.source_path, &original)?;
                entry.original_backup = Some(original);
            }

            for entry in &mut transaction {
                self.throw_if_cancelled(plan_id)?;
                let output = PathBuf::from(&entry.file.item.output_path);
                let source = PathBuf::from(&entry.file.item.source_path);
                if output != source && output.exists() {
                    if entry.file.conflict_policy == ConflictPolicy::Skip {
                        let _ = fs::remove_file(&entry.stage_path);
                        if let Some(backup) = entry.original_backup.take() {
                            let _ = fs::rename(backup, &source);
                        }
                        result.skipped.push(entry.file.item.source_path.clone());
                        continue;
                    }
                    let conflict = transaction_path(&output, "conflict");
                    fs::rename(&output, &conflict)?;
                    entry.conflict_backup = Some(conflict);
                }
                fs::rename(&entry.stage_path, &output)?;
                entry.committed = true;
                current += 1;
                progress(super::types::ProgressEvent {
                    current,
                    total,
                    message: format!("正在寫入：{}", file_name(&output)),
                });
            }

            let mut directory_items = remaining
                .into_iter()
                .filter(|item| item.item.kind == FileItemKind::Directory)
                .collect::<Vec<_>>();
            directory_items.sort_by_key(|item| {
                std::cmp::Reverse(path_depth(Path::new(&item.item.source_path)))
            });
            for item in directory_items {
                self.throw_if_cancelled(plan_id)?;
                if item.item.status != PlanStatus::Ready
                    || selection
                        .as_ref()
                        .is_some_and(|set| !set.contains(&resolve_path(&item.item.source_path)))
                    || item.item.output_path == item.item.source_path
                {
                    result.skipped.push(item.item.source_path);
                    continue;
                }
                let mut entry = DirectoryTransactionEntry {
                    temporary_path: transaction_path(
                        Path::new(&item.item.source_path),
                        "directory",
                    ),
                    item: item.item,
                    conflict_backup: None,
                    committed: false,
                    conflict_policy: item.conflict_policy,
                };
                fs::rename(&entry.item.source_path, &entry.temporary_path)?;
                if Path::new(&entry.item.output_path).exists() {
                    if entry.conflict_policy == ConflictPolicy::Skip {
                        let _ = fs::rename(&entry.temporary_path, &entry.item.source_path);
                        result.skipped.push(entry.item.source_path.clone());
                        continue;
                    }
                    let conflict = transaction_path(Path::new(&entry.item.output_path), "conflict");
                    fs::rename(&entry.item.output_path, &conflict)?;
                    entry.conflict_backup = Some(conflict);
                }
                fs::rename(&entry.temporary_path, &entry.item.output_path)?;
                entry.committed = true;
                current += 2;
                progress(super::types::ProgressEvent {
                    current,
                    total,
                    message: format!(
                        "正在重新命名資料夾：{}",
                        file_name(Path::new(&entry.item.output_path))
                    ),
                });
                directory_transaction.push(entry);
            }
            Ok(())
        })();

        if let Err(error) = apply_result {
            rollback_directories(&directory_transaction);
            rollback_transaction(&transaction);
            if let Ok(mut cancelled) = self.cancelled.lock() {
                cancelled.remove(plan_id);
            }
            if error.code == "PLAN_CANCELLED" {
                return Err(error);
            }
            result.failed.push(ApplyFailure {
                path: "批次作業".into(),
                message: error.message,
            });
            return Ok(result);
        }

        for entry in transaction {
            if !entry.committed {
                continue;
            }
            result.succeeded.push(
                resolve_committed_directory_path(
                    Path::new(&entry.file.item.output_path),
                    &directory_transaction,
                )
                .to_string_lossy()
                .into_owned(),
            );
            for backup in [entry.original_backup, entry.conflict_backup]
                .into_iter()
                .flatten()
            {
                let effective = resolve_committed_directory_path(&backup, &directory_transaction);
                if let Err(error) =
                    fs::remove_file(&effective).or_else(|_| fs::remove_dir_all(&effective))
                {
                    result.failed.push(ApplyFailure {
                        path: effective.to_string_lossy().into_owned(),
                        message: format!("已完成轉換，但無法清除復原暫存檔。{error}"),
                    });
                }
            }
        }
        for entry in directory_transaction {
            if entry.committed {
                result.succeeded.push(entry.item.output_path);
            }
            if let Some(backup) = entry.conflict_backup {
                if let Err(error) = fs::remove_dir_all(&backup) {
                    result.failed.push(ApplyFailure {
                        path: backup.to_string_lossy().into_owned(),
                        message: format!("已完成轉換，但無法清除復原暫存資料夾。{error}"),
                    });
                }
            }
        }
        if let Ok(mut cancelled) = self.cancelled.lock() {
            cancelled.remove(plan_id);
        }
        Ok(result)
    }

    fn throw_if_cancelled(&self, plan_id: &str) -> Result<(), CoreError> {
        if self
            .cancelled
            .lock()
            .ok()
            .is_some_and(|set| set.contains(plan_id))
        {
            return Err(CoreError::new("PLAN_CANCELLED", "檔案作業已由使用者取消。"));
        }
        Ok(())
    }
}

fn collect_files(
    inputs: &[String],
    recursive: bool,
    allowed_extensions: Option<&[String]>,
) -> Result<Vec<PathBuf>, CoreError> {
    let mut collected = HashSet::new();
    let allowed = allowed_extensions
        .unwrap_or(&[])
        .iter()
        .map(|extension| {
            if extension.starts_with('.') {
                extension.to_ascii_lowercase()
            } else {
                format!(".{}", extension.to_ascii_lowercase())
            }
        })
        .collect::<HashSet<_>>();
    for path in inputs {
        visit_files(path, recursive, false, &allowed, &mut collected)?;
    }
    let mut files = collected.into_iter().collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn visit_files(
    path: &str,
    recursive: bool,
    discovered: bool,
    allowed: &HashSet<String>,
    collected: &mut HashSet<PathBuf>,
) -> Result<(), CoreError> {
    let absolute = resolve_path(path);
    if path.contains(['*', '?']) {
        let directory = absolute.parent().unwrap_or(Path::new("."));
        let matcher = wildcard_matcher(
            absolute
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("*"),
        );
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                if let Some(name) = entry.file_name().to_str() {
                    if matcher.is_match(name) {
                        collected.insert(entry.path());
                    }
                }
            }
        }
        return Ok(());
    }
    let metadata = fs::symlink_metadata(&absolute)?;
    if metadata.file_type().is_symlink() {
        return Ok(());
    }
    if metadata.is_file() {
        if !discovered || allowed.is_empty() || allowed.contains(&extension_of(&absolute)) {
            collected.insert(absolute);
        }
        return Ok(());
    }
    if !metadata.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(&absolute)? {
        let entry = entry?;
        if entry.file_type()?.is_symlink() {
            continue;
        }
        if entry.file_type()?.is_file()
            && (allowed.is_empty() || allowed.contains(&extension_of(&entry.path())))
        {
            collected.insert(entry.path());
        } else if recursive && entry.file_type()?.is_dir() {
            visit_files(
                &entry.path().to_string_lossy(),
                true,
                true,
                allowed,
                collected,
            )?;
        }
    }
    Ok(())
}

fn collect_directories(inputs: &[String], recursive: bool) -> Result<Vec<PathBuf>, CoreError> {
    if !recursive {
        return Ok(Vec::new());
    }
    let mut collected = HashSet::new();
    for path in inputs {
        if path.contains(['*', '?']) {
            continue;
        }
        visit_directories(&resolve_path(path), &mut collected)?;
    }
    Ok(collected.into_iter().collect())
}

fn visit_directories(path: &Path, collected: &mut HashSet<PathBuf>) -> Result<(), CoreError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_symlink() || !entry.file_type()?.is_dir() {
            continue;
        }
        collected.insert(entry.path());
        visit_directories(&entry.path(), collected)?;
    }
    Ok(())
}

fn validate_output_pattern(
    input_pattern: Option<&str>,
    output_pattern: Option<&str>,
) -> Result<(), CoreError> {
    let (Some(input), Some(output)) = (input_pattern, output_pattern) else {
        return Ok(());
    };
    if !input.contains('*') {
        return Ok(());
    }
    let input_wildcards = input.matches('*').count();
    let output_wildcards = output.matches('*').count();
    if output_wildcards > 0 && output_wildcards != input_wildcards {
        return Err(CoreError::new(
            "CLI_WILDCARD",
            "輸入與輸出路徑的萬用字元數量不同。",
        ));
    }
    let resolved = resolve_path(output);
    if output_wildcards == 0 && resolved.is_file() {
        return Err(CoreError::new(
            "CLI_OUTPUT",
            "多檔輸入的輸出路徑不能是既有檔案。",
        ));
    }
    Ok(())
}

fn resolve_requested_output_path(
    source_path: &Path,
    input_pattern: &str,
    output_pattern: &str,
    converted_name: &str,
    mode: FileMode,
) -> PathBuf {
    let absolute_output = resolve_path(output_pattern);
    if !input_pattern.contains(['*', '?']) {
        return if mode == FileMode::Content {
            absolute_output
        } else {
            absolute_output
                .parent()
                .unwrap_or(Path::new("."))
                .join(converted_name)
        };
    }
    let matcher = wildcard_matcher(
        Path::new(input_pattern)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("*"),
    );
    let source_name = file_name(source_path);
    let captures = matcher.captures(&source_name);
    if output_pattern.contains('*') {
        let mut capture = 1;
        let mut replaced = String::new();
        for character in output_pattern.chars() {
            if character == '*' {
                replaced.push_str(
                    captures
                        .as_ref()
                        .and_then(|item| item.get(capture))
                        .map(|item| item.as_str())
                        .unwrap_or(""),
                );
                capture += 1;
            } else {
                replaced.push(character);
            }
        }
        return resolve_path(&replaced);
    }
    absolute_output.join(if mode == FileMode::Content {
        source_name
    } else {
        converted_name.to_string()
    })
}

async fn resolve_output_directory_path(
    files: &FileService,
    conversion: &ConversionService,
    source_path: &Path,
    inputs: &[String],
    output_directory: &str,
    converted_name: &str,
    mode: FileMode,
    conversion_request: &ConversionOptions,
) -> Result<PathBuf, CoreError> {
    let first_input = resolve_path(
        inputs
            .first()
            .unwrap_or(&source_path.to_string_lossy().into_owned()),
    );
    let base = if inputs.len() == 1 && !inputs[0].contains(['*', '?']) {
        first_input.clone()
    } else {
        first_input.parent().unwrap_or(Path::new(".")).to_path_buf()
    };
    let relative = source_path
        .strip_prefix(&base)
        .unwrap_or(Path::new(source_path.file_name().unwrap_or_default()));
    let relative_directory = relative
        .parent()
        .map(|parent| {
            parent
                .iter()
                .filter_map(|part| part.to_str().map(ToOwned::to_owned))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut parts = Vec::new();
    if mode == FileMode::Content {
        parts.extend(relative_directory);
    } else {
        for part in relative_directory {
            parts.push(
                files
                    .convert_text(conversion, conversion_request, part)
                    .await?
                    .text,
            );
        }
    }
    let mut output = resolve_path(output_directory);
    for part in parts {
        output.push(part);
    }
    output.push(converted_name);
    Ok(output)
}

fn resolve_output_encoding(
    requested: TextEncoding,
    detected: Option<TextEncoding>,
) -> TextEncoding {
    if requested != TextEncoding::Auto {
        requested
    } else {
        detected
            .filter(|value| *value != TextEncoding::Auto)
            .unwrap_or(TextEncoding::Utf8)
    }
}

fn repair_unrepresentable_big5(text: &str) -> String {
    text.chars()
        .map(|character| {
            let value = character.to_string();
            if can_roundtrip(&value, TextEncoding::Big5) {
                value
            } else {
                cjk2zht(&value)
            }
        })
        .collect()
}

fn fix_charset_declaration(
    text: &str,
    encoding: TextEncoding,
    extension: &str,
    configured: Option<&[String]>,
) -> String {
    let extensions = configured
        .filter(|items| !items.is_empty())
        .map(|items| {
            items
                .iter()
                .map(|value| {
                    let trimmed = value.trim().to_ascii_lowercase();
                    if trimmed.starts_with('.') {
                        trimmed
                    } else {
                        format!(".{trimmed}")
                    }
                })
                .collect::<HashSet<_>>()
        })
        .unwrap_or_else(|| {
            [
                ".htm", ".html", ".shtm", ".shtml", ".asp", ".aspx", ".php", ".css",
            ]
            .into_iter()
            .map(str::to_string)
            .collect()
        });
    let extension = if extension.starts_with('.') {
        extension.to_ascii_lowercase()
    } else {
        format!(".{}", extension.to_ascii_lowercase())
    };
    if !extensions.contains(&extension) {
        return text.to_string();
    }
    let charset = match encoding {
        TextEncoding::Utf8 | TextEncoding::Utf8Bom | TextEncoding::Auto => "utf-8",
        TextEncoding::Utf16le => "utf-16le",
        TextEncoding::Utf16be => "utf-16be",
        TextEncoding::Big5 => "big5",
        TextEncoding::Gbk => "gbk",
        TextEncoding::ShiftJis => "shift_jis",
        TextEncoding::EucJp => "euc-jp",
        TextEncoding::Iso2022Jp => "iso-2022-jp",
        TextEncoding::HzGb2312 => "hz-gb-2312",
    };
    let meta =
        regex::Regex::new(r#"(?i)(<meta\s+[^>]*charset\s*=\s*["']?)[^\s"'/>]+"#).expect("meta");
    let at = regex::Regex::new(r#"(@charset\s+["'])[^"']+(["'])"#).expect("at");
    let content = regex::Regex::new(r#"(?i)(content\s*=\s*["'][^"']*charset\s*=\s*)[^\s"';]+"#)
        .expect("content");
    let mut output = meta
        .replace_all(text, format!("${{1}}{charset}"))
        .into_owned();
    output = at
        .replace_all(&output, format!("${{1}}{charset}${{2}}"))
        .into_owned();
    content
        .replace_all(&output, format!("${{1}}{charset}"))
        .into_owned()
}

fn write_stage(path: &Path, content: &[u8], source_path: &Path) -> Result<(), CoreError> {
    let result = (|| -> Result<(), CoreError> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(content)?;
        file.sync_all()?;
        if let Ok(source) = fs::metadata(source_path) {
            let _ = file.set_permissions(source.permissions());
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result
}

fn assert_source_writable(path: &Path) -> Result<(), CoreError> {
    let source = fs::metadata(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if source.permissions().mode() & 0o222 == 0 {
            return Err(CoreError::with_details(
                "FILE_READONLY",
                "來源檔案為唯讀，無法安全取代。",
                serde_json::json!({ "path": path }),
            ));
        }
    }
    Ok(())
}

fn verify_stage(path: &Path, expected: Option<&[u8]>, source_path: &Path) -> Result<(), CoreError> {
    let staged = fs::read(path)?;
    let comparison = match expected {
        Some(bytes) => bytes.to_vec(),
        None => fs::read(source_path)?,
    };
    if staged != comparison {
        let _ = fs::remove_file(path);
        return Err(CoreError::with_details(
            "FILE_VERIFY",
            "暫存檔寫入驗證失敗。",
            serde_json::json!({ "path": path }),
        ));
    }
    Ok(())
}

fn rollback_transaction(transaction: &[TransactionEntry]) {
    for entry in transaction.iter().rev() {
        if entry.committed {
            let _ = fs::remove_file(&entry.file.item.output_path);
        }
    }
    for entry in transaction.iter().rev() {
        if let Some(backup) = &entry.original_backup {
            if backup.exists() && !Path::new(&entry.file.item.source_path).exists() {
                let _ = fs::rename(backup, &entry.file.item.source_path);
            }
        }
    }
    for entry in transaction.iter().rev() {
        if let Some(backup) = &entry.conflict_backup {
            if backup.exists() && !Path::new(&entry.file.item.output_path).exists() {
                let _ = fs::rename(backup, &entry.file.item.output_path);
            }
        }
        if entry.stage_path.exists() {
            let _ = fs::remove_file(&entry.stage_path);
        }
    }
}

fn rollback_directories(transaction: &[DirectoryTransactionEntry]) {
    for entry in transaction.iter().rev() {
        if entry.committed
            && Path::new(&entry.item.output_path).exists()
            && !Path::new(&entry.item.source_path).exists()
        {
            let _ = fs::rename(&entry.item.output_path, &entry.item.source_path);
        } else if !entry.committed
            && entry.temporary_path.exists()
            && !Path::new(&entry.item.source_path).exists()
        {
            let _ = fs::rename(&entry.temporary_path, &entry.item.source_path);
        }
        if let Some(backup) = &entry.conflict_backup {
            if backup.exists() && !Path::new(&entry.item.output_path).exists() {
                let _ = fs::rename(backup, &entry.item.output_path);
            }
        }
    }
}

fn transaction_path(path: &Path, kind: &str) -> PathBuf {
    path.with_file_name(format!(
        ".convertzz-{kind}-{}{}",
        Uuid::new_v4(),
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{value}"))
            .unwrap_or_default()
    ))
}

fn path_depth(path: &Path) -> usize {
    resolve_path(&path.to_string_lossy())
        .components()
        .filter(|component| {
            !matches!(
                component,
                std::path::Component::RootDir | std::path::Component::Prefix(_)
            )
        })
        .count()
}

fn resolve_committed_directory_path(
    path: &Path,
    transaction: &[DirectoryTransactionEntry],
) -> PathBuf {
    transaction
        .iter()
        .fold(path.to_path_buf(), |current, entry| {
            if !entry.committed {
                return current;
            }
            current
                .strip_prefix(&entry.item.source_path)
                .map(|suffix| Path::new(&entry.item.output_path).join(suffix))
                .unwrap_or(current)
        })
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string()
}

fn extension_of(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{}", value.to_ascii_lowercase()))
        .unwrap_or_default()
}

fn truncate(text: &str, max_units: usize) -> String {
    text.chars().take(max_units).collect()
}

fn unique(items: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(item.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::conversion::shared_conversion;
    use super::super::encoding::encode_text;
    use super::super::types::{ConversionOptions, Direction, EngineKind, FileMode};
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use uuid::Uuid;

    fn noop() -> ProgressReporter {
        Arc::new(|_| {})
    }

    fn temp_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!("convertzz-files-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn conversion_s2t() -> ConversionOptions {
        ConversionOptions {
            direction: Direction::S2t,
            engine: EngineKind::Segmented,
            dictionary_path: None,
            zhconvert: None,
            vocabulary_correction: None,
        }
    }

    fn filename_request(path: &Path, policy: ConflictPolicy) -> FilePlanRequest {
        FilePlanRequest {
            paths: vec![path.to_string_lossy().into_owned()],
            output_path: None,
            output_directory: None,
            mode: FileMode::Filename,
            recursive: false,
            input_encoding: TextEncoding::Auto,
            output_encoding: TextEncoding::Auto,
            add_bom: false,
            fix_charset_declaration: false,
            fix_charset_extensions: None,
            allowed_extensions: None,
            preview_max_bytes: None,
            conflict_policy: policy,
            backup: Some(false),
            conversion: conversion_s2t(),
        }
    }

    fn names(directory: &Path) -> Vec<String> {
        let mut items = std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        items.sort();
        items
    }

    #[tokio::test]
    async fn preview_limit_and_unicode_bom() {
        let directory = temp_dir();
        let path = directory.join("note.txt");
        let source = "里面".repeat(800);
        std::fs::write(&path, &source).unwrap();
        let service = FileService::new();
        let plan = service
            .plan(
                shared_conversion(),
                FilePlanRequest {
                    paths: vec![path.to_string_lossy().into_owned()],
                    output_path: None,
                    output_directory: None,
                    mode: FileMode::Content,
                    recursive: false,
                    input_encoding: TextEncoding::Utf8,
                    output_encoding: TextEncoding::Utf8,
                    add_bom: true,
                    fix_charset_declaration: false,
                    fix_charset_extensions: None,
                    allowed_extensions: None,
                    preview_max_bytes: Some(1024),
                    conflict_policy: ConflictPolicy::Skip,
                    backup: Some(false),
                    conversion: conversion_s2t(),
                },
                noop(),
            )
            .await
            .unwrap();
        assert_eq!(
            plan.items[0].source_preview,
            source.chars().take(1024).collect::<String>()
        );
        assert_eq!(plan.items[0].output_preview.chars().count(), 1024);
        let result = service.apply(&plan.plan_id, None, noop()).await.unwrap();
        assert!(result.failed.is_empty());
        let written = std::fs::read(&path).unwrap();
        assert_eq!(&written[..3], &[0xef, 0xbb, 0xbf]);
        assert_eq!(
            String::from_utf8(written[3..].to_vec()).unwrap(),
            "裡面".repeat(800)
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn preview_then_safe_write_and_fix_charset() {
        let directory = temp_dir();
        let path = directory.join("里面.html");
        std::fs::write(
            &path,
            encode_text(r#"<meta charset="gbk">里面开发"#, TextEncoding::Gbk, false).unwrap(),
        )
        .unwrap();
        let service = FileService::new();
        let plan = service
            .plan(
                shared_conversion(),
                FilePlanRequest {
                    paths: vec![path.to_string_lossy().into_owned()],
                    output_path: None,
                    output_directory: None,
                    mode: FileMode::Content,
                    recursive: false,
                    input_encoding: TextEncoding::Auto,
                    output_encoding: TextEncoding::Utf8,
                    add_bom: false,
                    fix_charset_declaration: true,
                    fix_charset_extensions: None,
                    allowed_extensions: None,
                    preview_max_bytes: None,
                    conflict_policy: ConflictPolicy::Skip,
                    backup: Some(false),
                    conversion: conversion_s2t(),
                },
                noop(),
            )
            .await
            .unwrap();
        assert!(plan.items[0].source_preview.contains("里面开发"));
        assert!(plan.items[0].output_preview.contains("裡面開發"));
        let result = service.apply(&plan.plan_id, None, noop()).await.unwrap();
        assert!(result.failed.is_empty());
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .contains(r#"<meta charset="utf-8">裡面開發"#));
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn skips_same_name_conflicts_by_default() {
        let directory = temp_dir();
        let source = directory.join("里面.txt");
        let output = directory.join("裡面.txt");
        std::fs::write(&source, "來源").unwrap();
        std::fs::write(&output, "既有目標").unwrap();
        let service = FileService::new();
        let plan = service
            .plan(
                shared_conversion(),
                filename_request(&source, ConflictPolicy::Skip),
                noop(),
            )
            .await
            .unwrap();
        assert_eq!(plan.items[0].status, PlanStatus::Conflict);
        let result = service.apply(&plan.plan_id, None, noop()).await.unwrap();
        assert_eq!(result.skipped, [source.to_string_lossy().into_owned()]);
        assert_eq!(std::fs::read_to_string(&source).unwrap(), "來源");
        assert_eq!(std::fs::read_to_string(&output).unwrap(), "既有目標");
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn cancel_rejects_old_plan() {
        let directory = temp_dir();
        let source = directory.join("里面.txt");
        std::fs::write(&source, "來源").unwrap();
        let service = FileService::new();
        let plan = service
            .plan(
                shared_conversion(),
                filename_request(&source, ConflictPolicy::Skip),
                noop(),
            )
            .await
            .unwrap();
        assert_eq!(service.cancel(&plan.plan_id)["cancelled"], true);
        let error = service
            .apply(&plan.plan_id, None, noop())
            .await
            .unwrap_err();
        assert_eq!(error.code, "PLAN_NOT_FOUND");
        assert_eq!(std::fs::read_to_string(&source).unwrap(), "來源");
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn overwrite_clears_transaction_temp_files() {
        let directory = temp_dir();
        let source = directory.join("里面.txt");
        let output = directory.join("裡面.txt");
        std::fs::write(&source, "來源").unwrap();
        std::fs::write(&output, "既有目標").unwrap();
        let service = FileService::new();
        let plan = service
            .plan(
                shared_conversion(),
                filename_request(&source, ConflictPolicy::Overwrite),
                noop(),
            )
            .await
            .unwrap();
        let result = service.apply(&plan.plan_id, None, noop()).await.unwrap();
        assert!(result.failed.is_empty());
        assert_eq!(result.succeeded, [output.to_string_lossy().into_owned()]);
        assert_eq!(std::fs::read_to_string(&output).unwrap(), "來源");
        assert!(names(&directory)
            .into_iter()
            .all(|name| !name.starts_with(".convertzz-")));
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn expands_wildcards_and_output_pattern() {
        let directory = temp_dir();
        let source_directory = directory.join("source");
        let output_directory = directory.join("output");
        std::fs::create_dir(&source_directory).unwrap();
        std::fs::write(source_directory.join("one.txt"), "里面开发").unwrap();
        std::fs::write(source_directory.join("two.log"), "不会选取").unwrap();
        let service = FileService::new();
        let plan = service
            .plan(
                shared_conversion(),
                FilePlanRequest {
                    paths: vec![source_directory
                        .join("*.txt")
                        .to_string_lossy()
                        .into_owned()],
                    output_path: Some(
                        output_directory
                            .join("*.txt")
                            .to_string_lossy()
                            .into_owned(),
                    ),
                    output_directory: None,
                    mode: FileMode::Content,
                    recursive: false,
                    input_encoding: TextEncoding::Utf8,
                    output_encoding: TextEncoding::Utf8,
                    add_bom: false,
                    fix_charset_declaration: false,
                    fix_charset_extensions: None,
                    allowed_extensions: None,
                    preview_max_bytes: None,
                    conflict_policy: ConflictPolicy::Skip,
                    backup: Some(false),
                    conversion: conversion_s2t(),
                },
                noop(),
            )
            .await
            .unwrap();
        assert_eq!(plan.items.len(), 1);
        assert_eq!(
            plan.items[0].output_path,
            output_directory.join("one.txt").to_string_lossy()
        );
        let result = service.apply(&plan.plan_id, None, noop()).await.unwrap();
        assert!(result.failed.is_empty());
        assert_eq!(
            std::fs::read_to_string(output_directory.join("one.txt")).unwrap(),
            "裡面開發"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn rejects_mismatched_wildcard_counts() {
        let directory = temp_dir();
        let service = FileService::new();
        let error = service
            .plan(
                shared_conversion(),
                FilePlanRequest {
                    paths: vec![directory.join("*.txt").to_string_lossy().into_owned()],
                    output_path: Some(directory.join("*.*.txt").to_string_lossy().into_owned()),
                    output_directory: None,
                    mode: FileMode::Content,
                    recursive: false,
                    input_encoding: TextEncoding::Utf8,
                    output_encoding: TextEncoding::Utf8,
                    add_bom: false,
                    fix_charset_declaration: false,
                    fix_charset_extensions: None,
                    allowed_extensions: None,
                    preview_max_bytes: None,
                    conflict_policy: ConflictPolicy::Skip,
                    backup: Some(false),
                    conversion: conversion_s2t(),
                },
                noop(),
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, "CLI_WILDCARD");
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn recursive_rename_keeps_nested_files() {
        let directory = temp_dir();
        let source_directory = directory.join("里面资料");
        let source_file = source_directory.join("开发.txt");
        std::fs::create_dir(&source_directory).unwrap();
        std::fs::write(&source_file, "內容").unwrap();
        let service = FileService::new();
        let mut request = filename_request(&directory, ConflictPolicy::Skip);
        request.recursive = true;
        request.allowed_extensions = Some(vec!["txt".into()]);
        let plan = service
            .plan(shared_conversion(), request, noop())
            .await
            .unwrap();
        assert!(plan.items.iter().any(|item| {
            item.source_path == source_directory.to_string_lossy()
                && item.output_path == directory.join("裡面資料").to_string_lossy()
                && item.kind == FileItemKind::Directory
        }));
        assert!(plan.items.iter().any(|item| {
            item.source_path == source_file.to_string_lossy()
                && item.output_path == source_directory.join("開發.txt").to_string_lossy()
                && item.kind == FileItemKind::File
        }));
        let result = service.apply(&plan.plan_id, None, noop()).await.unwrap();
        assert!(result.failed.is_empty());
        assert!(result.succeeded.contains(
            &directory
                .join("裡面資料")
                .join("開發.txt")
                .to_string_lossy()
                .into_owned()
        ));
        assert!(result
            .succeeded
            .contains(&directory.join("裡面資料").to_string_lossy().into_owned()));
        assert_eq!(
            std::fs::read_to_string(directory.join("裡面資料").join("開發.txt")).unwrap(),
            "內容"
        );
        assert_eq!(names(&directory), ["裡面資料"]);
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn directory_input_respects_extension_filter() {
        let directory = temp_dir();
        let nested = directory.join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(directory.join("one.txt"), "一").unwrap();
        std::fs::write(directory.join("two.log"), "二").unwrap();
        std::fs::write(nested.join("three.TXT"), "三").unwrap();
        std::fs::write(nested.join("four.md"), "四").unwrap();
        let service = FileService::new();
        let plan = service
            .plan(
                shared_conversion(),
                FilePlanRequest {
                    paths: vec![directory.to_string_lossy().into_owned()],
                    output_path: None,
                    output_directory: None,
                    mode: FileMode::Content,
                    recursive: true,
                    input_encoding: TextEncoding::Utf8,
                    output_encoding: TextEncoding::Utf8,
                    add_bom: false,
                    fix_charset_declaration: false,
                    fix_charset_extensions: None,
                    allowed_extensions: Some(vec![".txt".into()]),
                    preview_max_bytes: None,
                    conflict_policy: ConflictPolicy::Skip,
                    backup: Some(false),
                    conversion: ConversionOptions {
                        direction: Direction::None,
                        engine: EngineKind::Segmented,
                        dictionary_path: None,
                        zhconvert: None,
                        vocabulary_correction: None,
                    },
                },
                noop(),
            )
            .await
            .unwrap();
        let mut sources: Vec<_> = plan
            .items
            .iter()
            .map(|item| item.source_path.clone())
            .collect();
        sources.sort();
        let mut expected = vec![
            directory.join("one.txt").to_string_lossy().into_owned(),
            nested.join("three.TXT").to_string_lossy().into_owned(),
        ];
        expected.sort();
        assert_eq!(sources, expected);
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn recursive_scan_does_not_follow_symlinks() {
        let directory = temp_dir();
        let outside = temp_dir();
        std::fs::write(directory.join("inside.txt"), "內部").unwrap();
        std::fs::write(outside.join("outside.txt"), "外部").unwrap();
        std::os::unix::fs::symlink(&outside, directory.join("linked-directory")).unwrap();
        let service = FileService::new();
        let mut request = filename_request(&directory, ConflictPolicy::Skip);
        request.recursive = true;
        request.allowed_extensions = Some(vec!["txt".into()]);
        let plan = service
            .plan(shared_conversion(), request, noop())
            .await
            .unwrap();
        assert_eq!(
            plan.items
                .iter()
                .map(|item| item.source_path.clone())
                .collect::<Vec<_>>(),
            [directory.join("inside.txt").to_string_lossy().into_owned()]
        );
        assert!(!plan.items.iter().any(|item| item
            .source_path
            .starts_with(&outside.to_string_lossy().into_owned())));
        let _ = std::fs::remove_dir_all(&directory);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[tokio::test]
    async fn second_phase_failure_rolls_back() {
        let directory = temp_dir();
        let first = directory.join("里面一.txt");
        let second = directory.join("里面二.txt");
        std::fs::write(&first, "來源一").unwrap();
        std::fs::write(&second, "來源二").unwrap();
        let service = FileService::new();
        let mut request = filename_request(&directory, ConflictPolicy::Overwrite);
        request.recursive = false;
        let plan = service
            .plan(shared_conversion(), request, noop())
            .await
            .unwrap();
        let outputs: Vec<_> = plan
            .items
            .iter()
            .map(|item| item.output_path.clone())
            .collect();
        let removed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let removed_flag = Arc::clone(&removed);
        let dir = directory.clone();
        let progress: ProgressReporter = Arc::new(move |event| {
            if removed_flag.load(std::sync::atomic::Ordering::SeqCst)
                || !event.message.starts_with("正在寫入：")
            {
                return;
            }
            if let Some(stage) = std::fs::read_dir(&dir)
                .unwrap()
                .filter_map(|entry| entry.ok())
                .find(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".convertzz-stage-")
                })
            {
                let _ = std::fs::remove_file(stage.path());
                removed_flag.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        });
        let result = service.apply(&plan.plan_id, None, progress).await.unwrap();
        assert!(removed.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(result.failed.len(), 1);
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "來源一");
        assert_eq!(std::fs::read_to_string(&second).unwrap(), "來源二");
        assert_eq!(names(&directory), ["里面一.txt", "里面二.txt"]);
        assert!(outputs.iter().all(|path| {
            !names(&directory).contains(
                &Path::new(path)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            )
        }));
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn two_phase_rename_swaps_names() {
        let directory = temp_dir();
        let first = directory.join("甲.txt");
        let second = directory.join("乙.txt");
        std::fs::write(&first, "甲的內容").unwrap();
        std::fs::write(&second, "乙的內容").unwrap();
        let service = FileService::new().with_convert_hook(|text| match text {
            "甲.txt" => "乙.txt".into(),
            "乙.txt" => "甲.txt".into(),
            other => other.into(),
        });
        let result = service
            .plan(
                shared_conversion(),
                filename_request(&directory, ConflictPolicy::Overwrite),
                noop(),
            )
            .await
            .unwrap();
        let applied = service.apply(&result.plan_id, None, noop()).await.unwrap();
        assert!(applied.failed.is_empty());
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "乙的內容");
        assert_eq!(std::fs::read_to_string(&second).unwrap(), "甲的內容");
        assert_eq!(
            names(&directory)
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>(),
            ["甲.txt".into(), "乙.txt".into()].into_iter().collect()
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn stage_validation_failure_keeps_original() {
        let directory = temp_dir();
        let source = directory.join("里面.txt");
        std::fs::write(&source, "來源內容").unwrap();
        let service = FileService::new()
            .with_stage_validator(|_, _, _| Err(CoreError::new("FILE_VERIFY", "受控驗證失敗")));
        let plan = service
            .plan(
                shared_conversion(),
                filename_request(&source, ConflictPolicy::Overwrite),
                noop(),
            )
            .await
            .unwrap();
        let result = service.apply(&plan.plan_id, None, noop()).await.unwrap();
        assert_eq!(result.failed.len(), 1);
        assert_eq!(result.failed[0].path, "批次作業");
        assert_eq!(result.failed[0].message, "受控驗證失敗");
        assert_eq!(std::fs::read_to_string(&source).unwrap(), "來源內容");
        assert!(names(&directory)
            .into_iter()
            .all(|name| !name.starts_with(".convertzz-")));
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn readonly_file_is_reported_during_plan() {
        let directory = temp_dir();
        let source = directory.join("里面.txt");
        std::fs::write(&source, "來源").unwrap();
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o444)).unwrap();
        let service = FileService::new();
        let plan = service
            .plan(
                shared_conversion(),
                filename_request(&source, ConflictPolicy::Overwrite),
                noop(),
            )
            .await
            .unwrap();
        assert_eq!(plan.items[0].status, PlanStatus::Error);
        assert_eq!(
            plan.items[0].warning.as_deref(),
            Some("來源檔案為唯讀，無法安全取代。")
        );
        assert_eq!(std::fs::read_to_string(&source).unwrap(), "來源");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644)).unwrap();
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn file_becoming_readonly_is_not_replaced() {
        let directory = temp_dir();
        let source = directory.join("里面.txt");
        std::fs::write(&source, "來源").unwrap();
        let service = FileService::new();
        let plan = service
            .plan(
                shared_conversion(),
                filename_request(&source, ConflictPolicy::Overwrite),
                noop(),
            )
            .await
            .unwrap();
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o444)).unwrap();
        let result = service.apply(&plan.plan_id, None, noop()).await.unwrap();
        assert_eq!(result.failed[0].path, "批次作業");
        assert_eq!(result.failed[0].message, "來源檔案為唯讀，無法安全取代。");
        assert_eq!(std::fs::read_to_string(&source).unwrap(), "來源");
        assert_eq!(names(&directory), ["里面.txt"]);
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644)).unwrap();
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn creates_file_bak_before_conversion() {
        let directory = temp_dir();
        let path = directory.join("note.txt");
        std::fs::write(&path, "里面开发").unwrap();
        let service = FileService::new();
        let plan = service
            .plan(
                shared_conversion(),
                FilePlanRequest {
                    paths: vec![path.to_string_lossy().into_owned()],
                    output_path: None,
                    output_directory: None,
                    mode: FileMode::Content,
                    recursive: false,
                    input_encoding: TextEncoding::Utf8,
                    output_encoding: TextEncoding::Utf8,
                    add_bom: false,
                    fix_charset_declaration: false,
                    fix_charset_extensions: None,
                    allowed_extensions: None,
                    preview_max_bytes: None,
                    conflict_policy: ConflictPolicy::Skip,
                    backup: Some(true),
                    conversion: conversion_s2t(),
                },
                noop(),
            )
            .await
            .unwrap();
        let result = service.apply(&plan.plan_id, None, noop()).await.unwrap();
        assert!(result.failed.is_empty());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "裡面開發");
        assert_eq!(
            std::fs::read_to_string(format!("{}.bak", path.display())).unwrap(),
            "里面开发"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn folder_selection_backs_up_whole_folder() {
        let parent = temp_dir();
        let folder = parent.join("docs");
        let nested = folder.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(folder.join("one.txt"), "里面").unwrap();
        std::fs::write(nested.join("two.txt"), "开发").unwrap();
        let service = FileService::new();
        let plan = service
            .plan(
                shared_conversion(),
                FilePlanRequest {
                    paths: vec![folder.to_string_lossy().into_owned()],
                    output_path: None,
                    output_directory: None,
                    mode: FileMode::Content,
                    recursive: true,
                    input_encoding: TextEncoding::Utf8,
                    output_encoding: TextEncoding::Utf8,
                    add_bom: false,
                    fix_charset_declaration: false,
                    fix_charset_extensions: None,
                    allowed_extensions: Some(vec![".txt".into()]),
                    preview_max_bytes: None,
                    conflict_policy: ConflictPolicy::Skip,
                    backup: Some(true),
                    conversion: conversion_s2t(),
                },
                noop(),
            )
            .await
            .unwrap();
        let result = service.apply(&plan.plan_id, None, noop()).await.unwrap();
        assert!(result.failed.is_empty());
        assert_eq!(
            std::fs::read_to_string(folder.join("one.txt")).unwrap(),
            "裡面"
        );
        assert_eq!(
            std::fs::read_to_string(nested.join("two.txt")).unwrap(),
            "開發"
        );
        assert_eq!(
            std::fs::read_to_string(
                PathBuf::from(format!("{}.bak", folder.display())).join("one.txt")
            )
            .unwrap(),
            "里面"
        );
        assert_eq!(
            std::fs::read_to_string(
                PathBuf::from(format!("{}.bak", folder.display())).join("nested/two.txt")
            )
            .unwrap(),
            "开发"
        );
        assert!(names(&folder)
            .into_iter()
            .all(|name| !name.ends_with(".bak") && name != "one.txt.bak"));
        let _ = std::fs::remove_dir_all(&parent);
    }

    #[tokio::test]
    async fn backup_false_skips_bak() {
        let directory = temp_dir();
        let path = directory.join("note.txt");
        std::fs::write(&path, "里面").unwrap();
        let service = FileService::new();
        let plan = service
            .plan(
                shared_conversion(),
                FilePlanRequest {
                    paths: vec![path.to_string_lossy().into_owned()],
                    output_path: None,
                    output_directory: None,
                    mode: FileMode::Content,
                    recursive: false,
                    input_encoding: TextEncoding::Utf8,
                    output_encoding: TextEncoding::Utf8,
                    add_bom: false,
                    fix_charset_declaration: false,
                    fix_charset_extensions: None,
                    allowed_extensions: None,
                    preview_max_bytes: None,
                    conflict_policy: ConflictPolicy::Skip,
                    backup: Some(false),
                    conversion: conversion_s2t(),
                },
                noop(),
            )
            .await
            .unwrap();
        service.apply(&plan.plan_id, None, noop()).await.unwrap();
        assert_eq!(names(&directory), ["note.txt"]);
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn readonly_directory_keeps_original() {
        let directory = temp_dir();
        let source = directory.join("里面.txt");
        std::fs::write(&source, "來源").unwrap();
        let service = FileService::new();
        let plan = service
            .plan(
                shared_conversion(),
                filename_request(&source, ConflictPolicy::Overwrite),
                noop(),
            )
            .await
            .unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o555)).unwrap();
        let result = service.apply(&plan.plan_id, None, noop()).await.unwrap();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(result.failed.len(), 1);
        assert_eq!(std::fs::read_to_string(&source).unwrap(), "來源");
        let _ = std::fs::remove_dir_all(&directory);
    }
}

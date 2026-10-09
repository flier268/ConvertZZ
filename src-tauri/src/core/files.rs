use super::backup::{
    create_user_backups, resolve_backup_roots, resolve_path, wildcard_matcher, BackupRoot,
};
use super::conversion::{base_convert, ConversionService};
use super::encoding::{can_roundtrip, decode_text_detailed, detect_encoding, encode_text};
use super::error::CoreError;
use super::parallelism::default_convert_jobs;
use super::types::{
    ApplyFailure, ApplyResult, CancelCheck, ConflictPolicy, ConversionOptions, Direction,
    FileConversionPlan, FileItemKind, FileMode, FilePlanItem, FilePlanRequest, FilePreviewRequest,
    PlanStatus, ProgressEvent, ProgressReporter, TextEncoding,
};
use chrono::Utc;
use futures::stream::{self, StreamExt};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
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
    request: FilePlanRequest,
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
    cancelled: Arc<Mutex<HashSet<String>>>,
    convert_hook: Option<ConvertHook>,
    stage_validator: Option<StageValidator>,
}

impl FileService {
    pub fn new() -> Self {
        Self {
            plans: Mutex::new(std::collections::HashMap::new()),
            cancelled: Arc::new(Mutex::new(HashSet::new())),
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
        progress: Option<ProgressReporter>,
        is_cancelled: Option<CancelCheck>,
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
        conversion
            .convert_with_progress(options.with_text(text), progress, is_cancelled)
            .await
    }

    pub fn cancel(&self, plan_id: &str) -> serde_json::Value {
        let removed = self
            .plans
            .lock()
            .ok()
            .is_some_and(|mut plans| plans.remove(plan_id).is_some());
        let marked = self
            .cancelled
            .lock()
            .map(|mut set| set.insert(plan_id.to_string()))
            .unwrap_or(false);
        serde_json::json!({ "cancelled": removed || marked })
    }

    fn combined_cancel_check(&self, plan_id: &str, request_cancelled: CancelCheck) -> CancelCheck {
        let cancelled = Arc::clone(&self.cancelled);
        let plan_id = plan_id.to_string();
        Arc::new(move || {
            if request_cancelled() {
                return true;
            }
            cancelled
                .lock()
                .ok()
                .is_some_and(|set| set.contains(&plan_id))
        })
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

        for (index, source_path) in paths.iter().enumerate() {
            match self.enumerate_file(conversion, &request, source_path).await {
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
                        preview_loaded: false,
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
                message: format!("正在掃描：{}", file_name(source_path)),
            });
        }

        if request.output_directory.is_none() && request.output_path.is_none() {
            let mut directories = directories;
            directories.sort_by_key(|path| std::cmp::Reverse(path_depth(path)));
            for source_path in directories {
                let converted_name = self
                    .convert_text(
                        conversion,
                        &request.conversion,
                        file_name(&source_path),
                        None,
                        None,
                    )
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
                        preview_loaded: true,
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
                    request,
                },
            );
        }
        Ok(public)
    }

    pub async fn preview(
        &self,
        conversion: &ConversionService,
        request: FilePreviewRequest,
        progress: ProgressReporter,
        request_cancelled: CancelCheck,
    ) -> Result<FilePlanItem, CoreError> {
        let (plan_request, source_path, existing) = {
            let plans = self
                .plans
                .lock()
                .map_err(|_| CoreError::new("PLAN_LOCK", "無法讀取檔案轉換計畫。"))?;
            let plan = plans.get(&request.plan_id).ok_or_else(|| {
                CoreError::new("PLAN_NOT_FOUND", "檔案轉換計畫已失效。請重新預覽。")
            })?;
            let file = plan
                .files
                .iter()
                .find(|file| {
                    resolve_path(&file.item.source_path) == resolve_path(&request.source_path)
                })
                .ok_or_else(|| CoreError::new("PLAN_PATH", "預覽路徑不在目前的檔案轉換計畫內。"))?;
            (
                plan.request.clone(),
                PathBuf::from(&file.item.source_path),
                file.item.clone(),
            )
        };

        if existing.kind == FileItemKind::Directory || plan_request.mode == FileMode::Filename {
            return Ok(existing);
        }

        let is_cancelled = self.combined_cancel_check(&request.plan_id, request_cancelled);
        if is_cancelled() {
            return Err(CoreError::new("PLAN_CANCELLED", "檔案作業已由使用者取消。"));
        }

        let preview_max_bytes = plan_request
            .preview_max_bytes
            .unwrap_or(6 * 1024)
            .clamp(1024, 1024 * 1024) as usize;
        let buffer = fs::read(&source_path)?;
        let inspected = inspect_file_bytes(&buffer, plan_request.input_encoding)?;
        let (source_preview, output_preview, detected_encoding, content_warning) = match inspected {
            InspectedBytes::Skip(reason) => (String::new(), String::new(), None, Some(reason)),
            InspectedBytes::Text { text, encoding } => {
                let source_preview = truncate(&text, preview_max_bytes);
                let file_label = file_name(&source_path);
                let convert_progress =
                    map_progress(progress, 0, 1, move |current, total, _message| {
                        format!("正在預覽：{file_label}（{current}/{total}）")
                    });
                let converted = self
                    .convert_text(
                        conversion,
                        &plan_request.conversion,
                        source_preview.clone(),
                        Some(convert_progress),
                        Some(is_cancelled),
                    )
                    .await?;
                let output_encoding =
                    resolve_output_encoding(plan_request.output_encoding, Some(encoding));
                let mut output_text = converted.text;
                if plan_request.fix_charset_declaration {
                    output_text = fix_charset_declaration(
                        &output_text,
                        output_encoding,
                        source_path
                            .extension()
                            .and_then(|value| value.to_str())
                            .unwrap_or(""),
                        plan_request.fix_charset_extensions.as_deref(),
                    );
                }
                if plan_request.conversion.direction == super::types::Direction::None
                    && output_encoding == TextEncoding::Big5
                {
                    output_text = repair_unrepresentable_big5(&output_text);
                }
                (source_preview, output_text, Some(encoding), None)
            }
        };

        let mut plans = self
            .plans
            .lock()
            .map_err(|_| CoreError::new("PLAN_LOCK", "無法更新檔案轉換計畫。"))?;
        let plan = plans
            .get_mut(&request.plan_id)
            .ok_or_else(|| CoreError::new("PLAN_NOT_FOUND", "檔案轉換計畫已失效。請重新預覽。"))?;
        let file = plan
            .files
            .iter_mut()
            .find(|file| resolve_path(&file.item.source_path) == resolve_path(&request.source_path))
            .ok_or_else(|| CoreError::new("PLAN_PATH", "預覽路徑不在目前的檔案轉換計畫內。"))?;
        file.item.detected_encoding = detected_encoding;
        file.item.source_preview = source_preview;
        file.item.output_preview = output_preview;
        file.item.preview_loaded = true;
        if content_warning.is_some() && plan_request.mode == FileMode::Content {
            file.item.output_path = file.item.source_path.clone();
        }
        file.item.warning = merge_warning(
            content_warning.map(|reason| content_skip_message(reason).to_string()),
            file.item.warning.clone(),
        );
        Ok(file.item.clone())
    }

    /// 僅列舉路徑、檔名轉換與衝突；內容轉換延後到 preview／apply。
    async fn enumerate_file(
        &self,
        conversion: &ConversionService,
        request: &FilePlanRequest,
        source_path: &Path,
    ) -> Result<(PreparedFile, Vec<String>), CoreError> {
        assert_source_writable(source_path)?;
        let converted_name = if request.mode == FileMode::Content {
            file_name(source_path)
        } else {
            self.convert_text(
                conversion,
                &request.conversion,
                file_name(source_path),
                None,
                None,
            )
            .await?
            .text
        };
        let output_path = self
            .resolve_item_output_path(conversion, request, source_path, &converted_name)
            .await?;
        let conflict = output_path != source_path && output_path.exists();
        let (source_preview, output_preview, preview_loaded) = if request.mode == FileMode::Filename
        {
            (file_name(source_path), converted_name, true)
        } else {
            (String::new(), String::new(), false)
        };
        Ok((
            PreparedFile {
                item: FilePlanItem {
                    source_path: source_path.to_string_lossy().into_owned(),
                    output_path: output_path.to_string_lossy().into_owned(),
                    kind: FileItemKind::File,
                    selected: true,
                    detected_encoding: None,
                    source_preview,
                    output_preview,
                    preview_loaded,
                    status: if conflict && request.conflict_policy == ConflictPolicy::Skip {
                        PlanStatus::Conflict
                    } else {
                        PlanStatus::Ready
                    },
                    warning: conflict.then(|| "輸出路徑已存在。".into()),
                },
                content: None,
                conflict_policy: request.conflict_policy,
            },
            Vec::new(),
        ))
    }

    async fn prepare_file(
        &self,
        conversion: &ConversionService,
        request: &FilePlanRequest,
        source_path: &Path,
        selected: bool,
        existing_item: Option<&FilePlanItem>,
        progress: Option<ProgressReporter>,
        is_cancelled: Option<CancelCheck>,
    ) -> Result<(PreparedFile, Vec<String>), CoreError> {
        assert_source_writable(source_path)?;
        if is_cancelled.as_ref().is_some_and(|check| check()) {
            return Err(CoreError::new("PLAN_CANCELLED", "檔案作業已由使用者取消。"));
        }
        let source_buffer = if request.mode == FileMode::Filename {
            None
        } else {
            Some(fs::read(source_path)?)
        };
        let inspected = source_buffer
            .as_deref()
            .map(|buffer| inspect_file_bytes(buffer, request.input_encoding))
            .transpose()?;
        let content_skip = match &inspected {
            Some(InspectedBytes::Skip(reason)) => Some(*reason),
            _ => None,
        };
        let decoded = match inspected {
            Some(InspectedBytes::Text { text, encoding }) => Some((text, encoding)),
            _ => None,
        };
        let converted_content = if let Some((text, _)) = &decoded {
            Some(
                self.convert_text(
                    conversion,
                    &request.conversion,
                    text.clone(),
                    progress.clone(),
                    is_cancelled.clone(),
                )
                .await?,
            )
        } else {
            None
        };
        let converted_name = if request.mode == FileMode::Content {
            file_name(source_path)
        } else {
            self.convert_text(
                conversion,
                &request.conversion,
                file_name(source_path),
                None,
                is_cancelled.clone(),
            )
            .await?
            .text
        };
        let mut output_path = self
            .resolve_item_output_path(conversion, request, source_path, &converted_name)
            .await?;
        if content_skip.is_some() && request.mode == FileMode::Content {
            output_path = source_path.to_path_buf();
        }
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
        let mut warnings = converted_content
            .as_ref()
            .map(|item| item.warnings.clone())
            .unwrap_or_default();
        if let Some(reason) = content_skip {
            warnings.push(content_skip_message(reason).to_string());
        }
        let content = if content_skip.is_some() {
            None
        } else if converted_content.is_some() {
            Some(encode_text(&output_text, output_encoding, request.add_bom)?)
        } else {
            None
        };
        let preview_max_bytes = request
            .preview_max_bytes
            .unwrap_or(6 * 1024)
            .clamp(1024, 1024 * 1024) as usize;
        let (source_preview, output_preview, preview_loaded) = if content_skip.is_some() {
            (String::new(), String::new(), true)
        } else if let Some(item) = existing_item.filter(|item| item.preview_loaded) {
            (
                item.source_preview.clone(),
                item.output_preview.clone(),
                true,
            )
        } else if converted_content.is_some() {
            (
                decoded
                    .as_ref()
                    .map(|(text, _)| truncate(text, preview_max_bytes))
                    .unwrap_or_default(),
                truncate(&output_text, preview_max_bytes),
                true,
            )
        } else {
            (file_name(source_path), converted_name, true)
        };
        let status = if let Some(item) = existing_item {
            if item.status == PlanStatus::Conflict
                || (conflict && request.conflict_policy == ConflictPolicy::Skip)
            {
                PlanStatus::Conflict
            } else {
                PlanStatus::Ready
            }
        } else if conflict && request.conflict_policy == ConflictPolicy::Skip {
            PlanStatus::Conflict
        } else {
            PlanStatus::Ready
        };
        let skipped_content = content_skip.is_some();
        let warning = merge_warning(
            content_skip.map(|reason| content_skip_message(reason).to_string()),
            existing_item.and_then(|item| item.warning.clone()),
        );
        let warning = merge_warning(warning, conflict.then(|| "輸出路徑已存在。".into()));
        Ok((
            PreparedFile {
                item: FilePlanItem {
                    source_path: source_path.to_string_lossy().into_owned(),
                    output_path: output_path.to_string_lossy().into_owned(),
                    kind: FileItemKind::File,
                    selected,
                    detected_encoding: if skipped_content {
                        None
                    } else {
                        decoded
                            .as_ref()
                            .map(|item| item.1)
                            .or_else(|| existing_item.and_then(|item| item.detected_encoding))
                    },
                    source_preview,
                    output_preview,
                    preview_loaded,
                    status,
                    warning,
                },
                content,
                conflict_policy: request.conflict_policy,
            },
            warnings,
        ))
    }

    async fn resolve_item_output_path(
        &self,
        conversion: &ConversionService,
        request: &FilePlanRequest,
        source_path: &Path,
        converted_name: &str,
    ) -> Result<PathBuf, CoreError> {
        let default_output = source_path
            .parent()
            .unwrap_or(Path::new("."))
            .join(converted_name);
        if let Some(directory) = &request.output_directory {
            resolve_output_directory_path(
                self,
                conversion,
                source_path,
                &request.paths,
                directory,
                converted_name,
                request.mode,
                &request.conversion,
            )
            .await
        } else if let Some(pattern) = &request.output_path {
            Ok(resolve_requested_output_path(
                source_path,
                request.paths.first().map(String::as_str).unwrap_or(""),
                pattern,
                converted_name,
                request.mode,
            ))
        } else {
            Ok(default_output)
        }
    }

    pub async fn apply(
        &self,
        conversion: &ConversionService,
        plan_id: &str,
        selected_paths: Option<&[String]>,
        progress: ProgressReporter,
        request_cancelled: CancelCheck,
    ) -> Result<ApplyResult, CoreError> {
        let plan = self
            .plans
            .lock()
            .ok()
            .and_then(|mut plans| plans.remove(plan_id))
            .ok_or_else(|| CoreError::new("PLAN_NOT_FOUND", "檔案轉換計畫已失效。請重新預覽。"))?;
        let is_cancelled = self.combined_cancel_check(plan_id, request_cancelled);
        if is_cancelled() {
            return Err(CoreError::new("PLAN_CANCELLED", "檔案作業已由使用者取消。"));
        }
        let mut result = ApplyResult {
            succeeded: Vec::new(),
            skipped: Vec::new(),
            failed: Vec::new(),
            warnings: Vec::new(),
        };
        let selection = selected_paths.map(|paths| {
            paths
                .iter()
                .map(|path| resolve_path(path))
                .collect::<HashSet<_>>()
        });
        let request = Arc::new(plan.request.clone());
        let mut pending_files = Vec::new();
        let mut pending_directories = Vec::new();
        for file in plan.files {
            let selected = selection
                .as_ref()
                .is_none_or(|set| set.contains(&resolve_path(&file.item.source_path)));
            if file.item.kind == FileItemKind::Directory {
                if file.item.status == PlanStatus::Error {
                    continue;
                }
                if file.item.status != PlanStatus::Ready || !selected {
                    result.skipped.push(file.item.source_path);
                    continue;
                }
                pending_directories.push(file);
                continue;
            }
            if file.item.status == PlanStatus::Error {
                continue;
            }
            if file.item.status != PlanStatus::Ready || !selected {
                result.skipped.push(file.item.source_path);
                continue;
            }
            pending_files.push(file);
        }

        if plan.backup && (!pending_files.is_empty() || !pending_directories.is_empty()) {
            progress(ProgressEvent {
                current: 0,
                total: (pending_files.len() + pending_directories.len()).max(1) as u64,
                message: "正在建立備份…".into(),
            });
            if let Err(error) = create_user_backups(
                &plan.backup_roots,
                &pending_files
                    .iter()
                    .chain(pending_directories.iter())
                    .map(|file| PathBuf::from(&file.item.source_path))
                    .collect::<Vec<_>>(),
                ConflictPolicy::Overwrite,
            ) {
                result.failed.push(ApplyFailure {
                    path: "備份".into(),
                    message: error.message,
                });
                self.clear_cancelled(plan_id);
                return Ok(finalize_apply(result));
            }
        }

        let file_total = pending_files.len();
        let total = (file_total + pending_directories.len()).max(1) as u64;
        let mut committed_outputs = Vec::new();
        let mut stopped = false;

        if !pending_files.is_empty() {
            // 輸出路徑落在其他來源上（例如兩檔交換檔名）必須兩階段整批提交，
            // 否則單檔寫入會破壞尚未處理的來源。其餘情況逐檔轉換並立即寫入。
            if outputs_overlap_sources(&pending_files) {
                match self
                    .materialize_and_commit_overlapping_files(
                        conversion,
                        request.as_ref(),
                        pending_files,
                        Arc::clone(&progress),
                        Arc::clone(&is_cancelled),
                        total,
                    )
                    .await
                {
                    Ok(batch) => {
                        committed_outputs.extend(batch.committed_outputs);
                        result.skipped.extend(batch.skipped);
                        result.failed.extend(batch.failed);
                        result.warnings.extend(batch.warnings);
                        stopped = batch.stopped;
                    }
                    Err(error)
                        if error.code == "PLAN_CANCELLED" || error.code == "CONVERT_CANCELLED" =>
                    {
                        self.clear_cancelled(plan_id);
                        return Err(error);
                    }
                    Err(error) => {
                        result.failed.push(ApplyFailure {
                            path: "重新命名".into(),
                            message: error.message,
                        });
                    }
                }
            } else {
                stopped = self
                    .materialize_and_commit_files_incrementally(
                        conversion,
                        request.as_ref(),
                        pending_files,
                        Arc::clone(&progress),
                        Arc::clone(&is_cancelled),
                        total,
                        &mut committed_outputs,
                        &mut result,
                    )
                    .await;
            }
        }

        if stopped {
            for file in pending_directories {
                result.skipped.push(file.item.source_path);
            }
            self.clear_cancelled(plan_id);
            if committed_outputs.is_empty() && result.failed.is_empty() {
                return Err(CoreError::new("PLAN_CANCELLED", "檔案作業已由使用者取消。"));
            }
            result.succeeded.extend(
                committed_outputs
                    .into_iter()
                    .map(|path| path.to_string_lossy().into_owned()),
            );
            return Ok(finalize_apply(result));
        }

        pending_directories
            .sort_by_key(|item| std::cmp::Reverse(path_depth(Path::new(&item.item.source_path))));
        let mut directory_transaction = Vec::new();
        let mut pending_directories = pending_directories.into_iter().enumerate();
        while let Some((offset, item)) = pending_directories.next() {
            if is_cancelled() {
                result.skipped.push(item.item.source_path);
                for (_, remaining) in pending_directories {
                    result.skipped.push(remaining.item.source_path);
                }
                stopped = true;
                break;
            }
            if item.item.output_path == item.item.source_path {
                result.skipped.push(item.item.source_path);
                continue;
            }
            let source_path = item.item.source_path.clone();
            match commit_directory(item) {
                Ok(Some(entry)) => {
                    progress(ProgressEvent {
                        current: (file_total + offset + 1) as u64,
                        total,
                        message: format!(
                            "正在重新命名資料夾：{}",
                            file_name(Path::new(&entry.item.output_path))
                        ),
                    });
                    directory_transaction.push(entry);
                }
                Ok(None) => result.skipped.push(source_path),
                Err(error)
                    if error.code == "PLAN_CANCELLED" || error.code == "CONVERT_CANCELLED" =>
                {
                    result.skipped.push(source_path);
                    for (_, remaining) in pending_directories {
                        result.skipped.push(remaining.item.source_path);
                    }
                    stopped = true;
                    break;
                }
                Err(error) => result.failed.push(ApplyFailure {
                    path: source_path,
                    message: error.message,
                }),
            }
        }

        for output in committed_outputs {
            result.succeeded.push(
                resolve_committed_directory_path(&output, &directory_transaction)
                    .to_string_lossy()
                    .into_owned(),
            );
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

        self.clear_cancelled(plan_id);
        if stopped && result.succeeded.is_empty() && result.failed.is_empty() {
            return Err(CoreError::new("PLAN_CANCELLED", "檔案作業已由使用者取消。"));
        }
        Ok(finalize_apply(result))
    }

    async fn materialize_and_commit_files_incrementally(
        &self,
        conversion: &ConversionService,
        request: &FilePlanRequest,
        pending_files: Vec<PreparedFile>,
        progress: ProgressReporter,
        is_cancelled: CancelCheck,
        total: u64,
        committed_outputs: &mut Vec<PathBuf>,
        result: &mut ApplyResult,
    ) -> bool {
        let file_total = pending_files.len();
        let jobs = default_convert_jobs().min(pending_files.len().max(1));
        if jobs <= 1 || pending_files.len() <= 1 {
            let mut pending_files = pending_files.into_iter().enumerate();
            while let Some((offset, file)) = pending_files.next() {
                if is_cancelled() {
                    result.skipped.push(file.item.source_path);
                    for (_, remaining) in pending_files {
                        result.skipped.push(remaining.item.source_path);
                    }
                    return true;
                }
                let source_path = file.item.source_path.clone();
                let file_label = file_name(Path::new(&source_path));
                let progress_label = file_label.clone();
                let file_progress = map_progress(
                    Arc::clone(&progress),
                    offset as u64,
                    total,
                    move |current, local_total, _message| {
                        format!(
                            "正在轉換並寫入：{progress_label}（檔案 {}/{total}，內容 {current}/{local_total}）",
                            offset + 1
                        )
                    },
                );
                match self
                    .materialize_and_commit_file(
                        conversion,
                        request,
                        file,
                        Some(file_progress),
                        Arc::clone(&is_cancelled),
                    )
                    .await
                {
                    Ok(FileCommitOutcome::Succeeded {
                        output_path,
                        warnings,
                    }) => {
                        committed_outputs.push(output_path);
                        result.warnings.extend(warnings);
                        progress(ProgressEvent {
                            current: (offset + 1) as u64,
                            total,
                            message: format!("已寫入：{file_label}"),
                        });
                    }
                    Ok(FileCommitOutcome::Skipped { path, warnings }) => {
                        result.skipped.push(path);
                        result.warnings.extend(warnings);
                    }
                    Err(error)
                        if error.code == "PLAN_CANCELLED" || error.code == "CONVERT_CANCELLED" =>
                    {
                        result.skipped.push(source_path);
                        for (_, remaining) in pending_files {
                            result.skipped.push(remaining.item.source_path);
                        }
                        return true;
                    }
                    Err(error) => result.failed.push(ApplyFailure {
                        path: source_path,
                        message: error.message,
                    }),
                }
            }
            return false;
        }

        // 必須在 async 路徑上回報進度：rayon worker 上 emit 時前端常收不到更新。
        progress(ProgressEvent {
            current: 0,
            total,
            message: format!("正在以最多 {jobs} 個並行任務轉換並寫入 {file_total} 個檔案…"),
        });
        let completed = Arc::new(AtomicU64::new(0));
        let outcomes = stream::iter(pending_files.into_iter())
            .map(|file| {
                let progress = Arc::clone(&progress);
                let is_cancelled = Arc::clone(&is_cancelled);
                let completed = Arc::clone(&completed);
                async move {
                    let source_path = file.item.source_path.clone();
                    if is_cancelled() {
                        return (
                            source_path,
                            Err(CoreError::new("PLAN_CANCELLED", "檔案作業已由使用者取消。")),
                        );
                    }
                    let file_label = file_name(Path::new(&source_path));
                    let outcome = self
                        .materialize_and_commit_file(
                            conversion,
                            request,
                            file,
                            None,
                            Arc::clone(&is_cancelled),
                        )
                        .await;
                    let current = completed.fetch_add(1, Ordering::Relaxed) + 1;
                    progress(ProgressEvent {
                        current: current.min(total),
                        total,
                        message: format!("正在轉換並寫入：{file_label}（{current}/{file_total}）"),
                    });
                    (source_path, outcome)
                }
            })
            .buffer_unordered(jobs)
            .collect::<Vec<_>>()
            .await;
        let mut stopped = false;
        for (source_path, outcome) in outcomes {
            match outcome {
                Ok(FileCommitOutcome::Succeeded {
                    output_path,
                    warnings,
                }) => {
                    committed_outputs.push(output_path);
                    result.warnings.extend(warnings);
                }
                Ok(FileCommitOutcome::Skipped { path, warnings }) => {
                    result.skipped.push(path);
                    result.warnings.extend(warnings);
                }
                Err(error)
                    if error.code == "PLAN_CANCELLED" || error.code == "CONVERT_CANCELLED" =>
                {
                    result.skipped.push(source_path);
                    stopped = true;
                }
                Err(error) => result.failed.push(ApplyFailure {
                    path: source_path,
                    message: error.message,
                }),
            }
        }
        stopped
    }

    async fn materialize_and_commit_overlapping_files(
        &self,
        conversion: &ConversionService,
        request: &FilePlanRequest,
        pending_files: Vec<PreparedFile>,
        progress: ProgressReporter,
        is_cancelled: CancelCheck,
        total: u64,
    ) -> Result<OverlapBatchResult, CoreError> {
        let mut batch = OverlapBatchResult::default();
        let mut prepared_files = Vec::with_capacity(pending_files.len());
        for (offset, file) in pending_files.into_iter().enumerate() {
            if is_cancelled() {
                batch.skipped.push(file.item.source_path);
                batch.stopped = true;
                break;
            }
            let source_path = file.item.source_path.clone();
            let file_label = file_name(Path::new(&source_path));
            let progress_label = file_label.clone();
            let file_progress = map_progress(
                Arc::clone(&progress),
                offset as u64,
                total,
                move |current, local_total, _message| {
                    format!(
                        "正在轉換：{progress_label}（檔案 {}/{total}，內容 {current}/{local_total}）",
                        offset + 1
                    )
                },
            );
            match self
                .materialize_file(
                    conversion,
                    request,
                    file,
                    Some(file_progress),
                    Arc::clone(&is_cancelled),
                )
                .await
            {
                Ok(Some(prepared)) => prepared_files.push(prepared),
                Ok(None) => batch.skipped.push(source_path),
                Err(error)
                    if error.code == "PLAN_CANCELLED" || error.code == "CONVERT_CANCELLED" =>
                {
                    batch.skipped.push(source_path);
                    batch.stopped = true;
                    break;
                }
                Err(error) => batch.failed.push(ApplyFailure {
                    path: source_path,
                    message: error.message,
                }),
            }
        }
        if batch.stopped {
            return Ok(batch);
        }

        let mut transaction = Vec::new();
        let mut skipped_during_commit = Vec::new();
        let mut skipped_warnings = Vec::new();
        let stage_result = (|| -> Result<(), CoreError> {
            for file in prepared_files {
                if is_cancelled() {
                    return Err(CoreError::new("PLAN_CANCELLED", "檔案作業已由使用者取消。"));
                }
                if leaves_source_unchanged(request, &file) {
                    skipped_warnings.extend(content_skip_warnings(&file));
                    skipped_during_commit.push(file.item.source_path);
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
                progress(ProgressEvent {
                    current: transaction.len() as u64,
                    total,
                    message: format!("正在準備：{}", file_name(&source)),
                });
            }

            for entry in &mut transaction {
                if is_cancelled() {
                    return Err(CoreError::new("PLAN_CANCELLED", "檔案作業已由使用者取消。"));
                }
                let original =
                    transaction_path(Path::new(&entry.file.item.source_path), "original");
                fs::rename(&entry.file.item.source_path, &original)?;
                entry.original_backup = Some(original);
            }

            for entry in &mut transaction {
                if is_cancelled() {
                    return Err(CoreError::new("PLAN_CANCELLED", "檔案作業已由使用者取消。"));
                }
                let output = PathBuf::from(&entry.file.item.output_path);
                let source = PathBuf::from(&entry.file.item.source_path);
                if output != source && output.exists() {
                    if entry.file.conflict_policy == ConflictPolicy::Skip {
                        let _ = fs::remove_file(&entry.stage_path);
                        if let Some(backup) = entry.original_backup.take() {
                            let _ = fs::rename(backup, &source);
                        }
                        skipped_warnings.extend(content_skip_warnings(&entry.file));
                        skipped_during_commit.push(entry.file.item.source_path.clone());
                        continue;
                    }
                    let conflict = transaction_path(&output, "conflict");
                    fs::rename(&output, &conflict)?;
                    entry.conflict_backup = Some(conflict);
                }
                fs::rename(&entry.stage_path, &output)?;
                entry.committed = true;
                progress(ProgressEvent {
                    current: total,
                    total,
                    message: format!("正在寫入：{}", file_name(&output)),
                });
            }
            Ok(())
        })();

        batch.skipped.extend(skipped_during_commit);
        batch.warnings.extend(skipped_warnings);

        if let Err(error) = stage_result {
            rollback_transaction(&transaction);
            if error.code == "PLAN_CANCELLED" || error.code == "CONVERT_CANCELLED" {
                return Err(error);
            }
            batch.failed.push(ApplyFailure {
                path: "重新命名".into(),
                message: error.message,
            });
            return Ok(batch);
        }

        for entry in transaction {
            if !entry.committed {
                continue;
            }
            batch.warnings.extend(content_skip_warnings(&entry.file));
            batch
                .committed_outputs
                .push(PathBuf::from(&entry.file.item.output_path));
            for backup in [entry.original_backup, entry.conflict_backup]
                .into_iter()
                .flatten()
            {
                if let Err(error) =
                    fs::remove_file(&backup).or_else(|_| fs::remove_dir_all(&backup))
                {
                    batch.failed.push(ApplyFailure {
                        path: backup.to_string_lossy().into_owned(),
                        message: format!("已完成轉換，但無法清除復原暫存檔。{error}"),
                    });
                }
            }
        }
        Ok(batch)
    }

    async fn materialize_file(
        &self,
        conversion: &ConversionService,
        request: &FilePlanRequest,
        mut file: PreparedFile,
        progress: Option<ProgressReporter>,
        is_cancelled: CancelCheck,
    ) -> Result<Option<PreparedFile>, CoreError> {
        if is_cancelled() {
            return Err(CoreError::new("PLAN_CANCELLED", "檔案作業已由使用者取消。"));
        }
        if request.mode != FileMode::Filename && file.content.is_none() {
            let item = file.item.clone();
            let (prepared, _) = self
                .prepare_file(
                    conversion,
                    request,
                    Path::new(&item.source_path),
                    item.selected,
                    Some(&item),
                    progress,
                    Some(Arc::clone(&is_cancelled)),
                )
                .await?;
            file = prepared;
        }
        if file.item.status != PlanStatus::Ready {
            return Ok(None);
        }
        Ok(Some(file))
    }

    async fn materialize_and_commit_file(
        &self,
        conversion: &ConversionService,
        request: &FilePlanRequest,
        file: PreparedFile,
        progress: Option<ProgressReporter>,
        is_cancelled: CancelCheck,
    ) -> Result<FileCommitOutcome, CoreError> {
        let source_path = file.item.source_path.clone();
        let Some(file) = self
            .materialize_file(
                conversion,
                request,
                file,
                progress,
                Arc::clone(&is_cancelled),
            )
            .await?
        else {
            return Ok(FileCommitOutcome::Skipped {
                path: source_path,
                warnings: Vec::new(),
            });
        };
        if is_cancelled() {
            return Err(CoreError::new("PLAN_CANCELLED", "檔案作業已由使用者取消。"));
        }
        if leaves_source_unchanged(request, &file) {
            let warnings = content_skip_warnings(&file);
            return Ok(FileCommitOutcome::Skipped {
                path: file.item.source_path,
                warnings,
            });
        }
        commit_prepared_file(file, self.stage_validator.as_ref())
    }

    fn clear_cancelled(&self, plan_id: &str) {
        if let Ok(mut cancelled) = self.cancelled.lock() {
            cancelled.remove(plan_id);
        }
    }
}

#[derive(Default)]
struct OverlapBatchResult {
    committed_outputs: Vec<PathBuf>,
    skipped: Vec<String>,
    failed: Vec<ApplyFailure>,
    warnings: Vec<String>,
    stopped: bool,
}

fn outputs_overlap_sources(files: &[PreparedFile]) -> bool {
    let sources = files
        .iter()
        .map(|file| resolve_path(&file.item.source_path))
        .collect::<HashSet<_>>();
    files.iter().any(|file| {
        let source = resolve_path(&file.item.source_path);
        let output = resolve_path(&file.item.output_path);
        output != source && sources.contains(&output)
    })
}

enum FileCommitOutcome {
    Succeeded {
        output_path: PathBuf,
        warnings: Vec<String>,
    },
    Skipped {
        path: String,
        warnings: Vec<String>,
    },
}

fn commit_prepared_file(
    file: PreparedFile,
    stage_validator: Option<&StageValidator>,
) -> Result<FileCommitOutcome, CoreError> {
    if file.item.output_path == file.item.source_path && file.content.is_none() {
        let warnings = content_skip_warnings(&file);
        return Ok(FileCommitOutcome::Skipped {
            path: file.item.source_path,
            warnings,
        });
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
    if let Some(validator) = stage_validator {
        if let Err(error) = validator(&stage_path, file.content.as_deref(), &source) {
            let _ = fs::remove_file(&stage_path);
            return Err(error);
        }
    }

    let mut entry = TransactionEntry {
        file,
        stage_path,
        original_backup: None,
        conflict_backup: None,
        committed: false,
    };
    let commit_result = (|| -> Result<FileCommitOutcome, CoreError> {
        let original = transaction_path(Path::new(&entry.file.item.source_path), "original");
        fs::rename(&entry.file.item.source_path, &original)?;
        entry.original_backup = Some(original);

        let output = PathBuf::from(&entry.file.item.output_path);
        let source = PathBuf::from(&entry.file.item.source_path);
        if output != source && output.exists() {
            if entry.file.conflict_policy == ConflictPolicy::Skip {
                let _ = fs::remove_file(&entry.stage_path);
                if let Some(backup) = entry.original_backup.take() {
                    let _ = fs::rename(backup, &source);
                }
                return Ok(FileCommitOutcome::Skipped {
                    path: entry.file.item.source_path.clone(),
                    warnings: content_skip_warnings(&entry.file),
                });
            }
            let conflict = transaction_path(&output, "conflict");
            fs::rename(&output, &conflict)?;
            entry.conflict_backup = Some(conflict);
        }
        fs::rename(&entry.stage_path, &output)?;
        entry.committed = true;
        Ok(FileCommitOutcome::Succeeded {
            output_path: output,
            warnings: content_skip_warnings(&entry.file),
        })
    })();

    match commit_result {
        Ok(FileCommitOutcome::Succeeded {
            output_path,
            warnings,
        }) => {
            for backup in [entry.original_backup.take(), entry.conflict_backup.take()]
                .into_iter()
                .flatten()
            {
                let _ = fs::remove_file(&backup).or_else(|_| fs::remove_dir_all(&backup));
            }
            Ok(FileCommitOutcome::Succeeded {
                output_path,
                warnings,
            })
        }
        Ok(skipped @ FileCommitOutcome::Skipped { .. }) => Ok(skipped),
        Err(error) => {
            rollback_transaction(std::slice::from_ref(&entry));
            Err(error)
        }
    }
}

fn commit_directory(item: PreparedFile) -> Result<Option<DirectoryTransactionEntry>, CoreError> {
    let mut entry = DirectoryTransactionEntry {
        temporary_path: transaction_path(Path::new(&item.item.source_path), "directory"),
        item: item.item,
        conflict_backup: None,
        committed: false,
        conflict_policy: item.conflict_policy,
    };
    let commit_result = (|| -> Result<bool, CoreError> {
        fs::rename(&entry.item.source_path, &entry.temporary_path)?;
        if Path::new(&entry.item.output_path).exists() {
            if entry.conflict_policy == ConflictPolicy::Skip {
                let _ = fs::rename(&entry.temporary_path, &entry.item.source_path);
                return Ok(false);
            }
            let conflict = transaction_path(Path::new(&entry.item.output_path), "conflict");
            fs::rename(&entry.item.output_path, &conflict)?;
            entry.conflict_backup = Some(conflict);
        }
        fs::rename(&entry.temporary_path, &entry.item.output_path)?;
        entry.committed = true;
        Ok(true)
    })();

    match commit_result {
        Ok(true) => Ok(Some(entry)),
        Ok(false) => Ok(None),
        Err(error) => {
            rollback_directories(std::slice::from_ref(&entry));
            Err(error)
        }
    }
}

fn map_progress(
    progress: ProgressReporter,
    file_index: u64,
    file_total: u64,
    message: impl Fn(u64, u64, &str) -> String + Send + Sync + 'static,
) -> ProgressReporter {
    let file_total = file_total.max(1);
    Arc::new(move |event: ProgressEvent| {
        let local_total = event.total.max(1);
        let overall_total = file_total.saturating_mul(local_total);
        let overall_current = file_index
            .saturating_mul(local_total)
            .saturating_add(event.current.min(local_total));
        progress(ProgressEvent {
            current: overall_current.min(overall_total),
            total: overall_total,
            message: message(event.current, local_total, &event.message),
        });
    })
}

/// 資料夾掃描的副檔名閘門。`All` 只來自明確的所有檔案（請求省略清單）。
/// `Only` 的空集合表示沒有符合的副檔名，不收任何檔案。
enum ExtensionGate {
    All,
    Only(HashSet<String>),
}

fn extension_gate(allowed_extensions: Option<&[String]>) -> ExtensionGate {
    match allowed_extensions {
        None => ExtensionGate::All,
        Some(extensions) => ExtensionGate::Only(normalize_extension_list(extensions)),
    }
}

fn is_all_files_token(value: &str) -> bool {
    value == "*" || value.eq_ignore_ascii_case("*.*")
}

/// `*` 與 `*.*` 不是副檔名。清單裡只剩這些標記時變成空集合，而不是所有檔案。
fn normalize_extension_list(extensions: &[String]) -> HashSet<String> {
    extensions
        .iter()
        .filter_map(|extension| {
            let trimmed = extension.trim();
            if trimmed.is_empty() || is_all_files_token(trimmed) {
                return None;
            }
            let lowered = trimmed.to_ascii_lowercase();
            if let Some(rest) = lowered.strip_prefix('.') {
                if rest.is_empty() || is_all_files_token(rest) {
                    None
                } else {
                    Some(lowered)
                }
            } else {
                Some(format!(".{lowered}"))
            }
        })
        .collect()
}

/// 直接指定的檔案一律收，因為選擇對話框不會回報使用者選了哪個篩選，
/// 使用者明確點選的檔案不應被副檔名擋下（包含空清單時）。
/// 資料夾掃描找到的檔案：`All` 全收；`Only` 必須符合副檔名，空集合時一個都不收。
fn include_file(discovered: bool, gate: &ExtensionGate, path: &Path) -> bool {
    if !discovered {
        return true;
    }
    match gate {
        ExtensionGate::All => true,
        ExtensionGate::Only(allowed) => allowed.contains(&extension_of(path)),
    }
}

fn collect_files(
    inputs: &[String],
    recursive: bool,
    allowed_extensions: Option<&[String]>,
) -> Result<Vec<PathBuf>, CoreError> {
    let mut collected = HashSet::new();
    let gate = extension_gate(allowed_extensions);
    for path in inputs {
        visit_files(path, recursive, false, &gate, &mut collected)?;
    }
    let mut files = collected.into_iter().collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn visit_files(
    path: &str,
    recursive: bool,
    discovered: bool,
    gate: &ExtensionGate,
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
        if include_file(discovered, gate, &absolute) {
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
        if entry.file_type()?.is_file() && include_file(true, gate, &entry.path()) {
            collected.insert(entry.path());
        } else if recursive && entry.file_type()?.is_dir() {
            visit_files(&entry.path().to_string_lossy(), true, true, gate, collected)?;
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
                    .convert_text(conversion, conversion_request, part, None, None)
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
                base_convert(&value, Direction::S2t)
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
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_READONLY: u32 = 0x1;
        if source.file_attributes() & FILE_ATTRIBUTE_READONLY != 0 {
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

const BINARY_CONTENT_WARNING: &str = "此檔案為二進位內容，已略過內容轉換。";
const DECODE_ERROR_WARNING: &str = "解碼時發生錯誤，已略過內容轉換。";
/// 少於這個數量的 U+FFFD 不視為二進位。只用於 UTF-16；其他編碼看 `had_errors`。
const REPLACEMENT_MIN_COUNT: usize = 4;
/// 替換字元佔解碼後字元數的比例達到此值，且數量達下限，才視為二進位。
const REPLACEMENT_RATIO: f64 = 0.02;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContentSkip {
    Binary,
    DecodeError,
}

#[derive(Debug)]
enum InspectedBytes {
    Skip(ContentSkip),
    Text {
        text: String,
        encoding: TextEncoding,
    },
}

fn content_skip_message(reason: ContentSkip) -> &'static str {
    match reason {
        ContentSkip::Binary => BINARY_CONTENT_WARNING,
        ContentSkip::DecodeError => DECODE_ERROR_WARNING,
    }
}

fn is_content_skip_warning(warning: &str) -> bool {
    warning.contains(BINARY_CONTENT_WARNING) || warning.contains(DECODE_ERROR_WARNING)
}

fn content_skip_warnings(file: &PreparedFile) -> Vec<String> {
    file.item
        .warning
        .iter()
        .filter(|warning| is_content_skip_warning(warning))
        .map(|warning| format!("{}：{warning}", file.item.source_path))
        .collect()
}

fn leaves_source_unchanged(request: &FilePlanRequest, file: &PreparedFile) -> bool {
    file.content.is_none()
        && (request.mode == FileMode::Content || file.item.output_path == file.item.source_path)
}

fn finalize_apply(mut result: ApplyResult) -> ApplyResult {
    result.warnings = unique(result.warnings);
    result
}

fn merge_warning(incoming: Option<String>, existing: Option<String>) -> Option<String> {
    match (incoming, existing) {
        (Some(incoming), Some(existing)) if !existing.contains(&incoming) => {
            Some(format!("{incoming} {existing}"))
        }
        (Some(incoming), _) => Some(incoming),
        (None, existing) => existing,
    }
}

fn inspect_file_bytes(buffer: &[u8], requested: TextEncoding) -> Result<InspectedBytes, CoreError> {
    if has_binary_magic(buffer) {
        return Ok(InspectedBytes::Skip(ContentSkip::Binary));
    }
    let encoding = if requested == TextEncoding::Auto {
        detect_encoding(buffer)
    } else {
        requested
    };
    // UTF-16 文字的 ASCII 會含 NUL；那不是二進位檔。
    if !matches!(encoding, TextEncoding::Utf16le | TextEncoding::Utf16be) && buffer.contains(&0) {
        return Ok(InspectedBytes::Skip(ContentSkip::Binary));
    }
    let decoded = decode_text_detailed(buffer, encoding)?;
    if decoded.had_errors {
        return Ok(InspectedBytes::Skip(ContentSkip::DecodeError));
    }
    // encoding_rs 路徑靠 had_errors。比例只留給 UTF-16，避免合法 UTF-8 裡的「�」被當成二進位。
    if matches!(
        decoded.encoding,
        TextEncoding::Utf16le | TextEncoding::Utf16be
    ) && replacement_ratio_too_high(&decoded.text)
    {
        return Ok(InspectedBytes::Skip(ContentSkip::Binary));
    }
    Ok(InspectedBytes::Text {
        text: decoded.text,
        encoding: decoded.encoding,
    })
}

fn replacement_ratio_too_high(text: &str) -> bool {
    let mut chars = 0usize;
    let mut replacements = 0usize;
    for character in text.chars() {
        chars += 1;
        if character == '\u{FFFD}' {
            replacements += 1;
        }
    }
    chars > 0
        && replacements >= REPLACEMENT_MIN_COUNT
        && (replacements as f64) / (chars as f64) >= REPLACEMENT_RATIO
}

fn has_binary_magic(buffer: &[u8]) -> bool {
    has_id3v2(buffer)
        || has_flac(buffer)
        || has_ogg(buffer)
        || has_riff_media(buffer)
        || buffer.starts_with(b"%PDF-")
        || buffer.starts_with(b"PK\x03\x04")
        || buffer.starts_with(b"PK\x05\x06")
        || buffer.starts_with(b"PK\x07\x08")
        || buffer.starts_with(b"\x89PNG\r\n\x1a\n")
        || buffer.starts_with(b"\xFF\xD8\xFF")
        || buffer.starts_with(b"GIF87a")
        || buffer.starts_with(b"GIF89a")
        || buffer.starts_with(b"\x7FELF")
        || buffer.starts_with(b"Rar!\x1A\x07")
        || buffer.starts_with(b"7z\xBC\xAF\x27\x1C")
        || has_gzip(buffer)
        || has_bzip2(buffer)
        || buffer.starts_with(b"\xFD7zXZ\x00")
        || has_ape(buffer)
        || has_ape_tag(buffer)
        || has_caf(buffer)
        || has_midi(buffer)
        || has_wavpack(buffer)
        || has_asf(buffer)
        || has_flv(buffer)
        || is_mpeg_audio_sync(buffer)
        || is_iso_bmff(buffer)
        || is_aiff(buffer)
        || is_pe_executable(buffer)
}

fn has_id3v2(buffer: &[u8]) -> bool {
    if buffer.len() < 10 || &buffer[..3] != b"ID3" {
        return false;
    }
    let version = buffer[3];
    if !(2..=4).contains(&version) {
        return false;
    }
    let flags = buffer[5];
    let flags_ok = match version {
        2 => flags & 0x3F == 0,
        3 => flags & 0x1F == 0,
        _ => flags & 0x0F == 0,
    };
    flags_ok && buffer[6..10].iter().all(|byte| *byte < 0x80)
}

fn has_flac(buffer: &[u8]) -> bool {
    buffer.len() >= 8
        && buffer.starts_with(b"fLaC")
        && buffer[4] & 0x7F == 0
        && u32::from_be_bytes([0, buffer[5], buffer[6], buffer[7]]) == 34
}

fn has_ogg(buffer: &[u8]) -> bool {
    buffer.len() >= 5 && buffer.starts_with(b"OggS") && buffer[4] == 0
}

fn has_riff_media(buffer: &[u8]) -> bool {
    buffer.len() >= 12
        && buffer.starts_with(b"RIFF")
        && matches!(&buffer[8..12], b"WAVE" | b"AVI " | b"WEBP")
}

fn has_gzip(buffer: &[u8]) -> bool {
    buffer.len() >= 3 && buffer.starts_with(b"\x1F\x8B") && buffer[2] == 8
}

fn has_bzip2(buffer: &[u8]) -> bool {
    buffer.len() >= 4 && buffer.starts_with(b"BZh") && (b'1'..=b'9').contains(&buffer[3])
}

fn has_ape(buffer: &[u8]) -> bool {
    if buffer.len() < 6 || !buffer.starts_with(b"MAC ") {
        return false;
    }
    let version = u16::from_le_bytes([buffer[4], buffer[5]]);
    (3800..=3990).contains(&version)
}

fn has_ape_tag(buffer: &[u8]) -> bool {
    if buffer.len() < 12 || !buffer.starts_with(b"APETAGEX") {
        return false;
    }
    let version = u32::from_le_bytes([buffer[8], buffer[9], buffer[10], buffer[11]]);
    version == 1000 || version == 2000
}

fn has_caf(buffer: &[u8]) -> bool {
    buffer.len() >= 6 && buffer.starts_with(b"caff") && buffer[4..6] == [0x00, 0x01]
}

fn has_midi(buffer: &[u8]) -> bool {
    buffer.len() >= 8 && buffer.starts_with(b"MThd") && buffer[4..8] == [0x00, 0x00, 0x00, 0x06]
}

fn has_wavpack(buffer: &[u8]) -> bool {
    if buffer.len() < 10 || !buffer.starts_with(b"wvpk") {
        return false;
    }
    let version = u16::from_le_bytes([buffer[8], buffer[9]]);
    (0x402..=0x410).contains(&version)
}

fn has_asf(buffer: &[u8]) -> bool {
    const HEADER: [u8; 16] = [
        0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE,
        0x6C,
    ];
    buffer.len() >= HEADER.len() && buffer[..HEADER.len()] == HEADER
}

fn has_flv(buffer: &[u8]) -> bool {
    buffer.len() >= 9
        && buffer.starts_with(b"FLV\x01")
        && buffer[4] & 0xFA == 0
        && buffer[5..9] == [0x00, 0x00, 0x00, 0x09]
}

fn is_mpeg_audio_sync(buffer: &[u8]) -> bool {
    // UTF-16LE BOM 是 FF FE。第二個位元組落在 E0–FF，不能當成檔案開頭的 MPEG 幀同步。
    if buffer.starts_with(&[0xFF, 0xFE]) {
        return false;
    }
    // 比一幀還短的緩衝區不是完整音訊幀。無 BOM 的 UTF-16BE「￥」(FF E5) 因此不會被當成 MP3。
    // 檔案剛好裝下一幀，或尾端不足 4 byte 再構成下一幀頭時，接受單幀。
    let Some(frame_len) = mpeg_frame_length(buffer) else {
        return false;
    };
    if buffer.len() < frame_len {
        return false;
    }
    if buffer.len() < frame_len + 4 {
        return true;
    }
    mpeg_frame_length(&buffer[frame_len..]).is_some()
}

fn mpeg_frame_length(buffer: &[u8]) -> Option<usize> {
    if buffer.len() < 4 || buffer[0] != 0xFF || buffer[1] & 0xE0 != 0xE0 {
        return None;
    }
    let version_id = (buffer[1] >> 3) & 0b11;
    let layer_id = (buffer[1] >> 1) & 0b11;
    if version_id == 0b01 || layer_id == 0b00 {
        return None;
    }
    let bitrate_index = (buffer[2] >> 4) & 0x0F;
    let sample_index = (buffer[2] >> 2) & 0b11;
    // 1111 是非法位元率。0000 是 free，沒有固定幀長，無法核對下一幀。
    if bitrate_index == 0x00 || bitrate_index == 0x0F || sample_index == 0b11 {
        return None;
    }
    let bitrate_kbps = mpeg_bitrate_kbps(version_id, layer_id, bitrate_index)?;
    let sample_rate = mpeg_sample_rate(version_id, sample_index)?;
    let padding = ((buffer[2] >> 1) & 1) as usize;
    let bitrate = bitrate_kbps as usize * 1000;
    let sample_rate = sample_rate as usize;
    let length = if layer_id == 0b11 {
        (12 * bitrate / sample_rate + padding) * 4
    } else if layer_id == 0b01 && version_id != 0b11 {
        72 * bitrate / sample_rate + padding
    } else {
        144 * bitrate / sample_rate + padding
    };
    (length >= 4).then_some(length)
}

fn mpeg_bitrate_kbps(version_id: u8, layer_id: u8, index: u8) -> Option<u32> {
    const MPEG1_LAYER1: [u32; 14] = [
        32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448,
    ];
    const MPEG1_LAYER2: [u32; 14] = [
        32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384,
    ];
    const MPEG1_LAYER3: [u32; 14] = [
        32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
    ];
    const MPEG2_LAYER1: [u32; 14] = [
        32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256,
    ];
    const MPEG2_LAYER23: [u32; 14] = [8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];
    let table: &[u32] = match (version_id == 0b11, layer_id) {
        (true, 0b11) => &MPEG1_LAYER1,
        (true, 0b10) => &MPEG1_LAYER2,
        (true, 0b01) => &MPEG1_LAYER3,
        (false, 0b11) => &MPEG2_LAYER1,
        (false, 0b10) | (false, 0b01) => &MPEG2_LAYER23,
        _ => return None,
    };
    table.get(usize::from(index.checked_sub(1)?)).copied()
}

fn mpeg_sample_rate(version_id: u8, index: u8) -> Option<u32> {
    let table: &[u32] = match version_id {
        0b11 => &[44100, 48000, 32000],
        0b10 => &[22050, 24000, 16000],
        0b00 => &[11025, 12000, 8000],
        _ => return None,
    };
    table.get(usize::from(index)).copied()
}

fn is_iso_bmff(buffer: &[u8]) -> bool {
    if buffer.len() < 12 || &buffer[4..8] != b"ftyp" {
        return false;
    }
    let size = u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
    let len = u32::try_from(buffer.len()).unwrap_or(u32::MAX);
    // 0 與 1 是延伸到檔尾／64-bit 大小，不是一般 ftyp。大小至少要蓋過 brand。
    if !(12..=len).contains(&size) {
        return false;
    }
    buffer[8..12]
        .iter()
        .all(|byte| (0x20..=0x7E).contains(byte))
}

fn is_aiff(buffer: &[u8]) -> bool {
    buffer.len() >= 12
        && buffer.starts_with(b"FORM")
        && (&buffer[8..12] == b"AIFF" || &buffer[8..12] == b"AIFC")
}

fn is_pe_executable(buffer: &[u8]) -> bool {
    if buffer.len() < 0x40 || !buffer.starts_with(b"MZ") {
        return false;
    }
    let offset =
        u32::from_le_bytes([buffer[0x3C], buffer[0x3D], buffer[0x3E], buffer[0x3F]]) as usize;
    let Some(end) = offset.checked_add(4) else {
        return false;
    };
    buffer.len() >= end && &buffer[offset..end] == b"PE\0\0"
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
mod tests;

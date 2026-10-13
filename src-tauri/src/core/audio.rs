use super::backup::{create_user_backups, resolve_backup_roots, resolve_path, BackupRoot};
use super::conversion::ConversionService;
mod operations;
use super::encoding::{decode_text, encode_text, encode_text_detailed};
use super::error::CoreError;
use super::types::{
    ApplyFailure, ApplyResult, AudioContainer, AudioFieldWarning, AudioFieldWarningCode,
    AudioFormat, AudioScanRequest, AudioTagField, AudioTagFile, AudioTagPlan, AudioTagPlanRequest,
    AudioTagPlanSummary, CancelCheck, ConflictPolicy, ConversionOptions, ProgressReporter,
    TextEncoding,
};
use chrono::Utc;
use lofty::config::WriteOptions;
use lofty::file::{AudioFile, FileType, TaggedFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::items::Timestamp;
use lofty::tag::{Accessor, ItemKey, ItemValue, Tag, TagItem};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const STANDARD_FIELDS: &[&str] = &[
    "title", "artist", "album", "year", "track", "comment", "genre",
];

pub(crate) const AUDIO_STREAM_BATCH: usize = 32;

pub(crate) fn take_prioritized<T>(
    pending: &mut VecDeque<T>,
    priority: &mut VecDeque<String>,
    key: impl Fn(&T) -> String,
) -> Option<T> {
    while let Some(wanted) = priority.pop_front() {
        if let Some(pos) = pending
            .iter()
            .position(|item| paths_match(&key(item), &wanted))
        {
            return pending.remove(pos);
        }
    }
    pending.pop_front()
}

pub(crate) fn drain_stream_batch<T>(buffer: &mut Vec<T>, force: bool) -> Option<Vec<T>> {
    if buffer.is_empty() || (!force && buffer.len() < AUDIO_STREAM_BATCH) {
        return None;
    }
    Some(std::mem::take(buffer))
}

pub(super) fn paths_match(left: &str, right: &str) -> bool {
    left == right || resolve_path(left) == resolve_path(right)
}

fn ignore_cancel() -> CancelCheck {
    Arc::new(|| false)
}

struct PreparedAudio {
    path: PathBuf,
    format: AudioFormat,
    updates: HashMap<String, Vec<String>>,
    selected_containers: HashSet<AudioContainer>,
    /// 即使欄位文字沒變，這些容器仍要重寫編碼或 ID3 版本。
    rewrite_containers: HashSet<AudioContainer>,
    /// 寫入時整段拿掉的標籤。音訊位元組仍用 `mp3_audio_payload` 核對。
    remove_containers: HashSet<AudioContainer>,
    request: AudioTagPlanRequest,
    original_picture_count: usize,
}

struct StoredAudioPlan {
    files: Vec<PreparedAudio>,
    backup: bool,
    backup_roots: Vec<BackupRoot>,
    conflict_policy: ConflictPolicy,
}

#[derive(Clone, PartialEq, Eq)]
struct ScanCacheKey {
    paths: Vec<String>,
    recursive: bool,
    id3v1_source_encoding: Option<TextEncoding>,
    id3v2_source_encoding: Option<TextEncoding>,
    id3v2_repair_source_encoding: Option<bool>,
}

pub struct AudioService {
    plans: Mutex<HashMap<String, StoredAudioPlan>>,
    cancelled: Arc<Mutex<HashSet<String>>>,
    /// 最近一次掃描。`audio.plan` 在路徑與讀取選項相同時直接重用，不再讀碟。
    scan_cache: Mutex<Option<(ScanCacheKey, Vec<AudioTagFile>)>>,
    /// 使用者正在看的路徑，掃描與預覽會把它插到待處理佇列前面。
    priority: Mutex<VecDeque<String>>,
}

impl AudioService {
    pub fn new() -> Self {
        Self {
            plans: Mutex::new(HashMap::new()),
            cancelled: Arc::new(Mutex::new(HashSet::new())),
            scan_cache: Mutex::new(None),
            priority: Mutex::new(VecDeque::new()),
        }
    }

    pub fn prioritize(&self, path: &str) {
        let Ok(mut queue) = self.priority.lock() else {
            return;
        };
        let path = path.to_string();
        queue.retain(|item| !paths_match(item, &path));
        queue.push_front(path);
    }

    fn take_next<T>(&self, pending: &mut VecDeque<T>, key: impl Fn(&T) -> String) -> Option<T> {
        let mut priority = match self.priority.lock() {
            Ok(mut guard) => std::mem::take(&mut *guard),
            Err(_) => VecDeque::new(),
        };
        let next = take_prioritized(pending, &mut priority, key);
        if let Ok(mut guard) = self.priority.lock() {
            let mut arrived = std::mem::take(&mut *guard);
            arrived.append(&mut priority);
            *guard = arrived;
        }
        next
    }

    fn scan_key(request: &AudioScanRequest) -> ScanCacheKey {
        ScanCacheKey {
            paths: request.paths.clone(),
            recursive: request.recursive.unwrap_or(false),
            id3v1_source_encoding: request.id3v1_source_encoding,
            id3v2_source_encoding: request.id3v2_source_encoding,
            id3v2_repair_source_encoding: request.id3v2_repair_source_encoding,
        }
    }

    fn store_scan(&self, request: &AudioScanRequest, files: &[AudioTagFile]) {
        if let Ok(mut cache) = self.scan_cache.lock() {
            *cache = Some((Self::scan_key(request), files.to_vec()));
        }
    }

    fn load_scan(&self, request: &AudioScanRequest) -> Option<Vec<AudioTagFile>> {
        self.scan_cache.lock().ok().and_then(|cache| {
            cache
                .as_ref()
                .and_then(|(key, files)| (key == &Self::scan_key(request)).then(|| files.clone()))
        })
    }

    fn clear_scan_cache(&self) {
        if let Ok(mut cache) = self.scan_cache.lock() {
            *cache = None;
        }
    }

    /// 單檔重掃只換掉快取裡的同一路徑，且讀取選項必須相同。
    fn patch_scan_file(&self, request: &AudioTagPlanRequest, file: &AudioTagFile) {
        let Ok(mut cache) = self.scan_cache.lock() else {
            return;
        };
        let Some((key, files)) = cache.as_mut() else {
            return;
        };
        let options_match = key.recursive == request.recursive.unwrap_or(false)
            && key.id3v1_source_encoding == request.id3v1_source_encoding
            && key.id3v2_source_encoding == request.id3v2_source_encoding
            && key.id3v2_repair_source_encoding == request.id3v2_repair_source_encoding;
        if !options_match {
            return;
        }
        let resolved = resolve_path(&file.path);
        if let Some(slot) = files
            .iter_mut()
            .find(|item| resolve_path(&item.path) == resolved)
        {
            *slot = file.clone();
        }
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

    /// 測試與舊呼叫點用。正式 IPC 走 `scan_cancellable`，所以這裡在非測試建置沒有呼叫者。
    #[allow(dead_code)]
    pub async fn scan(
        &self,
        _conversion: &ConversionService,
        request: AudioScanRequest,
        progress: ProgressReporter,
    ) -> Result<Vec<AudioTagFile>, CoreError> {
        self.scan_cancellable(_conversion, request, progress, ignore_cancel())
            .await
    }

    pub async fn scan_cancellable(
        &self,
        _conversion: &ConversionService,
        request: AudioScanRequest,
        progress: ProgressReporter,
        is_cancelled: CancelCheck,
    ) -> Result<Vec<AudioTagFile>, CoreError> {
        let files = read_audio_files(self, &request, progress, &is_cancelled).await?;
        self.store_scan(&request, &files);
        Ok(files)
    }

    /// 測試與舊呼叫點用。正式 IPC 走 `plan_cancellable`。
    #[allow(dead_code)]
    pub async fn plan(
        &self,
        conversion: &ConversionService,
        request: AudioTagPlanRequest,
        progress: ProgressReporter,
    ) -> Result<AudioTagPlan, CoreError> {
        self.plan_cancellable(conversion, request, progress, ignore_cancel())
            .await
    }

    pub async fn plan_cancellable(
        &self,
        conversion: &ConversionService,
        request: AudioTagPlanRequest,
        progress: ProgressReporter,
        is_cancelled: CancelCheck,
    ) -> Result<AudioTagPlan, CoreError> {
        let scan_request = request.scan_request();
        let mut scanned = if let Some(files) = self.load_scan(&scan_request) {
            let total = files.len().max(1) as u64;
            progress(super::types::ProgressEvent {
                current: total,
                total,
                message: "使用已掃描的音訊標籤。".into(),

                batch: None,
            });
            files
        } else {
            let files =
                read_audio_files(self, &scan_request, progress.clone(), &is_cancelled).await?;
            self.store_scan(&scan_request, &files);
            files
        };
        let mut prepared = Vec::new();
        let mut warnings = Vec::new();
        let selected_paths = request
            .selected_paths
            .iter()
            .map(|path| resolve_path(path))
            .collect::<HashSet<_>>();

        let total = scanned.len() as u64;
        let mut pending: VecDeque<usize> = (0..scanned.len()).collect();
        let mut preview_batch = Vec::new();
        let mut done = 0u64;
        while let Some(index) = self.take_next(&mut pending, |item| scanned[*item].path.clone()) {
            if is_cancelled() {
                return Err(CoreError::new(
                    "PLAN_CANCELLED",
                    "音訊標籤作業已由使用者取消。",
                ));
            }
            let file = &mut scanned[index];
            file.selected =
                file.warning.is_none() && selected_paths.contains(&resolve_path(&file.path));
            let message = if file.warning.is_some() {
                format!("無法讀取標籤：{}", file_name(Path::new(&file.path)))
            } else {
                file.previewed = true;
                if !file.selected {
                    for field in &mut file.fields {
                        field.selected = false;
                    }
                    format!("已略過未選檔案：{}", file_name(Path::new(&file.path)))
                } else {
                    let mut item =
                        convert_selected_fields(conversion, &request, file, &mut warnings).await?;
                    operations::expand_tag_operations(&request, file, &mut item, &mut warnings)?;
                    if let Some(item) = item {
                        prepared.push(item);
                    }
                    format!("正在建立標籤預覽：{}", file_name(Path::new(&file.path)))
                }
            };
            done += 1;
            preview_batch.push(scanned[index].clone());
            emit_audio_progress(
                &progress,
                done,
                total.max(1),
                message,
                "plan",
                &mut preview_batch,
                false,
            );
        }
        emit_audio_progress(
            &progress,
            done,
            total.max(1),
            "標籤預覽這一批已完成。".into(),
            "plan",
            &mut preview_batch,
            true,
        );

        let plan_id = Uuid::new_v4().to_string();
        let public = AudioTagPlan {
            plan_id: plan_id.clone(),
            created_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            summary: summarize_audio_files(&scanned),
            files: scanned,
            warnings: unique(warnings),
        };
        if let Ok(mut plans) = self.plans.lock() {
            plans.insert(
                plan_id,
                StoredAudioPlan {
                    files: prepared,
                    backup: request.backup != Some(false),
                    backup_roots: resolve_backup_roots(&request.paths)?,
                    conflict_policy: request.conflict_policy,
                },
            );
        }
        Ok(public)
    }

    /// 只重掃一個路徑。有 `plan_id` 時用目前選項重算該檔，並換掉計畫裡同一個路徑的寫入項目。
    pub async fn rescan_file(
        &self,
        conversion: &ConversionService,
        mut request: super::types::AudioRescanFileRequest,
        progress: ProgressReporter,
    ) -> Result<AudioTagFile, CoreError> {
        let target = request.path.clone();
        request.plan.paths = vec![target.clone()];
        request.plan.recursive = Some(false);
        request.plan.selected_paths = vec![target.clone()];
        let mut scanned = read_audio_files(
            self,
            &request.plan.scan_request(),
            progress,
            &ignore_cancel(),
        )
        .await?;
        let Some(mut file) = scanned.pop() else {
            return Err(CoreError::new("AUDIO_NOT_FOUND", "找不到這個音訊檔案。"));
        };
        self.patch_scan_file(&request.plan, &file);
        let Some(plan_id) = request.plan_id.clone() else {
            return Ok(file);
        };
        if file.warning.is_some() {
            self.upsert_prepared(&plan_id, &file.path, None)?;
            return Ok(file);
        }
        file.selected = true;
        let field_ids = file
            .fields
            .iter()
            .filter(|field| field.selected)
            .map(|field| field_id(field.container, &field.key))
            .collect::<Vec<_>>();
        request
            .plan
            .selected_fields
            .insert(file.path.clone(), field_ids);
        let mut warnings = Vec::new();
        let mut prepared =
            convert_selected_fields(conversion, &request.plan, &mut file, &mut warnings).await?;
        operations::expand_tag_operations(&request.plan, &mut file, &mut prepared, &mut warnings)?;
        self.upsert_prepared(&plan_id, &file.path, prepared)?;
        file.previewed = true;
        Ok(file)
    }

    fn upsert_prepared(
        &self,
        plan_id: &str,
        path: &str,
        prepared: Option<PreparedAudio>,
    ) -> Result<(), CoreError> {
        let mut plans = self
            .plans
            .lock()
            .map_err(|_| CoreError::new("PLAN_LOCK", "無法更新音訊計畫。"))?;
        let stored = plans
            .get_mut(plan_id)
            .ok_or_else(|| CoreError::new("PLAN_NOT_FOUND", "音訊標籤計畫已失效。請重新預覽。"))?;
        let resolved = resolve_path(path);
        stored
            .files
            .retain(|item| resolve_path(item.path.to_string_lossy().as_ref()) != resolved);
        if let Some(item) = prepared {
            stored.files.push(item);
        }
        Ok(())
    }

    pub async fn apply(
        &self,
        plan_id: &str,
        progress: ProgressReporter,
        request_cancelled: CancelCheck,
    ) -> Result<ApplyResult, CoreError> {
        self.apply_filtered(plan_id, None, None, progress, request_cancelled)
            .await
    }

    /// `only_paths` 有值時只寫這些路徑。`selected_fields` 有值時每個路徑只寫列出的欄位。
    /// 兩者都省略時寫入計畫裡全部有變更的欄位（無頭模式）。
    pub async fn apply_filtered(
        &self,
        plan_id: &str,
        only_paths: Option<&[String]>,
        selected_fields: Option<&HashMap<String, Vec<String>>>,
        progress: ProgressReporter,
        request_cancelled: CancelCheck,
    ) -> Result<ApplyResult, CoreError> {
        self.clear_scan_cache();
        let plan = self
            .plans
            .lock()
            .ok()
            .and_then(|mut plans| plans.remove(plan_id))
            .ok_or_else(|| CoreError::new("PLAN_NOT_FOUND", "音訊標籤計畫已失效。請重新預覽。"))?;
        if request_cancelled()
            || self
                .cancelled
                .lock()
                .ok()
                .is_some_and(|set| set.contains(plan_id))
        {
            return Err(CoreError::new(
                "PLAN_CANCELLED",
                "音訊標籤作業已由使用者取消。",
            ));
        }
        let mut result = ApplyResult {
            succeeded: Vec::new(),
            skipped: Vec::new(),
            failed: Vec::new(),
            warnings: Vec::new(),
        };
        let allowed = only_paths.map(|paths| {
            paths
                .iter()
                .map(|path| resolve_path(path))
                .collect::<HashSet<_>>()
        });
        let included = |path: &Path| {
            allowed
                .as_ref()
                .is_none_or(|set| set.contains(&resolve_path(&path.to_string_lossy())))
        };
        let total = {
            // 進度分母是整份計畫，不能用「目前索引 + 1」，否則每一筆都是 100%。
            let included = |path: &Path| {
                allowed
                    .as_ref()
                    .is_none_or(|set| set.contains(&resolve_path(&path.to_string_lossy())))
            };
            plan.files
                .iter()
                .filter(|file| included(&file.path))
                .count()
                .max(1) as u64
        };
        let mut files = plan.files;
        let mut seen = 0u64;
        for file in &mut files {
            if !included(&file.path) {
                file.updates.clear();
                file.selected_containers.clear();
                file.remove_containers.clear();
                continue;
            }
            if let Some(selection) = selected_fields {
                restrict_to_fields(file, &fields_for_path(selection, &file.path));
            }
        }
        let writable = files
            .iter()
            .filter(|file| file_should_write(file))
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        if plan.backup && !writable.is_empty() {
            progress(super::types::ProgressEvent {
                current: 0,
                total: files.len().max(1) as u64,
                message: "正在建立備份…".into(),

                batch: None,
            });
            if let Err(error) =
                create_user_backups(&plan.backup_roots, &writable, plan.conflict_policy)
            {
                if let Ok(mut cancelled) = self.cancelled.lock() {
                    cancelled.remove(plan_id);
                }
                result.failed.push(ApplyFailure {
                    path: "備份".into(),
                    message: error.message,
                });
                return Ok(result);
            }
        }

        for file in files.into_iter() {
            self.throw_if_cancelled(plan_id)?;
            if request_cancelled() {
                return Err(CoreError::new(
                    "PLAN_CANCELLED",
                    "音訊標籤作業已由使用者取消。",
                ));
            }
            if !included(&file.path) {
                result
                    .skipped
                    .push(file.path.to_string_lossy().into_owned());
                continue;
            }
            seen += 1;
            if !file_should_write(&file) {
                result
                    .skipped
                    .push(file.path.to_string_lossy().into_owned());
                progress(super::types::ProgressEvent {
                    current: seen,
                    total,
                    message: format!("已略過：{}", file_name(&file.path)),

                    batch: None,
                });
                continue;
            }
            match apply_file(&file) {
                Ok(()) => result
                    .succeeded
                    .push(file.path.to_string_lossy().into_owned()),
                Err(error) => result.failed.push(ApplyFailure {
                    path: file.path.to_string_lossy().into_owned(),
                    message: error.message,
                }),
            }
            progress(super::types::ProgressEvent {
                current: seen,
                total,
                message: format!("正在寫入標籤：{}", file_name(&file.path)),

                batch: None,
            });
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
            return Err(CoreError::new(
                "PLAN_CANCELLED",
                "音訊標籤作業已由使用者取消。",
            ));
        }
        Ok(())
    }
}

fn emit_audio_progress(
    progress: &ProgressReporter,
    current: u64,
    total: u64,
    message: String,
    phase: &str,
    batch: &mut Vec<AudioTagFile>,
    force: bool,
) {
    if let Some(files) = drain_stream_batch(batch, force) {
        if files.is_empty() {
            return;
        }
        progress(
            super::types::ProgressEvent::tick(current, total, message)
                .with_audio_batch(phase, files),
        );
    } else if !force {
        progress(super::types::ProgressEvent::tick(current, total, message));
    }
}

async fn read_audio_files(
    service: &AudioService,
    request: &AudioScanRequest,
    progress: ProgressReporter,
    is_cancelled: &CancelCheck,
) -> Result<Vec<AudioTagFile>, CoreError> {
    let paths = expand_audio_paths(&request.paths, request.recursive.unwrap_or(false))?;
    let total = paths.len().max(1) as u64;
    let mut pending: VecDeque<PathBuf> = paths.into();
    let mut files = Vec::new();
    let mut batch = Vec::new();
    let mut done = 0u64;
    while let Some(path) =
        service.take_next(&mut pending, |item| item.to_string_lossy().into_owned())
    {
        if is_cancelled() {
            return Err(CoreError::new(
                "PLAN_CANCELLED",
                "音訊標籤作業已由使用者取消。",
            ));
        }
        let file = match scan_file(&path, request) {
            Ok(file) => file,
            Err(error) => AudioTagFile {
                path: path.to_string_lossy().into_owned(),
                format: format_from_path(&path).unwrap_or(AudioFormat::Mp3),
                selected: false,
                fields: Vec::new(),
                has_cover_art: false,
                duration_seconds: None,
                warning: Some(error.message),
                previewed: false,
                rewrite_containers: Vec::new(),
                removals: Vec::new(),
            },
        };
        done += 1;
        batch.push(file.clone());
        files.push(file);
        emit_audio_progress(
            &progress,
            done,
            total,
            format!("正在掃描：{}", file_name(&path)),
            "scan",
            &mut batch,
            false,
        );
    }
    emit_audio_progress(
        &progress,
        done,
        total,
        "掃描這一批音訊標籤。".into(),
        "scan",
        &mut batch,
        true,
    );
    Ok(files)
}

fn scan_file(path: &Path, request: &AudioScanRequest) -> Result<AudioTagFile, CoreError> {
    let format = format_from_path(path)?;
    if format == AudioFormat::Mp3 {
        return scan_mp3(
            path,
            request.id3v1_source_encoding.unwrap_or(TextEncoding::Gbk),
            request.id3v2_source_encoding.unwrap_or(TextEncoding::Gbk),
            request.id3v2_repair_source_encoding.unwrap_or(false),
        );
    }
    let tagged = read_tagged(path)?;
    let mut fields = Vec::new();
    for tag in tagged.tags() {
        let Some(container) = operations::container_of_lofty(tag.tag_type()) else {
            continue;
        };
        // MP3 的 ID3 由 scan_mp3 讀。這裡的 APE／OGG／OPUS 要讀全部標籤，
        // 不能只留 primary_tag，否則 APE 檔裡的 ID3v1 會消失。
        push_tag_fields(&mut fields, container, tag);
    }
    let has_cover_art = tagged.tags().iter().any(|tag| !tag.pictures().is_empty());
    Ok(AudioTagFile {
        path: path.to_string_lossy().into_owned(),
        format,
        selected: true,
        fields,
        has_cover_art,
        duration_seconds: Some(tagged.properties().duration().as_secs_f64()),
        warning: None,
        previewed: false,
        rewrite_containers: Vec::new(),
        removals: Vec::new(),
    })
}

fn scan_mp3(
    path: &Path,
    id3v1_encoding: TextEncoding,
    id3v2_encoding: TextEncoding,
    repair_id3v2: bool,
) -> Result<AudioTagFile, CoreError> {
    let buffer = fs::read(path)?;
    let mut fields = Vec::new();
    if let Some(v1) = read_id3v1(&buffer, id3v1_encoding)? {
        for (key, value) in v1.values {
            fields.push(make_field(AudioContainer::Id3v1, key, vec![value]));
        }
    }
    if let Ok(tag) = id3::Tag::read_from_path(path) {
        for key in STANDARD_FIELDS {
            if let Some(value) = id3v2_standard(&tag, key) {
                fields.push(make_field(
                    AudioContainer::Id3v2,
                    key,
                    vec![repair_id3v2_value(&value, id3v2_encoding, repair_id3v2)?],
                ));
            }
        }
        for frame in tag.frames() {
            let id = frame.id();
            if COMMON_ID3V2_FRAMES.contains(&id) {
                continue;
            }
            if id.starts_with('T') && id.len() >= 3 && id.len() <= 4 {
                // TXXX 可以有多框，彼此靠 description 區分。合成同一個 frame:TXXX
                // 再 set_text 會只留下一框。
                if let Some(extended) = frame.content().extended_text() {
                    let mut field = make_field(
                        AudioContainer::Id3v2,
                        &format!("frame:{id}:{}", extended.description),
                        vec![repair_id3v2_value(
                            &extended.value,
                            id3v2_encoding,
                            repair_id3v2,
                        )?],
                    );
                    field.label = if extended.description.is_empty() {
                        id.to_string()
                    } else {
                        format!("{id} ({})", extended.description)
                    };
                    field.selected = false;
                    fields.push(field);
                    continue;
                }
                if let Some(text) = frame.content().text() {
                    let mut field = make_field(
                        AudioContainer::Id3v2,
                        &format!("frame:{id}"),
                        vec![repair_id3v2_value(text, id3v2_encoding, repair_id3v2)?],
                    );
                    field.label = id.to_string();
                    field.selected = false;
                    fields.push(field);
                }
            }
        }
    }
    let has_cover_art = id3::Tag::read_from_path(path)
        .ok()
        .is_some_and(|tag| tag.pictures().next().is_some());
    // MP3 上的 APEv2 不在 id3 crate 裡。讀不到就保持 ID3 結果，不讓整檔失敗。
    if let Ok(tagged) = read_tagged(path) {
        for tag in tagged.tags() {
            if tag.tag_type() == lofty::tag::TagType::Ape {
                push_tag_fields(&mut fields, AudioContainer::Apev2, tag);
            }
        }
    }
    Ok(AudioTagFile {
        path: path.to_string_lossy().into_owned(),
        format: AudioFormat::Mp3,
        selected: true,
        fields,
        has_cover_art,
        duration_seconds: None,
        warning: None,
        previewed: false,
        rewrite_containers: Vec::new(),
        removals: Vec::new(),
    })
}

fn push_tag_fields(
    fields: &mut Vec<AudioTagField>,
    container: AudioContainer,
    tag: &lofty::tag::Tag,
) {
    push_standard_fields(fields, container, tag);
    let mut custom_order = Vec::new();
    let mut custom: HashMap<&'static str, Vec<String>> = HashMap::new();
    for item in tag.items() {
        if let Some(key) = custom_item_key(item.key()) {
            if let ItemValue::Text(text) = item.value() {
                if !custom.contains_key(key) {
                    custom_order.push(key);
                }
                custom.entry(key).or_default().push(text.clone());
            }
        }
    }
    for key in custom_order {
        if let Some(values) = custom.remove(key) {
            fields.push(make_field(container, key, values));
        }
    }
}

fn frame_carries_text(frame: &id3::frame::Frame) -> bool {
    let id = frame.id();
    id.starts_with('T') || matches!(id, "COMM" | "USLT" | "SYLT" | "USER" | "WXXX")
}

fn id3_frame_selected(frame: &id3::frame::Frame, keys: &HashSet<String>) -> bool {
    let frame_id = frame.id();
    keys.iter().any(|key| match key.as_str() {
        "title" => frame_id == "TIT2",
        "artist" => frame_id == "TPE1",
        "album" => frame_id == "TALB",
        "year" => matches!(frame_id, "TYER" | "TDRC"),
        "track" => frame_id == "TRCK",
        "genre" => frame_id == "TCON",
        "comment" => frame_id == "COMM",
        "albumArtist" => frame_id == "TPE2",
        "composer" => frame_id == "TCOM",
        "lyrics" => frame_id == "USLT",
        other => match other.strip_prefix("frame:") {
            // frame:TXXX:<description> 只對到同一個描述，避免改一框時連帶改其餘 TXXX。
            Some(rest) => match rest.split_once(':') {
                Some((id, description)) if id == "TXXX" => {
                    frame_id == "TXXX"
                        && frame
                            .content()
                            .extended_text()
                            .is_some_and(|extended| extended.description == description)
                }
                _ => rest == frame_id,
            },
            None => false,
        },
    })
}

fn selected_id3v2_keys(file: &PreparedAudio) -> HashSet<String> {
    fields_for_path(&file.request.selected_fields, &file.path)
        .into_iter()
        .filter_map(|identifier| {
            let (container, key) = split_field_id(&identifier);
            (container == AudioContainer::Id3v2).then_some(key)
        })
        .collect()
}

/// 只改已勾選文字框的編碼。未勾選的文字框與封面保持原編碼。
fn apply_id3v2_encoding(tag: &mut id3::Tag, encoding: &str, keys: &HashSet<String>) {
    use id3::{frame::Frame, Encoding, TagLike};
    let encoding = match encoding {
        "utf16" | "utf-16" => Encoding::UTF16,
        "utf16be" | "utf-16be" => Encoding::UTF16BE,
        "latin1" => Encoding::Latin1,
        _ => Encoding::UTF8,
    };
    let frames: Vec<Frame> = tag
        .frames()
        .cloned()
        .map(|frame| {
            if frame_carries_text(&frame) && id3_frame_selected(&frame, keys) {
                frame.set_encoding(Some(encoding))
            } else {
                frame
            }
        })
        .collect();
    let mut replacement = id3::Tag::new();
    for frame in frames {
        replacement.add_frame(frame);
    }
    *tag = replacement;
}

fn apply_file(file: &PreparedAudio) -> Result<(), CoreError> {
    if file.format == AudioFormat::Mp3 {
        apply_mp3(file)
    } else {
        apply_taglib(file)
    }
}

fn apply_mp3(file: &PreparedAudio) -> Result<(), CoreError> {
    let temporary = temporary_path(&file.path);
    let result = (|| -> Result<(), CoreError> {
        let source = fs::read(&file.path)?;
        let remove_v2 = file.remove_containers.contains(&AudioContainer::Id3v2);
        let remove_v1 = file.remove_containers.contains(&AudioContainer::Id3v1);
        let remove_ape = file.remove_containers.contains(&AudioContainer::Apev2);
        let writes_v2 = !remove_v2 && file.selected_containers.contains(&AudioContainer::Id3v2);
        let writes_v1 = !remove_v1 && file.selected_containers.contains(&AudioContainer::Id3v1);
        fs::copy(&file.path, &temporary)?;
        let mut output = if writes_v2 {
            // 原檔已有 ID3v2 卻解析失敗時，空標籤會整段換掉讀不到的影格。
            // 檔頭沒有 ID3v2（含只有 ID3v1）才允許從空標籤建立。
            let has_id3v2 = id3v2_tag_length(&source) > 0;
            let mut tag = match id3::Tag::read_from_path(&temporary) {
                Ok(tag) => tag,
                Err(error) if has_id3v2 => {
                    let _ = fs::remove_file(&temporary);
                    return Err(CoreError::new(
                        "ID3_READ",
                        format!("原檔有 ID3v2，但無法讀取，已停止寫入：{error}"),
                    ));
                }
                Err(_) => id3::Tag::new(),
            };
            for (identifier, values) in &file.updates {
                let (container, key) = split_field_id(identifier);
                if container != AudioContainer::Id3v2 {
                    continue;
                }
                set_id3v2_value(&mut tag, &key, values.first().cloned().unwrap_or_default());
            }
            let version = if file.request.id3v2_version == 4 {
                id3::Version::Id3v24
            } else {
                id3::Version::Id3v23
            };
            // 版本不同時 write_to_path 會換標頭。文字編碼已經符合時不要再重寫每一框。
            if file.rewrite_containers.contains(&AudioContainer::Id3v2)
                && id3v2_text_encodings(&source)
                    .into_iter()
                    .any(|encoding| encoding != id3_encoding_byte(&file.request.id3v2_encoding))
            {
                apply_id3v2_encoding(
                    &mut tag,
                    &file.request.id3v2_encoding,
                    &selected_id3v2_keys(file),
                );
            }
            tag.write_to_path(&temporary, version)
                .map_err(|error| CoreError::new("ID3_WRITE", error.to_string()))?;
            fs::read(&temporary)?
        } else if remove_v2 {
            strip_id3v1(&strip_id3v2(&source)).to_vec()
        } else {
            strip_id3v1(&source).to_vec()
        };
        if writes_v2 {
            if id3v2_header_version(&output) != Some(file.request.id3v2_version) {
                let _ = fs::remove_file(&temporary);
                return Err(CoreError::new(
                    "ID3_WRITE",
                    format!(
                        "ID3v2 版本寫入為 {}，預期 {}。",
                        id3v2_header_version(&output)
                            .map_or_else(|| "未知".into(), |value| value.to_string()),
                        file.request.id3v2_version
                    ),
                ));
            }
        }
        let existing_v1 = read_id3v1(
            &source,
            file.request
                .id3v1_source_encoding
                .unwrap_or(TextEncoding::Gbk),
        )?;
        if writes_v1 {
            let mut values = existing_v1
                .as_ref()
                .map(|item| item.values.clone())
                .unwrap_or_else(empty_id3v1);
            for (identifier, converted) in &file.updates {
                let (container, key) = split_field_id(identifier);
                if container == AudioContainer::Id3v1 {
                    if let Some(slot) = values.get_mut(key.as_str()) {
                        *slot = converted.first().cloned().unwrap_or_default();
                    }
                }
            }
            output = append_id3v1(
                &strip_id3v1(&output),
                &values,
                existing_v1
                    .as_ref()
                    .map(|item| item.genre_code)
                    .unwrap_or(255),
                file.request.id3v1_output_encoding,
            )?;
            fs::write(&temporary, &output)?;
        } else if remove_v1 {
            output = strip_id3v1(&output).to_vec();
            fs::write(&temporary, &output)?;
        } else if existing_v1.is_some() {
            output = [strip_id3v1(&output), &source[source.len() - 128..]].concat();
            fs::write(&temporary, &output)?;
        } else if !writes_v2 {
            fs::write(&temporary, &output)?;
        }
        if remove_ape {
            output = strip_trailing_ape(&fs::read(&temporary).unwrap_or(output));
            fs::write(&temporary, &output)?;
        }

        if verified_mp3_audio(&source) != verified_mp3_audio(&fs::read(&temporary)?) {
            let _ = fs::remove_file(&temporary);
            return Err(CoreError::new(
                "AUDIO_VERIFY",
                "MP3 標籤寫入改變了音訊資料。",
            ));
        }
        // 拿掉整個 ID3v2 時封面跟著消失，不比對。其餘寫入（含移除 ID3v1／APEv2）要留下原本的 APIC。
        if !remove_v2 && picture_count(&temporary)? != file.original_picture_count {
            let _ = fs::remove_file(&temporary);
            return Err(CoreError::new(
                "AUDIO_PICTURE",
                "標籤寫入造成封面圖片數量改變。",
            ));
        }
        if remove_v2 {
            reject_leftover_id3_family(&fs::read(&temporary)?)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    finish_temporary(&file.path, &temporary)
}

fn apply_taglib(file: &PreparedAudio) -> Result<(), CoreError> {
    let source = fs::read(&file.path)?;
    let temporary = temporary_path(&file.path);
    fs::copy(&file.path, &temporary)?;
    let result = (|| -> Result<(), CoreError> {
        if file
            .remove_containers
            .contains(&AudioContainer::VorbisComment)
        {
            return Err(CoreError::new(
                "AUDIO_REMOVE",
                "第一版不能移除 vorbis-comment。",
            ));
        }
        if file.remove_containers.is_empty() || !file.updates.is_empty() {
            let mut tagged = read_tagged(&temporary)?;
            let tag = tagged
                .primary_tag_mut()
                .ok_or_else(|| CoreError::new("AUDIO_INVALID", "音訊檔案沒有可寫入的標籤。"))?;
            for (identifier, values) in &file.updates {
                let (_, key) = split_field_id(identifier);
                set_lofty_values(tag, &key, values);
            }
            tagged
                .save_to_path(&temporary, WriteOptions::default())
                .map_err(|error| CoreError::new("AUDIO_WRITE", error.to_string()))?;
        }
        if !file.remove_containers.is_empty() {
            if file.format == AudioFormat::Ape {
                // TagType::remove_from_path 經 Probe::open 以唯讀開啟，
                // 寫回時 set_len 得到 EINVAL。檔尾有無 ID3v1 都一樣失敗。
                // APE 允許的容器只在檔頭（ID3v2）或檔尾（APEv2、ID3v1），直接剝除。
                let mut output = fs::read(&temporary)?;
                if file.remove_containers.contains(&AudioContainer::Id3v2) {
                    output = strip_id3v2(&output);
                }
                if file.remove_containers.contains(&AudioContainer::Apev2) {
                    output = strip_trailing_ape(&output);
                }
                if file.remove_containers.contains(&AudioContainer::Id3v1) {
                    output = strip_id3v1(&output).to_vec();
                }
                fs::write(&temporary, &output)?;
            } else {
                let mut tagged = read_tagged(&temporary)?;
                for container in &file.remove_containers {
                    let tag_type = lofty_tag_type(*container).ok_or_else(|| {
                        CoreError::new("AUDIO_REMOVE", "第一版不能移除這個標籤。")
                    })?;
                    // 記憶體裡先拿掉，磁碟再交給 TagType::remove_from_path。
                    // save_to 只會重寫還留著的標籤，不會清掉已刪的容器。
                    let _ = tagged.remove(tag_type);
                    tag_type
                        .remove_from_path(&temporary, WriteOptions::default())
                        .map_err(|error| {
                            let mut detail = error.to_string();
                            let mut source = std::error::Error::source(&error);
                            while let Some(inner) = source {
                                detail.push_str(": ");
                                detail.push_str(&inner.to_string());
                                source = inner.source();
                            }
                            CoreError::new("AUDIO_REMOVE", detail)
                        })?;
                }
            }
        }
        if file.format == AudioFormat::Ape {
            let written = fs::read(&temporary)?;
            if verified_ape_audio(&source) != verified_ape_audio(&written) {
                return Err(CoreError::new("AUDIO_VERIFY", "標籤寫入改變了音訊資料。"));
            }
        }
        if file.remove_containers.contains(&AudioContainer::Id3v2) {
            reject_leftover_id3_family(&fs::read(&temporary)?)?;
        }
        let verification = read_tagged(&temporary).map_err(|error| {
            CoreError::new(
                "AUDIO_VERIFY",
                format!("標籤寫入後的音訊檔案無法驗證。{error}"),
            )
        })?;
        if file.remove_containers.is_empty() {
            let pictures = verification
                .primary_tag()
                .map(|tag| tag.pictures().len())
                .unwrap_or(0);
            if pictures != file.original_picture_count {
                return Err(CoreError::new(
                    "AUDIO_PICTURE",
                    "標籤寫入造成封面圖片數量改變。",
                ));
            }
        } else if let Ok(before) = read_tagged(&file.path) {
            for tag in before.tags() {
                let removed = file
                    .remove_containers
                    .iter()
                    .any(|container| lofty_tag_type(*container) == Some(tag.tag_type()));
                if removed {
                    continue;
                }
                let after_count = verification
                    .tag(tag.tag_type())
                    .map(|item| item.pictures().len())
                    .unwrap_or(0);
                if after_count != tag.pictures().len() {
                    return Err(CoreError::new(
                        "AUDIO_PICTURE",
                        "標籤寫入造成封面圖片數量改變。",
                    ));
                }
            }
        }
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    finish_temporary(&file.path, &temporary)
}

fn push_standard_fields(fields: &mut Vec<AudioTagField>, container: AudioContainer, tag: &Tag) {
    for (key, item_key) in [
        ("title", ItemKey::TrackTitle),
        ("artist", ItemKey::TrackArtist),
        ("album", ItemKey::AlbumTitle),
        ("genre", ItemKey::Genre),
        ("comment", ItemKey::Comment),
    ] {
        let values: Vec<String> = tag.get_strings(item_key).map(str::to_owned).collect();
        if !values.is_empty() {
            fields.push(make_field(container, key, values));
        }
    }
    if let Some(date) = tag.date() {
        fields.push(make_field(container, "year", vec![date.year.to_string()]));
    }
    if let Some(value) = tag.track() {
        fields.push(make_field(container, "track", vec![value.to_string()]));
    }
}

fn custom_item_key(key: ItemKey) -> Option<&'static str> {
    match key {
        ItemKey::AlbumArtist => Some("albumArtist"),
        ItemKey::Composer => Some("composer"),
        ItemKey::Lyrics | ItemKey::UnsyncLyrics => Some("lyrics"),
        _ => None,
    }
}

fn item_key_for_field(key: &str) -> Option<ItemKey> {
    match key {
        "title" => Some(ItemKey::TrackTitle),
        "artist" => Some(ItemKey::TrackArtist),
        "album" => Some(ItemKey::AlbumTitle),
        "genre" => Some(ItemKey::Genre),
        "comment" => Some(ItemKey::Comment),
        "albumArtist" => Some(ItemKey::AlbumArtist),
        "composer" => Some(ItemKey::Composer),
        "lyrics" => Some(ItemKey::Lyrics),
        _ => None,
    }
}

fn set_lofty_values(tag: &mut Tag, key: &str, values: &[String]) {
    match key {
        "year" => {
            if let Some(value) = values.first() {
                if let Ok(year) = value.parse::<u16>() {
                    tag.set_date(Timestamp {
                        year,
                        ..Timestamp::default()
                    });
                }
            }
        }
        "track" => {
            if let Some(value) = values.first() {
                if let Ok(track) = value.parse() {
                    tag.set_track(track);
                }
            }
        }
        _ => {
            let Some(item_key) = item_key_for_field(key) else {
                return;
            };
            tag.remove_key(item_key);
            for value in values {
                let _ = tag.push(TagItem::new(item_key, ItemValue::Text(value.clone())));
            }
        }
    }
}

fn id3v2_standard(tag: &id3::Tag, key: &str) -> Option<String> {
    use id3::TagLike;
    match key {
        "title" => tag.title().map(ToOwned::to_owned),
        "artist" => tag.artist().map(ToOwned::to_owned),
        "album" => tag.album().map(ToOwned::to_owned),
        // 純數字走 year()/track()。TYER／TRCK 也可以是「录制年」「第一辑」，
        // 解析失敗時仍要讀原文字，否則寫入後下次掃描會把欄位弄丟。
        "year" => tag.year().map(|value| value.to_string()).or_else(|| {
            tag.get("TYER")
                .or_else(|| tag.get("TDRC"))
                .and_then(|frame| frame.content().text())
                .map(ToOwned::to_owned)
        }),
        "track" => tag.track().map(|value| value.to_string()).or_else(|| {
            tag.get("TRCK")
                .and_then(|frame| frame.content().text())
                .map(ToOwned::to_owned)
        }),
        "genre" => tag.genre().map(ToOwned::to_owned),
        "comment" => tag.comments().next().map(|comment| comment.text.clone()),
        _ => None,
    }
}

fn set_id3v2_value(tag: &mut id3::Tag, key: &str, value: String) {
    use id3::TagLike;
    match key {
        "title" => tag.set_title(value),
        "artist" => tag.set_artist(value),
        "album" => tag.set_album(value),
        "year" => {
            if let Ok(year) = value.parse() {
                tag.set_year(year);
            } else {
                // 預覽已顯示這段文字。解析失敗時寫原 TYER，不要留下舊年份。
                tag.set_text("TYER", value);
            }
        }
        "track" => {
            if let Ok(track) = value.parse() {
                tag.set_track(track);
            } else {
                tag.set_text("TRCK", value);
            }
        }
        "genre" => tag.set_genre(value),
        "albumArtist" => tag.set_album_artist(value),
        "composer" => tag.set_text("TCOM", value),
        "lyrics" => {
            // 改第一則歌詞的文字，其餘語言與描述保留。沒有歌詞才新建 eng 框。
            let mut lyrics: Vec<_> = tag.lyrics().cloned().collect();
            tag.remove_all_lyrics();
            if lyrics.is_empty() {
                tag.add_frame(id3::frame::Lyrics {
                    lang: "eng".into(),
                    description: String::new(),
                    text: value,
                });
            } else {
                lyrics[0].text = value;
                for lyric in lyrics {
                    tag.add_frame(lyric);
                }
            }
        }
        "comment" => {
            // 改第一則註解的文字，其餘保留。重複套用不會再追加一框。
            let mut comments: Vec<_> = tag.comments().cloned().collect();
            tag.remove_comment(None, None);
            if comments.is_empty() {
                tag.add_frame(id3::frame::Comment {
                    lang: "eng".into(),
                    description: String::new(),
                    text: value,
                });
            } else {
                comments[0].text = value;
                for comment in comments {
                    tag.add_frame(comment);
                }
            }
        }
        other if other.starts_with("frame:") => {
            let rest = other.trim_start_matches("frame:");
            if let Some((id, description)) = rest.split_once(':') {
                if id == "TXXX" {
                    // 只換這個 description，其餘 TXXX 照原順序寫回。
                    // set_text("TXXX") 會加成沒有描述的文字框，和其他 TXXX 分不開。
                    let mut texts: Vec<_> = tag.extended_texts().cloned().collect();
                    tag.remove_extended_text(None, None);
                    if let Some(item) = texts
                        .iter_mut()
                        .find(|item| item.description == description)
                    {
                        item.value = value;
                    } else {
                        texts.push(id3::frame::ExtendedText {
                            description: description.to_string(),
                            value,
                        });
                    }
                    for text in texts {
                        tag.add_frame(text);
                    }
                    return;
                }
            }
            tag.set_text(rest, value);
        }
        _ => {}
    }
}

const COMMON_ID3V2_FRAMES: &[&str] = &[
    "TT2", "TIT2", "TP1", "TPE1", "TAL", "TALB", "TYE", "TYER", "TDRC", "TRK", "TRCK", "TCO",
    "TCON",
];

async fn convert_selected_fields(
    conversion: &ConversionService,
    request: &AudioTagPlanRequest,
    file: &mut AudioTagFile,
    warnings: &mut Vec<String>,
) -> Result<Option<PreparedAudio>, CoreError> {
    let selected = request
        .selected_fields
        .get(&file.path)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect::<HashSet<_>>();
    let mut updates = HashMap::new();
    let mut containers = HashSet::new();
    for field in &mut file.fields {
        let identifier = field_id(field.container, &field.key);
        field.selected =
            selected.contains(&identifier) && container_enabled(request, field.container);
        if !field.selected {
            continue;
        }
        let mut converted_values = Vec::new();
        for value in &field.values {
            let result = conversion
                .convert(conversion_for_container(request, field.container).with_text(value))
                .await?;
            converted_values.push(result.text);
            warnings.extend(result.warnings);
        }
        if converted_values != field.values {
            field.converted_values = Some(converted_values.clone());
            field.origin = Some("convert".into());
            updates.insert(identifier, converted_values);
            containers.insert(field.container);
        } else {
            field.converted_values = None;
            field.origin = None;
        }
    }
    let rewrite = rewrite_containers(request, file);
    for container in &rewrite {
        containers.insert(*container);
    }
    file.rewrite_containers = rewrite.clone();
    if containers.contains(&AudioContainer::Id3v1) || rewrite.contains(&AudioContainer::Id3v1) {
        note_id3v1_truncation(request, file, &mut updates, warnings)?;
    }
    if updates.is_empty() && rewrite.is_empty() {
        return Ok(None);
    }
    Ok(Some(PreparedAudio {
        path: PathBuf::from(&file.path),
        format: file.format,
        updates,
        selected_containers: containers,
        rewrite_containers: rewrite.into_iter().collect(),
        remove_containers: HashSet::new(),
        request: request.clone(),
        original_picture_count: if file.has_cover_art {
            picture_count(Path::new(&file.path))?
        } else {
            0
        },
    }))
}

fn container_enabled(request: &AudioTagPlanRequest, container: AudioContainer) -> bool {
    match container {
        AudioContainer::Id3v1 => request.id3v1_enabled,
        AudioContainer::Id3v2 => request.id3v2_enabled,
        _ => true,
    }
}

fn conversion_for_container(
    request: &AudioTagPlanRequest,
    container: AudioContainer,
) -> ConversionOptions {
    let mut conversion = request.conversion.clone();
    conversion.direction = match container {
        AudioContainer::Id3v1 => request.id3v1_direction,
        AudioContainer::Id3v2 => request.id3v2_direction,
        _ => request.conversion.direction,
    };
    conversion.zhconvert = match container {
        AudioContainer::Id3v1 => request
            .id3v1_zhconvert
            .clone()
            .or(request.conversion.zhconvert.clone()),
        AudioContainer::Id3v2 => request
            .id3v2_zhconvert
            .clone()
            .or(request.conversion.zhconvert.clone()),
        _ => request.conversion.zhconvert.clone(),
    };
    conversion
}

fn repair_id3v2_value(
    value: &str,
    encoding: TextEncoding,
    enabled: bool,
) -> Result<String, CoreError> {
    if !enabled || value.is_empty() {
        return Ok(value.to_string());
    }
    let latin1 = "¡¢£¤¥¦§¨©ª«¬®¯°±²³´µ¶·¸¹º»¼½¾¿ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕÖ×ØÙÚÛÜÝÞßàáâãäåæçèéêëìíîïðñòóôõö÷øùúûüýþÿ";
    let count = value
        .chars()
        .filter(|character| latin1.contains(*character))
        .count();
    if count as f64 / value.chars().count() as f64 <= 0.2 {
        return Ok(value.to_string());
    }
    let bytes = value
        .chars()
        .map(|character| character as u8)
        .collect::<Vec<_>>();
    Ok(decode_text(&bytes, encoding)?.0)
}

fn lofty_tag_type(container: AudioContainer) -> Option<lofty::tag::TagType> {
    match container {
        AudioContainer::Apev2 => Some(lofty::tag::TagType::Ape),
        AudioContainer::Id3v1 => Some(lofty::tag::TagType::Id3v1),
        AudioContainer::Id3v2 => Some(lofty::tag::TagType::Id3v2),
        AudioContainer::VorbisComment => None,
    }
}

/// 去掉 APE 檔頭尾的 APEv2、ID3v2、ID3v1，留下 Monkey's Audio 本體。
#[cfg(test)]
fn ape_audio_payload(buffer: &[u8]) -> Vec<u8> {
    let without_v1 = strip_id3v1(buffer);
    let without_tail = strip_ape_tail(without_v1);
    strip_leading_ape(&strip_id3v2(without_tail))
}

#[cfg(test)]
fn strip_leading_ape(buffer: &[u8]) -> Vec<u8> {
    if buffer.len() < 32 || &buffer[..8] != b"APETAGEX" {
        return buffer.to_vec();
    }
    let size = u32::from_le_bytes(buffer[12..16].try_into().unwrap_or([0; 4])) as usize;
    let flags = u32::from_le_bytes(buffer[20..24].try_into().unwrap_or([0; 4]));
    // size 含 footer、不含 header。bit 31 才表示前面多了 32 bytes 標頭。
    let total = if flags & 0x8000_0000 != 0 {
        size.saturating_add(32)
    } else {
        size
    };
    if total < 32 || total > buffer.len() {
        return buffer.to_vec();
    }
    buffer[total..].to_vec()
}

#[cfg(test)]
fn mp3_audio_payload(buffer: &[u8]) -> Vec<u8> {
    let without_v1 = strip_ape_tail(strip_id3v1(buffer));
    let mut start = id3v2_tag_length(without_v1);
    while start + 1 < without_v1.len()
        && !(without_v1[start] == 0xff && without_v1[start + 1] >= 0xf0)
    {
        start += 1;
    }
    without_v1[start..].to_vec()
}

fn id3v2_tag_length(buffer: &[u8]) -> usize {
    if buffer.len() < 10 || &buffer[..3] != b"ID3" {
        return 0;
    }
    let size = ((buffer[6] as usize & 0x7f) << 21)
        | ((buffer[7] as usize & 0x7f) << 14)
        | ((buffer[8] as usize & 0x7f) << 7)
        | (buffer[9] as usize & 0x7f);
    let footer = if buffer[3] == 4 && buffer[5] & 0x10 != 0 {
        10
    } else {
        0
    };
    10 + size + footer
}

fn id3v2_header_version(buffer: &[u8]) -> Option<u8> {
    if buffer.len() >= 4 && &buffer[..3] == b"ID3" {
        Some(buffer[3])
    } else {
        None
    }
}

pub(crate) fn summarize_audio_files(files: &[AudioTagFile]) -> AudioTagPlanSummary {
    let mut changed = 0usize;
    let mut errors = 0usize;
    let mut selected_fields = 0usize;
    for file in files {
        if file.warning.is_some() {
            errors += 1;
            continue;
        }
        let mut file_changed = !file.rewrite_containers.is_empty()
            || file
                .removals
                .iter()
                .any(|removal| removal.selected && removal.refused.is_none());
        for field in &file.fields {
            if field
                .converted_values
                .as_ref()
                .is_some_and(|values| values != &field.values)
            {
                file_changed = true;
            }
            if file.selected && field.selected {
                selected_fields += 1;
            }
        }
        if file_changed {
            changed += 1;
        }
    }
    let total = files.len();
    AudioTagPlanSummary {
        total,
        changed,
        unchanged: total - changed - errors,
        errors,
        selected_fields,
    }
}

fn make_field(container: AudioContainer, key: &str, values: Vec<String>) -> AudioTagField {
    AudioTagField {
        key: key.to_string(),
        label: label_for(key).to_string(),
        container,
        values,
        converted_values: None,
        selected: STANDARD_FIELDS.contains(&key),
        truncated: false,
        truncation: None,
        origin: None,
        warnings: Vec::new(),
    }
}

fn id3v1_comment_limit(file: &AudioTagFile) -> usize {
    let track = file
        .fields
        .iter()
        .find(|field| field.container == AudioContainer::Id3v1 && field.key == "track");
    let text = track.and_then(|field| {
        field
            .converted_values
            .as_ref()
            .and_then(|values| values.first())
            .or_else(|| field.values.first())
    });
    let number = text.and_then(|value| value.parse::<u8>().ok()).unwrap_or(0);
    if number > 0 {
        28
    } else {
        30
    }
}

fn id3v1_text_limit(key: &str, comment_limit: usize) -> Option<usize> {
    match key {
        "title" | "artist" | "album" => Some(30),
        "year" => Some(4),
        "comment" => Some(comment_limit),
        _ => None,
    }
}

#[derive(Debug)]
pub(super) struct EncodedFit {
    pub text: String,
    pub truncated: bool,
    pub from_bytes: usize,
    pub to_bytes: usize,
    pub unrepresentable: usize,
}

fn char_representable(character: char, encoding: TextEncoding) -> bool {
    let text = character.to_string();
    encode_text_detailed(&text, encoding, false)
        .map(|encoded| !encoded.had_errors)
        .unwrap_or(false)
}

/// 先把目標編碼沒有的字改成 `?`，再依位元組上限停在完整字元。
pub(super) fn fit_encoded_text(
    value: &str,
    limit: usize,
    encoding: TextEncoding,
) -> Result<EncodedFit, CoreError> {
    let mut unrepresentable = 0usize;
    let mut substituted = String::new();
    for character in value.chars() {
        if char_representable(character, encoding) {
            substituted.push(character);
        } else {
            unrepresentable += 1;
            substituted.push('?');
        }
    }
    let from_bytes = encode_text(&substituted, encoding, false)?.len();
    let (text, truncated) = truncate_to_encoding(&substituted, limit, encoding)?;
    let to_bytes = encode_text(&text, encoding, false)?.len();
    Ok(EncodedFit {
        text,
        truncated,
        from_bytes,
        to_bytes,
        unrepresentable,
    })
}

/// 依輸出編碼逐字放入，停在最後一個完整字元。
fn truncate_to_encoding(
    value: &str,
    limit: usize,
    encoding: TextEncoding,
) -> Result<(String, bool), CoreError> {
    if encode_text(value, encoding, false)?.len() <= limit {
        return Ok((value.to_string(), false));
    }
    let mut kept = String::new();
    for character in value.chars() {
        let mut next = kept.clone();
        next.push(character);
        if encode_text(&next, encoding, false)?.len() > limit {
            break;
        }
        kept = next;
    }
    Ok((kept, true))
}

fn note_id3v1_truncation(
    request: &AudioTagPlanRequest,
    file: &mut AudioTagFile,
    updates: &mut HashMap<String, Vec<String>>,
    warnings: &mut Vec<String>,
) -> Result<(), CoreError> {
    let comment_limit = id3v1_comment_limit(file);
    let encoding = request.id3v1_output_encoding;
    for field in &mut file.fields {
        if field.container != AudioContainer::Id3v1 {
            continue;
        }
        let Some(limit) = id3v1_text_limit(&field.key, comment_limit) else {
            continue;
        };
        let source = field
            .converted_values
            .clone()
            .unwrap_or_else(|| field.values.clone());
        let mut fitted = Vec::with_capacity(source.len());
        let mut notice = None;
        let mut field_warnings = Vec::new();
        for value in &source {
            let fit = fit_encoded_text(value, limit, encoding)?;
            if fit.truncated && notice.is_none() {
                notice = Some(format!(
                    "「ID3v1 {}超過 {limit} 位元組，已截斷為 {}」",
                    field.label, fit.text
                ));
                field_warnings.push(AudioFieldWarning {
                    code: AudioFieldWarningCode::Truncated,
                    message: format!("截斷 {}→{} B", fit.from_bytes, fit.to_bytes),
                    from_bytes: Some(fit.from_bytes),
                    to_bytes: Some(fit.to_bytes),
                    count: None,
                });
            }
            if fit.unrepresentable > 0 {
                field_warnings.push(AudioFieldWarning {
                    code: AudioFieldWarningCode::Unrepresentable,
                    message: format!("{} 字無法表示", fit.unrepresentable),
                    from_bytes: None,
                    to_bytes: None,
                    count: Some(fit.unrepresentable),
                });
            }
            fitted.push(fit.text);
        }
        if field_warnings.is_empty() {
            continue;
        }
        field.warnings = field_warnings;
        field.truncated = notice.is_some();
        field.truncation = notice.clone();
        field.converted_values = Some(fitted.clone());
        if let Some(message) = notice {
            warnings.push(format!("{}：{message}", file.path));
        }
        if field.selected {
            updates.insert(field_id(field.container, &field.key), fitted);
        }
    }
    Ok(())
}

fn label_for(key: &str) -> &str {
    match key {
        "title" => "標題",
        "artist" => "演出者",
        "album" => "專輯",
        "albumArtist" => "專輯演出者",
        "comment" => "註解",
        "genre" => "類型",
        "composer" => "作曲者",
        "lyrics" => "歌詞",
        "year" => "年份",
        "track" => "音軌",
        other => other,
    }
}

fn file_should_write(file: &PreparedAudio) -> bool {
    !file.updates.is_empty()
        || !file.selected_containers.is_empty()
        || !file.remove_containers.is_empty()
}

fn fields_for_path(selection: &HashMap<String, Vec<String>>, path: &Path) -> HashSet<String> {
    let owned = path.to_string_lossy().into_owned();
    if let Some(fields) = selection.get(&owned) {
        return fields.iter().cloned().collect();
    }
    let resolved = resolve_path(&owned);
    selection
        .iter()
        .find(|(key, _)| resolve_path(key) == resolved)
        .map(|(_, fields)| fields.iter().cloned().collect())
        .unwrap_or_default()
}

/// 只留下這次勾選的欄位。沒被勾的文字變更不會寫入。
fn restrict_to_fields(file: &mut PreparedAudio, allowed: &HashSet<String>) {
    file.remove_containers
        .retain(|container| allowed.contains(&operations::removal_field_id(*container)));
    file.updates
        .retain(|identifier, _| allowed.contains(identifier));
    let mut containers = HashSet::new();
    for identifier in allowed {
        if identifier.starts_with("remove:") {
            continue;
        }
        let (container, _) = split_field_id(identifier);
        let has_text = file
            .updates
            .keys()
            .any(|key| split_field_id(key).0 == container);
        if has_text || file.rewrite_containers.contains(&container) {
            containers.insert(container);
        }
    }
    file.selected_containers = containers;
}

fn rewrite_containers(request: &AudioTagPlanRequest, file: &AudioTagFile) -> Vec<AudioContainer> {
    let mut containers = Vec::new();
    let selected = |container: AudioContainer| {
        file.fields
            .iter()
            .any(|field| field.selected && field.container == container)
    };
    if selected(AudioContainer::Id3v2) && id3v2_needs_rewrite(Path::new(&file.path), request) {
        containers.push(AudioContainer::Id3v2);
    }
    if selected(AudioContainer::Id3v1) && id3v1_needs_rewrite(request) {
        containers.push(AudioContainer::Id3v1);
    }
    containers
}

fn id3v1_needs_rewrite(request: &AudioTagPlanRequest) -> bool {
    let source = request.id3v1_source_encoding.unwrap_or(TextEncoding::Gbk);
    request.id3v1_output_encoding != source
}

fn id3_encoding_byte(encoding: &str) -> u8 {
    match encoding {
        "latin1" => 0,
        "utf16" | "utf-16" => 1,
        "utf16be" | "utf-16be" => 2,
        _ => 3,
    }
}

/// 已選的 ID3v2 標頭版本或文字框編碼與要求不同。
/// 壓縮、實驗性或長度不完整的標籤回傳 false，避免依錯誤偏移重寫。
fn id3v2_needs_rewrite(path: &Path, request: &AudioTagPlanRequest) -> bool {
    let buffer = match read_id3v2_declared_tag(path) {
        Ok(buffer) => buffer,
        Err(_) => return false,
    };
    let wanted = id3_encoding_byte(&request.id3v2_encoding);
    match classify_id3v2(&buffer) {
        Id3v2Class::Missing => true,
        Id3v2Class::Unsafe(reason) => {
            // 理由留給測試與呼叫端；這裡只略過重寫，不改檔案。
            let _ = reason;
            false
        }
        Id3v2Class::Frames { version, encodings } => {
            version != request.id3v2_version
                || encodings.into_iter().any(|encoding| encoding != wanted)
        }
    }
}

const ID3_TAG_READ_LIMIT: usize = 16 * 1024 * 1024;

#[derive(Debug)]
enum Id3v2Class {
    Missing,
    Frames {
        version: u8,
        encodings: Vec<u8>,
    },
    /// 無法安全解析。呼叫端必須略過編碼／版本重寫。
    Unsafe(&'static str),
}

fn decode_synchsafe(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 4 || bytes.iter().any(|byte| byte & 0x80 != 0) {
        return None;
    }
    Some(
        ((bytes[0] as usize) << 21)
            | ((bytes[1] as usize) << 14)
            | ((bytes[2] as usize) << 7)
            | (bytes[3] as usize),
    )
}

/// 只讀標頭宣告的標籤位元組，不把後面的音訊讀進記憶體。
fn read_id3v2_declared_tag(path: &Path) -> Result<Vec<u8>, &'static str> {
    let mut file = fs::File::open(path).map_err(|_| "無法開啟音訊檔，略過編碼重寫。")?;
    let mut header = [0u8; 10];
    let read = file
        .read(&mut header)
        .map_err(|_| "無法讀取 ID3 標頭，略過編碼重寫。")?;
    if read < 10 || &header[..3] != b"ID3" {
        return Ok(header[..read].to_vec());
    }
    let Some(size) = decode_synchsafe(&header[6..10]) else {
        return Err("ID3 大小不是同步安全整數，略過編碼重寫。");
    };
    if size > ID3_TAG_READ_LIMIT {
        return Err("ID3 標籤超過 16MB，略過編碼重寫。");
    }
    let mut body = vec![0u8; size];
    file.read_exact(&mut body)
        .map_err(|_| "ID3 標籤比標頭宣告的還短，略過編碼重寫。")?;
    let mut buffer = Vec::with_capacity(10 + size);
    buffer.extend_from_slice(&header);
    buffer.extend_from_slice(&body);
    Ok(buffer)
}

fn deunsync(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(data.len());
    let mut index = 0;
    while index < data.len() {
        output.push(data[index]);
        if data[index] == 0xff && index + 1 < data.len() && data[index + 1] == 0 {
            index += 2;
        } else {
            index += 1;
        }
    }
    output
}

fn skip_extended_header(version: u8, body: &[u8]) -> Result<usize, &'static str> {
    if body.len() < 4 {
        return Err("延伸標頭太短，略過編碼重寫。");
    }
    if version == 4 {
        let Some(size) = decode_synchsafe(&body[..4]) else {
            return Err("ID3v2.4 延伸標頭大小無效，略過編碼重寫。");
        };
        if size < 6 || size > body.len() {
            return Err("ID3v2.4 延伸標頭長度不合理，略過編碼重寫。");
        }
        return Ok(size);
    }
    let size = u32::from_be_bytes([body[0], body[1], body[2], body[3]]) as usize;
    let Some(total) = size.checked_add(4) else {
        return Err("ID3v2.3 延伸標頭長度溢出，略過編碼重寫。");
    };
    if size < 6 || total > body.len() {
        return Err("ID3v2.3 延伸標頭長度不合理，略過編碼重寫。");
    }
    Ok(total)
}

fn text_frame(id: &str) -> bool {
    id.starts_with('T') || matches!(id, "COMM" | "USLT" | "USER" | "WXXX" | "COM")
}

fn classify_id3v2(buffer: &[u8]) -> Id3v2Class {
    if buffer.len() < 10 || &buffer[..3] != b"ID3" {
        return Id3v2Class::Missing;
    }
    let version = buffer[3];
    let flags = buffer[5];
    let Some(declared) = decode_synchsafe(&buffer[6..10]) else {
        return Id3v2Class::Unsafe("ID3 大小不是同步安全整數，略過編碼重寫。");
    };
    if buffer.len() < 10 + declared {
        return Id3v2Class::Unsafe("ID3 標籤比標頭宣告的還短，略過編碼重寫。");
    }
    let body = &buffer[10..10 + declared];
    if version == 2 {
        if flags & 0x40 != 0 {
            return Id3v2Class::Unsafe("ID3v2.2 壓縮標籤無法安全判斷文字編碼，略過重寫。");
        }
        if flags & !0xc0 != 0 {
            return Id3v2Class::Unsafe("ID3v2.2 有未知旗標，略過重寫。");
        }
        return parse_v22_frames(body, flags & 0x80 != 0);
    }
    if version != 3 && version != 4 {
        return Id3v2Class::Unsafe("不支援的 ID3v2 版本，略過重寫。");
    }
    if flags & 0x20 != 0 {
        return Id3v2Class::Unsafe("實驗性 ID3v2 標籤，略過重寫。");
    }
    let known = if version == 4 { 0xf0 } else { 0xe0 };
    if flags & !known != 0 {
        return Id3v2Class::Unsafe("ID3v2 有未知旗標，略過重寫。");
    }
    let mut offset = 0usize;
    if flags & 0x40 != 0 {
        match skip_extended_header(version, body) {
            Ok(next) => offset = next,
            Err(reason) => return Id3v2Class::Unsafe(reason),
        }
    }
    parse_v23_frames(version, &body[offset..], flags & 0x80 != 0)
}

fn parse_v22_frames(body: &[u8], unsync: bool) -> Id3v2Class {
    let mut offset = 0usize;
    let mut encodings = Vec::new();
    while offset + 6 <= body.len() {
        let id_bytes = &body[offset..offset + 3];
        if id_bytes == [0, 0, 0] {
            break;
        }
        if !id_bytes
            .iter()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        {
            return Id3v2Class::Unsafe("ID3v2.2 幀識別碼無效，略過重寫。");
        }
        let frame_size = ((body[offset + 3] as usize) << 16)
            | ((body[offset + 4] as usize) << 8)
            | (body[offset + 5] as usize);
        let Some(end) = offset.checked_add(6 + frame_size) else {
            return Id3v2Class::Unsafe("ID3v2.2 幀長度溢出，略過重寫。");
        };
        if end > body.len() {
            return Id3v2Class::Unsafe("ID3v2.2 幀超出標籤，略過重寫。");
        }
        let id = std::str::from_utf8(id_bytes).unwrap_or("");
        if text_frame(id) && frame_size > 0 {
            let raw = &body[offset + 6..end];
            let payload = if unsync { deunsync(raw) } else { raw.to_vec() };
            if let Some(encoding) = payload.first().copied() {
                encodings.push(encoding);
            }
        }
        offset = end;
    }
    Id3v2Class::Frames {
        version: 2,
        encodings,
    }
}

fn parse_v23_frames(version: u8, body: &[u8], tag_unsync: bool) -> Id3v2Class {
    let mut offset = 0usize;
    let mut encodings = Vec::new();
    while offset + 10 <= body.len() {
        let id_bytes = &body[offset..offset + 4];
        if id_bytes == [0, 0, 0, 0] {
            break;
        }
        if !id_bytes
            .iter()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        {
            return Id3v2Class::Unsafe("ID3v2 幀識別碼無效，略過重寫。");
        }
        let frame_size = if version == 4 {
            let Some(size) = decode_synchsafe(&body[offset + 4..offset + 8]) else {
                return Id3v2Class::Unsafe("ID3v2.4 幀大小無效，略過重寫。");
            };
            size
        } else {
            u32::from_be_bytes([
                body[offset + 4],
                body[offset + 5],
                body[offset + 6],
                body[offset + 7],
            ]) as usize
        };
        let Some(end) = offset.checked_add(10 + frame_size) else {
            return Id3v2Class::Unsafe("ID3v2 幀長度溢出，略過重寫。");
        };
        if end > body.len() {
            return Id3v2Class::Unsafe("ID3v2 幀超出標籤，略過重寫。");
        }
        let flags = [body[offset + 8], body[offset + 9]];
        let id = std::str::from_utf8(id_bytes).unwrap_or("");
        if text_frame(id) && frame_size > 0 {
            if let Some(encoding) =
                frame_text_encoding(version, flags, &body[offset + 10..end], tag_unsync)
            {
                encodings.push(encoding);
            }
        }
        offset = end;
    }
    Id3v2Class::Frames { version, encodings }
}

/// 壓縮或加密的幀不拿來判斷編碼。回傳 None 表示略過這一幀。
fn frame_text_encoding(version: u8, flags: [u8; 2], data: &[u8], tag_unsync: bool) -> Option<u8> {
    let (compressed, encrypted, grouped, unsync, length_indicator) = if version == 4 {
        let format = flags[1];
        (
            format & 0x08 != 0,
            format & 0x04 != 0,
            format & 0x40 != 0,
            format & 0x02 != 0,
            format & 0x01 != 0,
        )
    } else {
        let format = flags[1];
        (
            format & 0x80 != 0,
            format & 0x40 != 0,
            format & 0x20 != 0,
            false,
            false,
        )
    };
    if compressed || encrypted {
        return None;
    }
    let payload = if unsync || tag_unsync {
        deunsync(data)
    } else {
        data.to_vec()
    };
    let mut cursor = 0usize;
    if grouped {
        cursor += 1;
    }
    if length_indicator {
        cursor += 4;
    }
    payload.get(cursor).copied()
}

fn id3v2_text_encodings(buffer: &[u8]) -> Vec<u8> {
    match classify_id3v2(buffer) {
        Id3v2Class::Frames { encodings, .. } => encodings,
        Id3v2Class::Missing | Id3v2Class::Unsafe(_) => Vec::new(),
    }
}

fn field_id(container: AudioContainer, key: &str) -> String {
    let container = match container {
        AudioContainer::Id3v1 => "id3v1",
        AudioContainer::Id3v2 => "id3v2",
        AudioContainer::Apev2 => "apev2",
        AudioContainer::VorbisComment => "vorbis-comment",
    };
    format!("{container}:{key}")
}

fn split_field_id(identifier: &str) -> (AudioContainer, String) {
    let (container, key) = identifier.split_once(':').unwrap_or(("id3v2", identifier));
    let container = match container {
        "id3v1" => AudioContainer::Id3v1,
        "apev2" => AudioContainer::Apev2,
        "vorbis-comment" => AudioContainer::VorbisComment,
        _ => AudioContainer::Id3v2,
    };
    (container, key.to_string())
}

fn format_from_path(path: &Path) -> Result<AudioFormat, CoreError> {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "mp3" => Ok(AudioFormat::Mp3),
        "ape" => Ok(AudioFormat::Ape),
        "opus" => Ok(AudioFormat::Opus),
        "ogg" | "oga" => Ok(AudioFormat::Ogg),
        other => Err(CoreError::new(
            "AUDIO_FORMAT",
            format!(
                "不支援音訊格式 {}。",
                if other.is_empty() { "未知" } else { other }
            ),
        )),
    }
}

struct Id3v1Tag {
    values: HashMap<&'static str, String>,
    genre_code: u8,
}

fn empty_id3v1() -> HashMap<&'static str, String> {
    HashMap::from([
        ("title", String::new()),
        ("artist", String::new()),
        ("album", String::new()),
        ("year", String::new()),
        ("comment", String::new()),
        ("track", String::new()),
        ("genre", String::new()),
    ])
}

fn read_id3v1(buffer: &[u8], encoding: TextEncoding) -> Result<Option<Id3v1Tag>, CoreError> {
    if buffer.len() < 128 || &buffer[buffer.len() - 128..buffer.len() - 125] != b"TAG" {
        return Ok(None);
    }
    let tag = &buffer[buffer.len() - 128..];
    let decode = |start: usize, length: usize| -> Result<String, CoreError> {
        Ok(decode_text(&tag[start..start + length], encoding)?
            .0
            .trim_end_matches(['\0', ' '])
            .to_string())
    };
    let track = if tag[125] == 0 && tag[126] > 0 {
        tag[126].to_string()
    } else {
        String::new()
    };
    Ok(Some(Id3v1Tag {
        values: HashMap::from([
            ("title", decode(3, 30)?),
            ("artist", decode(33, 30)?),
            ("album", decode(63, 30)?),
            ("year", decode(93, 4)?),
            (
                "comment",
                decode(97, if track.is_empty() { 30 } else { 28 })?,
            ),
            ("track", track),
            ("genre", tag[127].to_string()),
        ]),
        genre_code: tag[127],
    }))
}

fn append_id3v1(
    buffer: &[u8],
    values: &HashMap<&str, String>,
    genre_code: u8,
    encoding: TextEncoding,
) -> Result<Vec<u8>, CoreError> {
    let mut tag = vec![0_u8; 128];
    tag[..3].copy_from_slice(b"TAG");
    write_encoded_field(
        &mut tag,
        3,
        30,
        values.get("title").map(String::as_str).unwrap_or(""),
        encoding,
    )?;
    write_encoded_field(
        &mut tag,
        33,
        30,
        values.get("artist").map(String::as_str).unwrap_or(""),
        encoding,
    )?;
    write_encoded_field(
        &mut tag,
        63,
        30,
        values.get("album").map(String::as_str).unwrap_or(""),
        encoding,
    )?;
    write_encoded_field(
        &mut tag,
        93,
        4,
        values.get("year").map(String::as_str).unwrap_or(""),
        encoding,
    )?;
    let track = values
        .get("track")
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(0);
    write_encoded_field(
        &mut tag,
        97,
        if track == 0 { 30 } else { 28 },
        values.get("comment").map(String::as_str).unwrap_or(""),
        encoding,
    )?;
    if track > 0 {
        tag[125] = 0;
        tag[126] = track;
    }
    tag[127] = values
        .get("genre")
        .and_then(|value| value.parse().ok())
        .unwrap_or(genre_code);
    Ok([buffer, tag.as_slice()].concat())
}

fn write_encoded_field(
    target: &mut [u8],
    offset: usize,
    length: usize,
    value: &str,
    encoding: TextEncoding,
) -> Result<(), CoreError> {
    let fitted = fit_encoded_text(value, length, encoding)?.text;
    let encoded = encode_text(&fitted, encoding, false)?;
    let copy = encoded.len().min(length);
    target[offset..offset + copy].copy_from_slice(&encoded[..copy]);
    Ok(())
}

fn strip_id3v2(buffer: &[u8]) -> Vec<u8> {
    let length = id3v2_tag_length(buffer).min(buffer.len());
    buffer[length..].to_vec()
}

/// 只剝檔頭第一個 ID3v2。第二個檔頭標籤、檔尾 ID3v2 或 Lyrics3v2 還在時不能回報成功。
fn reject_leftover_id3_family(buffer: &[u8]) -> Result<(), CoreError> {
    if let Some(reason) = leftover_id3_family(buffer) {
        return Err(CoreError::new(
            "AUDIO_REMOVE",
            format!("移除 ID3v2 後標籤仍在：{reason}"),
        ));
    }
    Ok(())
}

fn leftover_id3_family(buffer: &[u8]) -> Option<&'static str> {
    if declared_id3v2_len(buffer).is_some() {
        return Some("檔頭仍有 ID3v2");
    }
    let body = strip_id3v1(buffer);
    if lyrics3v2_footer(body) {
        return Some("仍有 Lyrics3v2");
    }
    if has_trailing_id3v2(body) {
        return Some("檔尾仍有 ID3v2");
    }
    None
}

fn declared_id3v2_len(buffer: &[u8]) -> Option<usize> {
    if buffer.len() < 10 || &buffer[..3] != b"ID3" || !(2..=4).contains(&buffer[3]) {
        return None;
    }
    if buffer[6..10].iter().any(|byte| byte & 0x80 != 0) {
        return None;
    }
    let length = id3v2_tag_length(buffer);
    (length >= 10 && length <= buffer.len()).then_some(length)
}

fn lyrics3v2_footer(body: &[u8]) -> bool {
    if body.len() < 15 {
        return false;
    }
    let at = body.len() - 15;
    &body[at..at + 9] == b"LYRICS200" && body[at + 9..].iter().all(|byte| byte.is_ascii_digit())
}

fn has_trailing_id3v2(body: &[u8]) -> bool {
    let mut offset = body.len().saturating_sub(8 * 1024 * 1024).max(1);
    while offset + 10 <= body.len() {
        if let Some(length) = declared_id3v2_len(&body[offset..]) {
            if offset + length == body.len() {
                return true;
            }
        }
        offset += 1;
    }
    false
}

/// 去掉結尾的 APEv2（ID3v1 若還在，先視為已剝除）。
/// 標籤 size 含 footer、不含 header。footer flags 的 bit 31 才表示有標頭。
fn strip_ape_tail(body: &[u8]) -> &[u8] {
    if body.len() < 32 || &body[body.len() - 32..body.len() - 24] != b"APETAGEX" {
        return body;
    }
    let footer = &body[body.len() - 32..];
    let size = u32::from_le_bytes(footer[12..16].try_into().unwrap_or([0; 4])) as usize;
    let flags = u32::from_le_bytes(footer[20..24].try_into().unwrap_or([0; 4]));
    let total = if flags & 0x8000_0000 != 0 {
        size.saturating_add(32)
    } else {
        size
    };
    if total < 32 || total > body.len() {
        return body;
    }
    &body[..body.len() - total]
}

/// 寫入後核對音訊用。刻意不呼叫 `strip_ape_tail`，避免同一套邊界算錯時前後比對一起通過。
fn verified_tag_body(buffer: &[u8]) -> &[u8] {
    let mut end = buffer.len();
    if end >= 128 && &buffer[end - 128..end - 125] == b"TAG" {
        end -= 128;
    }
    if end >= 32 && &buffer[end - 32..end - 24] == b"APETAGEX" {
        let footer = &buffer[end - 32..end];
        let size = u32::from_le_bytes(footer[12..16].try_into().unwrap_or([0; 4])) as usize;
        let has_header = footer[23] & 0x80 != 0;
        let total = if has_header {
            size.saturating_add(32)
        } else {
            size
        };
        if total >= 32 && total <= end {
            end -= total;
        }
    }
    let start = id3v2_tag_length(&buffer[..end]);
    let rest = &buffer[start..end];
    if rest.len() >= 32 && &rest[..8] == b"APETAGEX" {
        let size = u32::from_le_bytes(rest[12..16].try_into().unwrap_or([0; 4])) as usize;
        let has_header = rest[23] & 0x80 != 0;
        let total = if has_header {
            size.saturating_add(32)
        } else {
            size
        };
        if total >= 32 && total <= rest.len() {
            return &rest[total..];
        }
    }
    rest
}

fn verified_mp3_audio(buffer: &[u8]) -> Vec<u8> {
    let span = verified_tag_body(buffer);
    let mut start = 0;
    while start + 1 < span.len() && !(span[start] == 0xff && span[start + 1] >= 0xf0) {
        start += 1;
    }
    span[start..].to_vec()
}

fn verified_ape_audio(buffer: &[u8]) -> Vec<u8> {
    verified_tag_body(buffer).to_vec()
}

fn strip_trailing_ape(buffer: &[u8]) -> Vec<u8> {
    let has_v1 = buffer.len() >= 128 && &buffer[buffer.len() - 128..buffer.len() - 125] == b"TAG";
    if has_v1 {
        let (body, v1) = buffer.split_at(buffer.len() - 128);
        [strip_ape_tail(body), v1].concat()
    } else {
        strip_ape_tail(buffer).to_vec()
    }
}

fn strip_id3v1(buffer: &[u8]) -> &[u8] {
    if buffer.len() >= 128 && &buffer[buffer.len() - 128..buffer.len() - 125] == b"TAG" {
        &buffer[..buffer.len() - 128]
    } else {
        buffer
    }
}

fn picture_count(path: &Path) -> Result<usize, CoreError> {
    if format_from_path(path)? == AudioFormat::Mp3 {
        let tag = id3::no_tag_ok(id3::Tag::read_from_path(path))
            .map_err(|error| CoreError::new("AUDIO_PICTURE", error.to_string()))?;
        return Ok(tag
            .map(|tag| tag.frames().filter(|frame| frame.id() == "APIC").count())
            .unwrap_or(0));
    }
    let tagged = read_tagged(path)?;
    Ok(tagged
        .primary_tag()
        .map(|tag| tag.pictures().len())
        .unwrap_or(0))
}

fn lofty_file_type(path: &Path) -> Option<FileType> {
    match format_from_path(path).ok()? {
        AudioFormat::Mp3 => Some(FileType::Mpeg),
        AudioFormat::Ape => Some(FileType::Ape),
        AudioFormat::Ogg => Some(FileType::Vorbis),
        AudioFormat::Opus => Some(FileType::Opus),
    }
}

fn read_tagged(path: &Path) -> Result<TaggedFile, CoreError> {
    let probe = Probe::open(path)
        .map_err(|error| CoreError::new("AUDIO_INVALID", format!("音訊檔案無法解析。{error}")))?;
    let probe = if probe.file_type().is_none() {
        if let Some(file_type) = lofty_file_type(path) {
            probe.set_file_type(file_type)
        } else {
            probe.guess_file_type().map_err(|error| {
                CoreError::new("AUDIO_INVALID", format!("音訊檔案無法解析。{error}"))
            })?
        }
    } else {
        probe
    };
    probe
        .read()
        .map_err(|error| CoreError::new("AUDIO_INVALID", format!("音訊檔案無法解析。{error}")))
}

fn finish_temporary(path: &Path, temporary: &Path) -> Result<(), CoreError> {
    if let Err(error) = commit_temporary(path, temporary) {
        let _ = fs::remove_file(temporary);
        return Err(error);
    }
    Ok(())
}

fn commit_temporary(path: &Path, temporary: &Path) -> Result<(), CoreError> {
    // 先把暫存內容與目錄項刷上磁碟，再改名，避免當機後只看到空檔或舊目錄項。
    {
        let file = fs::File::open(temporary)?;
        file.sync_all()?;
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        let directory = fs::File::open(parent)?;
        directory.sync_all()?;
    }
    let backup = path.with_file_name(format!(
        ".convertzz-audio-backup-{}{}",
        Uuid::new_v4(),
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{value}"))
            .unwrap_or_default()
    ));
    fs::rename(path, &backup)?;
    if let Err(error) = fs::rename(temporary, path) {
        let _ = fs::rename(&backup, path);
        return Err(error.into());
    }
    let _ = fs::remove_file(&backup);
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    path.with_file_name(format!(
        ".convertzz-audio-{}{}",
        Uuid::new_v4(),
        path.extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{value}"))
            .unwrap_or_default()
    ))
}

fn expand_audio_paths(input_paths: &[String], recursive: bool) -> Result<Vec<PathBuf>, CoreError> {
    let mut paths = HashSet::new();
    for input in input_paths {
        let path = resolve_path(input);
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_file() {
            format_from_path(&path)?;
            paths.insert(path);
        } else if metadata.is_dir() {
            collect_audio_files(&path, recursive, &mut paths)?;
        }
    }
    let mut list = paths.into_iter().collect::<Vec<_>>();
    list.sort();
    Ok(list)
}

fn collect_audio_files(
    directory: &Path,
    recursive: bool,
    paths: &mut HashSet<PathBuf>,
) -> Result<(), CoreError> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_symlink() {
            continue;
        }
        let path = entry.path();
        if entry.file_type()?.is_file() && format_from_path(&path).is_ok() {
            paths.insert(path);
        } else if entry.file_type()?.is_dir() && recursive {
            collect_audio_files(&path, true, paths)?;
        }
    }
    Ok(())
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string()
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

use super::backup::{create_user_backups, resolve_backup_roots, resolve_path, BackupRoot};
use super::conversion::ConversionService;
use super::encoding::{decode_text, encode_text};
use super::error::CoreError;
use super::types::{
    ApplyFailure, ApplyResult, AudioContainer, AudioFormat, AudioScanRequest, AudioTagField,
    AudioTagFile, AudioTagPlan, AudioTagPlanRequest, ConversionOptions, ProgressReporter,
    TextEncoding,
};
use chrono::Utc;
use lofty::config::WriteOptions;
use lofty::file::{AudioFile, FileType, TaggedFile, TaggedFileExt};
use lofty::picture::PictureType;
use lofty::probe::Probe;
use lofty::tag::items::Timestamp;
use lofty::tag::{Accessor, ItemKey, ItemValue, Tag, TagItem};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

const STANDARD_FIELDS: &[&str] = &[
    "title", "artist", "album", "year", "track", "comment", "genre",
];

struct PreparedAudio {
    path: PathBuf,
    format: AudioFormat,
    updates: HashMap<String, Vec<String>>,
    selected_containers: HashSet<AudioContainer>,
    request: AudioTagPlanRequest,
    original_picture_count: usize,
}

struct StoredAudioPlan {
    files: Vec<PreparedAudio>,
    backup: bool,
    backup_roots: Vec<BackupRoot>,
}

pub struct AudioService {
    plans: Mutex<HashMap<String, StoredAudioPlan>>,
    cancelled: Mutex<HashSet<String>>,
}

impl AudioService {
    pub fn new() -> Self {
        Self {
            plans: Mutex::new(HashMap::new()),
            cancelled: Mutex::new(HashSet::new()),
        }
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

    pub async fn scan(
        &self,
        _conversion: &ConversionService,
        request: AudioScanRequest,
        progress: ProgressReporter,
    ) -> Result<Vec<AudioTagFile>, CoreError> {
        let paths = expand_audio_paths(&request.paths, request.recursive.unwrap_or(false))?;
        let mut files = Vec::new();
        for (index, path) in paths.iter().enumerate() {
            files.push(match scan_file(path, &request) {
                Ok(file) => file,
                Err(error) => AudioTagFile {
                    path: path.to_string_lossy().into_owned(),
                    format: format_from_path(path).unwrap_or(AudioFormat::Mp3),
                    selected: true,
                    fields: Vec::new(),
                    has_cover_art: false,
                    duration_seconds: None,
                    warning: Some(error.message),
                },
            });
            progress(super::types::ProgressEvent {
                current: (index + 1) as u64,
                total: paths.len() as u64,
                message: format!("正在掃描：{}", file_name(path)),
            });
        }
        Ok(files)
    }

    pub async fn plan(
        &self,
        conversion: &ConversionService,
        request: AudioTagPlanRequest,
        progress: ProgressReporter,
    ) -> Result<AudioTagPlan, CoreError> {
        let mut scanned = self
            .scan(conversion, request.scan_request(), progress.clone())
            .await?;
        let mut prepared = Vec::new();
        let mut warnings = Vec::new();
        let selected_paths = request
            .selected_paths
            .iter()
            .map(|path| resolve_path(path))
            .collect::<HashSet<_>>();

        let total = scanned.len() as u64;
        for (index, file) in scanned.iter_mut().enumerate() {
            file.selected = selected_paths.contains(&resolve_path(&file.path));
            if file.warning.is_some() {
                continue;
            }
            if !file.selected {
                for field in &mut file.fields {
                    field.selected = false;
                }
                progress(super::types::ProgressEvent {
                    current: (index + 1) as u64,
                    total,
                    message: format!("已略過未選檔案：{}", file_name(Path::new(&file.path))),
                });
                continue;
            }
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
                    selected.contains(&identifier) && container_enabled(&request, field.container);
                if !field.selected {
                    continue;
                }
                let mut converted_values = Vec::new();
                for value in &field.values {
                    let result = conversion
                        .convert(
                            conversion_for_container(&request, field.container).with_text(value),
                        )
                        .await?;
                    converted_values.push(result.text);
                    warnings.extend(result.warnings);
                }
                field.converted_values = Some(converted_values.clone());
                updates.insert(identifier, converted_values);
                containers.insert(field.container);
            }
            prepared.push(PreparedAudio {
                path: PathBuf::from(&file.path),
                format: file.format,
                updates,
                selected_containers: containers,
                request: request.clone(),
                original_picture_count: if file.has_cover_art {
                    picture_count(&file.path)?
                } else {
                    0
                },
            });
            progress(super::types::ProgressEvent {
                current: (index + 1) as u64,
                total,
                message: format!("正在建立標籤預覽：{}", file_name(Path::new(&file.path))),
            });
        }

        let plan_id = Uuid::new_v4().to_string();
        let public = AudioTagPlan {
            plan_id: plan_id.clone(),
            created_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
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
                },
            );
        }
        Ok(public)
    }

    pub async fn apply(
        &self,
        plan_id: &str,
        progress: ProgressReporter,
    ) -> Result<ApplyResult, CoreError> {
        let plan = self
            .plans
            .lock()
            .ok()
            .and_then(|mut plans| plans.remove(plan_id))
            .ok_or_else(|| CoreError::new("PLAN_NOT_FOUND", "音訊標籤計畫已失效。請重新預覽。"))?;
        let mut result = ApplyResult {
            succeeded: Vec::new(),
            skipped: Vec::new(),
            failed: Vec::new(),
        };
        let writable = plan
            .files
            .iter()
            .filter(|file| !file.updates.is_empty())
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        if plan.backup && !writable.is_empty() {
            progress(super::types::ProgressEvent {
                current: 0,
                total: plan.files.len().max(1) as u64,
                message: "正在建立備份…".into(),
            });
            if let Err(error) = create_user_backups(&plan.backup_roots, &writable) {
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

        for (index, file) in plan.files.into_iter().enumerate() {
            self.throw_if_cancelled(plan_id)?;
            if file.updates.is_empty() {
                result
                    .skipped
                    .push(file.path.to_string_lossy().into_owned());
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
                current: (index + 1) as u64,
                total: 1.max(index + 1) as u64,
                message: format!("正在寫入標籤：{}", file_name(&file.path)),
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
    let container = if format == AudioFormat::Ape {
        AudioContainer::Apev2
    } else {
        AudioContainer::VorbisComment
    };
    let mut fields = Vec::new();
    if let Some(tag) = tagged.primary_tag() {
        push_standard_fields(&mut fields, container, tag);
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
    Ok(AudioTagFile {
        path: path.to_string_lossy().into_owned(),
        format,
        selected: true,
        fields,
        has_cover_art: tagged.primary_tag().is_some_and(|tag| {
            tag.pictures()
                .iter()
                .any(|picture| picture.pic_type() != PictureType::Other || true)
        }),
        duration_seconds: Some(tagged.properties().duration().as_secs_f64()),
        warning: None,
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
    Ok(AudioTagFile {
        path: path.to_string_lossy().into_owned(),
        format: AudioFormat::Mp3,
        selected: true,
        fields,
        has_cover_art,
        duration_seconds: None,
        warning: None,
    })
}

fn apply_id3v2_encoding(tag: &mut id3::Tag, encoding: &str) {
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
        .map(|frame| frame.set_encoding(Some(encoding)))
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
    let source = fs::read(&file.path)?;
    let temporary = temporary_path(&file.path);
    let writes_v2 = file.selected_containers.contains(&AudioContainer::Id3v2);
    let writes_v1 = file.selected_containers.contains(&AudioContainer::Id3v1);
    fs::copy(&file.path, &temporary)?;
    let mut output = if writes_v2 {
        let mut tag = id3::Tag::read_from_path(&temporary).unwrap_or_else(|_| id3::Tag::new());
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
        apply_id3v2_encoding(&mut tag, &file.request.id3v2_encoding);
        tag.write_to_path(&temporary, version)
            .map_err(|error| CoreError::new("ID3_WRITE", error.to_string()))?;
        fs::read(&temporary)?
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
    } else if existing_v1.is_some() {
        output = [strip_id3v1(&output), &source[source.len() - 128..]].concat();
        fs::write(&temporary, &output)?;
    } else if !writes_v2 {
        fs::write(&temporary, &output)?;
    }

    if mp3_audio_payload(&source) != mp3_audio_payload(&fs::read(&temporary)?) {
        let _ = fs::remove_file(&temporary);
        return Err(CoreError::new(
            "AUDIO_VERIFY",
            "MP3 標籤寫入改變了音訊資料。",
        ));
    }
    commit_temporary(&file.path, &temporary)
}

fn apply_taglib(file: &PreparedAudio) -> Result<(), CoreError> {
    let temporary = temporary_path(&file.path);
    fs::copy(&file.path, &temporary)?;
    let result = (|| -> Result<(), CoreError> {
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
        let verification = read_tagged(&temporary).map_err(|error| {
            CoreError::new(
                "AUDIO_VERIFY",
                format!("標籤寫入後的音訊檔案無法驗證。{error}"),
            )
        })?;
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
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    commit_temporary(&file.path, &temporary)
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
        "year" => tag.year().map(|value| value.to_string()),
        "track" => tag.track().map(|value| value.to_string()),
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
            }
        }
        "track" => {
            if let Ok(track) = value.parse() {
                tag.set_track(track);
            }
        }
        "genre" => tag.set_genre(value),
        "comment" => {
            tag.add_frame(id3::frame::Comment {
                lang: "eng".into(),
                description: String::new(),
                text: value,
            });
        }
        other if other.starts_with("frame:") => {
            let id = other.trim_start_matches("frame:");
            tag.set_text(id, value);
        }
        _ => {}
    }
}

const COMMON_ID3V2_FRAMES: &[&str] = &[
    "TT2", "TIT2", "TP1", "TPE1", "TAL", "TALB", "TYE", "TYER", "TDRC", "TRK", "TRCK", "TCO",
    "TCON",
];

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

fn mp3_audio_payload(buffer: &[u8]) -> Vec<u8> {
    let without_v1 = strip_id3v1(buffer);
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

fn make_field(container: AudioContainer, key: &str, values: Vec<String>) -> AudioTagField {
    AudioTagField {
        key: key.to_string(),
        label: label_for(key).to_string(),
        container,
        values,
        converted_values: None,
        selected: STANDARD_FIELDS.contains(&key),
    }
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
    let encoded = encode_text(value, encoding, false)?;
    let copy = encoded.len().min(length);
    target[offset..offset + copy].copy_from_slice(&encoded[..copy]);
    Ok(())
}

fn strip_id3v1(buffer: &[u8]) -> &[u8] {
    if buffer.len() >= 128 && &buffer[buffer.len() - 128..buffer.len() - 125] == b"TAG" {
        &buffer[..buffer.len() - 128]
    } else {
        buffer
    }
}

fn picture_count(path: &str) -> Result<usize, CoreError> {
    if format_from_path(Path::new(path))? == AudioFormat::Mp3 {
        return Ok(0);
    }
    let tagged = read_tagged(Path::new(path))?;
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

fn commit_temporary(path: &Path, temporary: &Path) -> Result<(), CoreError> {
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
mod tests {
    use super::super::conversion::shared_conversion;
    use super::super::encoding::encode_text;
    use super::super::types::{ConflictPolicy, ConversionOptions, Direction, EngineKind};
    use super::*;
    use id3::{TagLike, Version};
    use std::process::Command;
    use std::sync::Arc;
    use uuid::Uuid;

    fn noop() -> ProgressReporter {
        Arc::new(|_| {})
    }

    fn temp_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!("convertzz-audio-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn ffmpeg_bin() -> Option<String> {
        if let Ok(path) = std::env::var("FFMPEG_BIN") {
            if Path::new(&path).is_file() {
                return Some(path);
            }
        }
        Command::new("ffmpeg")
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .ok()
            .filter(|status| status.success())
            .map(|_| "ffmpeg".into())
    }

    fn generate_audio(path: &Path, codec: &str) {
        let ffmpeg = ffmpeg_bin().expect("測試需要 ffmpeg");
        let status = Command::new(ffmpeg)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=0.25",
                "-c:a",
                codec,
                "-y",
            ])
            .arg(path)
            .status()
            .expect("無法啟動 ffmpeg");
        assert!(status.success(), "ffmpeg 產生 {path:?} 失敗");
    }

    fn audio_fingerprint(path: &Path) -> String {
        let ffmpeg = ffmpeg_bin().expect("測試需要 ffmpeg");
        let output = Command::new(ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-i"])
            .arg(path)
            .args(["-map", "0:a:0", "-f", "framemd5", "-"])
            .output()
            .expect("無法啟動 ffmpeg 計算音訊指紋");
        assert!(
            output.status.success(),
            "ffmpeg framemd5 失敗：{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
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

    fn base_plan(paths: &[PathBuf]) -> AudioTagPlanRequest {
        AudioTagPlanRequest {
            paths: paths
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            recursive: None,
            id3v1_source_encoding: Some(TextEncoding::Gbk),
            id3v2_source_encoding: None,
            id3v2_repair_source_encoding: None,
            selected_paths: paths
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            selected_fields: HashMap::new(),
            conversion: conversion_s2t(),
            conflict_policy: ConflictPolicy::Skip,
            backup: Some(false),
            id3v1_enabled: true,
            id3v1_direction: Direction::S2t,
            id3v1_zhconvert: None,
            id3v1_output_encoding: TextEncoding::Big5,
            id3v2_enabled: true,
            id3v2_direction: Direction::S2t,
            id3v2_zhconvert: None,
            id3v2_version: 4,
            id3v2_encoding: "utf8".into(),
        }
    }

    fn mpeg_frame() -> Vec<u8> {
        vec![0xff, 0xfb, 0x90, 0x64, 0, 0, 0, 0, 0, 0]
    }

    fn write_tagged_mp3(path: &Path, id3v1_title: &str, id3v2_title: &str, with_picture: bool) {
        std::fs::write(path, mpeg_frame()).unwrap();
        let mut tag = id3::Tag::new();
        tag.set_title(id3v2_title);
        if with_picture {
            tag.add_frame(id3::frame::Picture {
                mime_type: "image/png".into(),
                picture_type: id3::frame::PictureType::CoverFront,
                description: "cover".into(),
                data: vec![1, 2, 3, 4],
            });
        }
        tag.write_to_path(path, Version::Id3v24).unwrap();
        let mut output = std::fs::read(path).unwrap();
        output = append_id3v1(
            &strip_id3v1(&output),
            &HashMap::from([("title", id3v1_title.to_string())]),
            255,
            TextEncoding::Gbk,
        )
        .unwrap();
        std::fs::write(path, output).unwrap();
    }

    fn id3v1_only_mp3(title: &str, encoding: TextEncoding) -> Vec<u8> {
        append_id3v1(
            &mpeg_frame(),
            &HashMap::from([("title", title.to_string())]),
            255,
            encoding,
        )
        .unwrap()
    }

    fn field<'a>(
        file: &'a AudioTagFile,
        container: AudioContainer,
        key: &str,
    ) -> Option<&'a AudioTagField> {
        file.fields
            .iter()
            .find(|field| field.container == container && field.key == key)
    }

    fn picture_data(path: &Path) -> Option<Vec<u8>> {
        id3::Tag::read_from_path(path)
            .ok()?
            .pictures()
            .next()
            .map(|picture| picture.data.clone())
    }

    fn frame_encoding(buffer: &[u8], frame_id: &str) -> u8 {
        let version = id3v2_header_version(buffer).expect("ID3v2");
        let size = ((buffer[6] as usize & 0x7f) << 21)
            | ((buffer[7] as usize & 0x7f) << 14)
            | ((buffer[8] as usize & 0x7f) << 7)
            | (buffer[9] as usize & 0x7f);
        let mut offset = 10;
        let end = (10 + size).min(buffer.len());
        while offset + 11 <= end {
            let id = std::str::from_utf8(&buffer[offset..offset + 4]).unwrap_or_default();
            if id == "\0\0\0\0" {
                break;
            }
            let frame_size = if version == 4 {
                ((buffer[offset + 4] as usize & 0x7f) << 21)
                    | ((buffer[offset + 5] as usize & 0x7f) << 14)
                    | ((buffer[offset + 6] as usize & 0x7f) << 7)
                    | (buffer[offset + 7] as usize & 0x7f)
            } else {
                u32::from_be_bytes([
                    buffer[offset + 4],
                    buffer[offset + 5],
                    buffer[offset + 6],
                    buffer[offset + 7],
                ]) as usize
            };
            if id == frame_id && frame_size > 0 {
                return buffer[offset + 10];
            }
            offset += 10 + frame_size;
        }
        panic!("找不到 {frame_id} 文字編碼");
    }

    #[tokio::test]
    async fn damaged_audio_reports_warning() {
        let directory = temp_dir();
        let path = directory.join("truncated.ogg");
        std::fs::write(&path, b"OggS\0truncated").unwrap();
        let service = AudioService::new();
        let files = service
            .scan(
                shared_conversion(),
                AudioScanRequest {
                    paths: vec![path.to_string_lossy().into_owned()],
                    recursive: None,
                    id3v1_source_encoding: None,
                    id3v2_source_encoding: None,
                    id3v2_repair_source_encoding: None,
                },
                noop(),
            )
            .await
            .unwrap();
        assert!(files[0].warning.is_some());
        assert!(files[0].fields.is_empty());
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn writes_file_bak_before_apply() {
        let directory = temp_dir();
        let path = directory.join("song.mp3");
        write_tagged_mp3(&path, "里面", "里面", false);
        let before = std::fs::read(&path).unwrap();
        let service = AudioService::new();
        let mut request = base_plan(&[path.clone()]);
        request.backup = Some(true);
        request.selected_fields = HashMap::from([(
            path.to_string_lossy().into_owned(),
            vec!["id3v1:title".into()],
        )]);
        let plan = service
            .plan(shared_conversion(), request, noop())
            .await
            .unwrap();
        let result = service.apply(&plan.plan_id, noop()).await.unwrap();
        assert!(result.failed.is_empty());
        assert_eq!(
            std::fs::read(format!("{}.bak", path.display())).unwrap(),
            before
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn folder_backup_copies_entire_directory() {
        let parent = temp_dir();
        let folder = parent.join("music");
        std::fs::create_dir(&folder).unwrap();
        let first = folder.join("a.mp3");
        let second = folder.join("b.mp3");
        write_tagged_mp3(&first, "里面", "里面", false);
        write_tagged_mp3(&second, "开发", "开发", false);
        let first_before = std::fs::read(&first).unwrap();
        let second_before = std::fs::read(&second).unwrap();
        let service = AudioService::new();
        let mut request = base_plan(&[folder.clone()]);
        request.backup = Some(true);
        request.recursive = Some(false);
        request.selected_paths = vec![
            first.to_string_lossy().into_owned(),
            second.to_string_lossy().into_owned(),
        ];
        request.selected_fields = HashMap::from([
            (
                first.to_string_lossy().into_owned(),
                vec!["id3v1:title".into()],
            ),
            (
                second.to_string_lossy().into_owned(),
                vec!["id3v1:title".into()],
            ),
        ]);
        let plan = service
            .plan(shared_conversion(), request, noop())
            .await
            .unwrap();
        let result = service.apply(&plan.plan_id, noop()).await.unwrap();
        assert!(result.failed.is_empty());
        assert_eq!(
            std::fs::read(PathBuf::from(format!("{}.bak", folder.display())).join("a.mp3"))
                .unwrap(),
            first_before
        );
        assert_eq!(
            std::fs::read(PathBuf::from(format!("{}.bak", folder.display())).join("b.mp3"))
                .unwrap(),
            second_before
        );
        assert!(!PathBuf::from(format!("{}.bak", first.display())).exists());
        let _ = std::fs::remove_dir_all(&parent);
    }

    #[tokio::test]
    async fn recursive_scan_expands_folders() {
        let directory = temp_dir();
        let nested = directory.join("nested");
        std::fs::create_dir(&nested).unwrap();
        write_tagged_mp3(&directory.join("top.mp3"), "里面", "裡面", false);
        write_tagged_mp3(&nested.join("child.mp3"), "里面", "裡面", false);
        std::fs::write(nested.join("ignored.txt"), "not audio").unwrap();
        let service = AudioService::new();
        let shallow = service
            .scan(
                shared_conversion(),
                AudioScanRequest {
                    paths: vec![directory.to_string_lossy().into_owned()],
                    recursive: Some(false),
                    id3v1_source_encoding: None,
                    id3v2_source_encoding: None,
                    id3v2_repair_source_encoding: None,
                },
                noop(),
            )
            .await
            .unwrap();
        assert_eq!(
            shallow
                .iter()
                .map(|file| file.path.clone())
                .collect::<Vec<_>>(),
            [directory.join("top.mp3").to_string_lossy().into_owned()]
        );
        let recursive = service
            .scan(
                shared_conversion(),
                AudioScanRequest {
                    paths: vec![directory.to_string_lossy().into_owned()],
                    recursive: Some(true),
                    id3v1_source_encoding: None,
                    id3v2_source_encoding: None,
                    id3v2_repair_source_encoding: None,
                },
                noop(),
            )
            .await
            .unwrap();
        assert_eq!(
            recursive
                .iter()
                .map(|file| file.path.clone())
                .collect::<Vec<_>>(),
            [
                nested.join("child.mp3").to_string_lossy().into_owned(),
                directory.join("top.mp3").to_string_lossy().into_owned()
            ]
        );
        assert!(recursive.iter().all(|file| file.selected));
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn only_checked_files_are_written() {
        let directory = temp_dir();
        let selected = directory.join("selected.mp3");
        let skipped = directory.join("skipped.mp3");
        write_tagged_mp3(&selected, "里面", "裡面", false);
        write_tagged_mp3(&skipped, "里面", "裡面", false);
        let skipped_before = std::fs::read(&skipped).unwrap();
        let service = AudioService::new();
        let scanned = service
            .scan(
                shared_conversion(),
                AudioScanRequest {
                    paths: vec![directory.to_string_lossy().into_owned()],
                    recursive: Some(false),
                    id3v1_source_encoding: None,
                    id3v2_source_encoding: None,
                    id3v2_repair_source_encoding: None,
                },
                noop(),
            )
            .await
            .unwrap();
        let selected_title = field(
            scanned
                .iter()
                .find(|file| file.path == selected.to_string_lossy())
                .unwrap(),
            AudioContainer::Id3v1,
            "title",
        )
        .unwrap();
        let mut request = base_plan(&[directory.clone()]);
        request.recursive = Some(false);
        request.selected_paths = vec![selected.to_string_lossy().into_owned()];
        request.selected_fields = HashMap::from([
            (
                selected.to_string_lossy().into_owned(),
                vec![format!("id3v1:{}", selected_title.key)],
            ),
            (
                skipped.to_string_lossy().into_owned(),
                vec!["id3v1:title".into()],
            ),
        ]);
        let plan = service
            .plan(shared_conversion(), request, noop())
            .await
            .unwrap();
        assert_eq!(
            plan.files
                .iter()
                .find(|file| file.path == selected.to_string_lossy())
                .unwrap()
                .selected,
            true
        );
        assert_eq!(
            plan.files
                .iter()
                .find(|file| file.path == skipped.to_string_lossy())
                .unwrap()
                .selected,
            false
        );
        let result = service.apply(&plan.plan_id, noop()).await.unwrap();
        assert_eq!(result.succeeded, [selected.to_string_lossy().into_owned()]);
        assert_eq!(std::fs::read(&skipped).unwrap(), skipped_before);
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn applies_id3v1_and_id3v2_directions_separately() {
        let directory = temp_dir();
        let path = directory.join("directions.mp3");
        write_tagged_mp3(&path, "里面", "裡面", true);
        let before_audio = mp3_audio_payload(&std::fs::read(&path).unwrap());
        let before_picture = picture_data(&path);
        let service = AudioService::new();
        let mut request = base_plan(&[path.clone()]);
        request.selected_fields = HashMap::from([(
            path.to_string_lossy().into_owned(),
            vec!["id3v1:title".into(), "id3v2:title".into()],
        )]);
        request.id3v1_direction = Direction::S2t;
        request.id3v2_direction = Direction::T2s;
        let plan = service
            .plan(shared_conversion(), request, noop())
            .await
            .unwrap();
        assert_eq!(
            field(&plan.files[0], AudioContainer::Id3v1, "title")
                .and_then(|field| field.converted_values.clone()),
            Some(vec!["裡面".into()])
        );
        assert_eq!(
            field(&plan.files[0], AudioContainer::Id3v2, "title")
                .and_then(|field| field.converted_values.clone()),
            Some(vec!["里面".into()])
        );
        let result = service.apply(&plan.plan_id, noop()).await.unwrap();
        assert!(result.failed.is_empty());
        let verified = service
            .scan(
                shared_conversion(),
                AudioScanRequest {
                    paths: vec![path.to_string_lossy().into_owned()],
                    recursive: None,
                    id3v1_source_encoding: Some(TextEncoding::Big5),
                    id3v2_source_encoding: None,
                    id3v2_repair_source_encoding: None,
                },
                noop(),
            )
            .await
            .unwrap();
        assert_eq!(
            field(&verified[0], AudioContainer::Id3v1, "title").map(|field| field.values.clone()),
            Some(vec!["裡面".into()])
        );
        assert_eq!(
            field(&verified[0], AudioContainer::Id3v2, "title").map(|field| field.values.clone()),
            Some(vec!["里面".into()])
        );
        assert_eq!(
            mp3_audio_payload(&std::fs::read(&path).unwrap()),
            before_audio
        );
        assert_eq!(picture_data(&path), before_picture);
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn disabled_id3_container_is_not_converted() {
        let directory = temp_dir();
        let path = directory.join("disabled.mp3");
        write_tagged_mp3(&path, "里面", "里面", false);
        let service = AudioService::new();
        let mut request = base_plan(&[path.clone()]);
        request.selected_fields = HashMap::from([(
            path.to_string_lossy().into_owned(),
            vec!["id3v1:title".into(), "id3v2:title".into()],
        )]);
        request.id3v2_enabled = false;
        let plan = service
            .plan(shared_conversion(), request, noop())
            .await
            .unwrap();
        assert_eq!(
            field(&plan.files[0], AudioContainer::Id3v2, "title")
                .unwrap()
                .selected,
            false
        );
        service.apply(&plan.plan_id, noop()).await.unwrap();
        let verified = service
            .scan(
                shared_conversion(),
                AudioScanRequest {
                    paths: vec![path.to_string_lossy().into_owned()],
                    recursive: None,
                    id3v1_source_encoding: Some(TextEncoding::Big5),
                    id3v2_source_encoding: None,
                    id3v2_repair_source_encoding: None,
                },
                noop(),
            )
            .await
            .unwrap();
        assert_eq!(
            field(&verified[0], AudioContainer::Id3v1, "title").map(|field| field.values.clone()),
            Some(vec!["裡面".into()])
        );
        assert_eq!(
            field(&verified[0], AudioContainer::Id3v2, "title").map(|field| field.values.clone()),
            Some(vec!["里面".into()])
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn repairs_id3v2_mojibake() {
        let directory = temp_dir();
        let path = directory.join("mojibake.mp3");
        let encoded = encode_text("裡面", TextEncoding::Big5, false).unwrap();
        let mojibake: String = encoded.iter().map(|byte| *byte as char).collect();
        write_tagged_mp3(&path, "里面", &mojibake, false);
        let service = AudioService::new();
        let raw = service
            .scan(
                shared_conversion(),
                AudioScanRequest {
                    paths: vec![path.to_string_lossy().into_owned()],
                    recursive: None,
                    id3v1_source_encoding: None,
                    id3v2_source_encoding: Some(TextEncoding::Big5),
                    id3v2_repair_source_encoding: Some(false),
                },
                noop(),
            )
            .await
            .unwrap();
        assert_eq!(
            field(&raw[0], AudioContainer::Id3v2, "title").map(|field| field.values.clone()),
            Some(vec![mojibake.clone()])
        );
        let repaired = service
            .scan(
                shared_conversion(),
                AudioScanRequest {
                    paths: vec![path.to_string_lossy().into_owned()],
                    recursive: None,
                    id3v1_source_encoding: None,
                    id3v2_source_encoding: Some(TextEncoding::Big5),
                    id3v2_repair_source_encoding: Some(true),
                },
                noop(),
            )
            .await
            .unwrap();
        assert_eq!(
            field(&repaired[0], AudioContainer::Id3v2, "title").map(|field| field.values.clone()),
            Some(vec!["裡面".into()])
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn reads_id3v1_with_big5_and_gbk() {
        let directory = temp_dir();
        let service = AudioService::new();
        for encoding in [TextEncoding::Big5, TextEncoding::Gbk] {
            let path = directory.join(format!("{encoding:?}.mp3"));
            std::fs::write(&path, id3v1_only_mp3("裡面", encoding)).unwrap();
            let scanned = service
                .scan(
                    shared_conversion(),
                    AudioScanRequest {
                        paths: vec![path.to_string_lossy().into_owned()],
                        recursive: None,
                        id3v1_source_encoding: Some(encoding),
                        id3v2_source_encoding: None,
                        id3v2_repair_source_encoding: None,
                    },
                    noop(),
                )
                .await
                .unwrap();
            assert_eq!(
                field(&scanned[0], AudioContainer::Id3v1, "title")
                    .map(|field| field.values.clone()),
                Some(vec!["裡面".into()])
            );
        }
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn writes_requested_id3v2_version_and_encoding() {
        let directory = temp_dir();
        let service = AudioService::new();
        for (version, encoding, encoding_byte) in
            [(3_u8, "utf16", 1_u8), (4, "utf8", 3), (4, "utf16", 1)]
        {
            let path = directory.join(format!("v{version}-{encoding}.mp3"));
            write_tagged_mp3(&path, "里面", "里面", false);
            assert_eq!(
                id3v2_header_version(&std::fs::read(&path).unwrap()),
                Some(4)
            );
            let mut request = base_plan(&[path.clone()]);
            request.selected_fields = HashMap::from([(
                path.to_string_lossy().into_owned(),
                vec!["id3v2:title".into()],
            )]);
            request.id3v2_version = version;
            request.id3v2_encoding = encoding.into();
            let plan = service
                .plan(shared_conversion(), request, noop())
                .await
                .unwrap();
            let result = service.apply(&plan.plan_id, noop()).await.unwrap();
            assert!(result.failed.is_empty(), "{result:?}");
            let written = std::fs::read(&path).unwrap();
            assert_eq!(id3v2_header_version(&written), Some(version));
            assert_eq!(frame_encoding(&written, "TIT2"), encoding_byte);
            let verified = service
                .scan(
                    shared_conversion(),
                    AudioScanRequest {
                        paths: vec![path.to_string_lossy().into_owned()],
                        recursive: None,
                        id3v1_source_encoding: None,
                        id3v2_source_encoding: None,
                        id3v2_repair_source_encoding: None,
                    },
                    noop(),
                )
                .await
                .unwrap();
            assert_eq!(
                field(&verified[0], AudioContainer::Id3v2, "title")
                    .map(|field| field.values.clone()),
                Some(vec!["裡面".into()])
            );
        }
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn converts_ape_ogg_oga_and_opus_without_touching_unselected_fields() {
        let ape = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/mac-399.ape");
        let ogg = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/test.ogg");
        if !ape.is_file() || !ogg.is_file() {
            return;
        }
        let directory = temp_dir();
        let oga_source = directory.join("fixture.oga");
        std::fs::copy(&ogg, &oga_source).unwrap();
        let mut samples = vec![
            (ape, "sample.ape".to_string(), AudioContainer::Apev2),
            (ogg, "sample.ogg".to_string(), AudioContainer::VorbisComment),
            (
                oga_source,
                "sample.oga".to_string(),
                AudioContainer::VorbisComment,
            ),
        ];
        if ffmpeg_bin().is_some() {
            let opus = directory.join("fixture.opus");
            generate_audio(&opus, "libopus");
            samples.push((
                opus,
                "sample.opus".to_string(),
                AudioContainer::VorbisComment,
            ));
        }
        for (source, name, container) in samples {
            let path = directory.join(name);
            std::fs::copy(&source, &path).unwrap();
            let mut tagged = read_tagged(&path).unwrap();
            if tagged.primary_tag().is_none() {
                tagged.insert_tag(lofty::tag::Tag::new(tagged.primary_tag_type()));
            }
            if let Some(tag) = tagged.primary_tag_mut() {
                tag.set_title("里面开发".to_string());
                tag.set_artist("头发".to_string());
                tag.set_album("未选择里面".to_string());
            }
            tagged.save_to_path(&path, WriteOptions::default()).unwrap();
            let before = read_tagged(&path).unwrap();
            let before_artist = before
                .primary_tag()
                .and_then(|tag| tag.artist())
                .map(|value| value.to_string());
            let before_album = before
                .primary_tag()
                .and_then(|tag| tag.album())
                .map(|value| value.to_string());
            let before_pictures = before
                .primary_tag()
                .map(|tag| tag.pictures().len())
                .unwrap_or(0);
            let before_audio = ffmpeg_bin().map(|_| audio_fingerprint(&path));
            let service = AudioService::new();
            let scanned = service
                .scan(
                    shared_conversion(),
                    AudioScanRequest {
                        paths: vec![path.to_string_lossy().into_owned()],
                        recursive: None,
                        id3v1_source_encoding: Some(TextEncoding::Gbk),
                        id3v2_source_encoding: None,
                        id3v2_repair_source_encoding: None,
                    },
                    noop(),
                )
                .await
                .unwrap();
            assert!(scanned[0].warning.is_none(), "{:?}", scanned[0].warning);
            let title = scanned[0]
                .fields
                .iter()
                .find(|field| {
                    field.container == container && field.key.eq_ignore_ascii_case("title")
                })
                .expect("title");
            let mut request = base_plan(&[path.clone()]);
            request.selected_fields = HashMap::from([(
                path.to_string_lossy().into_owned(),
                vec![format!(
                    "{}:{}",
                    match container {
                        AudioContainer::Apev2 => "apev2",
                        _ => "vorbis-comment",
                    },
                    title.key
                )],
            )]);
            let plan = service
                .plan(shared_conversion(), request, noop())
                .await
                .unwrap();
            let result = service.apply(&plan.plan_id, noop()).await.unwrap();
            assert!(result.failed.is_empty(), "{result:?}");
            let after = read_tagged(&path).unwrap();
            assert_eq!(
                after.primary_tag().and_then(|tag| tag.title()).as_deref(),
                Some("裡面開發")
            );
            assert_eq!(
                after
                    .primary_tag()
                    .and_then(|tag| tag.artist())
                    .map(|value| value.to_string()),
                before_artist
            );
            assert_eq!(
                after
                    .primary_tag()
                    .and_then(|tag| tag.album())
                    .map(|value| value.to_string()),
                before_album
            );
            assert_eq!(
                after
                    .primary_tag()
                    .map(|tag| tag.pictures().len())
                    .unwrap_or(0),
                before_pictures
            );
            if let Some(before_audio) = before_audio {
                assert_eq!(audio_fingerprint(&path), before_audio);
            }
        }
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[tokio::test]
    async fn converts_multivalue_fields_value_by_value() {
        let ogg = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/test.ogg");
        if !ogg.is_file() {
            return;
        }
        let directory = temp_dir();
        let path = directory.join("multivalue.ogg");
        std::fs::copy(&ogg, &path).unwrap();
        let mut tagged = read_tagged(&path).unwrap();
        if tagged.primary_tag().is_none() {
            tagged.insert_tag(lofty::tag::Tag::new(tagged.primary_tag_type()));
        }
        if let Some(tag) = tagged.primary_tag_mut() {
            tag.remove_key(ItemKey::TrackArtist);
            let _ = tag.push(TagItem::new(
                ItemKey::TrackArtist,
                ItemValue::Text("头发".into()),
            ));
            let _ = tag.push(TagItem::new(
                ItemKey::TrackArtist,
                ItemValue::Text("皇后".into()),
            ));
        }
        tagged.save_to_path(&path, WriteOptions::default()).unwrap();

        let service = AudioService::new();
        let scanned = service
            .scan(
                shared_conversion(),
                AudioScanRequest {
                    paths: vec![path.to_string_lossy().into_owned()],
                    recursive: None,
                    id3v1_source_encoding: None,
                    id3v2_source_encoding: None,
                    id3v2_repair_source_encoding: None,
                },
                noop(),
            )
            .await
            .unwrap();
        let artist = scanned[0]
            .fields
            .iter()
            .find(|field| field.key.eq_ignore_ascii_case("artist"))
            .expect("artist");
        assert_eq!(artist.values, vec!["头发".to_string(), "皇后".to_string()]);

        let mut request = base_plan(&[path.clone()]);
        request.selected_fields = HashMap::from([(
            path.to_string_lossy().into_owned(),
            vec!["vorbis-comment:artist".into()],
        )]);
        let plan = service
            .plan(shared_conversion(), request, noop())
            .await
            .unwrap();
        let planned_artist = plan.files[0]
            .fields
            .iter()
            .find(|field| field.key.eq_ignore_ascii_case("artist"))
            .expect("planned artist");
        assert_eq!(
            planned_artist.converted_values.as_deref(),
            Some(&["頭髮".to_string(), "皇后".to_string()][..])
        );
        let result = service.apply(&plan.plan_id, noop()).await.unwrap();
        assert!(result.failed.is_empty(), "{result:?}");

        let after = read_tagged(&path).unwrap();
        let after_artists: Vec<String> = after
            .primary_tag()
            .map(|tag| {
                tag.get_strings(ItemKey::TrackArtist)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(after_artists, vec!["頭髮".to_string(), "皇后".to_string()]);
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn format_from_path_recognizes_supported_extensions() {
        assert_eq!(
            format_from_path(Path::new("a.mp3")).unwrap(),
            AudioFormat::Mp3
        );
        assert_eq!(
            format_from_path(Path::new("a.APE")).unwrap(),
            AudioFormat::Ape
        );
        assert_eq!(
            format_from_path(Path::new("a.ogg")).unwrap(),
            AudioFormat::Ogg
        );
        assert_eq!(
            format_from_path(Path::new("a.oga")).unwrap(),
            AudioFormat::Ogg
        );
        assert_eq!(
            format_from_path(Path::new("a.opus")).unwrap(),
            AudioFormat::Opus
        );
        assert!(format_from_path(Path::new("a.flac")).is_err());
    }

    #[tokio::test]
    async fn identifies_formats_by_extension() {
        let ape = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/mac-399.ape");
        let ogg = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/test.ogg");
        let directory = temp_dir();
        let mp3 = directory.join("identify.mp3");
        write_tagged_mp3(&mp3, "里面", "里面", false);
        let mut paths = vec![mp3];
        if ape.is_file() {
            let dest = directory.join("identify.ape");
            std::fs::copy(&ape, &dest).unwrap();
            paths.push(dest);
        }
        if ogg.is_file() {
            let dest = directory.join("identify.ogg");
            std::fs::copy(&ogg, &dest).unwrap();
            paths.push(dest);
            let oga = directory.join("identify.oga");
            std::fs::copy(&ogg, &oga).unwrap();
            paths.push(oga);
        }
        if ffmpeg_bin().is_some() {
            let opus = directory.join("identify.opus");
            generate_audio(&opus, "libopus");
            paths.push(opus);
        }
        let service = AudioService::new();
        let scanned = service
            .scan(
                shared_conversion(),
                AudioScanRequest {
                    paths: paths
                        .iter()
                        .map(|path| path.to_string_lossy().into_owned())
                        .collect(),
                    recursive: None,
                    id3v1_source_encoding: None,
                    id3v2_source_encoding: None,
                    id3v2_repair_source_encoding: None,
                },
                noop(),
            )
            .await
            .unwrap();
        let formats: std::collections::BTreeSet<_> =
            scanned.iter().map(|file| file.format).collect();
        assert!(formats.contains(&AudioFormat::Mp3));
        if ape.is_file() {
            assert!(formats.contains(&AudioFormat::Ape));
        }
        if ogg.is_file() {
            assert!(formats.contains(&AudioFormat::Ogg));
            assert_eq!(
                scanned
                    .iter()
                    .filter(|file| file.path.ends_with(".oga"))
                    .count(),
                1
            );
            assert!(scanned.iter().any(|file| {
                file.path.ends_with(".oga")
                    && file.format == AudioFormat::Ogg
                    && file.warning.is_none()
            }));
        }
        if ffmpeg_bin().is_some() {
            assert!(formats.contains(&AudioFormat::Opus));
            assert!(scanned.iter().any(|file| {
                file.path.ends_with(".opus")
                    && file.format == AudioFormat::Opus
                    && file.warning.is_none()
            }));
        }
        assert!(scanned.iter().all(|file| file.warning.is_none()));
        let _ = std::fs::remove_dir_all(&directory);
    }
}

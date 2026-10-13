use super::super::conversion::shared_conversion;
use super::super::encoding::encode_text;
use super::super::types::{
    AudioContainer, AudioFieldWarningCode, AudioFormat, AudioRescanFileRequest, AudioTagField,
    AudioTagFile, ConflictPolicy, ConversionOptions, Direction, EngineKind, TagAction,
    TagOnConflict, TagOperation,
};
use super::*;
use id3::{TagLike, Version};
use lofty::picture::PictureType;
use std::collections::HashSet;
use std::process::Command;
use std::sync::Arc;
use uuid::Uuid;

fn noop() -> ProgressReporter {
    Arc::new(|_| {})
}

fn never_cancel() -> super::super::types::CancelCheck {
    Arc::new(|| false)
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

fn require_ffmpeg() -> String {
    ffmpeg_bin().expect("測試需要 ffmpeg（CI 與本機皆應安裝）")
}

fn generate_audio(path: &Path, codec: &str) {
    let ffmpeg = require_ffmpeg();
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
    let ffmpeg = require_ffmpeg();
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
        tag_operations: Vec::new(),
    }
}

fn mpeg_frame() -> Vec<u8> {
    vec![0xff, 0xfb, 0x90, 0x64, 0, 0, 0, 0, 0, 0]
}

/// MPEG1 Layer III、256 kbps、44100 Hz、無 padding，幀長 720。
/// 幀尾 128 bytes 剛好以 TAG 開頭時，那是音訊，不是 ID3v1。
#[test]
fn id3v1_marker_inside_mpeg_frame_is_not_cut() {
    let mut frame = vec![0_u8; 720];
    frame[..4].copy_from_slice(&[0xff, 0xfb, 0x90, 0x64]);
    let tag_at = frame.len() - 128;
    frame[tag_at..tag_at + 3].copy_from_slice(b"TAG");
    eprintln!(
        "frame_len={:?} overlap path starts {:02x?}",
        super::super::files::mpeg_frame_length(&frame),
        &frame[..4]
    );
    assert_eq!(
        strip_id3v1(&frame).len(),
        frame.len(),
        "MPEG 幀內的 TAG 不該被當成 ID3v1 切掉"
    );
    assert!(read_id3v1(&frame, TextEncoding::Utf8).unwrap().is_none());
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

/// 16 個字、Big5 超過 30 位元組。其中一個雙位元組字跨在第 30 位元組上，
/// 直接切片會留下 lead byte。
#[test]
fn id3v1_big5_title_truncates_on_character_boundary() {
    use super::super::encoding::decode_text_detailed;

    let title = format!("A{}", "測".repeat(15));
    let encoded = encode_text(&title, TextEncoding::Big5, false).unwrap();
    assert_eq!(title.chars().count(), 16);
    assert!(encoded.len() > 30, "encoded {} bytes", encoded.len());

    let mp3 = id3v1_only_mp3(&title, TextEncoding::Big5);
    let raw = &mp3[mp3.len() - 125..mp3.len() - 95];
    let end = raw.iter().position(|byte| *byte == 0).unwrap_or(raw.len());
    let decoded = decode_text_detailed(&raw[..end], TextEncoding::Big5).unwrap();
    assert!(
        !decoded.had_errors,
        "dangling lead byte in {:?}",
        &raw[..end]
    );
    assert!(
        title.starts_with(&decoded.text),
        "truncated {:?} is not a prefix of {title}",
        decoded.text
    );
    assert!(decoded.text.chars().count() < 16);
    assert!(
        encode_text(&decoded.text, TextEncoding::Big5, false)
            .unwrap()
            .len()
            <= 30
    );
}

#[tokio::test]
async fn id3v1_plan_warns_when_output_encoding_truncates_title() {
    let directory = temp_dir();
    let path = directory.join("long.mp3");
    let title = "測".repeat(15);
    std::fs::write(&path, id3v1_only_mp3(&title, TextEncoding::Gbk)).unwrap();
    let service = AudioService::new();
    let mut request = base_plan(&[path.clone()]);
    request.id3v1_source_encoding = Some(TextEncoding::Gbk);
    request.id3v1_output_encoding = TextEncoding::Utf8;
    request.id3v1_direction = Direction::None;
    request.conversion.direction = Direction::None;
    request.selected_fields = HashMap::from([(
        path.to_string_lossy().into_owned(),
        vec!["id3v1:title".into()],
    )]);
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    let field = field(&plan.files[0], AudioContainer::Id3v1, "title").unwrap();
    assert!(field.truncated);
    let message = field.truncation.clone().unwrap();
    assert!(message.contains("ID3v1 標題超過 30 位元組"), "{message}");
    assert!(plan
        .warnings
        .iter()
        .any(|warning| warning.contains(&message)));
    let written = field.converted_values.as_ref().unwrap()[0].clone();
    assert!(title.starts_with(&written));
    assert!(
        encode_text(&written, TextEncoding::Utf8, false)
            .unwrap()
            .len()
            <= 30
    );
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    let stored = std::fs::read(&path).unwrap();
    let tag = read_id3v1(&stored, TextEncoding::Utf8).unwrap().unwrap();
    assert_eq!(tag.values["title"], written);
    let _ = std::fs::remove_dir_all(&directory);
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
    assert!(!files[0].selected);
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
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
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
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty());
    assert_eq!(
        std::fs::read(PathBuf::from(format!("{}.bak", folder.display())).join("a.mp3")).unwrap(),
        first_before
    );
    assert_eq!(
        std::fs::read(PathBuf::from(format!("{}.bak", folder.display())).join("b.mp3")).unwrap(),
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
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
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
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
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
    service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
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
            field(&scanned[0], AudioContainer::Id3v1, "title").map(|field| field.values.clone()),
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
        let result = service
            .apply(&plan.plan_id, noop(), never_cancel())
            .await
            .unwrap();
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
            field(&verified[0], AudioContainer::Id3v2, "title").map(|field| field.values.clone()),
            Some(vec!["裡面".into()])
        );
    }
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn converts_ape_ogg_oga_and_opus_without_touching_unselected_fields() {
    let ape = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/mac-399.ape");
    let ogg = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/test.ogg");
    assert!(ape.is_file(), "缺少 tests/fixtures/mac-399.ape");
    assert!(ogg.is_file(), "缺少 tests/fixtures/test.ogg");
    let _ = require_ffmpeg();
    let directory = temp_dir();
    let oga_source = directory.join("fixture.oga");
    std::fs::copy(&ogg, &oga_source).unwrap();
    let opus = directory.join("fixture.opus");
    generate_audio(&opus, "libopus");
    let cover_bytes: Vec<u8> = {
        let mut bytes = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
        bytes.extend_from_slice(b"convertzz-cover-fixture");
        bytes.extend((0u8..=63).collect::<Vec<_>>());
        bytes
    };
    let samples = vec![
        (ape, "sample.ape".to_string(), AudioContainer::Apev2),
        (ogg, "sample.ogg".to_string(), AudioContainer::VorbisComment),
        (
            oga_source,
            "sample.oga".to_string(),
            AudioContainer::VorbisComment,
        ),
        (
            opus,
            "sample.opus".to_string(),
            AudioContainer::VorbisComment,
        ),
    ];
    for (source, name, container) in samples {
        let path = directory.join(name);
        std::fs::copy(&source, &path).unwrap();
        let mut tagged = read_tagged(&path).unwrap();
        if tagged.primary_tag().is_none() {
            tagged.insert_tag(lofty::tag::Tag::new(tagged.primary_tag_type()));
        }
        let embed_cover = container == AudioContainer::VorbisComment;
        if let Some(tag) = tagged.primary_tag_mut() {
            tag.set_title("里面开发".to_string());
            tag.set_artist("头发".to_string());
            tag.set_album("未选择里面".to_string());
            if embed_cover {
                tag.push_picture(
                    lofty::picture::Picture::unchecked(cover_bytes.clone())
                        .mime_type(lofty::picture::MimeType::Png)
                        .pic_type(PictureType::CoverFront)
                        .description("cover")
                        .build(),
                );
            }
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
        let before_picture = before
            .primary_tag()
            .and_then(|tag| tag.pictures().first())
            .map(|picture| picture.data().to_vec());
        if embed_cover {
            assert_eq!(before_picture.as_deref(), Some(cover_bytes.as_slice()));
        }
        let before_audio = audio_fingerprint(&path);
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
        if embed_cover {
            assert!(scanned[0].has_cover_art);
        }
        let title = scanned[0]
            .fields
            .iter()
            .find(|field| field.container == container && field.key.eq_ignore_ascii_case("title"))
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
        let result = service
            .apply(&plan.plan_id, noop(), never_cancel())
            .await
            .unwrap();
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
                .and_then(|tag| tag.pictures().first())
                .map(|picture| picture.data().to_vec()),
            before_picture
        );
        assert_eq!(audio_fingerprint(&path), before_audio);
    }
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn converts_multivalue_fields_value_by_value() {
    let ogg = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/test.ogg");
    assert!(ogg.is_file(), "缺少 tests/fixtures/test.ogg");
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
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
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
    assert!(ape.is_file(), "缺少 tests/fixtures/mac-399.ape");
    assert!(ogg.is_file(), "缺少 tests/fixtures/test.ogg");
    let _ = require_ffmpeg();
    let directory = temp_dir();
    let mp3 = directory.join("identify.mp3");
    write_tagged_mp3(&mp3, "里面", "里面", false);
    let mut paths = vec![mp3];
    let ape_dest = directory.join("identify.ape");
    std::fs::copy(&ape, &ape_dest).unwrap();
    paths.push(ape_dest);
    let ogg_dest = directory.join("identify.ogg");
    std::fs::copy(&ogg, &ogg_dest).unwrap();
    paths.push(ogg_dest);
    let oga = directory.join("identify.oga");
    std::fs::copy(&ogg, &oga).unwrap();
    paths.push(oga);
    let opus = directory.join("identify.opus");
    generate_audio(&opus, "libopus");
    paths.push(opus);
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
    let formats: std::collections::BTreeSet<_> = scanned.iter().map(|file| file.format).collect();
    assert!(formats.contains(&AudioFormat::Mp3));
    assert!(formats.contains(&AudioFormat::Ape));
    assert!(formats.contains(&AudioFormat::Ogg));
    assert!(formats.contains(&AudioFormat::Opus));
    assert_eq!(
        scanned
            .iter()
            .filter(|file| file.path.ends_with(".oga"))
            .count(),
        1
    );
    assert!(scanned.iter().any(|file| {
        file.path.ends_with(".oga") && file.format == AudioFormat::Ogg && file.warning.is_none()
    }));
    assert!(scanned.iter().any(|file| {
        file.path.ends_with(".opus") && file.format == AudioFormat::Opus && file.warning.is_none()
    }));
    assert!(scanned.iter().all(|file| file.warning.is_none()));
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn keeps_unknown_text_binary_frames_and_cover_bytes() {
    let directory = temp_dir();
    let path = directory.join("unknown.mp3");
    write_tagged_mp3(&path, "里面", "里面", true);
    let mut tag = id3::Tag::read_from_path(&path).unwrap();
    tag.set_text("TIT3", "未知字幕里面");
    tag.add_frame(id3::frame::EncapsulatedObject {
        mime_type: "application/octet-stream".into(),
        filename: "payload.bin".into(),
        description: "binary".into(),
        data: vec![9, 8, 7, 6, 5],
    });
    tag.write_to_path(&path, Version::Id3v24).unwrap();
    let before = std::fs::read(&path).unwrap();
    let before_picture = picture_data(&path).expect("封面");
    let before_geob = id3::Tag::read_from_path(&path)
        .unwrap()
        .encapsulated_objects()
        .next()
        .map(|object| object.data.clone())
        .expect("GEOB");
    let before_subtitle = id3::Tag::read_from_path(&path)
        .unwrap()
        .get("TIT3")
        .and_then(|frame| frame.content().text())
        .map(str::to_owned)
        .expect("TIT3");
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
    let unknown = field(&scanned[0], AudioContainer::Id3v2, "frame:TIT3").expect("未知文字欄位");
    assert!(!unknown.selected);
    assert!(!scanned[0]
        .fields
        .iter()
        .any(|field| field.key.contains("GEOB") || field.key.contains("APIC")));
    let mut request = base_plan(&[path.clone()]);
    request.selected_fields = HashMap::from([(
        path.to_string_lossy().into_owned(),
        vec!["id3v2:title".into()],
    )]);
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    let after = id3::Tag::read_from_path(&path).unwrap();
    assert_eq!(after.title().as_deref(), Some("裡面"));
    assert_eq!(
        after
            .get("TIT3")
            .and_then(|frame| frame.content().text())
            .map(str::to_owned),
        Some(before_subtitle)
    );
    assert_eq!(
        picture_data(&path).as_deref(),
        Some(before_picture.as_slice())
    );
    assert_eq!(
        after
            .encapsulated_objects()
            .next()
            .map(|object| object.data.clone()),
        Some(before_geob)
    );
    assert_eq!(
        mp3_audio_payload(&before),
        mp3_audio_payload(&std::fs::read(&path).unwrap())
    );
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn keeps_unlisted_ogg_fields_unchanged() {
    let ogg = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/test.ogg");
    assert!(ogg.is_file(), "缺少 tests/fixtures/test.ogg");
    let directory = temp_dir();
    let path = directory.join("unknown.ogg");
    std::fs::copy(&ogg, &path).unwrap();
    let mut tagged = read_tagged(&path).unwrap();
    if tagged.primary_tag().is_none() {
        tagged.insert_tag(lofty::tag::Tag::new(tagged.primary_tag_type()));
    }
    if let Some(tag) = tagged.primary_tag_mut() {
        tag.set_title("里面开发".to_string());
        assert!(
            tag.insert_text(ItemKey::ContentGroup, "未知群組里面".to_string()),
            "應能寫入未列出的 Vorbis 欄位"
        );
    }
    tagged.save_to_path(&path, WriteOptions::default()).unwrap();
    let before_group = read_tagged(&path)
        .unwrap()
        .primary_tag()
        .and_then(|tag| tag.get_string(ItemKey::ContentGroup))
        .map(str::to_owned)
        .expect("ContentGroup");
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
    assert!(scanned[0].fields.iter().all(|field| {
        field.key != "contentGroup" && field.key != "grouping" && !field.label.contains("群組")
    }));
    let title = scanned[0]
        .fields
        .iter()
        .find(|field| field.container == AudioContainer::VorbisComment && field.key == "title")
        .expect("title");
    let mut request = base_plan(&[path.clone()]);
    request.selected_fields = HashMap::from([(
        path.to_string_lossy().into_owned(),
        vec![format!("vorbis-comment:{}", title.key)],
    )]);
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    let after = read_tagged(&path).unwrap();
    assert_eq!(
        after.primary_tag().and_then(|tag| tag.title()).as_deref(),
        Some("裡面開發")
    );
    assert_eq!(
        after
            .primary_tag()
            .and_then(|tag| tag.get_string(ItemKey::ContentGroup))
            .map(str::to_owned),
        Some(before_group)
    );
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn summarize_audio_files_counts_changed_unchanged_and_errors() {
    let changed = AudioTagFile {
        path: "a.mp3".into(),
        format: AudioFormat::Mp3,
        selected: true,
        fields: vec![AudioTagField {
            key: "title".into(),
            label: "標題".into(),
            container: AudioContainer::Id3v2,
            values: vec!["风".into()],
            converted_values: Some(vec!["風".into()]),
            selected: true,
            truncated: false,
            truncation: None,
            origin: None,
            warnings: Vec::new(),
        }],
        has_cover_art: false,
        duration_seconds: None,
        warning: None,
        previewed: true,
        rewrite_containers: Vec::new(),
        removals: Vec::new(),
    };
    let same = AudioTagFile {
        path: "b.ogg".into(),
        format: AudioFormat::Ogg,
        selected: true,
        fields: vec![AudioTagField {
            key: "title".into(),
            label: "標題".into(),
            container: AudioContainer::VorbisComment,
            values: vec!["同".into()],
            converted_values: Some(vec!["同".into()]),
            selected: true,
            truncated: false,
            truncation: None,
            origin: None,
            warnings: Vec::new(),
        }],
        has_cover_art: false,
        duration_seconds: None,
        warning: None,
        previewed: true,
        rewrite_containers: Vec::new(),
        removals: Vec::new(),
    };
    let broken = AudioTagFile {
        path: "c.mp3".into(),
        format: AudioFormat::Mp3,
        selected: false,
        fields: vec![],
        has_cover_art: false,
        duration_seconds: None,
        warning: Some("無法讀取".into()),
        previewed: false,
        rewrite_containers: Vec::new(),
        removals: Vec::new(),
    };
    let summary = summarize_audio_files(&[changed, same, broken]);
    assert_eq!(summary.total, 3);
    assert_eq!(summary.changed, 1);
    assert_eq!(summary.unchanged, 1);
    assert_eq!(summary.errors, 1);
    assert_eq!(summary.selected_fields, 2);
}

#[tokio::test]
async fn rescan_file_replaces_one_plan_entry() {
    let directory = temp_dir();
    let good = directory.join("good.mp3");
    let broken = directory.join("broken.mp3");
    write_tagged_mp3(&good, "里面", "里面", false);
    std::fs::write(&broken, b"not-an-audio-file").unwrap();
    let mut blocked = std::fs::metadata(&broken).unwrap().permissions();
    blocked.set_readonly(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        blocked.set_mode(0);
    }
    std::fs::set_permissions(&broken, blocked).unwrap();
    let service = AudioService::new();
    let mut request = base_plan(&[good.clone(), broken.clone()]);
    request.selected_fields = HashMap::from([(
        good.to_string_lossy().into_owned(),
        vec!["id3v2:title".into()],
    )]);
    let plan = service
        .plan(shared_conversion(), request.clone(), noop())
        .await
        .unwrap();
    assert_eq!(plan.summary.errors, 1);
    let broken_path = broken.to_string_lossy().into_owned();
    assert!(plan
        .files
        .iter()
        .any(|file| file.path == broken_path && file.warning.is_some()));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut readable = std::fs::metadata(&broken).unwrap().permissions();
        readable.set_mode(0o644);
        std::fs::set_permissions(&broken, readable).unwrap();
    }
    write_tagged_mp3(&broken, "后来", "后来", false);
    let updated = service
        .rescan_file(
            shared_conversion(),
            AudioRescanFileRequest {
                path: broken_path.clone(),
                plan_id: Some(plan.plan_id.clone()),
                plan: request,
            },
            noop(),
        )
        .await
        .unwrap();
    assert!(updated.warning.is_none(), "{updated:?}");
    assert_eq!(updated.path, broken_path);
    let title = updated
        .fields
        .iter()
        .find(|field| field.container == AudioContainer::Id3v2 && field.key == "title")
        .expect("title");
    assert_eq!(
        title.converted_values.as_deref(),
        Some(["後來".to_string()].as_slice())
    );

    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    assert!(
        result
            .succeeded
            .iter()
            .any(|path| path == &broken_path || path.ends_with("broken.mp3")),
        "{result:?}"
    );
    let reread = service
        .scan(
            shared_conversion(),
            super::super::types::AudioScanRequest {
                paths: vec![broken_path.clone()],
                recursive: Some(false),
                id3v1_source_encoding: Some(TextEncoding::Utf8),
                id3v2_source_encoding: Some(TextEncoding::Utf8),
                id3v2_repair_source_encoding: Some(false),
            },
            noop(),
        )
        .await
        .unwrap();
    let written = reread
        .iter()
        .find_map(|file| {
            file.fields
                .iter()
                .find(|field| field.container == AudioContainer::Id3v2 && field.key == "title")
        })
        .expect("written title");
    assert_eq!(written.values, vec!["後來".to_string()]);
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn cancel_rejects_old_audio_plan() {
    let directory = temp_dir();
    let path = directory.join("cancel.mp3");
    write_tagged_mp3(&path, "里面", "里面", false);
    let before = std::fs::read(&path).unwrap();
    let service = AudioService::new();
    let mut request = base_plan(&[path.clone()]);
    request.selected_fields = HashMap::from([(
        path.to_string_lossy().into_owned(),
        vec!["id3v1:title".into(), "id3v2:title".into()],
    )]);
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    assert!(plan.summary.total >= 1);
    assert_eq!(plan.summary.errors, 0);
    assert_eq!(service.cancel(&plan.plan_id)["cancelled"], true);
    let error = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap_err();
    assert_eq!(error.code, "PLAN_NOT_FOUND");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn truncated_ogg_reports_warning() {
    let ogg = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/test.ogg");
    assert!(ogg.is_file(), "缺少 tests/fixtures/test.ogg");
    let directory = temp_dir();
    let path = directory.join("truncated.ogg");
    let bytes = std::fs::read(&ogg).unwrap();
    std::fs::write(&path, &bytes[..bytes.len() / 4]).unwrap();
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
    assert!(!files[0].selected);
    assert!(files[0].fields.is_empty());
    let _ = std::fs::remove_dir_all(&directory);
}

fn write_mp3_title_artist(path: &Path, title: &str, artist: &str) {
    std::fs::write(path, mpeg_frame()).unwrap();
    let mut tag = id3::Tag::new();
    tag.set_title(title);
    tag.set_artist(artist);
    tag.write_to_path(path, Version::Id3v24).unwrap();
}

#[tokio::test]
async fn apply_honours_current_file_and_field_selection() {
    let directory = temp_dir();
    let selected = directory.join("selected.mp3");
    let other = directory.join("other.mp3");
    write_mp3_title_artist(&selected, "里面", "头发");
    write_mp3_title_artist(&other, "中国", "头发");
    let other_before = std::fs::read(&other).unwrap();
    let service = AudioService::new();
    let mut request = base_plan(&[selected.clone(), other.clone()]);
    request.id3v1_enabled = false;
    request.selected_fields = HashMap::from([
        (
            selected.to_string_lossy().into_owned(),
            vec!["id3v2:title".into(), "id3v2:artist".into()],
        ),
        (
            other.to_string_lossy().into_owned(),
            vec!["id3v2:title".into(), "id3v2:artist".into()],
        ),
    ]);
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    let selected_path = selected.to_string_lossy().into_owned();
    let other_path = other.to_string_lossy().into_owned();
    let selection = HashMap::from([(selected_path.clone(), vec!["id3v2:title".into()])]);
    let result = service
        .apply_filtered(
            &plan.plan_id,
            Some(&[selected_path.clone()]),
            Some(&selection),
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert_eq!(result.succeeded, vec![selected_path.clone()], "{result:?}");
    assert!(result.skipped.iter().any(|path| path == &other_path));
    assert_eq!(std::fs::read(&other).unwrap(), other_before);
    let written = service
        .scan(
            shared_conversion(),
            AudioScanRequest {
                paths: vec![selected_path],
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
        field(&written[0], AudioContainer::Id3v2, "title").map(|field| field.values.clone()),
        Some(vec!["裡面".into()])
    );
    assert_eq!(
        field(&written[0], AudioContainer::Id3v2, "artist").map(|field| field.values.clone()),
        Some(vec!["头发".into()])
    );
    let _ = std::fs::remove_dir_all(&directory);
}

fn write_id3v2(path: &Path, title: &str, version: Version, encoding: &str) {
    std::fs::write(path, mpeg_frame()).unwrap();
    let mut tag = id3::Tag::new();
    tag.set_title(title);
    apply_id3v2_encoding(&mut tag, encoding, &HashSet::from(["title".to_string()]));
    tag.write_to_path(path, version).unwrap();
}

#[tokio::test]
async fn encoding_or_version_only_changes_are_written() {
    let directory = temp_dir();
    let version_path = directory.join("version.mp3");
    write_id3v2(&version_path, "Hello", Version::Id3v23, "utf8");
    let service = AudioService::new();
    let mut request = base_plan(&[version_path.clone()]);
    request.id3v1_enabled = false;
    request.id3v2_direction = Direction::None;
    request.id3v2_version = 4;
    request.id3v2_encoding = "utf8".into();
    request.selected_fields = HashMap::from([(
        version_path.to_string_lossy().into_owned(),
        vec!["id3v2:title".into()],
    )]);
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    assert_eq!(plan.summary.changed, 1);
    assert_eq!(plan.summary.unchanged, 0);
    assert_eq!(
        plan.files[0].rewrite_containers,
        vec![AudioContainer::Id3v2]
    );
    assert_eq!(
        field(&plan.files[0], AudioContainer::Id3v2, "title")
            .and_then(|field| field.converted_values.clone()),
        None
    );
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    assert_eq!(result.succeeded.len(), 1, "{result:?}");
    let written = std::fs::read(&version_path).unwrap();
    assert_eq!(id3v2_header_version(&written), Some(4));
    assert_eq!(frame_encoding(&written, "TIT2"), 3);

    let encoding_path = directory.join("encoding.mp3");
    write_id3v2(&encoding_path, "Hello", Version::Id3v24, "utf16");
    let before = std::fs::read(&encoding_path).unwrap();
    assert_eq!(frame_encoding(&before, "TIT2"), 1);
    let mut request = base_plan(&[encoding_path.clone()]);
    request.id3v1_enabled = false;
    request.id3v2_direction = Direction::None;
    request.id3v2_version = 4;
    request.id3v2_encoding = "utf8".into();
    let encoding_key = encoding_path.to_string_lossy().into_owned();
    request.selected_fields = HashMap::from([(encoding_key.clone(), vec!["id3v2:title".into()])]);
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    assert_eq!(plan.summary.changed, 1);
    let skipped = service
        .apply_filtered(
            &plan.plan_id,
            Some(&[encoding_key.clone()]),
            Some(&HashMap::from([(encoding_key, Vec::new())])),
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert!(skipped.succeeded.is_empty(), "{skipped:?}");
    assert_eq!(std::fs::read(&encoding_path).unwrap(), before);

    let v1 = directory.join("v1.mp3");
    std::fs::write(&v1, id3v1_only_mp3("裡面", TextEncoding::Gbk)).unwrap();
    let v1_before = std::fs::read(&v1).unwrap();
    let mut request = base_plan(&[v1.clone()]);
    request.id3v2_enabled = false;
    request.id3v1_direction = Direction::None;
    request.id3v1_source_encoding = Some(TextEncoding::Gbk);
    request.id3v1_output_encoding = TextEncoding::Utf8;
    request.selected_fields = HashMap::from([(
        v1.to_string_lossy().into_owned(),
        vec!["id3v1:title".into()],
    )]);
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    assert_eq!(plan.summary.changed, 1);
    assert_eq!(
        plan.files[0].rewrite_containers,
        vec![AudioContainer::Id3v1]
    );
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    let after = std::fs::read(&v1).unwrap();
    assert_ne!(after, v1_before);
    let reread = service
        .scan(
            shared_conversion(),
            AudioScanRequest {
                paths: vec![v1.to_string_lossy().into_owned()],
                recursive: None,
                id3v1_source_encoding: Some(TextEncoding::Utf8),
                id3v2_source_encoding: None,
                id3v2_repair_source_encoding: None,
            },
            noop(),
        )
        .await
        .unwrap();
    assert_eq!(
        field(&reread[0], AudioContainer::Id3v1, "title").map(|field| field.values.clone()),
        Some(vec!["裡面".into()])
    );
    let _ = std::fs::remove_dir_all(&directory);
}

#[cfg(unix)]
#[tokio::test]
async fn write_failure_keeps_original_audio() {
    use std::os::unix::fs::PermissionsExt;
    let directory = temp_dir();
    let path = directory.join("readonly.mp3");
    write_tagged_mp3(&path, "里面", "里面", true);
    let before = std::fs::read(&path).unwrap();
    let before_picture = picture_data(&path);
    let service = AudioService::new();
    let mut request = base_plan(&[path.clone()]);
    request.selected_fields = HashMap::from([(
        path.to_string_lossy().into_owned(),
        vec!["id3v2:title".into()],
    )]);
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o555)).unwrap();
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert_eq!(result.failed.len(), 1);
    assert!(result.succeeded.is_empty());
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(picture_data(&path), before_picture);
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn bak_conflict_skip_keeps_existing_backup() {
    let directory = temp_dir();
    let path = directory.join("song.mp3");
    write_tagged_mp3(&path, "里面", "里面", false);
    let bak = PathBuf::from(format!("{}.bak", path.display()));
    std::fs::write(&bak, b"old-backup").unwrap();
    let service = AudioService::new();
    let mut request = base_plan(&[path.clone()]);
    request.backup = Some(true);
    request.conflict_policy = ConflictPolicy::Skip;
    request.selected_fields = HashMap::from([(
        path.to_string_lossy().into_owned(),
        vec!["id3v2:title".into()],
    )]);
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    assert_eq!(std::fs::read(&bak).unwrap(), b"old-backup");
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
        field(&verified[0], AudioContainer::Id3v2, "title").map(|field| field.values.clone()),
        Some(vec!["裡面".into()])
    );
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn bak_conflict_overwrite_replaces_existing_backup() {
    let directory = temp_dir();
    let path = directory.join("song.mp3");
    write_tagged_mp3(&path, "里面", "里面", false);
    let before = std::fs::read(&path).unwrap();
    let bak = PathBuf::from(format!("{}.bak", path.display()));
    std::fs::write(&bak, b"old-backup").unwrap();
    let service = AudioService::new();
    let mut request = base_plan(&[path.clone()]);
    request.backup = Some(true);
    request.conflict_policy = ConflictPolicy::Overwrite;
    request.selected_fields = HashMap::from([(
        path.to_string_lossy().into_owned(),
        vec!["id3v2:title".into()],
    )]);
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    assert_eq!(std::fs::read(&bak).unwrap(), before);
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn apply_reports_progress_against_the_whole_plan() {
    let directory = temp_dir();
    let changed = directory.join("changed.mp3");
    let same = directory.join("same.mp3");
    write_tagged_mp3(&changed, "里面", "里面", false);
    write_tagged_mp3(&same, "中国", "中国", false);
    let service = AudioService::new();
    let mut request = base_plan(&[changed.clone(), same.clone()]);
    request.id3v1_enabled = false;
    request.selected_fields = HashMap::from([
        (
            changed.to_string_lossy().into_owned(),
            vec!["id3v2:title".into()],
        ),
        (
            same.to_string_lossy().into_owned(),
            vec!["id3v2:title".into()],
        ),
    ]);
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    let events = Arc::new(std::sync::Mutex::new(Vec::<(u64, u64)>::new()));
    let progress = {
        let events = events.clone();
        Arc::new(move |event: super::super::types::ProgressEvent| {
            events
                .lock()
                .expect("progress")
                .push((event.current, event.total));
        })
    };
    let result = service
        .apply(&plan.plan_id, progress, never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    let events = events.lock().expect("progress").clone();
    assert!(
        events
            .iter()
            .any(|(current, total)| *total == 2 && current < total),
        "{events:?}"
    );
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn repeated_comment_apply_does_not_append_frames() {
    let directory = temp_dir();
    let path = directory.join("note.mp3");
    std::fs::write(&path, mpeg_frame()).unwrap();
    let mut tag = id3::Tag::new();
    tag.set_title("歌");
    tag.add_frame(id3::frame::Comment {
        lang: "eng".into(),
        description: String::new(),
        text: "里面".into(),
    });
    tag.write_to_path(&path, Version::Id3v24).unwrap();
    let service = AudioService::new();
    let mut request = base_plan(&[path.clone()]);
    request.id3v1_enabled = false;
    request.selected_fields = HashMap::from([(
        path.to_string_lossy().into_owned(),
        vec!["id3v2:comment".into()],
    )]);
    for _ in 0..2 {
        let plan = service
            .plan(shared_conversion(), request.clone(), noop())
            .await
            .unwrap();
        let result = service
            .apply(&plan.plan_id, noop(), never_cancel())
            .await
            .unwrap();
        assert!(result.failed.is_empty(), "{result:?}");
    }
    let written = id3::Tag::read_from_path(&path).unwrap();
    let comments: Vec<String> = written
        .comments()
        .map(|comment| comment.text.clone())
        .collect();
    assert_eq!(comments, vec!["裡面".to_string()]);
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn writing_lyrics_keeps_other_uslt_frames() {
    let mut tag = id3::Tag::new();
    tag.add_frame(id3::frame::Lyrics {
        lang: "zho".into(),
        description: "演唱".into(),
        text: "里面".into(),
    });
    tag.add_frame(id3::frame::Lyrics {
        lang: "eng".into(),
        description: "translation".into(),
        text: "inside".into(),
    });
    set_id3v2_value(&mut tag, "lyrics", "裡面".into());
    let lyrics: Vec<_> = tag.lyrics().cloned().collect();
    assert_eq!(
        lyrics
            .iter()
            .map(|frame| (
                frame.lang.as_str(),
                frame.description.as_str(),
                frame.text.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![("zho", "演唱", "裡面"), ("eng", "translation", "inside"),]
    );
}

/// 多個 TXXX 以描述區分。轉換其中一則時，另一則的描述與文字都要留下。
#[tokio::test]
async fn converting_one_txxx_keeps_the_other_description() {
    let directory = temp_dir();
    let path = directory.join("txxx.mp3");
    std::fs::write(&path, mpeg_frame()).unwrap();
    let mut tag = id3::Tag::new();
    tag.set_title("歌");
    tag.add_frame(id3::frame::ExtendedText {
        description: "来源".into(),
        value: "里面".into(),
    });
    tag.add_frame(id3::frame::ExtendedText {
        description: "备注".into(),
        value: "简体".into(),
    });
    tag.write_to_path(&path, Version::Id3v24).unwrap();

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
    let source = field(&scanned[0], AudioContainer::Id3v2, "frame:TXXX:来源").expect("TXXX 来源");
    let note = field(&scanned[0], AudioContainer::Id3v2, "frame:TXXX:备注").expect("TXXX 备注");
    assert_eq!(source.values, vec!["里面".to_string()]);
    assert_eq!(note.values, vec!["简体".to_string()]);

    let mut request = base_plan(&[path.clone()]);
    request.id3v1_enabled = false;
    request.selected_fields = HashMap::from([(
        path.to_string_lossy().into_owned(),
        vec!["id3v2:frame:TXXX:来源".into()],
    )]);
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    let written = id3::Tag::read_from_path(&path).unwrap();
    let texts: Vec<(String, String)> = written
        .extended_texts()
        .map(|item| (item.description.clone(), item.value.clone()))
        .collect();
    assert_eq!(
        texts,
        vec![
            ("来源".to_string(), "裡面".to_string()),
            ("备注".to_string(), "简体".to_string()),
        ]
    );
    assert_eq!(
        written
            .frames()
            .filter(|frame| frame.id() == "TXXX")
            .count(),
        2
    );
    let _ = std::fs::remove_dir_all(&directory);
}

fn id3_text(tag: &id3::Tag, id: &str) -> Option<String> {
    tag.get(id)
        .and_then(|frame| frame.content().text())
        .map(str::to_string)
}

/// 年份／音軌不是純數字時，預覽仍會顯示轉換後的文字。寫入不可默默留下舊數字。
#[test]
fn non_numeric_id3v2_year_and_track_are_stored_as_text_frames() {
    let mut tag = id3::Tag::new();
    tag.set_year(1998);
    tag.set_track(3);
    set_id3v2_value(&mut tag, "year", "录制年".into());
    set_id3v2_value(&mut tag, "track", "第一辑".into());
    assert_eq!(id3_text(&tag, "TYER").as_deref(), Some("录制年"));
    assert_eq!(id3_text(&tag, "TRCK").as_deref(), Some("第一辑"));
    assert_eq!(id3v2_standard(&tag, "year").as_deref(), Some("录制年"));
    assert_eq!(id3v2_standard(&tag, "track").as_deref(), Some("第一辑"));

    let mut numeric = id3::Tag::new();
    numeric.set_text("TRCK", "4/12");
    set_id3v2_value(&mut numeric, "year", "1998".into());
    set_id3v2_value(&mut numeric, "track", "7".into());
    assert_eq!(numeric.year(), Some(1998));
    assert_eq!(numeric.track(), Some(7));
    assert_eq!(id3_text(&numeric, "TRCK").as_deref(), Some("7/12"));
}

#[tokio::test]
async fn plan_reuses_scan_cache_and_omits_duplicate_text() {
    let directory = temp_dir();
    let path = directory.join("cached.mp3");
    write_tagged_mp3(&path, "里面", "Hello", false);
    let service = AudioService::new();
    let path_text = path.to_string_lossy().into_owned();
    service
        .scan(
            shared_conversion(),
            AudioScanRequest {
                paths: vec![path_text.clone()],
                recursive: Some(false),
                id3v1_source_encoding: Some(TextEncoding::Gbk),
                id3v2_source_encoding: None,
                id3v2_repair_source_encoding: None,
            },
            noop(),
        )
        .await
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    let mut request = base_plan(&[path.clone()]);
    request.recursive = Some(false);
    request.id3v1_enabled = false;
    request.id3v2_direction = Direction::None;
    request.selected_fields = HashMap::from([(path_text, vec!["id3v2:title".into()])]);
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    assert!(
        plan.files[0].warning.is_none(),
        "{:?}",
        plan.files[0].warning
    );
    assert!(plan.files[0].previewed);
    assert!(field(&plan.files[0], AudioContainer::Id3v2, "title")
        .unwrap()
        .converted_values
        .is_none());
    let json = serde_json::to_value(&plan).unwrap();
    assert!(json.get("warnings").is_none());
    assert!(json["files"][0].get("hasCoverArt").is_none());
    assert_eq!(json["files"][0]["previewed"], true);
    assert!(json["files"][0]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .all(|field| field.get("convertedValues").is_none()));
    let _ = std::fs::remove_dir_all(&directory);
}

#[cfg(unix)]
#[tokio::test]
async fn recursive_audio_scan_skips_symlinks() {
    let directory = temp_dir();
    let outside = temp_dir();
    let inside = directory.join("inside.mp3");
    let linked = outside.join("outside.mp3");
    write_tagged_mp3(&inside, "里面", "里面", false);
    write_tagged_mp3(&linked, "开发", "开发", false);
    std::os::unix::fs::symlink(&linked, directory.join("linked.mp3")).unwrap();
    std::os::unix::fs::symlink(&outside, directory.join("linked-dir")).unwrap();
    let service = AudioService::new();
    let scanned = service
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
        scanned
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>(),
        [inside.to_string_lossy().into_owned()]
    );
    let _ = std::fs::remove_dir_all(&directory);
    let _ = std::fs::remove_dir_all(&outside);
}

fn synchsafe(size: usize) -> [u8; 4] {
    [
        ((size >> 21) & 0x7f) as u8,
        ((size >> 14) & 0x7f) as u8,
        ((size >> 7) & 0x7f) as u8,
        (size & 0x7f) as u8,
    ]
}

fn id3_header(version: u8, flags: u8, body_len: usize) -> Vec<u8> {
    let size = synchsafe(body_len);
    vec![
        b'I', b'D', b'3', version, 0, flags, size[0], size[1], size[2], size[3],
    ]
}

fn v23_frame(id: &str, encoding: u8, text: &[u8]) -> Vec<u8> {
    let mut data = vec![encoding];
    data.extend_from_slice(text);
    let size = (data.len() as u32).to_be_bytes();
    let mut frame = id.as_bytes().to_vec();
    frame.extend_from_slice(&size);
    frame.extend_from_slice(&[0, 0]);
    frame.extend_from_slice(&data);
    frame
}

#[test]
fn id3_header_parser_handles_extended_unsync_and_v22_without_reading_audio() {
    let directory = temp_dir();
    let extended = directory.join("extended.mp3");
    let real = v23_frame("TIT2", 3, b"Hi");
    let mut padding = b"TIT2".to_vec();
    padding.extend_from_slice(&10u32.to_be_bytes());
    padding.extend_from_slice(&[0, 0, 0]);
    let extended_size = (6 + padding.len()) as u32;
    let mut body = extended_size.to_be_bytes().to_vec();
    body.extend_from_slice(&[0, 0]);
    body.extend_from_slice(&0u32.to_be_bytes());
    body.extend_from_slice(&padding);
    body.extend_from_slice(&real);
    let mut file = id3_header(3, 0x40, body.len());
    file.extend_from_slice(&body);
    file.extend_from_slice(&[0xAB; 64]);
    std::fs::write(&extended, &file).unwrap();
    let read = read_id3v2_declared_tag(&extended).unwrap();
    assert_eq!(read.len(), 10 + body.len());
    assert!(read.ends_with(&real));
    match classify_id3v2(&read) {
        Id3v2Class::Frames { version, encodings } => {
            assert_eq!(version, 3);
            assert_eq!(encodings, vec![3]);
        }
        other => panic!("延伸標頭應略過填充裡的假 TIT2：{other:?}"),
    }

    let unsync = directory.join("unsync.mp3");
    // 資料長度指示先出現。未同步化把 FF 00 插在指示裡，還原後編碼才是 3。
    let mut indicated = b"TIT2".to_vec();
    indicated.extend_from_slice(&6u32.to_be_bytes());
    indicated.extend_from_slice(&[0, 0x01]);
    indicated.extend_from_slice(&[0, 0, 0, 0xff, 0x00, 3]);
    let mut unsync_file = id3_header(4, 0x80, indicated.len());
    unsync_file.extend_from_slice(&indicated);
    unsync_file.extend_from_slice(b"NOT-A-TAG-AUDIO");
    std::fs::write(&unsync, &unsync_file).unwrap();
    let unsync_tag = read_id3v2_declared_tag(&unsync).unwrap();
    assert!(!unsync_tag.windows(4).any(|window| window == b"NOT-"));
    match classify_id3v2(&unsync_tag) {
        Id3v2Class::Frames { encodings, .. } => assert_eq!(encodings, vec![3]),
        other => panic!("未同步化應還原後再讀幀：{other:?}"),
    }

    let legacy = directory.join("v22.mp3");
    let mut v22 = id3_header(2, 0, 10);
    v22.extend_from_slice(b"TT2");
    v22.extend_from_slice(&[0, 0, 4]);
    v22.extend_from_slice(&[0, b'H', b'i', 0]);
    v22.extend_from_slice(b"AUDIO");
    std::fs::write(&legacy, &v22).unwrap();
    let legacy_tag = read_id3v2_declared_tag(&legacy).unwrap();
    assert_eq!(legacy_tag.len(), 20);
    match classify_id3v2(&legacy_tag) {
        Id3v2Class::Frames { version, encodings } => {
            assert_eq!(version, 2);
            assert_eq!(encodings, vec![0]);
        }
        other => panic!("ID3v2.2 應以 6 位元組幀頭解析：{other:?}"),
    }

    let compressed = directory.join("compressed.mp3");
    let mut compressed_tag = id3_header(2, 0x40, 6);
    compressed_tag.extend_from_slice(b"TT2");
    compressed_tag.extend_from_slice(&[0, 0, 0]);
    std::fs::write(&compressed, &compressed_tag).unwrap();
    let before = std::fs::read(&compressed).unwrap();
    let request = base_plan(&[compressed.clone()]);
    assert!(!id3v2_needs_rewrite(&compressed, &request));
    match classify_id3v2(&before) {
        Id3v2Class::Unsafe(reason) => assert!(reason.contains("壓縮")),
        other => panic!("壓縮的 v2.2 必須略過：{other:?}"),
    }
    assert_eq!(std::fs::read(&compressed).unwrap(), before);

    let oversized = directory.join("huge.mp3");
    let mut huge = id3_header(3, 0, 0);
    let big = synchsafe(ID3_TAG_READ_LIMIT + 1);
    huge[6..10].copy_from_slice(&big);
    std::fs::write(&oversized, &huge).unwrap();
    assert!(read_id3v2_declared_tag(&oversized).is_err());
    assert!(!id3v2_needs_rewrite(
        &oversized,
        &base_plan(&[oversized.clone()])
    ));

    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn encoding_rewrite_leaves_unchecked_text_frames() {
    let directory = temp_dir();
    let path = directory.join("partial-encoding.mp3");
    std::fs::write(&path, mpeg_frame()).unwrap();
    let mut tag = id3::Tag::new();
    tag.set_title("裡面");
    tag.set_artist("歌手");
    tag.write_to_path(&path, id3::Version::Id3v24).unwrap();
    let before_artist = frame_encoding(&std::fs::read(&path).unwrap(), "TPE1");
    let service = AudioService::new();
    let mut request = base_plan(&[path.clone()]);
    request.id3v2_version = 4;
    request.id3v2_encoding = "utf16".into();
    request.selected_fields = HashMap::from([(
        path.to_string_lossy().into_owned(),
        vec!["id3v2:title".into()],
    )]);
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    let written = std::fs::read(&path).unwrap();
    assert_eq!(frame_encoding(&written, "TIT2"), 1);
    assert_eq!(frame_encoding(&written, "TPE1"), before_artist);
    assert_eq!(mp3_audio_payload(&written), mpeg_frame());
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn priority_queue_runs_the_requested_path_first_and_batches_hold_32() {
    use std::collections::VecDeque;
    let mut pending: VecDeque<&str> = VecDeque::from(["a", "b", "c", "d"]);
    let mut priority = VecDeque::from(["c".to_string(), "missing".to_string()]);
    let first = take_prioritized(&mut pending, &mut priority, |item| (*item).to_string());
    assert_eq!(first, Some("c"));
    let second = take_prioritized(&mut pending, &mut priority, |item| (*item).to_string());
    assert_eq!(second, Some("a"));
    assert_eq!(pending.into_iter().collect::<Vec<_>>(), vec!["b", "d"]);

    let mut buffer = Vec::new();
    let mut emitted = Vec::new();
    for index in 0..40 {
        buffer.push(index);
        if let Some(batch) = drain_stream_batch(&mut buffer, false) {
            emitted.push(batch.len());
        }
    }
    assert_eq!(emitted, vec![AUDIO_STREAM_BATCH]);
    assert_eq!(AUDIO_STREAM_BATCH, 32);

    // 掃描與預覽共用同一條優先佇列。較新的預覽路徑排在前面，但掃描的待處理
    // 清單裡沒有它。兩邊都必須各自拿到自己的優先檔，不能互相清掉。
    let service = AudioService::new();
    service.prioritize("scan-hit");
    service.prioritize("preview-hit");
    let mut scan_pending = VecDeque::from(["scan-other", "scan-hit", "scan-tail"]);
    let mut preview_pending = VecDeque::from(["preview-other", "preview-hit"]);
    let scan_next = service.take_next(&mut scan_pending, |item| (*item).to_string());
    let preview_next = service.take_next(&mut preview_pending, |item| (*item).to_string());
    assert_eq!(scan_next, Some("scan-hit"));
    assert_eq!(preview_next, Some("preview-hit"));
    assert_eq!(
        scan_pending.into_iter().collect::<Vec<_>>(),
        vec!["scan-other", "scan-tail"]
    );
    assert_eq!(
        preview_pending.into_iter().collect::<Vec<_>>(),
        vec!["preview-other"]
    );
    let rest = drain_stream_batch(&mut buffer, true).expect("remainder");
    assert_eq!(rest.len(), 8);
    assert!(drain_stream_batch(&mut buffer, true).is_none());
}

fn quiet_plan(paths: &[PathBuf]) -> AudioTagPlanRequest {
    let mut request = base_plan(paths);
    request.conversion.direction = Direction::None;
    request.id3v1_direction = Direction::None;
    request.id3v2_direction = Direction::None;
    request.id3v1_enabled = false;
    request.id3v2_enabled = false;
    request
}

/// 掃描只露出第一則 COMM，以及有文字的 T*。多出來的 COMM、USLT、
/// 沒有文字內容的 T*（例如 TIPL）都該算進隱藏欄位。
#[tokio::test]
async fn id3v2_hidden_count_includes_frames_the_scan_does_not_show() {
    let directory = temp_dir();
    let path = directory.join("hidden-frames.mp3");
    std::fs::write(&path, mpeg_frame()).unwrap();
    let mut tag = id3::Tag::new();
    tag.set_title("可見標題");
    tag.add_frame(id3::frame::Comment {
        lang: "eng".into(),
        description: String::new(),
        text: "第一則註解".into(),
    });
    tag.add_frame(id3::frame::Comment {
        lang: "eng".into(),
        description: "extra".into(),
        text: "第二則註解".into(),
    });
    tag.add_frame(id3::frame::Lyrics {
        lang: "eng".into(),
        description: String::new(),
        text: "未列出的歌詞".into(),
    });
    tag.add_frame(id3::frame::Frame::with_content(
        "TIPL",
        id3::frame::Content::InvolvedPeopleList(id3::frame::InvolvedPeopleList {
            items: vec![id3::frame::InvolvedPeopleListItem {
                involvement: "producer".into(),
                involvee: "某人".into(),
            }],
        }),
    ));
    tag.set_text("TPUB", "可見出版社");
    tag.write_to_path(&path, Version::Id3v24).unwrap();
    let mut output = std::fs::read(&path).unwrap();
    output = append_id3v1(
        &strip_id3v1(&output),
        &HashMap::from([("title", "v1".to_string())]),
        17,
        TextEncoding::Utf8,
    )
    .unwrap();
    std::fs::write(&path, output).unwrap();

    let service = AudioService::new();
    let scanned = service
        .scan(
            shared_conversion(),
            quiet_plan(&[path.clone()]).scan_request(),
            noop(),
        )
        .await
        .unwrap();
    let shown: Vec<&str> = scanned[0]
        .fields
        .iter()
        .filter(|field| field.container == AudioContainer::Id3v2)
        .map(|field| field.key.as_str())
        .collect();
    assert!(shown.contains(&"title"));
    assert!(shown.contains(&"comment"));
    assert!(shown.contains(&"frame:TPUB"));
    assert_eq!(shown.iter().filter(|key| **key == "comment").count(), 1);
    assert!(!shown
        .iter()
        .any(|key| key.contains("第二則") || *key == "lyrics"));
    let comment = field(&scanned[0], AudioContainer::Id3v2, "comment").unwrap();
    assert_eq!(comment.values, vec!["第一則註解".to_string()]);

    let mut request = quiet_plan(&[path.clone()]);
    request.tag_operations = vec![remove_operation(AudioContainer::Id3v2)];
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    let removal = plan.files[0]
        .removals
        .iter()
        .find(|item| item.container == AudioContainer::Id3v2)
        .expect("id3v2 removal");
    assert!(removal.refused.is_none(), "{removal:?}");
    // 第二則 COMM、USLT、沒有文字的 TIPL。標題、第一則註解、TPUB 算可見。
    assert_eq!(removal.hidden_field_count, 3, "{removal:?}");
}

fn remove_operation(container: AudioContainer) -> TagOperation {
    TagOperation {
        action: TagAction::Remove,
        source: Some(container),
        target: None,
        fields: Vec::new(),
        on_conflict: None,
        enabled: true,
        paths: Vec::new(),
    }
}

fn attach_ape(path: &Path, title: &str) {
    use lofty::tag::{Accessor, ItemKey, Tag};
    let mut tagged = read_tagged(path).unwrap();
    let mut ape = Tag::new(lofty::tag::TagType::Ape);
    ape.set_title(title.to_string());
    ape.insert_text(ItemKey::ReplayGainTrackGain, String::from("-1.5 dB"));
    tagged.insert_tag(ape);
    tagged
        .save_to_path(path, lofty::config::WriteOptions::default())
        .unwrap();
}

fn write_triple_mp3(path: &Path) {
    generate_audio(path, "libmp3lame");
    let mut tag = id3::Tag::new();
    tag.set_title("v2-title");
    tag.set_text("TPUB", "hidden-publisher");
    tag.write_to_path(path, Version::Id3v24).unwrap();
    attach_ape(path, "ape-title");
    let mut output = std::fs::read(path).unwrap();
    output = append_id3v1(
        &strip_id3v1(&output),
        &HashMap::from([("title", "v1-title".to_string())]),
        17,
        TextEncoding::Utf8,
    )
    .unwrap();
    std::fs::write(path, output).unwrap();
}

#[tokio::test]
async fn tag_operations_remove_containers_keep_audio_bytes() {
    let directory = temp_dir();
    let original = directory.join("triple.mp3");
    write_triple_mp3(&original);
    let before = std::fs::read(&original).unwrap();
    let payload = mp3_audio_payload(&before);
    let service = AudioService::new();
    let scanned = service
        .scan(
            shared_conversion(),
            quiet_plan(&[original.clone()]).scan_request(),
            noop(),
        )
        .await
        .unwrap();
    assert!(field(&scanned[0], AudioContainer::Apev2, "title").is_some());
    assert!(field(&scanned[0], AudioContainer::Id3v1, "title").is_some());
    assert!(field(&scanned[0], AudioContainer::Id3v2, "title").is_some());

    for container in [
        AudioContainer::Id3v1,
        AudioContainer::Id3v2,
        AudioContainer::Apev2,
    ] {
        let path = directory.join(format!("remove-{container:?}.mp3"));
        std::fs::copy(&original, &path).unwrap();
        let mut request = quiet_plan(&[path.clone()]);
        request.tag_operations = vec![remove_operation(container)];
        let plan = service
            .plan(shared_conversion(), request, noop())
            .await
            .unwrap();
        let removal = plan.files[0]
            .removals
            .iter()
            .find(|item| item.container == container)
            .unwrap_or_else(|| panic!("missing removal {container:?}"));
        assert!(
            removal.refused.is_none(),
            "{container:?} {:?}",
            removal.refused
        );
        assert!(removal.selected);
        assert_eq!(removal.origin, "rule 1");
        if container == AudioContainer::Apev2 {
            assert!(
                removal.hidden_field_count >= 1,
                "{}",
                removal.hidden_field_count
            );
        }
        let result = service
            .apply(&plan.plan_id, noop(), never_cancel())
            .await
            .unwrap();
        assert!(result.failed.is_empty(), "{container:?} {result:?}");
        let stored = std::fs::read(&path).unwrap();
        assert_eq!(mp3_audio_payload(&stored), payload, "{container:?}");
        match container {
            AudioContainer::Id3v1 => {
                assert!(read_id3v1(&stored, TextEncoding::Utf8).unwrap().is_none())
            }
            AudioContainer::Id3v2 => assert!(id3v2_header_version(&stored).is_none()),
            AudioContainer::Apev2 => {
                use lofty::file::TaggedFileExt;
                let tagged = read_tagged(&path).unwrap();
                assert!(tagged.tag(lofty::tag::TagType::Ape).is_none());
            }
            AudioContainer::VorbisComment => {}
        }
    }

    let ape_only = directory.join("ape-only.mp3");
    generate_audio(&ape_only, "libmp3lame");
    attach_ape(&ape_only, "only-ape");
    let stripped = strip_id3v1(&strip_id3v2(&std::fs::read(&ape_only).unwrap())).to_vec();
    std::fs::write(&ape_only, &stripped).unwrap();
    let ape_before = stripped;
    let mut request = quiet_plan(&[ape_only.clone()]);
    request.tag_operations = vec![remove_operation(AudioContainer::Apev2)];
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    let removal = &plan.files[0].removals[0];
    assert!(removal.refused.is_some(), "{removal:?}");
    assert!(!removal.selected);
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    assert_eq!(std::fs::read(&ape_only).unwrap(), ape_before);

    let v22 = directory.join("v22.mp3");
    std::fs::write(&v22, mpeg_frame()).unwrap();
    let mut tag = id3::Tag::new();
    tag.set_title("v22");
    tag.write_to_path(&v22, Version::Id3v22).unwrap();
    let mut output = std::fs::read(&v22).unwrap();
    output = append_id3v1(
        &strip_id3v1(&output),
        &HashMap::from([("title", "keep".to_string())]),
        255,
        TextEncoding::Utf8,
    )
    .unwrap();
    std::fs::write(&v22, &output).unwrap();
    assert_eq!(id3v2_header_version(&output), Some(2));
    let payload_v22 = mp3_audio_payload(&output);
    let mut request = quiet_plan(&[v22.clone()]);
    request.tag_operations = vec![remove_operation(AudioContainer::Id3v2)];
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    let stored = std::fs::read(&v22).unwrap();
    assert_eq!(mp3_audio_payload(&stored), payload_v22);
    assert!(id3v2_header_version(&stored).is_none());

    let unsync = directory.join("unsync.mp3");
    std::fs::write(&unsync, mpeg_frame()).unwrap();
    let mut tag = id3::Tag::new();
    tag.set_title("unsync");
    tag.write_to_path(&unsync, Version::Id3v23).unwrap();
    let mut raw = std::fs::read(&unsync).unwrap();
    raw[5] |= 0x80;
    raw = append_id3v1(
        &strip_id3v1(&raw),
        &HashMap::from([("title", "keep".to_string())]),
        255,
        TextEncoding::Utf8,
    )
    .unwrap();
    std::fs::write(&unsync, &raw).unwrap();
    assert_ne!(raw[5] & 0x80, 0);
    let payload_unsync = mp3_audio_payload(&raw);
    let mut request = quiet_plan(&[unsync.clone()]);
    request.tag_operations = vec![remove_operation(AudioContainer::Id3v2)];
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    let stored = std::fs::read(&unsync).unwrap();
    assert_eq!(mp3_audio_payload(&stored), payload_unsync);
    assert!(id3v2_header_version(&stored).is_none());

    // ID3v2.2 與未同步化標籤只拿掉旁邊的容器，本體與音訊都不能被重寫。
    let v22_side = directory.join("v22-side.mp3");
    generate_audio(&v22_side, "libmp3lame");
    let audio = mp3_audio_payload(&std::fs::read(&v22_side).unwrap());
    let ape_host = directory.join("ape-host.mp3");
    std::fs::write(&ape_host, &audio).unwrap();
    attach_ape(&ape_host, "ape-title");
    let host = strip_id3v2(&std::fs::read(&ape_host).unwrap());
    let host_audio = mp3_audio_payload(&host);
    assert_eq!(host_audio, audio);
    let ape = &host[host_audio.len()..];
    assert!(ape.windows(8).any(|window| window == b"APETAGEX"));
    std::fs::write(&v22_side, &audio).unwrap();
    let mut tag = id3::Tag::new();
    tag.set_title("v22");
    tag.write_to_path(&v22_side, Version::Id3v22).unwrap();
    let with_v22 = std::fs::read(&v22_side).unwrap();
    assert_eq!(id3v2_header_version(&with_v22), Some(2));
    assert_eq!(mp3_audio_payload(&with_v22), audio);
    let mut combined = with_v22;
    combined.extend_from_slice(ape);
    combined = append_id3v1(
        &strip_id3v1(&combined),
        &HashMap::from([("title", "v1-title".to_string())]),
        17,
        TextEncoding::Utf8,
    )
    .unwrap();
    std::fs::write(&v22_side, &combined).unwrap();
    let payload_v22_side = mp3_audio_payload(&combined);
    for container in [AudioContainer::Id3v1, AudioContainer::Apev2] {
        let path = directory.join(format!("v22-remove-{container:?}.mp3"));
        std::fs::copy(&v22_side, &path).unwrap();
        let mut request = quiet_plan(&[path.clone()]);
        request.tag_operations = vec![remove_operation(container)];
        let plan = service
            .plan(shared_conversion(), request, noop())
            .await
            .unwrap();
        assert!(plan.files[0].removals[0].refused.is_none());
        let result = service
            .apply(&plan.plan_id, noop(), never_cancel())
            .await
            .unwrap();
        assert!(result.failed.is_empty(), "{container:?} {result:?}");
        let stored = std::fs::read(&path).unwrap();
        assert_eq!(
            mp3_audio_payload(&stored),
            payload_v22_side,
            "{container:?}"
        );
        assert_eq!(id3v2_header_version(&stored), Some(2), "{container:?}");
        match container {
            AudioContainer::Id3v1 => {
                assert!(read_id3v1(&stored, TextEncoding::Utf8).unwrap().is_none());
            }
            AudioContainer::Apev2 => {
                use lofty::file::TaggedFileExt;
                assert!(read_tagged(&path)
                    .unwrap()
                    .tag(lofty::tag::TagType::Ape)
                    .is_none());
                assert!(read_id3v1(&stored, TextEncoding::Utf8).unwrap().is_some());
            }
            _ => {}
        }
    }

    let unsync_v1 = directory.join("unsync-v1.mp3");
    std::fs::write(&unsync_v1, mpeg_frame()).unwrap();
    let mut tag = id3::Tag::new();
    tag.set_title("unsync");
    tag.write_to_path(&unsync_v1, Version::Id3v23).unwrap();
    let mut raw = std::fs::read(&unsync_v1).unwrap();
    raw[5] |= 0x80;
    raw = append_id3v1(
        &strip_id3v1(&raw),
        &HashMap::from([("title", "keep".to_string())]),
        255,
        TextEncoding::Utf8,
    )
    .unwrap();
    std::fs::write(&unsync_v1, &raw).unwrap();
    let payload_unsync_v1 = mp3_audio_payload(&raw);
    let mut request = quiet_plan(&[unsync_v1.clone()]);
    request.tag_operations = vec![remove_operation(AudioContainer::Id3v1)];
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    assert!(plan.files[0].removals[0].refused.is_none());
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    let stored = std::fs::read(&unsync_v1).unwrap();
    assert_eq!(mp3_audio_payload(&stored), payload_unsync_v1);
    assert_eq!(id3v2_header_version(&stored), Some(3));
    assert_ne!(stored[5] & 0x80, 0);
    assert!(read_id3v1(&stored, TextEncoding::Utf8).unwrap().is_none());

    let covered = directory.join("covered.mp3");
    std::fs::write(&covered, mpeg_frame()).unwrap();
    let mut tag = id3::Tag::new();
    tag.set_title("with-cover");
    tag.add_frame(id3::frame::Picture {
        mime_type: "image/png".into(),
        picture_type: id3::frame::PictureType::CoverFront,
        description: String::new(),
        data: vec![1, 2, 3, 4],
    });
    tag.write_to_path(&covered, Version::Id3v24).unwrap();
    let mut covered_bytes = std::fs::read(&covered).unwrap();
    covered_bytes = append_id3v1(
        &strip_id3v1(&covered_bytes),
        &HashMap::from([("title", "v1".to_string())]),
        255,
        TextEncoding::Utf8,
    )
    .unwrap();
    std::fs::write(&covered, &covered_bytes).unwrap();
    assert_eq!(picture_count(&covered).unwrap(), 1);
    let payload_covered = mp3_audio_payload(&covered_bytes);
    let mut request = quiet_plan(&[covered.clone()]);
    request.tag_operations = vec![remove_operation(AudioContainer::Id3v1)];
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    let stored = std::fs::read(&covered).unwrap();
    assert_eq!(mp3_audio_payload(&stored), payload_covered);
    assert_eq!(picture_count(&covered).unwrap(), 1);

    let drop_v2 = directory.join("drop-v2-cover.mp3");
    std::fs::copy(&covered, &drop_v2).unwrap();
    // 上面已拿掉 ID3v1，這裡要同時留 v1，移除 ID3v2 才不是唯一標籤。
    let mut both = std::fs::read(&covered).unwrap();
    both = append_id3v1(
        &strip_id3v1(&both),
        &HashMap::from([("title", "v1".to_string())]),
        255,
        TextEncoding::Utf8,
    )
    .unwrap();
    std::fs::write(&drop_v2, &both).unwrap();
    let mut request = quiet_plan(&[drop_v2.clone()]);
    request.tag_operations = vec![remove_operation(AudioContainer::Id3v2)];
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    assert!(plan.files[0].removals[0].refused.is_none());
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    assert_eq!(picture_count(&drop_v2).unwrap(), 0);

    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn ape_file_removes_apev2_or_id3v1_without_changing_audio() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/mac-399.ape");
    assert!(fixture.is_file(), "缺少 tests/fixtures/mac-399.ape");
    let directory = temp_dir();
    let original = directory.join("both.ape");
    std::fs::copy(&fixture, &original).unwrap();
    {
        let mut tagged = read_tagged(&original).unwrap();
        let mut ape = lofty::tag::Tag::new(lofty::tag::TagType::Ape);
        ape.set_title("ape-title".to_string());
        ape.insert_text(
            lofty::tag::ItemKey::ReplayGainTrackGain,
            String::from("-1.5 dB"),
        );
        tagged.insert_tag(ape);
        tagged
            .save_to_path(&original, lofty::config::WriteOptions::default())
            .unwrap();
    }
    let mut output = std::fs::read(&original).unwrap();
    output = append_id3v1(
        &strip_id3v1(&output),
        &HashMap::from([("title", "v1-title".to_string())]),
        17,
        TextEncoding::Utf8,
    )
    .unwrap();
    std::fs::write(&original, &output).unwrap();
    let payload = ape_audio_payload(&output);
    assert_eq!(payload, std::fs::read(&fixture).unwrap());

    let service = AudioService::new();
    let scanned = service
        .scan(
            shared_conversion(),
            quiet_plan(&[original.clone()]).scan_request(),
            noop(),
        )
        .await
        .unwrap();
    assert_eq!(
        field(&scanned[0], AudioContainer::Apev2, "title")
            .unwrap()
            .values,
        vec!["ape-title".to_string()]
    );
    assert_eq!(
        field(&scanned[0], AudioContainer::Id3v1, "title")
            .unwrap()
            .values,
        vec!["v1-title".to_string()]
    );

    for container in [AudioContainer::Apev2, AudioContainer::Id3v1] {
        let path = directory.join(format!("remove-{container:?}.ape"));
        std::fs::copy(&original, &path).unwrap();
        let mut request = quiet_plan(&[path.clone()]);
        request.tag_operations = vec![remove_operation(container)];
        let plan = service
            .plan(shared_conversion(), request, noop())
            .await
            .unwrap();
        let removal = plan.files[0]
            .removals
            .iter()
            .find(|item| item.container == container)
            .unwrap_or_else(|| panic!("missing removal {container:?}"));
        assert!(removal.refused.is_none(), "{container:?} {removal:?}");
        assert!(removal.selected);
        let result = service
            .apply(&plan.plan_id, noop(), never_cancel())
            .await
            .unwrap();
        assert!(result.failed.is_empty(), "{container:?} {result:?}");
        let stored = std::fs::read(&path).unwrap();
        assert_eq!(ape_audio_payload(&stored), payload, "{container:?}");
        use lofty::file::TaggedFileExt;
        let tagged = read_tagged(&path).unwrap();
        match container {
            AudioContainer::Apev2 => {
                assert!(tagged.tag(lofty::tag::TagType::Ape).is_none());
                assert!(tagged.tag(lofty::tag::TagType::Id3v1).is_some());
                assert_eq!(
                    read_id3v1(&stored, TextEncoding::Utf8)
                        .unwrap()
                        .unwrap()
                        .values["title"],
                    "v1-title"
                );
            }
            AudioContainer::Id3v1 => {
                assert!(read_id3v1(&stored, TextEncoding::Utf8).unwrap().is_none());
                assert_eq!(
                    tagged
                        .tag(lofty::tag::TagType::Ape)
                        .and_then(|tag| tag.title())
                        .as_deref(),
                    Some("ape-title")
                );
            }
            _ => {}
        }
    }

    let only = directory.join("ape-only.ape");
    std::fs::copy(&fixture, &only).unwrap();
    {
        let mut tagged = read_tagged(&only).unwrap();
        let mut ape = lofty::tag::Tag::new(lofty::tag::TagType::Ape);
        ape.set_title("only-ape".to_string());
        tagged.insert_tag(ape);
        tagged
            .save_to_path(&only, lofty::config::WriteOptions::default())
            .unwrap();
    }
    let before = std::fs::read(&only).unwrap();
    let mut request = quiet_plan(&[only.clone()]);
    request.tag_operations = vec![remove_operation(AudioContainer::Apev2)];
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    assert!(plan.files[0].removals[0].refused.is_some());
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    assert_eq!(std::fs::read(&only).unwrap(), before);

    let _ = std::fs::remove_dir_all(&directory);
}

/// 音訊段的 FNV-1a。刻意不用 `mp3_audio_payload`／`ape_audio_payload`，
/// 那兩條路跟 `strip_ape_tail` 共用邊界，無標頭時會一起少 32 bytes。
fn audio_segment_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn ape_tag_bytes(has_header: bool) -> Vec<u8> {
    let key = b"Title";
    let value = b"tail";
    let mut item = Vec::new();
    item.extend_from_slice(&(value.len() as u32).to_le_bytes());
    item.extend_from_slice(&0u32.to_le_bytes());
    item.extend_from_slice(key);
    item.push(0);
    item.extend_from_slice(value);
    let mut footer = Vec::new();
    footer.extend_from_slice(b"APETAGEX");
    footer.extend_from_slice(&2000u32.to_le_bytes());
    footer.extend_from_slice(&((item.len() + 32) as u32).to_le_bytes());
    footer.extend_from_slice(&1u32.to_le_bytes());
    // bit 31：含標頭。bit 30：這段是 footer 且標籤含 footer。
    let mut flags = 1u32 << 30;
    if has_header {
        flags |= 1 << 31;
    }
    footer.extend_from_slice(&flags.to_le_bytes());
    footer.extend_from_slice(&[0; 8]);
    let mut tag = Vec::new();
    if has_header {
        let mut header = footer.clone();
        let header_flags = flags | (1 << 29);
        header[20..24].copy_from_slice(&header_flags.to_le_bytes());
        tag.extend_from_slice(&header);
    }
    tag.extend_from_slice(&item);
    tag.extend_from_slice(&footer);
    tag
}

#[tokio::test]
async fn apev2_header_flag_roundtrip_hashes_audio_segment() {
    let directory = temp_dir();
    let seed = directory.join("seed.mp3");
    generate_audio(&seed, "libmp3lame");
    let encoded = std::fs::read(&seed).unwrap();
    let audio = strip_id3v1(&strip_id3v2(&encoded)).to_vec();
    assert!(audio.len() > 32, "{}", audio.len());
    let expected = audio_segment_hash(&audio);
    let service = AudioService::new();

    for has_header in [true, false] {
        for with_v1 in [true, false] {
            let path = directory.join(format!("ape-h{has_header}-v1{with_v1}.mp3"));
            std::fs::write(&path, &audio).unwrap();
            let mut tag = id3::Tag::new();
            tag.set_title("keep-v2");
            tag.write_to_path(&path, Version::Id3v24).unwrap();
            let mut bytes = std::fs::read(&path).unwrap();
            let audio_at = id3v2_tag_length(&bytes);
            assert_eq!(&bytes[audio_at..], audio.as_slice());
            bytes.extend_from_slice(&ape_tag_bytes(has_header));
            if with_v1 {
                bytes = append_id3v1(
                    &bytes,
                    &HashMap::from([("title", "keep-v1".to_string())]),
                    255,
                    TextEncoding::Utf8,
                )
                .unwrap();
            }
            std::fs::write(&path, &bytes).unwrap();

            let mut request = quiet_plan(&[path.clone()]);
            request.tag_operations = vec![remove_operation(AudioContainer::Apev2)];
            let plan = service
                .plan(shared_conversion(), request, noop())
                .await
                .unwrap();
            let removal = plan.files[0]
                .removals
                .iter()
                .find(|item| item.container == AudioContainer::Apev2)
                .unwrap_or_else(|| panic!("header={has_header} v1={with_v1} 沒有 APEv2"));
            assert!(
                removal.refused.is_none(),
                "header={has_header} v1={with_v1} {:?}",
                removal.refused
            );
            let result = service
                .apply(&plan.plan_id, noop(), never_cancel())
                .await
                .unwrap();
            assert!(
                result.failed.is_empty(),
                "header={has_header} v1={with_v1} {result:?}"
            );

            let stored = std::fs::read(&path).unwrap();
            assert!(
                !stored.windows(8).any(|window| window == b"APETAGEX"),
                "header={has_header} v1={with_v1} 仍留著 APEv2"
            );
            let start = id3v2_tag_length(&stored);
            let end = stored.len() - if with_v1 { 128 } else { 0 };
            let segment = &stored[start..end];
            assert_eq!(
                audio_segment_hash(segment),
                expected,
                "header={has_header} v1={with_v1} 音訊段 hash 不符（len {}，預期 {}）",
                segment.len(),
                audio.len()
            );
        }
    }

    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn big5_reports_unrepresentable_character() {
    let fit = fit_encoded_text("A🙂", 30, TextEncoding::Big5).unwrap();
    assert!(fit.unrepresentable >= 1, "{fit:?}");
    assert!(fit.text.contains('?'));
}

#[tokio::test]
async fn copy_id3v2_to_id3v1_reports_big5_truncation_and_unrepresentable() {
    let directory = temp_dir();
    let path = directory.join("copy.mp3");
    std::fs::write(&path, mpeg_frame()).unwrap();
    let title = format!("🙂{}", "測".repeat(16));
    let mut tag = id3::Tag::new();
    tag.set_title(&title);
    tag.set_genre("Rock");
    tag.write_to_path(&path, Version::Id3v24).unwrap();
    let service = AudioService::new();
    let mut request = quiet_plan(&[path.clone()]);
    request.id3v1_output_encoding = TextEncoding::Big5;
    request.tag_operations = vec![TagOperation {
        action: TagAction::Copy,
        source: Some(AudioContainer::Id3v2),
        target: Some(AudioContainer::Id3v1),
        fields: vec!["title".into(), "genre".into()],
        on_conflict: Some(TagOnConflict::Overwrite),
        enabled: true,
        paths: Vec::new(),
    }];
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    let title_field = field(&plan.files[0], AudioContainer::Id3v1, "title").unwrap();
    assert_eq!(title_field.origin.as_deref(), Some("rule 1"));
    assert!(title_field.truncated, "{title_field:?}");
    assert!(title_field.warnings.iter().any(|warning| {
        warning.code == AudioFieldWarningCode::Truncated
            && warning.from_bytes.unwrap_or(0) > warning.to_bytes.unwrap_or(0)
    }));
    let missing = title_field
        .warnings
        .iter()
        .find(|warning| warning.code == AudioFieldWarningCode::Unrepresentable)
        .unwrap_or_else(|| panic!("{title_field:?}"));
    assert!(missing.count.unwrap_or(0) >= 1);
    let written = title_field.converted_values.as_ref().unwrap()[0].clone();
    assert!(!written.contains('🙂'));
    assert!(written.contains('?'));
    let genre = field(&plan.files[0], AudioContainer::Id3v1, "genre").unwrap();
    assert_eq!(genre.converted_values.as_ref().unwrap()[0], "17");
    let before_payload = mp3_audio_payload(&std::fs::read(&path).unwrap());
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    let stored = std::fs::read(&path).unwrap();
    assert_eq!(mp3_audio_payload(&stored), before_payload);
    let tag = read_id3v1(&stored, TextEncoding::Big5).unwrap().unwrap();
    assert_eq!(tag.values["title"], written);
    assert_eq!(tag.values["genre"], "17");
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn removing_the_only_id3v2_tag_is_refused() {
    let directory = temp_dir();
    let path = directory.join("only-v2.mp3");
    std::fs::write(&path, mpeg_frame()).unwrap();
    let mut tag = id3::Tag::new();
    tag.set_title("only");
    tag.write_to_path(&path, Version::Id3v24).unwrap();
    let before = std::fs::read(&path).unwrap();
    let service = AudioService::new();
    let mut request = quiet_plan(&[path.clone()]);
    request.tag_operations = vec![remove_operation(AudioContainer::Id3v2)];
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    assert!(plan.files[0].removals[0].refused.is_some());
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn manual_path_operation_stays_on_that_file_and_marks_origin_manual() {
    let directory = temp_dir();
    let first = directory.join("first.mp3");
    let second = directory.join("second.mp3");
    for path in [&first, &second] {
        std::fs::write(path, mpeg_frame()).unwrap();
        let mut tag = id3::Tag::new();
        tag.set_title("v2-title");
        tag.write_to_path(path, Version::Id3v24).unwrap();
        let output = append_id3v1(
            &strip_id3v1(&std::fs::read(path).unwrap()),
            &HashMap::from([
                ("title", "v1-title".to_string()),
                ("artist", "v1-artist".to_string()),
            ]),
            255,
            TextEncoding::Utf8,
        )
        .unwrap();
        std::fs::write(path, output).unwrap();
    }
    let service = AudioService::new();
    let mut request = quiet_plan(&[first.clone(), second.clone()]);
    request.tag_operations = vec![
        TagOperation {
            action: TagAction::Copy,
            source: Some(AudioContainer::Id3v2),
            target: Some(AudioContainer::Id3v1),
            fields: vec!["artist".into()],
            on_conflict: Some(TagOnConflict::Overwrite),
            enabled: false,
            paths: Vec::new(),
        },
        TagOperation {
            action: TagAction::Copy,
            source: Some(AudioContainer::Id3v1),
            target: Some(AudioContainer::Id3v2),
            fields: vec!["title".into(), "artist".into()],
            on_conflict: None,
            enabled: true,
            paths: vec![first.to_string_lossy().into_owned()],
        },
        TagOperation {
            action: TagAction::Copy,
            source: Some(AudioContainer::Id3v2),
            target: Some(AudioContainer::Id3v1),
            fields: vec!["title".into()],
            on_conflict: Some(TagOnConflict::Overwrite),
            enabled: true,
            paths: Vec::new(),
        },
    ];
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    let first_file = plan
        .files
        .iter()
        .find(|file| file.path.ends_with("first.mp3"))
        .unwrap();
    let second_file = plan
        .files
        .iter()
        .find(|file| file.path.ends_with("second.mp3"))
        .unwrap();
    let kept = field(first_file, AudioContainer::Id3v2, "title").unwrap();
    // ID3v1 當來源且沒指定衝突策略時只補空白，不會蓋掉已有的 v2 標題。
    assert!(kept.converted_values.is_none(), "{kept:?}");
    assert!(kept.origin.is_none(), "{kept:?}");
    let manual = field(first_file, AudioContainer::Id3v2, "artist").unwrap();
    assert_eq!(manual.origin.as_deref(), Some("manual"));
    assert_eq!(manual.converted_values.as_ref().unwrap()[0], "v1-artist");
    let rule = field(first_file, AudioContainer::Id3v1, "title").unwrap();
    assert_eq!(rule.origin.as_deref(), Some("rule 2"));
    assert_eq!(rule.converted_values.as_ref().unwrap()[0], "v2-title");
    assert!(field(second_file, AudioContainer::Id3v2, "title")
        .and_then(|item| item.origin.clone())
        .is_none());
    let second_rule = field(second_file, AudioContainer::Id3v1, "title").unwrap();
    assert_eq!(second_rule.origin.as_deref(), Some("rule 2"));
    let _ = std::fs::remove_dir_all(&directory);
}

/// 合法的 ID3v2.3 標頭，但影格設了加密旗標。id3 crate 讀整份標籤會失敗，
/// 寫入時卻只靠標頭長度整段換掉。
fn unreadable_id3v2_with_v1(title: &str) -> Vec<u8> {
    let payload = b"ORIGINAL-ID3-BODY";
    let mut frame = b"TALB".to_vec();
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(&0x0040u16.to_be_bytes());
    frame.extend_from_slice(payload);
    let mut tag = b"ID3".to_vec();
    tag.extend_from_slice(&[3, 0, 0, 0, 0, 0, frame.len() as u8]);
    tag.extend_from_slice(&frame);
    tag.extend_from_slice(&mpeg_frame());
    append_id3v1(
        &tag,
        &HashMap::from([("title", title.to_string())]),
        255,
        TextEncoding::Utf8,
    )
    .unwrap()
}

#[tokio::test]
async fn unreadable_id3v2_is_not_replaced_with_an_empty_tag() {
    let directory = temp_dir();
    let path = directory.join("opaque.mp3");
    let original = unreadable_id3v2_with_v1("SourceTitle");
    std::fs::write(&path, &original).unwrap();
    assert!(id3::Tag::read_from_path(&path).is_err());
    assert!(original
        .windows(17)
        .any(|window| window == b"ORIGINAL-ID3-BODY"));
    let service = AudioService::new();
    let mut request = quiet_plan(&[path.clone()]);
    request.tag_operations = vec![TagOperation {
        action: TagAction::Copy,
        source: Some(AudioContainer::Id3v1),
        target: Some(AudioContainer::Id3v2),
        fields: vec!["title".into()],
        on_conflict: Some(TagOnConflict::Overwrite),
        enabled: true,
        paths: Vec::new(),
    }];
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    let result = service
        .apply(&plan.plan_id, noop(), never_cancel())
        .await
        .unwrap();
    assert_eq!(result.failed.len(), 1, "{result:?}");
    assert!(result.succeeded.is_empty(), "{result:?}");
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let _ = std::fs::remove_dir_all(&directory);
}

/// ID3v2 讀不到、只改 ID3v1 時，picture_count 的 `?` 會中斷寫入。
/// 暫存檔必須清掉，不能留在原檔旁邊。
#[test]
fn apply_mp3_error_removes_temporary_file() {
    let directory = temp_dir();
    let path = directory.join("opaque.mp3");
    let original = unreadable_id3v2_with_v1("SourceTitle");
    std::fs::write(&path, &original).unwrap();
    let mut request = base_plan(&[path.clone()]);
    request.id3v2_enabled = false;
    let file = PreparedAudio {
        path: path.clone(),
        format: AudioFormat::Mp3,
        updates: HashMap::from([("id3v1:title".into(), vec!["裡面".into()])]),
        selected_containers: HashSet::from([AudioContainer::Id3v1]),
        rewrite_containers: HashSet::new(),
        remove_containers: HashSet::new(),
        request,
        original_picture_count: 0,
    };
    let error = apply_mp3(&file).expect_err("壞掉的 ID3v2 不該完成寫入");
    assert_eq!(std::fs::read(&path).unwrap(), original, "{error}");
    let leftovers: Vec<String> = std::fs::read_dir(&directory)
        .unwrap()
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            name.starts_with(".convertzz-audio-").then_some(name)
        })
        .collect();
    assert!(
        leftovers.is_empty(),
        "寫入失敗後仍留下暫存檔 {leftovers:?}：{error}"
    );
    let _ = std::fs::remove_dir_all(&directory);
}

fn id3v2_blob(title: &str) -> Vec<u8> {
    let mut tag = id3::Tag::new();
    tag.set_title(title);
    let mut cursor = std::io::Cursor::new(Vec::new());
    tag.write_to(&mut cursor, Version::Id3v24).unwrap();
    cursor.into_inner()
}

/// 開頭已有一個 ID3v2，後面還留著第二個 v2、檔尾 v2 或 Lyrics3v2。
/// 移除只剝第一個標頭，卻仍回報成功。
#[tokio::test]
async fn removing_id3v2_reports_failure_when_another_container_remains() {
    let directory = temp_dir();
    let audio = mpeg_frame();
    let leading = id3v2_blob("第一個");
    let extra = id3v2_blob("第二個");
    let lyrics = b"LYRICSBEGININD000021LYRICS200000021".to_vec();
    let with_v1 = |body: Vec<u8>| {
        append_id3v1(
            &body,
            &HashMap::from([("title", "v1".to_string())]),
            255,
            TextEncoding::Utf8,
        )
        .unwrap()
    };
    // 同時留 ID3v1，移除 ID3v2 才不會被當成「唯一標籤」而拒絕。
    let cases = [
        (
            "double-leading",
            with_v1([leading.clone(), extra.clone(), audio.clone()].concat()),
        ),
        (
            "trailing",
            with_v1([leading.clone(), audio.clone(), extra].concat()),
        ),
        ("lyrics3", with_v1([leading, audio, lyrics].concat())),
    ];
    let service = AudioService::new();
    for (name, original) in cases {
        let path = directory.join(format!("{name}.mp3"));
        std::fs::write(&path, &original).unwrap();
        let mut request = quiet_plan(&[path.clone()]);
        request.tag_operations = vec![remove_operation(AudioContainer::Id3v2)];
        let plan = service
            .plan(shared_conversion(), request, noop())
            .await
            .unwrap();
        let removal = plan.files[0]
            .removals
            .iter()
            .find(|item| item.container == AudioContainer::Id3v2)
            .unwrap_or_else(|| panic!("{name} 應列出 ID3v2 移除"));
        assert!(removal.refused.is_none(), "{name} {:?}", removal.refused);
        let result = service
            .apply(&plan.plan_id, noop(), never_cancel())
            .await
            .unwrap();
        assert_eq!(result.failed.len(), 1, "{name} {result:?}");
        assert!(result.succeeded.is_empty(), "{name} {result:?}");
        assert!(
            result.failed[0].message.contains("ID3v2")
                || result.failed[0].message.contains("Lyrics3"),
            "{name} {}",
            result.failed[0].message
        );
        assert_eq!(std::fs::read(&path).unwrap(), original, "{name}");
    }
    let _ = std::fs::remove_dir_all(&directory);
}

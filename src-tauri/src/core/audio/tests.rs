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
    let formats: std::collections::BTreeSet<_> = scanned.iter().map(|file| file.format).collect();
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
            file.path.ends_with(".oga") && file.format == AudioFormat::Ogg && file.warning.is_none()
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

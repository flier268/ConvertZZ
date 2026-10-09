use super::super::conversion::shared_conversion;
use super::super::encoding::encode_text;
use super::super::headless::{extensions_from_type_filter, ParsedTypeExtensions};
use super::super::settings::migrate;
use super::super::types::{ConversionOptions, Direction, EngineKind, FileMode};
use super::*;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::atomic::Ordering;
use uuid::Uuid;

fn noop() -> ProgressReporter {
    Arc::new(|_| {})
}

fn never_cancel() -> CancelCheck {
    Arc::new(|| false)
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
        include_ignored: false,
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
    // 用可穩定字形轉換的字，避免無標點長串「里面」依賴同義詞分詞。
    let source = "软件".repeat(800);
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
                include_ignored: false,
                conversion: conversion_s2t(),
            },
            noop(),
        )
        .await
        .unwrap();
    assert!(!plan.items[0].preview_loaded);
    assert!(plan.items[0].source_preview.is_empty());
    let previewed = service
        .preview(
            shared_conversion(),
            FilePreviewRequest {
                plan_id: plan.plan_id.clone(),
                source_path: path.to_string_lossy().into_owned(),
            },
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert!(previewed.preview_loaded);
    assert_eq!(
        previewed.source_preview,
        source.chars().take(1024).collect::<String>()
    );
    assert_eq!(previewed.output_preview.chars().count(), 1024);
    let result = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    let written = std::fs::read(&path).unwrap();
    assert_eq!(&written[..3], &[0xef, 0xbb, 0xbf]);
    assert_eq!(
        String::from_utf8(written[3..].to_vec()).unwrap(),
        "軟件".repeat(800)
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
                include_ignored: false,
                conversion: conversion_s2t(),
            },
            noop(),
        )
        .await
        .unwrap();
    assert!(!plan.items[0].preview_loaded);
    let previewed = service
        .preview(
            shared_conversion(),
            FilePreviewRequest {
                plan_id: plan.plan_id.clone(),
                source_path: path.to_string_lossy().into_owned(),
            },
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert!(previewed.source_preview.contains("里面开发"));
    assert!(previewed.output_preview.contains("裡面開發"));
    let result = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
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
    let result = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
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
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "PLAN_NOT_FOUND");
    assert_eq!(std::fs::read_to_string(&source).unwrap(), "來源");
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn request_cancel_stops_apply_before_write() {
    let directory = temp_dir();
    let source = directory.join("demo.txt");
    std::fs::write(&source, "里面").unwrap();
    let service = FileService::new();
    let plan = service
        .plan(
            shared_conversion(),
            FilePlanRequest {
                paths: vec![source.to_string_lossy().into_owned()],
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
                include_ignored: false,
                conversion: conversion_s2t(),
            },
            noop(),
        )
        .await
        .unwrap();
    let request_cancelled: CancelCheck = Arc::new(|| true);
    let error = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            request_cancelled,
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "PLAN_CANCELLED");
    assert_eq!(std::fs::read_to_string(&source).unwrap(), "里面");
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
    let result = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
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
                include_ignored: false,
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
    let result = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
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
                include_ignored: false,
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
    let result = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
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
                include_ignored: false,
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
async fn later_file_failure_keeps_earlier_writes() {
    let directory = temp_dir();
    let first = directory.join("甲.txt");
    let second = directory.join("乙.txt");
    std::fs::write(&first, "里面开发").unwrap();
    std::fs::write(&second, "软件测试").unwrap();
    let service = FileService::new().with_stage_validator({
        let second = second.clone();
        move |stage, _, source| {
            if source == second {
                let _ = std::fs::remove_file(stage);
                return Err(CoreError::new("FILE_VERIFY", "受控第二檔失敗"));
            }
            Ok(())
        }
    });
    let plan = service
        .plan(
            shared_conversion(),
            FilePlanRequest {
                paths: vec![
                    first.to_string_lossy().into_owned(),
                    second.to_string_lossy().into_owned(),
                ],
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
                include_ignored: false,
                conversion: conversion_s2t(),
            },
            noop(),
        )
        .await
        .unwrap();
    let result = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert_eq!(result.succeeded.len(), 1, "{result:?}");
    assert_eq!(result.failed.len(), 1, "{result:?}");
    assert_eq!(result.failed[0].path, second.to_string_lossy().into_owned());
    assert_eq!(std::fs::read_to_string(&first).unwrap(), "裡面開發");
    assert_eq!(std::fs::read_to_string(&second).unwrap(), "软件测试");
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn cancel_after_first_file_keeps_written_changes() {
    // 強制序列，才能在第一檔寫入後停止並保留變更。
    std::env::set_var("CONVERTZZ_CONVERT_JOBS", "1");
    let directory = temp_dir();
    let first = directory.join("一.txt");
    let second = directory.join("二.txt");
    std::fs::write(&first, "里面开发").unwrap();
    std::fs::write(&second, "软件测试").unwrap();
    let service = FileService::new();
    let plan = service
        .plan(
            shared_conversion(),
            FilePlanRequest {
                paths: vec![
                    first.to_string_lossy().into_owned(),
                    second.to_string_lossy().into_owned(),
                ],
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
                include_ignored: false,
                conversion: conversion_s2t(),
            },
            noop(),
        )
        .await
        .unwrap();
    let cancel_after_first = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let cancel_flag = Arc::clone(&cancel_after_first);
    let progress: ProgressReporter = Arc::new(move |event| {
        if event.message.starts_with("已寫入：") {
            cancel_flag.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    });
    let request_cancelled: CancelCheck = Arc::new({
        let cancel_after_first = Arc::clone(&cancel_after_first);
        move || cancel_after_first.load(std::sync::atomic::Ordering::SeqCst)
    });
    let result = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            progress,
            request_cancelled,
        )
        .await
        .unwrap();
    assert_eq!(result.succeeded.len(), 1, "{result:?}");
    assert!(result
        .skipped
        .iter()
        .any(|path| path == &second.to_string_lossy().into_owned()));
    assert_eq!(std::fs::read_to_string(&first).unwrap(), "裡面開發");
    assert_eq!(std::fs::read_to_string(&second).unwrap(), "软件测试");
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
    let applied = service
        .apply(
            shared_conversion(),
            &result.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
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
    // 純改名不經暫存檔（同一磁碟區直接 rename）；用 both 模式走寫入暫存檔的路徑。
    let mut request = filename_request(&source, ConflictPolicy::Overwrite);
    request.mode = FileMode::Both;
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    let result = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert_eq!(result.failed.len(), 1);
    assert_eq!(result.failed[0].path, source.to_string_lossy().into_owned());
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
    let result = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert_eq!(result.failed[0].path, source.to_string_lossy().into_owned());
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
                include_ignored: false,
                conversion: conversion_s2t(),
            },
            noop(),
        )
        .await
        .unwrap();
    let result = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
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
                include_ignored: false,
                conversion: conversion_s2t(),
            },
            noop(),
        )
        .await
        .unwrap();
    let result = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
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
        std::fs::read_to_string(PathBuf::from(format!("{}.bak", folder.display())).join("one.txt"))
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
                include_ignored: false,
                conversion: conversion_s2t(),
            },
            noop(),
        )
        .await
        .unwrap();
    service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert_eq!(names(&directory), ["note.txt"]);
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn both_mode_converts_content_and_filename() {
    let directory = temp_dir();
    let source = directory.join("里面.txt");
    std::fs::write(&source, "里面开发").unwrap();
    let service = FileService::new();
    let plan = service
        .plan(
            shared_conversion(),
            FilePlanRequest {
                paths: vec![source.to_string_lossy().into_owned()],
                output_path: None,
                output_directory: None,
                mode: FileMode::Both,
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
                include_ignored: false,
                conversion: conversion_s2t(),
            },
            noop(),
        )
        .await
        .unwrap();
    assert_eq!(plan.items.len(), 1);
    assert!(!plan.items[0].preview_loaded);
    assert_eq!(
        plan.items[0].output_path,
        directory.join("裡面.txt").to_string_lossy().into_owned()
    );
    let previewed = service
        .preview(
            shared_conversion(),
            FilePreviewRequest {
                plan_id: plan.plan_id.clone(),
                source_path: source.to_string_lossy().into_owned(),
            },
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert!(previewed.source_preview.contains("里面开发"));
    assert!(previewed.output_preview.contains("裡面開發"));
    let result = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    assert!(!source.exists());
    let output = directory.join("裡面.txt");
    assert_eq!(std::fs::read_to_string(&output).unwrap(), "裡面開發");
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn cancel_rejects_content_plan_without_writing() {
    let directory = temp_dir();
    let source = directory.join("note.txt");
    std::fs::write(&source, "里面开发").unwrap();
    let before = std::fs::read(&source).unwrap();
    let service = FileService::new();
    let plan = service
        .plan(
            shared_conversion(),
            FilePlanRequest {
                paths: vec![source.to_string_lossy().into_owned()],
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
                include_ignored: false,
                conversion: conversion_s2t(),
            },
            noop(),
        )
        .await
        .unwrap();
    let previewed = service
        .preview(
            shared_conversion(),
            FilePreviewRequest {
                plan_id: plan.plan_id.clone(),
                source_path: source.to_string_lossy().into_owned(),
            },
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert!(previewed.output_preview.contains("裡面開發"));
    assert_eq!(service.cancel(&plan.plan_id)["cancelled"], true);
    let error = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "PLAN_NOT_FOUND");
    assert_eq!(std::fs::read(&source).unwrap(), before);
    let _ = std::fs::remove_dir_all(&directory);
}

#[cfg(unix)]
#[tokio::test]
async fn recursive_scan_skips_file_symlinks() {
    let directory = temp_dir();
    let outside = temp_dir();
    std::fs::write(directory.join("inside.txt"), "內部").unwrap();
    let linked_target = outside.join("outside.txt");
    std::fs::write(&linked_target, "外部").unwrap();
    std::os::unix::fs::symlink(&linked_target, directory.join("linked.txt")).unwrap();
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
    let _ = std::fs::remove_dir_all(&directory);
    let _ = std::fs::remove_dir_all(&outside);
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
    let result = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(result.failed.len(), 1);
    assert_eq!(std::fs::read_to_string(&source).unwrap(), "來源");
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn content_plan_lists_without_converting() {
    let directory = temp_dir();
    let first = directory.join("a.txt");
    let second = directory.join("b.txt");
    std::fs::write(&first, "里面开发").unwrap();
    std::fs::write(&second, "头发").unwrap();
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = calls.clone();
    let service = FileService::new().with_convert_hook(move |text| {
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        text.replace('里', "裡")
            .replace("开发", "開發")
            .replace("头发", "頭髮")
    });
    let plan = service
        .plan(
            shared_conversion(),
            FilePlanRequest {
                paths: vec![directory.to_string_lossy().into_owned()],
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
                include_ignored: false,
                conversion: conversion_s2t(),
            },
            noop(),
        )
        .await
        .unwrap();
    assert_eq!(plan.items.len(), 2);
    assert!(plan
        .items
        .iter()
        .all(|item| item.selected && !item.preview_loaded));
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    let previewed = service
        .preview(
            shared_conversion(),
            FilePreviewRequest {
                plan_id: plan.plan_id.clone(),
                source_path: first.to_string_lossy().into_owned(),
            },
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert!(previewed.preview_loaded);
    assert!(previewed.output_preview.contains("裡"));
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn preview_rejects_path_outside_plan() {
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
                include_ignored: false,
                conversion: conversion_s2t(),
            },
            noop(),
        )
        .await
        .unwrap();
    let error = service
        .preview(
            shared_conversion(),
            FilePreviewRequest {
                plan_id: plan.plan_id,
                source_path: directory.join("missing.txt").to_string_lossy().into_owned(),
            },
            noop(),
            never_cancel(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "PLAN_PATH");
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn apply_only_writes_selected_files() {
    let directory = temp_dir();
    let first = directory.join("a.txt");
    let second = directory.join("b.txt");
    std::fs::write(&first, "里面").unwrap();
    std::fs::write(&second, "头发").unwrap();
    let service = FileService::new();
    let plan = service
        .plan(
            shared_conversion(),
            FilePlanRequest {
                paths: vec![directory.to_string_lossy().into_owned()],
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
                include_ignored: false,
                conversion: conversion_s2t(),
            },
            noop(),
        )
        .await
        .unwrap();
    let selected = vec![first.to_string_lossy().into_owned()];
    let result = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            Some(selected.as_slice()),
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    assert_eq!(std::fs::read_to_string(&first).unwrap(), "裡面");
    assert_eq!(std::fs::read_to_string(&second).unwrap(), "头发");
    let _ = std::fs::remove_dir_all(&directory);
}

fn audio_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures")
        .join(name)
}

fn assert_text(bytes: &[u8], encoding: TextEncoding) {
    match super::inspect_file_bytes(bytes, encoding).unwrap() {
        super::InspectedBytes::Text { .. } => {}
        other => panic!("不應把文字判成 {other:?}"),
    }
}

fn assert_skip(bytes: &[u8], encoding: TextEncoding, expected: super::ContentSkip) {
    match super::inspect_file_bytes(bytes, encoding).unwrap() {
        super::InspectedBytes::Skip(reason) => assert_eq!(reason, expected),
        super::InspectedBytes::Text { text, .. } => panic!("不應當成文字：{text:?}"),
    }
}

#[test]
fn binary_detection_rejects_nul_magic_and_replacement_chars() {
    assert_skip(b"abc\0def", TextEncoding::Utf8, super::ContentSkip::Binary);
    assert_skip(
        b"ID3\x03\x00\x00\x00\x00\x00\x00rest",
        TextEncoding::Auto,
        super::ContentSkip::Binary,
    );
    // MPEG1 Layer III、128 kbps、44100 Hz、無 padding，幀長 417。短於一幀不算音訊。
    assert!(!super::has_binary_magic(b"\xFF\xFB\x90\x64AAAA"));
    let mut frame = vec![0xFF, 0xFB, 0x90, 0x64];
    frame.resize(417, 0);
    assert_eq!(super::mpeg_frame_length(&frame), Some(417));
    assert_skip(&frame, TextEncoding::Utf8, super::ContentSkip::Binary);
    assert_skip(
        &std::fs::read(audio_fixture("测试音乐b.mp3")).unwrap(),
        TextEncoding::Utf8,
        super::ContentSkip::Binary,
    );
    assert_skip(
        b"fLaC\x00\x00\x00\x22",
        TextEncoding::Utf8,
        super::ContentSkip::Binary,
    );
    assert_skip(
        b"OggS\x00\x02\x00\x00",
        TextEncoding::Utf8,
        super::ContentSkip::Binary,
    );
    assert_skip(
        b"RIFF\x24\x00\x00\x00WAVE",
        TextEncoding::Utf8,
        super::ContentSkip::Binary,
    );
    assert_skip(
        b"RIFF\x24\x00\x00\x00AVI ",
        TextEncoding::Utf8,
        super::ContentSkip::Binary,
    );
    assert_skip(
        b"RIFF\x24\x00\x00\x00WEBP",
        TextEncoding::Utf8,
        super::ContentSkip::Binary,
    );
    assert_skip(
        b"\x89PNG\r\n\x1a\nrest",
        TextEncoding::Utf8,
        super::ContentSkip::Binary,
    );
    assert_skip(
        b"\xFF\xD8\xFF\xE0JFIF",
        TextEncoding::Utf8,
        super::ContentSkip::Binary,
    );
    assert_skip(
        b"PK\x03\x04\x14\x01",
        TextEncoding::Utf8,
        super::ContentSkip::Binary,
    );
    assert_skip(
        b"%PDF-1.4\n",
        TextEncoding::Utf8,
        super::ContentSkip::Binary,
    );
    let mut m4a = vec![0x00, 0x00, 0x00, 0x14];
    m4a.extend_from_slice(b"ftypM4A ");
    m4a.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(m4a.len(), 20);
    assert_skip(&m4a, TextEncoding::Utf8, super::ContentSkip::Binary);
    assert_skip(
        b"MThd\x00\x00\x00\x06",
        TextEncoding::Utf8,
        super::ContentSkip::Binary,
    );
    assert_skip(
        b"caff\x00\x01",
        TextEncoding::Utf8,
        super::ContentSkip::Binary,
    );
    let ape = std::fs::read(audio_fixture("mac-399.ape")).unwrap();
    assert!(ape.starts_with(b"MAC "));
    assert_skip(&ape, TextEncoding::Utf8, super::ContentSkip::Binary);

    let replacements = "\u{FFFD}".repeat(8);
    assert_text(replacements.as_bytes(), TextEncoding::Utf8);
    assert!(super::replacement_ratio_too_high(&replacements));
    let mut utf16_replacements = vec![0xFF, 0xFE];
    for _ in 0..8 {
        utf16_replacements.extend(0xFFFDu16.to_le_bytes());
    }
    assert_skip(
        &utf16_replacements,
        TextEncoding::Utf16le,
        super::ContentSkip::Binary,
    );
    let rare = format!("{}{}", "測試".repeat(80), "\u{FFFD}");
    assert!(!super::replacement_ratio_too_high(&rare));
    assert_text(rare.as_bytes(), TextEncoding::Utf8);

    assert_skip(
        b"abc\x80\x80xyz",
        TextEncoding::Utf8,
        super::ContentSkip::DecodeError,
    );
}

#[test]
fn binary_detection_keeps_plain_text() {
    let utf8 = "檔案轉換測試，軟體裡面開發。\n";
    assert_text(utf8.as_bytes(), TextEncoding::Utf8);
    assert_text(utf8.as_bytes(), TextEncoding::Auto);

    let (gbk, _, gbk_errors) = encoding_rs::GBK.encode("软件测试，里面开发。");
    assert!(!gbk_errors);
    assert_text(&gbk, TextEncoding::Gbk);
    assert_text(&gbk, TextEncoding::Auto);

    let (big5, _, big5_errors) = encoding_rs::BIG5.encode("軟體測試，裡面開發。");
    assert!(!big5_errors);
    assert_text(&big5, TextEncoding::Big5);
    assert_text(&big5, TextEncoding::Auto);

    let mut utf16 = vec![0xFF, 0xFE];
    for unit in "測試ab".encode_utf16() {
        utf16.extend(unit.to_le_bytes());
    }
    assert!(utf16.contains(&0));
    assert_text(&utf16, TextEncoding::Auto);
    assert_text(&utf16, TextEncoding::Utf16le);

    assert_text("caffeine 软件".as_bytes(), TextEncoding::Auto);
    assert_text("MAC 地址列表".as_bytes(), TextEncoding::Auto);
    assert_text(
        "RIFF 是一種容器格式，不是 WAVE 音訊。".as_bytes(),
        TextEncoding::Auto,
    );
    assert_text(
        "ID3 標籤只是這份說明的開頭。".as_bytes(),
        TextEncoding::Auto,
    );
    assert_text(
        "%PDF 不是檔頭，後面沒有版本號。".as_bytes(),
        TextEncoding::Auto,
    );
    assert_text(b"    ftypM4A text", TextEncoding::Utf8);
    assert!(!super::has_binary_magic(b"MThd notes"));
    assert!(!super::has_binary_magic(b"caffeine"));
    assert!(!super::has_binary_magic(b"wvpktext!!"));
    assert!(!super::has_binary_magic(b"FLV\x01"));
    assert!(!super::has_binary_magic(b"\x30\x26\xB2\x75"));

    let mut yen = Vec::new();
    for unit in "￥這是一份純文字，不是音訊檔。軟體測試裡面開發。".encode_utf16()
    {
        yen.extend(unit.to_be_bytes());
    }
    assert!(yen.starts_with(&[0xFF, 0xE5]), "{:02x?}", &yen[..2]);
    assert!(!super::has_binary_magic(&yen));
    assert_text(&yen, TextEncoding::Utf16be);
}

#[test]
fn mp3_fixtures_keep_id3_or_mpeg_headers() {
    // 產生方式見 tests/fixtures/README.md。
    let tagged = std::fs::read(audio_fixture("测试音乐.mp3")).unwrap();
    assert!(tagged.starts_with(b"ID3"), "{:02x?}", &tagged[..4]);
    let raw = std::fs::read(audio_fixture("测试音乐b.mp3")).unwrap();
    assert!(
        raw.len() >= 2 && raw[0] == 0xFF && raw[1] & 0xE0 == 0xE0,
        "{:02x?}",
        &raw[..4]
    );
    assert!(!raw.starts_with(b"ID3"));
}

fn binary_file_request(path: &Path, mode: FileMode) -> FilePlanRequest {
    FilePlanRequest {
        paths: vec![path.to_string_lossy().into_owned()],
        output_path: None,
        output_directory: None,
        mode,
        recursive: false,
        input_encoding: TextEncoding::Auto,
        output_encoding: TextEncoding::Utf8,
        add_bom: false,
        fix_charset_declaration: false,
        fix_charset_extensions: None,
        allowed_extensions: None,
        preview_max_bytes: Some(4096),
        conflict_policy: ConflictPolicy::Skip,
        backup: Some(false),
        include_ignored: false,
        conversion: ConversionOptions {
            vocabulary_correction: Some(false),
            ..conversion_s2t()
        },
    }
}

async fn preview_and_apply(
    path: &Path,
    mode: FileMode,
) -> (
    super::super::types::ApplyResult,
    super::super::types::FilePlanItem,
) {
    let service = FileService::new();
    let plan = service
        .plan(shared_conversion(), binary_file_request(path, mode), noop())
        .await
        .unwrap();
    let previewed = service
        .preview(
            shared_conversion(),
            FilePreviewRequest {
                plan_id: plan.plan_id.clone(),
                source_path: path.to_string_lossy().into_owned(),
            },
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    let result = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    (result, previewed)
}

fn assert_binary_preview(previewed: &super::super::types::FilePlanItem) {
    let warning = previewed.warning.as_deref().unwrap_or("");
    assert!(
        warning.contains("二進位") && warning.contains("已略過內容轉換"),
        "{warning}"
    );
    assert!(
        previewed.source_preview.is_empty(),
        "{:?}",
        previewed.source_preview
    );
    assert!(
        previewed.output_preview.is_empty(),
        "{:?}",
        previewed.output_preview
    );
    assert!(!previewed.source_preview.contains('\u{FFFD}'));
    assert!(!previewed.output_preview.contains('\u{FFFD}'));
}

async fn assert_mp3_content_and_name(source_name: &str, converted_name: &str) {
    let original = std::fs::read(audio_fixture(source_name)).unwrap();
    let directory = temp_dir();
    let source = directory.join(source_name);
    std::fs::write(&source, &original).unwrap();

    let (content_result, content_preview) = preview_and_apply(&source, FileMode::Content).await;
    assert_binary_preview(&content_preview);
    assert!(
        content_result
            .warnings
            .iter()
            .any(|warning| warning.contains("已略過內容轉換")),
        "{content_result:?}"
    );
    assert!(content_result.succeeded.is_empty(), "{content_result:?}");
    assert!(content_result.failed.is_empty(), "{content_result:?}");
    assert_eq!(std::fs::read(&source).unwrap(), original);

    let renamed = directory.join(converted_name);
    std::fs::remove_file(&source).unwrap();
    std::fs::write(&source, &original).unwrap();
    let (both_result, both_preview) = preview_and_apply(&source, FileMode::Both).await;
    assert_binary_preview(&both_preview);
    assert!(
        both_result
            .warnings
            .iter()
            .any(|warning| warning.contains("已略過內容轉換")),
        "{both_result:?}"
    );
    assert!(both_result.failed.is_empty(), "{both_result:?}");
    assert_eq!(std::fs::read(&renamed).unwrap(), original);
    assert!(!source.exists());
    assert!(both_result
        .succeeded
        .iter()
        .any(|path| path.ends_with(converted_name)));
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn mp3_fixtures_skip_content_and_only_rename_in_both_mode() {
    // 產生方式見 tests/fixtures/README.md（ffmpeg 正弦波，含 ID3 與純 MPEG 幀）。
    assert_mp3_content_and_name("测试音乐.mp3", "測試音樂.mp3").await;
    assert_mp3_content_and_name("测试音乐b.mp3", "測試音樂b.mp3").await;
}

#[tokio::test]
async fn decode_errors_are_not_written_back() {
    let directory = temp_dir();
    let source = directory.join("broken.txt");
    let original = b"abc\x80\x80xyz".to_vec();
    std::fs::write(&source, &original).unwrap();
    let (result, previewed) = preview_and_apply(&source, FileMode::Content).await;
    let warning = previewed.warning.as_deref().unwrap_or("");
    assert!(warning.contains("解碼時發生錯誤"), "{warning}");
    assert!(previewed.source_preview.is_empty());
    assert!(
        result
            .warnings
            .iter()
            .any(|warning| warning.contains("解碼時發生錯誤")),
        "{result:?}"
    );
    assert_eq!(std::fs::read(&source).unwrap(), original);
    assert!(result.succeeded.is_empty());
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn utf16_odd_length_and_lone_surrogate_are_not_written_back() {
    let directory = temp_dir();
    for (name, mut original) in [
        ("odd.txt", utf16le_with_bom("里面")),
        ("surrogate.txt", utf16le_with_bom("里面")),
    ] {
        if name.starts_with("odd") {
            original.push(0x42);
        } else {
            original.extend_from_slice(&0xD800u16.to_le_bytes());
        }
        let source = directory.join(name);
        std::fs::write(&source, &original).unwrap();
        let (result, previewed) = preview_and_apply(&source, FileMode::Content).await;
        let warning = previewed.warning.as_deref().unwrap_or("");
        assert!(warning.contains("解碼時發生錯誤"), "{name}: {warning}");
        assert_eq!(std::fs::read(&source).unwrap(), original, "{name}");
        assert!(result.succeeded.is_empty(), "{name}: {result:?}");
        assert!(result.failed.is_empty(), "{name}: {result:?}");
    }
    let _ = std::fs::remove_dir_all(&directory);
}

fn utf16le_with_bom(text: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        bytes.extend(unit.to_le_bytes());
    }
    bytes
}

#[tokio::test]
async fn overlapping_rename_keeps_binary_bytes() {
    let directory = temp_dir();
    let first = directory.join("甲.mp3");
    let second = directory.join("乙.mp3");
    let first_bytes = std::fs::read(audio_fixture("测试音乐.mp3")).unwrap();
    let second_bytes = std::fs::read(audio_fixture("测试音乐b.mp3")).unwrap();
    std::fs::write(&first, &first_bytes).unwrap();
    std::fs::write(&second, &second_bytes).unwrap();
    let service = FileService::new().with_convert_hook(|text| match text {
        "甲.mp3" => "乙.mp3".into(),
        "乙.mp3" => "甲.mp3".into(),
        other => other.into(),
    });
    let mut request = binary_file_request(&directory, FileMode::Both);
    request.conflict_policy = ConflictPolicy::Overwrite;
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    let result = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    assert_eq!(std::fs::read(&first).unwrap(), second_bytes);
    assert_eq!(std::fs::read(&second).unwrap(), first_bytes);
    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.contains("已略過內容轉換")));
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn binary_output_directory_and_backup_keep_bytes() {
    let directory = temp_dir();
    let output_directory = directory.join("out");
    let source = directory.join("测试音乐.mp3");
    let original = std::fs::read(audio_fixture("测试音乐b.mp3")).unwrap();
    std::fs::write(&source, &original).unwrap();
    let backup_path = PathBuf::from(format!("{}.bak", source.display()));

    let mut content_request = binary_file_request(&source, FileMode::Content);
    content_request.output_directory = Some(output_directory.to_string_lossy().into_owned());
    content_request.backup = Some(true);
    let service = FileService::new();
    let content_plan = service
        .plan(shared_conversion(), content_request, noop())
        .await
        .unwrap();
    let content_result = service
        .apply(
            shared_conversion(),
            &content_plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert!(content_result.failed.is_empty(), "{content_result:?}");
    assert!(content_result.succeeded.is_empty(), "{content_result:?}");
    assert_eq!(std::fs::read(&source).unwrap(), original);
    assert_eq!(std::fs::read(&backup_path).unwrap(), original);
    assert!(!output_directory.join("测试音乐.mp3").exists());
    std::fs::remove_file(&backup_path).unwrap();

    let mut both_request = binary_file_request(&source, FileMode::Both);
    both_request.output_directory = Some(output_directory.to_string_lossy().into_owned());
    both_request.backup = Some(true);
    let both_plan = service
        .plan(shared_conversion(), both_request, noop())
        .await
        .unwrap();
    let renamed = output_directory.join("測試音樂.mp3");
    assert_eq!(both_plan.items[0].output_path, renamed.to_string_lossy());
    let both_result = service
        .apply(
            shared_conversion(),
            &both_plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert!(both_result.failed.is_empty(), "{both_result:?}");
    assert_eq!(std::fs::read(&renamed).unwrap(), original);
    assert!(!source.exists());
    assert_eq!(std::fs::read(&backup_path).unwrap(), original);
    assert!(both_result
        .warnings
        .iter()
        .any(|warning| warning.contains("已略過內容轉換")));
    let _ = std::fs::remove_dir_all(&directory);
}

#[derive(Deserialize)]
struct TypeFilterMigrationFixture {
    cases: Vec<TypeFilterMigrationCase>,
}

#[derive(Deserialize)]
struct TypeFilterMigrationCase {
    name: String,
    input: String,
}

#[test]
fn migrated_bracketless_type_filter_does_not_visit_audio_files() {
    // 舊格式字串來源：e7c3aeb ConvertZZ/Settings.cs:112、03204e6 ConvertZZ/Settings.cs:109。
    let fixture: TypeFilterMigrationFixture = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/type-filter-migration.json"
    ))
    .expect("type-filter-migration.json");
    let directory = temp_dir();
    std::fs::write(directory.join("note.txt"), "甲").unwrap();
    std::fs::write(directory.join("page.html"), "<p>乙</p>").unwrap();
    std::fs::write(directory.join("song.mp3"), b"mp3").unwrap();
    std::fs::write(directory.join("clip.wav"), b"wav").unwrap();
    let nested = directory.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("inner.txt"), "丙").unwrap();
    std::fs::write(nested.join("inner.mp3"), b"mp3").unwrap();

    for name in ["e7c3aeb 舊格式", "03204e6 舊格式"] {
        let case = fixture
            .cases
            .iter()
            .find(|case| case.name == name)
            .unwrap_or_else(|| panic!("缺少向量 {name}"));
        assert!(
            matches!(
                extensions_from_type_filter(&case.input),
                ParsedTypeExtensions::List(ref items) if items.is_empty()
            ),
            "{name} 遷移前不該解析出副檔名"
        );
        let migrated = migrate(json!({ "FileConvert": { "TypeFilter": case.input } }));
        let filter = migrated["files"]["typeFilter"].as_str().unwrap();
        let ParsedTypeExtensions::List(extensions) = extensions_from_type_filter(filter) else {
            panic!("{name} 遷移後不應變成所有檔案");
        };
        assert!(!extensions.is_empty(), "{name} 遷移後仍解析不到副檔名");
        assert!(extensions.iter().any(|extension| extension == ".txt"));
        assert!(extensions.iter().any(|extension| extension == ".html"));
        assert!(!extensions
            .iter()
            .any(|extension| extension == ".mp3" || extension == ".wav"));
        let allowed = extensions.into_iter().collect::<HashSet<_>>();
        let mut collected = HashSet::new();
        visit_files(
            &directory.to_string_lossy(),
            true,
            false,
            &ExtensionGate::Only(allowed),
            &mut collected,
            &mut Vec::new(),
        )
        .unwrap();
        let names = collected
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(names.iter().any(|item| item == "note.txt"), "{name}");
        assert!(names.iter().any(|item| item == "page.html"), "{name}");
        assert!(names.iter().any(|item| item == "inner.txt"), "{name}");
        assert!(
            !names
                .iter()
                .any(|item| item.ends_with(".mp3") || item.ends_with(".wav")),
            "{name} 收進了非預設副檔名：{names:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&directory);
}

fn directory_scan_request(
    path: &Path,
    mode: FileMode,
    allowed_extensions: Option<Vec<String>>,
) -> FilePlanRequest {
    FilePlanRequest {
        paths: vec![path.to_string_lossy().into_owned()],
        output_path: None,
        output_directory: None,
        mode,
        recursive: true,
        input_encoding: TextEncoding::Utf8,
        output_encoding: TextEncoding::Utf8,
        add_bom: false,
        fix_charset_declaration: false,
        fix_charset_extensions: None,
        allowed_extensions,
        preview_max_bytes: Some(4096),
        conflict_policy: ConflictPolicy::Skip,
        backup: Some(false),
        include_ignored: false,
        conversion: conversion_s2t(),
    }
}

fn sorted_file_names(plan: &super::super::types::FileConversionPlan) -> Vec<String> {
    let mut names = plan
        .items
        .iter()
        .map(|item| {
            Path::new(&item.source_path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}

#[tokio::test]
async fn all_files_directory_scan_includes_every_file() {
    let directory = temp_dir();
    std::fs::write(directory.join("note.txt"), "甲").unwrap();
    std::fs::write(directory.join("custom.mytxt"), "软件").unwrap();
    std::fs::write(directory.join("song.mp3"), b"mp3").unwrap();
    std::fs::write(directory.join("noext"), "乙").unwrap();
    let nested = directory.join("nested");
    std::fs::create_dir(&nested).unwrap();
    std::fs::write(nested.join("inner.log"), "丙").unwrap();
    let service = FileService::new();
    let plan = service
        .plan(
            shared_conversion(),
            directory_scan_request(&directory, FileMode::Content, None),
            noop(),
        )
        .await
        .unwrap();
    assert_eq!(
        sorted_file_names(&plan),
        ["custom.mytxt", "inner.log", "noext", "note.txt", "song.mp3"]
    );
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn all_files_directory_scan_converts_custom_text_extension() {
    let directory = temp_dir();
    let source = directory.join("note.mytxt");
    std::fs::write(&source, "软件").unwrap();
    let service = FileService::new();
    let plan = service
        .plan(
            shared_conversion(),
            directory_scan_request(&directory, FileMode::Content, None),
            noop(),
        )
        .await
        .unwrap();
    assert_eq!(sorted_file_names(&plan), ["note.mytxt"]);
    let result = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    assert_eq!(std::fs::read_to_string(&source).unwrap(), "軟件");
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn all_files_directory_scan_keeps_mp3_bytes_for_content_and_both() {
    let original = std::fs::read(audio_fixture("测试音乐.mp3")).unwrap();
    let directory = temp_dir();
    let source = directory.join("测试音乐.mp3");
    std::fs::write(&source, &original).unwrap();
    let service = FileService::new();
    let content_plan = service
        .plan(
            shared_conversion(),
            directory_scan_request(&directory, FileMode::Content, None),
            noop(),
        )
        .await
        .unwrap();
    assert_eq!(sorted_file_names(&content_plan), ["测试音乐.mp3"]);
    let content_result = service
        .apply(
            shared_conversion(),
            &content_plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert!(content_result.failed.is_empty(), "{content_result:?}");
    assert!(content_result.succeeded.is_empty(), "{content_result:?}");
    assert!(content_result
        .warnings
        .iter()
        .any(|warning| warning.contains("已略過內容轉換")));
    assert_eq!(std::fs::read(&source).unwrap(), original);

    let both_plan = service
        .plan(
            shared_conversion(),
            directory_scan_request(&directory, FileMode::Both, None),
            noop(),
        )
        .await
        .unwrap();
    let renamed = directory.join("測試音樂.mp3");
    assert_eq!(
        both_plan.items[0].output_path,
        renamed.to_string_lossy().into_owned()
    );
    let both_result = service
        .apply(
            shared_conversion(),
            &both_plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert!(both_result.failed.is_empty(), "{both_result:?}");
    assert!(both_result
        .warnings
        .iter()
        .any(|warning| warning.contains("已略過內容轉換")));
    assert_eq!(std::fs::read(&renamed).unwrap(), original);
    assert!(!source.exists());
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn empty_extension_list_collects_no_files() {
    let directory = temp_dir();
    std::fs::write(directory.join("note.txt"), "甲").unwrap();
    std::fs::write(directory.join("custom.mytxt"), "软件").unwrap();
    std::fs::write(directory.join("song.mp3"), b"mp3").unwrap();
    let service = FileService::new();
    let scanned = service
        .plan(
            shared_conversion(),
            directory_scan_request(&directory, FileMode::Content, Some(vec![])),
            noop(),
        )
        .await
        .unwrap();
    assert!(scanned.items.is_empty(), "{:?}", scanned.items);
    let starred = service
        .plan(
            shared_conversion(),
            directory_scan_request(
                &directory,
                FileMode::Content,
                Some(vec!["*".into(), "*.*".into()]),
            ),
            noop(),
        )
        .await
        .unwrap();
    assert!(
        starred.items.is_empty(),
        "清單裡的 * 不是所有檔案：{:?}",
        starred.items
    );
    let explicit = service
        .plan(
            shared_conversion(),
            directory_scan_request(&directory.join("note.txt"), FileMode::Content, Some(vec![])),
            noop(),
        )
        .await
        .unwrap();
    assert_eq!(
        explicit.items.len(),
        1,
        "直接指定的檔案即使清單為空也收：{:?}",
        explicit.items
    );
    let picked = service
        .plan(
            shared_conversion(),
            directory_scan_request(
                &directory.join("custom.mytxt"),
                FileMode::Content,
                Some(vec![".txt".into()]),
            ),
            noop(),
        )
        .await
        .unwrap();
    assert_eq!(sorted_file_names(&picked), ["custom.mytxt"]);
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn empty_extension_list_collects_nothing_and_does_not_rename_subfolders() {
    let directory = temp_dir();
    let nested = directory.join("简体资料夹");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("软件.txt"), "软件").unwrap();
    for mode in [FileMode::Filename, FileMode::Both] {
        let service = FileService::new();
        let plan = service
            .plan(
                shared_conversion(),
                directory_scan_request(&directory, mode, Some(Vec::new())),
                noop(),
            )
            .await
            .unwrap();
        assert!(plan.items.is_empty(), "{mode:?}：{:?}", plan.items);
        assert!(
            plan.warnings
                .iter()
                .any(|warning| warning.contains("沒有符合副檔名篩選器的檔案")),
            "{:?}",
            plan.warnings
        );
        let result = service
            .apply(
                shared_conversion(),
                &plan.plan_id,
                None,
                noop(),
                never_cancel(),
            )
            .await
            .unwrap();
        assert!(result.succeeded.is_empty());
        assert!(nested.exists(), "子資料夾不應被改名");
    }
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn folder_scan_with_matches_has_no_empty_folder_warning() {
    let directory = temp_dir();
    std::fs::write(directory.join("软件.txt"), "软件").unwrap();
    let plan = FileService::new()
        .plan(
            shared_conversion(),
            directory_scan_request(&directory, FileMode::Content, Some(vec![".txt".into()])),
            noop(),
        )
        .await
        .unwrap();
    assert_eq!(plan.items.len(), 1);
    assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn recursive_scan_skips_version_control_and_hidden_directories() {
    let directory = temp_dir();
    for hidden in [".git", ".svn", ".hg", ".bzr", ".cache"] {
        let path = directory.join(hidden).join("简体");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("软件.txt"), "软件").unwrap();
        std::fs::write(directory.join(hidden).join("COMMIT_EDITMSG"), "修正软件").unwrap();
    }
    std::fs::write(directory.join("软件.txt"), "软件").unwrap();
    let plan = FileService::new()
        .plan(
            shared_conversion(),
            directory_scan_request(&directory, FileMode::Both, None),
            noop(),
        )
        .await
        .unwrap();
    let sources = plan
        .items
        .iter()
        .map(|item| item.source_path.clone())
        .collect::<Vec<_>>();
    assert_eq!(sources.len(), 1, "{sources:?}");
    assert!(sources[0].ends_with("软件.txt"));
    assert!(plan.ignored_inputs.is_empty());
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn directly_selected_git_folder_needs_include_ignored() {
    let directory = temp_dir();
    let git = directory.join(".git");
    std::fs::create_dir_all(&git).unwrap();
    std::fs::write(git.join("说明.txt"), "软件").unwrap();
    let other = directory.join("软件.txt");
    std::fs::write(&other, "软件").unwrap();

    let mut request = directory_scan_request(&git, FileMode::Both, None);
    request.paths.push(other.to_string_lossy().into_owned());
    let skipped = FileService::new()
        .plan(shared_conversion(), request.clone(), noop())
        .await
        .unwrap();
    assert_eq!(
        skipped.ignored_inputs,
        vec![git.to_string_lossy().into_owned()]
    );
    assert_eq!(skipped.items.len(), 1);
    assert_eq!(skipped.items[0].source_path, other.to_string_lossy());
    assert!(skipped
        .warnings
        .iter()
        .any(|warning| warning.contains("版本控制資料夾")));

    // 直接指定 .git 內的檔案也算。
    let inner = git.join("说明.txt");
    let inner_plan = FileService::new()
        .plan(
            shared_conversion(),
            binary_file_request(&inner, FileMode::Both),
            noop(),
        )
        .await
        .unwrap();
    assert!(inner_plan.items.is_empty());
    assert_eq!(inner_plan.ignored_inputs.len(), 1);

    request.include_ignored = true;
    let allowed = FileService::new()
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    assert!(allowed.ignored_inputs.is_empty());
    assert_eq!(allowed.items.len(), 2, "{:?}", allowed.items);
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn version_control_detection_matches_components_only() {
    assert!(is_inside_version_control("/repo/.git"));
    assert!(is_inside_version_control("/repo/.GIT/config"));
    assert!(is_inside_version_control("/repo/.svn/entries"));
    assert!(is_inside_version_control("/repo/.hg"));
    assert!(is_inside_version_control("/repo/.bzr/x"));
    assert!(!is_inside_version_control("/repo/.gitignore"));
    assert!(!is_inside_version_control("/repo/my.git.txt"));
    assert!(!is_inside_version_control("/home/u/.config/a.txt"));
}

#[test]
fn streaming_compare_detects_equal_and_different_files() {
    let directory = temp_dir();
    let left = directory.join("left.bin");
    let right = directory.join("right.bin");
    let data = (0..(STREAM_CHUNK_BYTES * 3 + 17))
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    std::fs::write(&left, &data).unwrap();
    std::fs::write(&right, &data).unwrap();
    assert!(files_equal(&left, &right).unwrap());
    assert!(file_equals_bytes(&left, &data).unwrap());
    let mut changed = data.clone();
    let last = changed.len() - 1;
    changed[last] ^= 1;
    std::fs::write(&right, &changed).unwrap();
    assert!(!files_equal(&left, &right).unwrap());
    assert!(!file_equals_bytes(&left, &changed).unwrap());
    assert!(!file_equals_bytes(&left, &data[..data.len() - 1]).unwrap());
    assert!(verify_stage(&right, Some(&data), &left).is_err());
    assert!(!right.exists(), "驗證失敗應刪除暫存檔");
    let _ = std::fs::remove_dir_all(&directory);
}

/// 檔頭是 ID3 的大型稀疏檔：只讀檔頭就略過，不會整檔讀入。
fn sparse_binary(path: &Path, size: u64) {
    let file = std::fs::File::create(path).unwrap();
    use std::io::Write;
    (&file)
        .write_all(b"ID3\x03\x00\x00\x00\x00\x00\x0a")
        .unwrap();
    file.set_len(size).unwrap();
}

#[tokio::test]
async fn large_binary_is_skipped_from_header_and_renamed_without_copy() {
    let directory = temp_dir();
    let source = directory.join("简体影片.bin");
    // 遠大於文字上限；若整檔讀入會非常慢且吃大量記憶體。
    sparse_binary(&source, 4 * 1024 * 1024 * 1024);
    #[cfg(unix)]
    let inode = {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(&source).unwrap().ino()
    };

    let (content_result, content_item) = preview_and_apply(&source, FileMode::Content).await;
    assert!(content_result.succeeded.is_empty(), "{content_result:?}");
    assert!(content_item
        .warning
        .as_deref()
        .unwrap_or_default()
        .contains("二進位"));
    assert_eq!(
        std::fs::metadata(&source).unwrap().len(),
        4 * 1024 * 1024 * 1024
    );

    let (both_result, _) = preview_and_apply(&source, FileMode::Both).await;
    let renamed = directory.join("簡體影片.bin");
    assert_eq!(both_result.succeeded.len(), 1, "{both_result:?}");
    assert!(!source.exists());
    assert_eq!(
        std::fs::metadata(&renamed).unwrap().len(),
        4 * 1024 * 1024 * 1024
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(
            std::fs::metadata(&renamed).unwrap().ino(),
            inode,
            "同一磁碟區應直接 rename，不複製"
        );
    }
    assert!(names(&directory)
        .into_iter()
        .all(|name| !name.starts_with(".convertzz-")));
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn text_over_size_cap_is_skipped_with_warning() {
    let directory = temp_dir();
    let source = directory.join("大檔.txt");
    {
        use std::io::Write;
        let mut file = std::io::BufWriter::new(std::fs::File::create(&source).unwrap());
        let line = "软件开发测试\n".repeat(1024);
        while file.get_ref().metadata().unwrap().len() <= MAX_TEXT_FILE_BYTES {
            file.write_all(line.as_bytes()).unwrap();
            file.flush().unwrap();
        }
    }
    let size = std::fs::metadata(&source).unwrap().len();
    let (result, item) = preview_and_apply(&source, FileMode::Content).await;
    assert!(result.succeeded.is_empty(), "{result:?}");
    assert!(item
        .warning
        .as_deref()
        .unwrap_or_default()
        .contains("64 MiB"));
    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.contains("64 MiB")));
    assert_eq!(std::fs::metadata(&source).unwrap().len(), size);
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn rename_only_commit_moves_into_output_directory_and_handles_conflict() {
    let directory = temp_dir();
    let source = directory.join("简体.bin");
    let tagged = std::fs::read(audio_fixture("测试音乐.mp3")).unwrap();
    std::fs::write(&source, &tagged).unwrap();
    let existing = directory.join("簡體.bin");
    std::fs::write(&existing, b"old").unwrap();

    // Skip：既有檔保留，來源不動。
    let service = FileService::new();
    let plan = service
        .plan(
            shared_conversion(),
            binary_file_request(&source, FileMode::Both),
            noop(),
        )
        .await
        .unwrap();
    assert_eq!(plan.items[0].status, PlanStatus::Conflict);

    // Overwrite：既有檔被取代，內容與來源相同，沒有殘留暫存檔。
    let mut request = binary_file_request(&source, FileMode::Both);
    request.conflict_policy = ConflictPolicy::Overwrite;
    let service = FileService::new();
    let plan = service
        .plan(shared_conversion(), request, noop())
        .await
        .unwrap();
    let result = service
        .apply(
            shared_conversion(),
            &plan.plan_id,
            None,
            noop(),
            never_cancel(),
        )
        .await
        .unwrap();
    assert_eq!(result.succeeded.len(), 1, "{result:?}");
    assert!(!source.exists());
    assert_eq!(std::fs::read(&existing).unwrap(), tagged);
    assert!(names(&directory)
        .into_iter()
        .all(|name| !name.starts_with(".convertzz-")));
    let _ = std::fs::remove_dir_all(&directory);
}

fn prepared_unchanged_file(source: &Path, output: &Path, policy: ConflictPolicy) -> PreparedFile {
    PreparedFile {
        item: FilePlanItem {
            source_path: source.to_string_lossy().into_owned(),
            output_path: output.to_string_lossy().into_owned(),
            kind: FileItemKind::File,
            selected: true,
            detected_encoding: None,
            source_preview: String::new(),
            output_preview: String::new(),
            preview_loaded: true,
            status: PlanStatus::Ready,
            warning: None,
        },
        content: None,
        conflict_policy: policy,
    }
}

fn convertzz_temps(directory: &Path) -> Vec<String> {
    names(directory)
        .into_iter()
        .filter(|name| name.starts_with(".convertzz-"))
        .collect()
}

fn crosses_devices_for<'a>(
    source: &'a Path,
    output: &'a Path,
) -> impl Fn(&Path, &Path) -> std::io::Result<()> + 'a {
    move |from, to| {
        if from == source && to == output {
            Err(std::io::Error::new(
                std::io::ErrorKind::CrossesDevices,
                "cross-device",
            ))
        } else {
            std::fs::rename(from, to)
        }
    }
}

fn exdev_os_error_for<'a>(
    source: &'a Path,
    output: &'a Path,
) -> impl Fn(&Path, &Path) -> std::io::Result<()> + 'a {
    move |from, to| {
        if from == source && to == output {
            #[cfg(unix)]
            {
                Err(std::io::Error::from_raw_os_error(18))
            }
            #[cfg(windows)]
            {
                Err(std::io::Error::from_raw_os_error(17))
            }
            #[cfg(not(any(unix, windows)))]
            {
                Err(std::io::Error::new(
                    std::io::ErrorKind::CrossesDevices,
                    "cross-device",
                ))
            }
        } else {
            std::fs::rename(from, to)
        }
    }
}

fn copy_fail_after_bytes(
    limit: u64,
) -> impl Fn(&Path, &Path, Option<&CancelCheck>) -> Result<(), CoreError> {
    move |from, to, is_cancelled| {
        let mut reader = std::fs::File::open(from).map_err(CoreError::from)?;
        let mut writer = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(to)
            .map_err(CoreError::from)?;
        let mut buffer = vec![0u8; COPY_CHUNK_BYTES];
        let mut written = 0u64;
        loop {
            if is_cancelled.is_some_and(|check| check()) {
                let _ = std::fs::remove_file(to);
                return Err(CoreError::new("PLAN_CANCELLED", "檔案作業已由使用者取消。"));
            }
            let read = super::read_chunk(&mut reader, &mut buffer).map_err(CoreError::from)?;
            if read == 0 {
                break;
            }
            let remaining = limit.saturating_sub(written);
            if remaining == 0 {
                return Err(CoreError::new("IO_ERROR", "受控複製失敗"));
            }
            let take = read.min(remaining as usize);
            writer.write_all(&buffer[..take]).map_err(CoreError::from)?;
            written += take as u64;
            if take < read {
                return Err(CoreError::new("IO_ERROR", "受控複製失敗"));
            }
        }
        Ok(())
    }
}

fn assert_no_transaction_temps(directories: &[&Path]) {
    for directory in directories {
        assert!(
            convertzz_temps(directory).is_empty(),
            "{} 殘留 {:?}",
            directory.display(),
            convertzz_temps(directory)
        );
    }
}

#[test]
fn cross_device_rename_copy_failure_keeps_source_and_clears_temps() {
    let directory = temp_dir();
    let source_dir = directory.join("src");
    let output_dir = directory.join("out");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::create_dir_all(&output_dir).unwrap();
    let source = source_dir.join("payload.bin");
    let output = output_dir.join("payload.bin");
    let original = vec![7u8; COPY_CHUNK_BYTES + 32];
    std::fs::write(&source, &original).unwrap();
    let error = commit_unchanged_content_with(
        prepared_unchanged_file(&source, &output, ConflictPolicy::Overwrite),
        None,
        None,
        crosses_devices_for(&source, &output),
        copy_fail_after_bytes(64),
    )
    .unwrap_err();
    assert_eq!(error.message, "受控複製失敗");
    assert_eq!(std::fs::read(&source).unwrap(), original);
    assert!(!output.exists());
    assert_no_transaction_temps(&[&source_dir, &output_dir]);
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn cross_device_rename_cancel_keeps_source_and_clears_temps() {
    let directory = temp_dir();
    let source_dir = directory.join("src");
    let output_dir = directory.join("out");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::create_dir_all(&output_dir).unwrap();
    let source = source_dir.join("payload.bin");
    let output = output_dir.join("payload.bin");
    let original = vec![9u8; COPY_CHUNK_BYTES + 8];
    std::fs::write(&source, &original).unwrap();
    let checks = std::sync::atomic::AtomicU64::new(0);
    let cancelled: CancelCheck = Arc::new(move || checks.fetch_add(1, Ordering::Relaxed) >= 2);
    let error = commit_unchanged_content_with(
        prepared_unchanged_file(&source, &output, ConflictPolicy::Overwrite),
        None,
        Some(&cancelled),
        crosses_devices_for(&source, &output),
        copy_file_chunked,
    )
    .unwrap_err();
    assert_eq!(error.code, "PLAN_CANCELLED");
    assert_eq!(std::fs::read(&source).unwrap(), original);
    assert!(!output.exists());
    assert_no_transaction_temps(&[&source_dir, &output_dir]);
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn cross_device_rename_success_matches_bytes_and_clears_source() {
    let directory = temp_dir();
    let source_dir = directory.join("src");
    let output_dir = directory.join("out");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::create_dir_all(&output_dir).unwrap();
    let source = source_dir.join("payload.bin");
    let output = output_dir.join("payload.bin");
    let original = (0..(COPY_CHUNK_BYTES + 51))
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    std::fs::write(&source, &original).unwrap();
    let result = commit_unchanged_content_with(
        prepared_unchanged_file(&source, &output, ConflictPolicy::Overwrite),
        None,
        None,
        crosses_devices_for(&source, &output),
        copy_file_chunked,
    )
    .unwrap();
    let FileCommitOutcome::Succeeded { output_path, .. } = result else {
        panic!("跨磁碟區複製後應寫入輸出檔");
    };
    assert_eq!(output_path, output);
    assert!(!source.exists());
    assert_eq!(std::fs::read(&output).unwrap(), original);
    assert_no_transaction_temps(&[&source_dir, &output_dir]);
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn cross_device_os_error_and_verify_failure_keep_source() {
    let directory = temp_dir();
    let source_dir = directory.join("src");
    let output_dir = directory.join("out");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::create_dir_all(&output_dir).unwrap();
    let source = source_dir.join("payload.bin");
    let output = output_dir.join("payload.bin");
    let original = b"verify-me".to_vec();
    std::fs::write(&source, &original).unwrap();
    let error = commit_unchanged_content_with(
        prepared_unchanged_file(&source, &output, ConflictPolicy::Overwrite),
        None,
        None,
        exdev_os_error_for(&source, &output),
        |from, to, _| {
            std::fs::write(to, b"tampered").map_err(CoreError::from)?;
            let _ = from;
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(error.code, "FILE_VERIFY");
    assert_eq!(std::fs::read(&source).unwrap(), original);
    assert!(!output.exists());
    assert_no_transaction_temps(&[&source_dir, &output_dir]);
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn cross_device_conflict_copy_failure_restores_existing_output() {
    let directory = temp_dir();
    let source_dir = directory.join("src");
    let output_dir = directory.join("out");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::create_dir_all(&output_dir).unwrap();
    let source = source_dir.join("payload.bin");
    let output = output_dir.join("payload.bin");
    let original = b"source-bytes".to_vec();
    std::fs::write(&source, &original).unwrap();
    std::fs::write(&output, b"existing").unwrap();
    let error = commit_unchanged_content_with(
        prepared_unchanged_file(&source, &output, ConflictPolicy::Overwrite),
        None,
        None,
        crosses_devices_for(&source, &output),
        copy_fail_after_bytes(1),
    )
    .unwrap_err();
    assert_eq!(error.message, "受控複製失敗");
    assert_eq!(std::fs::read(&source).unwrap(), original);
    assert_eq!(std::fs::read(&output).unwrap(), b"existing");
    assert_no_transaction_temps(&[&source_dir, &output_dir]);
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn folder_scan_skips_convertzz_transaction_residue() {
    let directory = temp_dir();
    std::fs::write(directory.join("软件.txt"), "软件").unwrap();
    std::fs::write(directory.join(".convertzz-original-deadbeef.txt"), "殘留").unwrap();
    std::fs::write(directory.join(".convertzz-stage-abcd.bin"), b"stage").unwrap();
    let plan = FileService::new()
        .plan(
            shared_conversion(),
            directory_scan_request(&directory, FileMode::Content, None),
            noop(),
        )
        .await
        .unwrap();
    assert_eq!(sorted_file_names(&plan), ["软件.txt"]);
    let _ = std::fs::remove_dir_all(&directory);
}

#[cfg(unix)]
struct RestoreMode<'a> {
    path: &'a Path,
    mode: u32,
}

#[cfg(unix)]
impl Drop for RestoreMode<'_> {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(self.path, std::fs::Permissions::from_mode(self.mode));
    }
}

#[cfg(unix)]
#[tokio::test]
async fn unreadable_subdirectory_is_skipped_with_plan_warning() {
    let directory = temp_dir();
    std::fs::write(directory.join("ok.txt"), "软件").unwrap();
    let blocked = directory.join("blocked");
    std::fs::create_dir(&blocked).unwrap();
    std::fs::write(blocked.join("secret.txt"), "里面").unwrap();
    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o000)).unwrap();
    let _restore = RestoreMode {
        path: &blocked,
        mode: 0o755,
    };
    if std::fs::read_dir(&blocked).is_ok() {
        drop(_restore);
        let _ = std::fs::remove_dir_all(&directory);
        return;
    }
    let plan = FileService::new()
        .plan(
            shared_conversion(),
            directory_scan_request(&directory, FileMode::Both, None),
            noop(),
        )
        .await
        .unwrap();
    let sources = plan
        .items
        .iter()
        .map(|item| {
            Path::new(&item.source_path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    assert!(sources.iter().any(|name| name == "ok.txt"), "{sources:?}");
    assert!(
        !sources.iter().any(|name| name == "secret.txt"),
        "{sources:?}"
    );
    assert!(
        plan.warnings
            .iter()
            .any(|warning| warning.contains(&blocked.to_string_lossy().into_owned())),
        "{:?}",
        plan.warnings
    );
    drop(_restore);
    let _ = std::fs::remove_dir_all(&directory);
}

#[cfg(unix)]
#[tokio::test]
async fn unreadable_top_level_directory_still_fails_plan() {
    let directory = temp_dir();
    std::fs::write(directory.join("ok.txt"), "软件").unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o000)).unwrap();
    let _restore = RestoreMode {
        path: &directory,
        mode: 0o755,
    };
    if std::fs::read_dir(&directory).is_ok() {
        drop(_restore);
        let _ = std::fs::remove_dir_all(&directory);
        return;
    }
    let error = FileService::new()
        .plan(
            shared_conversion(),
            directory_scan_request(&directory, FileMode::Content, None),
            noop(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "IO_ERROR");
    drop(_restore);
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn merge_warning_does_not_duplicate_existing_text() {
    assert_eq!(
        merge_warning(
            Some("輸出路徑已存在。".into()),
            Some("輸出路徑已存在。".into())
        ),
        Some("輸出路徑已存在。".into())
    );
    assert_eq!(
        merge_warning(
            Some("二進位檔案，已略過內容轉換。".into()),
            Some("二進位檔案，已略過內容轉換。 輸出路徑已存在。".into())
        ),
        Some("二進位檔案，已略過內容轉換。 輸出路徑已存在。".into())
    );
    assert_eq!(
        merge_warning(
            Some("二進位檔案，已略過內容轉換。 輸出路徑已存在。".into()),
            Some("輸出路徑已存在。".into())
        ),
        Some("二進位檔案，已略過內容轉換。 輸出路徑已存在。".into())
    );
    assert_eq!(
        merge_warning(
            Some("二進位檔案，已略過內容轉換。".into()),
            Some("輸出路徑已存在。".into())
        ),
        Some("二進位檔案，已略過內容轉換。 輸出路徑已存在。".into())
    );
}

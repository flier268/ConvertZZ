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
    assert!(!plan.items[0].preview_loaded);
    assert!(plan.items[0].source_preview.is_empty());
    let previewed = service
        .preview(
            shared_conversion(),
            FilePreviewRequest {
                plan_id: plan.plan_id.clone(),
                source_path: path.to_string_lossy().into_owned(),
            },
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
        .apply(shared_conversion(), &plan.plan_id, None, noop())
        .await
        .unwrap();
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
    assert!(!plan.items[0].preview_loaded);
    let previewed = service
        .preview(
            shared_conversion(),
            FilePreviewRequest {
                plan_id: plan.plan_id.clone(),
                source_path: path.to_string_lossy().into_owned(),
            },
        )
        .await
        .unwrap();
    assert!(previewed.source_preview.contains("里面开发"));
    assert!(previewed.output_preview.contains("裡面開發"));
    let result = service
        .apply(shared_conversion(), &plan.plan_id, None, noop())
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
        .apply(shared_conversion(), &plan.plan_id, None, noop())
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
        .apply(shared_conversion(), &plan.plan_id, None, noop())
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
    let result = service
        .apply(shared_conversion(), &plan.plan_id, None, noop())
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
        .apply(shared_conversion(), &plan.plan_id, None, noop())
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
        .apply(shared_conversion(), &plan.plan_id, None, noop())
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
    let result = service
        .apply(shared_conversion(), &plan.plan_id, None, progress)
        .await
        .unwrap();
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
    let applied = service
        .apply(shared_conversion(), &result.plan_id, None, noop())
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
    let plan = service
        .plan(
            shared_conversion(),
            filename_request(&source, ConflictPolicy::Overwrite),
            noop(),
        )
        .await
        .unwrap();
    let result = service
        .apply(shared_conversion(), &plan.plan_id, None, noop())
        .await
        .unwrap();
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
    let result = service
        .apply(shared_conversion(), &plan.plan_id, None, noop())
        .await
        .unwrap();
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
    let result = service
        .apply(shared_conversion(), &plan.plan_id, None, noop())
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
                conversion: conversion_s2t(),
            },
            noop(),
        )
        .await
        .unwrap();
    let result = service
        .apply(shared_conversion(), &plan.plan_id, None, noop())
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
                conversion: conversion_s2t(),
            },
            noop(),
        )
        .await
        .unwrap();
    service
        .apply(shared_conversion(), &plan.plan_id, None, noop())
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
        )
        .await
        .unwrap();
    assert!(previewed.source_preview.contains("里面开发"));
    assert!(previewed.output_preview.contains("裡面開發"));
    let result = service
        .apply(shared_conversion(), &plan.plan_id, None, noop())
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
        )
        .await
        .unwrap();
    assert!(previewed.output_preview.contains("裡面開發"));
    assert_eq!(service.cancel(&plan.plan_id)["cancelled"], true);
    let error = service
        .apply(shared_conversion(), &plan.plan_id, None, noop())
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
        .apply(shared_conversion(), &plan.plan_id, None, noop())
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
        )
        .await
        .unwrap();
    assert!(result.failed.is_empty(), "{result:?}");
    assert_eq!(std::fs::read_to_string(&first).unwrap(), "裡面");
    assert_eq!(std::fs::read_to_string(&second).unwrap(), "头发");
    let _ = std::fs::remove_dir_all(&directory);
}

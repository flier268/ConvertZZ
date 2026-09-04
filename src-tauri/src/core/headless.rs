use super::cli::parse_cli;
use super::settings::{migrate, migrate_from_path};
use super::types::{
    ApplyResult, AudioTagField, AudioTagFile, AudioTagPlan, AudioTagPlanRequest, CliMode,
    ConflictPolicy, ConversionOptions, Direction, EngineKind, FileConversionPlan, FileMode,
    FilePlanRequest, ParsedCli, PlanStatus, ProgressEvent, TextEncoding, VocabularyCorrection,
    ZhConvertModules, ZhConvertOptions,
};
use super::{dispatch, zhconvert_client, CoreError, CoreState};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

const PORTABLE_MARKER: &str = "portable";
const SETTINGS_STORE_FILE: &str = "settings-v2.json";
const APP_IDENTIFIER: &str = "dev.flier268.convertzz";

/// 無頭且未載入設定時，與 GUI 預設一致的副檔名過濾。
const DEFAULT_ALLOWED_EXTENSIONS: &[&str] = &[
    ".txt", ".log", ".ini", ".inf", ".bat", ".cmd", ".srt", ".ass", ".lang", ".htm", ".html",
    ".php", ".asp", ".css", ".js", ".mp3", ".ape", ".ogg", ".oga", ".opus",
];

const DEFAULT_FIX_CHARSET_EXTENSIONS: &[&str] = &[
    ".htm", ".html", ".shtm", ".shtml", ".asp", ".aspx", ".php", ".css",
];

/// 無頭 CLI 進入點。回傳行程結束碼（0＝成功）。
pub fn run(args: &[String], dictionary_path: Option<PathBuf>) -> i32 {
    match run_inner(args, dictionary_path) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("錯誤：{error}");
            1
        }
    }
}

fn run_inner(args: &[String], dictionary_path: Option<PathBuf>) -> Result<i32, String> {
    let mut parsed = parse_cli(args, None);
    if !parsed.headless {
        return Err("內部錯誤：非無頭參數不應進入無頭路徑。".into());
    }
    if !parsed.parse_errors.is_empty() {
        return Err(parsed.parse_errors.join("；"));
    }
    if parsed.paths.is_empty() {
        return Err("無頭模式需要至少一個來源路徑。".into());
    }
    if parsed.mode == CliMode::Interactive {
        return Err("無頭模式無法判斷作業類型；請加上 --file、--audio，或提供路徑。".into());
    }
    if parsed.use_global_config && parsed.config_path.is_some() {
        return Err("--globalconfig 與 --config 不可同時使用。".into());
    }

    let settings = load_cli_settings(&parsed)?;
    apply_settings_defaults(&mut parsed, settings.as_ref())?;

    let state = Arc::new(CoreState::new(dictionary_path).map_err(|error| error.message.clone())?);
    if let Some(api_key) = load_zhconvert_api_key_quiet() {
        zhconvert_client(&state).configure(&api_key);
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("無法建立非同步執行環境：{error}"))?;

    runtime.block_on(async {
        match parsed.mode {
            CliMode::Audio => {
                let do_tags = parsed.operation != FileMode::Filename;
                let do_filename = matches!(parsed.operation, FileMode::Both | FileMode::Filename);
                if do_tags && do_filename {
                    run_audio_then_filename(state, &mut parsed, settings.as_ref()).await
                } else if do_tags {
                    run_audio(state, &mut parsed, settings.as_ref()).await
                } else {
                    let mut filename_parsed = parsed.clone();
                    filename_parsed.mode = CliMode::File;
                    filename_parsed.operation = FileMode::Filename;
                    run_files(state, &mut filename_parsed, settings.as_ref()).await
                }
            }
            CliMode::File => run_files(state, &mut parsed, settings.as_ref()).await,
            CliMode::Interactive => Err("無頭模式無法判斷作業類型。".into()),
        }
    })
}

/// 合併兩段作業結束碼：取消(2) > 部分失敗(3) > 失敗(1) > 成功(0)。
fn merge_exit_codes(first: i32, second: i32) -> i32 {
    for code in [2, 3, 1] {
        if first == code || second == code {
            return code;
        }
    }
    0
}

struct PreparedFileWrite {
    plan: FileConversionPlan,
    ready_paths: Vec<String>,
}

struct PreparedAudioWrite {
    plan: AudioTagPlan,
    writable: usize,
}

async fn run_audio_then_filename(
    state: Arc<CoreState>,
    parsed: &mut ParsedCli,
    settings: Option<&Value>,
) -> Result<i32, String> {
    let audio = prepare_audio(Arc::clone(&state), parsed, settings).await?;
    let mut filename_parsed = parsed.clone();
    filename_parsed.mode = CliMode::File;
    filename_parsed.operation = FileMode::Filename;
    let files = prepare_files(Arc::clone(&state), &filename_parsed, settings).await?;

    let tag_count = audio.as_ref().map(|item| item.writable).unwrap_or(0);
    let name_count = files.ready_paths.len();
    if tag_count == 0 && name_count == 0 {
        eprintln!("沒有可寫入的音訊標籤或檔名。");
        return Ok(1);
    }

    if !confirm_tags_then_rename(parsed, tag_count, name_count)? {
        eprintln!("已取消寫入。");
        return Ok(2);
    }

    let mut code = 0;
    if let Some(audio) = audio {
        if audio.writable > 0 {
            code = apply_audio(Arc::clone(&state), audio).await?;
        }
    }
    if !files.ready_paths.is_empty() {
        let file_code = apply_files(state, files).await?;
        code = merge_exit_codes(code, file_code);
    }
    Ok(code)
}

async fn run_files(
    state: Arc<CoreState>,
    parsed: &mut ParsedCli,
    settings: Option<&Value>,
) -> Result<i32, String> {
    let prepared = prepare_files(Arc::clone(&state), parsed, settings).await?;
    if prepared.ready_paths.is_empty() {
        eprintln!("沒有可寫入的檔案。");
        return Ok(
            if prepared
                .plan
                .items
                .iter()
                .any(|item| item.status == PlanStatus::Error)
            {
                1
            } else {
                0
            },
        );
    }
    if !confirm_write(parsed, prepared.ready_paths.len(), "個檔案")? {
        eprintln!("已取消寫入。");
        return Ok(2);
    }
    apply_files(state, prepared).await
}

async fn prepare_files(
    state: Arc<CoreState>,
    parsed: &ParsedCli,
    settings: Option<&Value>,
) -> Result<PreparedFileWrite, String> {
    let vocabulary_correction = resolve_vocabulary_correction(parsed, settings)?;
    let direction = parsed.direction;
    let request = FilePlanRequest {
        paths: parsed.paths.clone(),
        output_path: parsed.output_path.clone(),
        output_directory: None,
        mode: parsed.operation,
        // 與 GUI 檔案頁／音訊掃描預設一致，目錄輸入會遞迴。
        recursive: true,
        input_encoding: parsed.input_encoding,
        output_encoding: parsed.output_encoding,
        add_bom: settings
            .and_then(|value| value.pointer("/files/unicodeAddBom"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        fix_charset_declaration: true,
        fix_charset_extensions: Some(fix_charset_extensions(settings)),
        allowed_extensions: Some(allowed_extensions(settings)),
        preview_max_bytes: settings
            .and_then(|value| value.get("previewMaxKb"))
            .and_then(Value::as_u64)
            .map(|kb| kb.saturating_mul(1024)),
        conflict_policy: ConflictPolicy::Skip,
        backup: Some(parsed.backup),
        conversion: ConversionOptions {
            direction,
            engine: parsed.engine,
            dictionary_path: settings
                .and_then(|value| value.get("dictionaryPath"))
                .and_then(Value::as_str)
                .map(str::to_string),
            zhconvert: Some(zhconvert_options(settings, direction)),
            vocabulary_correction: Some(vocabulary_correction),
        },
    };

    eprintln!("正在建立檔案轉換計劃…");
    let plan_value = dispatch_op(
        Arc::clone(&state),
        "files.plan",
        serde_json::to_value(request).map_err(|e| e.to_string())?,
    )
    .await?;
    let plan: FileConversionPlan =
        serde_json::from_value(plan_value).map_err(|e| format!("無法解析檔案計劃：{e}"))?;

    print_file_plan_summary(&plan);
    if !plan.warnings.is_empty() {
        for warning in &plan.warnings {
            eprintln!("警告：{warning}");
        }
    }

    let ready_paths: Vec<String> = plan
        .items
        .iter()
        .filter(|item| item.status == PlanStatus::Ready && item.selected)
        .map(|item| item.source_path.clone())
        .collect();
    Ok(PreparedFileWrite { plan, ready_paths })
}

async fn apply_files(state: Arc<CoreState>, prepared: PreparedFileWrite) -> Result<i32, String> {
    eprintln!("正在寫入…");
    let apply_value = dispatch_op(
        state,
        "files.apply",
        json!({
            "planId": prepared.plan.plan_id,
            "selectedPaths": prepared.ready_paths,
        }),
    )
    .await?;
    let result: ApplyResult =
        serde_json::from_value(apply_value).map_err(|e| format!("無法解析寫入結果：{e}"))?;
    print_apply_result(&result);
    Ok(apply_exit_code(&result))
}

async fn run_audio(
    state: Arc<CoreState>,
    parsed: &mut ParsedCli,
    settings: Option<&Value>,
) -> Result<i32, String> {
    let Some(prepared) = prepare_audio(Arc::clone(&state), parsed, settings).await? else {
        return Ok(1);
    };
    if prepared.writable == 0 {
        eprintln!("沒有可寫入的音訊標籤。");
        return Ok(1);
    }
    if !confirm_write(parsed, prepared.writable, "個音訊檔案")? {
        eprintln!("已取消寫入。");
        return Ok(2);
    }
    apply_audio(state, prepared).await
}

async fn prepare_audio(
    state: Arc<CoreState>,
    parsed: &ParsedCli,
    settings: Option<&Value>,
) -> Result<Option<PreparedAudioWrite>, String> {
    let vocabulary_correction = resolve_vocabulary_correction(parsed, settings)?;
    let direction = parsed.direction;

    eprintln!("正在掃描音訊標籤…");
    let scanned_value = dispatch_op(
        Arc::clone(&state),
        "audio.scan",
        json!({
            "paths": parsed.paths,
            "recursive": true,
            "id3v1SourceEncoding": "gbk",
            "id3v2SourceEncoding": "gbk",
            "id3v2RepairSourceEncoding": true,
        }),
    )
    .await?;
    let scanned: Vec<AudioTagFile> =
        serde_json::from_value(scanned_value).map_err(|e| format!("無法解析掃描結果：{e}"))?;

    let selected_paths: Vec<String> = scanned
        .iter()
        .filter(|file| file.selected && file.warning.is_none())
        .map(|file| file.path.clone())
        .collect();
    let selected_fields: HashMap<String, Vec<String>> = scanned
        .iter()
        .map(|file| {
            (
                file.path.clone(),
                file.fields
                    .iter()
                    .filter(|field| field.selected)
                    .map(field_identifier)
                    .collect(),
            )
        })
        .collect();

    if selected_paths.is_empty() {
        eprintln!("沒有可轉換的音訊檔案。");
        for file in &scanned {
            if let Some(warning) = &file.warning {
                eprintln!("  {}：{warning}", file.path);
            }
        }
        return Ok(None);
    }

    let zh = zhconvert_options(settings, direction);
    let request = AudioTagPlanRequest {
        paths: parsed.paths.clone(),
        recursive: Some(true),
        id3v1_source_encoding: Some(TextEncoding::Gbk),
        id3v2_source_encoding: Some(TextEncoding::Gbk),
        id3v2_repair_source_encoding: Some(true),
        selected_paths: selected_paths.clone(),
        selected_fields,
        conversion: ConversionOptions {
            direction,
            engine: parsed.engine,
            dictionary_path: settings
                .and_then(|value| value.get("dictionaryPath"))
                .and_then(Value::as_str)
                .map(str::to_string),
            zhconvert: Some(zh.clone()),
            vocabulary_correction: Some(vocabulary_correction),
        },
        conflict_policy: ConflictPolicy::Skip,
        backup: Some(parsed.backup),
        id3v1_enabled: true,
        id3v1_direction: direction,
        id3v1_zhconvert: Some(zh.clone()),
        id3v1_output_encoding: TextEncoding::Big5,
        id3v2_enabled: true,
        id3v2_direction: direction,
        id3v2_zhconvert: Some(zh),
        id3v2_version: 4,
        id3v2_encoding: "utf8".into(),
    };

    eprintln!("正在建立音訊標籤計劃…");
    let plan_value = dispatch_op(
        Arc::clone(&state),
        "audio.plan",
        serde_json::to_value(request).map_err(|e| e.to_string())?,
    )
    .await?;
    let plan: AudioTagPlan =
        serde_json::from_value(plan_value).map_err(|e| format!("無法解析音訊計劃：{e}"))?;

    print_audio_plan_summary(&plan);
    if !plan.warnings.is_empty() {
        for warning in &plan.warnings {
            eprintln!("警告：{warning}");
        }
    }

    let writable = plan
        .files
        .iter()
        .filter(|file| file.selected && file.warning.is_none())
        .filter(|file| file.fields.iter().any(|field| field.selected))
        .count();
    Ok(Some(PreparedAudioWrite { plan, writable }))
}

async fn apply_audio(state: Arc<CoreState>, prepared: PreparedAudioWrite) -> Result<i32, String> {
    eprintln!("正在寫入…");
    let apply_value = dispatch_op(
        state,
        "audio.apply",
        json!({ "planId": prepared.plan.plan_id }),
    )
    .await?;
    let result: ApplyResult =
        serde_json::from_value(apply_value).map_err(|e| format!("無法解析寫入結果：{e}"))?;
    print_apply_result(&result);
    Ok(apply_exit_code(&result))
}

async fn dispatch_op(
    state: Arc<CoreState>,
    operation: &str,
    payload: Value,
) -> Result<Value, String> {
    let request_id = Uuid::new_v4().to_string();
    state.begin_request(&request_id);
    let progress = Arc::new(|event: ProgressEvent| {
        if event.total > 0 {
            eprint!("\r[{}/{}] {}", event.current, event.total, event.message);
            let _ = io::stderr().flush();
        }
    });
    let result = dispatch(state.clone(), operation, payload, progress, &request_id).await;
    state.finish_request(&request_id);
    eprintln!();
    result.map_err(|error: CoreError| error.message)
}

fn confirm_write(parsed: &mut ParsedCli, count: usize, unit: &str) -> Result<bool, String> {
    confirm_yes_no(parsed, &format!("將寫入 {count} {unit}。"), "是否繼續？")
}

fn confirm_tags_then_rename(
    parsed: &mut ParsedCli,
    tag_count: usize,
    name_count: usize,
) -> Result<bool, String> {
    confirm_yes_no(
        parsed,
        &format!("將先寫入標籤（{tag_count} 個音訊檔案），再轉換檔名（{name_count} 項）。"),
        "是否繼續？",
    )
}

fn confirm_yes_no(parsed: &mut ParsedCli, summary: &str, question: &str) -> Result<bool, String> {
    eprintln!("{summary}");
    if parsed.confirm_write {
        eprintln!("{question}（已確認）。");
        return Ok(true);
    }
    if !io::stdin().is_terminal() {
        return Err("非互動環境請加上 --yes 或 -y 以確認寫入。".into());
    }
    eprint!("{question}[是/否] ");
    let _ = io::stderr().flush();
    let mut line = String::new();
    io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|error| format!("讀取確認失敗：{error}"))?;
    Ok(is_yes_answer(&line))
}

fn is_yes_answer(line: &str) -> bool {
    matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes" | "是"
    )
}

fn print_file_plan_summary(plan: &FileConversionPlan) {
    eprintln!("檔案轉換計劃（{} 項）：", plan.items.len());
    for item in &plan.items {
        let status = match item.status {
            PlanStatus::Ready => "就緒",
            PlanStatus::Skipped => "略過",
            PlanStatus::Conflict => "衝突",
            PlanStatus::Error => "錯誤",
        };
        eprintln!("  [{status}] {} → {}", item.source_path, item.output_path);
        if let Some(warning) = &item.warning {
            eprintln!("           {warning}");
        }
    }
}

fn print_audio_plan_summary(plan: &AudioTagPlan) {
    eprintln!("音訊標籤計劃（{} 檔）：", plan.files.len());
    for file in &plan.files {
        let selected_fields: Vec<_> = file.fields.iter().filter(|field| field.selected).collect();
        if let Some(warning) = &file.warning {
            eprintln!("  [警告] {}：{warning}", file.path);
            continue;
        }
        if !file.selected || selected_fields.is_empty() {
            eprintln!("  [略過] {}", file.path);
            continue;
        }
        eprintln!("  [就緒] {}", file.path);
        for field in selected_fields {
            let before = field.values.join(" / ");
            let after = field
                .converted_values
                .as_ref()
                .map(|values| values.join(" / "))
                .unwrap_or_else(|| before.clone());
            eprintln!("           {}：{} → {}", field.label, before, after);
        }
    }
}

fn print_apply_result(result: &ApplyResult) {
    eprintln!(
        "完成：成功 {}、略過 {}、失敗 {}",
        result.succeeded.len(),
        result.skipped.len(),
        result.failed.len()
    );
    for path in &result.succeeded {
        eprintln!("  成功：{path}");
    }
    for path in &result.skipped {
        eprintln!("  略過：{path}");
    }
    for failure in &result.failed {
        eprintln!("  失敗：{}（{}）", failure.path, failure.message);
    }
}

fn apply_exit_code(result: &ApplyResult) -> i32 {
    if result.failed.is_empty() {
        0
    } else if result.succeeded.is_empty() {
        1
    } else {
        3
    }
}

fn field_identifier(field: &AudioTagField) -> String {
    let container = match field.container {
        super::types::AudioContainer::Id3v1 => "id3v1",
        super::types::AudioContainer::Id3v2 => "id3v2",
        super::types::AudioContainer::Apev2 => "apev2",
        super::types::AudioContainer::VorbisComment => "vorbis-comment",
    };
    format!("{container}:{}", field.key)
}

fn apply_settings_defaults(parsed: &mut ParsedCli, settings: Option<&Value>) -> Result<(), String> {
    if let Some(settings) = settings {
        if !parsed.direction_explicit {
            parsed.direction = direction_from_settings(settings);
            parsed.direction_explicit = true;
        }
        if !parsed.engine_explicit {
            parsed.engine = engine_from_settings(settings);
            parsed.engine_explicit = true;
        }
        if !parsed.backup_explicit {
            parsed.backup = settings
                .get("autoBackupBeforeConversion")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            parsed.backup_explicit = true;
        }
        if !parsed.input_encoding_explicit {
            let recognize = settings
                .get("recognizeEncoding")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            parsed.input_encoding = if recognize {
                TextEncoding::Auto
            } else {
                TextEncoding::Utf8
            };
            parsed.input_encoding_explicit = true;
        }
        return Ok(());
    }

    if !parsed.direction_explicit {
        return Err(
            "無頭模式未載入設定時必須指定 --direction（s2t｜t2s｜none）；或加上 --globalconfig／--config。"
                .into(),
        );
    }
    if matches!(parsed.vocabulary_correction, VocabularyCorrection::Settings) {
        if parsed.vocabulary_explicit {
            return Err(
                "無頭模式未載入設定時不可使用 --vocabulary settings；請改用 on／off，或加上 --globalconfig／--config。"
                    .into(),
            );
        }
        // 未指定時採固定預設 on（不讀設定檔）。
        parsed.vocabulary_correction = VocabularyCorrection::Enabled;
    }
    Ok(())
}

fn resolve_vocabulary_correction(
    parsed: &ParsedCli,
    settings: Option<&Value>,
) -> Result<bool, String> {
    match parsed.vocabulary_correction {
        VocabularyCorrection::Enabled => Ok(true),
        VocabularyCorrection::Disabled => Ok(false),
        VocabularyCorrection::Settings => {
            let Some(settings) = settings else {
                return Err(
                    "無頭模式未載入設定時不可使用 --vocabulary settings；請改用 on／off，或加上 --globalconfig／--config。"
                        .into(),
                );
            };
            Ok(settings
                .get("vocabularyCorrection")
                .and_then(Value::as_bool)
                .unwrap_or(true))
        }
    }
}

fn engine_from_settings(settings: &Value) -> EngineKind {
    match settings.get("engine").and_then(Value::as_str) {
        Some("legacy") => EngineKind::Legacy,
        Some("zhconvert") => EngineKind::Zhconvert,
        _ => EngineKind::Segmented,
    }
}

fn direction_from_settings(settings: &Value) -> Direction {
    match settings.get("direction").and_then(Value::as_str) {
        Some("t2s") => Direction::T2s,
        Some("none") => Direction::None,
        _ => Direction::S2t,
    }
}

fn zhconvert_options(settings: Option<&Value>, direction: Direction) -> ZhConvertOptions {
    let Some(settings) = settings else {
        return ZhConvertOptions {
            converter: Some(match direction {
                Direction::T2s => "Simplified".into(),
                _ => "Taiwan".into(),
            }),
            ..ZhConvertOptions::default()
        };
    };
    let zh = settings.get("zhconvert");
    let converter = match direction {
        Direction::T2s => zh
            .and_then(|value| value.get("converterT2S"))
            .and_then(Value::as_str)
            .map(str::to_string),
        _ => zh
            .and_then(|value| value.get("converterS2T"))
            .and_then(Value::as_str)
            .map(str::to_string),
    };
    let modules = zh
        .and_then(|value| value.get("modules"))
        .and_then(|value| serde_json::from_value::<ZhConvertModules>(value.clone()).ok());
    ZhConvertOptions {
        converter,
        modules,
        jp_text_conversion_strategy: zh
            .and_then(|value| value.get("jpTextConversionStrategy"))
            .and_then(Value::as_str)
            .map(str::to_string),
        jp_style_conversion_strategy: zh
            .and_then(|value| value.get("jpStyleConversionStrategy"))
            .and_then(Value::as_str)
            .map(str::to_string),
        clean_up_text: zh
            .and_then(|value| value.get("cleanUpText"))
            .and_then(Value::as_bool),
        user_pre_replace: zh
            .and_then(|value| value.get("userPreReplace"))
            .and_then(Value::as_str)
            .map(str::to_string),
        user_post_replace: zh
            .and_then(|value| value.get("userPostReplace"))
            .and_then(Value::as_str)
            .map(str::to_string),
        user_protect_replace: zh
            .and_then(|value| value.get("userProtectReplace"))
            .and_then(Value::as_str)
            .map(str::to_string),
        ensure_newline_at_eof: zh
            .and_then(|value| value.get("ensureNewlineAtEof"))
            .and_then(Value::as_bool),
        translate_tabs_to_spaces: zh
            .and_then(|value| value.get("translateTabsToSpaces"))
            .and_then(Value::as_i64),
        trim_trailing_white_spaces: zh
            .and_then(|value| value.get("trimTrailingWhiteSpaces"))
            .and_then(Value::as_bool),
        unify_leading_hyphen: zh
            .and_then(|value| value.get("unifyLeadingHyphen"))
            .and_then(Value::as_bool),
        ignore_text_styles: zh
            .and_then(|value| value.get("ignoreTextStyles"))
            .and_then(Value::as_str)
            .map(str::to_string),
        jp_text_styles: zh
            .and_then(|value| value.get("jpTextStyles"))
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

fn fix_charset_extensions(settings: Option<&Value>) -> Vec<String> {
    if let Some(items) = settings
        .and_then(|value| value.pointer("/files/fixCharsetExtensions"))
        .and_then(Value::as_array)
    {
        return items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
    }
    DEFAULT_FIX_CHARSET_EXTENSIONS
        .iter()
        .map(|item| (*item).to_string())
        .collect()
}

fn allowed_extensions(settings: Option<&Value>) -> Vec<String> {
    if let Some(settings) = settings {
        if let Some(from_filter) = allowed_extensions_from_settings(settings) {
            return from_filter;
        }
    }
    DEFAULT_ALLOWED_EXTENSIONS
        .iter()
        .map(|item| (*item).to_string())
        .collect()
}

fn allowed_extensions_from_settings(settings: &Value) -> Option<Vec<String>> {
    let filter = settings
        .pointer("/files/typeFilter")
        .and_then(Value::as_str)?;
    let mut extensions = Vec::new();
    for matched in filter.match_indices('<') {
        let rest = &filter[matched.0..];
        let Some(end) = rest.find('>') else {
            continue;
        };
        let inner = &rest[1..end];
        let Some((_, patterns)) = inner.split_once('|') else {
            continue;
        };
        for pattern in patterns.split(';') {
            let extension = pattern
                .trim()
                .trim_start_matches('*')
                .trim_start_matches('.')
                .to_ascii_lowercase();
            if extension.is_empty() || extension == "*" {
                continue;
            }
            let dotted = format!(".{extension}");
            if !extensions.contains(&dotted) {
                extensions.push(dotted);
            }
        }
    }
    if extensions.is_empty() {
        None
    } else {
        Some(extensions)
    }
}

fn load_cli_settings(parsed: &ParsedCli) -> Result<Option<Value>, String> {
    if let Some(path) = &parsed.config_path {
        return Ok(Some(load_settings_from_path(Path::new(path))?));
    }
    if parsed.use_global_config {
        return Ok(Some(load_global_settings()?));
    }
    Ok(None)
}

fn load_global_settings() -> Result<Value, String> {
    if let Some(path) = portable_settings_store_path() {
        if path.is_file() {
            return load_settings_from_path(&path);
        }
    }
    if let Some(path) = installed_settings_store_path() {
        if path.is_file() {
            return load_settings_from_path(&path);
        }
    }
    for candidate in legacy_settings_candidates() {
        if candidate.is_file() {
            return migrate_from_path(&candidate.to_string_lossy())
                .map_err(|error| format!("讀取全域舊設定失敗：{}", error.message));
        }
    }
    Err(
        "找不到全域設定（settings-v2.json 或 ConvertZZ.json）。請先在 GUI 儲存設定，或改用 --config <路徑>／命令列參數。"
            .into(),
    )
}

fn load_settings_from_path(path: &Path) -> Result<Value, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| format!("讀取設定檔失敗（{}）：{error}", path.display()))?;
    let trimmed = raw.trim_start_matches('\u{feff}');
    let parsed: Value = serde_json::from_str(trimmed)
        .map_err(|error| format!("設定檔 JSON 無效（{}）：{error}", path.display()))?;
    Ok(normalize_settings_document(parsed))
}

fn normalize_settings_document(raw: Value) -> Value {
    // plugin-store／可攜 store 外層是 { settings: {...}, ... }；亦可直接給 SettingsV2／舊版 JSON。
    let input = match raw.get("settings") {
        Some(inner) if inner.is_object() => inner.clone(),
        _ => raw,
    };
    migrate(input)
}

fn portable_settings_store_path() -> Option<PathBuf> {
    let directory = std::env::current_exe().ok()?.parent()?.to_path_buf();
    if !directory.join(PORTABLE_MARKER).is_file() {
        return None;
    }
    Some(directory.join(SETTINGS_STORE_FILE))
}

fn installed_settings_store_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var_os("APPDATA")?;
        return Some(
            PathBuf::from(appdata)
                .join(APP_IDENTIFIER)
                .join(SETTINGS_STORE_FILE),
        );
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")?;
        return Some(
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join(APP_IDENTIFIER)
                .join(SETTINGS_STORE_FILE),
        );
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(".local").join("share"))
            })?;
        Some(base.join(APP_IDENTIFIER).join(SETTINGS_STORE_FILE))
    }
}

fn legacy_settings_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            candidates.push(directory.join("ConvertZZ.json"));
        }
    }
    if let Ok(directory) = std::env::current_dir() {
        candidates.push(directory.join("ConvertZZ.json"));
    }
    candidates
}

fn load_zhconvert_api_key_quiet() -> Option<String> {
    let entry = keyring::Entry::new(APP_IDENTIFIER, "zhconvert-api-key").ok()?;
    match entry.get_password() {
        Ok(value) => Some(value),
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests;

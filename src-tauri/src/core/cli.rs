use super::types::{
    CliMode, Direction, EngineKind, FileMode, ParsedCli, TextEncoding, VocabularyCorrection,
};

/// 解析命令列。2.0 以 `--flag` 為準；舊版 `/file`、`/o:utf8` 等仍可解析但不做後續擴充。
pub fn parse_cli(args: &[String], default_engine: Option<EngineKind>) -> ParsedCli {
    let mut explicit_mode = false;
    let mut file_flag = false;
    let mut audio_flag = false;
    let mut filename_flag = false;
    let mut operation_explicit = false;
    let mut output_encoding_explicit = false;
    let mut output_flag_seen = false;
    let mut input_encoding_explicit = false;
    let mut direction_explicit = false;
    let mut engine_explicit = false;
    let mut vocabulary_explicit = false;
    let mut backup_explicit = false;
    let mut headless = false;
    let mut confirm_write = false;
    let mut use_global_config = false;
    let mut config_path: Option<String> = None;
    let mut parse_errors = Vec::new();
    let mut parsed = ParsedCli {
        mode: CliMode::Interactive,
        paths: Vec::new(),
        output_path: None,
        input_encoding: TextEncoding::Auto,
        output_encoding: TextEncoding::Auto,
        direction: Direction::None,
        engine: default_engine.unwrap_or(EngineKind::Segmented),
        operation: FileMode::Content,
        vocabulary_correction: VocabularyCorrection::Settings,
        backup: true,
        headless: false,
        confirm_write: false,
        output_encoding_explicit: false,
        input_encoding_explicit: false,
        direction_explicit: false,
        engine_explicit: false,
        vocabulary_explicit: false,
        backup_explicit: false,
        use_global_config: false,
        config_path: None,
        parse_errors: Vec::new(),
    };

    let mut index = 0;
    while index < args.len() {
        let raw = &args[index];
        let lowered = raw.to_ascii_lowercase();

        // --flag= 但值為空：split_long_equals 會略過，改記缺值錯誤以免當成路徑。
        if let Some((flag, rest)) = raw.split_once('=') {
            if flag.starts_with('-') && rest.is_empty() && is_valued_flag(flag) {
                parse_errors.push(format!("{flag} 需要值"));
                if is_output_flag(flag) {
                    output_flag_seen = true;
                }
                index += 1;
                continue;
            }
        }

        // 新式：--flag=value（路徑保留原始大小寫）
        if let Some((flag, value)) = split_long_equals(raw) {
            match flag.to_ascii_lowercase().as_str() {
                "--input" | "-i" => {
                    if let Some(encoding) = parse_encoding_value(value) {
                        parsed.input_encoding = encoding;
                        input_encoding_explicit = true;
                    } else {
                        parse_errors.push(format!("無效的 {flag} 值：{value}"));
                    }
                }
                "--output" | "-o" => {
                    output_flag_seen = true;
                    if let Some(encoding) = parse_encoding_value(value) {
                        parsed.output_encoding = encoding;
                        output_encoding_explicit = true;
                    } else {
                        parse_errors.push(format!("無效的 {flag} 值：{value}"));
                    }
                }
                "--output-path" => parsed.output_path = Some(value.to_string()),
                "--direction" => {
                    if let Some(direction) = parse_direction_value(value) {
                        parsed.direction = direction;
                        direction_explicit = true;
                    } else {
                        parse_errors.push(format!("無效的 --direction 值：{value}"));
                    }
                }
                "--engine" => {
                    if let Some(engine) = parse_engine_value(value) {
                        parsed.engine = engine;
                        engine_explicit = true;
                    } else {
                        parse_errors.push(format!("無效的 --engine 值：{value}"));
                    }
                }
                "--vocabulary" => {
                    if let Some(vocab) = parse_vocabulary_value(value) {
                        parsed.vocabulary_correction = vocab;
                        vocabulary_explicit = true;
                    } else {
                        parse_errors.push(format!("無效的 --vocabulary 值：{value}"));
                    }
                }
                "--backup" => {
                    if let Some(flag) = parse_boolish(value) {
                        parsed.backup = flag;
                        backup_explicit = true;
                    } else {
                        parse_errors.push(format!("無效的 --backup 值：{value}"));
                    }
                }
                "--operation" => {
                    if let Some(operation) = parse_operation_value(value) {
                        parsed.operation = operation;
                        operation_explicit = true;
                    } else {
                        parse_errors.push(format!("無效的 --operation 值：{value}"));
                    }
                }
                "--config" => config_path = Some(value.to_string()),
                _ => parsed.paths.push(raw.clone()),
            }
            index += 1;
            continue;
        }

        match lowered.as_str() {
            // —— 2.0 新式 ——
            "--file" => {
                parsed.mode = CliMode::File;
                explicit_mode = true;
                file_flag = true;
            }
            "--audio" => {
                parsed.mode = CliMode::Audio;
                explicit_mode = true;
                audio_flag = true;
            }
            "--headless" => headless = true,
            "--yes" | "-y" => confirm_write = true,
            "--globalconfig" => use_global_config = true,
            "--no-backup" => {
                parsed.backup = false;
                backup_explicit = true;
            }
            "--backup" => {
                if let Some(value) = peek_value(args, index) {
                    if let Some(flag) = parse_boolish(&value.to_ascii_lowercase()) {
                        parsed.backup = flag;
                        backup_explicit = true;
                        index += 1;
                    } else {
                        parsed.backup = true;
                        backup_explicit = true;
                    }
                } else {
                    parsed.backup = true;
                    backup_explicit = true;
                }
            }
            "--input" | "-i" => {
                if let Some(value) =
                    take_required_value(args, &mut index, "--input", &mut parse_errors)
                {
                    if let Some(encoding) = parse_encoding_value(&value) {
                        parsed.input_encoding = encoding;
                        input_encoding_explicit = true;
                    } else {
                        parse_errors.push(format!("無效的 --input 值：{value}"));
                    }
                }
            }
            "--output" | "-o" => {
                output_flag_seen = true;
                if let Some(value) =
                    take_required_value(args, &mut index, "--output", &mut parse_errors)
                {
                    if let Some(encoding) = parse_encoding_value(&value) {
                        parsed.output_encoding = encoding;
                        output_encoding_explicit = true;
                    } else {
                        parse_errors.push(format!("無效的 --output 值：{value}"));
                    }
                }
            }
            "--output-path" => {
                if let Some(value) =
                    take_required_value(args, &mut index, "--output-path", &mut parse_errors)
                {
                    parsed.output_path = Some(value);
                }
            }
            "--direction" => {
                if let Some(value) =
                    take_required_value(args, &mut index, "--direction", &mut parse_errors)
                {
                    if let Some(direction) = parse_direction_value(&value) {
                        parsed.direction = direction;
                        direction_explicit = true;
                    } else {
                        parse_errors.push(format!("無效的 --direction 值：{value}"));
                    }
                }
            }
            "--engine" => {
                if let Some(value) =
                    take_required_value(args, &mut index, "--engine", &mut parse_errors)
                {
                    if let Some(engine) = parse_engine_value(&value) {
                        parsed.engine = engine;
                        engine_explicit = true;
                    } else {
                        parse_errors.push(format!("無效的 --engine 值：{value}"));
                    }
                }
            }
            "--vocabulary" => {
                if let Some(value) =
                    take_required_value(args, &mut index, "--vocabulary", &mut parse_errors)
                {
                    if let Some(vocab) = parse_vocabulary_value(&value) {
                        parsed.vocabulary_correction = vocab;
                        vocabulary_explicit = true;
                    } else {
                        parse_errors.push(format!("無效的 --vocabulary 值：{value}"));
                    }
                }
            }
            "--operation" => {
                if let Some(value) =
                    take_required_value(args, &mut index, "--operation", &mut parse_errors)
                {
                    if let Some(operation) = parse_operation_value(&value) {
                        parsed.operation = operation;
                        operation_explicit = true;
                    } else {
                        parse_errors.push(format!("無效的 --operation 值：{value}"));
                    }
                }
            }
            "--config" => {
                if let Some(value) =
                    take_required_value(args, &mut index, "--config", &mut parse_errors)
                {
                    config_path = Some(value);
                }
            }
            // --filename 單獨＝只轉檔名；搭配 --file／--audio＝內容（或標籤）＋檔名。
            "--filename" => filename_flag = true,
            "--help" | "-h" => {
                // 由進入點處理；此處略過以免被當成路徑。
            }

            // —— 舊版相容（不再擴充）——
            "/file" => {
                parsed.mode = CliMode::File;
                explicit_mode = true;
                file_flag = true;
            }
            "/audio" => {
                parsed.mode = CliMode::Audio;
                explicit_mode = true;
                audio_flag = true;
            }
            "/headless" => headless = true,
            "/y" => confirm_write = true,
            "/i:ule" => {
                parsed.input_encoding = TextEncoding::Utf16le;
                input_encoding_explicit = true;
            }
            "/i:ube" => {
                parsed.input_encoding = TextEncoding::Utf16be;
                input_encoding_explicit = true;
            }
            "/i:utf8" => {
                parsed.input_encoding = TextEncoding::Utf8;
                input_encoding_explicit = true;
            }
            "/i:gbk" => {
                parsed.input_encoding = TextEncoding::Gbk;
                input_encoding_explicit = true;
            }
            "/i:big5" => {
                parsed.input_encoding = TextEncoding::Big5;
                input_encoding_explicit = true;
            }
            "/o:ule" => {
                parsed.output_encoding = TextEncoding::Utf16le;
                output_encoding_explicit = true;
            }
            "/o:ube" => {
                parsed.output_encoding = TextEncoding::Utf16be;
                output_encoding_explicit = true;
            }
            "/o:utf8" => {
                parsed.output_encoding = TextEncoding::Utf8;
                output_encoding_explicit = true;
            }
            "/o:gbk" => {
                parsed.output_encoding = TextEncoding::Gbk;
                output_encoding_explicit = true;
            }
            "/o:big5" => {
                parsed.output_encoding = TextEncoding::Big5;
                output_encoding_explicit = true;
            }
            "/f:t" => {
                parsed.direction = Direction::S2t;
                direction_explicit = true;
            }
            "/f:s" => {
                parsed.direction = Direction::T2s;
                direction_explicit = true;
            }
            "/f:d" => {
                parsed.direction = Direction::None;
                direction_explicit = true;
            }
            "/d:t" => {
                parsed.vocabulary_correction = VocabularyCorrection::Enabled;
                vocabulary_explicit = true;
            }
            "/d:f" => {
                parsed.vocabulary_correction = VocabularyCorrection::Disabled;
                vocabulary_explicit = true;
            }
            "/d:s" => {
                parsed.vocabulary_correction = VocabularyCorrection::Settings;
                vocabulary_explicit = true;
            }
            "/e:l" => {
                parsed.engine = EngineKind::Legacy;
                engine_explicit = true;
            }
            "/e:f" => {
                parsed.engine = EngineKind::Zhconvert;
                engine_explicit = true;
            }
            "/e:n" => {
                parsed.engine = EngineKind::Segmented;
                engine_explicit = true;
            }
            "/b:t" => {
                parsed.backup = true;
                backup_explicit = true;
            }
            "/b:f" => {
                parsed.backup = false;
                backup_explicit = true;
            }

            _ => parsed.paths.push(raw.clone()),
        }
        index += 1;
    }

    if !operation_explicit {
        if filename_flag && (file_flag || audio_flag) {
            parsed.operation = FileMode::Both;
        } else if filename_flag {
            parsed.operation = FileMode::Filename;
            parsed.mode = CliMode::File;
            // 不設 explicit_mode：純 --filename 搭配 --output 仍可走經典無頭。
        }
    }

    if !explicit_mode && !parsed.paths.is_empty() {
        if parsed.mode == CliMode::Interactive {
            parsed.mode = CliMode::File;
        }
        if parsed.output_path.is_none() && parsed.paths.len() > 1 {
            parsed.output_path = Some(parsed.paths[1].clone());
            parsed.paths.truncate(1);
        }
    }
    parsed.output_encoding_explicit = output_encoding_explicit;
    parsed.input_encoding_explicit = input_encoding_explicit;
    parsed.direction_explicit = direction_explicit;
    parsed.engine_explicit = engine_explicit;
    parsed.vocabulary_explicit = vocabulary_explicit;
    parsed.backup_explicit = backup_explicit;
    parsed.confirm_write = confirm_write;
    parsed.use_global_config = use_global_config;
    parsed.config_path = config_path;
    parsed.parse_errors = parse_errors;
    // 顯式 --headless，或經典：有路徑、出現 --output／-o（值無效仍算），且未指定 --file／--audio。
    parsed.headless = headless
        || (!file_flag
            && !audio_flag
            && !parsed.paths.is_empty()
            && (output_encoding_explicit || output_flag_seen));
    parsed
}

/// 僅依參數列判斷是否應走無頭 early-exit（不需先載入設定）。
pub fn args_request_headless(args: &[String]) -> bool {
    parse_cli(args, None).headless
}

pub fn args_request_help(args: &[String]) -> bool {
    args.iter().any(|arg| {
        let lowered = arg.to_ascii_lowercase();
        lowered == "--help" || lowered == "-h"
    })
}

pub fn print_cli_help() {
    eprintln!(
        "\
ConvertZZ 命令列

用法：
  ConvertZZ [選項] <來源路徑…> [輸出路徑]

2.0 選項：
  --file                 轉換檔案內容（預設不含檔名；GUI 預覽，搭配 --headless 則為 CLI）
  --audio                轉換音訊標籤（預設不含檔名）
  --filename             只轉檔名；與 --file／--audio 併用＝內容或標籤＋檔名（無頭 --audio --filename 先寫標籤再改名，只確認一次）
  --headless             無頭 CLI（不開視窗）
  --yes, -y              確認寫入（無頭非 TTY 必填；互動為是／否）
  --input, -i <編碼>     來源編碼：utf8｜utf16le｜utf16be｜gbk｜big5｜ule｜ube
  --output, -o <編碼>    輸出編碼（同上；明確指定且未用 --file／--audio 時會進無頭）
  --output-path <路徑>   輸出路徑（亦可寫成第二個位置參數）
  --direction <方向>     s2t｜t2s｜none（無頭且未載入設定時必填）
  --engine <引擎>        segmented｜legacy｜zhconvert（無頭預設 segmented）
  --vocabulary <值>      on｜off｜settings（無頭未載入設定時 settings 無效，預設 on）
  --globalconfig         無頭時載入本機全域／可攜設定
  --config <路徑>        無頭時載入指定設定檔（不可與 --globalconfig 併用）
  --operation <模式>     content｜filename｜both（進階；預設 content）
  --backup / --no-backup 轉換前 .bak 備份（預設開啟）
  --help, -h             顯示此說明

無頭預設不讀設定，請用命令列提供必要參數；需要設定時加上 --globalconfig 或 --config。
舊版 /file、/o:utf8、/f:t 等語法仍可使用，僅供相容，不再擴充。
"
    );
}

fn split_long_equals(arg: &str) -> Option<(&str, &str)> {
    let (flag, value) = arg.split_once('=')?;
    if !flag.starts_with('-') || value.is_empty() {
        return None;
    }
    Some((flag, value))
}

fn peek_value(args: &[String], index: usize) -> Option<&str> {
    args.get(index + 1)
        .map(String::as_str)
        // 只略過以 `-` 開頭的選項；Unix 路徑常以 `/` 開頭，不可過濾。
        .filter(|value| !value.starts_with('-'))
}

fn take_value(args: &[String], index: &mut usize) -> Option<String> {
    let value = peek_value(args, *index)?.to_string();
    *index += 1;
    Some(value)
}

fn take_required_value(
    args: &[String],
    index: &mut usize,
    flag: &str,
    errors: &mut Vec<String>,
) -> Option<String> {
    match take_value(args, index) {
        Some(value) => Some(value),
        None => {
            errors.push(format!("{flag} 需要值"));
            None
        }
    }
}

fn is_valued_flag(flag: &str) -> bool {
    matches!(
        flag.to_ascii_lowercase().as_str(),
        "--input"
            | "-i"
            | "--output"
            | "-o"
            | "--output-path"
            | "--direction"
            | "--engine"
            | "--vocabulary"
            | "--operation"
            | "--config"
            | "--backup"
    )
}

fn is_output_flag(flag: &str) -> bool {
    matches!(flag.to_ascii_lowercase().as_str(), "--output" | "-o")
}

fn parse_encoding_value(value: &str) -> Option<TextEncoding> {
    match value.to_ascii_lowercase().as_str() {
        "utf8" | "utf-8" => Some(TextEncoding::Utf8),
        "utf8-bom" | "utf-8-bom" => Some(TextEncoding::Utf8Bom),
        "utf16le" | "utf-16le" | "ule" => Some(TextEncoding::Utf16le),
        "utf16be" | "utf-16be" | "ube" => Some(TextEncoding::Utf16be),
        "gbk" | "gb2312" => Some(TextEncoding::Gbk),
        "big5" => Some(TextEncoding::Big5),
        "auto" => Some(TextEncoding::Auto),
        _ => None,
    }
}

fn parse_direction_value(value: &str) -> Option<Direction> {
    match value.to_ascii_lowercase().as_str() {
        "s2t" | "t" | "traditional" => Some(Direction::S2t),
        "t2s" | "s" | "simplified" => Some(Direction::T2s),
        "none" | "d" | "off" => Some(Direction::None),
        _ => None,
    }
}

fn parse_engine_value(value: &str) -> Option<EngineKind> {
    match value.to_ascii_lowercase().as_str() {
        "segmented" | "n" | "new" => Some(EngineKind::Segmented),
        "legacy" | "l" | "local" => Some(EngineKind::Legacy),
        "zhconvert" | "f" | "fanhuaji" => Some(EngineKind::Zhconvert),
        _ => None,
    }
}

fn parse_vocabulary_value(value: &str) -> Option<VocabularyCorrection> {
    match value.to_ascii_lowercase().as_str() {
        "on" | "true" | "enabled" | "t" => Some(VocabularyCorrection::Enabled),
        "off" | "false" | "disabled" | "f" => Some(VocabularyCorrection::Disabled),
        "settings" | "s" => Some(VocabularyCorrection::Settings),
        _ => None,
    }
}

fn parse_operation_value(value: &str) -> Option<FileMode> {
    match value.to_ascii_lowercase().as_str() {
        "content" => Some(FileMode::Content),
        "filename" => Some(FileMode::Filename),
        "both" => Some(FileMode::Both),
        _ => None,
    }
}

fn parse_boolish(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" | "t" => Some(true),
        "0" | "false" | "no" | "off" | "f" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests;

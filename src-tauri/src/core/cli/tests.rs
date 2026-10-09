use super::*;

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

#[test]
fn keeps_legacy_flags_and_new_engine() {
    let parsed = parse_cli(
        &args(&[
            "/file", "/i:gbk", "/o:big5", "/f:t", "/d:t", "/e:n", "book.txt",
        ]),
        None,
    );
    assert_eq!(parsed.mode, CliMode::File);
    assert_eq!(parsed.input_encoding, TextEncoding::Gbk);
    assert_eq!(parsed.output_encoding, TextEncoding::Big5);
    assert_eq!(parsed.operation, FileMode::Content);
    assert_eq!(parsed.direction, Direction::S2t);
    assert_eq!(parsed.engine, EngineKind::Segmented);
    assert_eq!(parsed.vocabulary_correction, VocabularyCorrection::Enabled);
    assert_eq!(parsed.paths, ["book.txt"]);
}

#[test]
fn recognizes_audio_and_legacy_engine() {
    let parsed = parse_cli(&args(&["/audio", "/e:l", "song.ape"]), None);
    assert_eq!(parsed.mode, CliMode::Audio);
    assert_eq!(parsed.engine, EngineKind::Legacy);
    assert_eq!(parsed.paths, ["song.ape"]);
    assert_eq!(
        parse_cli(
            &args(&["/audio", "a.mp3", "b.ape", "c.ogg", "d.opus"]),
            None
        )
        .paths,
        ["a.mp3", "b.ape", "c.ogg", "d.opus"]
    );
}

#[test]
fn maps_engine_flags() {
    assert_eq!(
        parse_cli(&args(&["/e:l", "book.txt"]), None).engine,
        EngineKind::Legacy
    );
    assert_eq!(
        parse_cli(&args(&["/e:f", "book.txt"]), None).engine,
        EngineKind::Zhconvert
    );
    assert_eq!(
        parse_cli(&args(&["/e:n", "book.txt"]), None).engine,
        EngineKind::Segmented
    );
}

#[test]
fn uses_saved_engine_when_unspecified() {
    assert_eq!(
        parse_cli(&args(&["book.txt"]), Some(EngineKind::Legacy)).engine,
        EngineKind::Legacy
    );
}

#[test]
fn keeps_t2s_and_disabled_dictionary() {
    let parsed = parse_cli(&args(&["/f:s", "/d:f", "book.txt"]), None);
    assert_eq!(parsed.mode, CliMode::File);
    assert_eq!(parsed.direction, Direction::T2s);
    assert_eq!(parsed.vocabulary_correction, VocabularyCorrection::Disabled);
    assert_eq!(parsed.paths, ["book.txt"]);
}

#[test]
fn backup_defaults_true_and_can_disable() {
    assert!(parse_cli(&args(&["/file", "book.txt"]), None).backup);
    assert!(!parse_cli(&args(&["/file", "/b:f", "book.txt"]), None).backup);
    assert!(parse_cli(&args(&["/audio", "/b:t", "song.mp3"]), None).backup);
}

#[test]
fn keeps_input_and_output_path_semantics() {
    let parsed = parse_cli(&args(&["/f:t", "books/*.txt", "converted/*.txt"]), None);
    assert_eq!(parsed.mode, CliMode::File);
    assert_eq!(parsed.paths, ["books/*.txt"]);
    assert_eq!(parsed.output_path.as_deref(), Some("converted/*.txt"));
}

#[test]
fn explicit_file_mode_keeps_multiple_sources() {
    let parsed = parse_cli(&args(&["/file", "a.txt", "b.txt"]), None);
    assert_eq!(parsed.mode, CliMode::File);
    assert_eq!(parsed.paths, ["a.txt", "b.txt"]);
}

#[test]
fn preserves_raw_path_strings() {
    let parsed = parse_cli(
        &args(&[
            "/file",
            r"\\?\C:\Temp\里面.txt",
            r"\\?\UNC\server\share\converted.txt",
        ]),
        None,
    );
    assert_eq!(parsed.mode, CliMode::File);
    assert_eq!(
        parsed.paths,
        [
            r"\\?\C:\Temp\里面.txt",
            r"\\?\UNC\server\share\converted.txt"
        ]
    );
}

#[test]
fn classic_output_encoding_enters_headless() {
    let parsed = parse_cli(&args(&["/o:utf8", "/f:t", "book.txt", "out.txt"]), None);
    assert!(parsed.headless);
    assert!(parsed.output_encoding_explicit);
    assert!(!parsed.confirm_write);
    assert_eq!(parsed.mode, CliMode::File);
    assert_eq!(parsed.output_path.as_deref(), Some("out.txt"));
    assert!(args_request_headless(&args(&["/o:big5", "input.txt"])));
}

#[test]
fn explicit_file_or_audio_without_headless_stays_gui() {
    assert!(!parse_cli(&args(&["/file", "/o:utf8", "book.txt"]), None).headless);
    assert!(!parse_cli(&args(&["/audio", "song.mp3"]), None).headless);
    assert!(!args_request_headless(&args(&["/file", "book.txt"])));
}

#[test]
fn headless_flag_works_with_file_and_audio() {
    let file = parse_cli(&args(&["/headless", "/file", "/y", "a.txt"]), None);
    assert!(file.headless);
    assert!(file.confirm_write);
    assert_eq!(file.mode, CliMode::File);

    let audio = parse_cli(&args(&["--headless", "/audio", "--yes", "song.mp3"]), None);
    assert!(audio.headless);
    assert!(audio.confirm_write);
    assert_eq!(audio.mode, CliMode::Audio);
}

#[test]
fn path_only_without_output_encoding_stays_gui() {
    let parsed = parse_cli(&args(&["book.txt"]), None);
    assert!(!parsed.headless);
    assert_eq!(parsed.mode, CliMode::File);
}

#[test]
fn modern_flags_match_legacy_semantics() {
    let parsed = parse_cli(
        &args(&[
            "--file",
            "--input",
            "gbk",
            "--output=big5",
            "--direction",
            "s2t",
            "--vocabulary",
            "on",
            "--engine",
            "segmented",
            "--no-backup",
            "book.txt",
        ]),
        None,
    );
    assert_eq!(parsed.mode, CliMode::File);
    assert_eq!(parsed.input_encoding, TextEncoding::Gbk);
    assert_eq!(parsed.output_encoding, TextEncoding::Big5);
    assert!(parsed.output_encoding_explicit);
    assert!(parsed.direction_explicit);
    assert_eq!(parsed.direction, Direction::S2t);
    assert_eq!(parsed.engine, EngineKind::Segmented);
    assert_eq!(parsed.vocabulary_correction, VocabularyCorrection::Enabled);
    assert!(!parsed.backup);
    assert!(!parsed.headless);
    assert_eq!(parsed.paths, ["book.txt"]);
}

#[test]
fn modern_output_encoding_enters_headless() {
    let parsed = parse_cli(
        &args(&[
            "-o",
            "utf8",
            "--direction=s2t",
            "--yes",
            "book.txt",
            "out.txt",
        ]),
        None,
    );
    assert!(parsed.headless);
    assert!(parsed.confirm_write);
    assert_eq!(parsed.output_encoding, TextEncoding::Utf8);
    assert_eq!(parsed.output_path.as_deref(), Some("out.txt"));
    assert!(args_request_headless(&args(&[
        "--output",
        "big5",
        "input.txt"
    ])));
}

#[test]
fn modern_headless_file_and_audio() {
    let file = parse_cli(
        &args(&["--headless", "--file", "--filename", "-y", "a.txt"]),
        None,
    );
    assert!(file.headless);
    assert!(file.confirm_write);
    assert_eq!(file.mode, CliMode::File);
    assert_eq!(file.operation, FileMode::Both);

    let audio = parse_cli(&args(&["--headless", "--audio", "--yes", "song.mp3"]), None);
    assert!(audio.headless);
    assert_eq!(audio.mode, CliMode::Audio);
    assert_eq!(audio.operation, FileMode::Content);
}

#[test]
fn filename_flag_combines_with_file_or_audio() {
    // 預設 --file 只轉內容
    assert_eq!(
        parse_cli(&args(&["--file", "a.txt"]), None).operation,
        FileMode::Content
    );
    // --filename 單獨＝只轉檔名
    let only_name = parse_cli(&args(&["--filename", "a.txt"]), None);
    assert_eq!(only_name.mode, CliMode::File);
    assert_eq!(only_name.operation, FileMode::Filename);
    assert!(!only_name.headless);
    // --filename 搭配 --output 仍可走經典無頭（未用 --file／--audio）
    let named_headless = parse_cli(&args(&["--filename", "--output", "utf8", "a.txt"]), None);
    assert!(named_headless.headless);
    assert_eq!(named_headless.operation, FileMode::Filename);
    // --file --filename＝內容＋檔名
    assert_eq!(
        parse_cli(&args(&["--file", "--filename", "a.txt"]), None).operation,
        FileMode::Both
    );
    // --audio --filename＝標籤＋檔名
    let audio_both = parse_cli(&args(&["--audio", "--filename", "song.mp3"]), None);
    assert_eq!(audio_both.mode, CliMode::Audio);
    assert_eq!(audio_both.operation, FileMode::Both);
    assert_eq!(
        parse_cli(&args(&["--file", "--operation=filename", "a.txt"]), None).operation,
        FileMode::Filename
    );
}

#[test]
fn modern_output_path_flag_preserves_case() {
    let parsed = parse_cli(
        &args(&[
            "--headless",
            "--file",
            "--output-path=Out/Dir/File.TXT",
            "In.TXT",
        ]),
        None,
    );
    assert_eq!(parsed.paths, ["In.TXT"]);
    assert_eq!(parsed.output_path.as_deref(), Some("Out/Dir/File.TXT"));
}

#[test]
fn help_flag_detected() {
    assert!(args_request_help(&args(&["--help"])));
    assert!(args_request_help(&args(&["-h", "book.txt"])));
    assert!(!args_request_help(&args(&["--headless", "book.txt"])));
}

#[test]
fn config_flags_parse() {
    let global = parse_cli(
        &args(&["--headless", "--file", "--globalconfig", "a.txt"]),
        None,
    );
    assert!(global.use_global_config);
    assert!(global.config_path.is_none());
    assert!(!global.direction_explicit);

    let path = parse_cli(
        &args(&[
            "--headless",
            "--file",
            "--config",
            "/tmp/settings.json",
            "a.txt",
        ]),
        None,
    );
    assert!(!path.use_global_config);
    assert_eq!(path.config_path.as_deref(), Some("/tmp/settings.json"));

    let equals = parse_cli(
        &args(&["--headless", "--file", "--config=/opt/cz.json", "a.txt"]),
        None,
    );
    assert_eq!(equals.config_path.as_deref(), Some("/opt/cz.json"));
}

#[test]
fn engine_flag_is_explicit() {
    assert!(!parse_cli(&args(&["--file", "a.txt"]), None).engine_explicit);
    assert!(parse_cli(&args(&["--file", "--engine", "legacy", "a.txt"]), None).engine_explicit);
    assert!(parse_cli(&args(&["/file", "/e:n", "a.txt"]), None).engine_explicit);
}

#[test]
fn invalid_flag_values_collect_parse_errors() {
    let parsed = parse_cli(
        &args(&[
            "--headless",
            "--file",
            "--direction",
            "sideways",
            "--engine",
            "nope",
            "--output",
            "ebcdic",
            "a.txt",
        ]),
        None,
    );
    assert_eq!(parsed.parse_errors.len(), 3);
    assert!(parsed
        .parse_errors
        .iter()
        .any(|e| e.contains("--direction")));
    assert!(parsed.parse_errors.iter().any(|e| e.contains("--engine")));
    assert!(parsed.parse_errors.iter().any(|e| e.contains("--output")));
    assert!(!parsed.direction_explicit);
    assert!(!parsed.output_encoding_explicit);
    // 經典無頭：出現 --output 即使值無效仍走 CLI，不開 GUI。
    let classic_bad = parse_cli(&args(&["--output", "ebcdic", "book.txt"]), None);
    assert!(classic_bad.headless);
    assert!(!classic_bad.parse_errors.is_empty());
    assert!(args_request_headless(&args(&[
        "--output", "ebcdic", "book.txt"
    ])));
    // GUI：--file 即使有解析錯誤仍開預覽（由畫面提示）。
    let gui_bad = parse_cli(&args(&["--file", "--direction", "sideways", "a.txt"]), None);
    assert!(!gui_bad.headless);
    assert!(!gui_bad.parse_errors.is_empty());
    assert!(!args_request_headless(&args(&[
        "--file",
        "--direction",
        "sideways",
        "a.txt"
    ])));
}

#[test]
fn missing_flag_values_collect_parse_errors() {
    let parsed = parse_cli(
        &args(&["--output", "--direction", "s2t", "-y", "a.txt"]),
        None,
    );
    assert!(parsed
        .parse_errors
        .iter()
        .any(|e| e.contains("--output") && e.contains("需要值")));
    assert!(parsed.headless);
    let empty_equals = parse_cli(&args(&["--direction=", "a.txt"]), None);
    assert!(empty_equals
        .parse_errors
        .iter()
        .any(|e| e.contains("--direction") && e.contains("需要值")));
}

#[test]
fn backup_and_input_flags_are_explicit() {
    assert!(!parse_cli(&args(&["--file", "a.txt"]), None).backup_explicit);
    assert!(parse_cli(&args(&["--file", "--no-backup", "a.txt"]), None).backup_explicit);
    assert!(parse_cli(&args(&["--file", "/b:f", "a.txt"]), None).backup_explicit);
    assert!(parse_cli(&args(&["--file", "--input", "gbk", "a.txt"]), None).input_encoding_explicit);
    assert!(parse_cli(&args(&["--file", "/i:utf8", "a.txt"]), None).input_encoding_explicit);
}

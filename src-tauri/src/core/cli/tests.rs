use super::*;

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

#[test]
fn keeps_legacy_flags_and_new_engine() {
    let parsed = parse_legacy_cli(
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
    let parsed = parse_legacy_cli(&args(&["/audio", "/e:l", "song.ape"]), None);
    assert_eq!(parsed.mode, CliMode::Audio);
    assert_eq!(parsed.engine, EngineKind::Legacy);
    assert_eq!(parsed.paths, ["song.ape"]);
    assert_eq!(
        parse_legacy_cli(
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
        parse_legacy_cli(&args(&["/e:l", "book.txt"]), None).engine,
        EngineKind::Legacy
    );
    assert_eq!(
        parse_legacy_cli(&args(&["/e:f", "book.txt"]), None).engine,
        EngineKind::Zhconvert
    );
    assert_eq!(
        parse_legacy_cli(&args(&["/e:n", "book.txt"]), None).engine,
        EngineKind::Segmented
    );
}

#[test]
fn uses_saved_engine_when_unspecified() {
    assert_eq!(
        parse_legacy_cli(&args(&["book.txt"]), Some(EngineKind::Legacy)).engine,
        EngineKind::Legacy
    );
}

#[test]
fn keeps_t2s_and_disabled_dictionary() {
    let parsed = parse_legacy_cli(&args(&["/f:s", "/d:f", "book.txt"]), None);
    assert_eq!(parsed.mode, CliMode::File);
    assert_eq!(parsed.direction, Direction::T2s);
    assert_eq!(parsed.vocabulary_correction, VocabularyCorrection::Disabled);
    assert_eq!(parsed.paths, ["book.txt"]);
}

#[test]
fn backup_defaults_true_and_can_disable() {
    assert!(parse_legacy_cli(&args(&["/file", "book.txt"]), None).backup);
    assert!(!parse_legacy_cli(&args(&["/file", "/b:f", "book.txt"]), None).backup);
    assert!(parse_legacy_cli(&args(&["/audio", "/b:t", "song.mp3"]), None).backup);
}

#[test]
fn keeps_input_and_output_path_semantics() {
    let parsed = parse_legacy_cli(&args(&["/f:t", "books/*.txt", "converted/*.txt"]), None);
    assert_eq!(parsed.mode, CliMode::File);
    assert_eq!(parsed.paths, ["books/*.txt"]);
    assert_eq!(parsed.output_path.as_deref(), Some("converted/*.txt"));
}

#[test]
fn explicit_file_mode_keeps_multiple_sources() {
    let parsed = parse_legacy_cli(&args(&["/file", "a.txt", "b.txt"]), None);
    assert_eq!(parsed.mode, CliMode::File);
    assert_eq!(parsed.paths, ["a.txt", "b.txt"]);
}

#[test]
fn preserves_raw_path_strings() {
    let parsed = parse_legacy_cli(
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

use super::types::{
    CliMode, Direction, EngineKind, FileMode, ParsedCli, TextEncoding, VocabularyCorrection,
};

pub fn parse_legacy_cli(args: &[String], default_engine: Option<EngineKind>) -> ParsedCli {
    let mut explicit_mode = false;
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
    };
    for raw in args {
        match raw.to_ascii_lowercase().as_str() {
            "/file" => {
                parsed.mode = CliMode::File;
                explicit_mode = true;
            }
            "/audio" => {
                parsed.mode = CliMode::Audio;
                explicit_mode = true;
            }
            "/i:ule" => parsed.input_encoding = TextEncoding::Utf16le,
            "/i:ube" => parsed.input_encoding = TextEncoding::Utf16be,
            "/i:utf8" => parsed.input_encoding = TextEncoding::Utf8,
            "/i:gbk" => parsed.input_encoding = TextEncoding::Gbk,
            "/i:big5" => parsed.input_encoding = TextEncoding::Big5,
            "/o:ule" => parsed.output_encoding = TextEncoding::Utf16le,
            "/o:ube" => parsed.output_encoding = TextEncoding::Utf16be,
            "/o:utf8" => parsed.output_encoding = TextEncoding::Utf8,
            "/o:gbk" => parsed.output_encoding = TextEncoding::Gbk,
            "/o:big5" => parsed.output_encoding = TextEncoding::Big5,
            "/f:t" => parsed.direction = Direction::S2t,
            "/f:s" => parsed.direction = Direction::T2s,
            "/f:d" => parsed.direction = Direction::None,
            "/d:t" => parsed.vocabulary_correction = VocabularyCorrection::Enabled,
            "/d:f" => parsed.vocabulary_correction = VocabularyCorrection::Disabled,
            "/d:s" => parsed.vocabulary_correction = VocabularyCorrection::Settings,
            "/e:l" => parsed.engine = EngineKind::Legacy,
            "/e:f" => parsed.engine = EngineKind::Zhconvert,
            "/e:n" => parsed.engine = EngineKind::Segmented,
            "/b:t" => parsed.backup = true,
            "/b:f" => parsed.backup = false,
            _ => parsed.paths.push(raw.clone()),
        }
    }
    if !explicit_mode && !parsed.paths.is_empty() {
        parsed.mode = CliMode::File;
        if parsed.paths.len() > 1 {
            parsed.output_path = Some(parsed.paths[1].clone());
            parsed.paths.truncate(1);
        }
    }
    parsed
}

#[cfg(test)]
mod tests {
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
}

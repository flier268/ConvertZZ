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
mod tests;

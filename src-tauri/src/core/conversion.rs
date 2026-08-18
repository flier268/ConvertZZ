use super::dictionary::LegacyDictionary;
use super::error::CoreError;
use super::types::{ConversionRequest, ConversionResult, Direction};
use super::zhconvert::ZhConvertClient;
use cjk_convert_rs::{cjk2zht, cn2tw, tw2cn};
use novel_segment::{DoSegmentOptions, Segment, SegmentOptions};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

const MAX_CHUNK_CHARACTERS: usize = 8_192;

pub struct ConversionService {
    segmenter: Segment,
    dictionaries: Mutex<HashMap<PathBuf, (SystemTime, Arc<LegacyDictionary>)>>,
    default_dictionary: Option<PathBuf>,
    pub zhconvert: ZhConvertClient,
}

fn configure_segment_dict_root() {
    if std::env::var_os("NOVEL_SEGMENT_DICT_ROOT").is_some() {
        return;
    }
    let mut candidates = vec![
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/segment-dict"),
        PathBuf::from("src-tauri/resources/segment-dict"),
    ];
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            candidates.push(directory.join("segment-dict"));
            candidates.push(directory.join("resources/segment-dict"));
        }
    }
    if let Some(path) = candidates
        .into_iter()
        .find(|path| path.join("segment").is_dir())
    {
        // SAFETY: called during service construction before other threads share this env.
        std::env::set_var("NOVEL_SEGMENT_DICT_ROOT", path);
    }
}

impl ConversionService {
    pub fn new(default_dictionary: Option<PathBuf>) -> Result<Self, CoreError> {
        configure_segment_dict_root();
        let mut segmenter = Segment::new(SegmentOptions {
            auto_cjk: true,
            all_mod: true,
            ..SegmentOptions::default()
        });
        segmenter
            .use_default()
            .map_err(|error| CoreError::new("SEGMENTER", format!("無法初始化分詞引擎：{error}")))?;
        Ok(Self {
            segmenter,
            dictionaries: Mutex::new(HashMap::new()),
            default_dictionary,
            zhconvert: ZhConvertClient::new(),
        })
    }

    pub async fn convert(&self, request: ConversionRequest) -> Result<ConversionResult, CoreError> {
        let started = Instant::now();
        let mut warnings = Vec::new();
        let mut text = request.text;
        if request.direction != Direction::None && !text.is_empty() {
            if request.vocabulary_correction == Some(false) {
                text = base_convert(&text, request.direction);
                warnings.push("詞彙修正已停用。本次只執行 cjk-convert-rs 字形轉換。".into());
            } else if request.engine == super::types::EngineKind::Segmented {
                // Segmenting is CPU-bound; keep multi-thread runtimes responsive.
                text = run_cpu_bound(|| self.segmented_convert(&text, request.direction));
            } else if request.engine == super::types::EngineKind::Legacy {
                let path = request
                    .dictionary_path
                    .as_deref()
                    .map(PathBuf::from)
                    .or_else(|| self.default_dictionary.clone())
                    .ok_or_else(|| CoreError::new("DICTIONARY_MISSING", "找不到舊版字典。"))?;
                let dictionary = self.dictionary(&path)?;
                text = dictionary.replace(&text, request.direction, |value| {
                    base_convert(value, request.direction)
                });
                warnings.push(
                    "未命中字元使用跨平台 cjk-convert-rs，結果可能與舊版 Windows 映射略有差異。"
                        .into(),
                );
            } else {
                text = self
                    .zhconvert
                    .convert(&text, request.direction, request.zhconvert.as_ref())
                    .await?;
            }
        }
        Ok(ConversionResult {
            text,
            engine: request.engine,
            direction: request.direction,
            warnings,
            duration_ms: (started.elapsed().as_secs_f64() * 10_000.0).round() / 100.0,
        })
    }

    fn segmented_convert(&self, text: &str, direction: Direction) -> String {
        // Only run the expensive segmenter on CJK runs. HTML/JS/CSS/base64 stays on glyph path.
        let mut output = String::with_capacity(text.len());
        for run in split_cjk_runs(text) {
            match run {
                TextRun::Plain(plain) => output.push_str(&base_convert(plain, direction)),
                TextRun::Cjk(cjk) => {
                    for chunk in split_text(cjk) {
                        output.push_str(&self.segmented_convert_chunk(&chunk, direction));
                    }
                }
            }
        }
        output
    }

    fn segmented_convert_chunk(&self, chunk: &str, direction: Direction) -> String {
        let source = if direction == Direction::S2t {
            self.segmenter
                .do_segment_simple(
                    chunk,
                    DoSegmentOptions {
                        simple: Some(true),
                        strip_punctuation: Some(false),
                        strip_stopword: Some(false),
                        strip_space: Some(false),
                        convert_synonym: Some(false),
                        disable_modules: Vec::new(),
                    },
                )
                .into_iter()
                .map(|word| cjk2zht(&word))
                .collect::<String>()
        } else {
            chunk.to_string()
        };
        let words = self.segmenter.do_segment_simple(
            &source,
            DoSegmentOptions {
                simple: Some(true),
                strip_punctuation: Some(false),
                strip_stopword: Some(false),
                strip_space: Some(false),
                convert_synonym: Some(direction == Direction::S2t),
                disable_modules: Vec::new(),
            },
        );
        let segmented = words.join("");
        if direction == Direction::S2t {
            cjk2zht(&segmented)
        } else {
            base_convert(&segmented, direction)
        }
    }

    fn dictionary(&self, path: &Path) -> Result<Arc<LegacyDictionary>, CoreError> {
        let mtime = std::fs::metadata(path)?
            .modified()
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let mut cache = self
            .dictionaries
            .lock()
            .map_err(|_| CoreError::new("DICTIONARY_LOCK", "無法鎖定字典快取。"))?;
        if let Some((cached_mtime, dictionary)) = cache.get(path) {
            if *cached_mtime == mtime {
                return Ok(Arc::clone(dictionary));
            }
        }
        let dictionary = Arc::new(LegacyDictionary::load(path)?);
        cache.insert(path.to_path_buf(), (mtime, Arc::clone(&dictionary)));
        Ok(dictionary)
    }
}

pub fn base_convert(text: &str, direction: Direction) -> String {
    match direction {
        Direction::S2t => cn2tw(text),
        Direction::T2s => tw2cn(text),
        Direction::None => text.to_string(),
    }
}

fn run_cpu_bound<R>(work: impl FnOnce() -> R) -> R {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(work)
        }
        _ => work(),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum TextRun<'a> {
    Plain(&'a str),
    Cjk(&'a str),
}

fn is_cjk_char(character: char) -> bool {
    matches!(
        character,
        '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{20000}'..='\u{2A6DF}'
            | '\u{2A700}'..='\u{2B73F}'
            | '\u{2B740}'..='\u{2B81F}'
            | '\u{2B820}'..='\u{2CEAF}'
            | '\u{2CEB0}'..='\u{2EBEF}'
            | '\u{30000}'..='\u{3134F}'
    )
}

/// Split mixed text into pure CJK runs and everything else.
/// Non-CJK runs skip the segmenter; they only need glyph conversion.
fn split_cjk_runs(text: &str) -> Vec<TextRun<'_>> {
    let mut runs = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((start, first)) = chars.next() {
        let cjk = is_cjk_char(first);
        let mut end = start + first.len_utf8();
        while let Some(&(next_start, next)) = chars.peek() {
            if is_cjk_char(next) != cjk {
                break;
            }
            end = next_start + next.len_utf8();
            chars.next();
        }
        let slice = &text[start..end];
        if cjk {
            runs.push(TextRun::Cjk(slice));
        } else {
            runs.push(TextRun::Plain(slice));
        }
    }
    runs
}

fn split_text(text: &str) -> Vec<String> {
    let characters: Vec<char> = text.chars().collect();
    if characters.len() <= MAX_CHUNK_CHARACTERS {
        return vec![text.to_string()];
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    while characters.len() - start > MAX_CHUNK_CHARACTERS {
        let mut end = start + MAX_CHUNK_CHARACTERS;
        if let Some(offset) = characters[start..end]
            .iter()
            .rposition(|&character| character == '\n' || character == '。')
        {
            if offset > MAX_CHUNK_CHARACTERS / 2 {
                end = start + offset + 1;
            }
        }
        chunks.push(characters[start..end].iter().collect());
        start = end;
    }
    if start < characters.len() {
        chunks.push(characters[start..].iter().collect());
    }
    chunks
}

#[cfg(test)]
pub(crate) fn shared_conversion() -> &'static ConversionService {
    use std::sync::OnceLock;
    static SERVICE: OnceLock<ConversionService> = OnceLock::new();
    SERVICE.get_or_init(|| {
        ConversionService::new(Some(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../ConvertZZ/Dictionary.csv"),
        ))
        .unwrap()
    })
}

#[cfg(test)]
mod tests {
    use super::super::types::EngineKind;
    use super::*;

    fn service() -> &'static ConversionService {
        super::shared_conversion()
    }

    async fn convert(text: &str, direction: Direction, engine: EngineKind) -> String {
        service()
            .convert(ConversionRequest {
                text: text.into(),
                direction,
                engine,
                dictionary_path: None,
                zhconvert: None,
                vocabulary_correction: None,
            })
            .await
            .unwrap()
            .text
    }

    #[tokio::test]
    async fn segmented_s2t_golden_cases() {
        let service = service();
        for (source, expected) in [
            ("里面", "裡面"),
            ("皇后", "皇后"),
            ("头发", "頭髮"),
            ("开发", "開發"),
            ("面对表面", "面對表面"),
        ] {
            let result = service
                .convert(ConversionRequest {
                    text: source.into(),
                    direction: Direction::S2t,
                    engine: EngineKind::Segmented,
                    dictionary_path: None,
                    zhconvert: None,
                    vocabulary_correction: None,
                })
                .await
                .unwrap();
            assert_eq!(result.text, expected, "{source}");
        }
    }

    #[tokio::test]
    async fn segmented_t2s_golden_cases() {
        for (source, expected) in [
            ("裡面", "里面"),
            ("皇后", "皇后"),
            ("頭髮", "头发"),
            ("開發", "开发"),
        ] {
            assert_eq!(
                convert(source, Direction::T2s, EngineKind::Segmented).await,
                expected
            );
        }
    }

    #[tokio::test]
    async fn preserves_whitespace_and_punctuation() {
        assert_eq!(
            convert("里面  开发\n头发", Direction::S2t, EngineKind::Segmented).await,
            "裡面  開發\n頭髮"
        );
        assert_eq!(
            convert("里面  😀\n《A》", Direction::S2t, EngineKind::Segmented).await,
            "裡面  😀\n《A》"
        );
    }

    #[test]
    fn split_text_breaks_on_ideographic_full_stop_without_slicing_mid_char() {
        let source = format!("{}。{}", "甲".repeat(5_000), "乙".repeat(4_000));
        let chunks = split_text(&source);
        assert!(chunks.len() >= 2);
        assert!(chunks.iter().all(|chunk| {
            chunk.chars().next().is_some() && chunk.is_char_boundary(chunk.len())
        }));
        assert_eq!(chunks.concat(), source);
        assert!(chunks[0].ends_with('。'));
    }

    #[test]
    fn split_cjk_runs_keeps_markup_and_cjk_separate() {
        let runs = split_cjk_runs("<div>里面</div>");
        assert_eq!(
            runs,
            vec![
                TextRun::Plain("<div>"),
                TextRun::Cjk("里面"),
                TextRun::Plain("</div>"),
            ]
        );
    }

    #[tokio::test]
    async fn long_text_does_not_split_unicode() {
        let source = format!("{}😀里面", "里".repeat(9_000));
        let result = convert(&source, Direction::S2t, EngineKind::Segmented).await;
        assert!(result.ends_with("😀裡面"));
        assert!(!result.contains('�'));
    }

    #[tokio::test]
    async fn legacy_dictionary() {
        let result = convert("软件和头发", Direction::S2t, EngineKind::Legacy).await;
        assert!(result.contains("軟體"));
        assert!(result.contains("頭髮"));
    }

    #[tokio::test]
    async fn vocabulary_off_uses_glyph_only() {
        let result = service()
            .convert(ConversionRequest {
                text: "里面".into(),
                direction: Direction::S2t,
                engine: EngineKind::Segmented,
                dictionary_path: None,
                zhconvert: None,
                vocabulary_correction: Some(false),
            })
            .await
            .unwrap();
        assert_eq!(result.text, "里麵");
        assert!(result.warnings[0].contains("詞彙修正已停用"));
    }

    #[tokio::test]
    async fn mixed_html_like_content_stays_interactive() {
        // Long non-CJK markup with sparse CJK (same shape as saved web pages).
        let mut text = String::new();
        text.push_str("<!DOCTYPE html><html><head><style>");
        text.push_str(&"body{margin:0;}".repeat(2_000));
        text.push_str("</style><script>");
        text.push_str(&"var x='base64-like-".repeat(1_500));
        text.push_str("';</script></head><body>");
        text.push_str("<p>里面开发头发软件</p>");
        text.push_str(&"<div class='pad'>........</div>".repeat(1_000));
        text.push_str("<p>皇后面对表面</p></body></html>");

        let service = service();
        // Exclude dictionary load from the conversion budget.
        let _ = convert("里面", Direction::S2t, EngineKind::Segmented).await;

        let started = Instant::now();
        let result = service
            .convert(ConversionRequest {
                text: text.clone(),
                direction: Direction::S2t,
                engine: EngineKind::Segmented,
                dictionary_path: None,
                zhconvert: None,
                vocabulary_correction: None,
            })
            .await
            .unwrap();
        let elapsed = started.elapsed();
        assert!(result.text.contains("裡面"));
        assert!(result.text.contains("頭髮"));
        assert!(result.text.contains("皇后"));
        assert!(result.text.contains("<script>"));
        assert_eq!(result.text.chars().count(), text.chars().count());
        // Old path fed whole HTML into the segmenter (~60s debug / ~3s release for ~90KB).
        assert!(
            elapsed.as_secs_f64() < 2.0,
            "mixed HTML conversion too slow: {elapsed:?}"
        );
    }
}

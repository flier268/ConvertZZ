use super::dictionary::LegacyDictionary;
use super::error::CoreError;
use super::types::{ConversionRequest, ConversionResult, Direction};
use super::zhconvert::ZhConvertClient;
use cjk_convert_rs::{cjk2zht, cn2tw_min_with, tw2cn, ConvertOptions};
use novel_segment::{DoSegmentOptions, Segment, SegmentOptions};
use std::collections::HashMap;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

const MAX_CHUNK_CHARACTERS: usize = 8_192;

/// Dictionary cache identity. mtime alone is insufficient on filesystems with one-second
/// resolution; length catches most edits, and platform identity catches atomic replaces that
/// keep the same byte length (Unix inode/dev; Windows creation time — `file_index` is nightly-only).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DictionaryStamp {
    modified: SystemTime,
    len: u64,
    identity: u64,
}

impl DictionaryStamp {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            len: metadata.len(),
            identity: file_identity(metadata),
        }
    }
}

fn file_identity(metadata: &Metadata) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        metadata.ino() ^ metadata.dev().rotate_left(32)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Stable stand-in for file_index: a replaced file keeps forced mtime/len but gets a
        // new creation_time after write-temp + rename.
        metadata.creation_time()
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        0
    }
}

pub struct ConversionService {
    segmenter: Segment,
    dictionaries: Mutex<HashMap<PathBuf, (DictionaryStamp, Arc<LegacyDictionary>)>>,
    default_dictionary: Option<PathBuf>,
    pub zhconvert: ZhConvertClient,
}

fn segment_dict_candidates(executable: Option<&Path>, appdir: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates = vec![
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/segment-dict"),
        PathBuf::from("src-tauri/resources/segment-dict"),
    ];
    if let Some(directory) = executable.and_then(Path::parent) {
        candidates.push(directory.join("segment-dict"));
        candidates.push(directory.join("resources/segment-dict"));
        // Linux AppImage／DEB／RPM：二進位在 usr/bin，資源在 usr/lib/ConvertZZ。
        candidates.push(directory.join("../lib/ConvertZZ/segment-dict"));
    }
    if let Some(appdir) = appdir {
        candidates.push(appdir.join("usr/lib/ConvertZZ/segment-dict"));
        candidates.push(appdir.join("segment-dict"));
    }
    candidates
}

fn configure_segment_dict_root() {
    if std::env::var_os("NOVEL_SEGMENT_DICT_ROOT").is_some() {
        return;
    }
    let executable = std::env::current_exe().ok();
    let appdir = std::env::var_os("APPDIR").map(PathBuf::from);
    if let Some(path) = segment_dict_candidates(executable.as_deref(), appdir.as_deref())
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
                .map(|word| glyph_s2t(&word))
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
            glyph_s2t(&segmented)
        } else {
            base_convert(&segmented, direction)
        }
    }

    fn dictionary(&self, path: &Path) -> Result<Arc<LegacyDictionary>, CoreError> {
        let stamp = DictionaryStamp::from_metadata(&std::fs::metadata(path)?);
        let mut cache = self
            .dictionaries
            .lock()
            .map_err(|_| CoreError::new("DICTIONARY_LOCK", "無法鎖定字典快取。"))?;
        if let Some((cached_stamp, dictionary)) = cache.get(path) {
            if *cached_stamp == stamp {
                return Ok(Arc::clone(dictionary));
            }
        }
        let dictionary = Arc::new(LegacyDictionary::load(path)?);
        cache.insert(path.to_path_buf(), (stamp, Arc::clone(&dictionary)));
        Ok(dictionary)
    }
}

/// S2T glyphs: `cn2tw_min` (safe: false) first, then `cjk2zht`.
/// Min avoids 面→麵-style over-conversion and prefers 鐘 over 鍾; zht then fills
/// CJK／日文變體 and ambiguous forms such as 里→裡. T2S keeps full `tw2cn`.
/// Vocabulary fixes stay with `ZhtSynonymOptimizer`.
const GLYPH_S2T_OPTS: ConvertOptions<'static> = ConvertOptions {
    safe: false,
    ..ConvertOptions::DEFAULT
};

fn glyph_s2t(text: &str) -> String {
    cjk2zht(&cn2tw_min_with(text, &GLYPH_S2T_OPTS))
}

pub fn base_convert(text: &str, direction: Direction) -> String {
    match direction {
        Direction::S2t => glyph_s2t(text),
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
                .join("../resources/Dictionary.csv"),
        ))
        .unwrap()
    })
}

#[cfg(test)]
mod tests;

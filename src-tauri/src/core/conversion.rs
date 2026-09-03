use super::dictionary::LegacyDictionary;
use super::error::CoreError;
use super::parallelism::default_convert_jobs;
use super::roundtrip_dict::{is_package_data_path, parse_synonym_entry};
use super::types::{
    CancelCheck, ConversionRequest, ConversionResult, Direction, ProgressEvent, ProgressReporter,
};
use super::zhconvert::ZhConvertClient;
use cjk_convert_rs::{cn2tw_min_with, tw2cn_with};
use novel_segment::{DoSegmentOptions, Segment, SegmentOptions, POSTAG};
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

pub(crate) mod specials;

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

#[derive(Clone, Debug)]
struct ExtraSynonymHit {
    canonical: String,
    pos: u32,
}

type ExtraSynonymMap = HashMap<String, Vec<ExtraSynonymHit>>;

fn lookup_extra_synonym<'a>(
    map: &'a ExtraSynonymMap,
    word: &str,
    pos: u32,
    prev_pos: u32,
) -> Option<&'a str> {
    let hits = map.get(word)?;
    hits.iter()
        .rev()
        .find(|hit| extra_should_apply(word, pos, prev_pos, hit))
        .map(|hit| hit.canonical.as_str())
}

fn extra_should_apply(token: &str, token_pos: u32, prev_pos: u32, hit: &ExtraSynonymHit) -> bool {
    if !extra_pos_allows(token_pos, prev_pos, hit.pos) {
        return false;
    }
    if token == hit.canonical {
        return false;
    }
    if base_convert(token, Direction::T2s) != base_convert(&hit.canonical, Direction::T2s) {
        return true;
    }
    let engine = base_convert(&base_convert(token, Direction::T2s), Direction::S2t);
    let Some(token_dist) = glyph_distance(token, &engine) else {
        return true;
    };
    let Some(canonical_dist) = glyph_distance(&hit.canonical, &engine) else {
        return true;
    };
    const MEASURE: u32 = POSTAG::D_MQ | POSTAG::A_Q;
    if token_dist < canonical_dist {
        // 目前詞已較接近引擎（機制／控制／只有）。量詞「七只→七隻」除外。
        // extra 正字經字形引擎會變成目前詞時（拮据→拮據），這層要覆寫引擎。
        if base_convert(&hit.canonical, Direction::S2t) == token {
            return true;
        }
        return hit.pos & MEASURE != 0 && extra_pos_allows(token_pos, prev_pos, MEASURE);
    }
    true
}

fn glyph_distance(left: &str, right: &str) -> Option<usize> {
    let left_chars: Vec<char> = left.chars().collect();
    let right_chars: Vec<char> = right.chars().collect();
    if left_chars.len() != right_chars.len() {
        return None;
    }
    Some(
        left_chars
            .iter()
            .zip(&right_chars)
            .filter(|(a, b)| a != b)
            .count(),
    )
}

/// 無詞性＝與套件相同、不限詞性。有詞性時必須看分詞器依前後文標的詞性：
/// 本詞詞性要與條目有交集，未知詞性不轉；條目含量詞時，本詞或前詞也必須是量詞／數詞。
fn extra_pos_allows(token_pos: u32, prev_pos: u32, allowed: u32) -> bool {
    if allowed == 0 {
        return true;
    }
    if token_pos == 0 {
        return false;
    }
    if token_pos & allowed == 0 {
        return false;
    }
    const MEASURE: u32 = POSTAG::D_MQ | POSTAG::A_Q;
    const NUMERAL: u32 = POSTAG::A_M | MEASURE;
    if allowed & MEASURE != 0 && token_pos & MEASURE == 0 && prev_pos & NUMERAL == 0 {
        return false;
    }
    true
}

pub struct ConversionService {
    segmenter: Segment,
    extra_synonym: ExtraSynonymMap,
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

fn extra_correction_candidates(executable: Option<&Path>, appdir: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("CONVERTZZ_EXTRA_CORRECTION") {
        candidates.push(PathBuf::from(path));
    }
    candidates.extend([
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/extra-correction"),
        PathBuf::from("src-tauri/resources/extra-correction"),
    ]);
    if let Some(directory) = executable.and_then(Path::parent) {
        candidates.push(directory.join("extra-correction"));
        candidates.push(directory.join("resources/extra-correction"));
        candidates.push(directory.join("../lib/ConvertZZ/extra-correction"));
    }
    if let Some(appdir) = appdir {
        candidates.push(appdir.join("usr/lib/ConvertZZ/extra-correction"));
        candidates.push(appdir.join("extra-correction"));
    }
    candidates
        .into_iter()
        .filter(|path| !is_package_data_path(path))
        .collect()
}

enum ExtraLoad {
    Skip,
    Discover,
    Dir(PathBuf),
}

fn load_extra_correction(
    segmenter: &mut Segment,
    extra_synonym: &mut ExtraSynonymMap,
) -> Result<(), CoreError> {
    let executable = std::env::current_exe().ok();
    let appdir = std::env::var_os("APPDIR").map(PathBuf::from);
    let Some(root) = extra_correction_candidates(executable.as_deref(), appdir.as_deref())
        .into_iter()
        .find(|path| path.is_dir())
    else {
        return Ok(());
    };
    apply_extra_correction(segmenter, extra_synonym, &root, false)
}

fn apply_extra_correction(
    segmenter: &mut Segment,
    extra_synonym: &mut ExtraSynonymMap,
    root: &Path,
    required: bool,
) -> Result<(), CoreError> {
    if is_package_data_path(root) {
        return Err(CoreError::new(
            "EXTRA_CORRECTION",
            "額外修正目錄不可位於分詞或簡轉繁套件資料內。",
        ));
    }
    if !root.is_dir() {
        return Err(CoreError::new(
            "EXTRA_CORRECTION",
            format!("額外修正目錄不存在：{}", root.display()),
        ));
    }
    let dict = root.join("zht.corpus.dict.txt");
    let synonym = root.join("zht.corpus.synonym.txt");
    if required && !synonym.is_file() {
        return Err(CoreError::new(
            "EXTRA_CORRECTION",
            format!(
                "額外修正目錄缺少 zht.corpus.synonym.txt：{}",
                root.display()
            ),
        ));
    }
    if dict.is_file() {
        segmenter.load_dict_file(&dict).map_err(|error| {
            CoreError::new("EXTRA_CORRECTION", format!("無法載入額外分詞表：{error}"))
        })?;
    }
    if synonym.is_file() {
        let text = std::fs::read_to_string(&synonym).map_err(|error| {
            CoreError::new("EXTRA_CORRECTION", format!("無法讀取額外同義詞：{error}"))
        })?;
        for line in text.lines() {
            let Some(entry) = parse_synonym_entry(line) else {
                continue;
            };
            // Roundtrip pairs are traditional-facing (品嚐,品嘗). S2T input is often
            // simplified, so also register tw2cn forms.
            let mut expanded = entry.variants.clone();
            for form in std::iter::once(&entry.canonical).chain(entry.variants.iter()) {
                let simplified = base_convert(form, Direction::T2s);
                if simplified != *form
                    && simplified != entry.canonical
                    && !expanded.iter().any(|item| item == &simplified)
                {
                    expanded.push(simplified);
                }
            }
            let refs: Vec<&str> = expanded
                .iter()
                .map(|item| item.as_str())
                .filter(|item| *item != entry.canonical)
                .collect();
            if refs.is_empty() {
                continue;
            }
            // extra 不走套件 convert_synonym（那個只看字形、不看前後詞性）。
            // |詞性 只改上下文詞性符合的整詞；無詞性則不限詞性。
            for variant in refs {
                extra_synonym
                    .entry(variant.to_string())
                    .or_default()
                    .push(ExtraSynonymHit {
                        canonical: entry.canonical.clone(),
                        pos: entry.pos,
                    });
            }
        }
    }
    Ok(())
}

fn apply_specials_pins(segmenter: &mut Segment) -> Result<(), CoreError> {
    for word in specials::current().pinned_words() {
        let spec = format!("{word}|{:#x}|{}", specials::PIN_POS, specials::PIN_FREQ);
        segmenter.add_word(&spec, None, None).map_err(|error| {
            CoreError::new(
                "CONVERSION_SPECIALS",
                format!("無法釘入分詞詞「{word}」：{error}"),
            )
        })?;
    }
    Ok(())
}

impl ConversionService {
    pub fn new(default_dictionary: Option<PathBuf>) -> Result<Self, CoreError> {
        Self::build(default_dictionary, ExtraLoad::Discover)
    }

    pub fn without_extra_correction(
        default_dictionary: Option<PathBuf>,
    ) -> Result<Self, CoreError> {
        Self::build(default_dictionary, ExtraLoad::Skip)
    }

    pub fn with_extra_correction(
        default_dictionary: Option<PathBuf>,
        extra_root: &Path,
    ) -> Result<Self, CoreError> {
        Self::build(default_dictionary, ExtraLoad::Dir(extra_root.to_path_buf()))
    }

    fn build(default_dictionary: Option<PathBuf>, extra: ExtraLoad) -> Result<Self, CoreError> {
        configure_segment_dict_root();
        specials::init()?;
        let mut segmenter = Segment::new(SegmentOptions {
            auto_cjk: true,
            all_mod: true,
            ..SegmentOptions::default()
        });
        segmenter
            .use_default()
            .map_err(|error| CoreError::new("SEGMENTER", format!("無法初始化分詞引擎：{error}")))?;
        let mut extra_synonym = ExtraSynonymMap::new();
        match extra {
            ExtraLoad::Skip => {}
            ExtraLoad::Discover => load_extra_correction(&mut segmenter, &mut extra_synonym)?,
            ExtraLoad::Dir(root) => {
                apply_extra_correction(&mut segmenter, &mut extra_synonym, &root, true)?
            }
        }
        apply_specials_pins(&mut segmenter)?;
        Ok(Self {
            segmenter,
            extra_synonym,
            dictionaries: Mutex::new(HashMap::new()),
            default_dictionary,
            zhconvert: ZhConvertClient::new(),
        })
    }

    pub fn convert_segmented(&self, text: &str, direction: Direction) -> String {
        if text.is_empty() || direction == Direction::None {
            return text.to_string();
        }
        self.segmented_convert_with_progress(text, direction, &None, &None)
            .unwrap_or_else(|_| text.to_string())
    }

    #[cfg(test)]
    pub(crate) fn debug_pos(&self, text: &str) -> Vec<(String, u32)> {
        self.segmenter
            .do_segment(text, segment_plain_options(true))
            .into_iter()
            .map(|word| (word.w.clone(), word.pos()))
            .collect()
    }

    pub fn segment_tokens(&self, text: &str) -> Vec<String> {
        if text.is_empty() {
            return Vec::new();
        }
        self.segmenter
            .do_segment_simple(text, segment_plain_options(false))
    }

    /// 回環對齊用：關閉同義詞與 ZhtSynonymOptimizer，保留原文「里／据」不被改寫。
    pub fn segment_tokens_align(&self, text: &str) -> Vec<String> {
        if text.is_empty() {
            return Vec::new();
        }
        self.segmenter
            .do_segment_simple(text, segment_align_options())
    }

    /// Isolated-word POS for extra-correction dict rows. Unknown or split words use `D_N`.
    pub fn word_pos(&self, word: &str) -> u32 {
        if word.is_empty() {
            return POSTAG::D_N;
        }
        let words = self
            .segmenter
            .do_segment(word, segment_plain_options(false));
        if words.len() == 1 && words[0].w == word && words[0].pos() != 0 {
            words[0].pos()
        } else {
            POSTAG::D_N
        }
    }

    pub async fn convert(&self, request: ConversionRequest) -> Result<ConversionResult, CoreError> {
        self.convert_with_progress(request, None, None).await
    }

    pub async fn convert_with_progress(
        &self,
        request: ConversionRequest,
        progress: Option<ProgressReporter>,
        is_cancelled: Option<CancelCheck>,
    ) -> Result<ConversionResult, CoreError> {
        let started = Instant::now();
        let mut warnings = Vec::new();
        let mut text = request.text;
        if request.direction != Direction::None && !text.is_empty() {
            throw_if_cancelled(&is_cancelled)?;
            if request.vocabulary_correction == Some(false) {
                text = run_cpu_bound(|| {
                    convert_glyphs_chunked(&text, request.direction, &progress, &is_cancelled)
                })?;
                warnings.push("詞彙修正已停用。本次只執行 cjk-convert-rs 字形轉換。".into());
            } else if request.engine == super::types::EngineKind::Segmented {
                // Segmenting is CPU-bound; keep multi-thread runtimes responsive.
                text = run_cpu_bound(|| {
                    self.segmented_convert_with_progress(
                        &text,
                        request.direction,
                        &progress,
                        &is_cancelled,
                    )
                })?;
            } else if request.engine == super::types::EngineKind::Legacy {
                let path = request
                    .dictionary_path
                    .as_deref()
                    .map(PathBuf::from)
                    .or_else(|| self.default_dictionary.clone())
                    .ok_or_else(|| CoreError::new("DICTIONARY_MISSING", "找不到舊版字典。"))?;
                let dictionary = self.dictionary(&path)?;
                text = run_cpu_bound(|| {
                    convert_legacy_chunked(
                        &dictionary,
                        &text,
                        request.direction,
                        &progress,
                        &is_cancelled,
                    )
                })?;
                warnings.push(
                    "未命中字元使用跨平台 cjk-convert-rs，結果可能與舊版 Windows 映射略有差異。"
                        .into(),
                );
            } else {
                text = self
                    .zhconvert
                    .convert_with_progress(
                        &text,
                        request.direction,
                        request.zhconvert.as_ref(),
                        progress.clone(),
                        is_cancelled.clone(),
                    )
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

    fn segmented_convert_with_progress(
        &self,
        text: &str,
        direction: Direction,
        progress: &Option<ProgressReporter>,
        is_cancelled: &Option<CancelCheck>,
    ) -> Result<String, CoreError> {
        // Only run the expensive segmenter on CJK runs. HTML/JS/CSS/base64 stays on glyph path.
        let units = collect_segment_units(text);
        let total = text.chars().count().max(1) as u64;
        let jobs = default_convert_jobs();
        if jobs <= 1 || units.len() < 2 {
            let mut output = String::with_capacity(text.len());
            let mut done = 0u64;
            for unit in units {
                throw_if_cancelled(is_cancelled)?;
                let piece = match unit.kind {
                    UnitKind::Plain => base_convert(&text[unit.start..unit.end], direction),
                    UnitKind::Cjk => {
                        self.segmented_convert_chunk(&text[unit.start..unit.end], direction)
                    }
                };
                output.push_str(&piece);
                done += unit.chars;
                report_convert_progress(progress, done, total);
            }
            return Ok(output);
        }
        convert_units_parallel(
            jobs,
            &units,
            total,
            progress,
            is_cancelled,
            |unit| match unit.kind {
                UnitKind::Plain => Ok(base_convert(&text[unit.start..unit.end], direction)),
                UnitKind::Cjk => {
                    Ok(self.segmented_convert_chunk(&text[unit.start..unit.end], direction))
                }
            },
        )
    }

    fn segmented_convert_chunk(&self, chunk: &str, direction: Direction) -> String {
        // 只分詞一次。extra 同義詞依整詞詞性拉回（胜肽、膿疱、錶現），不再跑第二次分詞。
        let words = self
            .segmenter
            .do_segment(chunk, segment_plain_options(direction == Direction::S2t));
        words
            .iter()
            .enumerate()
            .map(|(index, word)| {
                let pos = word.pos();
                let prev_pos = index
                    .checked_sub(1)
                    .map(|prev| words[prev].pos())
                    .unwrap_or(0);
                let from_extra = lookup_extra_synonym(&self.extra_synonym, &word.w, pos, prev_pos)
                    .unwrap_or(&word.w)
                    .to_string();
                let glyph = base_convert(&from_extra, direction);
                let pulled = if direction == Direction::S2t {
                    lookup_extra_synonym(&self.extra_synonym, &glyph, pos, prev_pos)
                        .map(str::to_string)
                        .unwrap_or(glyph)
                } else {
                    glyph
                };
                let next = words.get(index + 1).map(|item| item.w.as_str());
                specials::current().apply_token(&pulled, next, direction, pos, prev_pos)
            })
            .collect()
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

fn segment_plain_options(convert_synonym: bool) -> DoSegmentOptions {
    DoSegmentOptions {
        simple: Some(true),
        strip_punctuation: Some(false),
        strip_stopword: Some(false),
        strip_space: Some(false),
        convert_synonym: Some(convert_synonym),
        disable_modules: Vec::new(),
    }
}

fn segment_align_options() -> DoSegmentOptions {
    DoSegmentOptions {
        simple: Some(true),
        strip_punctuation: Some(false),
        strip_stopword: Some(false),
        strip_space: Some(false),
        convert_synonym: Some(false),
        disable_modules: vec!["ZhtSynonymOptimizer".into()],
    }
}

/// S2T glyphs: only `cn2tw_min` (safe: false). No `cjk2zht`——那張 JP／簡體表會把
/// 台灣也在用的「制／娘／里」整字改掉，分詞也擋不住。一簡多繁與日文整詞交給
/// `ZhtSynonymOptimizer`／extra-correction。璇／疱／么／胜肽／里長 等見
/// `resources/conversion-specials/rules.txt`（分詞時釘整詞，轉換時再套用）。
fn glyph_s2t(text: &str) -> String {
    cn2tw_min_with(text, &specials::current().s2t_options())
}

pub fn base_convert(text: &str, direction: Direction) -> String {
    match direction {
        Direction::S2t => glyph_s2t(text),
        Direction::T2s => tw2cn_with(text, &specials::current().t2s_options()),
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

fn throw_if_cancelled(is_cancelled: &Option<CancelCheck>) -> Result<(), CoreError> {
    if is_cancelled.as_ref().is_some_and(|check| check()) {
        return Err(CoreError::new("CONVERT_CANCELLED", "轉換已由使用者取消。"));
    }
    Ok(())
}

fn report_convert_progress(progress: &Option<ProgressReporter>, current: u64, total: u64) {
    if let Some(progress) = progress {
        progress(ProgressEvent {
            current: current.min(total),
            total: total.max(1),
            message: format!("正在轉換文字… {}/{}", current.min(total), total.max(1)),
        });
    }
}

fn convert_glyphs_chunked(
    text: &str,
    direction: Direction,
    progress: &Option<ProgressReporter>,
    is_cancelled: &Option<CancelCheck>,
) -> Result<String, CoreError> {
    let units = collect_plain_units(text);
    let total = text.chars().count().max(1) as u64;
    let jobs = default_convert_jobs();
    if jobs <= 1 || units.len() < 2 {
        let mut output = String::with_capacity(text.len());
        let mut done = 0u64;
        for unit in units {
            throw_if_cancelled(is_cancelled)?;
            output.push_str(&base_convert(&text[unit.start..unit.end], direction));
            done += unit.chars;
            report_convert_progress(progress, done, total);
        }
        return Ok(output);
    }
    convert_units_parallel(jobs, &units, total, progress, is_cancelled, |unit| {
        Ok(base_convert(&text[unit.start..unit.end], direction))
    })
}

fn convert_legacy_chunked(
    dictionary: &LegacyDictionary,
    text: &str,
    direction: Direction,
    progress: &Option<ProgressReporter>,
    is_cancelled: &Option<CancelCheck>,
) -> Result<String, CoreError> {
    let total = text.chars().count().max(1) as u64;
    if total <= MAX_CHUNK_CHARACTERS as u64 {
        throw_if_cancelled(is_cancelled)?;
        let converted = dictionary.replace(text, direction, |value| base_convert(value, direction));
        report_convert_progress(progress, total, total);
        return Ok(converted);
    }
    let units = collect_line_units(text);
    let jobs = default_convert_jobs();
    if jobs <= 1 || units.len() < 2 {
        let mut output = String::with_capacity(text.len());
        let mut done = 0u64;
        for unit in units {
            throw_if_cancelled(is_cancelled)?;
            let piece = &text[unit.start..unit.end];
            output.push_str(
                &dictionary.replace(piece, direction, |value| base_convert(value, direction)),
            );
            done += unit.chars;
            report_convert_progress(progress, done, total);
        }
        return Ok(output);
    }
    convert_units_parallel(jobs, &units, total, progress, is_cancelled, |unit| {
        let piece = &text[unit.start..unit.end];
        Ok(dictionary.replace(piece, direction, |value| base_convert(value, direction)))
    })
}

#[derive(Clone, Copy, Debug)]
enum UnitKind {
    Plain,
    Cjk,
}

#[derive(Clone, Copy, Debug)]
struct TextUnit {
    start: usize,
    end: usize,
    chars: u64,
    kind: UnitKind,
}

fn collect_segment_units(text: &str) -> Vec<TextUnit> {
    let mut units = Vec::new();
    for run in split_cjk_runs(text) {
        match run {
            TextRun::Plain(plain) => {
                let start = offset_of(text, plain);
                units.push(TextUnit {
                    start,
                    end: start + plain.len(),
                    chars: plain.chars().count() as u64,
                    kind: UnitKind::Plain,
                });
            }
            TextRun::Cjk(cjk) => {
                let run_start = offset_of(text, cjk);
                units.push(TextUnit {
                    start: run_start,
                    end: run_start + cjk.len(),
                    chars: cjk.chars().count() as u64,
                    kind: UnitKind::Cjk,
                });
            }
        }
    }
    units
}

fn collect_plain_units(text: &str) -> Vec<TextUnit> {
    let mut units = Vec::new();
    let mut local = 0usize;
    for chunk in split_text(text) {
        let start = local;
        let end = start + chunk.len();
        units.push(TextUnit {
            start,
            end,
            chars: chunk.chars().count() as u64,
            kind: UnitKind::Plain,
        });
        local = end;
    }
    units
}

fn collect_line_units(text: &str) -> Vec<TextUnit> {
    let mut units = Vec::new();
    let mut start = 0usize;
    for (idx, _) in text.match_indices('\n') {
        let end = idx + 1;
        let piece = &text[start..end];
        units.push(TextUnit {
            start,
            end,
            chars: piece.chars().count() as u64,
            kind: UnitKind::Plain,
        });
        start = end;
    }
    if start < text.len() {
        units.push(TextUnit {
            start,
            end: text.len(),
            chars: text[start..].chars().count() as u64,
            kind: UnitKind::Plain,
        });
    }
    units
}

fn offset_of(text: &str, slice: &str) -> usize {
    let text_addr = text.as_ptr() as usize;
    let slice_addr = slice.as_ptr() as usize;
    slice_addr.saturating_sub(text_addr)
}

fn convert_units_serial(
    units: &[TextUnit],
    total: u64,
    progress: &Option<ProgressReporter>,
    is_cancelled: &Option<CancelCheck>,
    convert_unit: impl Fn(&TextUnit) -> Result<String, CoreError>,
) -> Result<String, CoreError> {
    let mut output = String::with_capacity(units.last().map(|unit| unit.end).unwrap_or(0));
    let mut done = 0u64;
    for unit in units {
        throw_if_cancelled(is_cancelled)?;
        output.push_str(&convert_unit(unit)?);
        done += unit.chars;
        report_convert_progress(progress, done, total);
    }
    Ok(output)
}

fn convert_units_parallel(
    jobs: usize,
    units: &[TextUnit],
    total: u64,
    progress: &Option<ProgressReporter>,
    is_cancelled: &Option<CancelCheck>,
    convert_unit: impl Fn(&TextUnit) -> Result<String, CoreError> + Sync,
) -> Result<String, CoreError> {
    throw_if_cancelled(is_cancelled)?;
    // Nested ThreadPool::install from another pool's worker deadlocks (roundtrip-dict).
    if jobs <= 1 || units.len() < 2 || rayon::current_thread_index().is_some() {
        return convert_units_serial(units, total, progress, is_cancelled, convert_unit);
    }
    let batches = group_units(units, jobs);
    if batches.len() < 2 {
        return convert_units_serial(units, total, progress, is_cancelled, convert_unit);
    }
    let done = AtomicU64::new(0);
    let progress = progress.clone();
    let is_cancelled = is_cancelled.clone();
    let parts: Result<Vec<String>, CoreError> = batches
        .par_iter()
        .map(|batch| {
            throw_if_cancelled(&is_cancelled)?;
            let mut local = String::new();
            let mut local_chars = 0u64;
            for unit in batch.iter() {
                throw_if_cancelled(&is_cancelled)?;
                local.push_str(&convert_unit(unit)?);
                local_chars += unit.chars;
            }
            let current = done.fetch_add(local_chars, Ordering::Relaxed) + local_chars;
            report_convert_progress(&progress, current, total);
            Ok(local)
        })
        .collect();
    let parts = parts?;
    let mut output = String::with_capacity(units.last().map(|unit| unit.end).unwrap_or(0));
    for part in parts {
        output.push_str(&part);
    }
    report_convert_progress(&progress, total, total);
    Ok(output)
}

fn group_units(units: &[TextUnit], jobs: usize) -> Vec<Vec<TextUnit>> {
    if units.is_empty() {
        return Vec::new();
    }
    let total_chars = units.iter().map(|unit| unit.chars).sum::<u64>().max(1);
    let target_chars = (total_chars / jobs.max(1) as u64)
        .max(2_048)
        .min(MAX_CHUNK_CHARACTERS as u64);
    let mut batches = Vec::new();
    let mut current = Vec::new();
    let mut current_chars = 0u64;
    for unit in units {
        if !current.is_empty() && current_chars + unit.chars > target_chars {
            batches.push(std::mem::take(&mut current));
            current_chars = 0;
        }
        current_chars += unit.chars;
        current.push(*unit);
    }
    if !current.is_empty() {
        batches.push(current);
    }
    batches
}

#[derive(Debug, PartialEq, Eq)]
enum TextRun<'a> {
    Plain(&'a str),
    Cjk(&'a str),
}

pub(crate) fn is_cjk_char(character: char) -> bool {
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

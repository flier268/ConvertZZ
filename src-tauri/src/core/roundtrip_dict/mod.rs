mod aggregator;
mod checkpoint;
mod memory;
mod merge;
mod run;
mod synonym_audit;

pub use aggregator::{
    finish_from_shards, merge_sorted_shard_files, FinishStats, PairAggregator, PairStat,
    TEST_MAX_IN_MEMORY_KEYS, TEST_PEAK_IN_MEMORY_KEYS,
};
pub use checkpoint::{atomic_write, build_fingerprint, Checkpoint, Fingerprint};
pub use memory::{
    default_sampler, FakeSampler, MemoryPolicy, MemorySample, MemorySampler, ResolvedMemory,
};
pub use merge::{merge_extra_correction, MergeStats};
pub use run::{run_roundtrip, RoundtripRunConfig, RoundtripRunStatus, RunStatus};
pub use synonym_audit::{
    audit_synonym_orientation, format_orientation_report, is_left_simplified_right_traditional,
    resolve_synonym_path, write_orientation_audit_reports, LeftSimpRightTradHit,
    SimpTradConfidence, SynonymOrientationReport, ORIENTATION_FULL_REPORT, ORIENTATION_MIN_REPORT,
};

use super::conversion::{base_convert, is_cjk_char, ConversionService};
use super::types::Direction;
use novel_segment::POSTAG;
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

pub(crate) const MIN_WORD_CHARS: usize = 2;
pub(crate) const MAX_WORD_CHARS: usize = 8;
pub(crate) const MAX_LINE_CHARS: usize = 20_000;
pub(crate) const MAX_TOKEN_COUNT: usize = 800;
pub(crate) const MAX_EXAMPLES: usize = 3;
/// 固定寫入 extra 分詞表。來源是 conversion-specials：`pin` 與 `word=`／`word^=` 的整詞。
pub(crate) fn pinned_dict_words() -> Vec<(String, u32, u64)> {
    super::conversion::specials::current()
        .pinned_words()
        .iter()
        .map(|word| {
            (
                word.clone(),
                super::conversion::specials::PIN_POS,
                super::conversion::specials::PIN_FREQ,
            )
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct LineResult {
    pub skipped: bool,
    pub mismatched: bool,
    pub pairs: Vec<(String, String)>,
    /// 原文分詞（2–8 字 CJK）。用來判斷異體是否本身也是語料正詞。
    pub originals: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CorrectionEntry {
    pub canonical: String,
    pub variants: Vec<(String, u64)>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundtripReport {
    pub lines_read: u64,
    pub lines_skipped: u64,
    pub lines_mismatched: u64,
    pub raw_pair_occurrences: u64,
    pub unique_raw_pairs: usize,
    pub kept_entries: usize,
    pub kept_variants: usize,
    pub skipped_existing: usize,
    pub skipped_low_count: usize,
    pub skipped_ambiguous: usize,
    pub files: Vec<String>,
    pub top_pairs: Vec<ReportPair>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportPair {
    pub canonical: String,
    pub variant: String,
    pub count: u64,
    pub examples: Vec<String>,
}

pub fn process_line(service: &ConversionService, line: &str) -> LineResult {
    process_line_with_buf(service, line, None)
}

pub(crate) fn process_line_with_buf(
    service: &ConversionService,
    line: &str,
    lcs_buf: Option<&mut Vec<u32>>,
) -> LineResult {
    let line = line.trim();
    if !should_process(line) {
        return skipped_line();
    }
    let units = split_process_units(line);
    if units.len() == 1 {
        return process_unit_with_buf(service, units[0], lcs_buf);
    }
    let mut owned_buf;
    let buf: &mut Vec<u32> = match lcs_buf {
        Some(buf) => buf,
        None => {
            owned_buf = Vec::new();
            &mut owned_buf
        }
    };
    let mut pairs = Vec::new();
    let mut originals = Vec::new();
    let mut mismatched = false;
    let mut any = false;
    for unit in units {
        if !should_process(unit) {
            continue;
        }
        any = true;
        let result = process_unit_with_buf(service, unit, Some(buf));
        originals.extend(result.originals);
        if result.mismatched {
            mismatched = true;
            pairs.extend(result.pairs);
        }
    }
    if !any {
        return skipped_line();
    }
    LineResult {
        skipped: false,
        mismatched,
        pairs,
        originals,
    }
}

fn skipped_line() -> LineResult {
    LineResult {
        skipped: true,
        mismatched: false,
        pairs: Vec::new(),
        originals: Vec::new(),
    }
}

fn original_words(tokens: &[String]) -> Vec<String> {
    tokens
        .iter()
        .filter(|token| {
            let len = token.chars().count();
            len >= MIN_WORD_CHARS && len <= MAX_WORD_CHARS && is_cjk_word(token)
        })
        .cloned()
        .collect()
}

fn process_unit_with_buf(
    service: &ConversionService,
    unit: &str,
    lcs_buf: Option<&mut Vec<u32>>,
) -> LineResult {
    let original_tokens = service.segment_tokens_align(unit);
    let originals = original_words(&original_tokens);
    let simplified = service.convert_segmented(unit, Direction::T2s);
    let reconstructed = service.convert_segmented(&simplified, Direction::S2t);
    if reconstructed == unit {
        return LineResult {
            skipped: false,
            mismatched: false,
            pairs: Vec::new(),
            originals,
        };
    }
    let reconstructed_tokens = service.segment_tokens_align(&reconstructed);
    if original_tokens.len() > MAX_TOKEN_COUNT || reconstructed_tokens.len() > MAX_TOKEN_COUNT {
        return LineResult {
            skipped: false,
            mismatched: true,
            pairs: Vec::new(),
            originals,
        };
    }
    LineResult {
        skipped: false,
        mismatched: true,
        pairs: extract_pairs_with_buf(&original_tokens, &reconstructed_tokens, lcs_buf),
        originals,
    }
}

pub(crate) fn split_process_units(line: &str) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut start = 0;
    for (idx, ch) in line.char_indices() {
        if is_unit_break(ch) {
            let end = idx + ch.len_utf8();
            if start < end {
                pieces.push(&line[start..end]);
            }
            start = end;
        }
    }
    if start < line.len() {
        pieces.push(&line[start..]);
    }
    if pieces.is_empty() {
        vec![line]
    } else {
        pieces
    }
}

fn is_unit_break(ch: char) -> bool {
    matches!(
        ch,
        '。' | '！'
            | '？'
            | '；'
            | '：'
            | '，'
            | '、'
            | '.'
            | '!'
            | '?'
            | ';'
            | ':'
            | ','
            | '\n'
            | '…'
    )
}

pub fn should_process(line: &str) -> bool {
    if line.is_empty() || line.chars().count() > MAX_LINE_CHARS {
        return false;
    }
    cjk_char_count(line) >= MIN_WORD_CHARS
}

pub fn extract_pairs(original: &[String], reconstructed: &[String]) -> Vec<(String, String)> {
    extract_pairs_with_buf(original, reconstructed, None)
}

fn extract_pairs_with_buf(
    original: &[String],
    reconstructed: &[String],
    lcs_buf: Option<&mut Vec<u32>>,
) -> Vec<(String, String)> {
    if original == reconstructed {
        return Vec::new();
    }
    let mut pairs = Vec::new();
    let ops = alignment_ops(original, reconstructed, lcs_buf);
    for (index, op) in ops.iter().enumerate() {
        let AlignOp::Replace {
            original: original_hunk,
            reconstructed: reconstructed_hunk,
        } = op
        else {
            continue;
        };
        pairs.extend(pairs_from_hunk(original_hunk, reconstructed_hunk));
        pairs.extend(pairs_from_glued_single_char_neighbors(&ops, index));
    }
    pairs
}

fn pairs_from_hunk(original: &[String], reconstructed: &[String]) -> Vec<(String, String)> {
    if original.is_empty() || reconstructed.is_empty() {
        return Vec::new();
    }
    let original_joined = original.join("");
    let reconstructed_joined = reconstructed.join("");
    let original_chars = original_joined.chars().count();
    let reconstructed_chars = reconstructed_joined.chars().count();
    let mut pairs = Vec::new();
    if original_chars == reconstructed_chars {
        let reconstructed_chars: Vec<char> = reconstructed_joined.chars().collect();
        // 只接受落在「重建側連續分詞」邊界上的字元切片，避免「四|隻有」對到「四只|有」
        // 中間的「只有」這種跨詞假詞對。
        let aligned_spans = consecutive_token_char_spans(reconstructed);
        let mut index = 0;
        for token in original {
            let len = token.chars().count();
            let start = index;
            let end = index + len;
            index = end;
            if !aligned_spans.contains(&(start, end)) {
                continue;
            }
            let span: String = reconstructed_chars[start..end].iter().collect();
            if let Some(pair) = usable_pair(&span, token) {
                pairs.push(pair);
            }
        }
        return pairs;
    }
    let original_cjk = join_cjk_words(original);
    let reconstructed_cjk = join_cjk_words(reconstructed);
    if let Some(pair) = usable_pair(&reconstructed_cjk, &original_cjk) {
        pairs.push(pair);
    }
    pairs
}

/// 重建側每個連續 token 子序列的字元區間 `[start, end)`。
fn consecutive_token_char_spans(tokens: &[String]) -> HashSet<(usize, usize)> {
    let mut offsets = Vec::with_capacity(tokens.len() + 1);
    offsets.push(0usize);
    let mut pos = 0usize;
    for token in tokens {
        pos += token.chars().count();
        offsets.push(pos);
    }
    let mut spans = HashSet::new();
    for left in 0..tokens.len() {
        for right in (left + 1)..=tokens.len() {
            spans.insert((offsets[left], offsets[right]));
        }
    }
    spans
}

pub(crate) fn usable_pair(variant: &str, canonical: &str) -> Option<(String, String)> {
    if variant == canonical {
        return None;
    }
    if !is_cjk_word(variant) || !is_cjk_word(canonical) {
        return None;
    }
    let variant_len = variant.chars().count();
    let canonical_len = canonical.chars().count();
    if variant_len < MIN_WORD_CHARS
        || canonical_len < MIN_WORD_CHARS
        || variant_len > MAX_WORD_CHARS
        || canonical_len > MAX_WORD_CHARS
    {
        return None;
    }
    if variant_len.abs_diff(canonical_len) > 1 {
        return None;
    }
    if !same_conversion_family(variant, canonical) {
        return None;
    }
    Some((variant.to_string(), canonical.to_string()))
}

fn same_conversion_family(left: &str, right: &str) -> bool {
    if base_convert(left, Direction::T2s) == base_convert(right, Direction::T2s) {
        return true;
    }
    let left_chars: Vec<char> = left.chars().collect();
    let right_chars: Vec<char> = right.chars().collect();
    if left_chars.len() != right_chars.len() || left_chars.is_empty() {
        return false;
    }
    let equal = left_chars
        .iter()
        .zip(&right_chars)
        .filter(|(a, b)| a == b)
        .count();
    equal * 2 >= left_chars.len()
}

enum AlignOp {
    Equal(String),
    Replace {
        original: Vec<String>,
        reconstructed: Vec<String>,
    },
}

fn alignment_ops(
    original: &[String],
    reconstructed: &[String],
    lcs_buf: Option<&mut Vec<u32>>,
) -> Vec<AlignOp> {
    let n = original.len();
    let m = reconstructed.len();
    let cols = m + 1;
    let needed = (n + 1) * cols;
    let mut owned;
    let buf: &mut Vec<u32> = match lcs_buf {
        Some(buf) => buf,
        None => {
            owned = Vec::new();
            &mut owned
        }
    };
    buf.clear();
    buf.resize(needed, 0);
    for i in 0..n {
        for j in 0..m {
            buf[(i + 1) * cols + j + 1] = if original[i] == reconstructed[j] {
                buf[i * cols + j] + 1
            } else {
                buf[(i + 1) * cols + j].max(buf[i * cols + j + 1])
            };
        }
    }

    enum Op {
        Equal(String),
        Delete(String),
        Insert(String),
    }
    let mut ops = Vec::new();
    let mut i = n;
    let mut j = m;
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && original[i - 1] == reconstructed[j - 1] {
            ops.push(Op::Equal(original[i - 1].clone()));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || buf[i * cols + (j - 1)] >= buf[(i - 1) * cols + j]) {
            ops.push(Op::Insert(reconstructed[j - 1].clone()));
            j -= 1;
        } else {
            ops.push(Op::Delete(original[i - 1].clone()));
            i -= 1;
        }
    }
    ops.reverse();

    let mut aligned = Vec::new();
    let mut deleted = Vec::new();
    let mut inserted = Vec::new();
    let flush =
        |deleted: &mut Vec<String>, inserted: &mut Vec<String>, aligned: &mut Vec<AlignOp>| {
            if !deleted.is_empty() && !inserted.is_empty() {
                aligned.push(AlignOp::Replace {
                    original: std::mem::take(deleted),
                    reconstructed: std::mem::take(inserted),
                });
            } else {
                deleted.clear();
                inserted.clear();
            }
        };
    for op in ops {
        match op {
            Op::Equal(token) => {
                flush(&mut deleted, &mut inserted, &mut aligned);
                aligned.push(AlignOp::Equal(token));
            }
            Op::Delete(token) => deleted.push(token),
            Op::Insert(token) => inserted.push(token),
        }
    }
    flush(&mut deleted, &mut inserted, &mut aligned);
    aligned
}

fn single_cjk_char(token: &str) -> Option<&str> {
    let mut chars = token.chars();
    let first = chars.next()?;
    if chars.next().is_some() || !is_cjk_char(first) {
        return None;
    }
    Some(token)
}

fn replace_single_cjk_char(tokens: &[String]) -> Option<&str> {
    if tokens.len() != 1 {
        return None;
    }
    single_cjk_char(&tokens[0])
}

/// 單字差異（里→裡）本身不收；若前後剛好也是單字漢字，粘成 2 字詞（本里、里辦）。
/// 不跟 2 字以上鄰居粘，避免「房子里」「里垃圾車」。
fn pairs_from_glued_single_char_neighbors(
    ops: &[AlignOp],
    replace_index: usize,
) -> Vec<(String, String)> {
    let AlignOp::Replace {
        original,
        reconstructed,
    } = &ops[replace_index]
    else {
        return Vec::new();
    };
    let Some(orig_ch) = replace_single_cjk_char(original) else {
        return Vec::new();
    };
    let Some(recon_ch) = replace_single_cjk_char(reconstructed) else {
        return Vec::new();
    };
    let mut pairs = Vec::new();
    if let Some(left) = replace_index
        .checked_sub(1)
        .and_then(|index| match &ops[index] {
            AlignOp::Equal(token) => single_cjk_char(token),
            AlignOp::Replace { .. } => None,
        })
    {
        if let Some(pair) = usable_pair(&format!("{left}{recon_ch}"), &format!("{left}{orig_ch}")) {
            pairs.push(pair);
        }
    }
    if let Some(right) = ops.get(replace_index + 1).and_then(|op| match op {
        AlignOp::Equal(token) => single_cjk_char(token),
        AlignOp::Replace { .. } => None,
    }) {
        if let Some(pair) = usable_pair(&format!("{recon_ch}{right}"), &format!("{orig_ch}{right}"))
        {
            pairs.push(pair);
        }
    }
    pairs
}

pub fn is_cjk_word(text: &str) -> bool {
    !text.is_empty() && text.chars().all(is_cjk_char)
}

fn cjk_char_count(text: &str) -> usize {
    text.chars()
        .filter(|character| is_cjk_char(*character))
        .count()
}

fn join_cjk_words(tokens: &[String]) -> String {
    tokens
        .iter()
        .filter(|token| is_cjk_word(token))
        .cloned()
        .collect()
}

pub(crate) fn truncate_example(line: &str) -> String {
    const LIMIT: usize = 80;
    let mut count = 0;
    let mut end = 0;
    for (index, character) in line.char_indices() {
        count += 1;
        end = index + character.len_utf8();
        if count == LIMIT {
            break;
        }
    }
    if count < LIMIT || end >= line.len() {
        line.to_string()
    } else {
        format!("{}…", &line[..end])
    }
}

#[derive(Clone, Debug, Default)]
pub struct CorpusSelect {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

pub fn corpus_files(sources_root: &Path, select: &CorpusSelect) -> Result<Vec<PathBuf>, String> {
    if !sources_root.is_dir() {
        return Err(format!("來源必須是目錄：{}", sources_root.display()));
    }
    let mut files = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(sources_root)
        .map_err(|error| format!("無法讀取 {}：{error}", sources_root.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("無法讀取 {}：{error}", sources_root.display()))?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|error| format!("無法判斷 {}：{error}", entry.path().display()))?;
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if file_type.is_dir() {
            if !allowed_top_level(&name, select) {
                continue;
            }
            collect_txt_files(&path, &mut files)?;
        } else if file_type.is_file()
            && is_txt_file(&path)
            && select.include.is_empty()
            && !select.exclude.iter().any(|item| item == &name)
        {
            files.push(path);
        }
    }
    files.sort();
    if files.is_empty() {
        return Err(format!(
            "在 {} 找不到符合選取條件的 .txt 語料檔。",
            sources_root.display()
        ));
    }
    Ok(files)
}

fn allowed_top_level(name: &str, select: &CorpusSelect) -> bool {
    if select.exclude.iter().any(|item| item == name) {
        return false;
    }
    select.include.is_empty() || select.include.iter().any(|item| item == name)
}

fn collect_txt_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|error| format!("無法讀取 {}：{error}", dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("無法讀取 {}：{error}", dir.display()))?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|error| format!("無法判斷 {}：{error}", entry.path().display()))?;
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            collect_txt_files(&path, files)?;
        } else if file_type.is_file() && is_txt_file(&path) {
            files.push(path);
        }
    }
    Ok(())
}

fn is_txt_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("txt"))
}

pub fn load_existing_synonym_variants(segment_dict_root: &Path) -> HashSet<String> {
    let mut variants = HashSet::new();
    let synonym_dir = segment_dict_root.join("synonym");
    for name in ["synonym.txt", "zht.synonym.txt", "zht.common.synonym.txt"] {
        let path = synonym_dir.join(name);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        extend_synonym_variants(&mut variants, &text);
    }
    variants
}

pub fn load_extra_correction_variants(root: &Path) -> Result<HashSet<String>, String> {
    let path = root.join("zht.corpus.synonym.txt");
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("無法讀取額外同義詞 {}：{error}", path.display()))?;
    let mut variants = HashSet::new();
    extend_synonym_variants(&mut variants, &text);
    Ok(variants)
}

fn extend_synonym_variants(variants: &mut HashSet<String>, text: &str) {
    for line in text.lines() {
        if let Some((_, vars)) = parse_synonym_line(line) {
            variants.extend(vars);
        }
    }
}

pub fn parse_synonym_line(line: &str) -> Option<(String, Vec<String>)> {
    parse_synonym_entry(line).map(|entry| (entry.canonical, entry.variants))
}

/// Extra-correction synonym: `正字,錯字,...`（與套件相同）。舊檔可選 `|0xPOS` 或 `|D_F+D_S`。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SynonymEntry {
    pub canonical: String,
    pub variants: Vec<String>,
    pub pos: u32,
}

pub fn parse_synonym_entry(line: &str) -> Option<SynonymEntry> {
    let line = line.trim();
    if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
        return None;
    }
    let (body, pos) = match line.rsplit_once('|') {
        Some((body, raw)) => match parse_pos_mask(raw.trim()) {
            Some(mask) => (body.trim(), mask),
            None => (line, 0),
        },
        None => (line, 0),
    };
    let mut parts: Vec<String> = body
        .split(',')
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() < 2 {
        return None;
    }
    let canonical = parts.remove(0);
    Some(SynonymEntry {
        canonical,
        variants: parts,
        pos,
    })
}

pub(crate) fn parse_pos_mask(raw: &str) -> Option<u32> {
    if raw.is_empty() {
        return None;
    }
    if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        return u32::from_str_radix(hex, 16).ok();
    }
    let mut mask = 0u32;
    for part in raw.split('+') {
        let name = part.trim();
        if name.is_empty() {
            return None;
        }
        mask |= pos_name_bit(name)?;
    }
    Some(mask)
}

fn pos_name_bit(name: &str) -> Option<u32> {
    use novel_segment::POSTAG;
    Some(match name {
        "D_A" => POSTAG::D_A,
        "D_B" => POSTAG::D_B,
        "D_C" => POSTAG::D_C,
        "D_D" => POSTAG::D_D,
        "D_E" => POSTAG::D_E,
        "D_F" => POSTAG::D_F,
        "D_I" => POSTAG::D_I,
        "D_L" => POSTAG::D_L,
        "A_M" => POSTAG::A_M,
        "D_MQ" => POSTAG::D_MQ,
        "D_N" => POSTAG::D_N,
        "D_O" => POSTAG::D_O,
        "D_P" => POSTAG::D_P,
        "A_Q" => POSTAG::A_Q,
        "D_R" => POSTAG::D_R,
        "D_S" => POSTAG::D_S,
        "D_T" => POSTAG::D_T,
        "D_U" => POSTAG::D_U,
        "D_V" => POSTAG::D_V,
        "D_W" => POSTAG::D_W,
        "D_X" => POSTAG::D_X,
        "D_Y" => POSTAG::D_Y,
        "D_Z" => POSTAG::D_Z,
        "A_NR" => POSTAG::A_NR,
        "A_NS" => POSTAG::A_NS,
        "A_NT" => POSTAG::A_NT,
        "A_NX" => POSTAG::A_NX,
        "A_NZ" => POSTAG::A_NZ,
        "UNK" => POSTAG::UNK,
        _ => return None,
    })
}

pub fn format_synonym_file(entries: &[CorrectionEntry], pos_of: &dyn Fn(&str) -> u32) -> String {
    let mut output = String::from(
        "// ConvertZZ 額外修正（與套件同一趟分詞；同義詞格式：正字,錯字|詞性）\n\
         // 由繁體語料經套件引擎 T2S→S2T 後，以分詞對齊產生。\n\
         // 不得寫入 segment-dict 或 cjk-convert-rs 套件資料。\n\
         // 格式：正字,錯字,...|D_F 或 |0x02000000。詞性來自分詞器；套用時只改上下文詞性符合的整詞。\n",
    );
    for entry in entries {
        output.push_str(&entry.canonical);
        for (variant, _) in &entry.variants {
            output.push(',');
            output.push_str(variant);
        }
        output.push_str(&format_pos_suffix(pos_of(&entry.canonical)));
        output.push('\n');
    }
    output
}

fn format_pos_suffix(pos: u32) -> String {
    let pos = if pos == 0 { POSTAG::D_N } else { pos };
    match format_pos_names(pos) {
        Some(names) => format!("|{names}"),
        None => format!("|{pos:#x}"),
    }
}

fn format_pos_names(pos: u32) -> Option<String> {
    const NAMED: &[(&str, u32)] = &[
        ("D_A", POSTAG::D_A),
        ("D_B", POSTAG::D_B),
        ("D_C", POSTAG::D_C),
        ("D_D", POSTAG::D_D),
        ("D_E", POSTAG::D_E),
        ("D_F", POSTAG::D_F),
        ("D_I", POSTAG::D_I),
        ("D_L", POSTAG::D_L),
        ("A_M", POSTAG::A_M),
        ("D_MQ", POSTAG::D_MQ),
        ("D_N", POSTAG::D_N),
        ("D_O", POSTAG::D_O),
        ("D_P", POSTAG::D_P),
        ("A_Q", POSTAG::A_Q),
        ("D_R", POSTAG::D_R),
        ("D_S", POSTAG::D_S),
        ("D_T", POSTAG::D_T),
        ("D_U", POSTAG::D_U),
        ("D_V", POSTAG::D_V),
        ("D_W", POSTAG::D_W),
        ("D_X", POSTAG::D_X),
        ("D_Y", POSTAG::D_Y),
        ("D_Z", POSTAG::D_Z),
        ("A_NR", POSTAG::A_NR),
        ("A_NS", POSTAG::A_NS),
        ("A_NT", POSTAG::A_NT),
        ("A_NX", POSTAG::A_NX),
        ("A_NZ", POSTAG::A_NZ),
    ];
    let mut rest = pos;
    let mut names = Vec::new();
    for (name, bit) in NAMED {
        if rest & bit == *bit {
            names.push(*name);
            rest ^= bit;
        }
    }
    if rest != 0 || names.is_empty() {
        None
    } else {
        Some(names.join("+"))
    }
}

pub fn format_segment_dict(entries: &[CorrectionEntry], pos_of: &dyn Fn(&str) -> u32) -> String {
    let mut output = String::from(
        "// ConvertZZ 額外分詞表（詞|詞性|權值）。正字、錯字與其簡體詞形都寫入。不得併入套件檔。\n",
    );
    let mut seen = HashSet::new();
    for entry in entries {
        let pos = match pos_of(&entry.canonical) {
            0 => POSTAG::D_N,
            value => value,
        };
        let canonical_freq: u64 = entry.variants.iter().map(|item| item.1).sum();
        push_dict_row(
            &mut output,
            &mut seen,
            &entry.canonical,
            pos,
            canonical_freq,
        );
        for (variant, count) in &entry.variants {
            push_dict_row(&mut output, &mut seen, variant, pos, *count);
        }
    }
    for (word, pos, freq) in pinned_dict_words() {
        push_dict_row(&mut output, &mut seen, &word, pos, freq);
    }
    output
}

fn push_dict_row(output: &mut String, seen: &mut HashSet<String>, word: &str, pos: u32, freq: u64) {
    if word.is_empty() || !seen.insert(word.to_string()) {
        return;
    }
    output.push_str(&format!("{word}|{pos:#x}|{freq}\n"));
    let simplified = base_convert(word, Direction::T2s);
    if simplified != word && seen.insert(simplified.clone()) {
        output.push_str(&format!("{simplified}|{pos:#x}|{freq}\n"));
    }
}

pub fn format_pairs_tsv(pairs: &[ReportPair]) -> String {
    let mut output = String::from("canonical\tvariant\tcount\texample\n");
    for pair in pairs {
        let example = pair.examples.first().cloned().unwrap_or_default();
        output.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            pair.canonical,
            pair.variant,
            pair.count,
            example.replace(['\t', '\n'], " ")
        ));
    }
    output
}

pub fn default_segment_dict_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/segment-dict")
}

pub fn default_extra_correction_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/extra-correction")
}

pub fn is_package_data_path(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some("segment-dict" | "ws-segment-rs-dict" | "cjk-convert-rs")
        )
    })
}

pub fn is_extra_correction_path(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|name| name == "extra-correction")
    })
}

pub fn assert_output_outside_sources(output: &Path, sources: &Path) -> Result<(), String> {
    let output_abs = normalize_for_compare(output);
    let sources_abs = normalize_for_compare(sources);
    if output_abs.starts_with(&sources_abs) {
        return Err("輸出目錄不可位於語料來源目錄內，來源語料只讀。".into());
    }
    Ok(())
}

pub fn assert_output_outside_package_data(output: &Path) -> Result<(), String> {
    let output_abs = normalize_for_compare(output);
    if is_package_data_path(&output_abs) {
        return Err(
            "輸出目錄不可位於分詞或簡轉繁套件資料內。這是 ConvertZZ 額外修正，必須與套件字典分開。"
                .into(),
        );
    }
    Ok(())
}

pub fn assert_output_outside_extra_correction(output: &Path) -> Result<(), String> {
    let output_abs = normalize_for_compare(output);
    if is_extra_correction_path(&output_abs) {
        return Err(
            "輸出目錄不可位於 extra-correction 內。檢查點與 state/ 不得寫入套用目錄。".into(),
        );
    }
    Ok(())
}

pub fn assert_paths(output: &Path, sources: &Path) -> Result<(), String> {
    assert_output_outside_sources(output, sources)?;
    assert_output_outside_package_data(output)?;
    assert_output_outside_extra_correction(output)
}

pub fn assert_extra_correction_paths(
    extra: &Path,
    output: &Path,
    sources: &Path,
) -> Result<(), String> {
    if is_package_data_path(extra) {
        return Err("額外修正目錄不可位於分詞或簡轉繁套件資料內。".into());
    }
    if !extra.is_dir() {
        return Err(format!("額外修正目錄不存在：{}", extra.display()));
    }
    let synonym = extra.join("zht.corpus.synonym.txt");
    if !synonym.is_file() {
        return Err(format!(
            "額外修正目錄缺少 zht.corpus.synonym.txt：{}",
            extra.display()
        ));
    }
    let extra_abs = normalize_for_compare(extra);
    let output_abs = normalize_for_compare(output);
    let sources_abs = normalize_for_compare(sources);
    if extra_abs.starts_with(&sources_abs) {
        return Err("額外修正目錄不可位於語料來源目錄內，來源語料只讀。".into());
    }
    if extra_abs == output_abs
        || extra_abs.starts_with(&output_abs)
        || output_abs.starts_with(&extra_abs)
    {
        return Err(
            "額外修正目錄不可與輸出目錄重疊。探針產出不得寫回正在參考的 extra-correction。".into(),
        );
    }
    Ok(())
}

pub(crate) fn normalize_for_compare(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(path)
        }
    })
}

pub fn read_text_lines(
    path: &Path,
) -> Result<impl Iterator<Item = Result<String, String>>, String> {
    let file =
        fs::File::open(path).map_err(|error| format!("無法讀取 {}：{error}", path.display()))?;
    Ok(BufReader::new(file)
        .lines()
        .map(|line| line.map_err(|error| format!("讀取行失敗：{error}"))))
}

#[cfg(test)]
mod tests;

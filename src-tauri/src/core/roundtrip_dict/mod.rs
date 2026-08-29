mod aggregator;
mod checkpoint;
mod memory;
mod run;

pub use aggregator::{
    finish_from_shards, merge_sorted_shard_files, FinishStats, PairAggregator, PairStat,
    TEST_MAX_IN_MEMORY_KEYS, TEST_PEAK_IN_MEMORY_KEYS,
};
pub use checkpoint::{atomic_write, build_fingerprint, Checkpoint, Fingerprint};
pub use memory::{
    default_sampler, FakeSampler, MemoryPolicy, MemorySample, MemorySampler, ResolvedMemory,
};
pub use run::{run_roundtrip, RoundtripRunConfig, RoundtripRunStatus, RunStatus};

use super::conversion::{base_convert, is_cjk_char, ConversionService};
use super::types::Direction;
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

pub(crate) const MIN_WORD_CHARS: usize = 2;
pub(crate) const MAX_WORD_CHARS: usize = 8;
pub(crate) const MAX_LINE_CHARS: usize = 20_000;
/// DictTokenizer `get_chunks` 對長 CJK 句是指數展開；超過此字數會先切開再分詞。
pub(crate) const MAX_UNIT_CHARS: usize = 40;
pub(crate) const MAX_TOKEN_COUNT: usize = 800;
pub(crate) const MAX_EXAMPLES: usize = 3;
const DEFAULT_POS: &str = "0x100000";

#[derive(Clone, Debug)]
pub struct LineResult {
    pub skipped: bool,
    pub mismatched: bool,
    pub pairs: Vec<(String, String)>,
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
        return LineResult {
            skipped: true,
            mismatched: false,
            pairs: Vec::new(),
        };
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
    let mut mismatched = false;
    let mut any = false;
    for unit in units {
        if !should_process(unit) {
            continue;
        }
        any = true;
        let result = process_unit_with_buf(service, unit, Some(buf));
        if result.mismatched {
            mismatched = true;
            pairs.extend(result.pairs);
        }
    }
    if !any {
        return LineResult {
            skipped: true,
            mismatched: false,
            pairs: Vec::new(),
        };
    }
    LineResult {
        skipped: false,
        mismatched,
        pairs,
    }
}

fn process_unit_with_buf(
    service: &ConversionService,
    unit: &str,
    lcs_buf: Option<&mut Vec<u32>>,
) -> LineResult {
    let simplified = service.convert_segmented(unit, Direction::T2s);
    let reconstructed = service.convert_segmented(&simplified, Direction::S2t);
    if reconstructed == unit {
        return LineResult {
            skipped: false,
            mismatched: false,
            pairs: Vec::new(),
        };
    }
    let original_tokens = service.segment_tokens(unit);
    let reconstructed_tokens = service.segment_tokens(&reconstructed);
    if original_tokens.len() > MAX_TOKEN_COUNT || reconstructed_tokens.len() > MAX_TOKEN_COUNT {
        return LineResult {
            skipped: false,
            mismatched: true,
            pairs: Vec::new(),
        };
    }
    LineResult {
        skipped: false,
        mismatched: true,
        pairs: extract_pairs_with_buf(&original_tokens, &reconstructed_tokens, lcs_buf),
    }
}

pub(crate) fn split_process_units(line: &str) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut start = 0;
    for (idx, ch) in line.char_indices() {
        if is_unit_break(ch) {
            let end = idx + ch.len_utf8();
            push_capped(&line[start..end], &mut pieces);
            start = end;
        }
    }
    if start < line.len() {
        push_capped(&line[start..], &mut pieces);
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

fn push_capped<'a>(piece: &'a str, out: &mut Vec<&'a str>) {
    if piece.is_empty() {
        return;
    }
    let mut start = 0;
    let mut count = 0;
    for (i, ch) in piece.char_indices() {
        count += 1;
        if count == MAX_UNIT_CHARS {
            let end = i + ch.len_utf8();
            out.push(&piece[start..end]);
            start = end;
            count = 0;
        }
    }
    if start < piece.len() {
        out.push(&piece[start..]);
    }
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
    for (original_hunk, reconstructed_hunk) in token_replace_hunks(original, reconstructed, lcs_buf)
    {
        pairs.extend(pairs_from_hunk(&original_hunk, &reconstructed_hunk));
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
        let mut index = 0;
        for token in original {
            let len = token.chars().count();
            let span: String = reconstructed_chars[index..index + len].iter().collect();
            index += len;
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

fn usable_pair(variant: &str, canonical: &str) -> Option<(String, String)> {
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

fn token_replace_hunks(
    original: &[String],
    reconstructed: &[String],
    lcs_buf: Option<&mut Vec<u32>>,
) -> Vec<(Vec<String>, Vec<String>)> {
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
        Equal,
        Delete(String),
        Insert(String),
    }
    let mut ops = Vec::new();
    let mut i = n;
    let mut j = m;
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && original[i - 1] == reconstructed[j - 1] {
            ops.push(Op::Equal);
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

    let mut hunks = Vec::new();
    let mut deleted = Vec::new();
    let mut inserted = Vec::new();
    let flush = |deleted: &mut Vec<String>,
                 inserted: &mut Vec<String>,
                 hunks: &mut Vec<(Vec<String>, Vec<String>)>| {
        if !deleted.is_empty() && !inserted.is_empty() {
            hunks.push((std::mem::take(deleted), std::mem::take(inserted)));
        } else {
            deleted.clear();
            inserted.clear();
        }
    };
    for op in ops {
        match op {
            Op::Equal => flush(&mut deleted, &mut inserted, &mut hunks),
            Op::Delete(token) => deleted.push(token),
            Op::Insert(token) => inserted.push(token),
        }
    }
    flush(&mut deleted, &mut inserted, &mut hunks);
    hunks
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
    let line = line.trim();
    if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
        return None;
    }
    let mut parts: Vec<String> = line
        .split(',')
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() < 2 {
        return None;
    }
    let canonical = parts.remove(0);
    Some((canonical, parts))
}

pub fn format_synonym_file(entries: &[CorrectionEntry]) -> String {
    let mut output = String::from(
        "// ConvertZZ 額外修正（新式分詞 convert_synonym 之後套用）\n\
         // 由繁體語料經套件引擎 T2S→S2T 後，以分詞對齊產生。\n\
         // 不得寫入 segment-dict 或 cjk-convert-rs 套件資料。\n\
         // 格式：正字,錯字,...  套用時只取代分詞後的整詞，不做字串暴力取代。\n",
    );
    for entry in entries {
        output.push_str(&entry.canonical);
        for (variant, _) in &entry.variants {
            output.push(',');
            output.push_str(variant);
        }
        output.push('\n');
    }
    output
}

pub fn format_segment_dict(entries: &[CorrectionEntry]) -> String {
    let mut output =
        String::from("// ConvertZZ 額外分詞表，供整詞切出。不得併入 segment-dict 套件檔。\n");
    let mut seen = HashSet::new();
    for entry in entries {
        let canonical_freq: u64 = entry.variants.iter().map(|item| item.1).sum();
        if seen.insert(entry.canonical.clone()) {
            output.push_str(&format!(
                "{}|{DEFAULT_POS}|{canonical_freq}\n",
                entry.canonical
            ));
        }
        for (variant, count) in &entry.variants {
            if seen.insert(variant.clone()) {
                output.push_str(&format!("{variant}|{DEFAULT_POS}|{count}\n"));
            }
        }
    }
    output
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

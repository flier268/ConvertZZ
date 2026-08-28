use super::conversion::{base_convert, is_cjk_char, ConversionService};
use super::types::Direction;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

const MIN_WORD_CHARS: usize = 2;
const MAX_WORD_CHARS: usize = 8;
const MAX_LINE_CHARS: usize = 20_000;
const MAX_TOKEN_COUNT: usize = 800;
const MAX_EXAMPLES: usize = 3;
const DEFAULT_POS: &str = "0x100000";

#[derive(Clone, Debug, Default)]
pub struct PairStat {
    pub count: u64,
    pub examples: Vec<String>,
}

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

#[derive(Clone, Default)]
pub struct PairAggregator {
    stats: HashMap<(String, String), PairStat>,
}

impl PairAggregator {
    pub fn add(&mut self, variant: String, canonical: String, example: &str) {
        let stat = self.stats.entry((variant, canonical)).or_default();
        stat.count += 1;
        if stat.examples.len() < MAX_EXAMPLES && !stat.examples.iter().any(|item| item == example) {
            stat.examples.push(truncate_example(example));
        }
    }

    pub fn merge(&mut self, other: Self) {
        for (key, incoming) in other.stats {
            let stat = self.stats.entry(key).or_default();
            stat.count += incoming.count;
            for example in incoming.examples {
                if stat.examples.len() >= MAX_EXAMPLES {
                    break;
                }
                if !stat.examples.contains(&example) {
                    stat.examples.push(example);
                }
            }
        }
    }

    pub fn raw_occurrences(&self) -> u64 {
        self.stats.values().map(|stat| stat.count).sum()
    }

    pub fn unique_raw_pairs(&self) -> usize {
        self.stats.len()
    }

    pub fn finish(
        self,
        min_count: u64,
        min_dominance: f64,
        skip_variants: &HashSet<String>,
    ) -> (Vec<CorrectionEntry>, FinishStats) {
        let mut by_variant: HashMap<String, Vec<(String, PairStat)>> = HashMap::new();
        for ((variant, canonical), stat) in self.stats {
            by_variant
                .entry(variant)
                .or_default()
                .push((canonical, stat));
        }

        let mut skipped_existing = 0;
        let mut skipped_low_count = 0;
        let mut skipped_ambiguous = 0;
        let mut grouped: HashMap<String, Vec<(String, u64)>> = HashMap::new();
        let mut top_pairs = Vec::new();

        for (variant, mut candidates) in by_variant {
            if skip_variants.contains(&variant) {
                skipped_existing += 1;
                continue;
            }
            candidates.sort_by(|left, right| {
                right
                    .1
                    .count
                    .cmp(&left.1.count)
                    .then_with(|| left.0.cmp(&right.0))
            });
            let total: u64 = candidates.iter().map(|item| item.1.count).sum();
            let (canonical, best) = &candidates[0];
            if best.count < min_count {
                skipped_low_count += 1;
                continue;
            }
            if total == 0 || (best.count as f64 / total as f64) < min_dominance {
                skipped_ambiguous += 1;
                continue;
            }
            if canonical == &variant {
                continue;
            }
            top_pairs.push(ReportPair {
                canonical: canonical.clone(),
                variant: variant.clone(),
                count: best.count,
                examples: best.examples.clone(),
            });
            grouped
                .entry(canonical.clone())
                .or_default()
                .push((variant, best.count));
        }

        top_pairs.sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.canonical.cmp(&right.canonical))
                .then_with(|| left.variant.cmp(&right.variant))
        });

        let mut entries: Vec<CorrectionEntry> = grouped
            .into_iter()
            .map(|(canonical, mut variants)| {
                variants
                    .sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
                CorrectionEntry {
                    canonical,
                    variants,
                }
            })
            .collect();
        entries.sort_by(|left, right| left.canonical.cmp(&right.canonical));

        let kept_variants = entries.iter().map(|entry| entry.variants.len()).sum();
        (
            entries,
            FinishStats {
                skipped_existing,
                skipped_low_count,
                skipped_ambiguous,
                kept_variants,
                top_pairs,
            },
        )
    }
}

pub struct FinishStats {
    pub skipped_existing: usize,
    pub skipped_low_count: usize,
    pub skipped_ambiguous: usize,
    pub kept_variants: usize,
    pub top_pairs: Vec<ReportPair>,
}

pub fn process_line(service: &ConversionService, line: &str) -> LineResult {
    let line = line.trim();
    if !should_process(line) {
        return LineResult {
            skipped: true,
            mismatched: false,
            pairs: Vec::new(),
        };
    }
    let simplified = service.convert_segmented(line, Direction::T2s);
    let reconstructed = service.convert_segmented(&simplified, Direction::S2t);
    if reconstructed == line {
        return LineResult {
            skipped: false,
            mismatched: false,
            pairs: Vec::new(),
        };
    }
    let original_tokens = service.segment_tokens(line);
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
        pairs: extract_pairs(&original_tokens, &reconstructed_tokens),
    }
}

pub fn should_process(line: &str) -> bool {
    if line.is_empty() || line.chars().count() > MAX_LINE_CHARS {
        return false;
    }
    cjk_char_count(line) >= MIN_WORD_CHARS
}

/// Align segmented original tokens with round-tripped tokens.
/// Pairs are word-level; single characters and non-CJK spans are ignored.
pub fn extract_pairs(original: &[String], reconstructed: &[String]) -> Vec<(String, String)> {
    if original == reconstructed {
        return Vec::new();
    }
    let mut pairs = Vec::new();
    for (original_hunk, reconstructed_hunk) in token_replace_hunks(original, reconstructed) {
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
) -> Vec<(Vec<String>, Vec<String>)> {
    let n = original.len();
    let m = reconstructed.len();
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in 0..n {
        for j in 0..m {
            dp[i + 1][j + 1] = if original[i] == reconstructed[j] {
                dp[i][j] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
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
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
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

fn truncate_example(line: &str) -> String {
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
        for line in text.lines() {
            if let Some((_, vars)) = parse_synonym_line(line) {
                variants.extend(vars);
            }
        }
    }
    variants
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

fn normalize_for_compare(path: &Path) -> PathBuf {
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
mod tests {
    use super::*;
    use crate::core::conversion::shared_conversion;

    fn tokens(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_string()).collect()
    }

    #[test]
    fn extract_pairs_uses_original_word_boundaries() {
        let pairs = extract_pairs(&tokens(&["裡面"]), &tokens(&["裏", "面"]));
        assert_eq!(pairs, vec![("裏面".into(), "裡面".into())]);
        assert!(!pairs.iter().any(|(variant, canonical)| {
            variant.chars().count() == 1 || canonical.chars().count() == 1
        }));
    }

    #[test]
    fn extract_pairs_skips_identical_tokens() {
        assert!(extract_pairs(
            &tokens(&["我們", "在", "這裡"]),
            &tokens(&["我們", "在", "這裡"])
        )
        .is_empty());
    }

    #[test]
    fn extract_pairs_aligns_multiple_words() {
        let pairs = extract_pairs(
            &tokens(&["我們", "在", "這裡", "裡面"]),
            &tokens(&["我們", "在", "這裏", "裏面"]),
        );
        assert_eq!(
            pairs,
            vec![
                ("這裏".into(), "這裡".into()),
                ("裏面".into(), "裡面".into()),
            ]
        );
    }

    #[test]
    fn extract_pairs_ignores_single_character_and_non_cjk() {
        let pairs = extract_pairs(&tokens(&["裡", "A"]), &tokens(&["裏", "A"]));
        assert!(pairs.is_empty());
    }

    #[test]
    fn extract_pairs_does_not_emit_character_replace_across_word_boundary() {
        let pairs = extract_pairs(&tokens(&["皇后", "裡面"]), &tokens(&["皇后", "裏面"]));
        assert_eq!(pairs, vec![("裏面".into(), "裡面".into())]);
        assert!(!pairs.iter().any(|(variant, _)| variant.contains("皇后")));
    }

    #[test]
    fn aggregator_keeps_dominant_canonical() {
        let mut aggregator = PairAggregator::default();
        for _ in 0..10 {
            aggregator.add("裏面".into(), "裡面".into(), "冰箱裡面");
        }
        aggregator.add("裏面".into(), "裏麵".into(), "noise");
        let skip = HashSet::new();
        let (entries, stats) = aggregator.finish(3, 0.7, &skip);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].canonical, "裡面");
        assert_eq!(entries[0].variants, vec![("裏面".into(), 10)]);
        assert_eq!(stats.skipped_ambiguous, 0);
    }

    #[test]
    fn aggregator_drops_ambiguous_variant() {
        let mut aggregator = PairAggregator::default();
        aggregator.add("裏面".into(), "裡面".into(), "a");
        aggregator.add("裏面".into(), "裏麵".into(), "b");
        let skip = HashSet::new();
        let (entries, stats) = aggregator.finish(1, 0.7, &skip);
        assert!(entries.is_empty());
        assert_eq!(stats.skipped_ambiguous, 1);
    }

    #[test]
    fn synonym_format_is_canonical_then_variants() {
        let text = format_synonym_file(&[CorrectionEntry {
            canonical: "裡面".into(),
            variants: vec![("裏面".into(), 4), ("里边".into(), 2)],
        }]);
        assert!(text.contains("裡面,裏面,里边\n"));
        assert!(text.contains("分詞"));
    }

    #[test]
    fn parse_synonym_line_skips_comments() {
        assert!(parse_synonym_line("// comment").is_none());
        assert_eq!(
            parse_synonym_line("裡面,裏面"),
            Some(("裡面".into(), vec!["裏面".into()]))
        );
    }

    #[test]
    fn process_line_pairs_are_segmented_words() {
        let service = shared_conversion();
        let result = process_line(service, "冰箱裡面大概就剩幾顆蛋跟半盒牛奶");
        for (variant, canonical) in &result.pairs {
            assert!(is_cjk_word(variant), "{variant}");
            assert!(is_cjk_word(canonical), "{canonical}");
            assert!(variant.chars().count() >= 2);
            assert!(canonical.chars().count() >= 2);
            assert_ne!(variant, canonical);
        }
    }

    #[test]
    fn corpus_files_requires_txt_files() {
        let err = corpus_files(
            Path::new("/tmp/convertzz-missing-corpus"),
            &CorpusSelect::default(),
        )
        .unwrap_err();
        assert!(err.contains("必須是目錄") || err.contains("找不到"));
    }

    #[test]
    fn corpus_files_scans_nested_txt() {
        let root = std::env::temp_dir().join(format!("convertzz-corpus-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("a")).unwrap();
        fs::create_dir_all(root.join("b/nested")).unwrap();
        fs::write(root.join("a/one.txt"), "甲\n").unwrap();
        fs::write(root.join("b/two.txt"), "乙\n").unwrap();
        fs::write(root.join("b/nested/three.txt"), "丙\n").unwrap();
        fs::write(root.join("b/skip.json"), "{}\n").unwrap();
        let files = corpus_files(&root, &CorpusSelect::default()).expect("corpus");
        let names: Vec<String> = files
            .iter()
            .map(|path| {
                path.strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        let _ = fs::remove_dir_all(&root);
        assert_eq!(names, vec!["a/one.txt", "b/nested/three.txt", "b/two.txt"]);
    }

    #[test]
    fn corpus_files_include_only_named_top_level() {
        let root =
            std::env::temp_dir().join(format!("convertzz-corpus-include-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("keep")).unwrap();
        fs::create_dir_all(root.join("skip")).unwrap();
        fs::write(root.join("keep/one.txt"), "甲\n").unwrap();
        fs::write(root.join("skip/two.txt"), "乙\n").unwrap();
        let files = corpus_files(
            &root,
            &CorpusSelect {
                include: vec!["keep".into()],
                exclude: Vec::new(),
            },
        )
        .expect("corpus");
        let names: Vec<String> = files
            .iter()
            .map(|path| {
                path.strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        let _ = fs::remove_dir_all(&root);
        assert_eq!(names, vec!["keep/one.txt"]);
    }

    #[test]
    fn corpus_files_exclude_named_top_level() {
        let root =
            std::env::temp_dir().join(format!("convertzz-corpus-exclude-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("keep")).unwrap();
        fs::create_dir_all(root.join("skip")).unwrap();
        fs::write(root.join("keep/one.txt"), "甲\n").unwrap();
        fs::write(root.join("skip/two.txt"), "乙\n").unwrap();
        let files = corpus_files(
            &root,
            &CorpusSelect {
                include: Vec::new(),
                exclude: vec!["skip".into()],
            },
        )
        .expect("corpus");
        let names: Vec<String> = files
            .iter()
            .map(|path| {
                path.strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        let _ = fs::remove_dir_all(&root);
        assert_eq!(names, vec!["keep/one.txt"]);
    }

    #[test]
    fn output_must_not_sit_inside_sources() {
        let sources = Path::new("/tmp/convertzz-sources-root");
        let output = sources.join("nested");
        let err = assert_output_outside_sources(&output, sources).unwrap_err();
        assert!(err.contains("只讀"));
    }

    #[test]
    fn output_must_not_sit_inside_package_dicts() {
        let output = Path::new("/tmp/app/segment-dict/synonym");
        let err = assert_output_outside_package_data(output).unwrap_err();
        assert!(err.contains("套件"));
        assert!(is_package_data_path(output));
        assert!(!is_package_data_path(Path::new(
            "/tmp/app/extra-correction"
        )));
    }
}

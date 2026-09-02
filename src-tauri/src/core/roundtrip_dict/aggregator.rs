use super::{
    format_pairs_tsv, format_segment_dict, format_synonym_file, CorrectionEntry, ReportPair,
    MAX_EXAMPLES,
};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

pub const SHARD_MAGIC: &[u8] = b"CZRTPAIRS1\n";
pub const SHARD_FORMAT: &str = "czrt-pairs-v1";
const MAX_FIELD_BYTES: u32 = 4096;
const MAX_SHARD_BYTES: u64 = 8 * 1024 * 1024 * 1024;
pub const EST_BYTES_PER_PAIR: u64 = 768;

pub static TEST_MAX_IN_MEMORY_KEYS: AtomicUsize = AtomicUsize::new(0);
pub static TEST_PEAK_IN_MEMORY_KEYS: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, Default)]
pub struct PairStat {
    pub count: u64,
    pub examples: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct PairAggregator {
    stats: HashMap<(String, String), PairStat>,
    /// 原文詞頻。引擎產出的異體若本身也是語料正詞，不可互代。
    originals: HashMap<String, u64>,
}

impl PairAggregator {
    pub fn add(&mut self, variant: String, canonical: String, example: &str) {
        let stat = self.stats.entry((variant, canonical)).or_default();
        stat.count += 1;
        if stat.examples.len() < MAX_EXAMPLES && !stat.examples.iter().any(|item| item == example) {
            stat.examples.push(super::truncate_example(example));
        }
    }

    pub fn note_original(&mut self, word: impl Into<String>) {
        *self.originals.entry(word.into()).or_insert(0) += 1;
    }

    pub fn merge(&mut self, other: Self) {
        for (key, incoming) in other.stats {
            let stat = self.stats.entry(key).or_default();
            stat.count = stat.count.saturating_add(incoming.count);
            for example in incoming.examples {
                if stat.examples.len() >= MAX_EXAMPLES {
                    break;
                }
                if !stat.examples.contains(&example) {
                    stat.examples.push(example);
                }
            }
        }
        for (word, count) in other.originals {
            *self.originals.entry(word).or_insert(0) += count;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.stats.is_empty() && self.originals.is_empty()
    }

    pub fn raw_occurrences(&self) -> u64 {
        self.stats.values().map(|stat| stat.count).sum()
    }

    pub fn unique_raw_pairs(&self) -> usize {
        self.stats.len()
    }

    pub fn estimated_bytes(&self) -> u64 {
        (self.stats.len() as u64 + self.originals.len() as u64) * EST_BYTES_PER_PAIR
    }

    pub fn finish(
        &self,
        min_count: u64,
        min_dominance: f64,
        skip_variants: &HashSet<String>,
    ) -> (Vec<CorrectionEntry>, FinishStats) {
        let mut by_variant: HashMap<String, Vec<(String, PairStat)>> = HashMap::new();
        for ((variant, canonical), stat) in &self.stats {
            if variant == canonical {
                continue;
            }
            by_variant
                .entry(variant.clone())
                .or_default()
                .push((canonical.clone(), stat.clone()));
        }
        let unique = self.stats.len();
        let occurrences = self.raw_occurrences();
        let (entries, mut stats) = finish_grouped(
            by_variant,
            min_count,
            min_dominance,
            skip_variants,
            &self.originals,
        );
        stats.raw_pair_occurrences = occurrences;
        stats.unique_raw_pairs = unique;
        (entries, stats)
    }

    pub fn write_shard<W: Write>(&self, writer: &mut W) -> Result<(), String> {
        writer
            .write_all(SHARD_MAGIC)
            .map_err(|error| format!("寫入 shard 失敗：{error}"))?;
        let identity_stats: HashMap<(String, String), PairStat> = self
            .originals
            .iter()
            .map(|(word, count)| {
                (
                    (word.clone(), word.clone()),
                    PairStat {
                        count: *count,
                        examples: Vec::new(),
                    },
                )
            })
            .collect();
        let mut records: Vec<(&(String, String), &PairStat)> = self.stats.iter().collect();
        records.extend(identity_stats.iter().map(|(k, v)| (k, v)));
        records.sort_by(|left, right| {
            left.0
                 .0
                .as_bytes()
                .cmp(right.0 .0.as_bytes())
                .then_with(|| left.0 .1.as_bytes().cmp(right.0 .1.as_bytes()))
        });
        let mut previous: Option<(&str, &str)> = None;
        for ((variant, canonical), stat) in records {
            if let Some((prev_v, prev_c)) = previous {
                if (variant.as_bytes(), canonical.as_bytes())
                    <= (prev_v.as_bytes(), prev_c.as_bytes())
                {
                    return Err("shard 鍵必須嚴格遞增。".into());
                }
            }
            write_record(writer, variant, canonical, stat)?;
            previous = Some((variant.as_str(), canonical.as_str()));
        }
        Ok(())
    }

    pub fn write_shard_path(&self, path: &Path) -> Result<(), String> {
        write_via_temp(path, |writer| self.write_shard(writer))
    }

    pub fn read_shard<R: Read>(reader: &mut R) -> Result<Self, String> {
        expect_magic(reader)?;
        let mut aggregator = PairAggregator::default();
        let mut previous: Option<(String, String)> = None;
        loop {
            let Some(record) = read_record(reader)? else {
                break;
            };
            ensure_increasing(&previous, &record.variant, &record.canonical)?;
            previous = Some((record.variant.clone(), record.canonical.clone()));
            if record.variant == record.canonical {
                *aggregator.originals.entry(record.variant).or_insert(0) += record.count;
            } else {
                aggregator.stats.insert(
                    (record.variant, record.canonical),
                    PairStat {
                        count: record.count,
                        examples: record.examples,
                    },
                );
            }
        }
        Ok(aggregator)
    }
}

pub struct FinishStats {
    pub skipped_existing: usize,
    pub skipped_low_count: usize,
    pub skipped_ambiguous: usize,
    pub kept_variants: usize,
    pub top_pairs: Vec<ReportPair>,
    pub raw_pair_occurrences: u64,
    pub unique_raw_pairs: usize,
    all_pairs: Vec<ReportPair>,
}

impl FinishStats {
    pub fn pairs_for_tsv(&self) -> &[ReportPair] {
        if self.all_pairs.is_empty() {
            &self.top_pairs
        } else {
            &self.all_pairs
        }
    }
}

struct ShardRecord {
    variant: String,
    canonical: String,
    count: u64,
    examples: Vec<String>,
}

fn finish_grouped(
    by_variant: HashMap<String, Vec<(String, PairStat)>>,
    min_count: u64,
    min_dominance: f64,
    skip_variants: &HashSet<String>,
    originals: &HashMap<String, u64>,
) -> (Vec<CorrectionEntry>, FinishStats) {
    let mut skipped_existing = 0;
    let mut skipped_low_count = 0;
    let mut skipped_ambiguous = 0;
    let mut grouped: HashMap<String, Vec<(String, u64)>> = HashMap::new();
    let mut all_pairs = Vec::new();

    for (variant, mut candidates) in by_variant {
        if skip_variants.contains(&variant) {
            skipped_existing += 1;
            continue;
        }
        if originals.get(&variant).copied().unwrap_or(0) >= min_count {
            skipped_ambiguous += 1;
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
        all_pairs.push(ReportPair {
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

    all_pairs.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.canonical.cmp(&right.canonical))
            .then_with(|| left.variant.cmp(&right.variant))
    });
    let mut top_pairs = all_pairs.clone();
    top_pairs.truncate(50);

    let mut entries: Vec<CorrectionEntry> = grouped
        .into_iter()
        .map(|(canonical, mut variants)| {
            variants.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
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
            raw_pair_occurrences: 0,
            unique_raw_pairs: 0,
            all_pairs,
        },
    )
}

fn write_via_temp(
    path: &Path,
    write: impl FnOnce(&mut BufWriter<File>) -> Result<(), String>,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("無法建立 {}：{error}", parent.display()))?;
    }
    let tmp = super::checkpoint::tmp_path(path);
    {
        let file =
            File::create(&tmp).map_err(|error| format!("無法建立 {}：{error}", tmp.display()))?;
        let mut writer = BufWriter::new(file);
        write(&mut writer)?;
        writer
            .flush()
            .map_err(|error| format!("寫入 {} 失敗：{error}", tmp.display()))?;
        writer
            .get_ref()
            .sync_all()
            .map_err(|error| format!("sync {} 失敗：{error}", tmp.display()))?;
    }
    super::checkpoint::replace_file(&tmp, path)
}

pub fn merge_sorted_shard_files(inputs: &[PathBuf], output: &Path) -> Result<(), String> {
    write_via_temp(output, |writer| merge_sorted_into(inputs, writer))
}

fn merge_sorted_into(inputs: &[PathBuf], writer: &mut BufWriter<File>) -> Result<(), String> {
    writer
        .write_all(SHARD_MAGIC)
        .map_err(|error| format!("寫入 shard 失敗：{error}"))?;
    if inputs.is_empty() {
        return Ok(());
    }
    let mut readers = Vec::new();
    for path in inputs {
        readers.push(ShardReader::open(path)?);
    }
    TEST_PEAK_IN_MEMORY_KEYS.store(0, Ordering::SeqCst);
    let mut live = 0usize;
    let mut heap: BinaryHeap<Reverse<(Vec<u8>, Vec<u8>, usize)>> = BinaryHeap::new();
    for (idx, reader) in readers.iter_mut().enumerate() {
        if reader.peek()?.is_some() {
            bump_live(&mut live)?;
            let rec = reader.current().expect("peeked");
            heap.push(Reverse((
                rec.variant.as_bytes().to_vec(),
                rec.canonical.as_bytes().to_vec(),
                idx,
            )));
        }
    }
    let mut last_written: Option<(Vec<u8>, Vec<u8>)> = None;
    while let Some(Reverse((_, _, idx))) = heap.pop() {
        live = live.saturating_sub(1);
        let mut merged = readers[idx].take().expect("heap idx");
        let mut extras: Vec<(usize, ShardRecord)> = Vec::new();
        while let Some(Reverse((v, c, other))) = heap.peek() {
            if v.as_slice() == merged.variant.as_bytes()
                && c.as_slice() == merged.canonical.as_bytes()
            {
                let other = other.to_owned();
                heap.pop();
                live = live.saturating_sub(1);
                if let Some(record) = readers[other].take() {
                    extras.push((other, record));
                }
            } else {
                break;
            }
        }
        extras.sort_by_key(|(index, _)| *index);
        let mut parts = vec![(idx, merged.count, std::mem::take(&mut merged.examples))];
        let mut refill = vec![idx];
        for (other, record) in extras {
            parts.push((other, record.count, record.examples));
            refill.push(other);
        }
        parts.sort_by_key(|(index, _, _)| *index);
        merged.count = 0;
        merged.examples.clear();
        for (_, count, examples) in parts {
            let next = merged.count.saturating_add(count);
            if next < merged.count {
                eprintln!("警告：pair 計數飽和。");
            }
            merged.count = next;
            merge_examples(&mut merged.examples, examples);
        }
        let key = (
            merged.variant.as_bytes().to_vec(),
            merged.canonical.as_bytes().to_vec(),
        );
        if let Some(previous) = &last_written {
            if key <= *previous {
                return Err("合併後 shard 鍵非嚴格遞增。".into());
            }
        }
        write_record(
            writer,
            &merged.variant,
            &merged.canonical,
            &PairStat {
                count: merged.count,
                examples: merged.examples,
            },
        )?;
        last_written = Some(key);
        for other in refill {
            if readers[other].peek()?.is_some() {
                bump_live(&mut live)?;
                let rec = readers[other].current().expect("peeked");
                heap.push(Reverse((
                    rec.variant.as_bytes().to_vec(),
                    rec.canonical.as_bytes().to_vec(),
                    other,
                )));
            }
        }
    }
    Ok(())
}

fn bump_live(live: &mut usize) -> Result<(), String> {
    *live += 1;
    TEST_PEAK_IN_MEMORY_KEYS.fetch_max(*live, Ordering::SeqCst);
    let limit = TEST_MAX_IN_MEMORY_KEYS.load(Ordering::SeqCst);
    if limit > 0 && *live > limit {
        return Err(format!("merge 工作集超過 max_in_memory_keys={limit}"));
    }
    Ok(())
}

fn merge_examples(dest: &mut Vec<String>, incoming: Vec<String>) {
    for example in incoming {
        if dest.len() >= MAX_EXAMPLES {
            break;
        }
        if !dest.contains(&example) {
            dest.push(example);
        }
    }
}

struct ShardReader {
    reader: BufReader<File>,
    current: Option<ShardRecord>,
    previous: Option<(String, String)>,
}

impl ShardReader {
    fn open(path: &Path) -> Result<Self, String> {
        let meta = std::fs::metadata(path)
            .map_err(|error| format!("無法讀取 {}：{error}", path.display()))?;
        if meta.len() > MAX_SHARD_BYTES {
            return Err(format!("{} 超過 8 GiB。", path.display()));
        }
        let file =
            File::open(path).map_err(|error| format!("無法讀取 {}：{error}", path.display()))?;
        let mut reader = BufReader::new(file);
        expect_magic(&mut reader)?;
        Ok(Self {
            reader,
            current: None,
            previous: None,
        })
    }

    fn peek(&mut self) -> Result<Option<&ShardRecord>, String> {
        if self.current.is_none() {
            if let Some(record) = read_record(&mut self.reader)? {
                ensure_increasing(&self.previous, &record.variant, &record.canonical)?;
                self.previous = Some((record.variant.clone(), record.canonical.clone()));
                self.current = Some(record);
            }
        }
        Ok(self.current.as_ref())
    }

    fn current(&self) -> Option<&ShardRecord> {
        self.current.as_ref()
    }

    fn take(&mut self) -> Option<ShardRecord> {
        self.current.take()
    }
}

pub fn finish_from_shards(
    paths: &[PathBuf],
    min_count: u64,
    min_dominance: f64,
    skip_variants: &HashSet<String>,
) -> Result<(Vec<CorrectionEntry>, FinishStats), String> {
    if paths.is_empty() {
        return Ok(finish_grouped(
            HashMap::new(),
            min_count,
            min_dominance,
            skip_variants,
            &HashMap::new(),
        ));
    }
    let mut readers = Vec::new();
    for path in paths {
        readers.push(ShardReader::open(path)?);
    }
    let mut heap: BinaryHeap<Reverse<(Vec<u8>, Vec<u8>, usize)>> = BinaryHeap::new();
    for (idx, reader) in readers.iter_mut().enumerate() {
        if reader.peek()?.is_some() {
            let rec = reader.current().expect("peeked");
            heap.push(Reverse((
                rec.variant.as_bytes().to_vec(),
                rec.canonical.as_bytes().to_vec(),
                idx,
            )));
        }
    }
    let mut by_variant: HashMap<String, Vec<(String, PairStat)>> = HashMap::new();
    let mut originals: HashMap<String, u64> = HashMap::new();
    let mut unique = 0usize;
    let mut occurrences = 0u64;
    let mut current_variant: Option<String> = None;
    let mut candidates: Vec<(String, PairStat)> = Vec::new();

    let flush = |variant: Option<String>,
                 candidates: &mut Vec<(String, PairStat)>,
                 by_variant: &mut HashMap<String, Vec<(String, PairStat)>>| {
        if let Some(variant) = variant {
            if !candidates.is_empty() {
                by_variant.insert(variant, std::mem::take(candidates));
            }
        }
    };

    while let Some(Reverse((_, _, idx))) = heap.pop() {
        let mut merged = readers[idx].take().expect("heap");
        let mut extras = Vec::new();
        while let Some(Reverse((v, c, other))) = heap.peek() {
            if v.as_slice() == merged.variant.as_bytes()
                && c.as_slice() == merged.canonical.as_bytes()
            {
                let other = *other;
                heap.pop();
                if let Some(record) = readers[other].take() {
                    extras.push((other, record));
                }
            } else {
                break;
            }
        }
        extras.sort_by_key(|(index, _)| *index);
        let mut parts = vec![(idx, merged.count, std::mem::take(&mut merged.examples))];
        let mut refill = vec![idx];
        for (other, record) in extras {
            parts.push((other, record.count, record.examples));
            refill.push(other);
        }
        parts.sort_by_key(|(index, _, _)| *index);
        merged.count = 0;
        merged.examples.clear();
        for (_, count, examples) in parts {
            merged.count = merged.count.saturating_add(count);
            merge_examples(&mut merged.examples, examples);
        }
        if merged.variant == merged.canonical {
            *originals.entry(merged.variant).or_insert(0) += merged.count;
        } else {
            unique += 1;
            occurrences = occurrences.saturating_add(merged.count);
            if current_variant.as_deref() != Some(merged.variant.as_str()) {
                flush(current_variant.take(), &mut candidates, &mut by_variant);
                current_variant = Some(merged.variant.clone());
            }
            candidates.push((
                merged.canonical,
                PairStat {
                    count: merged.count,
                    examples: merged.examples,
                },
            ));
        }
        for other in refill {
            if readers[other].peek()?.is_some() {
                let rec = readers[other].current().expect("peeked");
                heap.push(Reverse((
                    rec.variant.as_bytes().to_vec(),
                    rec.canonical.as_bytes().to_vec(),
                    other,
                )));
            }
        }
    }
    flush(current_variant, &mut candidates, &mut by_variant);
    let (entries, mut stats) = finish_grouped(
        by_variant,
        min_count,
        min_dominance,
        skip_variants,
        &originals,
    );
    stats.raw_pair_occurrences = occurrences;
    stats.unique_raw_pairs = unique;
    Ok((entries, stats))
}

pub fn write_derived_outputs(
    output: &Path,
    entries: &[CorrectionEntry],
    stats: &FinishStats,
    report: &super::RoundtripReport,
    pos_of: &dyn Fn(&str) -> u32,
) -> Result<(), String> {
    super::checkpoint::atomic_write(
        &output.join("zht.corpus.synonym.txt"),
        format_synonym_file(entries, pos_of).as_bytes(),
    )?;
    super::checkpoint::atomic_write(
        &output.join("zht.corpus.dict.txt"),
        format_segment_dict(entries, pos_of).as_bytes(),
    )?;
    super::checkpoint::atomic_write(
        &output.join("pairs.tsv"),
        format_pairs_tsv(stats.pairs_for_tsv()).as_bytes(),
    )?;
    let report_text =
        serde_json::to_string_pretty(report).map_err(|error| format!("無法序列化報告：{error}"))?;
    super::checkpoint::atomic_write(
        &output.join("report.json"),
        format!("{report_text}\n").as_bytes(),
    )
}

fn expect_magic(reader: &mut impl Read) -> Result<(), String> {
    let mut magic = [0u8; 11];
    reader
        .read_exact(&mut magic)
        .map_err(|error| format!("讀取 shard magic 失敗：{error}"))?;
    if magic != SHARD_MAGIC {
        return Err("shard magic 不符。".into());
    }
    Ok(())
}

fn ensure_increasing(
    previous: &Option<(String, String)>,
    variant: &str,
    canonical: &str,
) -> Result<(), String> {
    if let Some((prev_v, prev_c)) = previous {
        if (variant.as_bytes(), canonical.as_bytes()) <= (prev_v.as_bytes(), prev_c.as_bytes()) {
            return Err("shard 鍵非嚴格遞增。".into());
        }
    }
    Ok(())
}

fn write_record(
    writer: &mut impl Write,
    variant: &str,
    canonical: &str,
    stat: &PairStat,
) -> Result<(), String> {
    write_len_str(writer, variant)?;
    write_len_str(writer, canonical)?;
    writer
        .write_all(&stat.count.to_le_bytes())
        .map_err(|error| format!("寫入 count 失敗：{error}"))?;
    let example_count = u8::try_from(stat.examples.len().min(MAX_EXAMPLES)).unwrap_or(0);
    writer
        .write_all(&[example_count])
        .map_err(|error| format!("寫入例句數失敗：{error}"))?;
    for example in stat.examples.iter().take(MAX_EXAMPLES) {
        write_len_str(writer, example)?;
    }
    Ok(())
}

fn write_len_str(writer: &mut impl Write, text: &str) -> Result<(), String> {
    let bytes = text.as_bytes();
    if bytes.len() > MAX_FIELD_BYTES as usize {
        return Err("欄位超過 4096 位元組。".into());
    }
    let len = bytes.len() as u32;
    writer
        .write_all(&len.to_le_bytes())
        .map_err(|error| format!("寫入長度失敗：{error}"))?;
    writer
        .write_all(bytes)
        .map_err(|error| format!("寫入欄位失敗：{error}"))
}

fn read_record(reader: &mut impl Read) -> Result<Option<ShardRecord>, String> {
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(format!("讀取 shard 失敗：{error}")),
    }
    let variant = read_str_with_len(reader, u32::from_le_bytes(len_buf))?;
    let canonical = read_str(reader)?;
    let mut count_buf = [0u8; 8];
    reader
        .read_exact(&mut count_buf)
        .map_err(|_| "shard 截斷（count）。".to_string())?;
    let count = u64::from_le_bytes(count_buf);
    let mut example_count = [0u8; 1];
    reader
        .read_exact(&mut example_count)
        .map_err(|_| "shard 截斷（例句數）。".to_string())?;
    if example_count[0] > MAX_EXAMPLES as u8 {
        return Err("example_count > 3。".into());
    }
    let mut examples = Vec::new();
    for _ in 0..example_count[0] {
        examples.push(read_str(reader)?);
    }
    Ok(Some(ShardRecord {
        variant,
        canonical,
        count,
        examples,
    }))
}

fn read_str(reader: &mut impl Read) -> Result<String, String> {
    let mut len_buf = [0u8; 4];
    reader
        .read_exact(&mut len_buf)
        .map_err(|_| "shard 截斷（長度）。".to_string())?;
    read_str_with_len(reader, u32::from_le_bytes(len_buf))
}

fn read_str_with_len(reader: &mut impl Read, len: u32) -> Result<String, String> {
    if len > MAX_FIELD_BYTES {
        return Err("欄位超過 4096 位元組。".into());
    }
    let mut buf = vec![0u8; len as usize];
    reader
        .read_exact(&mut buf)
        .map_err(|_| "shard 截斷（欄位）。".to_string())?;
    String::from_utf8(buf).map_err(|_| "shard 欄位不是 UTF-8。".to_string())
}

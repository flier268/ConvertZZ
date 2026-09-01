use super::aggregator::{
    compact_shards, finish_from_shards, merge_sorted_shard_files, next_shard_relative,
    write_derived_outputs, PairAggregator,
};
use super::checkpoint::{
    build_fingerprint, fingerprint_mismatch_message, list_uncommitted_for, load_checkpoint,
    reset_output, save_checkpoint, uncommitted_name, validate_completed_paths, wipe_uncommitted,
    Checkpoint,
};
use super::memory::{
    aggregator_estimate, auto_lcs_inflight, inflight_estimate, resolve_thresholds, usage,
    CountingSemaphore, LcsPool, MemoryPolicy, MemorySampler, SampleClock,
};
use super::{
    assert_extra_correction_paths, assert_paths, corpus_files, default_segment_dict_root,
    load_existing_synonym_variants, load_extra_correction_variants, process_line_with_buf,
    read_text_lines, CorpusSelect, RoundtripReport,
};
use crate::core::conversion::ConversionService;
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

const MIN_BATCH: usize = 64;
const PROGRESS_EVERY: u64 = 10_000;

pub struct RoundtripRunConfig {
    pub sources: PathBuf,
    pub output: PathBuf,
    pub select: CorpusSelect,
    pub min_count: u64,
    pub min_dominance: f64,
    pub limit: Option<u64>,
    pub jobs: usize,
    pub batch_size: usize,
    pub memory: MemoryPolicy,
    pub reset: bool,
    pub rebuild_outputs_only: bool,
    pub extra_correction: Option<PathBuf>,
    pub stop: Arc<AtomicBool>,
    pub sampler: Arc<dyn MemorySampler>,
    pub lines_processed: Option<Arc<AtomicU64>>,
    pub files_opened: Option<Arc<AtomicU64>>,
    pub process_line_inflight: Option<Arc<AtomicUsize>>,
    pub process_line_peak: Option<Arc<AtomicUsize>>,
    pub jobs_current_probe: Option<Arc<AtomicUsize>>,
    pub lcs_peak: Option<Arc<AtomicUsize>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunStatus {
    Complete,
    Interrupted,
    MemoryHard,
    Limit,
}

#[derive(Debug)]
pub struct RoundtripRunStatus {
    pub status: RunStatus,
    pub files_committed: Vec<String>,
    pub lines_read: u64,
    pub lines_skipped: u64,
    pub lines_mismatched: u64,
    pub unique_raw_pairs: usize,
    pub last_hard_file: Option<String>,
    pub jobs_final: usize,
    pub resumed_from_checkpoint: bool,
}

pub fn run_roundtrip(
    service: &ConversionService,
    config: RoundtripRunConfig,
) -> Result<RoundtripRunStatus, String> {
    assert_paths(&config.output, &config.sources)?;
    if let Some(extra) = config.extra_correction.as_deref() {
        assert_extra_correction_paths(extra, &config.output, &config.sources)?;
    }
    if config.output.exists() && config.output.is_file() {
        return Err("輸出路徑必須是目錄。".into());
    }
    if config.jobs == 0 {
        return Err("--jobs 必須大於 0。".into());
    }
    std::fs::create_dir_all(&config.output)
        .map_err(|error| format!("無法建立輸出目錄：{error}"))?;
    std::fs::create_dir_all(config.output.join("state/shards"))
        .map_err(|error| format!("無法建立 shards：{error}"))?;

    if config.reset {
        reset_output(&config.output)?;
        std::fs::create_dir_all(config.output.join("state/shards"))
            .map_err(|error| format!("無法建立 shards：{error}"))?;
    }

    if config.rebuild_outputs_only {
        return rebuild_only(service, &config);
    }

    let files = corpus_files(&config.sources, &config.select)?;
    for path in &files {
        if !path.starts_with(&config.sources) {
            return Err(format!("拒絕讀取來源目錄以外的檔案：{}", path.display()));
        }
    }
    let fingerprint = build_fingerprint(
        &config.sources,
        &config.select,
        &files,
        config.extra_correction.as_deref(),
    )?;
    let existing = load_checkpoint(&config.output)?;
    let resumed = existing.is_some();
    let mut checkpoint = match existing {
        Some(checkpoint) => {
            if checkpoint.fingerprint != fingerprint {
                return Err(fingerprint_mismatch_message().into());
            }
            validate_completed_paths(&config.sources, &checkpoint.completed_files)?;
            checkpoint
        }
        None => Checkpoint::new(fingerprint),
    };

    if checkpoint.status == "complete" {
        eprintln!("已完成，略過");
        return Ok(status_from_checkpoint(
            &checkpoint,
            RunStatus::Complete,
            config.jobs,
            resumed,
        ));
    }

    wipe_uncommitted(&config.output)?;

    let mut sample = config.sampler.sample();
    let linux = cfg!(target_os = "linux");
    if !linux && config.memory.hard_bytes.is_none() {
        eprintln!("警告：非 Linux 未指定 --memory-hard-mb，改用保守預設 2048 MiB。");
    }
    let resolved = resolve_thresholds(&config.memory, &sample, linux)?;
    let (baseline, kind) = usage(&sample);
    if let Some(hard) = resolved.hard {
        if baseline >= hard {
            return Err("硬上限低於轉換引擎基線記憶體。".into());
        }
    }
    let mut jobs_current = config.jobs;
    let mut batch_size = config.batch_size.max(MIN_BATCH);
    let mut lcs_inflight = config
        .memory
        .lcs_inflight
        .unwrap_or_else(|| auto_lcs_inflight(jobs_current, resolved.soft));
    if resolved.soft.is_some() && baseline >= resolved.soft.unwrap() {
        jobs_current = 1;
        batch_size = MIN_BATCH;
        lcs_inflight = 1;
        eprintln!("基線已過軟水位，改從 jobs=1、batch=64 起跳。");
    }
    eprintln!(
        "記憶體：soft={} hard={} 比較元={:?} jobs={} lcs_inflight={}",
        fmt_mb(resolved.soft),
        fmt_mb(resolved.hard),
        kind,
        jobs_current,
        lcs_inflight
    );
    if resolved.warned_estimate {
        eprintln!("警告：記憶體取樣失敗，僅以估量比較水位。");
    }
    probe_jobs(&config, jobs_current);

    let skip_variants = skip_variants_for(&config)?;
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(config.jobs)
        .build()
        .map_err(|error| format!("無法建立執行緒池：{error}"))?;
    let line_sem = Arc::new(CountingSemaphore::new(jobs_current));
    let lcs_pool = Arc::new(LcsPool::new(lcs_inflight));
    let clock = SampleClock::default();
    let started = Instant::now();
    let mut remaining = config
        .limit
        .map(|max| max.saturating_sub(checkpoint.lines_read));
    if remaining == Some(0) {
        checkpoint.status = "limit".into();
        save_checkpoint(&config.output, &mut checkpoint)?;
        write_outputs(service, &config, &checkpoint, &skip_variants)?;
        return Ok(status_from_checkpoint(
            &checkpoint,
            RunStatus::Limit,
            jobs_current,
            resumed,
        ));
    }

    checkpoint.status = "running".into();
    let completed: HashSet<String> = checkpoint.completed_files.iter().cloned().collect();
    let mut below_soft_streak = 0u32;

    for file in &files {
        let relative = relative_path(file, &config.sources);
        if completed.contains(&relative) {
            continue;
        }
        if remaining == Some(0) {
            break;
        }
        if let Some(counter) = &config.files_opened {
            counter.fetch_add(1, Ordering::SeqCst);
        }
        wipe_uncommitted(&config.output)?;
        let mut hard_strikes = 0u32;
        let mut file_acc = PairAggregator::default();
        let mut uncommitted_seq = 1u32;
        let mut file_lines = 0u64;
        let mut file_skipped = 0u64;
        let mut file_mismatched = 0u64;
        eprintln!("讀取 {}", file.display());
        let mut last_progress = Instant::now();
        let mut iterator = read_text_lines(file)?;
        let mut exhausted = false;
        loop {
            let mut batch = Vec::with_capacity(batch_size);
            let mut hit_eof = false;
            while batch.len() < batch_size {
                match iterator.next() {
                    Some(Ok(line)) => batch.push(line),
                    Some(Err(error)) => return Err(error),
                    None => {
                        hit_eof = true;
                        exhausted = true;
                        break;
                    }
                }
            }
            let mut limit_cut = false;
            if let Some(left) = remaining {
                if left == 0 {
                    limit_cut = true;
                    batch.clear();
                    hit_eof = false;
                } else if (left as usize) < batch.len() {
                    batch.truncate(left as usize);
                    limit_cut = true;
                    hit_eof = false;
                }
            }
            let true_eof = hit_eof && !limit_cut;

            if !batch.is_empty() {
                let outcome = process_batch(&pool, service, &batch, &line_sem, &lcs_pool, &config);
                file_lines += outcome.lines_read;
                file_skipped += outcome.lines_skipped;
                file_mismatched += outcome.lines_mismatched;
                file_acc.merge(outcome.aggregator);
                if let Some(left) = remaining.as_mut() {
                    *left = left.saturating_sub(outcome.lines_read);
                }
                if file_lines == outcome.lines_read
                    || file_lines % PROGRESS_EVERY < outcome.lines_read.max(1)
                    || last_progress.elapsed().as_secs() >= 2
                {
                    let elapsed = started.elapsed().as_secs_f64().max(0.001);
                    eprintln!(
                        "已處理 {} 行，回環差異 {}，候選 {}，{:.0} 行/s，{:.1}s",
                        checkpoint.lines_read + file_lines,
                        checkpoint.lines_mismatched + file_mismatched,
                        file_acc.unique_raw_pairs(),
                        (checkpoint.lines_read + file_lines) as f64 / elapsed,
                        elapsed
                    );
                    last_progress = Instant::now();
                }
            }

            if clock.due() {
                sample =
                    decorate_sample(config.sampler.sample(), &file_acc, lcs_inflight, batch_size);
            } else {
                sample.aggregator_est = file_acc.estimated_bytes();
                sample.inflight_est = inflight_estimate(lcs_inflight, batch_size);
            }
            let (used, _) = usage(&sample);
            let stop = config.stop.load(Ordering::SeqCst);
            let over_hard = resolved.hard.is_some_and(|hard| used >= hard);
            let over_soft = resolved.soft.is_some_and(|soft| used >= soft);

            if true_eof || (exhausted && batch.is_empty() && !limit_cut) {
                durable_commit(
                    &config,
                    &mut checkpoint,
                    &relative,
                    file_acc,
                    file_lines,
                    file_skipped,
                    file_mismatched,
                    &uncommitted_seq,
                )?;
                file_acc = PairAggregator::default();
                if let Some(peak) = &config.lcs_peak {
                    peak.store(lcs_pool.peak(), Ordering::SeqCst);
                }
                if stop {
                    return finish_run(
                        service,
                        &config,
                        &mut checkpoint,
                        RunStatus::Interrupted,
                        jobs_current,
                        resumed,
                        &skip_variants,
                    );
                }
                if over_hard {
                    checkpoint.last_hard_file = Some(relative.clone());
                    return finish_run(
                        service,
                        &config,
                        &mut checkpoint,
                        RunStatus::MemoryHard,
                        jobs_current,
                        resumed,
                        &skip_variants,
                    );
                }
                if remaining == Some(0) {
                    return finish_run(
                        service,
                        &config,
                        &mut checkpoint,
                        RunStatus::Limit,
                        jobs_current,
                        resumed,
                        &skip_variants,
                    );
                }
                break;
            }
            if stop {
                drop(file_acc);
                wipe_uncommitted(&config.output)?;
                return finish_run(
                    service,
                    &config,
                    &mut checkpoint,
                    RunStatus::Interrupted,
                    jobs_current,
                    resumed,
                    &skip_variants,
                );
            }
            if over_hard && hard_strikes >= 1 {
                drop(file_acc);
                wipe_uncommitted(&config.output)?;
                let repeat = checkpoint.last_hard_file.as_deref() == Some(relative.as_str());
                checkpoint.last_hard_file = Some(relative.clone());
                if repeat {
                    eprintln!("單檔超過硬上限，請切分 {relative}");
                } else {
                    eprintln!("記憶體硬上限，停下：{relative}");
                }
                return finish_run(
                    service,
                    &config,
                    &mut checkpoint,
                    RunStatus::MemoryHard,
                    jobs_current,
                    resumed,
                    &skip_variants,
                );
            }
            if over_hard && hard_strikes == 0 {
                hard_strikes = 1;
                jobs_current = 1;
                batch_size = MIN_BATCH;
                lcs_inflight = 1;
                line_sem.set_max(1);
                lcs_pool.resize(1);
                probe_jobs(&config, jobs_current);
                spill_file_acc(
                    &config.output,
                    &relative,
                    &mut file_acc,
                    &mut uncommitted_seq,
                )?;
                continue;
            }
            if remaining == Some(0) {
                drop(file_acc);
                wipe_uncommitted(&config.output)?;
                return finish_run(
                    service,
                    &config,
                    &mut checkpoint,
                    RunStatus::Limit,
                    jobs_current,
                    resumed,
                    &skip_variants,
                );
            }
            if over_soft {
                jobs_current = (jobs_current / 2).max(1);
                batch_size = (batch_size / 2).max(MIN_BATCH);
                lcs_inflight = (lcs_inflight / 2).max(1);
                line_sem.set_max(jobs_current);
                lcs_pool.resize(lcs_inflight);
                probe_jobs(&config, jobs_current);
                let spill_needed = resolved
                    .soft
                    .is_some_and(|soft| file_acc.estimated_bytes() > soft / 2)
                    || file_acc.estimated_bytes() > 0;
                if spill_needed {
                    spill_file_acc(
                        &config.output,
                        &relative,
                        &mut file_acc,
                        &mut uncommitted_seq,
                    )?;
                }
                below_soft_streak = 0;
            } else if resolved
                .soft
                .is_some_and(|soft| used < (soft as f64 * 0.60) as u64)
            {
                below_soft_streak += 1;
                if below_soft_streak >= 3 && jobs_current < config.jobs {
                    jobs_current += 1;
                    line_sem.set_max(jobs_current);
                    probe_jobs(&config, jobs_current);
                    below_soft_streak = 0;
                }
            } else {
                below_soft_streak = 0;
            }
            if let Some(peak) = &config.lcs_peak {
                peak.store(lcs_pool.peak(), Ordering::SeqCst);
            }
            if exhausted {
                break;
            }
        }
    }

    finish_run(
        service,
        &config,
        &mut checkpoint,
        RunStatus::Complete,
        jobs_current,
        resumed,
        &skip_variants,
    )
}

fn skip_variants_for(config: &RoundtripRunConfig) -> Result<HashSet<String>, String> {
    let mut skip = load_existing_synonym_variants(&default_segment_dict_root());
    if let Some(extra) = config.extra_correction.as_deref() {
        skip.extend(load_extra_correction_variants(extra)?);
    }
    Ok(skip)
}

fn rebuild_only(
    _service: &ConversionService,
    config: &RoundtripRunConfig,
) -> Result<RoundtripRunStatus, String> {
    let Some(checkpoint) = load_checkpoint(&config.output)? else {
        return Err("沒有檢查點可重建產出，請先跑語料或不要對空目錄 --rebuild-outputs。".into());
    };
    if checkpoint.shards.is_empty() {
        return Err("沒有已提交 shard 可重建產出。".into());
    }
    let skip_variants = skip_variants_for(config)?;
    let unique = write_outputs(_service, config, &checkpoint, &skip_variants)?;
    let mut result = status_from_checkpoint(
        &checkpoint,
        match checkpoint.status.as_str() {
            "interrupted" => RunStatus::Interrupted,
            "memory-hard" => RunStatus::MemoryHard,
            "limit" => RunStatus::Limit,
            _ => RunStatus::Complete,
        },
        config.jobs,
        true,
    );
    result.unique_raw_pairs = unique;
    Ok(result)
}

fn finish_run(
    service: &ConversionService,
    config: &RoundtripRunConfig,
    checkpoint: &mut Checkpoint,
    status: RunStatus,
    jobs_final: usize,
    resumed: bool,
    skip_variants: &HashSet<String>,
) -> Result<RoundtripRunStatus, String> {
    checkpoint.status = match status {
        RunStatus::Complete => "complete".into(),
        RunStatus::Interrupted => "interrupted".into(),
        RunStatus::MemoryHard => "memory-hard".into(),
        RunStatus::Limit => "limit".into(),
    };
    save_checkpoint(&config.output, checkpoint)?;
    let unique = write_outputs(service, config, checkpoint, skip_variants)?;
    let mut result = status_from_checkpoint(checkpoint, status, jobs_final, resumed);
    result.unique_raw_pairs = unique;
    Ok(result)
}

fn write_outputs(
    service: &ConversionService,
    config: &RoundtripRunConfig,
    checkpoint: &Checkpoint,
    skip_variants: &HashSet<String>,
) -> Result<usize, String> {
    let paths: Vec<PathBuf> = checkpoint
        .shards
        .iter()
        .map(|item| config.output.join(item))
        .collect();
    let (entries, stats) = finish_from_shards(
        &paths,
        config.min_count,
        config.min_dominance,
        skip_variants,
    )?;
    let report = RoundtripReport {
        lines_read: checkpoint.lines_read,
        lines_skipped: checkpoint.lines_skipped,
        lines_mismatched: checkpoint.lines_mismatched,
        raw_pair_occurrences: stats.raw_pair_occurrences,
        unique_raw_pairs: stats.unique_raw_pairs,
        kept_entries: entries.len(),
        kept_variants: stats.kept_variants,
        skipped_existing: stats.skipped_existing,
        skipped_low_count: stats.skipped_low_count,
        skipped_ambiguous: stats.skipped_ambiguous,
        files: checkpoint.completed_files.clone(),
        top_pairs: stats.top_pairs.clone(),
    };
    write_derived_outputs(&config.output, &entries, &stats, &report, &|word| {
        service.word_pos(word)
    })?;
    eprintln!(
        "已寫入產出：{} 行，保留 {} 個正字（{} 個異體）",
        checkpoint.lines_read,
        entries.len(),
        stats.kept_variants
    );
    Ok(stats.unique_raw_pairs)
}

fn durable_commit(
    config: &RoundtripRunConfig,
    checkpoint: &mut Checkpoint,
    relative: &str,
    file_acc: PairAggregator,
    file_lines: u64,
    file_skipped: u64,
    file_mismatched: u64,
    uncommitted_seq: &u32,
) -> Result<(), String> {
    let _ = uncommitted_seq;
    let mut pieces = list_uncommitted_for(&config.output, relative)?;
    if !file_acc.is_empty() {
        let ram_path = config.output.join("state/uncommitted").join(format!(
            "{}__ram.tmp.pairs",
            super::checkpoint::percent_encode_path(relative)
        ));
        file_acc.write_shard_path(&ram_path)?;
        drop(file_acc);
        pieces.push(ram_path);
    }
    let shard_rel = next_shard_relative(&checkpoint.shards);
    let shard_path = config.output.join(&shard_rel);
    if pieces.is_empty() {
        PairAggregator::default().write_shard_path(&shard_path)?;
    } else {
        merge_sorted_shard_files(&pieces, &shard_path)?;
    }
    checkpoint.completed_files.push(relative.to_string());
    checkpoint.lines_read += file_lines;
    checkpoint.lines_skipped += file_skipped;
    checkpoint.lines_mismatched += file_mismatched;
    checkpoint.shards.push(shard_rel);
    if checkpoint.shards.len() >= 8 {
        if let Ok(compacted) = compact_shards(&config.output, &checkpoint.shards) {
            if compacted.len() == 1 && compacted != checkpoint.shards {
                let old = checkpoint.shards.clone();
                checkpoint.shards = compacted;
                save_checkpoint(&config.output, checkpoint)?;
                for item in old {
                    if !checkpoint.shards.contains(&item) {
                        let _ = std::fs::remove_file(config.output.join(item));
                    }
                }
            } else {
                save_checkpoint(&config.output, checkpoint)?;
            }
        } else {
            save_checkpoint(&config.output, checkpoint)?;
        }
    } else {
        save_checkpoint(&config.output, checkpoint)?;
    }
    for piece in pieces {
        let _ = std::fs::remove_file(piece);
    }
    Ok(())
}

fn spill_file_acc(
    output: &Path,
    relative: &str,
    file_acc: &mut PairAggregator,
    seq: &mut u32,
) -> Result<(), String> {
    if file_acc.is_empty() {
        return Ok(());
    }
    let path = output
        .join("state/uncommitted")
        .join(uncommitted_name(relative, *seq));
    *seq += 1;
    file_acc.write_shard_path(&path)?;
    *file_acc = PairAggregator::default();
    Ok(())
}

struct BatchOutcome {
    aggregator: PairAggregator,
    lines_read: u64,
    lines_skipped: u64,
    lines_mismatched: u64,
}

impl BatchOutcome {
    fn merge(mut self, other: Self) -> Self {
        self.aggregator.merge(other.aggregator);
        self.lines_read += other.lines_read;
        self.lines_skipped += other.lines_skipped;
        self.lines_mismatched += other.lines_mismatched;
        self
    }
}

impl Default for BatchOutcome {
    fn default() -> Self {
        Self {
            aggregator: PairAggregator::default(),
            lines_read: 0,
            lines_skipped: 0,
            lines_mismatched: 0,
        }
    }
}

fn process_batch(
    pool: &rayon::ThreadPool,
    service: &ConversionService,
    batch: &[String],
    line_sem: &Arc<CountingSemaphore>,
    lcs_pool: &Arc<LcsPool>,
    config: &RoundtripRunConfig,
) -> BatchOutcome {
    pool.install(|| {
        batch
            .par_iter()
            .map(|line| {
                let _permit = line_sem.acquire();
                if let Some(counter) = &config.process_line_inflight {
                    let now = counter.fetch_add(1, Ordering::SeqCst) + 1;
                    if let Some(peak) = &config.process_line_peak {
                        peak.fetch_max(now, Ordering::SeqCst);
                    }
                }
                let mut guard = lcs_pool.acquire();
                let result = process_line_with_buf(service, line, Some(&mut guard));
                if let Some(counter) = &config.process_line_inflight {
                    counter.fetch_sub(1, Ordering::SeqCst);
                }
                if let Some(counter) = &config.lines_processed {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
                (line, result)
            })
            .fold(BatchOutcome::default, |mut acc, (line, result)| {
                acc.lines_read += 1;
                if result.skipped {
                    acc.lines_skipped += 1;
                } else if result.mismatched {
                    acc.lines_mismatched += 1;
                    let example = line.trim();
                    for (variant, canonical) in result.pairs {
                        acc.aggregator.add(variant, canonical, example);
                    }
                }
                acc
            })
            .reduce(BatchOutcome::default, BatchOutcome::merge)
    })
}

fn decorate_sample(
    mut sample: super::memory::MemorySample,
    file_acc: &PairAggregator,
    lcs_inflight: usize,
    batch_size: usize,
) -> super::memory::MemorySample {
    sample.aggregator_est = file_acc.estimated_bytes();
    sample.inflight_est = inflight_estimate(lcs_inflight, batch_size);
    let _ = aggregator_estimate(file_acc.unique_raw_pairs());
    sample
}

fn relative_path(file: &Path, sources: &Path) -> String {
    file.strip_prefix(sources)
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| file.display().to_string().replace('\\', "/"))
}

fn status_from_checkpoint(
    checkpoint: &Checkpoint,
    status: RunStatus,
    jobs_final: usize,
    resumed: bool,
) -> RoundtripRunStatus {
    RoundtripRunStatus {
        status,
        files_committed: checkpoint.completed_files.clone(),
        lines_read: checkpoint.lines_read,
        lines_skipped: checkpoint.lines_skipped,
        lines_mismatched: checkpoint.lines_mismatched,
        unique_raw_pairs: 0,
        last_hard_file: checkpoint.last_hard_file.clone(),
        jobs_final,
        resumed_from_checkpoint: resumed,
    }
}

fn probe_jobs(config: &RoundtripRunConfig, jobs_current: usize) {
    if let Some(probe) = &config.jobs_current_probe {
        probe.store(jobs_current, Ordering::SeqCst);
    }
}

fn fmt_mb(bytes: Option<u64>) -> String {
    match bytes {
        Some(0) | None => "off".into(),
        Some(value) => format!("{}MiB", value / 1024 / 1024),
    }
}

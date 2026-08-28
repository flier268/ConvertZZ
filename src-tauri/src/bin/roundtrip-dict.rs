use convertzz_lib::roundtrip_dict::{
    assert_output_outside_package_data, assert_output_outside_sources, corpus_files,
    default_segment_dict_root, format_pairs_tsv, format_segment_dict, format_synonym_file,
    load_existing_synonym_variants, process_line, PairAggregator, RoundtripReport,
};
use convertzz_lib::ConversionService;
use rayon::prelude::*;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

const BATCH_SIZE: usize = 256;
const PROGRESS_EVERY: u64 = 10_000;

struct Args {
    sources: PathBuf,
    output: PathBuf,
    min_count: u64,
    min_dominance: f64,
    limit: Option<u64>,
    jobs: usize,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    assert_output_outside_sources(&args.output, &args.sources)?;
    assert_output_outside_package_data(&args.output)?;
    if args.output.exists() && args.output.is_file() {
        return Err("輸出路徑必須是目錄。".into());
    }

    let files = corpus_files(&args.sources)?;
    eprintln!(
        "語料來源（只讀）：{}\n檔案數：{}\n執行緒：{}",
        args.sources.display(),
        files.len(),
        args.jobs
    );
    for path in &files {
        if !path.starts_with(&args.sources) {
            return Err(format!("拒絕讀取來源目錄以外的檔案：{}", path.display()));
        }
    }

    fs::create_dir_all(&args.output).map_err(|error| format!("無法建立輸出目錄：{error}"))?;

    let service = Arc::new(
        ConversionService::without_extra_correction(None)
            .map_err(|error| format!("無法初始化轉換核心：{error}"))?,
    );
    let skip_variants = load_existing_synonym_variants(&default_segment_dict_root());
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(args.jobs)
        .build()
        .map_err(|error| format!("無法建立執行緒池：{error}"))?;

    let started = Instant::now();
    let mut aggregator = PairAggregator::default();
    let mut lines_read = 0u64;
    let mut lines_skipped = 0u64;
    let mut lines_mismatched = 0u64;
    let mut reached_limit = false;
    let mut processed_files = Vec::new();

    for file in &files {
        if reached_limit {
            break;
        }
        let relative = file
            .strip_prefix(&args.sources)
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| file.display().to_string());
        processed_files.push(relative);
        eprintln!("讀取 {}", file.display());
        let iterator = convertzz_lib::roundtrip_dict::read_text_lines(file)?;
        let mut batch = Vec::with_capacity(BATCH_SIZE);
        for line in iterator {
            let line = line?;
            batch.push(line);
            if batch.len() >= BATCH_SIZE {
                process_batch(
                    &pool,
                    &service,
                    &batch,
                    args.limit,
                    &mut aggregator,
                    &mut lines_read,
                    &mut lines_skipped,
                    &mut lines_mismatched,
                    &mut reached_limit,
                    started,
                );
                batch.clear();
                if reached_limit {
                    break;
                }
            }
        }
        if !batch.is_empty() && !reached_limit {
            process_batch(
                &pool,
                &service,
                &batch,
                args.limit,
                &mut aggregator,
                &mut lines_read,
                &mut lines_skipped,
                &mut lines_mismatched,
                &mut reached_limit,
                started,
            );
        }
        write_snapshot(
            &args,
            &aggregator,
            &skip_variants,
            lines_read,
            lines_skipped,
            lines_mismatched,
            &processed_files,
        )?;
    }

    eprintln!(
        "完成。讀取 {} 行，回環差異 {} 行。\n輸出目錄：{}\n耗時 {:.1}s",
        lines_read,
        lines_mismatched,
        args.output.display(),
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

fn write_snapshot(
    args: &Args,
    aggregator: &PairAggregator,
    skip_variants: &HashSet<String>,
    lines_read: u64,
    lines_skipped: u64,
    lines_mismatched: u64,
    processed_files: &[String],
) -> Result<(), String> {
    let raw_occurrences = aggregator.raw_occurrences();
    let unique_raw_pairs = aggregator.unique_raw_pairs();
    let (entries, finish) =
        aggregator
            .clone()
            .finish(args.min_count, args.min_dominance, skip_variants);
    let report = RoundtripReport {
        lines_read,
        lines_skipped,
        lines_mismatched,
        raw_pair_occurrences: raw_occurrences,
        unique_raw_pairs,
        kept_entries: entries.len(),
        kept_variants: finish.kept_variants,
        skipped_existing: finish.skipped_existing,
        skipped_low_count: finish.skipped_low_count,
        skipped_ambiguous: finish.skipped_ambiguous,
        files: processed_files.to_vec(),
        top_pairs: finish.top_pairs.iter().take(50).cloned().collect(),
    };
    write_output(
        &args.output,
        "zht.corpus.synonym.txt",
        &format_synonym_file(&entries),
    )?;
    write_output(
        &args.output,
        "zht.corpus.dict.txt",
        &format_segment_dict(&entries),
    )?;
    write_output(
        &args.output,
        "pairs.tsv",
        &format_pairs_tsv(&finish.top_pairs),
    )?;
    let report_text = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("無法序列化報告：{error}"))?;
    write_output(&args.output, "report.json", &format!("{report_text}\n"))?;
    eprintln!(
        "已寫入快照：{} 行，保留 {} 個正字（{} 個異體）",
        lines_read,
        entries.len(),
        finish.kept_variants
    );
    Ok(())
}

fn process_batch(
    pool: &rayon::ThreadPool,
    service: &Arc<ConversionService>,
    batch: &[String],
    limit: Option<u64>,
    aggregator: &mut PairAggregator,
    lines_read: &mut u64,
    lines_skipped: &mut u64,
    lines_mismatched: &mut u64,
    reached_limit: &mut bool,
    started: Instant,
) {
    let remaining = limit.map(|max| max.saturating_sub(*lines_read));
    let slice = match remaining {
        Some(0) => {
            *reached_limit = true;
            return;
        }
        Some(max) if max < batch.len() as u64 => {
            *reached_limit = true;
            &batch[..max as usize]
        }
        _ => batch,
    };

    let service = Arc::clone(service);
    let outcome = pool.install(|| {
        slice
            .par_iter()
            .map(|line| {
                let result = process_line(service.as_ref(), line);
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
    });

    *lines_read += outcome.lines_read;
    *lines_skipped += outcome.lines_skipped;
    *lines_mismatched += outcome.lines_mismatched;
    aggregator.merge(outcome.aggregator);

    if *lines_read % PROGRESS_EVERY < slice.len() as u64 {
        eprintln!(
            "已處理 {} 行，回環差異 {}，候選 {}，{:.1}s",
            *lines_read,
            *lines_mismatched,
            aggregator.unique_raw_pairs(),
            started.elapsed().as_secs_f64()
        );
    }
}

#[derive(Default)]
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

fn write_output(directory: &Path, name: &str, contents: &str) -> Result<(), String> {
    let path = directory.join(name);
    fs::write(&path, contents).map_err(|error| format!("寫入 {} 失敗：{error}", path.display()))
}

fn parse_args() -> Result<Args, String> {
    let mut sources = None;
    let mut output = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/roundtrip-correction");
    let mut min_count = 5u64;
    let mut min_dominance = 0.7f64;
    let mut limit = None;
    let mut jobs = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(4);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "--sources" => sources = Some(required_path(&mut args, "--sources")?),
            "--output" => output = required_path(&mut args, "--output")?,
            "--min-count" => min_count = required_u64(&mut args, "--min-count")?,
            "--min-dominance" => min_dominance = required_f64(&mut args, "--min-dominance")?,
            "--limit" => limit = Some(required_u64(&mut args, "--limit")?),
            "--jobs" => jobs = required_u64(&mut args, "--jobs")? as usize,
            other => return Err(format!("未知參數：{other}\n使用 --help 查看用法。")),
        }
    }
    let sources = sources.ok_or_else(|| "必須指定 --sources DIR。".to_string())?;
    if jobs == 0 {
        return Err("--jobs 必須大於 0。".into());
    }
    if !(0.5..=1.0).contains(&min_dominance) {
        return Err("--min-dominance 必須介於 0.5 與 1.0。".into());
    }
    Ok(Args {
        sources,
        output,
        min_count,
        min_dominance,
        limit,
        jobs,
    })
}

fn required_path(args: &mut impl Iterator<Item = String>, name: &str) -> Result<PathBuf, String> {
    args.next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("{name} 需要路徑。"))
}

fn required_u64(args: &mut impl Iterator<Item = String>, name: &str) -> Result<u64, String> {
    let value = args.next().ok_or_else(|| format!("{name} 需要數值。"))?;
    value
        .parse()
        .map_err(|_| format!("{name} 不是有效的整數：{value}"))
}

fn required_f64(args: &mut impl Iterator<Item = String>, name: &str) -> Result<f64, String> {
    let value = args.next().ok_or_else(|| format!("{name} 需要數值。"))?;
    value
        .parse()
        .map_err(|_| format!("{name} 不是有效的浮點數：{value}"))
}

fn print_help() {
    println!(
        "\
roundtrip-dict — 以套件分詞／簡轉繁做回環，產出 ConvertZZ 額外修正詞典

來源語料只讀。產出是套件外的額外層，不可寫入 segment-dict。

用法：
  cargo run --manifest-path src-tauri/Cargo.toml --bin roundtrip-dict -- --sources DIR [選項]

選項：
  --sources DIR         語料根目錄（必填，只讀）
  --output DIR          輸出目錄（預設：data/roundtrip-correction）
  --min-count N         異體最少出現次數（預設：5）
  --min-dominance F     主對應佔比下限 0.5–1.0（預設：0.7）
  --limit N             最多處理行數（測試用）
  --jobs N              執行緒數
  -h, --help            顯示說明

輸出（ConvertZZ 額外修正，不寫入套件字典）：
  zht.corpus.synonym.txt   分詞後同義詞（正字,錯字）
  zht.corpus.dict.txt      額外分詞表
  pairs.tsv                對應次數與例句
  report.json              統計

套用時放到 extra-correction（與 segment-dict 同層、分開的目錄），不要併入 synonym.txt。"
    );
}

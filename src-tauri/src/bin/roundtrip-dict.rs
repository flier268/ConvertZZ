use convertzz_lib::roundtrip_dict::{
    default_sampler, run_roundtrip, CorpusSelect, MemoryPolicy, RoundtripRunConfig, RunStatus,
};
use convertzz_lib::ConversionService;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

struct Args {
    sources: PathBuf,
    output: PathBuf,
    select: CorpusSelect,
    min_count: u64,
    min_dominance: f64,
    limit: Option<u64>,
    jobs: usize,
    batch_size: usize,
    memory: MemoryPolicy,
    reset: bool,
    rebuild_outputs_only: bool,
    extra_correction: Option<PathBuf>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let stop = Arc::new(AtomicBool::new(false));
    let sigint = Arc::new(AtomicBool::new(false));
    let sigterm = Arc::new(AtomicBool::new(false));
    let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&stop));
    let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&sigint));
    let _ = signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&stop));
    let _ = signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&sigterm));

    let service = ConversionService::without_extra_correction(None)
        .map_err(|error| format!("無法初始化轉換核心：{error}"))?;
    let status = run_roundtrip(
        &service,
        RoundtripRunConfig {
            sources: args.sources,
            output: args.output.clone(),
            select: args.select,
            min_count: args.min_count,
            min_dominance: args.min_dominance,
            limit: args.limit,
            jobs: args.jobs,
            batch_size: args.batch_size,
            memory: args.memory,
            reset: args.reset,
            rebuild_outputs_only: args.rebuild_outputs_only,
            extra_correction: args.extra_correction.clone(),
            stop: Arc::clone(&stop),
            sampler: default_sampler(),
            lines_processed: None,
            files_opened: None,
            process_line_inflight: None,
            process_line_peak: None,
            jobs_current_probe: None,
            lcs_peak: None,
        },
    )?;
    eprintln!(
        "結束（{:?}）。已提交 {} 個檔，讀取 {} 行。\n輸出目錄：{}",
        status.status,
        status.files_committed.len(),
        status.lines_read,
        args.output.display()
    );
    std::process::exit(exit_code(
        &status.status,
        sigint.load(Ordering::SeqCst),
        sigterm.load(Ordering::SeqCst),
    ));
}

fn exit_code(status: &RunStatus, sigint: bool, sigterm: bool) -> i32 {
    match status {
        RunStatus::Complete | RunStatus::Limit => 0,
        RunStatus::MemoryHard => 2,
        RunStatus::Interrupted => {
            if cfg!(unix) {
                if sigterm && !sigint {
                    143
                } else {
                    130
                }
            } else {
                2
            }
        }
    }
}

fn parse_args() -> Result<Args, String> {
    let mut sources = None;
    let mut output = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/roundtrip-correction");
    let mut select = CorpusSelect::default();
    let mut min_count = 5u64;
    let mut min_dominance = 0.7f64;
    let mut limit = None;
    let mut jobs = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(4);
    let mut batch_size = 4096usize;
    let mut memory = MemoryPolicy {
        soft_bytes: None,
        hard_bytes: None,
        lcs_inflight: None,
    };
    let mut reset = false;
    let mut rebuild_outputs_only = false;
    let mut extra_correction = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "--sources" => sources = Some(required_path(&mut args, "--sources")?),
            "--output" => output = required_path(&mut args, "--output")?,
            "--include" => push_names(&mut select.include, &mut args, "--include")?,
            "--exclude" => push_names(&mut select.exclude, &mut args, "--exclude")?,
            "--min-count" => min_count = required_u64(&mut args, "--min-count")?,
            "--min-dominance" => min_dominance = required_f64(&mut args, "--min-dominance")?,
            "--limit" => limit = Some(required_u64(&mut args, "--limit")?),
            "--jobs" => jobs = required_u64(&mut args, "--jobs")? as usize,
            "--batch-size" => batch_size = required_u64(&mut args, "--batch-size")? as usize,
            "--memory-soft-mb" => {
                let mb = required_u64(&mut args, "--memory-soft-mb")?;
                memory.soft_bytes = Some(mb.saturating_mul(1024 * 1024));
            }
            "--memory-hard-mb" => {
                let mb = required_u64(&mut args, "--memory-hard-mb")?;
                memory.hard_bytes = Some(mb.saturating_mul(1024 * 1024));
            }
            "--lcs-inflight" => {
                memory.lcs_inflight = Some(required_u64(&mut args, "--lcs-inflight")? as usize);
            }
            "--reset" => reset = true,
            "--rebuild-outputs" => rebuild_outputs_only = true,
            "--extra-correction" => {
                extra_correction = Some(required_path(&mut args, "--extra-correction")?)
            }
            other => return Err(format!("未知參數：{other}\n使用 --help 查看用法。")),
        }
    }
    let sources = sources.ok_or_else(|| "必須指定 --sources DIR。".to_string())?;
    if jobs == 0 {
        return Err("--jobs 必須大於 0。".into());
    }
    if batch_size < 64 {
        return Err("--batch-size 必須 ≥ 64。".into());
    }
    if !(0.5..=1.0).contains(&min_dominance) {
        return Err("--min-dominance 必須介於 0.5 與 1.0。".into());
    }
    if reset && rebuild_outputs_only {
        return Err("--reset 與 --rebuild-outputs 不可同時使用。".into());
    }
    if let (Some(soft), Some(hard)) = (memory.soft_bytes, memory.hard_bytes) {
        if soft > 0 && hard > 0 && soft > hard {
            return Err("軟水位不可大於硬水位。".into());
        }
    }
    Ok(Args {
        sources,
        output,
        select,
        min_count,
        min_dominance,
        limit,
        jobs,
        batch_size,
        memory,
        reset,
        rebuild_outputs_only,
        extra_correction,
    })
}

fn push_names(
    target: &mut Vec<String>,
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<(), String> {
    let value = args.next().ok_or_else(|| format!("{flag} 需要名稱。"))?;
    let mut parsed = 0;
    for name in value.split(',') {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        if name.contains('/') || name.contains('\\') {
            return Err(format!("{flag} 只接受頂層目錄名稱，不可含路徑分隔。"));
        }
        parsed += 1;
        if !target.iter().any(|item| item == name) {
            target.push(name.to_string());
        }
    }
    if parsed == 0 {
        return Err(format!("{flag} 需要名稱。"));
    }
    Ok(())
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

來源語料只讀。產出是套件外的額外層，不可寫入 segment-dict 或 extra-correction。
掃描整份語料是 Θ(行數)；不能用抽樣把時間變成 O(log N)。高 RAM 來自 unique pair，
用檔級檢查點與溢寫處理，不是改對齊演算法。

用法：
  cargo run --manifest-path src-tauri/Cargo.toml --bin roundtrip-dict -- --sources DIR [選項]

選項：
  --sources DIR         語料根目錄（必填，只讀）
  --output DIR          輸出目錄（預設：data/roundtrip-correction）
  --include NAME        只讀這些頂層目錄，可重複或逗號分隔
  --exclude NAME        略過這些頂層目錄，可重複或逗號分隔
  --min-count N         異體最少出現次數（預設：5）
  --min-dominance F     主對應佔比下限 0.5–1.0（預設：0.7）
  --limit N             最多處理行數（測試用；半檔不提交）
  --jobs N              執行緒數上限
  --batch-size N        批次行數上限（預設 4096，下限 64）
  --memory-soft-mb N    軟水位 MiB；0=關；省略=auto
  --memory-hard-mb N    硬水位 MiB；0=關；省略=auto
  --lcs-inflight N      同時 LCS 數；省略=auto
  --reset               刪除檢查點、state/ 與衍生四檔後從頭跑
  --rebuild-outputs     只從已提交 shard 重寫衍生檔
  --extra-correction DIR
                        探針：聚合時略過該目錄 zht.corpus.synonym.txt 的錯詞。
                        回環引擎仍用套件基線，避免自己修正自己。
                        此模式產出是 QA 清單，不要複製進 extra-correction。
  -h, --help            顯示說明

退出碼：
  0    完成，或 --limit 正常結束，或 complete 再跑略過
  1    參數錯誤、fingerprint 不符、rebuild 無資料、基線 ≥ hard、IO／損壞 shard
  2    硬水位停下；Windows 上 interrupted 也可能是 2
  130  Unix SIGINT
  143  Unix SIGTERM

輸出（ConvertZZ 額外修正，不寫入套件字典；不要 cp -r 整個目錄到 extra-correction）：
  zht.corpus.synonym.txt   同義詞（正字,錯字|詞性）
  zht.corpus.dict.txt      額外分詞表（詞|詞性|權值，含簡繁詞形）
  pairs.tsv                對應次數與例句
  report.json              統計
  checkpoint.json / state/ 檔級復原（勿套用）

套用時只複製兩個 zht.corpus.* 到 extra-correction。"
    );
}

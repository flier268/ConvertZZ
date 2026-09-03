use convertzz_lib::roundtrip_dict::{
    audit_synonym_orientation, default_extra_correction_root, default_sampler,
    format_orientation_report, merge_extra_correction, resolve_synonym_path, run_roundtrip,
    CorpusSelect, MemoryPolicy, RoundtripRunConfig, RunStatus,
};
use convertzz_lib::ConversionService;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

enum Command {
    Roundtrip(RoundtripArgs),
    CheckOrientation(OrientationArgs),
    MergeExtra(MergeExtraArgs),
}

struct RoundtripArgs {
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

struct OrientationArgs {
    synonym: PathBuf,
    output: Option<PathBuf>,
    full: bool,
}

struct MergeExtraArgs {
    from: PathBuf,
    into: PathBuf,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    match parse_args()? {
        Command::Roundtrip(args) => run_roundtrip_command(args),
        Command::CheckOrientation(args) => run_orientation_command(args),
        Command::MergeExtra(args) => run_merge_extra_command(args),
    }
}

fn run_roundtrip_command(args: RoundtripArgs) -> Result<(), String> {
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
        "結束（{:?}）。已提交 {} 個檔，讀取 {} 行。\n輸出目錄：{}\n同義詞導向：min={}／full={} 筆可疑",
        status.status,
        status.files_committed.len(),
        status.lines_read,
        args.output.display(),
        status.orientation_min_hits,
        status.orientation_full_hits
    );
    std::process::exit(exit_code(
        &status.status,
        sigint.load(Ordering::SeqCst),
        sigterm.load(Ordering::SeqCst),
    ));
}

fn run_orientation_command(args: OrientationArgs) -> Result<(), String> {
    let synonym = resolve_synonym_path(&args.synonym)?;
    let report = audit_synonym_orientation(&synonym, args.full)?;
    let text = format_orientation_report(&report);
    if let Some(output) = args.output.as_ref() {
        if let Some(parent) = output.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("無法建立輸出目錄 {}：{error}", parent.display()))?;
            }
        }
        std::fs::write(output, text.as_bytes())
            .map_err(|error| format!("無法寫入 {}：{error}", output.display()))?;
        eprintln!(
            "已掃描 {} 筆，疑似左簡右繁 {} 筆。\n報告：{}",
            report.entries_scanned,
            report.hits.len(),
            output.display()
        );
    } else {
        print!("{text}");
        eprintln!(
            "已掃描 {} 筆，疑似左簡右繁 {} 筆。",
            report.entries_scanned,
            report.hits.len()
        );
    }
    if report.hits.is_empty() {
        Ok(())
    } else {
        std::process::exit(2);
    }
}

fn run_merge_extra_command(args: MergeExtraArgs) -> Result<(), String> {
    let stats = merge_extra_correction(&args.from, &args.into)?;
    eprintln!(
        "已合併進 {}。同義詞保留 {}、新增 {} 筆（+{} 個錯詞）；分詞表保留 {}、新增 {} 列。",
        args.into.display(),
        stats.synonym_entries_kept,
        stats.synonym_entries_added,
        stats.synonym_variants_added,
        stats.dict_rows_kept,
        stats.dict_rows_added
    );
    Ok(())
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

fn parse_args() -> Result<Command, String> {
    let mut argv = std::env::args().skip(1).peekable();
    match argv.peek().map(String::as_str) {
        Some("-h") | Some("--help") => {
            print_help();
            std::process::exit(0);
        }
        Some("check-synonym-orientation") => {
            argv.next();
            Ok(Command::CheckOrientation(parse_orientation_args(&mut argv)?))
        }
        Some("merge-extra") => {
            argv.next();
            Ok(Command::MergeExtra(parse_merge_extra_args(&mut argv)?))
        }
        _ => Ok(Command::Roundtrip(parse_roundtrip_args(&mut argv)?)),
    }
}

fn parse_orientation_args(
    args: &mut impl Iterator<Item = String>,
) -> Result<OrientationArgs, String> {
    let mut synonym = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../resources/extra-correction/zht.corpus.synonym.txt");
    let mut output = None;
    let mut full = false;
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_orientation_help();
                std::process::exit(0);
            }
            "--synonym" => synonym = required_path(&mut args, "--synonym")?,
            "--output" => output = Some(required_path(&mut args, "--output")?),
            "--full" => full = true,
            other => {
                return Err(format!(
                    "未知參數：{other}\n使用 check-synonym-orientation --help 查看用法。"
                ))
            }
        }
    }
    Ok(OrientationArgs {
        synonym,
        output,
        full,
    })
}

fn parse_merge_extra_args(
    args: &mut impl Iterator<Item = String>,
) -> Result<MergeExtraArgs, String> {
    let mut from =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/roundtrip-correction");
    let mut into = default_extra_correction_root();
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_merge_extra_help();
                std::process::exit(0);
            }
            "--from" => from = required_path(&mut args, "--from")?,
            "--into" => into = required_path(&mut args, "--into")?,
            other => {
                return Err(format!(
                    "未知參數：{other}\n使用 merge-extra --help 查看用法。"
                ))
            }
        }
    }
    Ok(MergeExtraArgs { from, into })
}

fn parse_roundtrip_args(args: &mut impl Iterator<Item = String>) -> Result<RoundtripArgs, String> {
    let mut sources = None;
    let mut output =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/roundtrip-correction");
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
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "--sources" => sources = Some(required_path(args, "--sources")?),
            "--output" => output = required_path(args, "--output")?,
            "--include" => push_names(&mut select.include, args, "--include")?,
            "--exclude" => push_names(&mut select.exclude, args, "--exclude")?,
            "--min-count" => min_count = required_u64(args, "--min-count")?,
            "--min-dominance" => min_dominance = required_f64(args, "--min-dominance")?,
            "--limit" => limit = Some(required_u64(args, "--limit")?),
            "--jobs" => jobs = required_u64(args, "--jobs")? as usize,
            "--batch-size" => batch_size = required_u64(args, "--batch-size")? as usize,
            "--memory-soft-mb" => {
                let mb = required_u64(args, "--memory-soft-mb")?;
                memory.soft_bytes = Some(mb.saturating_mul(1024 * 1024));
            }
            "--memory-hard-mb" => {
                let mb = required_u64(args, "--memory-hard-mb")?;
                memory.hard_bytes = Some(mb.saturating_mul(1024 * 1024));
            }
            "--lcs-inflight" => {
                memory.lcs_inflight = Some(required_u64(args, "--lcs-inflight")? as usize);
            }
            "--reset" => reset = true,
            "--rebuild-outputs" => rebuild_outputs_only = true,
            "--extra-correction" => {
                extra_correction = Some(required_path(args, "--extra-correction")?)
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
    Ok(RoundtripArgs {
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
  cargo run --manifest-path src-tauri/Cargo.toml -p roundtrip-dict -- --sources DIR [選項]
  cargo run --manifest-path src-tauri/Cargo.toml -p roundtrip-dict -- check-synonym-orientation [選項]
  cargo run --manifest-path src-tauri/Cargo.toml -p roundtrip-dict -- merge-extra [選項]

子命令：
  check-synonym-orientation
                        找出同義詞檔疑似「左邊簡體、右邊繁體」的條目
                        （詳見該子命令 --help）
  merge-extra           把產出的 zht.corpus.* 合併進 extra-correction，不整包覆蓋
                        （詳見該子命令 --help）

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
                        完整跑完後若只改少數語料檔，不必 --reset：會沿用未變更檔
                        的 shard，只重算變更檔並重寫衍生檔。套件詞典、
                        extra-correction、--include／--exclude 變更仍需 --reset。
                        已壓實成單一 committed.pairs 的舊檢查點無法增量，請 --reset 一次。
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
  synonym-orientation-min.tsv
                           自動檢查：疑似左簡右繁（cn2tw_min，優先處理）
  synonym-orientation-full.tsv
                           自動檢查：擴大召回（含一簡多繁，需人工覆核）
  checkpoint.json / state/ 檔級復原（勿套用）

寫入產出後會自動跑導向檢查；回環成功時即使有可疑條目仍 exit 0。
要單獨重跑檢查或讓可疑變成非零退出，用子命令 check-synonym-orientation。

套用請用 merge-extra 合併兩個 zht.corpus.*，不要 cp 整包覆蓋 extra-correction。"
    );
}

fn print_orientation_help() {
    println!(
        "\
roundtrip-dict check-synonym-orientation — 找出疑似左簡右繁的同義詞

契約：正字,錯字|詞性（台灣常用寫法在左）。`一个,一個` 應寫成 `一個,一个`。

預設只用 cn2tw_min（與引擎字形層相同），精準度高。
加 --full 會一併用完整 cn2tw／tw2cn，召回 于／志／范 等一簡多繁，但也可能把
`制度,製度` 這類合法回環保護列進來，只供人工覆核。

用法：
  cargo run --manifest-path src-tauri/Cargo.toml -p roundtrip-dict -- \\
    check-synonym-orientation [--synonym PATH] [--output FILE] [--full]

選項：
  --synonym PATH   同義詞檔，或含 zht.corpus.synonym.txt 的目錄
                   （預設：src-tauri/resources/extra-correction/zht.corpus.synonym.txt）
  --output FILE    寫入報告；省略則印到 stdout
  --full           擴大召回（含一簡多繁，需人工覆核）
  -h, --help       顯示說明

退出碼：
  0    無可疑條目
  1    參數或讀檔錯誤
  2    有可疑條目

報告欄位：line confidence reason canonical variant suggested_flip raw"
    );
}

fn print_merge_extra_help() {
    println!(
        "\
roundtrip-dict merge-extra — 把回環產出合併進 extra-correction

不整包覆蓋。既有正字優先（不會把 機制,機製 翻成 機製,機制）。
新的 2 字詞對（本里,本裡）與分詞列才追加。固定保護詞來自 conversion-specials（`pin`／`word=`／`place-names.txt`：和牛、本里、里辦、里民、里長、里名、胜肽、三星鄉、莊敬里），會寫回分詞表。

用法：
  cargo run --manifest-path src-tauri/Cargo.toml -p roundtrip-dict -- \\
    merge-extra [--from DIR] [--into DIR]

選項：
  --from DIR   roundtrip 產出目錄（預設：data/roundtrip-correction）
  --into DIR   extra-correction 目錄（預設：src-tauri/resources/extra-correction）
  -h, --help   顯示說明

來源必須含 zht.corpus.synonym.txt 與 zht.corpus.dict.txt。
不要把 --from 指到 extra-correction，也不要把 --into 指到套件字典。"
    );
}

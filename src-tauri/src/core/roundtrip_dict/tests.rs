use super::*;
use crate::core::conversion::{shared_conversion, ConversionService};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

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
fn extract_pairs_glues_single_char_neighbor_into_compound() {
    let pairs = extract_pairs(
        &tokens(&["本", "里", "垃圾車"]),
        &tokens(&["本", "裡", "垃圾車"]),
    );
    assert!(
        pairs.contains(&("本裡".into(), "本里".into())),
        "本+里 should become 本里/本裡, got {pairs:?}"
    );
    assert!(
        !pairs
            .iter()
            .any(|(variant, canonical)| variant.chars().count() == 1
                || canonical.chars().count() == 1),
        "must not keep 里/裡 as a 1-char pair: {pairs:?}"
    );
    assert!(
        !pairs
            .iter()
            .any(|(variant, _)| variant.contains("垃圾車") || variant.contains("垃圾")),
        "must not glue 里+垃圾車: {pairs:?}"
    );
}

#[test]
fn extract_pairs_glues_following_single_char_neighbor() {
    let pairs = extract_pairs(
        &tokens(&["聯繫", "里", "辦"]),
        &tokens(&["聯繫", "裡", "辦"]),
    );
    assert!(
        pairs.contains(&("裡辦".into(), "里辦".into())),
        "里+辦 should become 里辦/裡辦, got {pairs:?}"
    );
    assert!(
        !pairs.iter().any(|(variant, _)| variant.contains("聯繫")),
        "must not glue 2-char 聯繫+里: {pairs:?}"
    );
}

#[test]
fn extract_pairs_does_not_glue_multichar_locative_neighbor() {
    let pairs = extract_pairs(&tokens(&["房子", "里"]), &tokens(&["房子", "裡"]));
    assert!(
        pairs.is_empty(),
        "房子+里 is locative 2+1, must not learn 房子里: {pairs:?}"
    );
}

#[test]
fn extract_pairs_does_not_emit_character_replace_across_word_boundary() {
    let pairs = extract_pairs(&tokens(&["皇后", "裡面"]), &tokens(&["皇后", "裏面"]));
    assert_eq!(pairs, vec![("裏面".into(), "裡面".into())]);
    assert!(!pairs.iter().any(|(variant, _)| variant.contains("皇后")));
}

#[test]
fn extract_pairs_requires_reconstructed_token_boundaries() {
    // 「四|隻有」對「四只|有」若只按原詞字數切片，會假造「只有↔隻有」。
    let pairs = extract_pairs(&tokens(&["四", "隻有"]), &tokens(&["四只", "有"]));
    assert!(
        !pairs
            .iter()
            .any(|(variant, canonical)| variant == "只有" || canonical == "隻有"),
        "cross-token phantom pair: {pairs:?}"
    );
}

#[test]
fn extract_pairs_keeps_joined_reconstructed_tokens() {
    // 原詞「裡面」對重建「裏|面」仍應合併成「裏面」。
    let pairs = extract_pairs(&tokens(&["裡面"]), &tokens(&["裏", "面"]));
    assert_eq!(pairs, vec![("裏面".into(), "裡面".into())]);
}

#[test]
fn process_line_does_not_learn_zhiyou_from_measure_boundary() {
    let service = test_service();
    let misseg = process_line(&service, "對面五隻裡面四隻有CC");
    assert!(
        !misseg.pairs.iter().any(|(variant, canonical)| {
            (variant == "只有" && canonical == "隻有") || (variant == "隻有" && canonical == "只有")
        }),
        "unexpected 只有/隻有 pair: {:?}",
        misseg.pairs
    );

    let measure = process_line(&service, "七隻小狗");
    assert!(
        measure
            .pairs
            .iter()
            .any(|(variant, canonical)| variant == "七只" && canonical == "七隻"),
        "measure-word 七隻 should still be protected: {:?}",
        measure.pairs
    );

    let clean = process_line(&service, "我只有一本書");
    assert!(
        clean.pairs.is_empty(),
        "identity roundtrip should not emit pairs: {:?}",
        clean.pairs
    );
    let both = process_line(&service, "觸發機制");
    assert!(
        both.originals.iter().any(|word| word == "機制"),
        "identity 機制 must be attested as original: {:?}",
        both.originals
    );
}

#[test]
fn process_line_learns_split_neighborhood_li_and_jieju() {
    let service = test_service();
    let neighborhood = process_line(&service, "關於本里垃圾車時間，請聯繫里辦協助。");
    assert!(
        neighborhood.originals.iter().any(|word| word == "本里"),
        "specials pin 本里 as one token: {:?}",
        neighborhood.originals
    );
    assert!(
        neighborhood.originals.iter().any(|word| word == "里辦"),
        "specials pin 里辦 as one token: {:?}",
        neighborhood.originals
    );
    let places = process_line(&service, "宜蘭縣三星鄉公所通知莊敬里里民");
    assert!(
        places.originals.iter().any(|word| word == "三星鄉"),
        "place-names pin 三星鄉 as one token: {:?}",
        places.originals
    );
    assert!(
        places.originals.iter().any(|word| word == "莊敬里"),
        "place-names pin 莊敬里 as one token: {:?}",
        places.originals
    );
    assert!(
        !places
            .originals
            .iter()
            .any(|word| word == "鄉" || word == "里"),
        "must not split xx鄉／xx里: {:?}",
        places.originals
    );
    let jieju = process_line(&service, "讓玩家在後期感到拮据");
    assert!(
        jieju
            .pairs
            .iter()
            .any(|(variant, canonical)| variant == "拮據" && canonical == "拮据"),
        "拮据 is already one token and must be learned: {:?}",
        jieju.pairs
    );
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
fn aggregator_drops_variant_that_is_also_an_original_word() {
    let mut aggregator = PairAggregator::default();
    aggregator.add("機制".into(), "機製".into(), "觸發機製");
    for _ in 0..3 {
        aggregator.note_original("機制");
        aggregator.note_original("機製");
    }
    let skip = HashSet::new();
    let (entries, stats) = aggregator.finish(1, 0.7, &skip);
    assert!(
        entries.is_empty(),
        "機制 and 機製 are both corpus words: {entries:?}"
    );
    assert_eq!(stats.skipped_ambiguous, 1);
}

#[test]
fn aggregator_keeps_engine_only_variant() {
    let mut aggregator = PairAggregator::default();
    aggregator.add("裏面".into(), "裡面".into(), "冰箱裡面");
    aggregator.note_original("裡面");
    let skip = HashSet::new();
    let (entries, _) = aggregator.finish(1, 0.7, &skip);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].canonical, "裡面");
    assert_eq!(entries[0].variants, vec![("裏面".into(), 1)]);
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
fn finish_borrow_is_idempotent() {
    let mut aggregator = PairAggregator::default();
    for _ in 0..5 {
        aggregator.add("裏面".into(), "裡面".into(), "冰箱裡面");
    }
    let skip = HashSet::new();
    let (first, _) = aggregator.finish(1, 0.7, &skip);
    let (second, _) = aggregator.finish(1, 0.7, &skip);
    assert_eq!(first, second);
}

#[test]
fn synonym_format_is_canonical_then_variants() {
    let text = format_synonym_file(
        &[CorrectionEntry {
            canonical: "裡面".into(),
            variants: vec![("裏面".into(), 4), ("里边".into(), 2)],
        }],
        &|word| {
            if word == "裡面" {
                novel_segment::POSTAG::D_F
            } else {
                0
            }
        },
    );
    assert!(text.contains("裡面,裏面,里边|D_F\n"), "{text}");
    assert!(text.contains("正字,錯字|詞性"));
}

#[test]
fn parse_synonym_line_skips_comments() {
    assert!(parse_synonym_line("// comment").is_none());
    assert_eq!(
        parse_synonym_line("裡面,裏面"),
        Some(("裡面".into(), vec!["裏面".into()]))
    );
    let locative = super::parse_synonym_entry("裡面,里面,裏面|D_F+D_S").unwrap();
    assert_eq!(locative.canonical, "裡面");
    assert_eq!(
        locative.pos,
        novel_segment::POSTAG::D_F | novel_segment::POSTAG::D_S
    );
    let hex = super::parse_synonym_entry("公里,公裡|0x00100000").unwrap();
    assert_eq!(hex.pos, novel_segment::POSTAG::D_N);
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
fn split_process_units_breaks_on_punctuation_not_length() {
    let units = split_process_units("甲乙。丙丁，戊己");
    assert_eq!(units, vec!["甲乙。", "丙丁，", "戊己"]);
    let long: String = "裡".repeat(45);
    assert_eq!(split_process_units(&long), vec![long.as_str()]);
}

#[test]
fn format_segment_dict_writes_pos_and_simplified_form() {
    let text = format_segment_dict(
        &[CorrectionEntry {
            canonical: "裡面".into(),
            variants: vec![("裏面".into(), 4)],
        }],
        &|word| {
            if word == "裡面" {
                novel_segment::POSTAG::D_F
            } else {
                0
            }
        },
    );
    assert!(text.contains("裡面|0x2000000|4\n"), "{text}");
    assert!(text.contains("裏面|0x2000000|4\n"), "{text}");
    assert!(text.contains("里面|0x2000000|4\n"), "{text}");
}

#[test]
fn format_segment_dict_always_includes_wagyu() {
    let text = format_segment_dict(&[], &|_| 0);
    assert!(
        text.contains("和牛|0x100000|1000\n"),
        "pinned 和牛 missing: {text}"
    );
    for word in [
        "本里",
        "本裡",
        "里辦",
        "裡辦",
        "里民",
        "裡民",
        "里長",
        "裡長",
        "里名",
        "裡名",
        "胜肽",
        "勝肽",
        "三星鄉",
        "三星乡",
        "莊敬里",
        "莊敬裡",
        "水里鄉",
        "南庄鄉",
        "南莊鄉",
    ] {
        let row = format!("{word}|0x100000|1000\n");
        assert!(text.contains(&row), "pinned {word} missing");
    }
    assert!(!text.contains("羅東鎮|0x100000|1000\n"), "must not pin 鎮");
}

#[test]
fn process_line_keeps_pairs_across_clause_splits() {
    let service = shared_conversion();
    let short = process_line(service, "冰箱裡面大概就剩幾顆蛋跟半盒牛奶");
    let long = process_line(
        service,
        "冰箱裡面大概就剩幾顆蛋跟半盒牛奶。另外制度本身沒問題。",
    );
    for pair in &short.pairs {
        assert!(
            long.pairs.contains(pair),
            "clause split dropped {pair:?} from {short:?} vs {long:?}"
        );
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

#[test]
fn output_must_not_sit_inside_extra_correction() {
    let output = Path::new("/tmp/app/extra-correction");
    let err = assert_output_outside_extra_correction(output).unwrap_err();
    assert!(err.contains("extra-correction"));
    assert!(is_extra_correction_path(output));
}

fn temp_pair(name: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "convertzz-rt-{}-{}",
        name,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let sources = root.join("src");
    let output = root.join("out");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&sources).unwrap();
    (sources, output)
}

fn write_corpus(sources: &Path, relative: &str, body: &str) {
    let path = sources.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, body).unwrap();
}

fn test_service() -> ConversionService {
    ConversionService::without_extra_correction(None).expect("service")
}

fn base_config(sources: PathBuf, output: PathBuf) -> RoundtripRunConfig {
    RoundtripRunConfig {
        sources,
        output,
        select: CorpusSelect::default(),
        min_count: 1,
        min_dominance: 0.7,
        limit: None,
        jobs: 1,
        batch_size: 64,
        memory: MemoryPolicy {
            soft_bytes: Some(0),
            hard_bytes: Some(0),
            lcs_inflight: Some(1),
        },
        reset: false,
        rebuild_outputs_only: false,
        extra_correction: None,
        stop: Arc::new(AtomicBool::new(false)),
        sampler: default_sampler(),
        lines_processed: None,
        files_opened: None,
        process_line_inflight: None,
        process_line_peak: None,
        jobs_current_probe: None,
        lcs_peak: None,
    }
}

const LINE: &str = "冰箱裡面大概就剩幾顆蛋跟半盒牛奶\n";

#[test]
fn finish_from_shards_matches_ram_dominance() {
    let dir = std::env::temp_dir().join(format!("convertzz-shards-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let mut a = PairAggregator::default();
    a.add("裏面".into(), "裡面".into(), "a");
    let mut b = PairAggregator::default();
    b.add("裏面".into(), "裏麵".into(), "b");
    let pa = dir.join("a.pairs");
    let pb = dir.join("b.pairs");
    a.write_shard_path(&pa).unwrap();
    b.write_shard_path(&pb).unwrap();
    let skip = HashSet::new();
    let mut ram = PairAggregator::default();
    ram.merge(a);
    ram.merge(b);
    let (_, ram_stats) = ram.finish(1, 0.7, &skip);
    let (_, shard_stats) = finish_from_shards(&[pa, pb], 1, 0.7, &skip).unwrap();
    assert_eq!(ram_stats.skipped_ambiguous, shard_stats.skipped_ambiguous);
    assert_eq!(ram_stats.skipped_low_count, shard_stats.skipped_low_count);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn merge_sorted_shards_respects_in_memory_limit() {
    let dir = std::env::temp_dir().join(format!("convertzz-merge-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let mut a = PairAggregator::default();
    a.add("裏面".into(), "裡面".into(), "a");
    a.add("這裏".into(), "這裡".into(), "a");
    let mut b = PairAggregator::default();
    b.add("裏面".into(), "裡面".into(), "b");
    let pa = dir.join("a.pairs");
    let pb = dir.join("b.pairs");
    a.write_shard_path(&pa).unwrap();
    b.write_shard_path(&pb).unwrap();
    TEST_MAX_IN_MEMORY_KEYS.store(8, Ordering::SeqCst);
    TEST_PEAK_IN_MEMORY_KEYS.store(0, Ordering::SeqCst);
    let out = dir.join("out.pairs");
    merge_sorted_shard_files(&[pa, pb], &out).unwrap();
    let peak = TEST_PEAK_IN_MEMORY_KEYS.load(Ordering::SeqCst);
    TEST_MAX_IN_MEMORY_KEYS.store(0, Ordering::SeqCst);
    assert!(peak > 0 && peak <= 8, "peak={peak}");
    let merged = PairAggregator::read_shard(&mut fs::File::open(&out).unwrap()).unwrap();
    assert_eq!(merged.unique_raw_pairs(), 2);
    assert_eq!(merged.raw_occurrences(), 3);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn corrupt_shard_magic_is_rejected() {
    let path = std::env::temp_dir().join(format!("convertzz-bad-magic-{}", std::process::id()));
    fs::write(&path, b"NOTAMAGIC!\n").unwrap();
    let err = PairAggregator::read_shard(&mut fs::File::open(&path).unwrap()).unwrap_err();
    assert!(err.contains("magic"));
    let _ = fs::remove_file(&path);
}

#[test]
fn empty_shard_is_valid() {
    let path = std::env::temp_dir().join(format!("convertzz-empty-shard-{}", std::process::id()));
    PairAggregator::default().write_shard_path(&path).unwrap();
    let loaded = PairAggregator::read_shard(&mut fs::File::open(&path).unwrap()).unwrap();
    assert!(loaded.is_empty());
    let _ = fs::remove_file(&path);
}

#[test]
fn run_roundtrip_commits_files_and_resumes() {
    let (sources, output) = temp_pair("resume");
    write_corpus(&sources, "one.txt", &LINE.repeat(3));
    write_corpus(&sources, "two.txt", &LINE.repeat(3));
    let service = test_service();
    let opened = Arc::new(AtomicU64::new(0));
    let mut config = base_config(sources.clone(), output.clone());
    config.files_opened = Some(Arc::clone(&opened));
    let first = run_roundtrip(&service, config).unwrap();
    assert_eq!(first.status, RunStatus::Complete);
    assert_eq!(first.files_committed.len(), 2);
    assert_eq!(opened.load(Ordering::SeqCst), 2);
    assert!(output.join(ORIENTATION_MIN_REPORT).is_file());
    assert!(output.join(ORIENTATION_FULL_REPORT).is_file());

    opened.store(0, Ordering::SeqCst);
    let mut config = base_config(sources.clone(), output.clone());
    config.files_opened = Some(Arc::clone(&opened));
    let second = run_roundtrip(&service, config).unwrap();
    assert_eq!(second.status, RunStatus::Complete);
    assert_eq!(opened.load(Ordering::SeqCst), 0);
    let _ = fs::remove_dir_all(sources.parent().unwrap());
}

#[test]
fn reruns_only_changed_corpus_file() {
    let (sources, output) = temp_pair("incr");
    write_corpus(&sources, "keep.txt", &LINE.repeat(3));
    write_corpus(&sources, "touch.txt", &LINE.repeat(3));
    let service = test_service();
    let opened = Arc::new(AtomicU64::new(0));
    let mut config = base_config(sources.clone(), output.clone());
    config.files_opened = Some(Arc::clone(&opened));
    run_roundtrip(&service, config).unwrap();
    assert_eq!(opened.load(Ordering::SeqCst), 2);

    std::thread::sleep(std::time::Duration::from_millis(1100));
    write_corpus(&sources, "touch.txt", &LINE.repeat(4));
    opened.store(0, Ordering::SeqCst);
    let mut config = base_config(sources.clone(), output.clone());
    config.files_opened = Some(Arc::clone(&opened));
    let second = run_roundtrip(&service, config).unwrap();
    assert_eq!(second.status, RunStatus::Complete);
    assert_eq!(opened.load(Ordering::SeqCst), 1);
    assert_eq!(second.files_committed.len(), 2);
    let _ = fs::remove_dir_all(sources.parent().unwrap());
}

#[test]
fn limit_does_not_commit_half_file() {
    let (sources, output) = temp_pair("limit");
    write_corpus(&sources, "one.txt", &LINE.repeat(20));
    let service = test_service();
    let mut config = base_config(sources.clone(), output.clone());
    config.limit = Some(5);
    let first = run_roundtrip(&service, config).unwrap();
    assert_eq!(first.status, RunStatus::Limit);
    assert!(first.files_committed.is_empty());

    let mut config = base_config(sources.clone(), output.clone());
    config.limit = Some(5);
    let again = run_roundtrip(&service, config).unwrap();
    assert_eq!(again.status, RunStatus::Limit);
    assert!(again.files_committed.is_empty());
    let _ = fs::remove_dir_all(sources.parent().unwrap());
}

#[test]
fn stop_after_first_file_keeps_checkpoint() {
    let (sources, output) = temp_pair("stop");
    write_corpus(&sources, "a.txt", &LINE.repeat(4));
    write_corpus(&sources, "b.txt", &LINE.repeat(40));
    let service = test_service();
    let stop = Arc::new(AtomicBool::new(false));
    let files_opened = Arc::new(AtomicU64::new(0));
    let mut config = base_config(sources.clone(), output.clone());
    config.stop = Arc::clone(&stop);
    config.files_opened = Some(Arc::clone(&files_opened));
    config.batch_size = 4;
    config.jobs = 1;
    stop.store(true, Ordering::SeqCst);
    let result = run_roundtrip(&service, config).unwrap();
    assert_eq!(result.status, RunStatus::Interrupted);
    let _ = fs::remove_dir_all(sources.parent().unwrap());
}

#[test]
fn leftover_uncommitted_does_not_inflate_next_file() {
    let (sources, output) = temp_pair("orphan");
    write_corpus(&sources, "a.txt", &LINE.repeat(3));
    write_corpus(&sources, "b.txt", &LINE.repeat(3));
    let service = test_service();
    let config = base_config(sources.clone(), output.clone());
    run_roundtrip(&service, config).unwrap();
    fs::create_dir_all(output.join("state/uncommitted")).unwrap();
    let mut junk = PairAggregator::default();
    junk.add("裏面".into(), "裡面".into(), "junk");
    junk.write_shard_path(&output.join("state/uncommitted").join("a.txt__000001.pairs"))
        .unwrap();
    let unique_before = serde_json::from_str::<serde_json::Value>(
        &fs::read_to_string(output.join("report.json")).unwrap(),
    )
    .unwrap()["uniqueRawPairs"]
        .as_u64()
        .unwrap();
    let mut config = base_config(sources.clone(), output.clone());
    config.reset = false;
    run_roundtrip(&service, config).unwrap();
    let unique_after = serde_json::from_str::<serde_json::Value>(
        &fs::read_to_string(output.join("report.json")).unwrap(),
    )
    .unwrap()["uniqueRawPairs"]
        .as_u64()
        .unwrap();
    assert_eq!(unique_before, unique_after);
    let _ = fs::remove_dir_all(sources.parent().unwrap());
}

#[test]
fn rebuild_without_shards_fails() {
    let (sources, output) = temp_pair("rebuild");
    fs::create_dir_all(&output).unwrap();
    write_corpus(&sources, "a.txt", "甲\n");
    let service = test_service();
    let mut config = base_config(sources.clone(), output.clone());
    config.rebuild_outputs_only = true;
    let err = run_roundtrip(&service, config).unwrap_err();
    assert!(err.contains("shard") || err.contains("檢查點") || err.contains("重建"));
    let _ = fs::remove_dir_all(sources.parent().unwrap());
}

#[test]
fn fingerprint_rejects_changed_include() {
    let (sources, output) = temp_pair("fp");
    fs::create_dir_all(sources.join("keep")).unwrap();
    fs::create_dir_all(sources.join("other")).unwrap();
    write_corpus(&sources, "keep/a.txt", &LINE.repeat(3));
    write_corpus(&sources, "other/b.txt", &LINE.repeat(3));
    let service = test_service();
    let mut config = base_config(sources.clone(), output.clone());
    config.select.include = vec!["keep".into()];
    run_roundtrip(&service, config).unwrap();
    let mut config = base_config(sources.clone(), output.clone());
    config.select.include = vec!["other".into()];
    let err = run_roundtrip(&service, config).unwrap_err();
    assert!(err.contains("--reset"));
    let _ = fs::remove_dir_all(sources.parent().unwrap());
}

#[test]
fn roundtrip_does_not_collapse_two_attested_words() {
    let (sources, output) = temp_pair("jizhi-both");
    write_corpus(&sources, "a.txt", "觸發機制\n觸發機製\n觸發機制\n");
    let service = test_service();
    let config = base_config(sources.clone(), output.clone());
    run_roundtrip(&service, config).unwrap();
    let synonym = fs::read_to_string(output.join("zht.corpus.synonym.txt")).unwrap();
    assert!(
        !synonym
            .lines()
            .any(|line| line.starts_with("機製,機制") || line.starts_with("機制,機製")),
        "both 機制 and 機製 occur as originals, should not be synonyms:\n{synonym}"
    );
    let _ = fs::remove_dir_all(sources.parent().unwrap());
}

fn write_extra_correction(dir: &Path, synonym: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("zht.corpus.synonym.txt"), synonym).unwrap();
    let mut dict = String::from("// extra dict\n");
    for line in synonym.lines() {
        if let Some((canonical, variants)) = parse_synonym_line(line) {
            dict.push_str(&format!("{canonical}|0x100000|1\n"));
            for variant in variants {
                dict.push_str(&format!("{variant}|0x100000|1\n"));
            }
        }
    }
    fs::write(dir.join("zht.corpus.dict.txt"), dict).unwrap();
}

#[test]
fn extra_correction_must_not_sit_inside_sources() {
    let (sources, output) = temp_pair("extra-in-src");
    let extra = sources.join("extra");
    write_extra_correction(&extra, "裡面,裏面\n");
    let err = assert_extra_correction_paths(&extra, &output, &sources).unwrap_err();
    assert!(err.contains("來源"));
    let _ = fs::remove_dir_all(sources.parent().unwrap());
}

#[test]
fn extra_correction_must_not_overlap_output() {
    let (sources, output) = temp_pair("extra-out");
    fs::create_dir_all(&output).unwrap();
    write_extra_correction(&output, "裡面,裏面\n");
    let err = assert_extra_correction_paths(&output, &output, &sources).unwrap_err();
    assert!(err.contains("輸出"));
    let _ = fs::remove_dir_all(sources.parent().unwrap());
}

#[test]
fn extra_correction_missing_synonym_is_rejected() {
    let (sources, output) = temp_pair("extra-missing");
    let extra = sources.parent().unwrap().join("extra");
    fs::create_dir_all(&extra).unwrap();
    let err = assert_extra_correction_paths(&extra, &output, &sources).unwrap_err();
    assert!(err.contains("zht.corpus.synonym.txt"));
    let _ = fs::remove_dir_all(sources.parent().unwrap());
}

#[test]
fn fingerprint_rejects_changed_extra_correction() {
    let (sources, output) = temp_pair("fp-extra");
    write_corpus(&sources, "a.txt", &LINE.repeat(3));
    let extra = sources.parent().unwrap().join("extra");
    write_extra_correction(&extra, "裡面,裏面\n");
    let service = test_service();
    run_roundtrip(&service, base_config(sources.clone(), output.clone())).unwrap();
    let mut config = base_config(sources.clone(), output.clone());
    config.extra_correction = Some(extra);
    let err = run_roundtrip(&service, config).unwrap_err();
    assert!(err.contains("--reset"));
    let _ = fs::remove_dir_all(sources.parent().unwrap());
}

#[test]
fn extra_correction_skips_known_variants_in_output() {
    let (sources, output) = temp_pair("extra-skip");
    write_corpus(&sources, "a.txt", "這道菜值得品嚐\n".repeat(3).as_str());
    let extra = sources.parent().unwrap().join("extra");
    write_extra_correction(&extra, "品嚐,品嘗\n");
    let probe = sources.parent().unwrap().join("probe");
    let service = test_service();
    run_roundtrip(&service, base_config(sources.clone(), output.clone())).unwrap();
    let baseline = fs::read_to_string(output.join("zht.corpus.synonym.txt")).unwrap();
    assert!(
        baseline.lines().any(|line| line.contains("品嘗")),
        "baseline should keep 品嚐/品嘗, got {baseline}"
    );
    let mut config = base_config(sources.clone(), probe.clone());
    config.extra_correction = Some(extra);
    run_roundtrip(&service, config).unwrap();
    let synonym = fs::read_to_string(probe.join("zht.corpus.synonym.txt")).unwrap();
    assert!(
        !synonym.lines().any(|line| line.contains("品嘗")),
        "probe should skip extra-correction variants, got {synonym}"
    );
    let _ = fs::remove_dir_all(sources.parent().unwrap());
}

#[test]
fn baseline_over_hard_fails() {
    let (sources, output) = temp_pair("baseline");
    write_corpus(&sources, "a.txt", LINE);
    let service = test_service();
    let sampler = Arc::new(FakeSampler::default());
    sampler.rss_bytes.store(10 * 1024 * 1024, Ordering::SeqCst);
    let mut config = base_config(sources.clone(), output.clone());
    config.sampler = sampler;
    config.memory.hard_bytes = Some(1024);
    config.memory.soft_bytes = Some(512);
    let err = run_roundtrip(&service, config).unwrap_err();
    assert!(err.contains("基線"));
    let _ = fs::remove_dir_all(sources.parent().unwrap());
}

#[test]
fn roundtrip_mixed_units_from_pool_does_not_deadlock() {
    let (sources, output) = temp_pair("nested-pool");
    let body = "這裡是里辦廣播？各位里民，關於本里申請的。\n".repeat(80);
    write_corpus(&sources, "a.txt", &body);
    let service = test_service();
    let mut config = base_config(sources.clone(), output.clone());
    config.jobs = 4;
    config.batch_size = 64;
    config.memory.lcs_inflight = Some(4);
    let status = run_roundtrip(&service, config).unwrap();
    assert_eq!(status.lines_read, 80);
    let _ = fs::remove_dir_all(sources.parent().unwrap());
}

#[test]
fn process_line_semaphore_caps_inflight() {
    let (sources, output) = temp_pair("sem");
    write_corpus(&sources, "a.txt", &LINE.repeat(80));
    let service = test_service();
    let inflight = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let jobs = Arc::new(AtomicUsize::new(0));
    let mut config = base_config(sources.clone(), output.clone());
    config.jobs = 4;
    config.batch_size = 32;
    config.memory.lcs_inflight = Some(1);
    config.process_line_inflight = Some(Arc::clone(&inflight));
    config.process_line_peak = Some(Arc::clone(&peak));
    config.jobs_current_probe = Some(Arc::clone(&jobs));
    let lcs_peak = Arc::new(AtomicUsize::new(0));
    config.lcs_peak = Some(Arc::clone(&lcs_peak));
    run_roundtrip(&service, config).unwrap();
    assert!(peak.load(Ordering::SeqCst) <= 4);
    assert!(lcs_peak.load(Ordering::SeqCst) <= 1);
    let _ = fs::remove_dir_all(sources.parent().unwrap());
}

#[test]
fn skip_existing_synonym_variants_counts() {
    let mut aggregator = PairAggregator::default();
    aggregator.add("裏面".into(), "裡面".into(), "a");
    let mut skip = HashSet::new();
    skip.insert("裏面".into());
    let (entries, stats) = aggregator.finish(1, 0.7, &skip);
    assert!(entries.is_empty());
    assert_eq!(stats.skipped_existing, 1);
}

use super::super::types::EngineKind;
use super::*;

fn service() -> &'static ConversionService {
    super::shared_conversion()
}

async fn convert(text: &str, direction: Direction, engine: EngineKind) -> String {
    service()
        .convert(ConversionRequest {
            text: text.into(),
            direction,
            engine,
            dictionary_path: None,
            zhconvert: None,
            vocabulary_correction: None,
        })
        .await
        .unwrap()
        .text
}

#[tokio::test]
async fn segmented_s2t_golden_cases() {
    let service = service();
    for (source, expected) in [
        ("里面", "裡面"),
        ("個別選手表現觀察週報", "個別選手表現觀察週報"),
        ("選手表現", "選手表現"),
        ("选手表现", "選手表現"),
        ("表現", "表現"),
        ("表面", "表面"),
        ("手表", "手錶"),
        ("皇后", "皇后"),
        ("头发", "頭髮"),
        ("开发", "開發"),
        ("面对表面", "面對表面"),
        ("面包", "麵包"),
        ("日志", "日誌"),
        ("几率", "機率"),
        ("餅干", "餅乾"),
        ("秘密", "秘密"),
        ("公里", "公里"),
        ("台湾", "台灣"),
        ("为了规避监督", "為了規避監督"),
        ("為了規避監督", "為了規避監督"),
        ("了解", "了解"),
        ("瞭解", "了解"),
        ("瞭望", "瞭望"),
        ("尝试跟死忠的沟通", "嘗試跟死忠的溝通"),
        ("嘗試跟死忠的溝通", "嘗試跟死忠的溝通"),
        ("品尝", "品嚐"),
        ("上台发言", "上台發言"),
        ("平台", "平台"),
        ("头发", "頭髮"),
        ("頭髮", "頭髮"),
        ("制药", "製藥"),
        ("幹嗎", "幹嗎"),
        ("重复", "重複"),
        ("銷毀", "銷毀"),
        ("策划", "策劃"),
        ("淋漓尽致", "淋漓盡致"),
        ("鉴于", "鑑於"),
        ("身份证", "身分證"),
        ("部分", "部分"),
        ("注定", "注定"),
        ("新台幣", "新台幣"),
        ("电台", "電台"),
        ("后台", "後台"),
        ("混淆雙首長制", "混淆雙首長制"),
        ("小吃店老闆娘", "小吃店老闆娘"),
        ("雙四分之三", "雙四分之三"),
        ("幹你娘", "幹你娘"),
        ("轉換身份", "轉換身份"),
        ("現制的監察權", "現制的監察權"),
        ("譯者：曾依璇", "譯者：曾依璇"),
        ("孫運璿", "孫運璿"),
        ("製藥", "製藥"),
        ("処理", "處理"),
        ("説明書", "說明書"),
        ("把錢要回", "把錢要回"),
        ("水分子", "水分子"),
        ("知識分子", "知識分子"),
        ("不准進入", "不准進入"),
        ("金馬影后", "金馬影后"),
        ("編製預算", "編製預算"),
        ("機製麵", "機製麵"),
        ("觸發機制", "觸發機制"),
        ("保障機制", "保障機制"),
        ("機制", "機制"),
        ("本店專製", "本店專製"),
        ("蘇製武器", "蘇製武器"),
        ("律師公會", "律師公會"),
        ("這麼好", "這麼好"),
        ("那麼辦", "那麼辦"),
        ("回復健康", "回復健康"),
        ("這是他幹的", "這是他幹的"),
        ("了若指掌", "了若指掌"),
        ("症結", "症結"),
        ("並行處理", "並行處理"),
        ("三公里外", "三公里外"),
        ("三公裡外", "三公里外"),
        ("這裡是里辦廣播", "這裡是里辦廣播"),
        ("各位里民", "各位里民"),
        ("關於本里申請的", "關於本里申請的"),
        ("這說明了環境教育", "這說明了環境教育"),
        ("這說明瞭環境教育", "這說明了環境教育"),
        ("簡單明瞭", "簡單明瞭"),
        ("簡單明了", "簡單明瞭"),
        ("明瞭", "明瞭"),
        ("明了", "明了"),
        ("彭傑燊", "彭傑燊"),
        ("胜肽", "胜肽"),
        ("勝肽", "胜肽"),
        ("勝利", "勝利"),
        ("里長", "里長"),
        ("裡長", "里長"),
        ("里长", "里長"),
        ("里名", "里名"),
        ("裡名", "里名"),
        ("各位里長好", "各位里長好"),
        ("請填里名", "請填里名"),
        ("巴西里碎屑", "巴西里碎屑"),
        ("膿疱", "膿疱"),
        ("膿皰", "膿疱"),
        ("哪里", "哪裡"),
        ("家裡", "家裡"),
        ("家里", "家裡"),
        ("裡面", "裡面"),
    ] {
        let result = service
            .convert(ConversionRequest {
                text: source.into(),
                direction: Direction::S2t,
                engine: EngineKind::Segmented,
                dictionary_path: None,
                zhconvert: None,
                vocabulary_correction: None,
            })
            .await
            .unwrap();
        assert_eq!(
            result.text,
            expected,
            "{source} tokens={:?}",
            service.debug_pos(source)
        );
    }
}

#[tokio::test]
async fn me_particle_becomes_me_except_youngest_child() {
    for (source, expected) in [
        ("什么", "什麼"),
        ("怎么", "怎麼"),
        ("那么", "那麼"),
        ("这么", "這麼"),
        ("要么", "要麼"),
        ("好么", "好麼"),
        ("是么", "是麼"),
        ("干么", "幹麼"),
        ("老么", "老么"),
        ("么兒", "么兒"),
        ("么儿", "么兒"),
        ("么女", "么女"),
        ("他是家裡的老么", "他是家裡的老么"),
        ("老幺", "老么"),
        ("幺女", "么女"),
    ] {
        let got = convert(source, Direction::S2t, EngineKind::Segmented).await;
        assert_eq!(got, expected, "{source}");
    }
}

#[tokio::test]
async fn wagyu_stays_one_word_so_zhi_is_not_zhi_classifier() {
    let service = service();
    let source = "和牛只剩兩份";
    let tokens = service.debug_pos(source);
    assert!(
        tokens.iter().any(|(word, _)| word == "和牛"),
        "和牛 should be one token, got {tokens:?}"
    );
    assert!(
        !tokens
            .iter()
            .any(|(word, _)| word == "牛只" || word == "牛隻"),
        "must not split 和牛只剩 as 牛只, got {tokens:?}"
    );
    let result = service
        .convert(ConversionRequest {
            text: source.into(),
            direction: Direction::S2t,
            engine: EngineKind::Segmented,
            dictionary_path: None,
            zhconvert: None,
            vocabulary_correction: None,
        })
        .await
        .unwrap();
    assert_eq!(result.text, source, "tokens={tokens:?}");
}

#[tokio::test]
async fn athlete_performance_stays_two_words_so_watch_is_not_watch() {
    let service = service();
    for source in [
        "個別選手表現觀察週報",
        "个别选手表现观察周报",
        "選手表現",
        "选手表现",
    ] {
        let tokens = service.debug_pos(source);
        assert!(
            tokens
                .iter()
                .any(|(word, _)| word == "選手" || word == "选手"),
            "選手 should be one token, got {tokens:?}"
        );
        assert!(
            tokens
                .iter()
                .any(|(word, _)| word == "表現" || word == "表现"),
            "表現 should be one token, got {tokens:?}"
        );
        assert!(
            !tokens
                .iter()
                .any(|(word, _)| word == "手表" || word == "手錶"),
            "must not split 選手表現 as 手表, got {tokens:?}"
        );
    }
}

#[tokio::test]
async fn taiwan_xiang_li_place_names_stay() {
    let service = service();
    for (source, expected) in [
        ("三星鄉", "三星鄉"),
        ("三星乡", "三星鄉"),
        ("莊敬里", "莊敬里"),
        ("莊敬裡", "莊敬里"),
        ("水里鄉", "水里鄉"),
        ("水裡鄉", "水里鄉"),
        ("南庄鄉", "南庄鄉"),
        ("太麻里鄉", "太麻里鄉"),
        ("臺北市", "臺北市"),
        ("台北市", "臺北市"),
        ("宜蘭縣", "宜蘭縣"),
        ("宜兰县", "宜蘭縣"),
        ("竹北市", "竹北市"),
        ("羅東鎮", "羅東鎮"),
        ("羅東镇", "羅東鎮"),
        ("這裡", "這裡"),
        ("公里", "公里"),
        ("家裡", "家裡"),
    ] {
        let result = service
            .convert(ConversionRequest {
                text: source.into(),
                direction: Direction::S2t,
                engine: EngineKind::Segmented,
                dictionary_path: None,
                zhconvert: None,
                vocabulary_correction: None,
            })
            .await
            .unwrap();
        assert_eq!(result.text, expected, "{source}");
    }
}

#[tokio::test]
async fn peptide_and_li_titles_stay_one_word() {
    let service = service();
    for (source, word) in [
        ("胜肽合成", "胜肽"),
        ("勝肽合成", "勝肽"),
        ("各位里長好", "里長"),
        ("請填里名", "里名"),
        ("宜蘭縣三星鄉公所", "三星鄉"),
        ("宜蘭縣三星鄉公所", "宜蘭縣"),
        ("臺北市信義區", "臺北市"),
        ("宜蘭縣羅東鎮", "羅東鎮"),
        ("通知莊敬里里民", "莊敬里"),
        ("南庄里", "南庄里"),
        ("苗栗縣南庄鄉南庄里", "南庄里"),
    ] {
        let tokens = service.debug_pos(source);
        assert!(
            tokens.iter().any(|(item, _)| item == word),
            "{source}: {word} should be one token, got {tokens:?}"
        );
        assert!(
            !tokens.iter().any(|(item, _)| item == "胜"
                || item == "勝"
                || item == "肽"
                || item == "里"
                || item == "裡"
                || item == "長"
                || item == "名"
                || item == "鄉"
                || item == "乡"
                || item == "鎮"
                || item == "镇"
                || item == "縣"
                || item == "市"),
            "{source}: must not split {word}, got {tokens:?}"
        );
    }
}

#[tokio::test]
async fn segmented_t2s_golden_cases() {
    for (source, expected) in [
        ("裡面", "里面"),
        ("皇后", "皇后"),
        ("頭髮", "头发"),
        ("開發", "开发"),
    ] {
        assert_eq!(
            convert(source, Direction::T2s, EngineKind::Segmented).await,
            expected
        );
    }
}

#[tokio::test]
async fn preserves_whitespace_and_punctuation() {
    assert_eq!(
        convert("里面  开发\n头发", Direction::S2t, EngineKind::Segmented).await,
        "裡面  開發\n頭髮"
    );
    assert_eq!(
        convert("里面  😀\n《A》", Direction::S2t, EngineKind::Segmented).await,
        "裡面  😀\n《A》"
    );
}

#[test]
fn convert_segmented_inside_rayon_pool_does_not_deadlock() {
    let service = service();
    let text = "這裡是里辦廣播？各位里民，關於本里申請的。";
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .expect("pool");
    let results: Vec<String> = pool.install(|| {
        use rayon::prelude::*;
        (0..64)
            .into_par_iter()
            .map(|_| service.convert_segmented(text, Direction::S2t))
            .collect()
    });
    assert_eq!(results.len(), 64);
    assert!(results
        .iter()
        .all(|item| item.contains("這裡") && item.contains("里辦")));
}

#[test]
fn mixed_kana_does_not_panic_the_segmenter() {
    let tokens = service().segment_tokens("こんにちはカタカナ");
    assert!(
        tokens.iter().any(|token| token.contains('こ'))
            && tokens.iter().any(|token| token.contains('カ')),
        "{tokens:?}"
    );
    assert!(
        tokens
            .iter()
            .any(|token| token.contains('こ') && !token.contains('カ')),
        "hiragana and katakana should split: {tokens:?}"
    );
}

#[test]
fn with_extra_correction_requires_synonym_file() {
    let root = std::env::temp_dir().join(format!(
        "convertzz-extra-svc-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let err = match super::ConversionService::with_extra_correction(None, &root) {
        Ok(_) => panic!("expected missing synonym to fail"),
        Err(error) => error,
    };
    assert!(err.to_string().contains("zht.corpus.synonym.txt"));
    std::fs::write(root.join("zht.corpus.synonym.txt"), "制度,製度\n").unwrap();
    super::ConversionService::with_extra_correction(None, &root).expect("load extra");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn neighborhood_li_and_jieju_keep_taiwan_forms() {
    let service = service();
    for (source, expected) in [
        (
            "各位鄰居好，提醒大家關於本里垃圾車時間的異動。",
            "各位鄰居好，提醒大家關於本里垃圾車時間的異動。",
        ),
        (
            "請立刻往高處移動或聯繫里辦協助。",
            "請立刻往高處移動或聯繫里辦協助。",
        ),
        ("關於本里垃圾車時間", "關於本里垃圾車時間"),
        ("聯繫里辦協助", "聯繫里辦協助"),
        ("這裡是里辦廣播", "這裡是里辦廣播"),
        ("各位里民", "各位里民"),
        ("各位里長", "各位里長"),
        ("請填里名", "請填里名"),
        ("感到拮据", "感到拮据"),
        ("感到拮據", "感到拮据"),
        (
            "这款游戏的经济设计确实比较倾向于让玩家在后期感到拮据",
            "這款遊戲的經濟設計確實比較傾向於讓玩家在後期感到拮据",
        ),
    ] {
        let result = service
            .convert(ConversionRequest {
                text: source.into(),
                direction: Direction::S2t,
                engine: EngineKind::Segmented,
                dictionary_path: None,
                zhconvert: None,
                vocabulary_correction: None,
            })
            .await
            .unwrap();
        assert_eq!(
            result.text,
            expected,
            "{source} tokens={:?}",
            service.debug_pos(source)
        );
    }
}

#[tokio::test]
async fn extra_synonym_pos_is_consulted() {
    let root = std::env::temp_dir().join(format!(
        "convertzz-extra-pos-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("zht.corpus.dict.txt"),
        "錕鋙正|0x1000|20000\n錕鋙錯|0x100000|20000\n",
    )
    .unwrap();
    std::fs::write(root.join("zht.corpus.synonym.txt"), "錕鋙正,錕鋙錯|D_V\n").unwrap();
    let noun_only = super::ConversionService::with_extra_correction(None, &root).unwrap();
    let noun = noun_only
        .convert(ConversionRequest {
            text: "錕鋙錯".into(),
            direction: Direction::S2t,
            engine: EngineKind::Segmented,
            dictionary_path: None,
            zhconvert: None,
            vocabulary_correction: None,
        })
        .await
        .unwrap();
    assert_eq!(
        noun.text,
        "錕鋙錯",
        "名詞不該套用動詞同義詞 tokens={:?}",
        noun_only.debug_pos("錕鋙錯")
    );

    std::fs::write(
        root.join("zht.corpus.dict.txt"),
        "錕鋙正|0x1000|20000\n錕鋙錯|0x1000|20000\n",
    )
    .unwrap();
    let verb = super::ConversionService::with_extra_correction(None, &root).unwrap();
    let matched = verb
        .convert(ConversionRequest {
            text: "錕鋙錯".into(),
            direction: Direction::S2t,
            engine: EngineKind::Segmented,
            dictionary_path: None,
            zhconvert: None,
            vocabulary_correction: None,
        })
        .await
        .unwrap();
    assert_eq!(
        matched.text,
        "錕鋙正",
        "tokens={:?}",
        verb.debug_pos("錕鋙錯")
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn extra_synonym_does_not_overwrite_stable_taiwan_forms() {
    let root = std::env::temp_dir().join(format!(
        "convertzz-extra-stable-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("zht.corpus.dict.txt"),
        "\
上麵|0x2000000|9\n上面|0x2000000|9\n\
控製|0x1000|9\n控制|0x1000|9\n\
併於|0x10000000|9\n並於|0x10000000|9\n\
部份|0x100000|9\n部分|0x100000|9\n\
隻有|0x10000000|9\n只有|0x10000000|9\n\
機製|0x100000|9\n機制|0x100000|9\n\
七隻|0x300000|9\n七只|0x300000|9\n",
    )
    .unwrap();
    std::fs::write(
        root.join("zht.corpus.synonym.txt"),
        "\
上麵,上面|D_F\n\
控製,控制|D_V\n\
併於,並於|D_C\n\
部份,部分|D_N\n\
隻有,只有|D_C\n\
機製,機制|D_N\n\
七隻,七只|D_MQ+D_N\n",
    )
    .unwrap();
    let extra = super::ConversionService::with_extra_correction(None, &root).unwrap();
    for (source, expected) in [
        ("上面註記", "上面註記"),
        ("控制藥物", "控制藥物"),
        ("並於1週內", "並於1週內"),
        ("部分自行負擔", "部分自行負擔"),
        ("我只有一本書", "我只有一本書"),
        ("觸發機制", "觸發機制"),
        ("保障機制", "保障機制"),
        ("機製麵", "機製麵"),
        ("七只小狗", "七隻小狗"),
    ] {
        let result = extra
            .convert(ConversionRequest {
                text: source.into(),
                direction: Direction::S2t,
                engine: EngineKind::Segmented,
                dictionary_path: None,
                zhconvert: None,
                vocabulary_correction: None,
            })
            .await
            .unwrap();
        assert_eq!(
            result.text,
            expected,
            "{source} tokens={:?}",
            extra.debug_pos(source)
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn extra_correction_candidates_stay_outside_package_dicts() {
    let exe = Path::new("/tmp/squashfs-root/usr/bin/convertzz");
    let appdir = Path::new("/tmp/squashfs-root");
    let candidates = super::extra_correction_candidates(Some(exe), Some(appdir));
    assert!(candidates
        .iter()
        .all(|path| !super::super::roundtrip_dict::is_package_data_path(path)));
    assert!(candidates.iter().any(|path| {
        path.ends_with("extra-correction") && !path.ends_with("segment-dict/extra-correction")
    }));
}

#[test]
fn segment_dict_candidates_include_linux_bundle_layout() {
    let exe = Path::new("/tmp/squashfs-root/usr/bin/convertzz");
    let appdir = Path::new("/tmp/squashfs-root");
    let candidates = super::segment_dict_candidates(Some(exe), Some(appdir));
    assert!(candidates.iter().any(|path| {
        path == Path::new("/tmp/squashfs-root/usr/bin/../lib/ConvertZZ/segment-dict")
            || path.ends_with("lib/ConvertZZ/segment-dict")
    }));
    assert!(candidates
        .iter()
        .any(|path| path == Path::new("/tmp/squashfs-root/usr/lib/ConvertZZ/segment-dict")));
}

#[test]
fn segment_dict_candidates_resolve_extracted_appimage_layout() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/tmp-segment-dict-layout");
    let bin_dir = root.join("usr/bin");
    let dict_dir = root.join("usr/lib/ConvertZZ/segment-dict/segment");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&bin_dir).unwrap();
    std::fs::create_dir_all(&dict_dir).unwrap();
    let exe = bin_dir.join("convertzz");
    std::fs::write(&exe, []).unwrap();
    let resolved = super::segment_dict_candidates(Some(&exe), Some(&root))
        .into_iter()
        .find(|path| path.join("segment").is_dir());
    let _ = std::fs::remove_dir_all(&root);
    let resolved = resolved.expect("bundle layout segment-dict");
    assert!(resolved.join("segment").is_dir());
}

#[test]
fn split_text_breaks_on_ideographic_full_stop_without_slicing_mid_char() {
    let source = format!("{}。{}", "甲".repeat(5_000), "乙".repeat(4_000));
    let chunks = split_text(&source);
    assert!(chunks.len() >= 2);
    assert!(chunks
        .iter()
        .all(|chunk| { chunk.chars().next().is_some() && chunk.is_char_boundary(chunk.len()) }));
    assert_eq!(chunks.concat(), source);
    assert!(chunks[0].ends_with('。'));
}

#[test]
fn split_cjk_runs_keeps_markup_and_cjk_separate() {
    let runs = split_cjk_runs("<div>里面</div>");
    assert_eq!(
        runs,
        vec![
            TextRun::Plain("<div>"),
            TextRun::Cjk("里面"),
            TextRun::Plain("</div>"),
        ]
    );
}

#[test]
fn split_cjk_runs_breaks_on_newlines_and_punctuation() {
    assert_eq!(
        split_cjk_runs("里面\n开发。头发"),
        vec![
            TextRun::Cjk("里面"),
            TextRun::Plain("\n"),
            TextRun::Cjk("开发"),
            TextRun::Plain("。"),
            TextRun::Cjk("头发"),
        ]
    );
    assert_eq!(
        split_cjk_runs("里面\r\n开发"),
        vec![
            TextRun::Cjk("里面"),
            TextRun::Plain("\r\n"),
            TextRun::Cjk("开发"),
        ]
    );
}

#[tokio::test]
async fn long_text_does_not_split_unicode() {
    let source = format!("{}😀里面", "里".repeat(9_000));
    let result = convert(&source, Direction::S2t, EngineKind::Segmented).await;
    assert!(result.ends_with("😀裡面"));
    assert!(!result.contains('�'));
}

#[tokio::test]
async fn legacy_dictionary() {
    let result = convert("软件和头发", Direction::S2t, EngineKind::Legacy).await;
    assert!(result.contains("軟體"));
    assert!(result.contains("頭髮"));
}

#[tokio::test]
async fn legacy_dictionary_reloads_after_same_mtime_replace() {
    // CI filesystems may only resolve mtime to one second. Same-length replacements
    // (裡面→裏邊) must still invalidate the cache after an atomic rename (Unix inode /
    // Windows creation_time; file_index is nightly-only).
    let directory =
        std::env::temp_dir().join(format!("convertzz-dict-cache-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("Dictionary.csv");
    let original = "\u{feff}true\t一般\t里面\t1\t裡面\t1\n";
    let updated = "\u{feff}true\t一般\t里面\t3\t裏邊\t3\n";
    assert_eq!(original.len(), updated.len());
    std::fs::write(&path, original).unwrap();
    let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();

    let first = service()
        .convert(ConversionRequest {
            text: "里面".into(),
            direction: Direction::S2t,
            engine: EngineKind::Legacy,
            dictionary_path: Some(path.to_string_lossy().into_owned()),
            zhconvert: None,
            vocabulary_correction: None,
        })
        .await
        .unwrap();
    assert_eq!(first.text, "裡面");

    let temporary = directory.join(".convertzz-dictionary-next.csv");
    let previous = directory.join(".convertzz-dictionary-previous.csv");
    std::fs::write(&temporary, updated).unwrap();
    std::fs::rename(&path, &previous).unwrap();
    std::fs::rename(&temporary, &path).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(mtime)
        .unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), mtime);

    let second = service()
        .convert(ConversionRequest {
            text: "里面".into(),
            direction: Direction::S2t,
            engine: EngineKind::Legacy,
            dictionary_path: Some(path.to_string_lossy().into_owned()),
            zhconvert: None,
            vocabulary_correction: None,
        })
        .await
        .unwrap();
    assert_eq!(second.text, "裏邊");
    let _ = std::fs::remove_dir_all(&directory);
}

#[tokio::test]
async fn vocabulary_off_uses_glyph_only() {
    let result = service()
        .convert(ConversionRequest {
            text: "里面开发面对表面钟表简繁转换".into(),
            direction: Direction::S2t,
            engine: EngineKind::Segmented,
            dictionary_path: None,
            zhconvert: None,
            vocabulary_correction: Some(false),
        })
        .await
        .unwrap();
    // 詞彙關閉時只做 1-1 字形：里不改裡、面不改麵；钟仍是鐘。
    assert_eq!(result.text, "里面開發面對表面鐘表簡繁轉換");
    assert!(result.warnings[0].contains("詞彙修正已停用"));
}

#[test]
fn glyph_s2t_uses_cn2tw_min_only() {
    // min：钟→鐘、体類一對一。不用 cjk2zht，所以 里／制／娘／璇／処 維持原字。
    assert_eq!(super::glyph_s2t("钟表"), "鐘表");
    assert_eq!(super::glyph_s2t("秒钟"), "秒鐘");
    assert_eq!(super::glyph_s2t("里面"), "里面");
    assert_eq!(super::glyph_s2t("面对表面"), "面對表面");
    assert_eq!(super::glyph_s2t("简繁转换"), "簡繁轉換");
    assert_eq!(super::glyph_s2t("説明書"), "説明書");
    assert_eq!(super::glyph_s2t("疱"), "疱");
    assert_eq!(super::glyph_s2t("皰"), "疱");
    assert_eq!(super::glyph_s2t("制"), "制");
    assert_eq!(super::glyph_s2t("娘"), "娘");
    assert_eq!(super::glyph_s2t("璇"), "璇");
    assert_eq!(super::glyph_s2t("製"), "製");
    assert_eq!(super::glyph_s2t("処理"), "処理");
    assert_eq!(super::glyph_s2t("疱"), "疱");
}

#[tokio::test]
async fn mixed_html_like_content_stays_interactive() {
    // Long non-CJK markup with sparse CJK (same shape as saved web pages).
    let mut text = String::new();
    text.push_str("<!DOCTYPE html><html><head><style>");
    text.push_str(&"body{margin:0;}".repeat(2_000));
    text.push_str("</style><script>");
    text.push_str(&"var x='base64-like-".repeat(1_500));
    text.push_str("';</script></head><body>");
    text.push_str("<p>里面开发头发软件</p>");
    text.push_str(&"<div class='pad'>........</div>".repeat(1_000));
    text.push_str("<p>皇后面对表面</p></body></html>");

    let service = service();
    // Exclude dictionary load from the conversion budget.
    let _ = convert("里面", Direction::S2t, EngineKind::Segmented).await;

    let started = Instant::now();
    let result = service
        .convert(ConversionRequest {
            text: text.clone(),
            direction: Direction::S2t,
            engine: EngineKind::Segmented,
            dictionary_path: None,
            zhconvert: None,
            vocabulary_correction: None,
        })
        .await
        .unwrap();
    let elapsed = started.elapsed();
    assert!(result.text.contains("裡面"));
    assert!(result.text.contains("頭髮"));
    assert!(result.text.contains("皇后"));
    assert!(result.text.contains("<script>"));
    assert_eq!(result.text.chars().count(), text.chars().count());
    // Old path fed whole HTML into the segmenter (~60s debug / ~3s release for ~90KB).
    assert!(
        elapsed.as_secs_f64() < 2.0,
        "mixed HTML conversion too slow: {elapsed:?}"
    );
}

#[tokio::test]
async fn long_unpunctuated_cjk_does_not_explode() {
    let clause = "里面开发头发软件皇后面对表面今天天气很好几率日志面包";
    let text = clause.repeat(40);
    let _ = convert("里面", Direction::S2t, EngineKind::Segmented).await;
    let started = std::time::Instant::now();
    let result = convert(&text, Direction::S2t, EngineKind::Segmented).await;
    let elapsed = started.elapsed();
    assert_eq!(result.chars().count(), text.chars().count());
    assert!(
        elapsed.as_secs_f64() < 2.0,
        "unpunctuated CJK conversion too slow: {elapsed:?}"
    );
}

#[tokio::test]
async fn punctuated_chinese_does_not_explode() {
    let clause = "里面开发头发软件，皇后面对表面。今天天氣很好。\n";
    let text = clause.repeat(400);
    let _ = convert("里面", Direction::S2t, EngineKind::Segmented).await;
    let started = Instant::now();
    let result = convert(&text, Direction::S2t, EngineKind::Segmented).await;
    let elapsed = started.elapsed();
    assert!(result.contains("裡面"));
    assert!(result.contains("頭髮"));
    assert!(
        elapsed.as_secs_f64() < 2.0,
        "punctuated Chinese conversion too slow: {elapsed:?}"
    );
}

#[tokio::test]
async fn convert_with_progress_can_be_cancelled() {
    use super::super::types::{CancelCheck, ConversionRequest, EngineKind};
    use std::sync::atomic::{AtomicBool, Ordering};
    let service = shared_conversion();
    let flag = std::sync::Arc::new(AtomicBool::new(true));
    let flag2 = std::sync::Arc::clone(&flag);
    let is_cancelled: CancelCheck = std::sync::Arc::new(move || flag2.load(Ordering::SeqCst));
    let text = "里面".repeat(200);
    let error = service
        .convert_with_progress(
            ConversionRequest {
                text,
                direction: Direction::S2t,
                engine: EngineKind::Segmented,
                dictionary_path: None,
                zhconvert: None,
                vocabulary_correction: Some(true),
            },
            None,
            Some(is_cancelled),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "CONVERT_CANCELLED");
}

#[tokio::test]
async fn extra_correction_maps_fanwei_to_fanwei() {
    // extra-correction：範圍,范圍。cn2tw_min 不把單字「范」改成「範」。
    for source in ["范圍", "范围", "範圍"] {
        assert_eq!(
            convert(source, Direction::S2t, EngineKind::Segmented).await,
            "範圍",
            "{source}"
        );
    }
    let simplified = convert("範圍", Direction::T2s, EngineKind::Segmented).await;
    let roundtrip = convert(&simplified, Direction::S2t, EngineKind::Segmented).await;
    assert_eq!(simplified, "范围", "T2S {simplified}");
    assert_eq!(roundtrip, "範圍", "T2S→S2T {simplified} → {roundtrip}");
    assert_eq!(service().segment_tokens("范圍"), vec!["范圍".to_string()]);
    assert_eq!(service().segment_tokens("范围"), vec!["范围".to_string()]);
    let glyph_only = service()
        .convert(ConversionRequest {
            text: "范围".into(),
            direction: Direction::S2t,
            engine: EngineKind::Segmented,
            dictionary_path: None,
            zhconvert: None,
            vocabulary_correction: Some(false),
        })
        .await
        .unwrap();
    assert_eq!(
        glyph_only.text, "范圍",
        "min 表不改單字范：{}",
        glyph_only.text
    );
}

#[test]
fn glyph_s2t_min_leaves_fan_as_fan() {
    use cjk_convert_rs::cn2tw_min;
    assert_eq!(cn2tw_min("范围"), "范圍");
    assert_eq!(super::glyph_s2t("范围"), "范圍");
    assert_eq!(super::glyph_s2t("範圍"), "範圍");
    assert_eq!(super::glyph_s2t("范圍"), "范圍");
}

#[tokio::test]
async fn without_extra_correction_s2t_fanwei() {
    let isolated = super::ConversionService::without_extra_correction(None).expect("service");
    for source in ["范圍", "范围", "範圍"] {
        let text = isolated
            .convert(ConversionRequest {
                text: source.into(),
                direction: Direction::S2t,
                engine: EngineKind::Segmented,
                dictionary_path: None,
                zhconvert: None,
                vocabulary_correction: None,
            })
            .await
            .unwrap()
            .text;
        assert_eq!(
            text,
            "範圍",
            "{source} → {text} tokens={:?}",
            isolated.segment_tokens(source)
        );
    }
}

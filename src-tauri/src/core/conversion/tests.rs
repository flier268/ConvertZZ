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
        ("皇后", "皇后"),
        ("头发", "頭髮"),
        ("开发", "開發"),
        ("面对表面", "面對表面"),
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
    // cn2tw_min → cjk2zht: 里→裡, 钟→鐘, 换→換; 面 stays 面 (not 麵).
    assert_eq!(result.text, "裡面開發面對表面鐘表簡繁轉換");
    assert!(result.warnings[0].contains("詞彙修正已停用"));
}

#[test]
fn glyph_s2t_runs_min_before_zht() {
    // Order matters for 钟: min→鐘, zht alone would yield 鍾.
    assert_eq!(super::glyph_s2t("钟表"), "鐘表");
    assert_eq!(super::glyph_s2t("秒钟"), "秒鐘");
    assert_eq!(super::glyph_s2t("里面"), "裡面");
    assert_eq!(super::glyph_s2t("面对表面"), "面對表面");
    assert_eq!(super::glyph_s2t("简繁转换"), "簡繁轉換");
    assert_eq!(super::glyph_s2t("説明書"), "說明書");
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

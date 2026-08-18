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
async fn vocabulary_off_uses_glyph_only() {
    let result = service()
        .convert(ConversionRequest {
            text: "里面".into(),
            direction: Direction::S2t,
            engine: EngineKind::Segmented,
            dictionary_path: None,
            zhconvert: None,
            vocabulary_correction: Some(false),
        })
        .await
        .unwrap();
    assert_eq!(result.text, "里麵");
    assert!(result.warnings[0].contains("詞彙修正已停用"));
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

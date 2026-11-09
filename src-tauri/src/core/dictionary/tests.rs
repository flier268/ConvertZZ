use super::*;
use std::io::Write;

fn row(
    simplified: &str,
    simplified_priority: i64,
    traditional: &str,
    traditional_priority: i64,
    enabled: bool,
) -> String {
    format!(
        "{}\tTest\t{simplified}\t{simplified_priority}\t{traditional}\t{traditional_priority}",
        if enabled { "True" } else { "False" }
    )
}

fn load(rows: &[&str]) -> LegacyDictionary {
    let directory = tempfile_dir();
    let path = directory.join("Dictionary.csv");
    let mut file = std::fs::File::create(&path).unwrap();
    write!(file, "\u{feff}{}\r\n", rows.join("\r\n")).unwrap();
    LegacyDictionary::load(&path).unwrap()
}

fn tempfile_dir() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("convertzz-dict-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn same_priority_prefers_longer_word() {
    let dictionary = load(&[
        &row("开发", 10, "短詞", 10, true),
        &row("开发者", 10, "長詞", 10, true),
    ]);
    assert_eq!(
        dictionary.replace("开发者", Direction::S2t, |value| value.to_string()),
        "長詞"
    );
}

#[test]
fn higher_priority_beats_longer_word() {
    let dictionary = load(&[
        &row("开发", 100, "優先", 100, true),
        &row("开发者", 10, "長詞", 10, true),
    ]);
    assert_eq!(
        dictionary.replace("开发者", Direction::S2t, |value| value.to_string()),
        "優先者"
    );
}

#[test]
fn protected_words_skip_fallback() {
    let dictionary = load(&[&row("皇后", 9999, "皇后", 9999, true)]);
    assert_eq!(
        dictionary.replace("皇后", Direction::S2t, |value| value.replace('后', "後")),
        "皇后"
    );
}

#[test]
fn disabled_entries_use_fallback() {
    let dictionary = load(&[&row("软件", 100, "軟體", 100, false)]);
    assert_eq!(
        dictionary.replace("软件", Direction::S2t, |_| "軟件".into()),
        "軟件"
    );
}

#[test]
fn preserves_original_indexes_across_blank_lines() {
    let directory = tempfile_dir();
    let path = directory.join("Dictionary.csv");
    std::fs::write(
        &path,
        format!(
            "\u{feff}{}\n\n{}\n",
            row("一", 1, "壹", 1, true),
            row("二", 2, "貳", 2, true)
        ),
    )
    .unwrap();
    let entries = read_dictionary_entries(&path).unwrap();
    assert_eq!(
        entries.iter().map(|entry| entry.index).collect::<Vec<_>>(),
        vec![0, 2]
    );
}

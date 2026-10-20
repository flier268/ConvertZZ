//! 檢查同義詞檔是否出現「左邊簡體、右邊繁體」導向。
//!
//! extra-correction 契約是 `正字,錯字|詞性`（台灣常用寫法在左）。
//! `一个,一個` 這類會把繁體整詞改成簡體，必須反過來。

use super::parse_synonym_entry;
use cjk_convert_rs::{cn2tw, cn2tw_min, table_cn2tw, table_cn2tw_min, tw2cn};
use std::fs;
use std::path::Path;

/// 偵測信心：`min` 只靠套件引擎實際使用的 min 表；`full` 另含一簡多繁，需人工覆核。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SimpTradConfidence {
    Min,
    Full,
}

impl SimpTradConfidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Min => "min",
            Self::Full => "full",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftSimpRightTradHit {
    pub line_number: usize,
    pub canonical: String,
    pub variant: String,
    pub confidence: SimpTradConfidence,
    pub reason: &'static str,
    pub raw_line: String,
    pub suggested_flip: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SynonymOrientationReport {
    pub path: String,
    pub entries_scanned: usize,
    pub hits: Vec<LeftSimpRightTradHit>,
}

/// 判斷 `(左, 右)` 是否像「左簡右繁」。
///
/// - 預設（`full=false`）：`cn2tw_min(左)==右`，或字元對齊後每個差異都在 min 表。
/// - `full=true`：另收 `cn2tw(左)==右`／`tw2cn(右)==左` 等一簡多繁；可能含 `制度,製度` 這類
///   合法回環保護，輸出僅供覆核。
pub fn is_left_simplified_right_traditional(
    left: &str,
    right: &str,
    full: bool,
) -> Option<&'static str> {
    if left == right {
        return None;
    }
    if cn2tw_min(left) == right {
        return Some("cn2tw_min(left)==right");
    }
    if let Some(reason) = char_aligned_min(left, right) {
        return Some(reason);
    }
    if !full {
        return None;
    }
    // full 表有少數雙向鍵（鹹↔咸）。單向 cn2tw(左)=右可直接收；
    // 雙向時只在 tw2cn(右)=左 時視為左簡右繁，避免誤打 `有點鹹,有點咸`。
    if cn2tw(left) == right {
        if cn2tw(right) != left || tw2cn(right) == left {
            return Some("cn2tw(left)==right");
        }
    }
    if tw2cn(right) == left {
        return Some("tw2cn(right)==left");
    }
    char_aligned_full(left, right)
}

fn char_aligned_min(left: &str, right: &str) -> Option<&'static str> {
    let left_chars: Vec<char> = left.chars().collect();
    let right_chars: Vec<char> = right.chars().collect();
    if left_chars.len() != right_chars.len() {
        return None;
    }
    let min = table_cn2tw_min();
    let mut saw = false;
    for (l, r) in left_chars.iter().copied().zip(right_chars.iter().copied()) {
        if l == r {
            continue;
        }
        if min.get(&l) == Some(&r) {
            saw = true;
            continue;
        }
        return None;
    }
    saw.then_some("char-aligned cn2tw_min")
}

fn char_aligned_full(left: &str, right: &str) -> Option<&'static str> {
    let left_chars: Vec<char> = left.chars().collect();
    let right_chars: Vec<char> = right.chars().collect();
    if left_chars.len() != right_chars.len() {
        return None;
    }
    let min = table_cn2tw_min();
    let full = table_cn2tw();
    let mut saw = false;
    for (l, r) in left_chars.iter().copied().zip(right_chars.iter().copied()) {
        if l == r {
            continue;
        }
        if min.get(&l) == Some(&r) {
            saw = true;
            continue;
        }
        if full.get(&l) == Some(&r) {
            let left_glyph = l.to_string();
            let right_glyph = r.to_string();
            // 雙向鍵（鹹↔咸）改由整詞規則判斷，字元對齊直接放棄。
            if cn2tw(&right_glyph) == left_glyph {
                return None;
            }
            saw = true;
            continue;
        }
        return None;
    }
    saw.then_some("char-aligned cn2tw")
}

fn confidence_for_reason(reason: &str) -> SimpTradConfidence {
    if reason.starts_with("cn2tw_min") || reason.starts_with("char-aligned cn2tw_min") {
        SimpTradConfidence::Min
    } else {
        SimpTradConfidence::Full
    }
}

fn suggested_flip_line(
    canonical: &str,
    flagged_variant: &str,
    variants: &[String],
    raw_line: &str,
) -> String {
    let pos = raw_line
        .rsplit_once('|')
        .map(|(_, pos)| pos.trim())
        .filter(|pos| !pos.is_empty());
    let mut parts = Vec::with_capacity(1 + variants.len());
    // 把被標到的繁體 variant 提到正字，原正字改為錯字；其餘錯字保留。
    parts.push(flagged_variant.to_string());
    parts.push(canonical.to_string());
    for variant in variants {
        if variant != flagged_variant {
            parts.push(variant.clone());
        }
    }
    let body = parts.join(",");
    match pos {
        Some(pos) => format!("{body}|{pos}"),
        None => body,
    }
}

/// 掃描同義詞檔，回報疑似左簡右繁條目。
pub fn audit_synonym_orientation(
    path: &Path,
    full: bool,
) -> Result<SynonymOrientationReport, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("無法讀取同義詞 {}：{error}", path.display()))?;
    let mut hits = Vec::new();
    let mut entries_scanned = 0usize;
    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let Some(entry) = parse_synonym_entry(line) else {
            continue;
        };
        entries_scanned += 1;
        for variant in &entry.variants {
            let Some(reason) =
                is_left_simplified_right_traditional(&entry.canonical, variant, full)
            else {
                continue;
            };
            let confidence = confidence_for_reason(reason);
            if !full && confidence == SimpTradConfidence::Full {
                continue;
            }
            hits.push(LeftSimpRightTradHit {
                line_number,
                canonical: entry.canonical.clone(),
                variant: variant.clone(),
                confidence,
                reason,
                raw_line: line.trim().to_string(),
                suggested_flip: suggested_flip_line(
                    &entry.canonical,
                    variant,
                    &entry.variants,
                    line.trim(),
                ),
            });
        }
    }
    Ok(SynonymOrientationReport {
        path: path.display().to_string(),
        entries_scanned,
        hits,
    })
}

/// 接受檔案，或含 `zht.corpus.synonym.txt` 的目錄。
pub fn resolve_synonym_path(path: &Path) -> Result<std::path::PathBuf, String> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }
    let candidate = path.join("zht.corpus.synonym.txt");
    if candidate.is_file() {
        return Ok(candidate);
    }
    Err(format!(
        "找不到同義詞檔：{}（可傳檔案或含 zht.corpus.synonym.txt 的目錄）",
        path.display()
    ))
}

pub fn format_orientation_report(report: &SynonymOrientationReport) -> String {
    let mut out = String::new();
    out.push_str("# synonym orientation audit\n");
    out.push_str(&format!("# path: {}\n", report.path));
    out.push_str(&format!("# entries: {}\n", report.entries_scanned));
    out.push_str(&format!("# hits: {}\n", report.hits.len()));
    out.push_str("line\tconfidence\treason\tcanonical\tvariant\tsuggested_flip\traw\n");
    for hit in &report.hits {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            hit.line_number,
            hit.confidence.as_str(),
            hit.reason,
            hit.canonical,
            hit.variant,
            hit.suggested_flip,
            hit.raw_line
        ));
    }
    out
}

pub const ORIENTATION_MIN_REPORT: &str = "synonym-orientation-min.tsv";
pub const ORIENTATION_FULL_REPORT: &str = "synonym-orientation-full.tsv";

/// 對已寫入的 `zht.corpus.synonym.txt` 產出 min／full 兩份導向檢查報告。
pub fn write_orientation_audit_reports(output_dir: &Path) -> Result<(usize, usize), String> {
    let synonym = output_dir.join("zht.corpus.synonym.txt");
    if !synonym.is_file() {
        return Err(format!(
            "缺少同義詞產出，無法做導向檢查：{}",
            synonym.display()
        ));
    }
    let min_report = audit_synonym_orientation(&synonym, false)?;
    let full_report = audit_synonym_orientation(&synonym, true)?;
    super::checkpoint::atomic_write(
        &output_dir.join(ORIENTATION_MIN_REPORT),
        format_orientation_report(&min_report).as_bytes(),
    )?;
    super::checkpoint::atomic_write(
        &output_dir.join(ORIENTATION_FULL_REPORT),
        format_orientation_report(&full_report).as_bytes(),
    )?;
    Ok((min_report.hits.len(), full_report.hits.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn min_mode_flags_clear_simplified_canonical() {
        assert_eq!(
            is_left_simplified_right_traditional("一个", "一個", false),
            Some("cn2tw_min(left)==right")
        );
        assert_eq!(
            is_left_simplified_right_traditional("倾向於", "傾向於", false),
            Some("cn2tw_min(left)==right")
        );
    }

    #[test]
    fn min_mode_keeps_roundtrip_protection_and_trad_variants() {
        assert_eq!(
            is_left_simplified_right_traditional("制度", "製度", false),
            None
        );
        assert_eq!(
            is_left_simplified_right_traditional("裡面", "裏面", false),
            None
        );
        assert_eq!(
            is_left_simplified_right_traditional("乾了", "幹了", false),
            None
        );
        assert_eq!(
            is_left_simplified_right_traditional("一個", "一个", false),
            None
        );
    }

    #[test]
    fn full_mode_flags_plus_table_pairs_for_review() {
        assert_eq!(
            is_left_simplified_right_traditional("于是", "於是", true),
            Some("cn2tw(left)==right")
        );
        assert_eq!(
            is_left_simplified_right_traditional("日志", "日誌", true),
            Some("tw2cn(right)==left")
        );
        // 一簡多繁：full 會抓到，供人工判斷（預設 min 模式不抓）
        assert_eq!(
            is_left_simplified_right_traditional("制度", "製度", true),
            Some("cn2tw(left)==right")
        );
        // 正確正字在左、簡體／異體在右；full 表雙向鍵不可誤報
        assert_eq!(
            is_left_simplified_right_traditional("有點鹹", "有點咸", true),
            None
        );
        assert_eq!(
            is_left_simplified_right_traditional("咸酚辛", "鹹酚辛", true),
            Some("cn2tw(left)==right")
        );
    }

    #[test]
    fn audit_file_reports_hit_and_suggested_flip() {
        let dir = std::env::temp_dir().join(format!(
            "convertzz-synonym-audit-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("zht.corpus.synonym.txt");
        let mut file = fs::File::create(&path).unwrap();
        writeln!(file, "// comment").unwrap();
        writeln!(file, "一個,一个|D_MQ").unwrap();
        writeln!(file, "一个,一個|D_MQ").unwrap();
        writeln!(file, "制度,製度|D_N").unwrap();
        let report = audit_synonym_orientation(&path, false).unwrap();
        assert_eq!(report.entries_scanned, 3);
        assert_eq!(report.hits.len(), 1);
        assert_eq!(report.hits[0].canonical, "一个");
        assert_eq!(report.hits[0].variant, "一個");
        assert_eq!(report.hits[0].suggested_flip, "一個,一个|D_MQ");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_orientation_reports_min_and_full() {
        let dir = std::env::temp_dir().join(format!(
            "convertzz-synonym-audit-write-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("zht.corpus.synonym.txt");
        fs::write(&path, "一个,一個|D_MQ\n制度,製度|D_N\n于是,於是|D_C\n").unwrap();
        let (min_hits, full_hits) = write_orientation_audit_reports(&dir).unwrap();
        assert_eq!(min_hits, 1);
        assert!(full_hits >= 2, "full should include min plus plus-table");
        let min_text = fs::read_to_string(dir.join(ORIENTATION_MIN_REPORT)).unwrap();
        let full_text = fs::read_to_string(dir.join(ORIENTATION_FULL_REPORT)).unwrap();
        assert!(min_text.contains("一个"));
        assert!(!min_text.contains("制度"));
        assert!(full_text.contains("一个"));
        assert!(full_text.contains("于是") || full_text.contains("制度"));
        let _ = fs::remove_dir_all(&dir);
    }
}

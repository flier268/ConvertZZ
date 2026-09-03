//! 把 roundtrip 產出合併進 extra-correction，不整包覆蓋。
//!
//! extra 既有正字優先（不把 `機制,機製` 翻成 `機製,機制`）。
//! 新正詞與新錯詞才追加。分詞表聯集後再寫入 conversion-specials 保護詞（含 xx鄉／xx里）。

use super::{
    atomic_write, format_synonym_file, is_extra_correction_path, is_package_data_path,
    parse_synonym_entry, pinned_dict_words, CorrectionEntry,
};
use novel_segment::POSTAG;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MergeStats {
    pub synonym_entries_kept: usize,
    pub synonym_entries_added: usize,
    pub synonym_variants_added: usize,
    pub dict_rows_kept: usize,
    pub dict_rows_added: usize,
}

pub fn merge_extra_correction(from: &Path, into: &Path) -> Result<MergeStats, String> {
    assert_merge_paths(from, into)?;
    let incoming_synonym = read_required(from.join("zht.corpus.synonym.txt"))?;
    let incoming_dict = read_required(from.join("zht.corpus.dict.txt"))?;
    let extra_synonym = read_required(into.join("zht.corpus.synonym.txt"))?;
    let extra_dict = match fs::read_to_string(into.join("zht.corpus.dict.txt")) {
        Ok(text) => text,
        Err(_) => String::new(),
    };

    let (synonym_entries, synonym_stats) = merge_synonym_entries(&extra_synonym, &incoming_synonym);
    let pos_of = |word: &str| {
        synonym_entries
            .iter()
            .find(|entry| entry.canonical == word)
            .map(|entry| entry.pos)
            .filter(|pos| *pos != 0)
            .unwrap_or(POSTAG::D_N)
    };
    let synonym_text = format_synonym_file(
        &synonym_entries
            .iter()
            .map(|entry| CorrectionEntry {
                canonical: entry.canonical.clone(),
                variants: entry
                    .variants
                    .iter()
                    .map(|item| (item.clone(), 1))
                    .collect(),
            })
            .collect::<Vec<_>>(),
        &pos_of,
    );

    let (dict_entries, dict_stats) = merge_dict_rows(&extra_dict, &incoming_dict);
    let dict_text = format_merged_dict(&dict_entries);

    atomic_write(
        &into.join("zht.corpus.synonym.txt"),
        synonym_text.as_bytes(),
    )?;
    atomic_write(&into.join("zht.corpus.dict.txt"), dict_text.as_bytes())?;

    Ok(MergeStats {
        synonym_entries_kept: synonym_stats.0,
        synonym_entries_added: synonym_stats.1,
        synonym_variants_added: synonym_stats.2,
        dict_rows_kept: dict_stats.0,
        dict_rows_added: dict_stats.1,
    })
}

fn assert_merge_paths(from: &Path, into: &Path) -> Result<(), String> {
    if is_package_data_path(from) || is_package_data_path(into) {
        return Err("不可合併進分詞或簡轉繁套件資料。".into());
    }
    if !is_extra_correction_path(into) {
        return Err("合併目標必須是 extra-correction 目錄。".into());
    }
    if is_extra_correction_path(from) {
        return Err("合併來源不可是 extra-correction（請用 roundtrip 產出目錄）。".into());
    }
    if !from.is_dir() {
        return Err(format!("合併來源不存在：{}", from.display()));
    }
    if !into.is_dir() {
        return Err(format!("合併目標不存在：{}", into.display()));
    }
    Ok(())
}

fn read_required(path: std::path::PathBuf) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|error| format!("無法讀取 {}：{error}", path.display()))
}

#[derive(Clone, Debug)]
struct MergeSynonym {
    canonical: String,
    variants: Vec<String>,
    pos: u32,
}

fn merge_synonym_entries(
    extra: &str,
    incoming: &str,
) -> (Vec<MergeSynonym>, (usize, usize, usize)) {
    let mut entries: Vec<MergeSynonym> = Vec::new();
    let mut owner: HashMap<String, String> = HashMap::new();
    for line in extra.lines() {
        let Some(entry) = parse_synonym_entry(line) else {
            continue;
        };
        register_owner(&mut owner, &entry.canonical, &entry.canonical);
        for variant in &entry.variants {
            register_owner(&mut owner, variant, &entry.canonical);
        }
        entries.push(MergeSynonym {
            canonical: entry.canonical,
            variants: entry.variants,
            pos: entry.pos,
        });
    }
    let kept = entries.len();
    let mut added_entries = 0usize;
    let mut added_variants = 0usize;
    for line in incoming.lines() {
        let Some(entry) = parse_synonym_entry(line) else {
            continue;
        };
        if let Some(existing) = entries
            .iter_mut()
            .find(|item| item.canonical == entry.canonical)
        {
            for variant in entry.variants {
                if variant == existing.canonical {
                    continue;
                }
                if let Some(owned) = owner.get(&variant) {
                    if owned != &existing.canonical {
                        continue;
                    }
                }
                if existing.variants.iter().any(|item| item == &variant) {
                    continue;
                }
                register_owner(&mut owner, &variant, &existing.canonical);
                existing.variants.push(variant);
                added_variants += 1;
            }
            continue;
        }
        if owner
            .get(&entry.canonical)
            .is_some_and(|owned| owned != &entry.canonical)
        {
            continue;
        }
        let mut variants = Vec::new();
        for variant in entry.variants {
            if variant == entry.canonical {
                continue;
            }
            if owner
                .get(&variant)
                .is_some_and(|owned| owned != &entry.canonical)
            {
                continue;
            }
            variants.push(variant);
        }
        if variants.is_empty() {
            continue;
        }
        register_owner(&mut owner, &entry.canonical, &entry.canonical);
        for variant in &variants {
            register_owner(&mut owner, variant, &entry.canonical);
        }
        added_variants += variants.len();
        added_entries += 1;
        entries.push(MergeSynonym {
            canonical: entry.canonical,
            variants,
            pos: entry.pos,
        });
    }
    (entries, (kept, added_entries, added_variants))
}

fn register_owner(owner: &mut HashMap<String, String>, key: &str, canonical: &str) {
    owner
        .entry(key.to_string())
        .or_insert_with(|| canonical.to_string());
}

struct DictRow {
    word: String,
    pos: u32,
    freq: u64,
}

fn merge_dict_rows(extra: &str, incoming: &str) -> (Vec<DictRow>, (usize, usize)) {
    let mut rows = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for line in extra.lines() {
        let Some((word, pos, freq)) = parse_dict_row(line) else {
            continue;
        };
        if index.contains_key(&word) {
            continue;
        }
        index.insert(word.clone(), rows.len());
        rows.push(DictRow { word, pos, freq });
    }
    let kept = rows.len();
    for line in incoming.lines() {
        let Some((word, pos, freq)) = parse_dict_row(line) else {
            continue;
        };
        if let Some(&idx) = index.get(&word) {
            rows[idx].freq = rows[idx].freq.max(freq);
            continue;
        }
        index.insert(word.clone(), rows.len());
        rows.push(DictRow { word, pos, freq });
    }
    let added = rows.len().saturating_sub(kept);
    (rows, (kept, added))
}

fn parse_dict_row(line: &str) -> Option<(String, u32, u64)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
        return None;
    }
    let mut parts = line.split('|');
    let word = parts.next()?.trim();
    if word.is_empty() {
        return None;
    }
    let pos = super::parse_pos_mask(parts.next()?.trim())?;
    let freq = parts.next()?.trim().parse().ok()?;
    Some((word.to_string(), pos, freq))
}

fn format_merged_dict(rows: &[DictRow]) -> String {
    let mut output = String::from(
        "// ConvertZZ 額外分詞表（詞|詞性|權值）。正字、錯字與其簡體詞形都寫入。不得併入套件檔。\n",
    );
    let mut seen: HashSet<String> = HashSet::new();
    for row in rows {
        if !seen.insert(row.word.clone()) {
            continue;
        }
        output.push_str(&format!("{}|{:#x}|{}\n", row.word, row.pos, row.freq));
    }
    for (word, pos, freq) in pinned_dict_words() {
        if seen.insert(word.clone()) {
            output.push_str(&format!("{word}|{pos:#x}|{freq}\n"));
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_pair() -> (std::path::PathBuf, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "czrt-merge-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let from = root.join("roundtrip-correction");
        let into = root.join("extra-correction");
        fs::create_dir_all(&from).unwrap();
        fs::create_dir_all(&into).unwrap();
        (from, into)
    }

    #[test]
    fn merge_adds_new_pairs_and_keeps_existing_orientation() {
        let (from, into) = temp_pair();
        fs::write(
            into.join("zht.corpus.synonym.txt"),
            "機制,機製|D_N\n拮据,拮據|D_A\n",
        )
        .unwrap();
        fs::write(
            into.join("zht.corpus.dict.txt"),
            "機制|0x100000|9\n拮据|0x40000000|17\n",
        )
        .unwrap();
        fs::write(
            from.join("zht.corpus.synonym.txt"),
            "機製,機制|D_N\n本里,本裡|D_N\n里辦,裡辦|D_N\n拮据,拮據|D_A\n",
        )
        .unwrap();
        fs::write(
            from.join("zht.corpus.dict.txt"),
            "本里|0x100000|8\n本裡|0x100000|8\n里辦|0x100000|8\n",
        )
        .unwrap();

        let stats = merge_extra_correction(&from, &into).unwrap();
        assert_eq!(stats.synonym_entries_kept, 2);
        assert_eq!(stats.synonym_entries_added, 2);
        let synonym = fs::read_to_string(into.join("zht.corpus.synonym.txt")).unwrap();
        assert!(synonym.contains("機制,機製|D_N\n"), "{synonym}");
        assert!(!synonym.contains("機製,機制"), "{synonym}");
        assert!(synonym.contains("本里,本裡|D_N\n"), "{synonym}");
        assert!(synonym.contains("里辦,裡辦|D_N\n"), "{synonym}");
        assert!(synonym.contains("拮据,拮據|D_A\n"), "{synonym}");
        let dict = fs::read_to_string(into.join("zht.corpus.dict.txt")).unwrap();
        assert!(dict.contains("本里|0x100000|8\n"), "{dict}");
        assert!(dict.contains("和牛|0x100000|1000\n"), "{dict}");
        let _ = fs::remove_dir_all(from.parent().unwrap());
    }

    #[test]
    fn merge_rejects_package_and_extra_as_source() {
        let extra = Path::new("/tmp/app/extra-correction");
        let err = assert_merge_paths(extra, extra).unwrap_err();
        assert!(err.contains("extra-correction"), "{err}");
        let pkg = Path::new("/tmp/segment-dict");
        let err = assert_merge_paths(pkg, extra).unwrap_err();
        assert!(err.contains("套件"), "{err}");
    }
}

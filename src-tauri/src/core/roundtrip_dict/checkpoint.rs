use super::aggregator::SHARD_FORMAT;
use super::{default_segment_dict_root, CorpusSelect};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileFingerprint {
    pub path: String,
    pub len: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SynonymFingerprint {
    pub name: String,
    pub len: Option<u64>,
    pub mtime: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtraCorrectionFingerprint {
    pub path: String,
    pub files: Vec<SynonymFingerprint>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Fingerprint {
    pub sources: String,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub format: String,
    pub files: Vec<FileFingerprint>,
    pub synonym: Vec<SynonymFingerprint>,
    #[serde(default)]
    pub extra_correction: Option<ExtraCorrectionFingerprint>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub version: u32,
    pub status: String,
    pub fingerprint: Fingerprint,
    #[serde(rename = "completedFiles")]
    pub completed_files: Vec<String>,
    #[serde(rename = "linesRead")]
    pub lines_read: u64,
    #[serde(rename = "linesSkipped")]
    pub lines_skipped: u64,
    #[serde(rename = "linesMismatched")]
    pub lines_mismatched: u64,
    pub shards: Vec<String>,
    #[serde(rename = "lastHardFile")]
    pub last_hard_file: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

impl Checkpoint {
    pub fn new(fingerprint: Fingerprint) -> Self {
        Self {
            version: 1,
            status: "running".into(),
            fingerprint,
            completed_files: Vec::new(),
            lines_read: 0,
            lines_skipped: 0,
            lines_mismatched: 0,
            shards: Vec::new(),
            last_hard_file: None,
            updated_at: now_rfc3339(),
        }
    }
}

pub fn checkpoint_path(output: &Path) -> PathBuf {
    output.join("checkpoint.json")
}

pub fn load_checkpoint(output: &Path) -> Result<Option<Checkpoint>, String> {
    let path = checkpoint_path(output);
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path).map_err(|error| format!("無法讀取檢查點：{error}"))?;
    let checkpoint: Checkpoint =
        serde_json::from_str(&text).map_err(|error| format!("檢查點損壞：{error}"))?;
    if checkpoint.version != 1 {
        return Err("檢查點版本不符，請使用 --reset。".into());
    }
    Ok(Some(checkpoint))
}

pub fn save_checkpoint(output: &Path, checkpoint: &mut Checkpoint) -> Result<(), String> {
    checkpoint.updated_at = now_rfc3339();
    let text = serde_json::to_string_pretty(checkpoint)
        .map_err(|error| format!("無法序列化檢查點：{error}"))?;
    atomic_write(&checkpoint_path(output), format!("{text}\n").as_bytes())
}

pub fn build_fingerprint(
    sources: &Path,
    select: &CorpusSelect,
    files: &[PathBuf],
    extra_correction: Option<&Path>,
) -> Result<Fingerprint, String> {
    let sources_abs = super::normalize_for_compare(sources);
    let mut file_prints = Vec::new();
    for path in files {
        let relative = path
            .strip_prefix(sources)
            .or_else(|_| path.strip_prefix(&sources_abs))
            .map(|item| item.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| path.display().to_string().replace('\\', "/"));
        if relative.contains("..") || Path::new(&relative).is_absolute() {
            return Err(format!("拒絕不安全的語料相對路徑：{relative}"));
        }
        let len = fs::metadata(path)
            .map_err(|error| format!("無法讀取 {}：{error}", path.display()))?
            .len();
        file_prints.push(FileFingerprint {
            path: relative,
            len,
        });
    }
    let mut include = select.include.clone();
    let mut exclude = select.exclude.clone();
    include.sort();
    include.dedup();
    exclude.sort();
    exclude.dedup();
    Ok(Fingerprint {
        sources: sources_abs.to_string_lossy().into_owned(),
        include,
        exclude,
        format: SHARD_FORMAT.to_string(),
        files: file_prints,
        synonym: synonym_fingerprints(),
        extra_correction: extra_correction_fingerprint(extra_correction)?,
    })
}

pub fn fingerprint_mismatch_message() -> &'static str {
    "檢查點與目前來源、套件詞典或 extra-correction 不符，請使用 --reset。"
}

fn synonym_fingerprints() -> Vec<SynonymFingerprint> {
    file_fingerprints(
        &default_segment_dict_root().join("synonym"),
        &["synonym.txt", "zht.synonym.txt", "zht.common.synonym.txt"],
    )
}

fn extra_correction_fingerprint(
    extra_correction: Option<&Path>,
) -> Result<Option<ExtraCorrectionFingerprint>, String> {
    let Some(root) = extra_correction else {
        return Ok(None);
    };
    let path = super::normalize_for_compare(root);
    Ok(Some(ExtraCorrectionFingerprint {
        path: path.to_string_lossy().into_owned(),
        files: file_fingerprints(&path, &["zht.corpus.synonym.txt", "zht.corpus.dict.txt"]),
    }))
}

fn file_fingerprints(root: &Path, names: &[&str]) -> Vec<SynonymFingerprint> {
    names
        .iter()
        .map(|name| {
            let path = root.join(name);
            match fs::metadata(&path) {
                Ok(meta) => SynonymFingerprint {
                    name: (*name).to_string(),
                    len: Some(meta.len()),
                    mtime: meta
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
                        .map(|duration| duration.as_secs()),
                },
                Err(_) => SynonymFingerprint {
                    name: (*name).to_string(),
                    len: None,
                    mtime: None,
                },
            }
        })
        .collect()
}

pub fn validate_completed_paths(sources: &Path, completed: &[String]) -> Result<(), String> {
    let sources_abs = super::normalize_for_compare(sources);
    for relative in completed {
        if relative.contains("..") || Path::new(relative).is_absolute() {
            return Err(format!("檢查點含不安全路徑：{relative}"));
        }
        let joined = sources_abs.join(relative);
        if !joined.starts_with(&sources_abs) {
            return Err(format!("檢查點路徑超出來源：{relative}"));
        }
    }
    Ok(())
}

pub fn derived_files() -> &'static [&'static str] {
    &[
        "zht.corpus.synonym.txt",
        "zht.corpus.dict.txt",
        "pairs.tsv",
        "report.json",
        super::ORIENTATION_MIN_REPORT,
        super::ORIENTATION_FULL_REPORT,
    ]
}

pub fn reset_output(output: &Path) -> Result<(), String> {
    let _ = fs::remove_file(checkpoint_path(output));
    let _ = fs::remove_dir_all(output.join("state"));
    for name in derived_files() {
        let _ = fs::remove_file(output.join(name));
    }
    Ok(())
}

pub fn wipe_uncommitted(output: &Path) -> Result<(), String> {
    let dir = output.join("state/uncommitted");
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|error| format!("無法清除 uncommitted：{error}"))?;
    }
    fs::create_dir_all(&dir).map_err(|error| format!("無法建立 uncommitted：{error}"))
}

pub fn percent_encode_path(relative: &str) -> String {
    let mut out = String::new();
    for byte in relative.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

pub fn uncommitted_name(relative: &str, seq: u32) -> String {
    format!("{}__{seq:06}.pairs", percent_encode_path(relative))
}

pub fn uncommitted_matches(name: &str, relative: &str) -> bool {
    let prefix = format!("{}__", percent_encode_path(relative));
    name.starts_with(&prefix) && name.ends_with(".pairs")
}

pub fn list_uncommitted_for(output: &Path, relative: &str) -> Result<Vec<PathBuf>, String> {
    let dir = output.join("state/uncommitted");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|error| format!("無法讀取 uncommitted：{error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("無法讀取 uncommitted：{error}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if uncommitted_matches(&name, relative) {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

pub fn tmp_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| format!("{}.tmp", name.to_string_lossy()))
        .unwrap_or_else(|| "file.tmp".into());
    path.with_file_name(name)
}

#[cfg(windows)]
pub fn bak_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| format!("{}.bak", name.to_string_lossy()))
        .unwrap_or_else(|| "file.bak".into());
    path.with_file_name(name)
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("無法建立 {}：{error}", parent.display()))?;
    }
    let tmp = tmp_path(path);
    {
        let mut file =
            File::create(&tmp).map_err(|error| format!("無法建立 {}：{error}", tmp.display()))?;
        file.write_all(bytes)
            .map_err(|error| format!("寫入 {} 失敗：{error}", tmp.display()))?;
        file.sync_all()
            .map_err(|error| format!("sync {} 失敗：{error}", tmp.display()))?;
    }
    replace_file(&tmp, path)
}

pub fn replace_file(tmp: &Path, dest: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let bak = bak_path(dest);
        if dest.exists() {
            let _ = fs::remove_file(&bak);
            fs::rename(dest, &bak).map_err(|error| format!("備份舊檔失敗：{error}"))?;
            if let Err(error) = fs::rename(tmp, dest) {
                let _ = fs::rename(&bak, dest);
                let _ = fs::remove_file(tmp);
                return Err(format!("取代檔案失敗：{error}"));
            }
            let _ = fs::remove_file(&bak);
        } else if let Err(error) = fs::rename(tmp, dest) {
            let _ = fs::remove_file(tmp);
            return Err(format!("寫入檔案失敗：{error}"));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        if let Err(error) = fs::rename(tmp, dest) {
            let _ = fs::remove_file(tmp);
            return Err(format!("寫入 {} 失敗：{error}", dest.display()));
        }
        Ok(())
    }
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

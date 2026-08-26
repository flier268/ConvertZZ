use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

pub type ProgressReporter = Arc<dyn Fn(ProgressEvent) + Send + Sync>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub current: u64,
    pub total: u64,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EngineKind {
    Segmented,
    Legacy,
    Zhconvert,
}

impl Default for EngineKind {
    fn default() -> Self {
        Self::Segmented
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    None,
    S2t,
    T2s,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AudioContainer {
    Id3v1,
    Id3v2,
    Apev2,
    VorbisComment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictPolicy {
    Skip,
    Overwrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextEncoding {
    Auto,
    Utf8,
    #[serde(rename = "utf8-bom")]
    Utf8Bom,
    Utf16le,
    Utf16be,
    Big5,
    Gbk,
    #[serde(rename = "shift-jis")]
    ShiftJis,
    #[serde(rename = "euc-jp")]
    EucJp,
    #[serde(rename = "iso-2022-jp")]
    Iso2022Jp,
    #[serde(rename = "hz-gb-2312")]
    HzGb2312,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionRequest {
    pub text: String,
    pub direction: Direction,
    pub engine: EngineKind,
    #[serde(default)]
    pub dictionary_path: Option<String>,
    #[serde(default)]
    pub zhconvert: Option<ZhConvertOptions>,
    #[serde(default)]
    pub vocabulary_correction: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionOptions {
    pub direction: Direction,
    pub engine: EngineKind,
    #[serde(default)]
    pub dictionary_path: Option<String>,
    #[serde(default)]
    pub zhconvert: Option<ZhConvertOptions>,
    #[serde(default)]
    pub vocabulary_correction: Option<bool>,
}

impl ConversionOptions {
    pub fn with_text(&self, text: impl Into<String>) -> ConversionRequest {
        ConversionRequest {
            text: text.into(),
            direction: self.direction,
            engine: self.engine,
            dictionary_path: self.dictionary_path.clone(),
            zhconvert: self.zhconvert.clone(),
            vocabulary_correction: self.vocabulary_correction,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionResult {
    pub text: String,
    pub engine: EngineKind,
    pub direction: Direction,
    pub warnings: Vec<String>,
    pub duration_ms: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZhConvertOptions {
    #[serde(default)]
    pub converter: Option<String>,
    #[serde(default)]
    pub modules: Option<ZhConvertModules>,
    #[serde(default)]
    pub jp_text_conversion_strategy: Option<String>,
    #[serde(default)]
    pub jp_style_conversion_strategy: Option<String>,
    #[serde(default)]
    pub clean_up_text: Option<bool>,
    #[serde(default)]
    pub user_pre_replace: Option<String>,
    #[serde(default)]
    pub user_post_replace: Option<String>,
    #[serde(default)]
    pub user_protect_replace: Option<String>,
    #[serde(default)]
    pub ensure_newline_at_eof: Option<bool>,
    #[serde(default)]
    pub translate_tabs_to_spaces: Option<i64>,
    #[serde(default)]
    pub trim_trailing_white_spaces: Option<bool>,
    #[serde(default)]
    pub unify_leading_hyphen: Option<bool>,
    #[serde(default)]
    pub ignore_text_styles: Option<String>,
    #[serde(default)]
    pub jp_text_styles: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ZhConvertModules {
    Map(HashMap<String, i8>),
    List(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePlanRequest {
    pub paths: Vec<String>,
    #[serde(default)]
    pub output_path: Option<String>,
    #[serde(default)]
    pub output_directory: Option<String>,
    pub mode: FileMode,
    pub recursive: bool,
    pub input_encoding: TextEncoding,
    pub output_encoding: TextEncoding,
    pub add_bom: bool,
    pub fix_charset_declaration: bool,
    #[serde(default)]
    pub fix_charset_extensions: Option<Vec<String>>,
    #[serde(default)]
    pub allowed_extensions: Option<Vec<String>>,
    #[serde(default)]
    pub preview_max_bytes: Option<u64>,
    pub conflict_policy: ConflictPolicy,
    #[serde(default)]
    pub backup: Option<bool>,
    pub conversion: ConversionOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileMode {
    Content,
    Filename,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePlanItem {
    pub source_path: String,
    pub output_path: String,
    pub kind: FileItemKind,
    pub selected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_encoding: Option<TextEncoding>,
    pub source_preview: String,
    pub output_preview: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub preview_loaded: bool,
    pub status: PlanStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePreviewRequest {
    pub plan_id: String,
    pub source_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileItemKind {
    File,
    Directory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlanStatus {
    Ready,
    Skipped,
    Conflict,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileConversionPlan {
    pub plan_id: String,
    pub created_at: String,
    pub items: Vec<FilePlanItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTagField {
    pub key: String,
    pub label: String,
    pub container: AudioContainer,
    pub values: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub converted_values: Option<Vec<String>>,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTagFile {
    pub path: String,
    pub format: AudioFormat,
    pub selected: bool,
    pub fields: Vec<AudioTagField>,
    pub has_cover_art: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    Mp3,
    Ape,
    Ogg,
    Opus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioScanRequest {
    pub paths: Vec<String>,
    #[serde(default)]
    pub recursive: Option<bool>,
    #[serde(default)]
    pub id3v1_source_encoding: Option<TextEncoding>,
    #[serde(default)]
    pub id3v2_source_encoding: Option<TextEncoding>,
    #[serde(default)]
    pub id3v2_repair_source_encoding: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTagPlanRequest {
    pub paths: Vec<String>,
    #[serde(default)]
    pub recursive: Option<bool>,
    #[serde(default)]
    pub id3v1_source_encoding: Option<TextEncoding>,
    #[serde(default)]
    pub id3v2_source_encoding: Option<TextEncoding>,
    #[serde(default)]
    pub id3v2_repair_source_encoding: Option<bool>,
    pub selected_paths: Vec<String>,
    pub selected_fields: HashMap<String, Vec<String>>,
    pub conversion: ConversionOptions,
    pub conflict_policy: ConflictPolicy,
    #[serde(default)]
    pub backup: Option<bool>,
    pub id3v1_enabled: bool,
    pub id3v1_direction: Direction,
    #[serde(default)]
    pub id3v1_zhconvert: Option<ZhConvertOptions>,
    pub id3v1_output_encoding: TextEncoding,
    pub id3v2_enabled: bool,
    pub id3v2_direction: Direction,
    #[serde(default)]
    pub id3v2_zhconvert: Option<ZhConvertOptions>,
    pub id3v2_version: u8,
    pub id3v2_encoding: String,
}

impl AudioTagPlanRequest {
    pub fn scan_request(&self) -> AudioScanRequest {
        AudioScanRequest {
            paths: self.paths.clone(),
            recursive: self.recursive,
            id3v1_source_encoding: self.id3v1_source_encoding,
            id3v2_source_encoding: self.id3v2_source_encoding,
            id3v2_repair_source_encoding: self.id3v2_repair_source_encoding,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTagPlan {
    pub plan_id: String,
    pub created_at: String,
    pub files: Vec<AudioTagFile>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub succeeded: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<ApplyFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyFailure {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedCli {
    pub mode: CliMode,
    pub paths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    pub input_encoding: TextEncoding,
    pub output_encoding: TextEncoding,
    pub direction: Direction,
    pub engine: EngineKind,
    pub operation: FileMode,
    pub vocabulary_correction: VocabularyCorrection,
    pub backup: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CliMode {
    File,
    Audio,
    Interactive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VocabularyCorrection {
    Settings,
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UtilityConvertRequest {
    pub kind: UtilityKind,
    pub text: String,
    #[serde(default)]
    pub source_encoding: Option<TextEncoding>,
    #[serde(default)]
    pub target_encoding: Option<TextEncoding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UtilityKind {
    HtmlDecimalEncode,
    HtmlDecimalDecode,
    HtmlHexEncode,
    HtmlHexDecode,
    UnicodeEscapeEncode,
    UnicodeEscapeDecode,
    Encoding,
    Fullwidth,
    Halfwidth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryEntry {
    pub enabled: bool,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub simplified: String,
    pub simplified_priority: i64,
    pub traditional: String,
    pub traditional_priority: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedDictionaryEntry {
    pub index: usize,
    pub enabled: bool,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub simplified: String,
    pub simplified_priority: i64,
    pub traditional: String,
    pub traditional_priority: i64,
}

impl From<IndexedDictionaryEntry> for DictionaryEntry {
    fn from(entry: IndexedDictionaryEntry) -> Self {
        Self {
            enabled: entry.enabled,
            entry_type: entry.entry_type,
            simplified: entry.simplified,
            simplified_priority: entry.simplified_priority,
            traditional: entry.traditional,
            traditional_priority: entry.traditional_priority,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryReadRequest {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub offset: Option<usize>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub sort: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryUpdateRequest {
    pub path: String,
    #[serde(default)]
    pub updates: Option<Vec<DictionaryUpdate>>,
    #[serde(default)]
    pub inserts: Option<Vec<DictionaryEntry>>,
    #[serde(default)]
    pub deletes: Option<Vec<usize>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryUpdate {
    pub index: usize,
    pub entry: DictionaryEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryPreviewRequest {
    #[serde(default)]
    pub path: Option<String>,
    pub text: String,
    pub direction: Direction,
    #[serde(default)]
    pub updates: Option<Vec<DictionaryUpdate>>,
    #[serde(default)]
    pub inserts: Option<Vec<DictionaryEntry>>,
    #[serde(default)]
    pub deletes: Option<Vec<usize>>,
}

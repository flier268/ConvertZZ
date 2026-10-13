use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

pub type ProgressReporter = Arc<dyn Fn(ProgressEvent) + Send + Sync>;
pub type CancelCheck = Arc<dyn Fn() -> bool + Send + Sync>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioProgressBatch {
    pub phase: String,
    pub files: Vec<AudioTagFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub current: u64,
    pub total: u64,
    pub message: String,
    /// 音訊掃描或預覽的一批檔案。其他作業留空，不送出。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch: Option<AudioProgressBatch>,
}

impl ProgressEvent {
    pub fn tick(current: u64, total: u64, message: impl Into<String>) -> Self {
        Self {
            current,
            total,
            message: message.into(),
            batch: None,
        }
    }

    pub fn with_audio_batch(mut self, phase: &str, files: Vec<AudioTagFile>) -> Self {
        self.batch = Some(AudioProgressBatch {
            phase: phase.to_string(),
            files,
        });
        self
    }
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
    /// 省略（`None`）表示所有檔案。空陣列表示資料夾掃描不收任何檔案；直接指定的檔案仍收。
    #[serde(default)]
    pub allowed_extensions: Option<Vec<String>>,
    /// 副檔名篩選（白名單／黑名單／全部，可含無副檔名選項）。存在時優先於 `allowed_extensions`。
    #[serde(default)]
    pub extension_filter: Option<ExtensionFilter>,
    #[serde(default)]
    pub preview_max_bytes: Option<u64>,
    pub conflict_policy: ConflictPolicy,
    #[serde(default)]
    pub backup: Option<bool>,
    /// 直接指定版本控制資料夾（`.git`、`.svn`、`.hg`、`.bzr`）或其中的路徑時是否照樣處理。
    /// 預設 `false`：這些輸入會被略過並列在 `FileConversionPlan::ignored_inputs`。
    /// GUI 經使用者確認、或無頭加上 `--include-ignored` 時才設為 `true`。
    #[serde(default)]
    pub include_ignored: bool,
    pub conversion: ConversionOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExtensionFilterMode {
    Allow,
    Deny,
    All,
}

/// `include_no_extension`：白名單時收無副檔名檔案；黑名單時把無副檔名檔案一併排除。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionFilter {
    pub mode: ExtensionFilterMode,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub include_no_extension: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanExtensionStatsRequest {
    pub path: String,
    #[serde(default)]
    pub recursive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionCount {
    /// 小寫並含前導點。
    pub extension: String,
    pub count: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionStats {
    pub extensions: Vec<ExtensionCount>,
    pub no_extension_count: u64,
    pub total_files: u64,
    pub directory_count: u64,
    pub warnings: Vec<String>,
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
    /// 因位於版本控制資料夾而略過的直接輸入（`include_ignored` 為 `false` 時）。
    #[serde(default)]
    pub ignored_inputs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AudioFieldWarningCode {
    Truncated,
    Unrepresentable,
    Hidden,
    Unmappable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioFieldWarning {
    pub code: AudioFieldWarningCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_bytes: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_bytes: Option<usize>,
    /// 無法表示的字數，或隱藏欄位數。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTagField {
    pub key: String,
    pub label: String,
    pub container: AudioContainer,
    pub values: Vec<String>,
    /// 與 `values` 相同時留空，避免同一段文字傳兩次。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub converted_values: Option<Vec<String>>,
    pub selected: bool,
    /// ID3v1 固定寬度寫入時，這個欄位被切到完整字元邊界。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
    /// 例如「ID3v1 標題超過 30 位元組，已截斷為 …」。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation: Option<String>,
    /// `rule n`、`manual` 或 `convert`。省略表示這次沒有標成某種來源。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<AudioFieldWarning>,
}

/// 整個容器的移除。`refused` 有值時沒有真的排進寫入（例如這是檔案唯一的標籤）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioContainerRemoval {
    pub container: AudioContainer,
    pub hidden_field_count: usize,
    /// `rule n`。
    pub origin: String,
    pub selected: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refused: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTagFile {
    pub path: String,
    pub format: AudioFormat,
    pub selected: bool,
    pub fields: Vec<AudioTagField>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub has_cover_art: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    /// 這個檔案已經跑過轉換預覽。沒有文字差異的欄位不會再帶 `converted_values`。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub previewed: bool,
    /// 這些容器的文字編碼或 ID3 版本與寫入設定不同，欄位文字相同也要寫。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rewrite_containers: Vec<AudioContainer>,
    /// 這次計畫要移除的整個標籤。空陣列不輸出。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removals: Vec<AudioContainerRemoval>,
    /// 容器級說明。例如 ID3v2 會整段重寫成另一個版本。空陣列不輸出。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TagAction {
    Copy,
    Remove,
    Merge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TagOnConflict {
    Overwrite,
    FillEmpty,
}

/// 標籤之間的一條規則。來源在展開時一律讀掃描到的原始標籤。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagOperation {
    pub action: TagAction,
    #[serde(default)]
    pub source: Option<AudioContainer>,
    #[serde(default)]
    pub target: Option<AudioContainer>,
    /// 空陣列表示標題、演出者、專輯、年份、音軌、註解、類型。
    #[serde(default)]
    pub fields: Vec<String>,
    #[serde(default)]
    pub on_conflict: Option<TagOnConflict>,
    /// 略過時後端仍保留規則編號，但不會套用。省略視為啟用。
    #[serde(default = "default_tag_operation_enabled")]
    pub enabled: bool,
    /// 有路徑時只作用於這些檔案，來源欄記成 `manual`，不佔規則編號。
    /// 空陣列是整批規則。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
}

fn default_tag_operation_enabled() -> bool {
    true
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
    /// 空陣列時行為與沒有標籤操作相同。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tag_operations: Vec<TagOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioRescanFileRequest {
    pub path: String,
    #[serde(default)]
    pub plan_id: Option<String>,
    #[serde(flatten)]
    pub plan: AudioTagPlanRequest,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AudioTagPlanSummary {
    pub total: usize,
    pub changed: usize,
    pub unchanged: usize,
    pub errors: usize,
    pub selected_fields: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTagPlan {
    pub plan_id: String,
    pub created_at: String,
    pub files: Vec<AudioTagFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// 計畫檔案的統計。舊客戶端可忽略；缺少時視為全零。
    #[serde(default)]
    pub summary: AudioTagPlanSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub succeeded: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<ApplyFailure>,
    /// 略過內容轉換等非致命警告。空陣列不輸出，以維持舊回應。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
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
    /// 無頭 CLI：全程不開 GUI。`/headless`／`--headless`，或經典 `/o:*`＋路徑且未指定 `/file`／`/audio`。
    #[serde(default)]
    pub headless: bool,
    /// 寫入確認：`/y`／`--yes`。無頭非 TTY 時必須為 true。
    #[serde(default)]
    pub confirm_write: bool,
    /// 是否明確指定 `/o:*`（經典無頭觸發條件之一）。
    #[serde(default)]
    pub output_encoding_explicit: bool,
    /// 是否明確指定 `--input`／`/i:*`。
    #[serde(default)]
    pub input_encoding_explicit: bool,
    /// 是否明確指定 `/f:*`／`--direction`。
    #[serde(default)]
    pub direction_explicit: bool,
    /// 是否明確指定 `--engine`／`/e:*`。
    #[serde(default)]
    pub engine_explicit: bool,
    /// 是否明確指定 `--vocabulary`／`/d:*`。
    #[serde(default)]
    pub vocabulary_explicit: bool,
    /// 是否明確指定 `--backup`／`--no-backup`／`/b:*`。
    #[serde(default)]
    pub backup_explicit: bool,
    /// 無頭時載入本機全域／可攜設定（`--globalconfig`）。不可與 `config_path` 併用。
    #[serde(default)]
    pub use_global_config: bool,
    /// `--include-ignored`：直接指定的版本控制資料夾（`.git` 等）或其中的路徑也照樣處理。
    /// 未指定時無頭模式略過這些輸入並印出警告。
    #[serde(default)]
    pub include_ignored: bool,
    /// 無頭時載入指定設定檔（`--config <路徑>`）。不可與 `use_global_config` 併用。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
    /// 已知旗標但值無效時的解析錯誤（無頭會失敗；GUI 可忽略）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parse_errors: Vec<String>,
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

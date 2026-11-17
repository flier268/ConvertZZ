mod audio;
mod backup;
mod cli;
mod conversion;
mod encoding;
mod error;
mod files;
pub mod headless;
mod parallelism;
pub mod roundtrip_dict;
mod settings;
mod types;
mod user_dictionary;
mod utility;
mod zhconvert;

pub use cli::{args_request_headless, args_request_help, print_cli_help};

pub use conversion::ConversionService;
pub use error::CoreError;
pub use types::{CancelCheck, Direction, ProgressEvent, ProgressReporter};

use audio::AudioService;
use files::FileService;
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use user_dictionary::UserDictionaryEntry;
use zhconvert::ZhConvertClient;

pub struct CoreState {
    conversion: ConversionService,
    files: FileService,
    audio: AudioService,
    cancelled_requests: Mutex<HashSet<String>>,
    user_dictionary_path: Option<PathBuf>,
    /// 啟動時使用者詞典讀取失敗的原因；此時以空詞典啟動，避免整個程式無法開啟。
    user_dictionary_error: Mutex<Option<String>>,
    user_dictionary_write: Mutex<()>,
}

impl CoreState {
    /// GUI：載入與設定檔同目錄的使用者詞典。
    pub fn new() -> Result<Self, CoreError> {
        Self::build(user_dictionary::default_path())
    }

    /// 無頭模式只在載入全域設定時使用使用者詞典（使用者設定的一部分）。
    pub fn new_headless(load_user_dictionary: bool) -> Result<Self, CoreError> {
        Self::build(if load_user_dictionary {
            user_dictionary::default_path()
        } else {
            None
        })
    }

    fn build(user_dictionary_path: Option<PathBuf>) -> Result<Self, CoreError> {
        let (entries, load_error) = match user_dictionary_path.as_deref() {
            Some(path) => match user_dictionary::load(path) {
                Ok(entries) => (entries, None),
                Err(error) => (Vec::new(), Some(error.message)),
            },
            None => (Vec::new(), None),
        };
        let conversion = ConversionService::with_user_dictionary(&entries)?;
        Ok(Self {
            conversion,
            files: FileService::new(),
            audio: AudioService::new(),
            cancelled_requests: Mutex::new(HashSet::new()),
            user_dictionary_path,
            user_dictionary_error: Mutex::new(load_error),
            user_dictionary_write: Mutex::new(()),
        })
    }

    fn user_dictionary_path(&self) -> Result<PathBuf, CoreError> {
        self.user_dictionary_path
            .clone()
            .ok_or_else(|| CoreError::new("USER_DICTIONARY_PATH", "找不到使用者詞典的儲存位置。"))
    }

    fn read_user_dictionary(&self) -> Result<Value, CoreError> {
        let path = self.user_dictionary_path()?;
        let (entries, load_error) = match user_dictionary::load(&path) {
            Ok(entries) => (entries, None),
            Err(error) => (Vec::new(), Some(error.message)),
        };
        let startup_error = self
            .user_dictionary_error
            .lock()
            .ok()
            .and_then(|error| error.clone());
        Ok(serde_json::json!({
            "path": path,
            "entries": entries,
            "loadError": load_error.or(startup_error),
        }))
    }

    /// 先在記憶體建好新分詞器，再寫檔（暫存驗證後取代），寫檔成功才換上新分詞器。
    fn save_user_dictionary(&self, entries: Vec<UserDictionaryEntry>) -> Result<Value, CoreError> {
        let path = self.user_dictionary_path()?;
        user_dictionary::check_conflicts(&entries)?;
        let _guard = self
            .user_dictionary_write
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        run_blocking(|| {
            self.conversion
                .set_user_dictionary(&entries, || user_dictionary::save(&path, &entries))
        })?;
        if let Ok(mut error) = self.user_dictionary_error.lock() {
            *error = None;
        }
        Ok(serde_json::json!({ "saved": entries.len(), "path": path }))
    }

    pub fn begin_request(&self, request_id: &str) {
        if let Ok(mut set) = self.cancelled_requests.lock() {
            set.remove(request_id);
        }
    }

    pub fn cancel_request(&self, request_id: &str) -> bool {
        self.cancelled_requests
            .lock()
            .map(|mut set| set.insert(request_id.to_string()))
            .unwrap_or(false)
    }

    pub fn finish_request(&self, request_id: &str) {
        if let Ok(mut set) = self.cancelled_requests.lock() {
            set.remove(request_id);
        }
    }

    pub fn is_request_cancelled(&self, request_id: &str) -> bool {
        self.cancelled_requests
            .lock()
            .ok()
            .is_some_and(|set| set.contains(request_id))
    }

    pub fn request_cancel_check(self: &Arc<Self>, request_id: &str) -> CancelCheck {
        let state = Arc::clone(self);
        let request_id = request_id.to_string();
        Arc::new(move || state.is_request_cancelled(&request_id))
    }
}

pub async fn dispatch(
    state: Arc<CoreState>,
    operation: &str,
    payload: Value,
    progress: ProgressReporter,
    request_id: &str,
) -> Result<Value, CoreError> {
    let is_cancelled = state.request_cancel_check(request_id);
    match operation {
        "health" => Ok(serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "engine": "rust",
            "pid": std::process::id(),
        })),
        "convert.preview" => {
            let request = serde_json::from_value(payload)?;
            to_value(
                state
                    .conversion
                    .convert_with_progress(request, Some(progress), Some(is_cancelled))
                    .await?,
            )
        }
        "core.cancel" => {
            let id = required_string(&payload, "id")?;
            let cancelled = state.cancel_request(&id);
            Ok(serde_json::json!({ "cancelled": cancelled }))
        }
        "files.plan" => {
            let request = serde_json::from_value(payload)?;
            to_value(
                state
                    .files
                    .plan(&state.conversion, request, progress)
                    .await?,
            )
        }
        "files.preview" => {
            let request = serde_json::from_value(payload)?;
            to_value(
                state
                    .files
                    .preview(&state.conversion, request, progress, is_cancelled)
                    .await?,
            )
        }
        "files.apply" => {
            let plan_id = required_string(&payload, "planId")?;
            let selected = selected_paths(&payload);
            to_value(
                state
                    .files
                    .apply(
                        &state.conversion,
                        &plan_id,
                        selected.as_deref(),
                        progress,
                        is_cancelled,
                    )
                    .await?,
            )
        }
        "files.scanExtensions" => {
            let request = serde_json::from_value(payload)?;
            to_value(files::scan_extension_stats(request, progress, is_cancelled).await?)
        }
        "files.cancel" => {
            let plan_id = required_string(&payload, "planId")?;
            to_value(state.files.cancel(&plan_id))
        }
        "audio.scan" => {
            let request = serde_json::from_value(payload)?;
            to_value(
                state
                    .audio
                    .scan_cancellable(&state.conversion, request, progress, is_cancelled)
                    .await?,
            )
        }
        "audio.plan" => {
            let request = serde_json::from_value(payload)?;
            to_value(
                state
                    .audio
                    .plan_cancellable(&state.conversion, request, progress, is_cancelled)
                    .await?,
            )
        }
        "audio.prioritize" => {
            let path = required_string(&payload, "path")?;
            state.audio.prioritize(&path);
            Ok(serde_json::json!({ "ok": true }))
        }
        "audio.rescanFile" => {
            let request = serde_json::from_value(payload)?;
            to_value(
                state
                    .audio
                    .rescan_file(&state.conversion, request, progress)
                    .await?,
            )
        }
        "audio.apply" => {
            let plan_id = required_string(&payload, "planId")?;
            let only_paths = payload.get("paths").and_then(Value::as_array).map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            });
            let selected_fields = match payload.get("selectedFields") {
                Some(value) => Some(serde_json::from_value::<
                    std::collections::HashMap<String, Vec<String>>,
                >(value.clone())?),
                None => None,
            };
            to_value(
                state
                    .audio
                    .apply_filtered(
                        &plan_id,
                        only_paths.as_deref(),
                        selected_fields.as_ref(),
                        progress,
                        is_cancelled,
                    )
                    .await?,
            )
        }
        "audio.cancel" => {
            let plan_id = required_string(&payload, "planId")?;
            to_value(state.audio.cancel(&plan_id))
        }
        "userDictionary.read" => state.read_user_dictionary(),
        "userDictionary.save" => {
            let entries: Vec<UserDictionaryEntry> =
                serde_json::from_value(payload.get("entries").cloned().unwrap_or(Value::Null))?;
            state.save_user_dictionary(entries)
        }
        "userDictionary.previewLegacyImport" => {
            let path = required_string(&payload, "path")?;
            to_value(run_blocking(|| {
                user_dictionary::preview_legacy_import(std::path::Path::new(&path))
            })?)
        }
        "settings.migrate" => {
            if let Some(path) = payload.get("path").and_then(Value::as_str) {
                to_value(settings::migrate_from_path(path)?)
            } else {
                to_value(settings::migrate(
                    payload.get("input").cloned().unwrap_or(Value::Null),
                ))
            }
        }
        "zhconvert.configure" => {
            let api_key = payload
                .get("apiKey")
                .and_then(Value::as_str)
                .unwrap_or_default();
            state.conversion.zhconvert.configure(api_key);
            Ok(serde_json::json!({ "configured": true }))
        }
        "zhconvert.serviceInfo" => {
            let force = payload
                .get("force")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            to_value(state.conversion.zhconvert.service_info(force).await?)
        }
        "utility.convert" => {
            let request = serde_json::from_value(payload)?;
            Ok(serde_json::json!({ "text": utility::convert(request)? }))
        }
        "cli.parse" => {
            let args = payload
                .get("args")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|value| value.as_str().unwrap_or_default().to_string())
                .collect::<Vec<_>>();
            let default_engine = payload
                .get("defaultEngine")
                .and_then(Value::as_str)
                .and_then(|value| serde_json::from_value(Value::String(value.to_string())).ok());
            to_value(cli::parse_cli(&args, default_engine))
        }
        other => Err(CoreError::new(
            "UNKNOWN_OPERATION",
            format!("未知操作：{other}"),
        )),
    }
}

fn run_blocking<R>(work: impl FnOnce() -> R) -> R {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(work)
        }
        _ => work(),
    }
}

fn to_value<T: serde::Serialize>(value: T) -> Result<Value, CoreError> {
    serde_json::to_value(value)
        .map_err(|error| CoreError::new("SERIALIZE", format!("無法序列化結果：{error}")))
}

fn required_string(payload: &Value, key: &str) -> Result<String, CoreError> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| CoreError::new("INVALID_PAYLOAD", format!("缺少 {key}。")))
}

fn selected_paths(payload: &Value) -> Option<Vec<String>> {
    payload
        .get("selectedPaths")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
}

pub fn zhconvert_client(state: &CoreState) -> &ZhConvertClient {
    &state.conversion.zhconvert
}

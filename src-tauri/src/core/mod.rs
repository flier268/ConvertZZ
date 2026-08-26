mod audio;
mod backup;
mod cli;
mod conversion;
mod dictionary;
mod dictionary_service;
mod encoding;
mod error;
mod files;
mod settings;
mod types;
mod utility;
mod zhconvert;

pub use error::CoreError;
pub use types::{ProgressEvent, ProgressReporter};

use audio::AudioService;
use conversion::ConversionService;
use dictionary_service::DictionaryService;
use files::FileService;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use zhconvert::ZhConvertClient;

pub struct CoreState {
    conversion: ConversionService,
    files: FileService,
    audio: AudioService,
    dictionary: DictionaryService,
}

impl CoreState {
    pub fn new(dictionary_path: Option<PathBuf>) -> Result<Self, CoreError> {
        let conversion = ConversionService::new(dictionary_path.clone())?;
        Ok(Self {
            conversion,
            files: FileService::new(),
            audio: AudioService::new(),
            dictionary: DictionaryService::new(dictionary_path),
        })
    }
}

pub async fn dispatch(
    state: Arc<CoreState>,
    operation: &str,
    payload: Value,
    progress: ProgressReporter,
) -> Result<Value, CoreError> {
    match operation {
        "health" => Ok(serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "engine": "rust",
            "pid": std::process::id(),
        })),
        "convert.preview" => {
            let request = serde_json::from_value(payload)?;
            to_value(state.conversion.convert(request).await?)
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
            to_value(state.files.preview(&state.conversion, request).await?)
        }
        "files.apply" => {
            let plan_id = required_string(&payload, "planId")?;
            let selected = selected_paths(&payload);
            to_value(
                state
                    .files
                    .apply(&state.conversion, &plan_id, selected.as_deref(), progress)
                    .await?,
            )
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
                    .scan(&state.conversion, request, progress)
                    .await?,
            )
        }
        "audio.plan" => {
            let request = serde_json::from_value(payload)?;
            to_value(
                state
                    .audio
                    .plan(&state.conversion, request, progress)
                    .await?,
            )
        }
        "audio.apply" => {
            let plan_id = required_string(&payload, "planId")?;
            to_value(state.audio.apply(&plan_id, progress).await?)
        }
        "audio.cancel" => {
            let plan_id = required_string(&payload, "planId")?;
            to_value(state.audio.cancel(&plan_id))
        }
        "dictionary.read" => {
            let request = serde_json::from_value(payload)?;
            to_value(state.dictionary.read(request)?)
        }
        "dictionary.update" => {
            let request = serde_json::from_value(payload)?;
            to_value(state.dictionary.update(request)?)
        }
        "dictionary.preview" => {
            let request = serde_json::from_value(payload)?;
            to_value(state.dictionary.preview(request)?)
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
            to_value(cli::parse_legacy_cli(&args, default_engine))
        }
        other => Err(CoreError::new(
            "UNKNOWN_OPERATION",
            format!("未知操作：{other}"),
        )),
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

#[allow(dead_code)]
pub fn zhconvert_client(state: &CoreState) -> &ZhConvertClient {
    &state.conversion.zhconvert
}

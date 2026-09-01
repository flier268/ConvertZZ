use super::error::CoreError;
use super::types::{
    CancelCheck, Direction, ProgressEvent, ProgressReporter, ZhConvertModules, ZhConvertOptions,
};
use serde_json::Value;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct ZhConvertClient {
    client: reqwest::Client,
    base_url: String,
    api_key: Mutex<String>,
    cache: Mutex<Option<(Instant, Value)>>,
}

impl ZhConvertClient {
    pub fn new() -> Self {
        Self::with_base_url("https://api.zhconvert.org")
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into(),
            api_key: Mutex::new(String::new()),
            cache: Mutex::new(None),
        }
    }

    pub fn configure(&self, api_key: &str) {
        if let Ok(mut slot) = self.api_key.lock() {
            *slot = api_key.trim().to_string();
        }
    }

    fn api_key(&self) -> String {
        self.api_key
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default()
    }

    pub async fn service_info(&self, force: bool) -> Result<Value, CoreError> {
        if !force {
            if let Ok(cache) = self.cache.lock() {
                if let Some((expires_at, value)) = cache.as_ref() {
                    if *expires_at > Instant::now() {
                        return Ok(value.clone());
                    }
                }
            }
        }
        let mut request = self.client.get(format!("{}/service-info", self.base_url));
        let api_key = self.api_key();
        if !api_key.is_empty() {
            request = request.header("X-API-Key", api_key);
        }
        let response = request.send().await.map_err(network_error)?;
        let status = response.status();
        if !status.is_success() {
            return Err(CoreError::new(
                "ZHCONVERT_SERVICE_INFO",
                format!("ZhConvert 服務資訊讀取失敗。HTTP {status}"),
            ));
        }
        let value = response.json::<Value>().await.map_err(network_error)?;
        if let Ok(mut cache) = self.cache.lock() {
            *cache = Some((Instant::now() + Duration::from_secs(86_400), value.clone()));
        }
        Ok(value)
    }

    pub async fn convert(
        &self,
        text: &str,
        direction: Direction,
        options: Option<&ZhConvertOptions>,
    ) -> Result<String, CoreError> {
        self.convert_with_progress(text, direction, options, None, None)
            .await
    }

    pub async fn convert_with_progress(
        &self,
        text: &str,
        direction: Direction,
        options: Option<&ZhConvertOptions>,
        progress: Option<ProgressReporter>,
        is_cancelled: Option<CancelCheck>,
    ) -> Result<String, CoreError> {
        if direction == Direction::None || text.is_empty() {
            return Ok(text.to_string());
        }
        if is_cancelled.as_ref().is_some_and(|check| check()) {
            return Err(CoreError::new("CONVERT_CANCELLED", "轉換已由使用者取消。"));
        }
        let options = options.cloned().unwrap_or_default();
        let info = self.service_info(false).await?;
        let maximum = info
            .pointer("/data/maxPostBodyBytes")
            .and_then(Value::as_u64)
            .unwrap_or(50_000)
            .saturating_sub(2048)
            .max(1024) as usize;
        let total = text.chars().count().max(1) as u64;
        let mut done = 0u64;
        let mut converted = String::new();
        for chunk in split_utf8(text, maximum) {
            if is_cancelled.as_ref().is_some_and(|check| check()) {
                return Err(CoreError::new("CONVERT_CANCELLED", "轉換已由使用者取消。"));
            }
            converted.push_str(&self.convert_chunk(&chunk, direction, &options).await?);
            done += chunk.chars().count() as u64;
            if let Some(progress) = &progress {
                progress(ProgressEvent {
                    current: done.min(total),
                    total,
                    message: format!("正在呼叫 ZhConvert… {}/{total}", done.min(total)),
                });
            }
        }
        Ok(converted)
    }

    async fn convert_chunk(
        &self,
        chunk: &str,
        direction: Direction,
        options: &ZhConvertOptions,
    ) -> Result<String, CoreError> {
        let converter = options.converter.clone().unwrap_or_else(|| {
            if direction == Direction::S2t {
                "Taiwan".into()
            } else {
                "Simplified".into()
            }
        });
        let mut params = vec![
            ("text", chunk.to_string()),
            ("converter", converter),
            ("outputFormat", "json".into()),
        ];
        let api_key = self.api_key();
        if !api_key.is_empty() {
            params.push(("apiKey", api_key));
        }
        if let Some(modules) = &options.modules {
            let map = match modules {
                ZhConvertModules::Map(map) => map.clone(),
                ZhConvertModules::List(list) => {
                    list.iter().cloned().map(|name| (name, 1)).collect()
                }
            };
            if !map.is_empty() {
                params.push(("modules", serde_json::to_string(&map).unwrap_or_default()));
            }
        }
        push_opt(
            &mut params,
            "jpTextConversionStrategy",
            options.jp_text_conversion_strategy.as_deref(),
        );
        push_opt(
            &mut params,
            "jpStyleConversionStrategy",
            options.jp_style_conversion_strategy.as_deref(),
        );
        push_bool(&mut params, "cleanUpText", options.clean_up_text);
        push_opt(
            &mut params,
            "userPreReplace",
            options.user_pre_replace.as_deref(),
        );
        push_opt(
            &mut params,
            "userPostReplace",
            options.user_post_replace.as_deref(),
        );
        push_opt(
            &mut params,
            "userProtectReplace",
            options.user_protect_replace.as_deref(),
        );
        push_bool(
            &mut params,
            "ensureNewlineAtEof",
            options.ensure_newline_at_eof,
        );
        if let Some(value) = options.translate_tabs_to_spaces {
            params.push(("translateTabsToSpaces", value.to_string()));
        }
        push_bool(
            &mut params,
            "trimTrailingWhiteSpaces",
            options.trim_trailing_white_spaces,
        );
        push_bool(
            &mut params,
            "unifyLeadingHyphen",
            options.unify_leading_hyphen,
        );
        push_opt(
            &mut params,
            "ignoreTextStyles",
            options.ignore_text_styles.as_deref(),
        );
        push_opt(
            &mut params,
            "jpTextStyles",
            options.jp_text_styles.as_deref(),
        );

        let response = self
            .client
            .post(format!("{}/convert", self.base_url))
            .header(
                "content-type",
                "application/x-www-form-urlencoded;charset=UTF-8",
            )
            .form(&params)
            .send()
            .await
            .map_err(network_error)?;
        let status = response.status();
        if !status.is_success() {
            let detail = response.text().await.unwrap_or_default();
            return Err(CoreError::with_details(
                "ZHCONVERT_CONVERT",
                format!("ZhConvert 轉換失敗。HTTP {status}"),
                detail,
            ));
        }
        let payload = response.json::<Value>().await.map_err(network_error)?;
        extract_text(&payload).ok_or_else(|| {
            CoreError::with_details(
                "ZHCONVERT_RESPONSE",
                "ZhConvert 回應不含文字結果。",
                payload,
            )
        })
    }
}

fn extract_text(payload: &Value) -> Option<String> {
    if let Some(text) = payload.get("data").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(text) = payload.pointer("/data/text").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    payload
        .get("text")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn push_opt(params: &mut Vec<(&str, String)>, key: &'static str, value: Option<&str>) {
    if let Some(value) = value.filter(|value| !value.is_empty()) {
        params.push((key, value.to_string()));
    }
}

fn push_bool(params: &mut Vec<(&str, String)>, key: &'static str, value: Option<bool>) {
    if let Some(value) = value {
        params.push((key, value.to_string()));
    }
}

fn network_error(error: reqwest::Error) -> CoreError {
    CoreError::new("ZHCONVERT_NETWORK", error.to_string())
}

fn split_utf8(text: &str, maximum_bytes: usize) -> Vec<String> {
    if text.len() <= maximum_bytes {
        return vec![text.to_string()];
    }
    let mut chunks = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        let mut low = 1;
        let mut high = remaining.chars().count();
        while low < high {
            let middle = (low + high).div_ceil(2);
            let slice: String = remaining.chars().take(middle).collect();
            if slice.len() <= maximum_bytes {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        let mut boundary = remaining.chars().take(low).collect::<String>();
        if let Some(natural) = boundary.rfind(|character| "。！？!?\n".contains(character)) {
            if natural > boundary.len() / 2 {
                let width = boundary[natural..]
                    .chars()
                    .next()
                    .map(char::len_utf8)
                    .unwrap_or(0);
                boundary = boundary[..natural + width].to_string();
            }
        }
        while !remaining.starts_with(&boundary) && !boundary.is_empty() {
            boundary.pop();
        }
        let length = boundary
            .len()
            .max(remaining.chars().next().map(|c| c.len_utf8()).unwrap_or(0));
        let (chunk, rest) = remaining.split_at(length.min(remaining.len()));
        chunks.push(chunk.to_string());
        remaining = rest;
    }
    chunks
}

#[cfg(test)]
mod tests;

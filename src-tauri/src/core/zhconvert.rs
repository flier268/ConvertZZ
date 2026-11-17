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
        let limit = info
            .pointer("/data/maxPostBodyBytes")
            .and_then(Value::as_u64)
            .unwrap_or(50_000) as usize;
        // 主體是 form 編碼：中文每個位元組變成 %XX，其他參數（含自訂取代規則）也算在上限內。
        let fixed = form_body_len(&self.build_params("", direction, &options));
        let budget = limit.saturating_sub(fixed + BODY_MARGIN);
        if budget < MIN_CHUNK_COST {
            return Err(CoreError::new(
                "ZHCONVERT_BODY",
                "ZhConvert 的自訂取代規則等參數太長，超過服務允許的請求大小。",
            ));
        }
        let total = text.chars().count().max(1) as u64;
        let mut done = 0u64;
        let mut converted = String::new();
        for chunk in split_form_encoded(text, budget) {
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

                    batch: None,
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
        let params = self.build_params(chunk, direction, options);
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

    fn build_params(
        &self,
        chunk: &str,
        direction: Direction,
        options: &ZhConvertOptions,
    ) -> Vec<(&'static str, String)> {
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

        params
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

/// 請求主體除了文字之外預留的位元組，涵蓋 HTTP 實作差異。
const BODY_MARGIN: usize = 256;
/// 扣掉其他參數後，每段至少要能放下的編碼長度。
const MIN_CHUNK_COST: usize = 64;

fn form_body_len(params: &[(&str, String)]) -> usize {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(params)
        .finish()
        .len()
}

/// 與 `application/x-www-form-urlencoded` 相同：英數與 `*-._`、空白各占 1，其餘位元組各占 3（%XX）。
fn form_cost(character: char) -> usize {
    match character {
        'a'..='z' | 'A'..='Z' | '0'..='9' | '*' | '-' | '.' | '_' | ' ' => 1,
        other => 3 * other.len_utf8(),
    }
}

/// 依 form 編碼後的長度切段，每段不超過 `budget`；盡量在句尾切開。
fn split_form_encoded(text: &str, budget: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut cost = 0;
    let mut natural: Option<(usize, usize)> = None;
    for (index, character) in text.char_indices() {
        let width = form_cost(character);
        if cost + width > budget && index > start {
            let cut = match natural {
                Some((end, end_cost)) if end_cost * 2 > budget => end,
                _ => index,
            };
            chunks.push(text[start..cut].to_string());
            cost = form_cost_of(&text[cut..index]);
            start = cut;
            natural = None;
        }
        cost += width;
        if "。！？!?\n".contains(character) {
            natural = Some((index + character.len_utf8(), cost));
        }
    }
    if start < text.len() || chunks.is_empty() {
        chunks.push(text[start..].to_string());
    }
    chunks
}

fn form_cost_of(text: &str) -> usize {
    text.chars().map(form_cost).sum()
}

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) use tests::start_mock;

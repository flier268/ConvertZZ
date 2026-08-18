use super::error::CoreError;
use super::types::{Direction, ZhConvertModules, ZhConvertOptions};
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
        if direction == Direction::None || text.is_empty() {
            return Ok(text.to_string());
        }
        let options = options.cloned().unwrap_or_default();
        let info = self.service_info(false).await?;
        let maximum = info
            .pointer("/data/maxPostBodyBytes")
            .and_then(Value::as_u64)
            .unwrap_or(50_000)
            .saturating_sub(2048)
            .max(1024) as usize;
        let mut converted = String::new();
        for chunk in split_utf8(text, maximum) {
            converted.push_str(&self.convert_chunk(&chunk, direction, &options).await?);
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
mod tests {
    use super::super::types::ZhConvertOptions;
    use super::*;
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;

    #[derive(Default)]
    struct MockState {
        service_info: u32,
        convert: u32,
        fail: bool,
        last_body: HashMap<String, String>,
        stop: bool,
    }

    fn start_mock() -> (String, Arc<Mutex<MockState>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let state = Arc::new(Mutex::new(MockState::default()));
        let shared = Arc::clone(&state);
        thread::spawn(move || loop {
            if shared.lock().unwrap().stop {
                break;
            }
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buffer = [0_u8; 16 * 1024];
                    let _ = stream.set_nonblocking(false);
                    let count = stream.read(&mut buffer).unwrap_or(0);
                    let request = String::from_utf8_lossy(&buffer[..count]);
                    let mut state = shared.lock().unwrap();
                    if request.starts_with("GET /service-info") {
                        state.service_info += 1;
                        let body = r#"{"data":{"maxPostBodyBytes":2048}}"#;
                        let _ = write!(
                            stream,
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                    } else if request.starts_with("POST /convert") {
                        state.convert += 1;
                        if state.fail {
                            let body = "暫時無法使用";
                            let _ = write!(
                                stream,
                                "HTTP/1.1 503 Service Unavailable\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                                body.len()
                            );
                        } else {
                            let body = request.split("\r\n\r\n").nth(1).unwrap_or_default();
                            let params = urlencoding_pairs(body);
                            state.last_body = params.clone();
                            let text = params
                                .get("text")
                                .cloned()
                                .unwrap_or_default()
                                .replace('里', "裡");
                            let json = serde_json::json!({ "data": { "text": text } }).to_string();
                            let _ = write!(
                                stream,
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
                                json.len()
                            );
                        }
                    } else {
                        let _ = write!(stream, "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(_) => break,
            }
        });
        (format!("http://{address}"), state)
    }

    fn urlencoding_pairs(body: &str) -> HashMap<String, String> {
        body.split('&')
            .filter_map(|pair| {
                let (key, value) = pair.split_once('=')?;
                Some((decode_form(key), decode_form(value)))
            })
            .collect()
    }

    fn decode_form(value: &str) -> String {
        let mut bytes = Vec::new();
        let chars: Vec<char> = value.chars().collect();
        let mut index = 0;
        while index < chars.len() {
            match chars[index] {
                '+' => bytes.push(b' '),
                '%' if index + 2 < chars.len() => {
                    let hex: String = chars[index + 1..=index + 2].iter().collect();
                    if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                        bytes.push(byte);
                        index += 2;
                    } else {
                        bytes.push(b'%');
                    }
                }
                other => bytes.extend(other.encode_utf8(&mut [0; 4]).as_bytes()),
            }
            index += 1;
        }
        String::from_utf8_lossy(&bytes).into_owned()
    }

    #[test]
    fn split_utf8_keeps_multibyte_punctuation_on_char_boundaries() {
        let source = format!("{}。{}", "里".repeat(400), "面".repeat(400));
        let chunks = split_utf8(&source, 1024);
        assert!(chunks.len() > 1);
        assert_eq!(chunks.concat(), source);
        assert!(chunks
            .iter()
            .all(|chunk| chunk.is_char_boundary(chunk.len())));
    }

    #[tokio::test]
    async fn caches_service_info_and_splits_utf8() {
        let (url, state) = start_mock();
        let client = ZhConvertClient::with_base_url(url);
        let source = "里".repeat(1_200);
        assert_eq!(
            client.convert(&source, Direction::S2t, None).await.unwrap(),
            "裡".repeat(1_200)
        );
        assert_eq!(
            client.convert("里", Direction::S2t, None).await.unwrap(),
            "裡"
        );
        let mut snapshot = state.lock().unwrap();
        assert_eq!(snapshot.service_info, 1);
        assert!(snapshot.convert > 2);
        snapshot.stop = true;
    }

    #[tokio::test]
    async fn posts_official_convert_options() {
        let (url, state) = start_mock();
        let client = ZhConvertClient::with_base_url(url);
        client
            .convert(
                "里",
                Direction::S2t,
                Some(&ZhConvertOptions {
                    converter: Some("Hongkong".into()),
                    modules: Some(ZhConvertModules::Map(HashMap::from([(
                        "TaiwanPhrase".into(),
                        1,
                    )]))),
                    jp_text_conversion_strategy: Some("fix".into()),
                    jp_style_conversion_strategy: Some("none".into()),
                    clean_up_text: Some(true),
                    user_pre_replace: Some("甲=乙".into()),
                    user_post_replace: Some("丙=丁".into()),
                    user_protect_replace: Some("戊".into()),
                    ensure_newline_at_eof: Some(true),
                    translate_tabs_to_spaces: Some(4),
                    trim_trailing_white_spaces: Some(true),
                    unify_leading_hyphen: Some(true),
                    ignore_text_styles: Some("code".into()),
                    jp_text_styles: Some("jp".into()),
                }),
            )
            .await
            .unwrap();
        let mut snapshot = state.lock().unwrap();
        assert_eq!(
            snapshot.last_body.get("converter").map(String::as_str),
            Some("Hongkong")
        );
        assert_eq!(
            snapshot.last_body.get("modules").map(String::as_str),
            Some(r#"{"TaiwanPhrase":1}"#)
        );
        assert_eq!(
            snapshot
                .last_body
                .get("jpTextConversionStrategy")
                .map(String::as_str),
            Some("fix")
        );
        assert_eq!(
            snapshot
                .last_body
                .get("jpStyleConversionStrategy")
                .map(String::as_str),
            Some("none")
        );
        assert_eq!(
            snapshot.last_body.get("cleanUpText").map(String::as_str),
            Some("true")
        );
        assert_eq!(
            snapshot.last_body.get("userPreReplace").map(String::as_str),
            Some("甲=乙")
        );
        snapshot.stop = true;
    }

    #[tokio::test]
    async fn reports_structured_network_error() {
        let (url, state) = start_mock();
        state.lock().unwrap().fail = true;
        let client = ZhConvertClient::with_base_url(url);
        let error = client
            .convert("里面", Direction::S2t, None)
            .await
            .unwrap_err();
        assert_eq!(error.code, "ZHCONVERT_CONVERT");
        state.lock().unwrap().stop = true;
    }
}

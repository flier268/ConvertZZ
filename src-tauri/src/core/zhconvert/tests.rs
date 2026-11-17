use super::super::types::ZhConvertOptions;
use super::*;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Default)]
pub(crate) struct MockState {
    service_info: u32,
    convert: u32,
    fail: bool,
    last_body: HashMap<String, String>,
    max_body_len: usize,
    pub(crate) stop: bool,
}

pub(crate) fn start_mock() -> (String, Arc<Mutex<MockState>>) {
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
                    let body_len = request
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|value| value.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    state.max_body_len = state.max_body_len.max(body_len);
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
                    let _ = write!(
                        stream,
                        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
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
fn split_form_encoded_counts_percent_encoding_and_prefers_sentence_ends() {
    let source = format!("{}。{}", "里".repeat(400), "面".repeat(400));
    let chunks = split_form_encoded(&source, 4000);
    assert!(chunks.len() > 1);
    assert_eq!(chunks.concat(), source);
    assert!(chunks.iter().all(|chunk| form_cost_of(chunk) <= 4000));
    assert!(chunks[0].ends_with('。'), "{}", chunks[0].chars().count());
    let ascii = "ab c".repeat(100);
    assert!(split_form_encoded(&ascii, 400).len() == 1);
    let encoded = url::form_urlencoded::byte_serialize("里 a/面".as_bytes()).collect::<String>();
    assert_eq!(form_cost_of("里 a/面"), encoded.len());
}

#[tokio::test]
async fn request_bodies_stay_within_declared_limit_including_replace_rules() {
    let (url, state) = start_mock();
    let client = ZhConvertClient::with_base_url(url);
    let options = ZhConvertOptions {
        user_pre_replace: Some("甲=乙\n".repeat(30)),
        ..ZhConvertOptions::default()
    };
    let source = format!("{}\n", "里面".repeat(100)).repeat(6);
    let converted = client
        .convert(&source, Direction::S2t, Some(&options))
        .await
        .unwrap();
    assert_eq!(converted, source.replace('里', "裡"));
    let snapshot = state.lock().unwrap();
    assert!(snapshot.convert > 1);
    assert!(snapshot.max_body_len <= 2048, "{}", snapshot.max_body_len);
}

#[tokio::test]
async fn oversized_replace_rules_report_error_instead_of_sending() {
    let (url, state) = start_mock();
    let client = ZhConvertClient::with_base_url(url);
    let options = ZhConvertOptions {
        user_pre_replace: Some("甲=乙\n".repeat(200)),
        ..ZhConvertOptions::default()
    };
    let error = client
        .convert("里面", Direction::S2t, Some(&options))
        .await
        .unwrap_err();
    assert_eq!(error.code, "ZHCONVERT_BODY");
    assert_eq!(state.lock().unwrap().convert, 0);
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

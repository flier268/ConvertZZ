use super::encoding::reinterpret_text;
use super::error::CoreError;
use super::types::{TextEncoding, UtilityConvertRequest, UtilityKind};
use std::collections::HashMap;
use std::sync::OnceLock;

pub fn convert(request: UtilityConvertRequest) -> Result<String, CoreError> {
    Ok(match request.kind {
        UtilityKind::HtmlDecimalEncode => html_encode(&request.text, 10),
        UtilityKind::HtmlDecimalDecode | UtilityKind::HtmlHexDecode => html_decode(&request.text),
        UtilityKind::HtmlHexEncode => html_encode(&request.text, 16),
        UtilityKind::UnicodeEscapeEncode => unicode_escape_encode(&request.text),
        UtilityKind::UnicodeEscapeDecode => unicode_escape_decode(&request.text),
        UtilityKind::Encoding => reinterpret_text(
            &request.text,
            request.source_encoding.unwrap_or(TextEncoding::Utf8),
            request.target_encoding.unwrap_or(TextEncoding::Utf8),
        )?,
        UtilityKind::Fullwidth => replace_symbols(&request.text, symbol_table()),
        UtilityKind::Halfwidth => replace_symbols(&request.text, reverse_symbol_table()),
    })
}

fn html_encode(text: &str, radix: u32) -> String {
    text.chars()
        .map(|character| match character {
            '&' => "&amp;".into(),
            '<' => "&lt;".into(),
            '>' => "&gt;".into(),
            '\r' | '\n' => character.to_string(),
            other => {
                let code = other as u32;
                if (0x20..=0x7e).contains(&code) {
                    other.to_string()
                } else if radix == 10 {
                    format!("&#{code};")
                } else {
                    format!("&#x{code:X};")
                }
            }
        })
        .collect()
}

fn html_decode(text: &str) -> String {
    let hex = regex::Regex::new(r"(?i)&#x([\da-f]+);?").expect("hex entity");
    let dec = regex::Regex::new(r"&#(\d+);?").expect("dec entity");
    let mut output = hex
        .replace_all(text, |captures: &regex::Captures| {
            safe_code_point(
                captures.get(0).map(|m| m.as_str()).unwrap_or(""),
                &captures[1],
                16,
            )
        })
        .into_owned();
    output = dec
        .replace_all(&output, |captures: &regex::Captures| {
            safe_code_point(
                captures.get(0).map(|m| m.as_str()).unwrap_or(""),
                &captures[1],
                10,
            )
        })
        .into_owned();
    output
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn safe_code_point(original: &str, value: &str, radix: u32) -> String {
    u32::from_str_radix(value, radix)
        .ok()
        .and_then(char::from_u32)
        .map(String::from)
        .unwrap_or_else(|| original.to_string())
}

fn unicode_escape_encode(text: &str) -> String {
    text.chars()
        .map(|character| {
            let code = character as u32;
            if code <= 0xffff {
                format!("\\u{code:04X}")
            } else {
                let adjusted = code - 0x10000;
                let high = 0xd800 + (adjusted >> 10);
                let low = 0xdc00 + (adjusted & 0x3ff);
                format!("\\u{high:04X}\\u{low:04X}")
            }
        })
        .collect()
}

fn unicode_escape_decode(text: &str) -> String {
    let braced = regex::Regex::new(r"(?i)\\u\{([\da-f]{1,6})\}").expect("braced");
    let units = regex::Regex::new(r"(?i)(?:\\u[\da-f]{4})+").expect("units");
    let mut output = braced
        .replace_all(text, |captures: &regex::Captures| {
            safe_code_point(
                captures.get(0).map(|m| m.as_str()).unwrap_or(""),
                &captures[1],
                16,
            )
        })
        .into_owned();
    output = units
        .replace_all(&output, |captures: &regex::Captures| {
            let sequence = captures.get(0).map(|m| m.as_str()).unwrap_or("");
            let codes = regex::Regex::new(r"(?i)[\da-f]{4}")
                .expect("unit")
                .find_iter(sequence)
                .filter_map(|item| u16::from_str_radix(item.as_str(), 16).ok())
                .collect::<Vec<_>>();
            String::from_utf16_lossy(&codes)
        })
        .into_owned();
    output
}

fn replace_symbols(text: &str, table: &HashMap<char, char>) -> String {
    text.chars()
        .map(|character| table.get(&character).copied().unwrap_or(character))
        .collect()
}

fn symbol_table() -> &'static HashMap<char, char> {
    static TABLE: OnceLock<HashMap<char, char>> = OnceLock::new();
    TABLE.get_or_init(|| {
        [
            (',', '，'),
            ('~', '～'),
            ('!', '！'),
            ('#', '＃'),
            ('$', '＄'),
            ('%', '％'),
            ('^', '︿'),
            ('&', '＆'),
            ('*', '＊'),
            ('-', '－'),
            ('+', '＋'),
            ('{', '｛'),
            ('}', '｝'),
            (';', '；'),
            ('|', '｜'),
            ('?', '？'),
            ('(', '（'),
            (')', '）'),
            ('“', '「'),
            ('”', '」'),
            ('‘', '『'),
            ('’', '』'),
            ('[', '［'),
            (']', '］'),
            (' ', '　'),
            (':', '：'),
            ('.', '。'),
            ('"', '、'),
            ('@', '＠'),
            ('<', '＜'),
            ('>', '＞'),
            ('=', '＝'),
        ]
        .into_iter()
        .collect()
    })
}

fn reverse_symbol_table() -> &'static HashMap<char, char> {
    static TABLE: OnceLock<HashMap<char, char>> = OnceLock::new();
    TABLE.get_or_init(|| {
        symbol_table()
            .iter()
            .map(|(source, target)| (*target, *source))
            .collect()
    })
}

#[cfg(test)]
mod tests;

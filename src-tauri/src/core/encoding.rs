use super::error::CoreError;
use super::types::TextEncoding;
use encoding_rs::{Encoding, BIG5, EUC_JP, GBK, ISO_2022_JP, SHIFT_JIS, UTF_16BE, UTF_16LE, UTF_8};
use regex::Regex;
use std::sync::OnceLock;

pub fn detect_encoding(buffer: &[u8]) -> TextEncoding {
    if buffer.len() >= 3 && buffer[..3] == [0xef, 0xbb, 0xbf] {
        return TextEncoding::Utf8Bom;
    }
    if buffer.len() >= 2 && buffer[..2] == [0xff, 0xfe] {
        return TextEncoding::Utf16le;
    }
    if buffer.len() >= 2 && buffer[..2] == [0xfe, 0xff] {
        return TextEncoding::Utf16be;
    }
    if let Some(declared) = declared_encoding(buffer) {
        return declared;
    }
    if looks_like_hz(&buffer[..buffer.len().min(8192)]) {
        return TextEncoding::HzGb2312;
    }
    let sample = &buffer[..buffer.len().min(128 * 1024)];
    let mut detector = chardetng::EncodingDetector::new(chardetng::Iso2022JpDetection::Allow);
    detector.feed(sample, true);
    map_encoding(detector.guess(None, chardetng::Utf8Detection::Allow))
        .unwrap_or(TextEncoding::Utf8)
}

pub fn decode_text(
    buffer: &[u8],
    requested: TextEncoding,
) -> Result<(String, TextEncoding), CoreError> {
    let encoding = if requested == TextEncoding::Auto {
        detect_encoding(buffer)
    } else {
        requested
    };
    let without_bom = strip_bom(buffer, encoding);
    if encoding == TextEncoding::HzGb2312 {
        return Ok((decode_hz(without_bom)?, encoding));
    }
    if encoding == TextEncoding::Utf16le {
        return Ok((decode_utf16(without_bom, true), encoding));
    }
    if encoding == TextEncoding::Utf16be {
        return Ok((decode_utf16(without_bom, false), encoding));
    }
    let Some(codec) = encoding_codec(encoding) else {
        return Err(CoreError::new(
            "ENCODING_UNSUPPORTED",
            format!("不支援編碼 {encoding:?}。"),
        ));
    };
    let (text, _, _) = codec.decode(without_bom);
    Ok((text.into_owned(), encoding))
}

pub fn encode_text(
    text: &str,
    encoding: TextEncoding,
    add_bom: bool,
) -> Result<Vec<u8>, CoreError> {
    if encoding == TextEncoding::Auto {
        return Err(CoreError::new(
            "ENCODING_AUTO_OUTPUT",
            "輸出編碼不能使用自動偵測。",
        ));
    }
    if encoding == TextEncoding::HzGb2312 {
        return encode_hz(text);
    }
    if encoding == TextEncoding::Utf16le || encoding == TextEncoding::Utf16be {
        let mut output = encode_utf16(text, encoding == TextEncoding::Utf16le);
        if add_bom {
            prepend_bom(&mut output, encoding);
        }
        return Ok(output);
    }
    let Some(codec) = encoding_codec(encoding) else {
        return Err(CoreError::new(
            "ENCODING_UNSUPPORTED",
            format!("不支援編碼 {encoding:?}。"),
        ));
    };
    let (encoded, _, _) = codec.encode(text);
    let mut output = encoded.into_owned();
    if add_bom || encoding == TextEncoding::Utf8Bom {
        prepend_bom(&mut output, encoding);
    }
    Ok(output)
}

pub fn reinterpret_text(
    text: &str,
    source: TextEncoding,
    target: TextEncoding,
) -> Result<String, CoreError> {
    if source == TextEncoding::Auto || target == TextEncoding::Auto {
        return Err(CoreError::new(
            "ENCODING_REQUIRED",
            "重新解讀文字時必須指定來源與目標編碼。",
        ));
    }
    let encoded = encode_text(text, source, false)?;
    Ok(decode_text(&encoded, target)?.0)
}

pub fn can_roundtrip(text: &str, encoding: TextEncoding) -> bool {
    encode_text(text, encoding, false)
        .ok()
        .and_then(|bytes| decode_text(&bytes, encoding).ok())
        .is_some_and(|(decoded, _)| decoded == text)
}

fn encode_utf16(text: &str, little_endian: bool) -> Vec<u8> {
    let mut output = Vec::with_capacity(text.len() * 2);
    for unit in text.encode_utf16() {
        let bytes = if little_endian {
            unit.to_le_bytes()
        } else {
            unit.to_be_bytes()
        };
        output.extend_from_slice(&bytes);
    }
    output
}

fn decode_utf16(buffer: &[u8], little_endian: bool) -> String {
    let units = buffer
        .chunks_exact(2)
        .map(|chunk| {
            if little_endian {
                u16::from_le_bytes([chunk[0], chunk[1]])
            } else {
                u16::from_be_bytes([chunk[0], chunk[1]])
            }
        })
        .collect::<Vec<_>>();
    String::from_utf16_lossy(&units)
}

fn encoding_codec(encoding: TextEncoding) -> Option<&'static Encoding> {
    match encoding {
        TextEncoding::Utf8 | TextEncoding::Utf8Bom => Some(UTF_8),
        TextEncoding::Utf16le => Some(UTF_16LE),
        TextEncoding::Utf16be => Some(UTF_16BE),
        TextEncoding::Big5 => Some(BIG5),
        TextEncoding::Gbk => Some(GBK),
        TextEncoding::ShiftJis => Some(SHIFT_JIS),
        TextEncoding::EucJp => Some(EUC_JP),
        TextEncoding::Iso2022Jp => Some(ISO_2022_JP),
        TextEncoding::Auto | TextEncoding::HzGb2312 => None,
    }
}

fn map_encoding(encoding: &'static Encoding) -> Option<TextEncoding> {
    if encoding == UTF_8 {
        Some(TextEncoding::Utf8)
    } else if encoding == UTF_16LE {
        Some(TextEncoding::Utf16le)
    } else if encoding == UTF_16BE {
        Some(TextEncoding::Utf16be)
    } else if encoding == BIG5 {
        Some(TextEncoding::Big5)
    } else if encoding == GBK || encoding.name() == "gb18030" || encoding.name() == "gbk" {
        Some(TextEncoding::Gbk)
    } else if encoding == SHIFT_JIS {
        Some(TextEncoding::ShiftJis)
    } else if encoding == EUC_JP {
        Some(TextEncoding::EucJp)
    } else if encoding == ISO_2022_JP {
        Some(TextEncoding::Iso2022Jp)
    } else {
        None
    }
}

fn declared_encoding(buffer: &[u8]) -> Option<TextEncoding> {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        Regex::new(r#"(?i)(?:charset\s*=\s*["']?|@charset\s+["'])([a-z\d_-]+)"#).expect("charset")
    });
    let sample = &buffer[..buffer.len().min(16 * 1024)];
    let latin1: String = sample.iter().map(|&byte| byte as char).collect();
    let captured = pattern.captures(&latin1)?.get(1)?.as_str();
    alias(captured)
}

fn alias(name: &str) -> Option<TextEncoding> {
    match name.to_ascii_uppercase().replace('_', "-").as_str() {
        "UTF8" | "UTF-8" => Some(TextEncoding::Utf8),
        "UTF16LE" | "UTF-16LE" => Some(TextEncoding::Utf16le),
        "UTF16BE" | "UTF-16BE" => Some(TextEncoding::Utf16be),
        "BIG5" | "BIG5HKSCS" | "BIG5-HKSCS" => Some(TextEncoding::Big5),
        "GB18030" | "GB2312" | "GBK" => Some(TextEncoding::Gbk),
        "SHIFTJIS" | "SHIFT-JIS" | "SJIS" => Some(TextEncoding::ShiftJis),
        "EUCJP" | "EUC-JP" => Some(TextEncoding::EucJp),
        "ISO2022JP" | "ISO-2022-JP" => Some(TextEncoding::Iso2022Jp),
        _ => None,
    }
}

fn looks_like_hz(buffer: &[u8]) -> bool {
    buffer
        .windows(3)
        .any(|window| window[0] == b'~' && window[1] == b'{' && window[2].is_ascii_graphic())
}

fn strip_bom(buffer: &[u8], encoding: TextEncoding) -> &[u8] {
    match encoding {
        TextEncoding::Utf8 | TextEncoding::Utf8Bom if buffer.starts_with(&[0xef, 0xbb, 0xbf]) => {
            &buffer[3..]
        }
        TextEncoding::Utf16le if buffer.starts_with(&[0xff, 0xfe]) => &buffer[2..],
        TextEncoding::Utf16be if buffer.starts_with(&[0xfe, 0xff]) => &buffer[2..],
        _ => buffer,
    }
}

fn prepend_bom(output: &mut Vec<u8>, encoding: TextEncoding) {
    let bom: &[u8] = match encoding {
        TextEncoding::Utf8 | TextEncoding::Utf8Bom => &[0xef, 0xbb, 0xbf],
        TextEncoding::Utf16le => &[0xff, 0xfe],
        TextEncoding::Utf16be => &[0xfe, 0xff],
        _ => return,
    };
    if !output.starts_with(bom) {
        let mut with_bom = bom.to_vec();
        with_bom.append(output);
        *output = with_bom;
    }
}

fn decode_hz(buffer: &[u8]) -> Result<String, CoreError> {
    let mut bytes = Vec::new();
    let mut ascii = Vec::new();
    let mut chinese = false;
    let mut index = 0;
    let flush_ascii = |ascii: &mut Vec<u8>, bytes: &mut Vec<u8>| {
        bytes.append(ascii);
    };
    while index < buffer.len() {
        let value = buffer[index];
        if value == b'~' && index + 1 < buffer.len() {
            match buffer[index + 1] {
                b'{' => {
                    flush_ascii(&mut ascii, &mut bytes);
                    chinese = true;
                    index += 2;
                    continue;
                }
                b'}' => {
                    chinese = false;
                    index += 2;
                    continue;
                }
                b'~' => {
                    ascii.push(b'~');
                    index += 2;
                    continue;
                }
                b'\n' => {
                    index += 2;
                    continue;
                }
                _ => {}
            }
        }
        if chinese {
            if index + 1 >= buffer.len() {
                break;
            }
            flush_ascii(&mut ascii, &mut bytes);
            bytes.push(value | 0x80);
            bytes.push(buffer[index + 1] | 0x80);
            index += 2;
        } else {
            ascii.push(value);
            index += 1;
        }
    }
    flush_ascii(&mut ascii, &mut bytes);
    let (text, _, _) = encoding_rs::GBK.decode(&bytes);
    Ok(text.into_owned())
}

fn encode_hz(text: &str) -> Result<Vec<u8>, CoreError> {
    let mut output = String::new();
    let mut chinese = false;
    for character in text.chars() {
        let mut buf = [0; 4];
        let encoded_char = character.encode_utf8(&mut buf);
        let (encoded, _, _) = encoding_rs::GBK.encode(encoded_char);
        let is_ascii = encoded.len() == 1 && encoded[0] < 0x80;
        if is_ascii {
            if chinese {
                output.push_str("~}");
                chinese = false;
            }
            if character == '~' {
                output.push_str("~~");
            } else {
                output.push(character);
            }
            continue;
        }
        if encoded.len() != 2 {
            return Err(CoreError::new(
                "HZ_CHARACTER",
                format!("字元「{character}」無法使用 HZ-GB-2312 表示。"),
            ));
        }
        if !chinese {
            output.push_str("~{");
            chinese = true;
        }
        output.push((encoded[0] & 0x7f) as char);
        output.push((encoded[1] & 0x7f) as char);
    }
    if chinese {
        output.push_str("~}");
    }
    Ok(output.into_bytes())
}

#[cfg(test)]
mod tests;

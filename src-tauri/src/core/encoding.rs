use super::error::CoreError;
use super::types::TextEncoding;
use encoding_rs::{
    CoderResult, Decoder, Encoder, Encoding, BIG5, EUC_JP, GBK, ISO_2022_JP, SHIFT_JIS, UTF_16BE,
    UTF_16LE, UTF_8,
};
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

#[derive(Debug)]
pub struct DecodedText {
    pub text: String,
    pub encoding: TextEncoding,
    /// 解碼器無法對應的位元組，或 UTF-16 的奇數長度與落單 surrogate。
    pub had_errors: bool,
}

pub fn decode_text(
    buffer: &[u8],
    requested: TextEncoding,
) -> Result<(String, TextEncoding), CoreError> {
    let decoded = decode_text_detailed(buffer, requested)?;
    Ok((decoded.text, decoded.encoding))
}

pub fn decode_text_detailed(
    buffer: &[u8],
    requested: TextEncoding,
) -> Result<DecodedText, CoreError> {
    let encoding = if requested == TextEncoding::Auto {
        detect_encoding(buffer)
    } else {
        requested
    };
    let without_bom = strip_bom(buffer, encoding);
    if encoding == TextEncoding::HzGb2312 {
        let (text, had_errors) = decode_hz(without_bom)?;
        return Ok(DecodedText {
            text,
            encoding,
            had_errors,
        });
    }
    if encoding == TextEncoding::Utf16le || encoding == TextEncoding::Utf16be {
        let codec = if encoding == TextEncoding::Utf16le {
            UTF_16LE
        } else {
            UTF_16BE
        };
        // BOM 已先去掉。`decode` 會再嗅探 BOM 並可能換端序，所以用 without_bom_handling。
        let (text, decode_errors) = codec.decode_without_bom_handling(without_bom);
        // 奇數長度會丟掉最後一個位元組。encoding_rs 會標成錯誤；這裡仍明確算錯，避免靜默寫回。
        let had_errors = decode_errors || without_bom.len() % 2 == 1;
        return Ok(DecodedText {
            text: text.into_owned(),
            encoding,
            had_errors,
        });
    }
    let Some(codec) = encoding_codec(encoding) else {
        return Err(CoreError::new(
            "ENCODING_UNSUPPORTED",
            format!("不支援編碼 {encoding:?}。"),
        ));
    };
    let (text, _, had_errors) = codec.decode(without_bom);
    Ok(DecodedText {
        text: text.into_owned(),
        encoding,
        had_errors,
    })
}

#[derive(Debug)]
pub struct EncodedText {
    pub bytes: Vec<u8>,
    /// 編碼器無法表示的字元。encoding_rs 預設改寫成 `&#數字;`，不可當成成功。
    pub had_errors: bool,
}

pub fn encode_text(
    text: &str,
    encoding: TextEncoding,
    add_bom: bool,
) -> Result<Vec<u8>, CoreError> {
    Ok(encode_text_detailed(text, encoding, add_bom)?.bytes)
}

pub fn encode_text_detailed(
    text: &str,
    encoding: TextEncoding,
    add_bom: bool,
) -> Result<EncodedText, CoreError> {
    if encoding == TextEncoding::Auto {
        return Err(CoreError::new(
            "ENCODING_AUTO_OUTPUT",
            "輸出編碼不能使用自動偵測。",
        ));
    }
    if encoding == TextEncoding::HzGb2312 {
        return Ok(EncodedText {
            bytes: encode_hz(text)?,
            had_errors: false,
        });
    }
    if encoding == TextEncoding::Utf16le || encoding == TextEncoding::Utf16be {
        let mut output = encode_utf16(text, encoding == TextEncoding::Utf16le);
        if add_bom {
            prepend_bom(&mut output, encoding);
        }
        return Ok(EncodedText {
            bytes: output,
            had_errors: false,
        });
    }
    let Some(codec) = encoding_codec(encoding) else {
        return Err(CoreError::new(
            "ENCODING_UNSUPPORTED",
            format!("不支援編碼 {encoding:?}。"),
        ));
    };
    let (encoded, _, had_errors) = codec.encode(text);
    let mut output = encoded.into_owned();
    if add_bom || encoding == TextEncoding::Utf8Bom {
        prepend_bom(&mut output, encoding);
    }
    Ok(EncodedText {
        bytes: output,
        had_errors,
    })
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
    let encoded = encode_text_detailed(text, source, false)?;
    if encoded.had_errors {
        return Err(CoreError::new(
            "ENCODING_UNREPRESENTABLE",
            format!("有字元無法用 {source:?} 表示，已中止重新解讀。"),
        ));
    }
    let decoded = decode_text_detailed(&encoded.bytes, target)?;
    if decoded.had_errors {
        return Err(CoreError::new(
            "ENCODING_DECODE_ERROR",
            format!("以 {target:?} 解碼時遇到無法對應的位元組，已中止重新解讀。"),
        ));
    }
    Ok(decoded.text)
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

fn decode_hz(buffer: &[u8]) -> Result<(String, bool), CoreError> {
    HzDecoder::new().push(buffer, true)
}

struct HzDecoder {
    chinese: bool,
    pending: Option<u8>,
    gbk: Decoder,
}

impl HzDecoder {
    fn new() -> Self {
        Self {
            chinese: false,
            pending: None,
            gbk: GBK.new_decoder_without_bom_handling(),
        }
    }

    fn push(&mut self, bytes: &[u8], last: bool) -> Result<(String, bool), CoreError> {
        let mut framed = Vec::with_capacity(bytes.len() + 1);
        for &byte in bytes {
            self.push_byte(byte, &mut framed);
        }
        if last {
            if let Some(pending) = self.pending.take() {
                if pending == b'~' && !self.chinese {
                    framed.push(b'~');
                }
            }
        }
        decode_codec(&mut self.gbk, &framed, last)
    }

    fn push_byte(&mut self, byte: u8, framed: &mut Vec<u8>) {
        if let Some(first) = self.pending.take() {
            self.push_pair(first, byte, framed);
            return;
        }
        if byte == b'~' {
            self.pending = Some(b'~');
            return;
        }
        if self.chinese {
            self.pending = Some(byte);
            return;
        }
        framed.push(byte);
    }

    fn push_pair(&mut self, first: u8, second: u8, framed: &mut Vec<u8>) {
        if first == b'~' {
            match second {
                b'{' => self.chinese = true,
                b'}' => self.chinese = false,
                b'~' => framed.push(b'~'),
                b'\n' => {}
                _ if self.chinese => {
                    framed.push(first | 0x80);
                    framed.push(second | 0x80);
                }
                _ => {
                    framed.push(first);
                    self.push_byte(second, framed);
                }
            }
            return;
        }
        framed.push(first | 0x80);
        framed.push(second | 0x80);
    }
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

/// Incremental decoder. BOM is stripped from the first bytes of the first `push`.
pub struct StreamDecoder {
    inner: DecoderInner,
    bom_expected: &'static [u8],
    bom_buf: Vec<u8>,
}

enum DecoderInner {
    Codec(Decoder),
    Hz(HzDecoder),
}

impl StreamDecoder {
    pub fn new(encoding: TextEncoding) -> Result<Self, CoreError> {
        let bom_expected: &'static [u8] = match encoding {
            TextEncoding::Utf8 | TextEncoding::Utf8Bom => &[0xef, 0xbb, 0xbf],
            TextEncoding::Utf16le => &[0xff, 0xfe],
            TextEncoding::Utf16be => &[0xfe, 0xff],
            _ => &[],
        };
        let inner = match encoding {
            TextEncoding::HzGb2312 => DecoderInner::Hz(HzDecoder::new()),
            other => {
                let codec = encoding_codec(other).ok_or_else(|| {
                    CoreError::new("ENCODING_UNSUPPORTED", format!("不支援編碼 {other:?}。"))
                })?;
                DecoderInner::Codec(codec.new_decoder_without_bom_handling())
            }
        };
        Ok(Self {
            inner,
            bom_expected,
            bom_buf: Vec::new(),
        })
    }

    pub fn push(&mut self, mut bytes: &[u8], last: bool) -> Result<(String, bool), CoreError> {
        if !self.bom_expected.is_empty() {
            let need = self.bom_expected.len().saturating_sub(self.bom_buf.len());
            if need > 0 && !bytes.is_empty() {
                let take = need.min(bytes.len());
                self.bom_buf.extend_from_slice(&bytes[..take]);
                bytes = &bytes[take..];
            }
            if self.bom_buf.len() < self.bom_expected.len() && !last {
                return Ok((String::new(), false));
            }
            let expected = self.bom_expected;
            self.bom_expected = &[];
            if self.bom_buf == expected {
                self.bom_buf.clear();
            }
            if !self.bom_buf.is_empty() {
                let prefix = std::mem::take(&mut self.bom_buf);
                let (mut text, mut errors) =
                    self.decode_inner(&prefix, last && bytes.is_empty())?;
                if !bytes.is_empty() || last {
                    let (more, more_errors) = self.decode_inner(bytes, last)?;
                    text.push_str(&more);
                    errors |= more_errors;
                }
                return Ok((text, errors));
            }
        }
        self.decode_inner(bytes, last)
    }

    fn decode_inner(&mut self, bytes: &[u8], last: bool) -> Result<(String, bool), CoreError> {
        match &mut self.inner {
            DecoderInner::Codec(decoder) => decode_codec(decoder, bytes, last),
            DecoderInner::Hz(hz) => hz.push(bytes, last),
        }
    }
}

fn decode_codec(
    decoder: &mut Decoder,
    bytes: &[u8],
    last: bool,
) -> Result<(String, bool), CoreError> {
    let mut output = String::new();
    let mut offset = 0;
    let mut had_errors = false;
    loop {
        let remaining = bytes.len() - offset;
        if let Some(needed) = decoder.max_utf8_buffer_length(remaining) {
            output.reserve(needed);
        }
        let (result, read, errors) = decoder.decode_to_string(
            &bytes[offset..],
            &mut output,
            last && offset + remaining == bytes.len(),
        );
        had_errors |= errors;
        offset += read;
        match result {
            CoderResult::InputEmpty => break,
            CoderResult::OutputFull => continue,
        }
    }
    Ok((output, had_errors))
}

/// Incremental encoder. Writes BOM on the first non-empty output when requested.
pub struct StreamEncoder {
    encoding: TextEncoding,
    add_bom: bool,
    bom_done: bool,
    inner: EncoderInner,
}

enum EncoderInner {
    Codec(Encoder),
    Utf16 { little: bool },
    Hz { chinese: bool },
}

impl StreamEncoder {
    pub fn new(encoding: TextEncoding, add_bom: bool) -> Result<Self, CoreError> {
        if encoding == TextEncoding::Auto {
            return Err(CoreError::new(
                "ENCODING_AUTO_OUTPUT",
                "輸出編碼不能使用自動偵測。",
            ));
        }
        let inner = match encoding {
            TextEncoding::HzGb2312 => EncoderInner::Hz { chinese: false },
            TextEncoding::Utf16le => EncoderInner::Utf16 { little: true },
            TextEncoding::Utf16be => EncoderInner::Utf16 { little: false },
            other => {
                let codec = encoding_codec(other).ok_or_else(|| {
                    CoreError::new("ENCODING_UNSUPPORTED", format!("不支援編碼 {other:?}。"))
                })?;
                EncoderInner::Codec(codec.new_encoder())
            }
        };
        Ok(Self {
            encoding,
            add_bom: add_bom || encoding == TextEncoding::Utf8Bom,
            bom_done: false,
            inner,
        })
    }

    pub fn push(&mut self, text: &str, last: bool) -> Result<Vec<u8>, CoreError> {
        let mut output = match &mut self.inner {
            EncoderInner::Codec(encoder) => encode_codec(encoder, text, last)?,
            EncoderInner::Utf16 { little } => encode_utf16(text, *little),
            EncoderInner::Hz { chinese } => encode_hz_push(text, last, chinese)?,
        };
        if !self.bom_done {
            if self.add_bom && (!output.is_empty() || last) {
                prepend_bom(&mut output, self.encoding);
                self.bom_done = true;
            } else if !output.is_empty() {
                self.bom_done = true;
            }
        }
        Ok(output)
    }
}

fn encode_codec(encoder: &mut Encoder, text: &str, last: bool) -> Result<Vec<u8>, CoreError> {
    let mut output = Vec::new();
    let mut offset = 0;
    loop {
        let remaining = text.len() - offset;
        if let Some(needed) = encoder.max_buffer_length_from_utf8_if_no_unmappables(remaining) {
            output.reserve(needed);
        }
        let (result, read, had_errors) =
            encoder.encode_from_utf8_to_vec(&text[offset..], &mut output, last);
        let _ = had_errors;
        offset += read;
        match result {
            CoderResult::InputEmpty => break,
            CoderResult::OutputFull => continue,
        }
    }
    Ok(output)
}

fn encode_hz_push(text: &str, last: bool, chinese: &mut bool) -> Result<Vec<u8>, CoreError> {
    let mut output = String::new();
    for character in text.chars() {
        let mut buf = [0; 4];
        let encoded_char = character.encode_utf8(&mut buf);
        let (encoded, _, _) = encoding_rs::GBK.encode(encoded_char);
        let is_ascii = encoded.len() == 1 && encoded[0] < 0x80;
        if is_ascii {
            if *chinese {
                output.push_str("~}");
                *chinese = false;
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
        if !*chinese {
            output.push_str("~{");
            *chinese = true;
        }
        output.push((encoded[0] & 0x7f) as char);
        output.push((encoded[1] & 0x7f) as char);
    }
    if last && *chinese {
        output.push_str("~}");
        *chinese = false;
    }
    Ok(output.into_bytes())
}

#[cfg(test)]
mod tests;

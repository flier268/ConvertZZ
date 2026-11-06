//! Preview only reads a prefix. Apply streams decode → convert → encode → stage.

use super::super::conversion::is_cjk_char as cjk_char;
use super::super::encoding::{detect_encoding, StreamDecoder, StreamEncoder};
use super::{
    inspect_head, CancelCheck, ContentSkip, CoreError, InspectedBytes, TextEncoding,
    STREAM_CHUNK_BYTES,
};
use std::fs::{self, File};
use std::future::Future;
use std::io::{Read, Write};
use std::path::Path;

/// Convert-window length. Larger than the segmenter's 24-unit enum span.
pub(super) const STREAM_FLUSH_CHARS: usize = 8_192;
/// CJK context on each side of an emit cut. Must exceed the segmenter's 24-unit enum span.
pub(super) const STREAM_OVERLAP_CHARS: usize = 32;
const CHARSET_HEAD_CHARS: usize = 16 * 1024;
const DETECT_SAMPLE_BYTES: usize = 128 * 1024;
const SENTENCE_ENDS: &[char] = &['。', '！', '？', '!', '?', '\n'];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WindowKind {
    CjkOverlap,
    Sentence,
}

pub(super) struct ConvertedPiece {
    pub text: String,
    pub warnings: Vec<String>,
}

pub(super) struct Emit {
    pub text: String,
    /// Already-emitted lookbehind at the start of `text`, in original chars.
    pub skip_chars: usize,
    /// Lookahead at the end of `text` that the next window will emit.
    pub hold_chars: usize,
}

pub(super) struct RunBuffer {
    buf: String,
    cjk_skip: usize,
}

impl RunBuffer {
    pub(super) fn new() -> Self {
        Self {
            buf: String::new(),
            cjk_skip: 0,
        }
    }

    pub(super) fn push(&mut self, text: &str, flush: usize, overlap: usize) -> Vec<Emit> {
        self.buf.push_str(text);
        self.drain(true, flush, overlap)
    }

    pub(super) fn finish(&mut self, flush: usize, overlap: usize) -> Vec<Emit> {
        self.drain(false, flush, overlap)
    }

    fn drain(&mut self, keep_open: bool, flush: usize, overlap: usize) -> Vec<Emit> {
        let mut emits = Vec::new();
        loop {
            if self.buf.is_empty() {
                break;
            }
            let run_cjk = cjk_char(self.buf.chars().next().expect("non-empty"));
            let run_chars = count_run_chars(&self.buf, run_cjk);
            let complete = run_end_offset(&self.buf, run_chars) < self.buf.len() || !keep_open;
            if !complete && run_chars <= flush {
                break;
            }
            let window = if complete && run_chars <= flush {
                run_chars
            } else {
                flush.min(run_chars)
            };
            let piece = take_chars(&self.buf, window);
            let skip = if run_cjk {
                std::mem::take(&mut self.cjk_skip)
            } else {
                0
            };
            let split_cjk = run_cjk && window < run_chars;
            let hold = if split_cjk {
                overlap.min(window.saturating_sub(skip))
            } else {
                0
            };
            emits.push(Emit {
                text: piece,
                skip_chars: skip,
                hold_chars: hold,
            });
            if split_cjk {
                let keep = overlap.saturating_mul(2).min(window);
                self.cjk_skip = keep.saturating_sub(hold);
                self.buf = skip_chars(&self.buf, window - keep).to_string();
            } else {
                self.buf = skip_chars(&self.buf, window).to_string();
            }
        }
        emits
    }
}

struct SentenceBuffer {
    buf: String,
}

impl SentenceBuffer {
    fn new() -> Self {
        Self { buf: String::new() }
    }

    fn push(&mut self, text: &str, max_chars: usize) -> Vec<Emit> {
        self.buf.push_str(text);
        self.drain(false, max_chars)
    }

    fn finish(&mut self, max_chars: usize) -> Vec<Emit> {
        self.drain(true, max_chars)
    }

    fn drain(&mut self, last: bool, max_chars: usize) -> Vec<Emit> {
        let mut emits = Vec::new();
        while let Some(take) = next_sentence_take(&self.buf, max_chars, last) {
            emits.push(Emit {
                text: take_chars(&self.buf, take),
                skip_chars: 0,
                hold_chars: 0,
            });
            self.buf = skip_chars(&self.buf, take).to_string();
        }
        emits
    }
}

fn next_sentence_take(text: &str, max_chars: usize, last: bool) -> Option<usize> {
    let n = text.chars().count();
    if n == 0 {
        return None;
    }
    if last && n <= max_chars {
        return Some(n);
    }
    if let Some(end) = last_sentence_end_before(text, max_chars.min(n)) {
        return Some(end);
    }
    if n >= max_chars {
        return Some(max_chars);
    }
    if last {
        return Some(n);
    }
    None
}

fn last_sentence_end_before(text: &str, max_chars: usize) -> Option<usize> {
    let mut last = None;
    for (index, character) in text.chars().take(max_chars).enumerate() {
        if SENTENCE_ENDS.contains(&character) {
            last = Some(index + 1);
        }
    }
    last
}

enum Windows {
    Cjk(RunBuffer),
    Sentence(SentenceBuffer),
}

impl Windows {
    fn new(kind: WindowKind) -> Self {
        match kind {
            WindowKind::CjkOverlap => Self::Cjk(RunBuffer::new()),
            WindowKind::Sentence => Self::Sentence(SentenceBuffer::new()),
        }
    }

    fn push(&mut self, text: &str) -> Vec<Emit> {
        match self {
            Self::Cjk(runs) => runs.push(text, STREAM_FLUSH_CHARS, STREAM_OVERLAP_CHARS),
            Self::Sentence(runs) => runs.push(text, STREAM_FLUSH_CHARS),
        }
    }

    fn finish(&mut self) -> Vec<Emit> {
        match self {
            Self::Cjk(runs) => runs.finish(STREAM_FLUSH_CHARS, STREAM_OVERLAP_CHARS),
            Self::Sentence(runs) => runs.finish(STREAM_FLUSH_CHARS),
        }
    }
}

fn count_run_chars(text: &str, cjk: bool) -> usize {
    text.chars()
        .take_while(|character| cjk_char(*character) == cjk)
        .count()
}

fn run_end_offset(text: &str, chars: usize) -> usize {
    match text.char_indices().nth(chars) {
        Some((index, _)) => index,
        None => text.len(),
    }
}

fn take_chars(text: &str, chars: usize) -> String {
    text.chars().take(chars).collect()
}

pub(super) fn skip_chars(text: &str, chars: usize) -> &str {
    match text.char_indices().nth(chars) {
        Some((index, _)) => &text[index..],
        None => "",
    }
}

pub(super) fn preview_byte_budget(encoding: TextEncoding, max_chars: usize) -> usize {
    let per = match encoding {
        TextEncoding::Utf16le | TextEncoding::Utf16be => 2,
        TextEncoding::Utf8
        | TextEncoding::Utf8Bom
        | TextEncoding::Iso2022Jp
        | TextEncoding::HzGb2312 => 4,
        _ => 2,
    };
    max_chars.saturating_mul(per).saturating_add(16)
}

pub(super) struct RollingHash {
    pub len: u64,
    hash: u64,
}

impl RollingHash {
    pub(super) fn new() -> Self {
        Self {
            len: 0,
            hash: 0xcbf2_9ce4_8422_2325,
        }
    }

    pub(super) fn update(&mut self, bytes: &[u8]) {
        self.len += bytes.len() as u64;
        for &byte in bytes {
            self.hash = self
                .hash
                .wrapping_mul(0x100_0000_01b3)
                .wrapping_add(byte as u64);
        }
    }
}

impl PartialEq for RollingHash {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.hash == other.hash
    }
}

pub(super) fn hash_file(path: &Path) -> Result<RollingHash, CoreError> {
    let mut file = File::open(path)?;
    let mut buffer = vec![0u8; STREAM_CHUNK_BYTES];
    let mut hash = RollingHash::new();
    loop {
        let read = super::read_chunk(&mut file, &mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(hash)
}

#[derive(Debug)]
pub(super) struct StreamWrite {
    pub encoding: TextEncoding,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub(super) enum StreamResult {
    Skip(ContentSkip),
    Written(StreamWrite),
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn stream_convert_path<C, Fut>(
    source: &Path,
    stage: &Path,
    requested: TextEncoding,
    output_encoding: TextEncoding,
    add_bom: bool,
    window: WindowKind,
    mut convert: C,
    mut polish: impl FnMut(&mut String, bool),
    mut progress: impl FnMut(u64, u64),
    is_cancelled: Option<&CancelCheck>,
) -> Result<StreamResult, CoreError>
where
    C: FnMut(&str, usize, usize) -> Fut,
    Fut: Future<Output = Result<ConvertedPiece, CoreError>>,
{
    let mut file = File::open(source)?;
    let size = file.metadata()?.len();
    let mut head = Vec::with_capacity(super::INSPECT_HEAD_BYTES.min(size as usize));
    (&mut file)
        .take(super::INSPECT_HEAD_BYTES as u64)
        .read_to_end(&mut head)?;
    if let Some(reason) = inspect_head(&head, requested) {
        return Ok(StreamResult::Skip(reason));
    }
    if requested == TextEncoding::Auto
        && (head.len() as u64) < size
        && head.len() < DETECT_SAMPLE_BYTES
    {
        let extra = DETECT_SAMPLE_BYTES
            .saturating_sub(head.len())
            .min(size.saturating_sub(head.len() as u64) as usize);
        head.reserve(extra);
        (&mut file).take(extra as u64).read_to_end(&mut head)?;
    }
    let encoding = if requested == TextEncoding::Auto {
        detect_encoding(&head)
    } else {
        requested
    };
    if !matches!(encoding, TextEncoding::Utf16le | TextEncoding::Utf16be) && head.contains(&0) {
        return Ok(StreamResult::Skip(ContentSkip::Binary));
    }
    let output_encoding = super::resolve_output_encoding(output_encoding, Some(encoding));

    let utf16 = matches!(encoding, TextEncoding::Utf16le | TextEncoding::Utf16be);
    let mut job = StreamJob {
        encoding,
        utf16,
        decoder: StreamDecoder::new(encoding)?,
        encoder: StreamEncoder::new(output_encoding, add_bom)?,
        windows: Windows::new(window),
        writer: fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(stage)
            .map_err(|error| CoreError::new("FILE_STAGE", format!("無法建立暫存檔：{error}")))?,
        written: RollingHash::new(),
        decoded_errors: false,
        decoded_chars: 0,
        replacements: 0,
        read_bytes: 0,
        size,
        head_text: String::new(),
        head_done: false,
        convert: &mut convert,
        polish: &mut polish,
        progress: &mut progress,
        warnings: Vec::new(),
        is_cancelled,
    };
    let result = job.run(&mut file, &head, source, stage).await;
    if matches!(&result, Ok(StreamResult::Skip(_)) | Err(_)) {
        let _ = fs::remove_file(stage);
    }
    result
}

struct StreamJob<'a, C, P, R> {
    encoding: TextEncoding,
    utf16: bool,
    decoder: StreamDecoder,
    encoder: StreamEncoder,
    windows: Windows,
    writer: File,
    written: RollingHash,
    decoded_errors: bool,
    decoded_chars: usize,
    replacements: usize,
    read_bytes: u64,
    size: u64,
    head_text: String,
    head_done: bool,
    convert: &'a mut C,
    polish: &'a mut P,
    progress: &'a mut R,
    warnings: Vec<String>,
    is_cancelled: Option<&'a CancelCheck>,
}

impl<C, Fut, P, R> StreamJob<'_, C, P, R>
where
    C: FnMut(&str, usize, usize) -> Fut,
    Fut: Future<Output = Result<ConvertedPiece, CoreError>>,
    P: FnMut(&mut String, bool),
    R: FnMut(u64, u64),
{
    async fn run(
        &mut self,
        file: &mut File,
        head: &[u8],
        source: &Path,
        stage: &Path,
    ) -> Result<StreamResult, CoreError> {
        let size = self.size;
        if let Some(skip) = self.feed(head, size as usize <= head.len()).await? {
            return Ok(StreamResult::Skip(skip));
        }
        if (head.len() as u64) < size {
            let mut buffer = vec![0u8; STREAM_CHUNK_BYTES];
            loop {
                let read = super::read_chunk(file, &mut buffer)?;
                if read == 0 {
                    break;
                }
                if let Some(skip) = self.feed(&buffer[..read], false).await? {
                    return Ok(StreamResult::Skip(skip));
                }
            }
            if let Some(skip) = self.feed(&[], true).await? {
                return Ok(StreamResult::Skip(skip));
            }
        }
        if self.decoded_errors {
            return Ok(StreamResult::Skip(ContentSkip::DecodeError));
        }
        if self.utf16
            && self.decoded_chars > 0
            && self.replacements >= super::REPLACEMENT_MIN_COUNT
            && (self.replacements as f64) / (self.decoded_chars as f64) >= super::REPLACEMENT_RATIO
        {
            return Ok(StreamResult::Skip(ContentSkip::Binary));
        }
        if !self.head_done {
            (self.polish)(&mut self.head_text, true);
            let head = std::mem::take(&mut self.head_text);
            write_encoded(
                &mut self.writer,
                &mut self.encoder,
                &mut self.written,
                &head,
                true,
            )?;
        } else {
            write_encoded(
                &mut self.writer,
                &mut self.encoder,
                &mut self.written,
                "",
                true,
            )?;
        }
        self.writer.sync_all()?;
        if let Ok(source_meta) = fs::metadata(source) {
            let _ = self.writer.set_permissions(source_meta.permissions());
        }
        let on_disk = hash_file(stage)?;
        if on_disk != self.written {
            return Err(CoreError::with_details(
                "FILE_VERIFY",
                "暫存檔寫入驗證失敗。",
                serde_json::json!({ "path": stage }),
            ));
        }
        Ok(StreamResult::Written(StreamWrite {
            encoding: self.encoding,
            warnings: std::mem::take(&mut self.warnings),
        }))
    }

    async fn feed(&mut self, bytes: &[u8], last: bool) -> Result<Option<ContentSkip>, CoreError> {
        if self.is_cancelled.is_some_and(|check| check()) {
            return Err(CoreError::new("PLAN_CANCELLED", "檔案作業已由使用者取消。"));
        }
        if !self.utf16 && bytes.contains(&0) {
            return Ok(Some(ContentSkip::Binary));
        }
        self.read_bytes = self
            .read_bytes
            .saturating_add(bytes.len() as u64)
            .min(self.size);
        (self.progress)(self.read_bytes, self.size.max(1));
        let (text, errors) = self.decoder.push(bytes, last)?;
        self.decoded_errors |= errors;
        for character in text.chars() {
            self.decoded_chars += 1;
            if character == '\u{FFFD}' {
                self.replacements += 1;
            }
        }
        let emits = if last {
            let mut rest = self.windows.push(&text);
            rest.extend(self.windows.finish());
            rest
        } else {
            self.windows.push(&text)
        };
        self.consume(emits, last).await?;
        Ok(None)
    }

    async fn consume(&mut self, emits: Vec<Emit>, last: bool) -> Result<(), CoreError> {
        for emit in emits {
            let piece = (self.convert)(&emit.text, emit.skip_chars, emit.hold_chars).await?;
            for warning in piece.warnings {
                if !self.warnings.contains(&warning) {
                    self.warnings.push(warning);
                }
            }
            let mut converted = piece.text;
            if !self.head_done {
                self.head_text.push_str(&converted);
                let ready = last || self.head_text.chars().count() >= CHARSET_HEAD_CHARS;
                if ready {
                    (self.polish)(&mut self.head_text, true);
                    let head = std::mem::take(&mut self.head_text);
                    write_encoded(
                        &mut self.writer,
                        &mut self.encoder,
                        &mut self.written,
                        &head,
                        false,
                    )?;
                    self.head_done = true;
                }
            } else {
                (self.polish)(&mut converted, false);
                write_encoded(
                    &mut self.writer,
                    &mut self.encoder,
                    &mut self.written,
                    &converted,
                    false,
                )?;
            }
        }
        Ok(())
    }
}

fn write_encoded(
    writer: &mut File,
    encoder: &mut StreamEncoder,
    hash: &mut RollingHash,
    text: &str,
    last: bool,
) -> Result<(), CoreError> {
    let bytes = encoder.push(text, last)?;
    if !bytes.is_empty() {
        writer.write_all(&bytes)?;
        hash.update(&bytes);
    }
    Ok(())
}

pub(super) fn inspect_preview_from_reader(
    reader: &mut impl Read,
    size: u64,
    requested: TextEncoding,
    max_chars: usize,
) -> Result<InspectedBytes, CoreError> {
    let mut head = Vec::with_capacity(super::INSPECT_HEAD_BYTES.min(size as usize));
    reader
        .take(super::INSPECT_HEAD_BYTES as u64)
        .read_to_end(&mut head)?;
    if let Some(reason) = inspect_head(&head, requested) {
        return Ok(InspectedBytes::Skip(reason));
    }
    let encoding = if requested == TextEncoding::Auto {
        detect_encoding(&head)
    } else {
        requested
    };
    if !matches!(encoding, TextEncoding::Utf16le | TextEncoding::Utf16be) && head.contains(&0) {
        return Ok(InspectedBytes::Skip(ContentSkip::Binary));
    }
    let budget = preview_byte_budget(encoding, max_chars).max(head.len());
    let mut buffer = head;
    if (buffer.len() as u64) < size && buffer.len() < budget {
        let extra = budget
            .saturating_sub(buffer.len())
            .min(size.saturating_sub(buffer.len() as u64) as usize);
        buffer.reserve(extra);
        reader.take(extra as u64).read_to_end(&mut buffer)?;
    }
    if !matches!(encoding, TextEncoding::Utf16le | TextEncoding::Utf16be) && buffer.contains(&0) {
        return Ok(InspectedBytes::Skip(ContentSkip::Binary));
    }
    let mut decoder = StreamDecoder::new(encoding)?;
    let last = (buffer.len() as u64) >= size;
    let (text, errors) = decoder.push(&buffer, last)?;
    if errors {
        return Ok(InspectedBytes::Skip(ContentSkip::DecodeError));
    }
    if matches!(encoding, TextEncoding::Utf16le | TextEncoding::Utf16be)
        && super::replacement_ratio_too_high(&text)
    {
        return Ok(InspectedBytes::Skip(ContentSkip::Binary));
    }
    Ok(InspectedBytes::Text {
        text: text.chars().take(max_chars).collect(),
        encoding,
    })
}

pub(super) fn slice_chars(text: &str, skip: usize, hold: usize) -> String {
    let total = text.chars().count();
    let start = skip.min(total);
    let end = total.saturating_sub(hold).max(start);
    take_chars(skip_chars(text, start), end - start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::conversion::is_cjk_char;
    use std::io::Read;

    #[test]
    fn overlap_windows_cover_long_cjk_run() {
        let mut runs = RunBuffer::new();
        let text: String = "里面开发".chars().cycle().take(100).collect();
        let emits = {
            let mut items = runs.push(&text, 16, 4);
            items.extend(runs.finish(16, 4));
            items
        };
        let mut rebuilt = String::new();
        for emit in &emits {
            rebuilt.push_str(&slice_chars(&emit.text, emit.skip_chars, emit.hold_chars));
        }
        assert_eq!(rebuilt, text);
        assert!(text.chars().all(is_cjk_char));
        assert!(emits.len() > 1);
        assert!(emits.iter().any(|emit| emit.skip_chars > 0));
        assert!(emits.iter().any(|emit| emit.hold_chars > 0));
    }

    #[test]
    fn preview_stops_after_inspect_head() {
        let payload = "软件".repeat(80_000);
        let bytes = payload.as_bytes();
        assert!(bytes.len() > super::super::INSPECT_HEAD_BYTES * 2);
        let mut reader = CountRead {
            inner: std::io::Cursor::new(bytes),
            n: 0,
        };
        let inspected =
            inspect_preview_from_reader(&mut reader, bytes.len() as u64, TextEncoding::Utf8, 1024)
                .unwrap();
        match inspected {
            InspectedBytes::Text { text, .. } => {
                assert_eq!(text, payload.chars().take(1024).collect::<String>());
            }
            other => panic!("expected text, got {other:?}"),
        }
        assert!(reader.n <= super::super::INSPECT_HEAD_BYTES);
        assert!(reader.n < bytes.len());
    }

    #[tokio::test]
    async fn stream_identity_roundtrip_long_mixed() {
        let dir = std::env::temp_dir().join(format!("convertzz-stream-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("in.txt");
        let stage = dir.join("out.txt");
        let text = format!(
            "{}ABC{}。\nend",
            "里面开发".repeat(3_000),
            "软件".repeat(2_000)
        );
        std::fs::write(&source, &text).unwrap();
        let result = stream_convert_path(
            &source,
            &stage,
            TextEncoding::Utf8,
            TextEncoding::Utf8,
            false,
            WindowKind::CjkOverlap,
            |piece, skip, hold| {
                let text = slice_chars(piece, skip, hold);
                async move {
                    Ok(ConvertedPiece {
                        text,
                        warnings: Vec::new(),
                    })
                }
            },
            |_, _| {},
            |_, _| {},
            None,
        )
        .await
        .unwrap();
        assert!(matches!(result, StreamResult::Written(_)), "{result:?}");
        assert_eq!(std::fs::read_to_string(&stage).unwrap(), text);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sentence_windows_flush_on_terminator_without_overlap() {
        let mut buf = SentenceBuffer::new();
        let text = format!("{}。{}！rest", "里".repeat(20), "面".repeat(20));
        let emits = {
            let mut items = buf.push(&text, 16);
            items.extend(buf.finish(16));
            items
        };
        let rebuilt: String = emits.iter().map(|emit| emit.text.as_str()).collect();
        assert_eq!(rebuilt, text);
        assert!(emits
            .iter()
            .all(|emit| emit.skip_chars == 0 && emit.hold_chars == 0));
        assert!(emits.iter().any(|emit| emit.text.ends_with('。')));
        assert!(emits.len() > 1);
    }

    struct CountRead<R> {
        inner: R,
        n: usize,
    }

    impl<R: Read> Read for CountRead<R> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = self.inner.read(buf)?;
            self.n += n;
            Ok(n)
        }
    }
}

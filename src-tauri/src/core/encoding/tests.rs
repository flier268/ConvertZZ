use super::*;

#[test]
fn roundtrips_supported_encodings() {
    for encoding in [
        TextEncoding::Utf8,
        TextEncoding::Utf8Bom,
        TextEncoding::Utf16le,
        TextEncoding::Utf16be,
        TextEncoding::Big5,
        TextEncoding::Gbk,
        TextEncoding::ShiftJis,
        TextEncoding::EucJp,
        TextEncoding::Iso2022Jp,
    ] {
        let source = "中文 テスト ABC";
        let encoded = encode_text(source, encoding, encoding == TextEncoding::Utf8Bom).unwrap();
        assert_eq!(
            decode_text(&encoded, encoding).unwrap().0,
            source,
            "{encoding:?}"
        );
    }
    let source = "中文 ABC~";
    let encoded = encode_text(source, TextEncoding::HzGb2312, false).unwrap();
    assert_eq!(
        decode_text(&encoded, TextEncoding::HzGb2312).unwrap().0,
        source
    );
}

#[test]
fn utf16_odd_length_and_lone_surrogate_report_errors() {
    let odd = decode_text_detailed(&[0x41, 0x00, 0x42], TextEncoding::Utf16le).unwrap();
    assert!(odd.had_errors, "{odd:?}");
    let lone_high = decode_text_detailed(&[0x00, 0xD8], TextEncoding::Utf16le).unwrap();
    assert!(lone_high.had_errors, "{lone_high:?}");
    let lone_low = decode_text_detailed(&[0xD8, 0x00], TextEncoding::Utf16be).unwrap();
    assert!(lone_low.had_errors, "{lone_low:?}");

    let text = "𠀀";
    let encoded = encode_text(text, TextEncoding::Utf16le, false).unwrap();
    let decoded = decode_text_detailed(&encoded, TextEncoding::Utf16le).unwrap();
    assert!(!decoded.had_errors);
    assert_eq!(decoded.text, text);

    let even = decode_text_detailed(&[0x41, 0x00], TextEncoding::Utf16le).unwrap();
    assert!(!even.had_errors);
    assert_eq!(even.text, "A");
}

#[test]
fn reinterpret_rejects_unmappable_characters() {
    let emoji = reinterpret_text("😀简体中文.txt", TextEncoding::Big5, TextEncoding::Gbk);
    assert!(emoji.is_err(), "{emoji:?}");
    let emoji_other = reinterpret_text("😀中文.txt", TextEncoding::Gbk, TextEncoding::Big5);
    assert!(emoji_other.is_err(), "{emoji_other:?}");

    let simplified = reinterpret_text("简体", TextEncoding::Big5, TextEncoding::Gbk);
    assert!(simplified.is_err(), "{simplified:?}");

    let outside_gbk = reinterpret_text("𠀀", TextEncoding::Gbk, TextEncoding::Big5);
    assert!(outside_gbk.is_err(), "{outside_gbk:?}");

    // U+3000 的 Shift_JIS 位元組在 Big5 無法解碼。編碼本身沒有錯誤。
    let decode_error = reinterpret_text("\u{3000}", TextEncoding::ShiftJis, TextEncoding::Big5);
    assert!(decode_error.is_err(), "{decode_error:?}");

    let lossy = encode_text("😀简", TextEncoding::Big5, false).unwrap();
    assert_eq!(std::str::from_utf8(&lossy).unwrap(), "&#128512;&#31616;");
    assert!(
        encode_text_detailed("😀简", TextEncoding::Big5, false)
            .unwrap()
            .had_errors
    );
}

fn reinterpret_back(text: &str, stored_as: TextEncoding, read_as: TextEncoding) -> String {
    let bytes = encode_text(text, stored_as, false).unwrap();
    decode_text(&bytes, read_as).unwrap().0
}

#[test]
fn reinterpret_roundtrips_shared_cjk() {
    let big5_view = reinterpret_back("中文ABC.txt", TextEncoding::Gbk, TextEncoding::Big5);
    assert_eq!(
        reinterpret_text(&big5_view, TextEncoding::Big5, TextEncoding::Gbk).unwrap(),
        "中文ABC.txt"
    );
    let gbk_view = reinterpret_back("中文ABC.txt", TextEncoding::Big5, TextEncoding::Gbk);
    assert_eq!(
        reinterpret_text(&gbk_view, TextEncoding::Gbk, TextEncoding::Big5).unwrap(),
        "中文ABC.txt"
    );
}

#[test]
fn detects_bom() {
    assert_eq!(
        detect_encoding(&encode_text("測試", TextEncoding::Utf8Bom, true).unwrap()),
        TextEncoding::Utf8Bom
    );
    assert_eq!(
        detect_encoding(&encode_text("測試", TextEncoding::Utf16le, true).unwrap()),
        TextEncoding::Utf16le
    );
    assert_eq!(
        detect_encoding(&encode_text("測試", TextEncoding::Utf16be, true).unwrap()),
        TextEncoding::Utf16be
    );
}

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

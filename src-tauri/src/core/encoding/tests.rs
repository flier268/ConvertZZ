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

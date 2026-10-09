use super::*;

#[test]
fn encodes_html_decimal() {
    assert_eq!(
        convert(UtilityConvertRequest {
            kind: UtilityKind::HtmlDecimalEncode,
            text: "A<&裡\n".into(),
            source_encoding: None,
            target_encoding: None,
        })
        .unwrap(),
        "A&lt;&amp;&#35041;\n"
    );
}

#[test]
fn decodes_mixed_entities() {
    assert_eq!(
        convert(UtilityConvertRequest {
            kind: UtilityKind::HtmlHexDecode,
            text: "A&#35041; &#x958B; &lt;&amp;&gt;".into(),
            source_encoding: None,
            target_encoding: None,
        })
        .unwrap(),
        "A裡 開 <&>"
    );
}

#[test]
fn unicode_escape_roundtrip() {
    let encoded = convert(UtilityConvertRequest {
        kind: UtilityKind::UnicodeEscapeEncode,
        text: "A裡😀".into(),
        source_encoding: None,
        target_encoding: None,
    })
    .unwrap();
    assert_eq!(encoded, "\\u0041\\u88E1\\uD83D\\uDE00");
    assert_eq!(
        convert(UtilityConvertRequest {
            kind: UtilityKind::UnicodeEscapeDecode,
            text: encoded,
            source_encoding: None,
            target_encoding: None,
        })
        .unwrap(),
        "A裡😀"
    );
}

#[test]
fn fullwidth_and_halfwidth() {
    assert_eq!(
        convert(UtilityConvertRequest {
            kind: UtilityKind::Fullwidth,
            text: "A, .\"“”".into(),
            source_encoding: None,
            target_encoding: None,
        })
        .unwrap(),
        "A，　。、「」"
    );
    assert_eq!(
        convert(UtilityConvertRequest {
            kind: UtilityKind::Halfwidth,
            text: "A，　。、「」".into(),
            source_encoding: None,
            target_encoding: None,
        })
        .unwrap(),
        "A, .\"“”"
    );
}

use super::*;
use crate::core::conversion::ConversionService;
use crate::core::types::Direction;

fn temp_dir() -> PathBuf {
    let path = std::env::temp_dir().join(format!("convertzz-user-dict-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn entry(
    simplified: &str,
    traditional: &str,
    direction: UserDictionaryDirection,
) -> UserDictionaryEntry {
    UserDictionaryEntry {
        simplified: simplified.into(),
        traditional: traditional.into(),
        direction,
    }
}

#[test]
fn save_round_trips_and_leaves_no_stage_file() {
    let dir = temp_dir();
    let path = dir.join(USER_DICTIONARY_FILE);
    let entries = vec![
        entry("软件", "軟體", UserDictionaryDirection::Both),
        entry("内存", "記憶體", UserDictionaryDirection::S2t),
    ];
    save(&path, &entries).unwrap();
    assert_eq!(load(&path).unwrap(), entries);
    let names: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|item| item.unwrap().file_name())
        .collect();
    assert_eq!(names, [USER_DICTIONARY_FILE]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn missing_file_is_empty_dictionary() {
    assert!(load(&temp_dir().join(USER_DICTIONARY_FILE))
        .unwrap()
        .is_empty());
}

#[test]
fn rejects_separator_characters_and_conflicts_without_writing() {
    let dir = temp_dir();
    let path = dir.join(USER_DICTIONARY_FILE);
    for bad in ["软\t件", "软|件", "软\n件", " 软件", ""] {
        let error = save(&path, &[entry(bad, "軟體", UserDictionaryDirection::Both)]).unwrap_err();
        assert_eq!(error.code, "USER_DICTIONARY_INVALID", "{bad:?}");
    }
    let error = save(
        &path,
        &[
            entry("软件", "軟體", UserDictionaryDirection::Both),
            entry("软件", "軟件", UserDictionaryDirection::S2t),
        ],
    )
    .unwrap_err();
    assert_eq!(error.code, "USER_DICTIONARY_CONFLICT");
    assert!(!path.exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn same_source_is_allowed_in_the_other_direction() {
    check_conflicts(&[
        entry("干", "幹", UserDictionaryDirection::S2t),
        entry("干", "乾", UserDictionaryDirection::T2s),
    ])
    .unwrap();
}

fn write_csv(rows: &[&str]) -> PathBuf {
    let path = temp_dir().join("Dictionary.csv");
    let mut text = String::from("\u{feff}");
    for row in rows {
        text.push_str(row);
        text.push_str("\r\n");
    }
    fs::write(&path, text).unwrap();
    path
}

#[test]
fn legacy_import_lists_every_enabled_row_and_skips_disabled_invalid() {
    let path = write_csv(&[
        // 1.x 內建列不另外辨識，與自訂列同樣可匯入；全形空白前綴會去除。
        "True\tBI_TradFix\t　了望\t0\t　瞭望\t0",
        "True\tTWPhrasesIT\t软件\t0\t軟體\t0",
        "True\t自訂\t鲁班七号\t0\t魯班七號\t0",
        "False\t自訂\t停用词\t0\t停用詞\t0",
        "True\t自訂\t坏|词\t0\t壞詞\t0",
        "只有三欄\t自訂\t词",
        "True\t自訂\t鲁班七号\t0\t魯班七號\t0",
    ]);
    let preview = preview_legacy_import(&path).unwrap();
    assert_eq!(preview.total, 7);
    assert_eq!(preview.importable, 3);
    assert_eq!(preview.disabled, 1);
    assert_eq!(preview.invalid, 2);
    assert_eq!(preview.duplicate, 1);
    let importable: Vec<_> = preview
        .rows
        .iter()
        .filter(|row| row.status == LegacyImportStatus::Importable)
        .map(|row| (row.simplified.as_str(), row.traditional.as_str()))
        .collect();
    assert_eq!(
        importable,
        [("了望", "瞭望"), ("软件", "軟體"), ("鲁班七号", "魯班七號")]
    );
    let _ = fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn legacy_import_keeps_first_mapping_and_narrows_later_direction() {
    let path = write_csv(&[
        "True\t自訂\t甲乙\t0\t甲乙丙\t0",
        "True\t自訂\t甲乙\t0\t甲乙丁\t0",
        "True\t自訂\t戊己\t0\t甲乙丙\t0",
    ]);
    let preview = preview_legacy_import(&path).unwrap();
    let directions: Vec<_> = preview.rows.iter().map(|row| row.direction).collect();
    assert_eq!(
        directions,
        [
            Some(UserDictionaryDirection::Both),
            Some(UserDictionaryDirection::T2s),
            Some(UserDictionaryDirection::S2t),
        ]
    );
    let entries: Vec<_> = preview
        .rows
        .iter()
        .map(|row| entry(&row.simplified, &row.traditional, row.direction.unwrap()))
        .collect();
    check_conflicts(&entries).unwrap();
    let _ = fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn legacy_import_rejects_non_utf8() {
    let path = temp_dir().join("Dictionary.csv");
    fs::write(&path, [0xB3, 0x6E, 0xC5, 0xE9]).unwrap();
    assert_eq!(
        preview_legacy_import(&path).unwrap_err().code,
        "LEGACY_DICTIONARY_ENCODING"
    );
    let _ = fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn engine_applies_user_words_per_direction_and_reloads() {
    let service = ConversionService::with_user_dictionary(&[
        entry("软件", "軟體", UserDictionaryDirection::Both),
        entry("鼠标", "滑鼠", UserDictionaryDirection::S2t),
    ])
    .unwrap();
    assert_eq!(
        service.convert_segmented("我的软件和鼠标", Direction::S2t),
        "我的軟體和滑鼠"
    );
    assert_eq!(
        service.convert_segmented("我的軟體和滑鼠", Direction::T2s),
        "我的软件和滑鼠",
        "鼠标只設簡轉繁，繁轉簡不得套用"
    );

    service
        .set_user_dictionary(
            &[entry("鼠标", "滑鼠", UserDictionaryDirection::Both)],
            || Ok(()),
        )
        .unwrap();
    assert_eq!(
        service.convert_segmented("我的軟體和滑鼠", Direction::T2s),
        "我的软体和鼠标"
    );

    let failed = service.set_user_dictionary(&[], || {
        Err(crate::core::error::CoreError::new("TEST", "write failed"))
    });
    assert!(failed.is_err());
    assert_eq!(
        service.convert_segmented("滑鼠", Direction::T2s),
        "鼠标",
        "寫檔失敗時不得換上新分詞器"
    );
}

#[test]
fn removed_user_words_do_not_leak_into_new_engines() {
    let with_word = ConversionService::with_user_dictionary(&[entry(
        "软件",
        "軟體",
        UserDictionaryDirection::Both,
    )])
    .unwrap();
    assert_eq!(with_word.convert_segmented("软件", Direction::S2t), "軟體");
    let without = ConversionService::with_user_dictionary(&[]).unwrap();
    assert_ne!(
        without.convert_segmented("我的軟體", Direction::T2s),
        "我的软件"
    );
}

#[test]
fn user_words_apply_inside_running_text() {
    let service = ConversionService::with_user_dictionary(&[
        entry("优盘", "隨身碟", UserDictionaryDirection::S2t),
        entry("U盘", "隨身碟", UserDictionaryDirection::T2s),
        entry("信息", "資訊", UserDictionaryDirection::Both),
        entry("鲁班七号", "魯班七號機甲", UserDictionaryDirection::S2t),
    ])
    .unwrap();
    assert_eq!(
        service.convert_segmented("插上优盘后查看信息，再玩鲁班七号。", Direction::S2t),
        "插上隨身碟後查看資訊，再玩魯班七號機甲。"
    );
    assert_eq!(
        service.convert_segmented("插上隨身碟後查看資訊。", Direction::T2s),
        "插上U盘后查看信息。"
    );
}

#[test]
fn user_words_apply_across_long_text_chunks_and_keep_spacing() {
    let service = ConversionService::with_user_dictionary(&[
        entry("优盘", "隨身碟", UserDictionaryDirection::S2t),
        entry("信息", "資訊", UserDictionaryDirection::Both),
    ])
    .unwrap();
    let line = "　插上优盘后 ，查看信息！\n\t";
    let expected = "　插上隨身碟後 ，查看資訊！\n\t";
    let source = line.repeat(3_000);
    assert!(source.chars().count() > 30_000);
    assert_eq!(
        service.convert_segmented(&source, Direction::S2t),
        expected.repeat(3_000)
    );
}

#[test]
fn non_cjk_source_words_are_rejected_or_narrowed() {
    assert!(entry_problem(&entry("U盘", "隨身碟", UserDictionaryDirection::Both)).is_some());
    assert!(entry_problem(&entry("U盘", "隨身碟", UserDictionaryDirection::T2s)).is_none());
    let path = write_csv(&[
        "True\t自訂\tU盘\t0\t隨身碟\t0",
        "True\t自訂\tWin10\t0\tWin10\t0",
    ]);
    let preview = preview_legacy_import(&path).unwrap();
    assert_eq!(preview.rows[0].status, LegacyImportStatus::Importable);
    assert_eq!(
        preview.rows[0].direction,
        Some(UserDictionaryDirection::T2s)
    );
    assert_eq!(preview.rows[1].status, LegacyImportStatus::Invalid);
    let _ = fs::remove_dir_all(path.parent().unwrap());
}

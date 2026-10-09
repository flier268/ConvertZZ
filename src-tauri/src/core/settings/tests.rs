use super::*;
use uuid::Uuid;

#[test]
fn migrates_legacy_settings_to_v2() {
    let result = migrate(json!({
        "Engine": 1,
        "RecognitionEncoding": false,
        "Prompt": false,
        "MaxLengthPreview": 12,
        "AssistiveTouch": false,
        "PositionX": 100,
        "PositionY": 200,
        "HotKey": {
            "AutoCopy": false,
            "AutoPaste": true,
            "Feature1": { "Enable": true, "Modift": "Control, Shift", "Key": "F8", "Action": "a1" }
        },
        "QuickStart": { "LeftClick_Ctrl": "a3", "RightDrop_Shift": "ze2" },
        "FileConvert": {
            "DefaultPath": "D:\\Text",
            "TypeFilter": "<文字|*.txt>",
            "FixLabel": ".html|.php",
            "UnicodeAddBOM": true
        },
        "Fanhuaji_Setting": {
            "Converter_S_to_T": 4,
            "Converter_T_to_S": "Simplified",
            "JpTextConversionStrategy": 0,
            "JpStyleConversionStrategy": 1,
            "IgnoreTextStyles": "code",
            "JpTextStyles": "jp",
            "CleanUpText": true,
            "UserPreReplace": [{ "Key": "甲", "Value": "乙" }],
            "Modules": [{ "ModuleName": "TaiwanPhrase", "Enable": true }]
        }
    }));
    assert_eq!(result["version"], 2);
    assert_eq!(result["engine"], "zhconvert");
    assert_eq!(result["autoBackupBeforeConversion"], true);
    assert_eq!(result["showMainWindowOnStart"], false);
    assert_eq!(result["recognizeEncoding"], false);
    assert_eq!(result["floatingBall"]["enabled"], false);
    assert_eq!(result["floatingBall"]["x"], 100);
    assert_eq!(result["floatingBall"]["y"], 200);
    assert_eq!(result["hotkeys"]["shortcuts"][0]["enabled"], true);
    assert_eq!(
        result["hotkeys"]["shortcuts"][0]["accelerator"],
        "Control+Shift+F8"
    );
    assert_eq!(result["hotkeys"]["shortcuts"][0]["action"], "a1");
    assert_eq!(result["quickActions"]["leftClickCtrl"], "a3");
    assert_eq!(result["quickActions"]["rightDropShift"], "ze2");
    assert_eq!(result["files"]["unicodeAddBom"], true);
    assert_eq!(result["files"]["defaultPath"], "D:\\Text");
    assert_eq!(result["zhconvert"]["converterS2T"], "Taiwan");
    assert_eq!(result["zhconvert"]["converterT2S"], "Simplified");
    assert_eq!(
        result["zhconvert"]["jpTextConversionStrategy"],
        "protectOnlySameOrigin"
    );
    assert_eq!(result["zhconvert"]["jpStyleConversionStrategy"], "none");
    assert_eq!(result["zhconvert"]["ignoreTextStyles"], "code");
    assert_eq!(result["zhconvert"]["jpTextStyles"], "jp");
    assert_eq!(result["zhconvert"]["cleanUpText"], true);
    assert_eq!(result["zhconvert"]["userPreReplace"], "甲=乙");
    assert_eq!(result["zhconvert"]["modules"]["TaiwanPhrase"], 1);
}

#[test]
fn import_from_path_does_not_modify_source() {
    let directory = std::env::temp_dir().join(format!("convertzz-settings-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("ConvertZZ.json");
    let original = r#"{"Prompt":false}"#;
    std::fs::write(&source, original).unwrap();
    let result = migrate_from_path(source.to_str().unwrap()).unwrap();
    assert_eq!(result["promptAfterConversion"], false);
    assert_eq!(std::fs::read_to_string(&source).unwrap(), original);
    let names: Vec<_> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["ConvertZZ.json"]);
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn failed_import_does_not_write() {
    let directory =
        std::env::temp_dir().join(format!("convertzz-settings-missing-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("ConvertZZ.json");
    assert!(migrate_from_path(source.to_str().unwrap()).is_err());
    assert!(std::fs::read_dir(&directory).unwrap().next().is_none());
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn maps_legacy_engine_names() {
    assert_eq!(migrate(json!({ "Engine": 0 }))["engine"], "segmented");
    assert_eq!(migrate(json!({ "Engine": "Local" }))["engine"], "segmented");
    assert_eq!(
        migrate(json!({ "Engine": "Fanhuaji" }))["engine"],
        "zhconvert"
    );
}

#[test]
fn missing_v2_fields_default_main_window() {
    assert_eq!(migrate(Value::Null)["showMainWindowOnStart"], false);
    assert_eq!(
        migrate(json!({ "version": 2, "engine": "legacy" }))["showMainWindowOnStart"],
        false
    );
    assert_eq!(
        migrate(json!({ "version": 2, "showMainWindowOnStart": true }))["showMainWindowOnStart"],
        true
    );
}

#[test]
fn missing_v2_fields_default_skipped_update() {
    assert_eq!(migrate(Value::Null)["skippedUpdateVersion"], "");
    assert_eq!(
        migrate(json!({ "version": 2, "engine": "legacy" }))["skippedUpdateVersion"],
        ""
    );
    assert_eq!(
        migrate(json!({ "version": 2, "skippedUpdateVersion": "2.1.0" }))["skippedUpdateVersion"],
        "2.1.0"
    );
}

#[test]
fn missing_v2_fields_default_pre_release_updates_follow_current_version() {
    let expected = json!(is_pre_release_version(env!("CARGO_PKG_VERSION")));
    assert_eq!(default_settings()["checkPreReleaseUpdates"], expected);
    assert_eq!(migrate(Value::Null)["checkPreReleaseUpdates"], expected);
    assert_eq!(
        migrate(json!({ "version": 2, "engine": "legacy" }))["checkPreReleaseUpdates"],
        expected
    );
    assert_eq!(
        migrate(json!({ "version": 2, "checkPreReleaseUpdates": true }))["checkPreReleaseUpdates"],
        true
    );
    assert_eq!(
        migrate(json!({ "version": 2, "checkPreReleaseUpdates": false }))["checkPreReleaseUpdates"],
        false
    );
    assert_eq!(
        migrate(json!({ "version": 2, "checkPreReleaseUpdates": null }))["checkPreReleaseUpdates"],
        expected
    );
}

#[test]
fn pre_release_version_detection() {
    assert!(!is_pre_release_version("2.0.0"));
    assert!(!is_pre_release_version("v2.0.0"));
    assert!(!is_pre_release_version("2.0.0+build.1"));
    assert!(is_pre_release_version("2.0.0-beta5"));
    assert!(is_pre_release_version("v2.0.0-rc.1"));
    assert!(is_pre_release_version("2.0.0-alpha.1+build.1"));
}

#[test]
fn missing_backup_flag_defaults_true() {
    assert_eq!(migrate(Value::Null)["autoBackupBeforeConversion"], true);
    assert_eq!(
        migrate(json!({ "version": 2, "engine": "legacy" }))["autoBackupBeforeConversion"],
        true
    );
    assert_eq!(
        migrate(json!({ "version": 2, "autoBackupBeforeConversion": false }))
            ["autoBackupBeforeConversion"],
        false
    );
    assert_eq!(
        migrate(json!({ "Prompt": false }))["autoBackupBeforeConversion"],
        true
    );
}

#[test]
fn missing_last_drop_action_defaults() {
    assert_eq!(
        migrate(Value::Null)["lastDropAction"],
        json!({
            "kind": "file",
            "operation": "content",
            "direction": "s2t"
        })
    );
    assert_eq!(
        migrate(json!({
            "version": 2,
            "lastDropAction": { "kind": "audio", "direction": "t2s" }
        }))["lastDropAction"],
        json!({
            "kind": "audio",
            "operation": "content",
            "direction": "t2s"
        })
    );
}

#[test]
fn preserves_raw_saved_paths() {
    let result = migrate(json!({
        "version": 2,
        "dictionaryPath": r"\\?\C:\Program Files\ConvertZZ\Dictionary.csv",
        "files": { "defaultPath": r"\\?\D:\Text", "typeFilter": "", "fixCharsetExtensions": [] }
    }));
    assert_eq!(
        result["dictionaryPath"],
        r"\\?\C:\Program Files\ConvertZZ\Dictionary.csv"
    );
    assert_eq!(result["files"]["defaultPath"], r"\\?\D:\Text");
    assert_eq!(result["files"]["typeFilter"], "");
}

#[derive(serde::Deserialize)]
struct TypeFilterMigrationFixture {
    #[serde(rename = "defaultTypeFilter")]
    default_type_filter: String,
    cases: Vec<TypeFilterMigrationCase>,
}

#[derive(serde::Deserialize)]
struct TypeFilterMigrationCase {
    name: String,
    source: String,
    input: String,
    expected: String,
}

fn type_filter_migration_fixture() -> TypeFilterMigrationFixture {
    serde_json::from_str(include_str!(
        "../../../../tests/fixtures/type-filter-migration.json"
    ))
    .expect("type-filter-migration.json")
}

fn expected_type_filter<'a>(case: &'a TypeFilterMigrationCase, default_filter: &'a str) -> &'a str {
    match case.expected.as_str() {
        "default" => default_filter,
        "keep" => case.input.as_str(),
        other => panic!("{} 的期望不是 default 或 keep：{other}", case.name),
    }
}

#[test]
fn type_filter_migration_vectors_match_shared_fixture() {
    // 舊格式字串來源寫在 tests/fixtures/type-filter-migration.json 的 source：
    // e7c3aeb ConvertZZ/Settings.cs:112、03204e6 ConvertZZ/Settings.cs:109。
    let fixture = type_filter_migration_fixture();
    assert_eq!(super::DEFAULT_TYPE_FILTER, fixture.default_type_filter);
    assert!(fixture
        .cases
        .iter()
        .any(|case| case.source.contains("e7c3aeb") && case.source.contains("Settings.cs:112")));
    assert!(fixture
        .cases
        .iter()
        .any(|case| case.source.contains("03204e6") && case.source.contains("Settings.cs:109")));
    let legacy = fixture
        .cases
        .iter()
        .find(|case| case.name == "2.0 音訊版預設")
        .expect("2.0 音訊版預設");
    let csharp = fixture
        .cases
        .iter()
        .find(|case| case.name == "C# 音頻版預設")
        .expect("C# 音頻版預設");
    let current = fixture
        .cases
        .iter()
        .find(|case| case.name == "新預設")
        .expect("新預設");
    assert_eq!(legacy.input, super::LEGACY_TYPE_FILTER_WITH_AUDIO);
    assert_eq!(csharp.input, super::CSHARP_TYPE_FILTER_WITH_AUDIO);
    assert_eq!(current.input, fixture.default_type_filter);
    assert!(!fixture.default_type_filter.contains("mp3"));

    for case in &fixture.cases {
        assert!(!case.source.is_empty(), "{}", case.name);
        let expected = expected_type_filter(case, &fixture.default_type_filter);
        assert_eq!(
            migrate(json!({ "version": 2, "files": { "typeFilter": case.input } }))["files"]
                ["typeFilter"],
            expected,
            "v2 {}",
            case.name
        );
        assert_eq!(
            migrate(json!({ "FileConvert": { "TypeFilter": case.input } }))["files"]["typeFilter"],
            expected,
            "legacy {}",
            case.name
        );
    }

    // 空字串維持原行為：v2 保留空字串，舊版缺值才走 fallback。
    assert_eq!(
        migrate(json!({ "version": 2, "files": { "typeFilter": "" } }))["files"]["typeFilter"],
        ""
    );
    assert_eq!(
        migrate(json!({ "FileConvert": { "TypeFilter": "" } }))["files"]["typeFilter"],
        fixture.default_type_filter
    );
}

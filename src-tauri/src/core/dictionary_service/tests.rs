use super::super::conversion::shared_conversion;
use super::super::types::{ConversionRequest, DictionaryUpdate, Direction, EngineKind};
use super::*;
use uuid::Uuid;

fn temp_dir() -> PathBuf {
    let path = std::env::temp_dir().join(format!("convertzz-dict-svc-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn entry(
    simplified: &str,
    simplified_priority: i64,
    traditional: &str,
    traditional_priority: i64,
) -> DictionaryEntry {
    DictionaryEntry {
        enabled: true,
        entry_type: "一般".into(),
        simplified: simplified.into(),
        simplified_priority,
        traditional: traditional.into(),
        traditional_priority,
    }
}

#[tokio::test]
async fn successive_saves_create_distinct_backups() {
    let directory = temp_dir();
    let path = directory.join("Dictionary.csv");
    let original = "\u{feff}true\t一般\t里面\t1\t裡面\t1\n";
    std::fs::write(&path, original).unwrap();
    let service = DictionaryService::new(Some(path.clone()));
    let conversion = shared_conversion();
    let first_convert = conversion
        .convert(ConversionRequest {
            text: "里面".into(),
            direction: Direction::S2t,
            engine: EngineKind::Legacy,
            dictionary_path: Some(path.to_string_lossy().into_owned()),
            zhconvert: None,
            vocabulary_correction: None,
        })
        .await
        .unwrap();
    assert_eq!(first_convert.text, "裡面");
    let first = service
        .update(DictionaryUpdateRequest {
            path: path.to_string_lossy().into_owned(),
            updates: Some(vec![DictionaryUpdate {
                index: 0,
                entry: entry("里面", 2, "內部", 2),
            }]),
            inserts: None,
            deletes: None,
        })
        .unwrap();
    let after_first = std::fs::read_to_string(&path).unwrap();
    let second = service
        .update(DictionaryUpdateRequest {
            path: path.to_string_lossy().into_owned(),
            updates: Some(vec![DictionaryUpdate {
                index: 0,
                entry: entry("里面", 3, "裏邊", 3),
            }]),
            inserts: None,
            deletes: None,
        })
        .unwrap();
    assert_ne!(first["backupPath"], second["backupPath"]);
    assert_eq!(
        std::fs::read_to_string(first["backupPath"].as_str().unwrap()).unwrap(),
        original
    );
    assert_eq!(
        std::fs::read_to_string(second["backupPath"].as_str().unwrap()).unwrap(),
        after_first
    );
    let backups = std::fs::read_dir(&directory)
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .ok()
                .and_then(|item| {
                    item.file_name()
                        .to_str()
                        .map(|name| name.contains(".backup-"))
                })
                .unwrap_or(false)
        })
        .count();
    assert_eq!(backups, 2);
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .contains("\t3\t裏邊\t3"));
    let after = conversion
        .convert(ConversionRequest {
            text: "里面".into(),
            direction: Direction::S2t,
            engine: EngineKind::Legacy,
            dictionary_path: Some(path.to_string_lossy().into_owned()),
            zhconvert: None,
            vocabulary_correction: None,
        })
        .await
        .unwrap();
    assert_eq!(after.text, "裏邊");
    let leftovers = std::fs::read_dir(&directory)
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .ok()
                .and_then(|item| {
                    item.file_name()
                        .to_str()
                        .map(|name| name.starts_with(".convertzz-"))
                })
                .unwrap_or(false)
        })
        .count();
    assert_eq!(leftovers, 0);
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn supports_insert_delete_sort_and_unsaved_preview() {
    let directory = temp_dir();
    let path = directory.join("Dictionary.csv");
    std::fs::write(
        &path,
        "\u{feff}true\t一般\t专案\t10\t舊專案\t10\ntrue\t一般\t开发\t20\t舊開發\t20",
    )
    .unwrap();
    let service = DictionaryService::new(Some(path.clone()));
    let inserted = entry("开发者", 100, "新開發者", 100);
    let sorted = service
        .read(DictionaryReadRequest {
            path: None,
            query: None,
            offset: None,
            limit: None,
            sort: Some("s2t".into()),
        })
        .unwrap();
    let names: Vec<_> = sorted["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["simplified"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, ["开发", "专案"]);
    let preview = service
        .preview(DictionaryPreviewRequest {
            path: None,
            text: "专案开发者".into(),
            direction: Direction::S2t,
            updates: None,
            inserts: Some(vec![inserted.clone()]),
            deletes: Some(vec![0]),
        })
        .unwrap();
    assert_eq!(preview["text"], "專案新開發者");
    let updated = service
        .update(DictionaryUpdateRequest {
            path: path.to_string_lossy().into_owned(),
            updates: None,
            inserts: Some(vec![inserted]),
            deletes: Some(vec![0]),
        })
        .unwrap();
    assert_eq!(updated["updated"], 2);
    let saved = service
        .read(DictionaryReadRequest {
            path: None,
            query: None,
            offset: None,
            limit: None,
            sort: Some("s2t".into()),
        })
        .unwrap();
    let saved_names: Vec<_> = saved["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["simplified"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(saved_names, ["开发者", "开发"]);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("舊專案"));
    assert!(text.contains("新開發者"));
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn updates_items_after_blank_lines_by_original_index() {
    let directory = temp_dir();
    let path = directory.join("Dictionary.csv");
    std::fs::write(
        &path,
        "\u{feff}true\t一般\t一\t1\t壹\t1\n\ntrue\t一般\t二\t2\t貳\t2\n",
    )
    .unwrap();
    let service = DictionaryService::new(Some(path.clone()));
    let before = service
        .read(DictionaryReadRequest {
            path: None,
            query: None,
            offset: None,
            limit: None,
            sort: None,
        })
        .unwrap();
    let indexes: Vec<_> = before["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["index"].as_u64().unwrap())
        .collect();
    assert_eq!(indexes, [0, 2]);
    service
        .update(DictionaryUpdateRequest {
            path: path.to_string_lossy().into_owned(),
            updates: Some(vec![DictionaryUpdate {
                index: 2,
                entry: entry("二", 3, "兩", 3),
            }]),
            inserts: None,
            deletes: None,
        })
        .unwrap();
    let lines: Vec<_> = std::fs::read_to_string(&path)
        .unwrap()
        .trim_start_matches('\u{feff}')
        .split('\n')
        .map(ToOwned::to_owned)
        .collect();
    assert_eq!(lines[1], "");
    assert!(lines[2].contains("\t二\t3\t兩\t3"));
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn read_keeps_original_path() {
    let directory = temp_dir();
    let path = directory.join("Dictionary.csv");
    std::fs::write(&path, "\u{feff}true\t一般\t一\t1\t壹\t1\n").unwrap();
    let result = DictionaryService::new(Some(path.clone()))
        .read(DictionaryReadRequest {
            path: None,
            query: None,
            offset: None,
            limit: None,
            sort: None,
        })
        .unwrap();
    assert_eq!(result["path"], path.to_string_lossy().as_ref());
    let _ = std::fs::remove_dir_all(&directory);
}

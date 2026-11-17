use super::super::types::ConflictPolicy;
use super::*;
use uuid::Uuid;

fn temp_dir() -> PathBuf {
    let path = std::env::temp_dir().join(format!("convertzz-backup-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn file_backup_path_appends_bak() {
    assert_eq!(
        user_backup_path(Path::new("/tmp/note.txt")).unwrap(),
        PathBuf::from("/tmp/note.txt.bak")
    );
    assert_eq!(
        user_backup_path(Path::new("/tmp/docs")).unwrap(),
        PathBuf::from("/tmp/docs.bak")
    );
}

#[test]
fn backup_path_ignores_trailing_separator_and_rejects_parent_reference() {
    assert_eq!(
        user_backup_path(Path::new("/tmp/docs/")).unwrap(),
        PathBuf::from("/tmp/docs.bak")
    );
    assert_eq!(
        user_backup_path(Path::new("/tmp/docs/.")).unwrap(),
        PathBuf::from("/tmp/docs.bak")
    );
    assert!(user_backup_path(Path::new("/tmp/docs/..")).is_err());
    assert!(user_backup_path(Path::new("/")).is_err());
}

#[test]
fn directory_with_trailing_separator_is_backed_up_beside_it() {
    let root = temp_dir();
    let docs = root.join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("a.txt"), "a").unwrap();
    let source = PathBuf::from(format!("{}{}", docs.display(), std::path::MAIN_SEPARATOR));
    let backup = create_user_backup(&source, ConflictPolicy::Skip).unwrap();
    assert_eq!(backup, root.join("docs.bak"));
    assert_eq!(std::fs::read_to_string(backup.join("a.txt")).unwrap(), "a");
    assert!(!docs.join(".bak").exists(), "備份不得放進來源目錄");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn prunes_nested_directory_roots() {
    assert_eq!(
        prune_nested_backup_roots(vec![
            BackupRoot {
                path: PathBuf::from("/data/docs"),
                kind: BackupKind::Directory
            },
            BackupRoot {
                path: PathBuf::from("/data/docs/nested"),
                kind: BackupKind::Directory
            },
            BackupRoot {
                path: PathBuf::from("/data/docs/a.txt"),
                kind: BackupKind::File
            },
            BackupRoot {
                path: PathBuf::from("/data/other.txt"),
                kind: BackupKind::File
            },
        ]),
        vec![
            BackupRoot {
                path: PathBuf::from("/data/docs"),
                kind: BackupKind::Directory
            },
            BackupRoot {
                path: PathBuf::from("/data/other.txt"),
                kind: BackupKind::File
            },
        ]
    );
}

#[test]
fn classifies_folder_and_file_roots() {
    let directory = temp_dir();
    let folder = directory.join("folder");
    let file = directory.join("alone.txt");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(&file, "x").unwrap();
    std::fs::write(folder.join("inside.txt"), "y").unwrap();
    let roots = resolve_backup_roots(&[
        folder.to_string_lossy().into_owned(),
        file.to_string_lossy().into_owned(),
    ])
    .unwrap();
    assert_eq!(
        roots,
        vec![
            BackupRoot {
                path: folder,
                kind: BackupKind::Directory
            },
            BackupRoot {
                path: file,
                kind: BackupKind::File
            },
        ]
    );
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn copies_file_and_directory_backups() {
    let directory = temp_dir();
    let file = directory.join("a.txt");
    let folder = directory.join("box");
    std::fs::write(&file, "file-content").unwrap();
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("b.txt"), "folder-content").unwrap();
    assert_eq!(
        create_user_backup(&file, ConflictPolicy::Overwrite).unwrap(),
        PathBuf::from(format!("{}.bak", file.display()))
    );
    assert_eq!(
        std::fs::read_to_string(format!("{}.bak", file.display())).unwrap(),
        "file-content"
    );
    assert_eq!(
        create_user_backup(&folder, ConflictPolicy::Overwrite).unwrap(),
        PathBuf::from(format!("{}.bak", folder.display()))
    );
    assert_eq!(
        std::fs::read_to_string(PathBuf::from(format!("{}.bak", folder.display())).join("b.txt"))
            .unwrap(),
        "folder-content"
    );
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn create_user_backups_only_covers_affected_roots() {
    let directory = temp_dir();
    let kept = directory.join("kept.txt");
    let skipped = directory.join("skipped.txt");
    std::fs::write(&kept, "k").unwrap();
    std::fs::write(&skipped, "s").unwrap();
    let created = create_user_backups(
        &[
            BackupRoot {
                path: kept.clone(),
                kind: BackupKind::File,
            },
            BackupRoot {
                path: skipped.clone(),
                kind: BackupKind::File,
            },
        ],
        &[kept.clone()],
        ConflictPolicy::Overwrite,
    )
    .unwrap();
    assert_eq!(created, [PathBuf::from(format!("{}.bak", kept.display()))]);
    let names: Vec<_> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.contains(&"kept.txt".into()));
    assert!(names.contains(&"kept.txt.bak".into()));
    assert!(!names.contains(&"skipped.txt.bak".into()));
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn skip_keeps_existing_bak_overwrite_replaces() {
    let directory = temp_dir();
    let file = directory.join("song.mp3");
    std::fs::write(&file, "new-content").unwrap();
    let bak = PathBuf::from(format!("{}.bak", file.display()));
    std::fs::write(&bak, "old-backup").unwrap();
    assert_eq!(
        create_user_backup(&file, ConflictPolicy::Skip).unwrap(),
        bak
    );
    assert_eq!(std::fs::read_to_string(&bak).unwrap(), "old-backup");
    assert_eq!(
        create_user_backup(&file, ConflictPolicy::Overwrite).unwrap(),
        bak
    );
    assert_eq!(std::fs::read_to_string(&bak).unwrap(), "new-content");
    let _ = std::fs::remove_dir_all(&directory);
}

#[cfg(unix)]
#[test]
fn failed_overwrite_copy_keeps_existing_backup_and_leaves_no_temporary() {
    use std::os::unix::fs::PermissionsExt;
    let directory = temp_dir();
    let source = directory.join("docs");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("a.txt"), "new").unwrap();
    let locked = source.join("b.txt");
    std::fs::write(&locked, "secret").unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    let bak = directory.join("docs.bak");
    std::fs::create_dir_all(&bak).unwrap();
    std::fs::write(bak.join("a.txt"), "old").unwrap();
    if std::fs::read(&locked).is_ok() {
        // root 可讀取無權限檔案，無法觸發複製失敗。
        let _ = std::fs::remove_dir_all(&directory);
        return;
    }
    assert!(create_user_backup(&source, ConflictPolicy::Overwrite).is_err());
    assert_eq!(std::fs::read_to_string(bak.join("a.txt")).unwrap(), "old");
    let mut names = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, ["docs", "docs.bak"]);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o644)).unwrap();
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn overwrite_replaces_directory_backup_without_leftovers() {
    let directory = temp_dir();
    let source = directory.join("docs");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("a.txt"), "new").unwrap();
    let bak = directory.join("docs.bak");
    std::fs::create_dir_all(&bak).unwrap();
    std::fs::write(bak.join("stale.txt"), "old").unwrap();
    assert_eq!(
        create_user_backup(&source, ConflictPolicy::Overwrite).unwrap(),
        bak
    );
    assert_eq!(std::fs::read_to_string(bak.join("a.txt")).unwrap(), "new");
    assert!(!bak.join("stale.txt").exists());
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 2);
    let _ = std::fs::remove_dir_all(&directory);
}

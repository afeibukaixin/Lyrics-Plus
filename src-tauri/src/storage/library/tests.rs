#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::tests::test_dirs;

    fn scan(storage: &Storage) -> LibraryScanStatus {
        let status = storage.begin_library_scan();
        assert!(storage.run_library_scan(status.scan_id, |_| {}).unwrap());
        storage.library_scan_status()
    }

    #[test]
    fn selected_directory_persists_and_invalid_path_is_rejected() {
        let (app_dir, default_dir) = test_dirs("persist-library");
        let new_dir = app_dir.parent().unwrap().join("Selected Library");
        fs::create_dir_all(&new_dir).unwrap();
        let storage = Storage::open(app_dir.clone(), default_dir.clone()).unwrap();
        storage
            .set_library_directory(&new_dir.to_string_lossy())
            .unwrap();
        assert!(storage
            .set_library_directory(&app_dir.join("missing").to_string_lossy())
            .is_err());
        drop(storage);

        let reopened = Storage::open(app_dir.clone(), default_dir).unwrap();
        assert_eq!(
            reopened.library_directory(),
            new_dir.canonicalize().unwrap()
        );
        let _ = fs::remove_dir_all(app_dir.parent().unwrap());
    }

    #[test]
    fn newer_scan_cancels_previous_generation() {
        let (app_dir, library_dir) = test_dirs("scan-cancellation");
        fs::create_dir_all(&library_dir).unwrap();
        let storage = Storage::open(app_dir.clone(), library_dir).unwrap();
        let old = storage.begin_library_scan();
        let current = storage.begin_library_scan();
        assert!(!storage.run_library_scan(old.scan_id, |_| {}).unwrap());
        assert!(storage.run_library_scan(current.scan_id, |_| {}).unwrap());
        assert_eq!(storage.library_scan_status().scan_id, current.scan_id);
        let _ = fs::remove_dir_all(app_dir.parent().unwrap());
    }

    #[test]
    fn scan_commits_and_reports_in_two_hundred_file_batches() {
        let (app_dir, library_dir) = test_dirs("scan-batches");
        fs::create_dir_all(&library_dir).unwrap();
        for index in 0..201 {
            fs::write(
                library_dir.join(format!("Artist - Song {index}.lrc")),
                format!("[00:01]Line {index}"),
            )
            .unwrap();
        }
        let storage = Storage::open(app_dir.clone(), library_dir).unwrap();
        let status = storage.begin_library_scan();
        let mut processed = Vec::new();
        assert!(storage
            .run_library_scan(status.scan_id, |status| {
                if status.phase == LibraryScanPhase::Indexing && status.processed > 0 {
                    processed.push(status.processed);
                }
            })
            .unwrap());
        assert_eq!(processed, vec![200, 201]);
        let _ = fs::remove_dir_all(app_dir.parent().unwrap());
    }

    #[test]
    fn discovered_file_limit_rejects_the_next_file() {
        let mut files = vec![PathBuf::new(); MAX_LYRIC_FILES];
        assert!(push_discovered_file(&mut files, PathBuf::new()).is_err());
        assert_eq!(files.len(), MAX_LYRIC_FILES);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> NowPlayingInfo {
        NowPlayingInfo {
            is_playing: Some(true),
            title: Some(" Test Song ".into()),
            artist: Some("Some Artist".into()),
            album: Some("Album".into()),
            album_cover: None,
            elapsed_time: Some(12.345),
            duration: Some(123.456),
            info_update_time: Some(SystemTime::now()),
            bundle_id: Some("com.example.Player".into()),
            bundle_name: Some("Example Player".into()),
            bundle_icon: None,
        }
    }

    #[test]
    fn converts_system_media_info_to_snapshot() {
        let snapshot = snapshot_from_info(&TimedInfo {
            info: info(),
            received_at: Instant::now(),
        });
        assert_eq!(snapshot.player, Some(PlayerKind::System));
        assert_eq!(snapshot.position_ms, Some(12_345));
        assert_eq!(snapshot.duration_ms, Some(123_456));
        assert_eq!(snapshot.source_app_name.as_deref(), Some("Example Player"));
    }

    #[test]
    fn initial_adapter_snapshot_uses_calculated_current_position() {
        let latest = RwLock::new(Some(TimedInfo {
            info: info(),
            received_at: Instant::now(),
        }));
        assert!(sync_elapsed_from_adapter(
            &latest,
            br#"{"title":" Test Song ","bundleIdentifier":"com.example.Player","elapsedTimeNow":56.86}"#,
        ));
        let timed = latest.read().unwrap();
        assert_eq!(timed.as_ref().unwrap().info.elapsed_time, Some(56.86));
    }

    #[test]
    fn anchors_existing_playback_to_the_media_timestamp() {
        let mut current = info();
        current.info_update_time = Some(SystemTime::now() - Duration::from_secs(30));
        let snapshot = snapshot_from_info(&timed_info(current).unwrap());
        assert!(snapshot
            .position_ms
            .is_some_and(|position| (42_345..43_345).contains(&position)));
    }

    #[test]
    fn system_track_id_includes_source_application() {
        let first = system_track_id(&info()).unwrap();
        let mut other = info();
        other.bundle_id = Some("com.example.Other".into());
        assert_ne!(first, system_track_id(&other).unwrap());
    }

    #[test]
    fn rejects_invalid_times() {
        assert_eq!(milliseconds(Some(f64::NAN)), None);
        assert_eq!(milliseconds(Some(-1.0)), None);
    }

    #[test]
    fn advances_playing_position_from_monotonic_receive_time() {
        let snapshot = snapshot_from_info(&TimedInfo {
            info: info(),
            received_at: Instant::now() - Duration::from_millis(500),
        });
        assert!(snapshot
            .position_ms
            .is_some_and(|value| (12_845..=12_855).contains(&value)));
    }

    #[test]
    fn rejects_dependency_timestamp_overflow() {
        let mut invalid = info();
        invalid.elapsed_time = Some(978_307_212.0);
        assert!(!valid_elapsed_time(&invalid));
    }
}

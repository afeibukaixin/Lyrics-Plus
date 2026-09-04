#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_stable_fallback_track_id() {
        let mut snapshot = PlaybackSnapshot {
            title: Some("  Test   Song ".into()),
            artist: Some("Some ARTIST".into()),
            duration_ms: Some(123_000),
            ..PlaybackSnapshot::default()
        };
        ensure_track_id(&mut snapshot);
        assert_eq!(
            snapshot.track_id.as_deref(),
            Some("fallback:test song|some artist|123000")
        );
    }

    #[test]
    fn preserves_player_track_id() {
        let mut snapshot = PlaybackSnapshot {
            track_id: Some("native-id".into()),
            title: Some("Test".into()),
            artist: Some("Artist".into()),
            ..PlaybackSnapshot::default()
        };
        ensure_track_id(&mut snapshot);
        assert_eq!(snapshot.track_id.as_deref(), Some("native-id"));
    }

    #[test]
    fn matches_only_the_current_player_and_track() {
        let snapshot = PlaybackSnapshot {
            player: Some(PlayerKind::Spotify),
            track_id: Some("current-track".into()),
            ..PlaybackSnapshot::default()
        };
        assert!(snapshot.matches_track(PlayerKind::Spotify, "current-track"));
        assert!(!snapshot.matches_track(PlayerKind::Spotify, "other-track"));
        assert!(!snapshot.matches_track(PlayerKind::AppleMusic, "current-track"));
    }

    #[test]
    fn empty_snapshot_exposes_a_stable_waiting_error_code() {
        let snapshot = PlaybackSnapshot::empty();
        assert_eq!(snapshot.error_code, Some(PlaybackErrorCode::Waiting));
        assert!(snapshot.error.is_some());
    }

    #[test]
    fn restores_system_player_selection() {
        assert_eq!(
            PlayerSelection::from_stored(Some("system".into())),
            PlayerSelection::System
        );
        assert_eq!(
            PlayerSelection::System.preferred_kind(),
            Some(PlayerKind::System)
        );
    }

    #[test]
    fn system_source_filter_modes_handle_lists_and_unknown_sources() {
        let mut snapshot = PlaybackSnapshot {
            is_running: true,
            source_app_bundle_id: Some("org.example.Player".into()),
            ..PlaybackSnapshot::default()
        };
        let listed = [RegisteredApplication {
            name: "Player".into(),
            bundle_id: "org.example.Player".into(),
        }];
        assert!(!system_source_allowed(
            &snapshot,
            SystemMediaFilterMode::Allowlist,
            &[],
        ));
        assert!(system_source_allowed(
            &snapshot,
            SystemMediaFilterMode::Allowlist,
            &listed,
        ));
        assert!(system_source_allowed(
            &snapshot,
            SystemMediaFilterMode::Blocklist,
            &[],
        ));
        assert!(!system_source_allowed(
            &snapshot,
            SystemMediaFilterMode::Blocklist,
            &listed,
        ));
        snapshot.source_app_bundle_id = None;
        assert!(!system_source_allowed(
            &snapshot,
            SystemMediaFilterMode::Allowlist,
            &[],
        ));
        assert!(system_source_allowed(
            &snapshot,
            SystemMediaFilterMode::Blocklist,
            &[],
        ));
        snapshot.source_app_bundle_id = Some("com.apple.Music".into());
        assert!(system_source_allowed(
            &snapshot,
            SystemMediaFilterMode::Allowlist,
            &[],
        ));
    }

    #[test]
    fn manual_system_source_uses_the_same_allowlist() {
        let snapshot = PlaybackSnapshot {
            player: Some(PlayerKind::System),
            is_running: true,
            source_app_bundle_id: Some("org.example.Player".into()),
            ..PlaybackSnapshot::default()
        };
        let allowed = [RegisteredApplication {
            name: "Player".into(),
            bundle_id: "org.example.Player".into(),
        }];
        assert_eq!(
            filter_system_source(snapshot.clone(), SystemMediaFilterMode::Allowlist, &allowed,)
                .error_code,
            None
        );
        assert_eq!(
            filter_system_source(snapshot, SystemMediaFilterMode::Blocklist, &allowed,).error_code,
            Some(PlaybackErrorCode::SourceNotAllowed)
        );

        for bundle_id in ["com.apple.Music", "com.spotify.client"] {
            let builtin = PlaybackSnapshot {
                player: Some(PlayerKind::System),
                is_running: true,
                source_app_bundle_id: Some(bundle_id.into()),
                ..PlaybackSnapshot::default()
            };
            assert_eq!(
                filter_system_source(builtin, SystemMediaFilterMode::Blocklist, &allowed,)
                    .error_code,
                None
            );
        }
    }

    #[test]
    fn automatic_routing_prefers_system_source_then_native_fallbacks() {
        let playing_system = |bundle_id: &str| PlaybackSnapshot {
            player: Some(PlayerKind::System),
            is_running: true,
            is_playing: true,
            title: Some("Track".into()),
            source_app_bundle_id: Some(bundle_id.into()),
            ..PlaybackSnapshot::default()
        };
        let native_music = PlaybackSnapshot {
            player: Some(PlayerKind::AppleMusic),
            is_running: true,
            is_playing: true,
            title: Some("Track".into()),
            ..PlaybackSnapshot::default()
        };
        let (snapshot, selected) = query_auto_player(
            playing_system("com.apple.Music"),
            None,
            SystemMediaFilterMode::Allowlist,
            &[],
            |kind| {
                if kind == PlayerKind::AppleMusic {
                    native_music.clone()
                } else {
                    PlaybackSnapshot::default()
                }
            },
        );
        assert_eq!(snapshot.player, Some(PlayerKind::AppleMusic));
        assert_eq!(selected, Some(PlayerKind::AppleMusic));

        let (snapshot, selected) = query_auto_player(
            playing_system("com.spotify.client"),
            None,
            SystemMediaFilterMode::Allowlist,
            &[],
            |_| PlaybackSnapshot::default(),
        );
        assert_eq!(snapshot.player, Some(PlayerKind::System));
        assert_eq!(selected, Some(PlayerKind::System));

        let allowed = [RegisteredApplication {
            name: "Browser".into(),
            bundle_id: "org.example.Browser".into(),
        }];
        let (snapshot, _) = query_auto_player(
            playing_system("org.example.Browser"),
            None,
            SystemMediaFilterMode::Allowlist,
            &allowed,
            |_| PlaybackSnapshot::default(),
        );
        assert_eq!(snapshot.error_code, None);
        let (snapshot, _) = query_auto_player(
            playing_system("org.example.Blocked"),
            None,
            SystemMediaFilterMode::Allowlist,
            &allowed,
            |kind| {
                if kind == PlayerKind::Spotify {
                    PlaybackSnapshot {
                        player: Some(kind),
                        is_running: true,
                        is_playing: true,
                        ..PlaybackSnapshot::default()
                    }
                } else {
                    PlaybackSnapshot::default()
                }
            },
        );
        assert_eq!(snapshot.player, Some(PlayerKind::Spotify));

        let (snapshot, _) = query_auto_player(
            playing_system("org.example.Blocked"),
            None,
            SystemMediaFilterMode::Allowlist,
            &allowed,
            |_| PlaybackSnapshot::default(),
        );
        assert_eq!(
            snapshot.error_code,
            Some(PlaybackErrorCode::SourceNotAllowed)
        );
    }

    #[test]
    fn automatic_routing_keeps_paused_system_source_and_uses_legacy_detection_without_one() {
        let paused = PlaybackSnapshot {
            player: Some(PlayerKind::System),
            is_running: true,
            title: Some("Paused Track".into()),
            source_app_bundle_id: Some("org.example.Player".into()),
            ..PlaybackSnapshot::default()
        };
        let allowed = [RegisteredApplication {
            name: "Player".into(),
            bundle_id: "org.example.Player".into(),
        }];
        let (snapshot, selected) = query_auto_player(
            paused,
            Some(PlayerKind::System),
            SystemMediaFilterMode::Allowlist,
            &allowed,
            |_| PlaybackSnapshot::default(),
        );
        assert_eq!(snapshot.title.as_deref(), Some("Paused Track"));
        assert_eq!(selected, Some(PlayerKind::System));

        let (snapshot, selected) = query_auto_player(
            PlaybackSnapshot::default(),
            None,
            SystemMediaFilterMode::Allowlist,
            &[],
            |kind| PlaybackSnapshot {
                player: Some(kind),
                is_running: true,
                is_playing: kind == PlayerKind::Spotify,
                ..PlaybackSnapshot::default()
            },
        );
        assert_eq!(snapshot.player, Some(PlayerKind::Spotify));
        assert_eq!(selected, Some(PlayerKind::Spotify));
    }
}

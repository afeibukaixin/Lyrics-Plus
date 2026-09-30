/// Spotify DJ 的过场口播使用 media URI；歌曲使用 track URI。
/// 同时校验艺人，避免把其他 media 内容误认为 DJ 口播。
pub(crate) fn is_dj_interlude(track_id: &str, artist: &str) -> bool {
    track_id.trim().starts_with("spotify:media:") && artist.trim().eq_ignore_ascii_case("DJ X")
}

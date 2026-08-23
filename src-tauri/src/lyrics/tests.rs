#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_metadata_multiple_timestamps_and_offset() {
        let raw = "[ti:Song]\n[ar:Artist]\n[offset:120]\n[00:01.00][00:02.500]Hello\n[00:03]World";
        let result = parse_lrc(raw, "test").unwrap();
        assert_eq!(result.metadata.title.as_deref(), Some("Song"));
        assert_eq!(result.metadata.artist.as_deref(), Some("Artist"));
        assert_eq!(result.offset_ms, 120);
        assert_eq!(result.tracks.original.lines.len(), 3);
        assert_eq!(result.tracks.original.lines[1].start_ms, 2500);
        assert_eq!(result.tracks.original.lines[1].end_ms, Some(3000));
    }

    #[test]
    fn separates_translation_lines_with_matching_timestamps() {
        let raw = "[00:01.00]Hello\n[00:01.00]你好\n[00:03.00]World\n[00:03.00]世界";
        let result = parse_lrc_with_options(raw, "test", true).unwrap();
        assert_eq!(result.tracks.original.lines[0].text, "Hello");
        assert_eq!(
            result.tracks.translation.as_ref().unwrap().lines[1].text,
            "世界"
        );
        assert!(result.metadata.manual_selected);
    }

    #[test]
    fn duplicate_text_is_not_misclassified_as_translation() {
        let raw = "[00:01.00]Hello\n[00:01.00]Hello";
        let result = parse_lrc(raw, "test").unwrap();
        assert!(result.tracks.translation.is_none());
    }

    #[test]
    fn rejects_unsynchronised_text() {
        assert!(parse_lrc("hello\nworld", "test").is_err());
    }

    #[test]
    fn parses_enhanced_lrc_word_timestamps_without_exposing_tags() {
        let raw = "[00:01.00]<00:01.00>Hello <00:01.50>world\n[00:03.00]Next";
        let result = parse_lrc(raw, "test").unwrap();
        let line = &result.tracks.original.lines[0];
        assert_eq!(result.metadata.original_format, "enhanced_lrc");
        assert_eq!(line.text, "Hello world");
        assert_eq!(line.words.as_ref().unwrap()[0].end_ms, 1500);
        assert_eq!(line.words.as_ref().unwrap()[1].end_ms, 3000);
    }

    #[test]
    fn parses_yrc_absolute_word_ranges() {
        let raw =
            "[ti:Song]\n[1000,1200](1000,400,0)你(1400,800,0)好\n[00:01.00]Hello\n[00:01.00]ni hao";
        let result = parse_lrc(raw, "test").unwrap();
        let line = &result.tracks.original.lines[0];
        assert_eq!(result.metadata.original_format, "yrc");
        assert_eq!(line.text, "你好");
        assert_eq!(line.end_ms, Some(2200));
        assert_eq!(line.words.as_ref().unwrap()[1].start_ms, 1400);
        assert_eq!(line.words.as_ref().unwrap()[1].end_ms, 2200);
        assert_eq!(
            result.tracks.translation.as_ref().unwrap().lines[0].text,
            "Hello"
        );
        assert_eq!(
            result.tracks.romanization.as_ref().unwrap().lines[0].text,
            "ni hao"
        );
    }

    #[test]
    fn parses_qrc_trailing_word_ranges() {
        let raw = "[1000,1200]Hello (1000,400)world(1400,800)";
        let result = parse_lrc(raw, "test").unwrap();
        let words = result.tracks.original.lines[0].words.as_ref().unwrap();
        assert_eq!(result.metadata.original_format, "qrc");
        assert_eq!(words[0].text, "Hello ");
        assert_eq!(words[1].start_ms, 1400);
    }

    #[test]
    fn third_same_timestamp_line_is_reserved_as_romanization() {
        let raw = "[00:01]今日は\n[00:01]今天\n[00:01]kyou wa";
        let result = parse_lrc(raw, "test").unwrap();
        assert_eq!(
            result.tracks.translation.as_ref().unwrap().lines[0].text,
            "今天"
        );
        assert_eq!(
            result.tracks.romanization.as_ref().unwrap().lines[0].text,
            "kyou wa"
        );
    }

    #[test]
    fn parses_ttml_explicit_word_ranges() {
        let raw = r#"<tt xmlns="http://www.w3.org/ns/ttml"><body><div>
          <p begin="00:00:01.000" end="00:00:03.000"><span begin="1s" end="1.5s">Hello </span><span begin="1500ms" end="3s">&amp; world</span></p>
        </div></body></tt>"#;
        let result = parse_lrc(raw, "test").unwrap();
        let line = &result.tracks.original.lines[0];
        assert_eq!(result.metadata.original_format, "ttml");
        assert_eq!(line.text, "Hello & world");
        assert_eq!(line.words.as_ref().unwrap()[1].start_ms, 1500);
        assert_eq!(line.words.as_ref().unwrap()[1].end_ms, 3000);
    }
}

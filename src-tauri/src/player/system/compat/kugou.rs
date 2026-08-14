use super::TrackMetadata;

pub(super) const BUNDLE_ID: &str = "com.kugou.kgyouth";

pub(super) fn normalize(metadata: TrackMetadata) -> TrackMetadata {
    if metadata
        .artist
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return metadata;
    }

    let Some(combined) = metadata.title.as_deref() else {
        return metadata;
    };
    for separator in [" - ", " – ", " — "] {
        let Some((artist, title)) = combined.split_once(separator) else {
            continue;
        };
        let artist = artist.trim();
        let title = title.trim();
        if !artist.is_empty() && !title.is_empty() {
            return TrackMetadata::new(Some(title.to_owned()), Some(artist.to_owned()));
        }
    }
    metadata
}

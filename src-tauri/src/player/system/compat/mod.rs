mod kugou;

pub(super) struct TrackMetadata {
    pub(super) title: Option<String>,
    pub(super) artist: Option<String>,
}

impl TrackMetadata {
    pub(super) fn new(title: Option<String>, artist: Option<String>) -> Self {
        Self { title, artist }
    }
}

pub(super) fn normalize(bundle_id: Option<&str>, metadata: TrackMetadata) -> TrackMetadata {
    match bundle_id {
        Some(kugou::BUNDLE_ID) => kugou::normalize(metadata),
        _ => metadata,
    }
}

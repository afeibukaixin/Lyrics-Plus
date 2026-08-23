#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveKind {
    Automatic,
    ManualSelection,
    Import,
}

pub struct SaveRequest<'a> {
    pub track_key: &'a str,
    pub title: &'a str,
    pub artist: &'a str,
    pub source: &'a str,
    pub raw: &'a str,
    pub provider_id: Option<&'a str>,
    pub provider_item_id: Option<&'a str>,
    pub kind: SaveKind,
}

impl SaveKind {
    fn is_manual(self) -> bool {
        matches!(self, Self::ManualSelection | Self::Import)
    }
}

pub struct Storage {
    connection: Mutex<Connection>,
    database_path: PathBuf,
    library_dir: RwLock<PathBuf>,
    scanner: library::LibraryScanCoordinator,
}

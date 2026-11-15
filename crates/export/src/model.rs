//! What the stick's two databases hold, worked out once so `export.pdb` and
//! OneLibrary list the same tracks and lists under the same ids, as
//! rekordbox writes them.

/// One exported track. Its id is its place in [`Model::tracks`] plus one.
#[derive(Debug, Clone, Default)]
pub struct TrackRow {
    /// Where the audio is on the stick.
    pub path: String,
    /// Where its `.DAT` analysis file is on the stick.
    pub anlz_path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub label: String,
    pub remixer: String,
    pub composer: String,
    /// The mix name, e.g. "Club Mix".
    pub mix_name: String,
    pub comment: String,
    pub key: String,
    pub isrc: String,
    pub year: i32,
    pub track_number: i64,
    pub disc_number: i32,
    /// 0..5 stars.
    pub rating: i32,
    /// One of the players' eight track colours, 0 for none.
    pub colour_id: i64,
    pub bpmx100: i64,
    /// Whole seconds.
    pub length: i32,
    pub sample_rate: i64,
    /// kb/s.
    pub bitrate: i64,
    pub bit_depth: i32,
    pub file_size: i64,
    pub file_type: i32,
    /// `YYYY-MM-DD`.
    pub date_added: String,
    pub play_count: i32,
    /// Artwork id, 0 for none.
    pub artwork: i64,
    /// The track's id in DJ Sugar's library.
    pub library_id: i64,
    /// When it was added, as a Unix time.
    pub added_unix: i64,
    /// Engine DJ's own: its performance data, key number, album art id
    /// (0 for none) and whether the DJ locked the grid.
    pub engine: Option<crate::engine::Performance>,
    pub engine_key: Option<u32>,
    pub art_id: i64,
    pub grid_locked: bool,
}

/// A list or folder. Folders come before what they hold.
#[derive(Debug, Clone, Default)]
pub struct ListRow {
    pub name: String,
    /// Index of the folder it is in, `None` at the top.
    pub parent: Option<usize>,
    pub folder: bool,
    /// Track ids, in order.
    pub tracks: Vec<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct Model {
    pub device_name: String,
    /// `YYYY-MM-DD`.
    pub date: String,
    pub tracks: Vec<TrackRow>,
    pub lists: Vec<ListRow>,
    /// Artwork ids handed out, 1 up to this.
    pub artworks: u32,
    /// The stick's export folder: `PIONEER`, or `.PIONEER` where rekordbox
    /// already wrote it hidden.
    pub export_dir: &'static str,
    /// Engine DJ's album art.
    pub art: crate::engine::Art,
}

/// The library id the stick's databases name as where they came from, for
/// both formats. rekordbox uses its own collection's id; this one is
/// DJ Sugar's, so players never mistake the stick for a rekordbox library.
pub const MASTER_DB_ID: i64 = 0x0D3A_0001;

/// What rekordbox writes for a track it analysed, in both formats.
pub const ANALYSED_BITS: i32 = 0x29;
pub const CONTENT_LINK: i64 = 0x000C_0700;

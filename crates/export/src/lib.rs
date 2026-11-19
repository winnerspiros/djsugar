//! Writing lists of tracks to a USB stick that Pioneer DJ players read, as
//! rekordbox's USB export does: the audio, each track's analysis files
//! (beat grid, cues, loops, waveforms), album art, and the library in both
//! formats the players know:
//!
//! - **Device Library** (`export.pdb`): CDJ-3000, CDJ-2000NXS2 and older,
//!   XDJ-XZ, XDJ-RX3 and most other players.
//! - **OneLibrary** (`exportLibrary.db`): CDJ-3000X, XDJ-AZ, OPUS-QUAD and
//!   OMNIS-DUO, which don't read the Device Library.
//!
//! - **Engine DJ** (`Engine Library/Database2/m.db`): Denon DJ, Numark and
//!   Rane players running Engine OS (see [`engine`]).
//!
//! The Pioneer files are written with rbox (<https://github.com/dylanljones/rbox>).
//! Everything comes from DJ Sugar's own library and analysis; tracks that were
//! never analysed are analysed first.

mod anlz;
mod artwork;
pub mod engine;
mod layout;
mod model;
mod mp3;
mod onelib;
mod pdb;
mod wave;

use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use djsugar_engine::key::Key;
use djsugar_engine::track::Hotcue;
use djsugar_engine::{cache, loader, BeatGrid, Source, Track};

use model::{ListRow, Model, TrackRow};

/// What to write, and where.
#[derive(Debug, Clone)]
pub struct Options {
    /// The stick's top folder (where `PIONEER` and `Contents` go).
    pub root: PathBuf,
    /// The name players show for the stick.
    pub name: String,
    /// Write `export.pdb` for the CDJ-3000 and older players.
    pub device_library: bool,
    /// Write `exportLibrary.db` for the CDJ-3000X, XDJ-AZ and OPUS-QUAD.
    pub one_library: bool,
    /// Write Engine DJ's `m.db` for Denon, Numark and Rane players.
    pub engine_dj: bool,
    /// Keys in Camelot notation ("8A") rather than notes ("Am").
    pub camelot: bool,
}

impl Options {
    /// Writes a Pioneer library (either format).
    pub fn pioneer(&self) -> bool {
        self.device_library || self.one_library
    }
}

/// A list or folder to put on the stick. Folders come before what they hold.
#[derive(Debug, Clone, Default)]
pub struct List {
    pub name: String,
    /// Index of its folder in the lists given, `None` at the top.
    pub parent: Option<usize>,
    pub folder: bool,
    /// Indices into the tracks given.
    pub tracks: Vec<usize>,
}

/// Where the export is.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Analysing (when needed) and copying track `done + 1` of `total`.
    Track {
        done: usize,
        total: usize,
        title: String,
    },
    /// Writing the library files.
    Library,
}

/// How an export went.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    /// Tracks on the stick.
    pub tracks: usize,
    /// Audio files copied; the others were on the stick already.
    pub copied: usize,
    /// Tracks left out, and why.
    pub skipped: Vec<(String, String)>,
    /// Hot cues past H, which players don't have.
    pub hot_cues_dropped: usize,
    pub lists: usize,
    /// Files an earlier export put on the stick for tracks no longer on it,
    /// removed.
    pub removed: usize,
}

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Analysis(rbox_anlz::AnlzError),
    DeviceLibrary(rbox_devlib::Error),
    OneLibrary(String),
    /// The DJ stopped the export.
    Cancelled,
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Analysis(e) => write!(f, "analysis file: {e}"),
            Error::DeviceLibrary(e) => write!(f, "export.pdb: {e}"),
            Error::OneLibrary(e) => write!(f, "OneLibrary: {e}"),
            Error::Cancelled => write!(f, "stopped"),
            Error::Other(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<rbox_anlz::AnlzError> for Error {
    fn from(e: rbox_anlz::AnlzError) -> Self {
        Error::Analysis(e)
    }
}

impl From<rbox_devlib::Error> for Error {
    fn from(e: rbox_devlib::Error) -> Self {
        Error::DeviceLibrary(e)
    }
}

impl From<rbox_db::error::Error> for Error {
    fn from(e: rbox_db::error::Error) -> Self {
        Error::OneLibrary(e.to_string())
    }
}

impl From<diesel::result::Error> for Error {
    fn from(e: diesel::result::Error) -> Self {
        Error::OneLibrary(e.to_string())
    }
}

/// Library files a previous export may have left that this one replaces or
/// removes, so players never read an old library beside the new one.
const LIBRARY_FILES: [&str; 5] = [
    "export.pdb",
    "exportExt.pdb",
    "exportLibrary.db",
    "exportLibrary.db-wal",
    "exportLibrary.db-shm",
];

/// The stick's export folder: the one that already holds a library, else
/// `PIONEER`. rekordbox can write it hidden as `.PIONEER`, and a second
/// library beside it would leave players to pick one.
fn export_dir(root: &Path) -> Result<&'static str, Error> {
    let holds_library = |name: &str| {
        let dir = root.join(name);
        dir.join("rekordbox/export.pdb").is_file()
            || dir.join("rekordbox/exportLibrary.db").is_file()
            || dir.join("DEVSETTING.DAT").is_file()
    };
    match (holds_library("PIONEER"), holds_library(".PIONEER")) {
        (true, true) => Err(Error::Other(
            "the stick has two libraries, in PIONEER and .PIONEER; remove one first".into(),
        )),
        (false, true) => Ok(".PIONEER"),
        _ => Ok("PIONEER"),
    }
}

/// Joins a stick path (`/Contents/...`) onto the stick's folder.
fn on_stick(root: &Path, stick_path: &str) -> PathBuf {
    root.join(stick_path.trim_start_matches('/'))
}

fn file_type(path: &Path) -> i32 {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "mp3" => 1,
        "m4a" | "aac" | "mp4" => 4,
        "flac" => 5,
        "wav" => 11,
        "aif" | "aiff" => 12,
        _ => 0,
    }
}

/// Sample rate, bitrate (kb/s) and bit depth from the file's header.
fn properties(path: &Path) -> (i64, i64, i32) {
    use lofty::prelude::*;
    let Ok(file) = lofty::read_from_path(path) else {
        return (44100, 0, 16);
    };
    let p = file.properties();
    (
        p.sample_rate().unwrap_or(44100) as i64,
        p.audio_bitrate().or(p.overall_bitrate()).unwrap_or(0) as i64,
        p.bit_depth().unwrap_or(16) as i32,
    )
}

fn date(unix: i64) -> String {
    chrono::DateTime::from_timestamp(unix, 0)
        .unwrap_or_else(chrono::Utc::now)
        .format("%Y-%m-%d")
        .to_string()
}

fn year(text: &str) -> i32 {
    let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
    if digits.len() == 4 {
        digits.parse().unwrap_or(0)
    } else {
        0
    }
}

/// The grid the DJ plays with: the library's, else the analysis's.
fn grid_for(track: &Track, analysis: &djsugar_engine::analysis::Analysis) -> BeatGrid {
    if !track.grid.is_empty() {
        track.grid.clone()
    } else if track.bpm > 0.0 {
        BeatGrid::constant(track.bpm, track.first_beat)
    } else if !analysis.grid.is_empty() {
        analysis.grid.clone()
    } else if analysis.bpm > 0.0 {
        BeatGrid::constant(analysis.bpm, analysis.first_beat)
    } else {
        BeatGrid::default()
    }
}

/// Writes `lists` and their `tracks` to the stick. `progress` hears how it
/// goes and can stop it by returning false. The stick's library is
/// replaced; audio already on it is kept and not copied again.
pub fn export(
    tracks: &[Track],
    lists: &[List],
    options: &Options,
    progress: &mut dyn FnMut(Step) -> bool,
) -> Result<Report, Error> {
    if !options.pioneer() && !options.engine_dj {
        return Err(Error::Other("no library format picked".into()));
    }
    if !options.root.is_dir() {
        return Err(Error::Other(format!(
            "{} is not a folder",
            options.root.display()
        )));
    }
    let root = &options.root;
    let export_dir = if options.pioneer() {
        export_dir(root)?
    } else {
        "PIONEER"
    };
    if options.engine_dj {
        engine::check(root)?;
    }
    let today = date(chrono::Utc::now().timestamp());

    // Every track the lists hold, once, in the order they first appear.
    let mut wanted: Vec<usize> = Vec::new();
    let mut seen = vec![false; tracks.len()];
    for list in lists.iter().filter(|l| !l.folder) {
        for &i in &list.tracks {
            if i < tracks.len() && !seen[i] {
                seen[i] = true;
                wanted.push(i);
            }
        }
    }

    let mut report = Report::default();
    let mut model = Model {
        device_name: options.name.clone(),
        date: today.clone(),
        export_dir,
        ..Model::default()
    };
    let mut paths = layout::Paths::default();
    let mut slots = layout::AnlzSlots::default();
    // Track index -> id on the stick.
    let mut ids: Vec<Option<i64>> = vec![None; tracks.len()];
    let mut files: HashSet<String> = HashSet::new();
    let total = wanted.len();
    for (done, &i) in wanted.iter().enumerate() {
        let track = &tracks[i];
        let label = if track.artist.is_empty() {
            track.title.clone()
        } else {
            format!("{} - {}", track.artist, track.title)
        };
        if !progress(Step::Track {
            done,
            total,
            title: label.clone(),
        }) {
            return Err(Error::Cancelled);
        }
        match put_track(track, root, options, &mut paths, &mut slots, &mut model) {
            Ok(put) => {
                report.copied += put.copied as usize;
                report.hot_cues_dropped += put.hot_cues_dropped;
                files.extend(put.files);
                ids[i] = Some(model.tracks.len() as i64);
            }
            Err(why) => report.skipped.push((label, why)),
        }
    }

    for list in lists {
        let mut list_tracks: Vec<i64> = Vec::new();
        for &i in &list.tracks {
            if let Some(Some(id)) = ids.get(i) {
                // A list holds each track once on a player.
                if !list_tracks.contains(id) {
                    list_tracks.push(*id);
                }
            }
        }
        model.lists.push(ListRow {
            name: list.name.clone(),
            parent: list.parent,
            folder: list.folder,
            tracks: list_tracks,
        });
    }
    if model.tracks.is_empty() {
        // Leave the stick as it was rather than give it an empty library.
        return Err(Error::Other(match report.skipped.first() {
            Some((title, why)) => format!("no track could be exported ({title}: {why})"),
            None => "there are no tracks to export".into(),
        }));
    }
    report.tracks = model.tracks.len();
    report.lists = lists.iter().filter(|l| !l.folder).count();

    if !progress(Step::Library) {
        return Err(Error::Cancelled);
    }
    write_library(root, options, &model)?;
    report.removed = tidy(root, &files)?;
    Ok(report)
}

/// The files DJ Sugar's last export put on the stick, one per line from the
/// stick's top, hidden at its top (for Pioneer and Engine DJ sticks alike).
/// Only these are ever removed.
const MANIFEST: &str = ".djsugar-files.txt";

/// Removes what the last export put on the stick that this one didn't, and
/// folders that leaves empty; then notes this export's files. The DJ's own
/// files on the stick are never touched.
fn tidy(root: &Path, files: &HashSet<String>) -> Result<usize, Error> {
    let manifest = root.join(MANIFEST);
    let before = fs::read_to_string(&manifest).unwrap_or_default();
    let mut removed = 0;
    for old in before.lines().filter(|l| !files.contains(*l)) {
        // Only plain paths under the stick, whatever the file says.
        let rel = Path::new(old);
        if old.is_empty()
            || !rel
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
        {
            continue;
        }
        let path = root.join(rel);
        if fs::remove_file(&path).is_ok() {
            removed += 1;
            let mut up = path.parent();
            while let Some(d) = up.filter(|d| *d != root && d.starts_with(root)) {
                if fs::remove_dir(d).is_err() {
                    break;
                }
                up = d.parent();
            }
        }
    }
    let mut list: Vec<&str> = files.iter().map(String::as_str).collect();
    list.sort_unstable();
    fs::write(&manifest, list.join("\n") + "\n")?;
    Ok(removed)
}

struct Put {
    copied: bool,
    hot_cues_dropped: usize,
    /// What it put on the stick, from the stick's top (see [`MANIFEST`]).
    files: Vec<String>,
}

/// Copies one track's audio and writes its analysis files and art.
fn put_track(
    track: &Track,
    root: &Path,
    options: &Options,
    paths: &mut layout::Paths,
    slots: &mut layout::AnlzSlots,
    model: &mut Model,
) -> Result<Put, String> {
    let Source::File(source) = &track.source else {
        return Err("not a file".into());
    };
    let size = fs::metadata(source)
        .map_err(|_| "file not found".to_string())?
        .len();
    let file_name = source
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("track")
        .to_string();

    let analysis = match cache::load(source) {
        Some(a) => a,
        None => loader::analyse_file_keeping(track, false)?.1,
    };
    let grid = grid_for(track, &analysis);
    let finest = analysis.waveform.levels.first().map_or(0, Vec::len);
    let duration = if track.duration > 0.0 {
        track.duration
    } else {
        finest as f64 / analysis.waveform.bins_per_second.max(1.0)
    };
    let bpm = if track.bpm > 0.0 {
        track.bpm
    } else {
        grid.main_bpm(duration)
    };

    let stick_path = paths.assign(&track.artist, &track.album, &file_name);
    let stem = layout::rooted(&slots.assign(&stick_path), model.export_dir);
    let hot_cues_dropped = track
        .info
        .hotcues
        .iter()
        .filter(|c| !c.is_memory() && c.number >= anlz::HOT_CUES)
        .count();

    let err = |e: &dyn fmt::Display| e.to_string();
    let cues: Vec<Hotcue> = track.info.hotcues.clone();
    if options.pioneer() {
        let anlz_stem = on_stick(root, &stem);
        fs::create_dir_all(anlz_stem.parent().unwrap()).map_err(|e| err(&e))?;
        let mp3_frames = (file_type(source) == 1)
            .then(|| mp3::frames(source))
            .flatten();
        let files = anlz::build(
            &anlz_stem,
            anlz::TrackAnalysis {
                stick_path: &stick_path,
                beats: anlz::beats(&grid, duration),
                bpm,
                cues: &cues,
                waveforms: wave::waveforms(&analysis.waveform, duration),
                mp3_frames,
            },
        )
        .map_err(|e| err(&e))?;
        for mut f in files {
            f.dump().map_err(|e| err(&e))?;
        }
    }

    // Stick paths from the stick's top, for the manifest.
    let top = |p: &str| p.trim_start_matches('/').to_string();
    let mut files = vec![top(&stick_path)];
    if options.pioneer() {
        files.extend(["DAT", "EXT", "2EX"].map(|ext| top(&format!("{stem}.{ext}"))));
    }

    let dest = on_stick(root, &stick_path);
    let copied = match fs::metadata(&dest) {
        Ok(m) if m.len() == size => false,
        _ => {
            fs::create_dir_all(dest.parent().unwrap()).map_err(|e| err(&e))?;
            fs::copy(source, &dest).map_err(|e| err(&e))?;
            true
        }
    };

    let thumbs = artwork::for_track(source, &track.album);
    let art_id = match (&thumbs, options.engine_dj) {
        (Some(t), true) => model.art.add(t.medium.clone()),
        _ => 0,
    };
    let artwork = match thumbs.filter(|_| options.pioneer()) {
        Some(thumbs) => {
            let id = model.artworks + 1;
            for copy in ['a', 'b'] {
                for (medium, bytes) in [(false, &thumbs.small), (true, &thumbs.medium)] {
                    let art = layout::rooted(&layout::artwork(id, copy, medium), model.export_dir);
                    files.push(top(&art));
                    let path = on_stick(root, &art);
                    fs::create_dir_all(path.parent().unwrap()).map_err(|e| err(&e))?;
                    fs::write(&path, bytes).map_err(|e| err(&e))?;
                }
            }
            model.artworks = id;
            id as i64
        }
        None => 0,
    };

    let (sample_rate, bitrate, bit_depth) = properties(source);
    let parsed_key = Key::parse(&track.key);
    let engine = options.engine_dj.then(|| {
        engine::perf::build(&engine::perf::Input {
            sample_rate: sample_rate as f64,
            duration,
            grid: &grid,
            waveform: &analysis.waveform,
            cues: &cues,
            key: parsed_key,
        })
    });
    let key = parsed_key
        .map(|k| {
            if options.camelot {
                k.camelot()
            } else {
                k.name()
            }
        })
        .unwrap_or_default();
    let info = &track.info;
    model.tracks.push(TrackRow {
        path: stick_path,
        anlz_path: format!("{stem}.DAT"),
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: track.album.clone(),
        genre: track.genre.clone(),
        label: info.label.clone(),
        remixer: info.remixer.clone(),
        composer: info.composer.clone(),
        mix_name: info.remix.clone(),
        comment: info.comment.clone(),
        key,
        isrc: info.isrc.clone(),
        year: year(&info.year),
        track_number: info.track_number as i64,
        disc_number: info.disc_number as i32,
        rating: track.rating.min(5) as i32,
        colour_id: anlz::colour_id(info.color),
        bpmx100: (bpm * 100.0).round() as i64,
        length: duration.round() as i32,
        sample_rate,
        bitrate,
        bit_depth,
        file_size: size as i64,
        file_type: file_type(source),
        date_added: if info.added > 0 {
            date(info.added)
        } else {
            model.date.clone()
        },
        play_count: info.play_count as i32,
        artwork,
        library_id: track.id as i64,
        added_unix: if info.added > 0 {
            info.added
        } else {
            chrono::Utc::now().timestamp()
        },
        engine,
        engine_key: parsed_key.map(engine::perf::key_number),
        art_id,
        grid_locked: info.grid_locked,
    });
    Ok(Put {
        copied,
        hot_cues_dropped,
        files,
    })
}

/// Builds the library files on this computer, then puts them on the stick
/// in place of any old ones.
fn write_library(root: &Path, options: &Options, model: &Model) -> Result<(), Error> {
    if options.engine_dj {
        engine::write(root, model)?;
    }
    if !options.pioneer() {
        return Ok(());
    }
    let work = tempfile::tempdir()?;
    let built = work.path().join("rekordbox");
    if options.device_library {
        pdb::write(&built, model)?;
    } else {
        fs::create_dir_all(&built)?;
    }
    if options.one_library {
        onelib::write(&built.join("exportLibrary.db"), model)?;
    }
    let dir = root.join(model.export_dir).join("rekordbox");
    fs::create_dir_all(&dir)?;
    for name in LIBRARY_FILES {
        let path = dir.join(name);
        if path.exists() {
            fs::remove_file(&path)?;
        }
    }
    for name in LIBRARY_FILES {
        let from = built.join(name);
        if from.exists() {
            fs::copy(&from, dir.join(name))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod check;
#[cfg(test)]
mod tests;

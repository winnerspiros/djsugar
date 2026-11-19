//! A rekordbox USB stick or SD card for Pioneer players: its Device Library,
//! `PIONEER/rekordbox/export.pdb` (read with rbox), and each track's
//! analysis files beside it (`PIONEER/USBANLZ/…`), which hold its beat
//! grid, hot cues, memory cues, loops and phrases. Paths on the stick are
//! from its root, so a stick reads the same wherever it's mounted.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use djsugar_engine::import::{ImportedEntry, ImportedList, ImportedListKind};
use djsugar_engine::{Hotcue, LibraryInfo, Source, Track, MEMORY_CUE};
use rbox_anlz::Anlz;
use rbox_core::PlaylistType;
use rbox_devlib::DeviceLibrary;

use super::common::{self, Mood};
use super::{HISTORY, PLAYLISTS};
use crate::ImportedLibrary;

/// Whether `path` is a Device Library file.
pub fn is_device(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("export.pdb"))
}

/// The stick's root: `export.pdb` is in `PIONEER/rekordbox`.
fn root(pdb: &Path) -> PathBuf {
    pdb.parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .unwrap_or(Path::new("."))
        .to_path_buf()
}

/// A path on the stick (from its root) as a file here.
fn on_stick(root: &Path, path: &Path) -> PathBuf {
    root.join(path.strip_prefix("/").unwrap_or(path))
}

fn open(pdb: &Path) -> Result<DeviceLibrary, String> {
    let dir = pdb.parent().unwrap_or(Path::new("."));
    DeviceLibrary::new(dir).map_err(|e| format!("can't open the stick's library: {e}"))
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// What the stick holds, as a line for the wizard.
pub fn preview(pdb: &Path) -> Result<String, String> {
    let mut db = open(pdb)?;
    let tracks = db.get_contents().map_err(err)?.len();
    let lists = db
        .get_playlists()
        .map_err(err)?
        .iter()
        .filter(|p| p.attribute == PlaylistType::List)
        .count();
    let history = db.get_histories().map_err(err)?.len();
    Ok(super::summary(tracks, lists, 0, history))
}

/// The stick's tracks whose files `exists` says are there, and the lists
/// in `groups`.
pub fn read(
    pdb: &Path,
    groups: &[&str],
    exists: impl Fn(&Path) -> bool,
) -> Result<ImportedLibrary, String> {
    let root = root(pdb);
    let mut db = open(pdb)?;
    let mut tracks = Vec::new();
    let mut entries: HashMap<i64, ImportedEntry> = HashMap::new();
    for t in db.get_tracks().map_err(err)? {
        let Some(id) = t.id else {
            continue;
        };
        let file = on_stick(&root, &t.path);
        if !exists(&file) {
            continue;
        }
        let mut track = track(&t, file.clone());
        if let Some(anlz) = &t.analysis_data_path {
            analysis(&on_stick(&root, anlz), &mut track);
        }
        entries.insert(
            i64::from(id),
            ImportedEntry {
                path: file,
                played_at: None,
                streamed: None,
            },
        );
        tracks.push(track);
    }
    let lists = lists(&mut db, groups, &entries)?;
    Ok(ImportedLibrary { tracks, lists })
}

/// The playlists (in their folders) and history in `groups`.
pub fn lists(
    db: &mut DeviceLibrary,
    groups: &[&str],
    entries: &HashMap<i64, ImportedEntry>,
) -> Result<Vec<ImportedList>, String> {
    let mut out = Vec::new();
    if groups.contains(&PLAYLISTS) {
        let all = db.get_playlists().map_err(err)?;
        let by_id: HashMap<i64, (&str, i64)> = all
            .iter()
            .map(|p| (p.id, (p.name.as_str(), p.parent_id)))
            .collect();
        let mut order: Vec<_> = all
            .iter()
            .filter(|p| p.attribute == PlaylistType::List)
            .collect();
        // The order the stick shows them: by the path of positions.
        let place = |mut id: i64| {
            let mut path = Vec::new();
            while let Some(p) = all.iter().find(|p| p.id == id) {
                path.insert(0, p.seq);
                id = p.parent_id;
            }
            path
        };
        order.sort_by_key(|p| place(p.id));
        for p in order {
            let mut folders = Vec::new();
            let mut up = p.parent_id;
            while let Some(&(name, parent)) = by_id.get(&up) {
                folders.insert(0, name.to_string());
                up = parent;
            }
            let contents = db.get_playlist_contents(p.id).map_err(err)?;
            out.push(ImportedList {
                folders,
                name: p.name.clone(),
                kind: ImportedListKind::List,
                entries: contents
                    .iter()
                    .filter_map(|c| entries.get(&c.id).cloned())
                    .collect(),
            });
        }
    }
    if groups.contains(&HISTORY) {
        for h in db.get_histories().map_err(err)? {
            let contents = db.get_history_contents(h.id).map_err(err)?;
            out.push(ImportedList {
                folders: Vec::new(),
                name: h.name.clone(),
                kind: ImportedListKind::History {
                    started: crate::virtualdj::lists::day_start(&h.name).unwrap_or(0),
                },
                entries: contents
                    .iter()
                    .filter_map(|c| entries.get(&c.id).cloned())
                    .collect(),
            });
        }
    }
    Ok(out)
}

fn track(t: &rbox_core::Track, file: PathBuf) -> Track {
    let text = |s: &Option<String>| s.as_deref().unwrap_or_default().trim().to_string();
    Track {
        id: 0,
        artist: text(&t.artist),
        title: text(&t.title),
        album: text(&t.album),
        genre: text(&t.genre),
        rating: t.rating.unwrap_or(0).min(5),
        bpm: t.bpm.unwrap_or(0.0),
        key: text(&t.key),
        duration: t.length.unwrap_or(0.0).max(0.0),
        first_beat: 0.0,
        grid: Default::default(),
        source: Source::File(file),
        waveform: Arc::default(),
        audio: None,
        cover: None,
        info: LibraryInfo {
            comment: text(&t.commnt),
            color: t.color.and_then(|c| common::track_colour(c.into())),
            play_count: t.play_count.unwrap_or(0),
            year: t
                .release_year
                .filter(|&y| y > 0)
                .map(|y| y.to_string())
                .unwrap_or_default(),
            track_number: t.track_no.unwrap_or(0),
            disc_number: t.disc_no.unwrap_or(0),
            label: text(&t.label),
            composer: text(&t.composer),
            remixer: text(&t.remixer),
            isrc: text(&t.isrc),
            added: t
                .stock_date
                .as_deref()
                .and_then(crate::virtualdj::lists::day_start)
                .unwrap_or(0),
            ..Default::default()
        },
    }
}

/// The grid, cues and phrases from the track's analysis files: the beat
/// grid and plain cues in `.DAT`, cues with names and colours and the
/// phrases in `.EXT` when it's there.
fn analysis(dat: &Path, t: &mut Track) {
    let Ok(mut file) = Anlz::load(dat) else {
        return;
    };
    let beats: Vec<(f64, f64)> = match file.get_beat_grid() {
        Ok(Some(g)) => g
            .beats
            .iter()
            .map(|b| (f64::from(b.time) / 1000.0, f64::from(b.tempo) / 100.0))
            .collect(),
        _ => Vec::new(),
    };
    if !beats.is_empty() {
        t.set_grid(common::grid_from_beats(&beats));
        t.info.fixed_grid = true;
    }
    let mut cues: Vec<(u32, u32, u32, String, Option<u32>)> = Vec::new();
    let mut ext = Anlz::load(dat.with_extension("EXT")).ok();
    if let Some(ext) = ext.as_mut() {
        for memory in [false, true] {
            let list = if memory {
                ext.get_extended_memory_cues()
            } else {
                ext.get_extended_hot_cues()
            };
            let Ok(Some(list)) = list else {
                continue;
            };
            for c in &list.cues {
                let (r, g, b) = c.hot_cue_color_rgb;
                let colour = if c.hot_cue > 0 {
                    common::hot_cue_colour(c.hot_cue_color_index.into()).or_else(|| {
                        (r, g, b)
                            .ne(&(0, 0, 0))
                            .then(|| u32::from_be_bytes([0, r, g, b]))
                    })
                } else {
                    common::track_colour(u8::from(c.color).into())
                };
                cues.push((
                    c.hot_cue,
                    c.time,
                    c.loop_time,
                    c.comment.to_string(),
                    colour,
                ));
            }
        }
    }
    if cues.is_empty() {
        for memory in [false, true] {
            let list = if memory {
                file.get_memory_cues()
            } else {
                file.get_hot_cues()
            };
            let Ok(Some(list)) = list else {
                continue;
            };
            for c in &list.cues {
                cues.push((c.hot_cue, c.time, c.loop_time, String::new(), None));
            }
        }
    }
    let mut memory = MEMORY_CUE;
    for (pad, time, loop_end, name, color) in cues {
        let seconds = f64::from(time) / 1000.0;
        // A loop's end; players write u32::MAX for "none".
        let end = (loop_end > time && loop_end != u32::MAX).then(|| f64::from(loop_end) / 1000.0);
        let number = if pad == 0 {
            memory += 1;
            memory - 1
        } else {
            // A to C, then D to H as 3 and 5 to 8, as master.db's are kept.
            match common::hot_cue_number(if pad >= 4 { pad as i64 + 1 } else { pad as i64 }) {
                Some(n) => n,
                None => continue,
            }
        };
        if t.info.hotcues.iter().any(|h| h.number == number) {
            continue;
        }
        t.info.hotcues.push(Hotcue {
            number,
            seconds,
            name,
            end,
            color,
        });
    }
    t.info.hotcues.sort_by_key(|h| h.number);
    if let Some(ext) = ext.as_mut() {
        if let Ok(Some(s)) = ext.get_song_structure() {
            let mood = match s.mood {
                rbox_anlz::Mood::High => Mood::High,
                rbox_anlz::Mood::Low => Mood::Low,
                _ => Mood::Mid,
            };
            let starts: Vec<f64> = beats.iter().map(|b| b.0).collect();
            let phrases: Vec<(u16, u16)> = s.phrases.iter().map(|p| (p.beat, p.kind)).collect();
            t.info.markers = common::phrases(mood, &phrases, s.end_beat, &starts);
        }
    }
}

#[cfg(test)]
mod tests {
    use rbox_devlib::models::NewDevContent;
    use rbox_devlib::LibraryCreateOptions;

    use super::*;

    /// A stick made with rbox, read back.
    #[test]
    fn reads_a_stick() {
        let root = std::env::temp_dir().join(format!("djs-pdb-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("PIONEER/rekordbox");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(root.join("Contents")).unwrap();
        for f in ["a.mp3", "b.mp3"] {
            std::fs::write(root.join("Contents").join(f), b"x").unwrap();
        }
        {
            let options = LibraryCreateOptions::new("2026-11-19", "STICK", 1);
            let mut db = DeviceLibrary::create(&dir, &options).unwrap();
            let artist = db.create_artist("Someone").unwrap().id;
            let mut ids = Vec::new();
            for (path, title) in [
                ("/Contents/a.mp3", "A"),
                ("/Contents/b.mp3", "B"),
                ("/Contents/gone.mp3", "Gone"),
            ] {
                let mut row = NewDevContent::new(path);
                row.title = title.into();
                row.artist_id = artist;
                row.bpmx100 = 12_450;
                row.length = 200;
                row.rating = 3;
                ids.push(db.insert_content(row).unwrap().id);
            }
            let folder = db.create_playlist_folder("Gigs", None, None).unwrap().id;
            let list = db.create_playlist("Friday", Some(folder), None).unwrap().id;
            for &t in [ids[1], ids[2], ids[0]].iter() {
                db.create_playlist_content(list, t, None).unwrap();
            }
            db.save().unwrap();
        }
        let pdb = dir.join("export.pdb");
        assert!(is_device(&pdb));
        let summary = preview(&pdb).unwrap();
        let lib = read(&pdb, &[PLAYLISTS, HISTORY], |p| p.exists()).unwrap();
        std::fs::remove_dir_all(&root).ok();

        assert!(summary.contains('3'), "{summary}");
        let titles: Vec<&str> = lib.tracks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["A", "B"]);
        let a = &lib.tracks[0];
        assert_eq!(
            (a.artist.as_str(), a.bpm, a.duration, a.rating),
            ("Someone", 124.5, 200.0, 3)
        );
        assert!(matches!(&a.source, Source::File(f) if f.ends_with("Contents/a.mp3")));
        assert_eq!(lib.lists.len(), 1);
        assert_eq!(
            (lib.lists[0].folders.clone(), lib.lists[0].name.as_str()),
            (vec!["Gigs".to_string()], "Friday")
        );
        let names: Vec<String> = lib.lists[0]
            .entries
            .iter()
            .map(|e| e.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["b.mp3", "a.mp3"]);
    }
}

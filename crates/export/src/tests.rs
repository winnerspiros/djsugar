//! Exports of real (made-up) audio to a folder, read back with rbox and with
//! an independent reader (`check`).

use super::*;
use crate::check;
use djsugar_engine::scan;
use rbox_devlib::DeviceLibrary;
use rbox_onelib::OneLibrary;

/// A 16-bit mono WAV of clicks at 120 BPM.
fn click_track(path: &Path, seconds: f64) {
    let rate = 44100u32;
    let n = (seconds * rate as f64) as usize;
    let beat = rate as usize / 2;
    let samples: Vec<i16> = (0..n)
        .map(|i| {
            let t = i % beat;
            if t < 2000 {
                let env = 1.0 - t as f64 / 2000.0;
                let s = (i as f64 * 2.0 * std::f64::consts::PI * 80.0 / rate as f64).sin();
                (s * env * 30000.0) as i16
            } else {
                0
            }
        })
        .collect();
    let mut wav = Vec::new();
    let data = (samples.len() * 2) as u32;
    wav.extend(b"RIFF");
    wav.extend((36 + data).to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16u32.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(1u16.to_le_bytes());
    wav.extend(rate.to_le_bytes());
    wav.extend((rate * 2).to_le_bytes());
    wav.extend(2u16.to_le_bytes());
    wav.extend(16u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend(data.to_le_bytes());
    for s in samples {
        wav.extend(s.to_le_bytes());
    }
    fs::write(path, wav).unwrap();
}

fn hotcue(number: usize, seconds: f64) -> Hotcue {
    Hotcue {
        number,
        seconds,
        name: format!("cue {number}"),
        end: None,
        color: Some(0xE62828),
    }
}

#[test]
fn an_export_reads_back_in_both_formats() {
    let music = tempfile::tempdir().unwrap();
    let stick = tempfile::tempdir().unwrap();
    // The analysis cache lives beside the music, not in the real one.
    std::env::set_var("DJSUGAR_CACHE", music.path().join("cache"));

    let mut tracks = Vec::new();
    for (i, (artist, title)) in [("Ana", "One"), ("Ben", "Two"), ("Ana", "Three")]
        .iter()
        .enumerate()
    {
        let path = music.path().join(format!("{title}.wav"));
        click_track(&path, 6.0);
        let mut t = scan::read_track(i + 10, &path);
        t.artist = artist.to_string();
        t.title = title.to_string();
        t.album = "Clicks".into();
        t.key = "Am".into();
        t.info.hotcues = (0..10).map(|n| hotcue(n, 0.5 + n as f64 * 0.5)).collect();
        tracks.push(t);
    }
    let mut gone = scan::read_track(99, &music.path().join("gone.wav"));
    gone.title = "Gone".into();
    tracks.push(gone);

    let lists = vec![
        List {
            name: "Gigs".into(),
            folder: true,
            ..List::default()
        },
        List {
            name: "Friday".into(),
            parent: Some(0),
            tracks: vec![0, 1, 0, 3],
            ..List::default()
        },
        List {
            name: "Warm-up".into(),
            tracks: vec![2, 1],
            ..List::default()
        },
    ];
    let options = Options {
        root: stick.path().to_path_buf(),
        name: "TEST".into(),
        device_library: true,
        one_library: true,
        engine_dj: false,
        camelot: true,
    };
    let mut steps = Vec::new();
    let report = export(&tracks, &lists, &options, &mut |s| {
        steps.push(s);
        true
    })
    .unwrap();
    assert_eq!(report.tracks, 3);
    assert_eq!(report.copied, 3);
    assert_eq!(report.lists, 2);
    assert_eq!(report.skipped.len(), 1, "{:?}", report.skipped);
    assert_eq!(report.hot_cues_dropped, 3 * 2, "cues I and J on each track");
    assert_eq!(steps.last(), Some(&Step::Library));

    let rekordbox = stick.path().join("PIONEER/rekordbox");
    let mut pdb = DeviceLibrary::new(&rekordbox).unwrap();
    let contents = pdb.get_contents().unwrap();
    let titles: Vec<_> = contents.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(titles, ["One", "Two", "Three"]);
    let one = &contents[0];
    assert_eq!(one.master_content_id, 10);
    assert!(
        (11900..=12100).contains(&one.bpmx100),
        "bpm {}",
        one.bpmx100
    );
    assert_eq!(pdb.get_key_by_id(one.key_id).unwrap().unwrap().name, "8A");
    assert_eq!(
        pdb.get_artist_by_id(one.artist_id).unwrap().unwrap().name,
        "Ana"
    );
    assert_eq!(contents[2].artist_id, one.artist_id);
    assert_eq!(one.hot_cue_auto_load, "ON");
    let playlists = pdb.get_playlists().unwrap();
    let friday = playlists.iter().find(|p| p.name == "Friday").unwrap();
    let gigs = playlists.iter().find(|p| p.name == "Gigs").unwrap();
    assert_eq!(friday.parent_id, gigs.id);
    let in_friday: Vec<_> = pdb
        .get_playlist_contents(friday.id)
        .unwrap()
        .into_iter()
        .map(|c| c.title)
        .collect();
    assert_eq!(in_friday, ["One", "Two"]);

    let one_lib = OneLibrary::new(&rekordbox).unwrap();
    let contents = one_lib.get_contents().unwrap();
    assert_eq!(contents.len(), 3);
    assert_eq!(contents[1].title.as_deref(), Some("Two"));
    let playlists = one_lib.get_playlists().unwrap();
    let warm = playlists.iter().find(|p| p.name == "Warm-up").unwrap();
    let in_warm: Vec<_> = one_lib
        .get_playlist_contents(warm.id)
        .unwrap()
        .into_iter()
        .map(|c| c.title.unwrap_or_default())
        .collect();
    assert_eq!(in_warm, ["Three", "Two"]);

    // Audio, analysis files and the paths the databases point at line up.
    for c in pdb.get_contents().unwrap() {
        let path = pdb.get_content_by_id(c.id).unwrap().unwrap().path;
        assert!(on_stick(stick.path(), &path).is_file(), "{path}");
        let anlz = on_stick(stick.path(), &c.analysis_path);
        assert!(anlz.is_file(), "{}", c.analysis_path);
    }
    let audio = stick.path().join("Contents/Ana/Clicks/One.wav");
    assert!(audio.is_file());
    let anlz: Vec<_> = walk(&stick.path().join("PIONEER/USBANLZ"));
    assert_eq!(anlz.len(), 9, "{anlz:?}");

    // Windows won't replace files that are still open.
    drop((pdb, one_lib));

    // A second export keeps the audio and replaces the library.
    let lists = vec![List {
        name: "Only".into(),
        tracks: vec![1],
        ..List::default()
    }];
    let options = Options {
        one_library: false,
        ..options
    };
    let report = export(&tracks, &lists, &options, &mut |_| true).unwrap();
    assert_eq!((report.tracks, report.copied), (1, 0));
    assert!(!rekordbox.join("exportLibrary.db").exists());
    let mut pdb = DeviceLibrary::new(&rekordbox).unwrap();
    assert_eq!(pdb.get_contents().unwrap().len(), 1);
    drop(pdb);

    // Nothing to export leaves the stick alone.
    let empty = tempfile::tempdir().unwrap();
    let only_gone = vec![List {
        name: "Gone".into(),
        tracks: vec![3],
        ..List::default()
    }];
    let options_empty = Options {
        root: empty.path().to_path_buf(),
        ..options.clone()
    };
    assert!(export(&tracks, &only_gone, &options_empty, &mut |_| true).is_err());
    assert!(!empty.path().join("PIONEER/rekordbox").exists());

    // A stick rekordbox exported hidden, as .PIONEER, is written there.
    let hidden = tempfile::tempdir().unwrap();
    fs::create_dir_all(hidden.path().join(".PIONEER/rekordbox")).unwrap();
    fs::write(hidden.path().join(".PIONEER/rekordbox/export.pdb"), b"old").unwrap();
    let options_hidden = Options {
        root: hidden.path().to_path_buf(),
        ..options.clone()
    };
    export(&tracks, &lists, &options_hidden, &mut |_| true).unwrap();
    assert!(!hidden.path().join("PIONEER").exists());
    let mut pdb = DeviceLibrary::new(hidden.path().join(".PIONEER/rekordbox")).unwrap();
    let c = &pdb.get_contents().unwrap()[0];
    assert!(
        c.analysis_path.starts_with("/.PIONEER/USBANLZ/"),
        "{}",
        c.analysis_path
    );
    assert!(on_stick(hidden.path(), &c.analysis_path).is_file());

    // Stopping leaves the library as it was.
    let err = export(&tracks, &lists, &options, &mut |_| false).unwrap_err();
    assert!(matches!(err, Error::Cancelled));
}

#[test]
fn an_engine_dj_export_reads_back() {
    use rusqlite::Connection;
    let music = tempfile::tempdir().unwrap();
    let stick = tempfile::tempdir().unwrap();
    std::env::set_var("DJSUGAR_CACHE", music.path().join("cache"));

    let mut tracks = Vec::new();
    for (i, title) in ["One", "Two"].iter().enumerate() {
        let path = music.path().join(format!("{title}.wav"));
        click_track(&path, 6.0);
        let mut t = scan::read_track(i + 1, &path);
        t.artist = "Ana".into();
        t.title = title.to_string();
        t.album = "Clicks".into();
        t.key = "Am".into();
        t.rating = 4;
        t.info.hotcues = vec![hotcue(0, 0.5), hotcue(9, 1.0)];
        tracks.push(t);
    }
    let lists = vec![
        List {
            name: "Gigs".into(),
            folder: true,
            ..List::default()
        },
        List {
            name: "Friday".into(),
            parent: Some(0),
            tracks: vec![1, 0],
            ..List::default()
        },
        List {
            name: "Friday".into(),
            parent: Some(0),
            tracks: vec![0],
            ..List::default()
        },
    ];
    let options = Options {
        root: stick.path().to_path_buf(),
        name: "TEST".into(),
        device_library: false,
        one_library: false,
        engine_dj: true,
        camelot: true,
    };
    let report = export(&tracks, &lists, &options, &mut |_| true).unwrap();
    assert_eq!((report.tracks, report.lists), (2, 2));
    assert!(!stick.path().join("PIONEER").exists(), "no Pioneer files");

    let db = Connection::open(stick.path().join("Engine Library/Database2/m.db")).unwrap();
    let version: (i64, i64, i64) = db
        .query_row(
            "SELECT schemaVersionMajor, schemaVersionMinor, schemaVersionPatch FROM Information",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(version, (3, 0, 2));
    let rows: Vec<(String, String, i64, i64, i64, bool)> = db
        .prepare("SELECT title, path, key, rating, bpm, isAnalyzed FROM Track ORDER BY id")
        .unwrap()
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(rows.len(), 2);
    // Tracks come in the order the lists first hold them: Friday is Two, One.
    let (title, path, key, rating, bpm, analysed) = &rows[1];
    assert_eq!(title, "One");
    assert_eq!(path, "../Contents/Ana/Clicks/One.wav");
    assert!(stick.path().join("Engine Library").join(path).is_file());
    assert_eq!((*key, *rating, *bpm), (1, 80, 120));
    assert!(*analysed, "a grid and a waveform are there");

    // The performance data decodes: grid from the first beat, cue A only.
    let (beats, cues): (Vec<u8>, Vec<u8>) = db
        .query_row(
            "SELECT beatData, quickCues FROM PerformanceData WHERE trackId = 2",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let beats = engine::perf::tests::unq(&beats);
    assert_eq!(f64::from_be_bytes(beats[..8].try_into().unwrap()), 44100.0);
    assert!(u64::from_be_bytes(beats[17..25].try_into().unwrap()) >= 2);
    let cues = engine::perf::tests::unq(&cues);
    assert_eq!(&cues[8..14], b"\x05cue 0");

    // Lists in their folder, in order, with unique names.
    let lists: Vec<(i64, String, i64)> = db
        .prepare("SELECT id, title, parentListId FROM Playlist ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let names: Vec<&str> = lists.iter().map(|l| l.1.as_str()).collect();
    assert_eq!(names, ["Gigs", "Friday", "Friday (2)"]);
    assert_eq!(lists[1].2, lists[0].0);
    let order: Vec<String> = db
        .prepare(
            "WITH RECURSIVE chain(id, trackId, nextEntityId) AS ( \
               SELECT id, trackId, nextEntityId FROM PlaylistEntity \
                 WHERE listId = ?1 AND id NOT IN \
                   (SELECT nextEntityId FROM PlaylistEntity WHERE listId = ?1) \
               UNION ALL SELECT e.id, e.trackId, e.nextEntityId FROM PlaylistEntity e \
                 JOIN chain c ON e.id = c.nextEntityId) \
             SELECT t.title FROM chain JOIN Track t ON t.id = chain.trackId",
        )
        .unwrap()
        .query_map([lists[1].0], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(order, ["Two", "One"]);
    drop(db);

    // Exporting again replaces DJ Sugar's own library.
    export(&tracks, &lists_one(), &options, &mut |_| true).unwrap();
    let db = Connection::open(stick.path().join("Engine Library/Database2/m.db")).unwrap();
    let n: i64 = db
        .query_row("SELECT count(*) FROM Track", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
    drop(db);

    // A library Engine DJ wrote is left alone.
    let other = tempfile::tempdir().unwrap();
    let db2 = other.path().join("Engine Library/Database2");
    fs::create_dir_all(&db2).unwrap();
    fs::write(db2.join("m.db"), b"engine's").unwrap();
    let options_other = Options {
        root: other.path().to_path_buf(),
        ..options.clone()
    };
    assert!(export(&tracks, &lists_one(), &options_other, &mut |_| true).is_err());
    assert_eq!(fs::read(db2.join("m.db")).unwrap(), b"engine's");
}

#[test]
fn a_second_export_removes_only_what_the_first_put_there() {
    let music = tempfile::tempdir().unwrap();
    let stick = tempfile::tempdir().unwrap();
    std::env::set_var("DJSUGAR_CACHE", music.path().join("cache"));
    let tracks: Vec<Track> = ["Stays", "Goes"]
        .iter()
        .enumerate()
        .map(|(i, title)| {
            let path = music.path().join(format!("{title}.wav"));
            click_track(&path, 2.0);
            let mut t = scan::read_track(i + 1, &path);
            t.title = title.to_string();
            t
        })
        .collect();
    let options = Options {
        root: stick.path().to_path_buf(),
        name: "TEST".into(),
        device_library: true,
        one_library: false,
        engine_dj: false,
        camelot: false,
    };
    let list = |tracks| {
        vec![List {
            name: "Set".into(),
            tracks,
            ..List::default()
        }]
    };
    export(&tracks, &list(vec![0, 1]), &options, &mut |_| true).unwrap();
    let mine = stick.path().join("Contents/mine.mp3");
    std::fs::write(&mine, b"the DJ's own").unwrap();
    let audio = |title: &str| {
        walk(stick.path()).into_iter().any(|p| {
            p.file_name()
                .is_some_and(|n| n == format!("{title}.wav").as_str())
        })
    };
    let analysis_files = || {
        walk(stick.path())
            .iter()
            .filter(|p| p.to_string_lossy().contains("USBANLZ"))
            .count()
    };
    let before = analysis_files();
    assert!(audio("Stays") && audio("Goes"));

    let report = export(&tracks, &list(vec![0]), &options, &mut |_| true).unwrap();
    assert_eq!(report.copied, 0, "audio already there isn't copied again");
    assert!(report.removed > 0);
    assert!(audio("Stays") && !audio("Goes"));
    assert!(analysis_files() < before);
    assert!(mine.exists(), "the DJ's own files stay");
}

fn lists_one() -> Vec<List> {
    vec![List {
        name: "Only".into(),
        tracks: vec![0],
        ..List::default()
    }]
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The same kind of stick read back by `check`, a reader that shares no code
// with rbox (ported from EngineX OS, whose CDJ-3000 simulator loads sticks
// with it), so a mistake rbox makes both ways still shows.

/// The analysis of one track, from its `.DAT`, `.EXT` and `.2EX` together,
/// and the audio path each file names.
fn analysis_of(stick: &Path, t: &check::Track) -> (check::anlz::Analysis, Vec<String>) {
    let stem = t
        .analysis_path
        .strip_suffix(".DAT")
        .unwrap_or_else(|| panic!("{}", t.analysis_path));
    let mut all = check::anlz::Analysis::default();
    let mut paths = Vec::new();
    for ext in ["DAT", "EXT", "2EX"] {
        let file = on_stick(stick, &format!("{stem}.{ext}"));
        let a = check::anlz::read(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        for s in a.unknown.get("PPTH").into_iter().flatten() {
            let len = u32::from_be_bytes(s[12..16].try_into().unwrap()) as usize;
            let units: Vec<u16> = s[16..16 + len]
                .chunks(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            paths.push(
                String::from_utf16(&units)
                    .unwrap()
                    .trim_end_matches('\0')
                    .to_string(),
            );
        }
        all.merge(a);
    }
    (all, paths)
}

fn source(t: &Track) -> &Path {
    match &t.source {
        Source::File(p) => p,
        _ => panic!("not a file"),
    }
}

/// A square JPEG, as a folder's cover art.
fn cover(path: &Path) {
    let img = image::RgbImage::from_pixel(300, 300, image::Rgb([0xb6, 0x9c, 0xff]));
    img.save(path).unwrap();
}

fn memory(n: usize, seconds: f64, end: Option<f64>, color: Option<u32>) -> Hotcue {
    Hotcue {
        number: djsugar_engine::track::MEMORY_CUE + n,
        seconds,
        name: String::new(),
        end,
        color,
    }
}

#[test]
fn a_full_stick_reads_back_in_an_independent_reader() {
    let music = tempfile::tempdir().unwrap();
    let stick = tempfile::tempdir().unwrap();
    std::env::set_var("DJSUGAR_CACHE", music.path().join("cache"));

    // Many tracks of one file (analysed once) fill several pages and row
    // groups of export.pdb; one more has folder art.
    let click = music.path().join("click.wav");
    click_track(&click, 4.0);
    let art_dir = music.path().join("art");
    fs::create_dir_all(&art_dir).unwrap();
    let art_song = art_dir.join("With Art.wav");
    click_track(&art_song, 4.0);
    cover(&art_dir.join("cover.jpg"));

    let artists = ["Ana", "Ñandú Ensemble", "東京 Collective", "Ben"];
    let long_title = format!("Long {}", "a".repeat(150));
    let mut tracks = Vec::new();
    for i in 0..30 {
        let mut t = scan::read_track(100 + i, &click);
        t.artist = artists[i % artists.len()].to_string();
        t.title = match i {
            0 => "Café del Mar ✓".to_string(),
            1 => long_title.clone(),
            _ => format!("Track {i:02}"),
        };
        t.album = format!("Album {}", i % 3);
        t.key = "Am".into();
        tracks.push(t);
    }
    {
        let t = &mut tracks[2];
        t.genre = "House".into();
        t.rating = 4;
        t.info.label = "Lilac Records".into();
        t.info.remixer = "Ben".into();
        t.info.composer = "Cleo".into();
        t.info.remix = "Club Mix".into();
        t.info.comment = "warm-up opener".into();
        t.info.year = "2024".into();
        t.info.color = Some(0xFF0000);
    }
    tracks[0].info.hotcues = vec![
        Hotcue {
            name: "Drop".into(),
            ..hotcue(0, 0.5)
        },
        Hotcue {
            name: "Loop".into(),
            end: Some(2.0),
            color: Some(0x305AFF),
            ..hotcue(1, 1.0)
        },
        hotcue(4, 2.5),
        memory(0, 3.0, None, None),
        memory(1, 1.0, Some(1.5), Some(0x00FF00)),
    ];
    let mut art = scan::read_track(200, &art_song);
    art.artist = "Ana".into();
    art.title = "With Art".into();
    art.album = "Art".into();
    art.key = "Am".into();
    tracks.push(art);

    let mut lists = vec![
        List {
            name: "Gigs".into(),
            folder: true,
            ..List::default()
        },
        List {
            name: "2026".into(),
            folder: true,
            parent: Some(0),
            ..List::default()
        },
        List {
            name: "Friday".into(),
            parent: Some(1),
            tracks: (0..20).collect(),
            ..List::default()
        },
        List {
            name: "Domingo Ñ".into(),
            parent: Some(1),
            tracks: (0..31).rev().collect(),
            ..List::default()
        },
    ];
    for n in 0..25 {
        lists.push(List {
            name: format!("List {n:02}"),
            tracks: vec![n, 30],
            ..List::default()
        });
    }
    let options = Options {
        root: stick.path().to_path_buf(),
        name: "TEST".into(),
        device_library: true,
        one_library: true,
        engine_dj: false,
        camelot: false,
    };
    let report = export(&tracks, &lists, &options, &mut |_| true).unwrap();
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert_eq!(report.tracks, 31);

    let rekordbox = stick.path().join("PIONEER/rekordbox");
    let pdb = check::pdb::read(&rekordbox.join("export.pdb")).unwrap();
    let raw = fs::read(rekordbox.join("export.pdb")).unwrap();
    check::pdb::strict(&raw).unwrap();
    check::pdb::track_strings_set(&raw).unwrap();
    let one = check::onelib::read(&rekordbox.join("exportLibrary.db")).unwrap();

    // Tracks get ids in the order the lists first hold them: Friday's,
    // then the rest of Domingo's.
    let ids: Vec<u32> = pdb.tracks.iter().map(|t| t.id).collect();
    assert_eq!(ids, (1..=31).collect::<Vec<_>>());
    let order: Vec<usize> = (0..20).chain((20..31).rev()).collect();
    for (t, &i) in pdb.tracks.iter().zip(&order) {
        let want = &tracks[i];
        assert_eq!(t.title, want.title, "track {}", t.id);
        assert_eq!(t.artist, want.artist, "track {}", t.id);
        assert_eq!(t.album, want.album, "track {}", t.id);
        assert_eq!(t.key, "Am", "track {}", t.id);
        assert!((t.bpm - 120.0).abs() < 1.0, "track {}: bpm {}", t.id, t.bpm);
        assert_eq!(t.duration, 4.0, "track {}", t.id);
        assert_eq!((t.sample_rate, t.bit_depth), (44100, 16), "track {}", t.id);
        assert_eq!(t.file_size, fs::metadata(source(want)).unwrap().len());
        let audio = on_stick(stick.path(), &t.path);
        assert_eq!(
            fs::metadata(&audio).unwrap().len(),
            t.file_size,
            "{}",
            t.path
        );
    }
    let t = &pdb.tracks[2];
    assert_eq!(
        (
            t.genre.as_str(),
            t.label.as_str(),
            t.remixer.as_str(),
            t.composer.as_str(),
            t.comment.as_str(),
            t.year,
            t.rating
        ),
        (
            "House",
            "Lilac Records",
            "Ben",
            "Cleo",
            "warm-up opener",
            2024,
            4
        )
    );

    // Art: the track with a cover names it in both databases, the others
    // name none.
    for lib in [&pdb, &one] {
        let with_art: Vec<_> = lib
            .tracks
            .iter()
            .filter(|t| !t.artwork_path.is_empty())
            .collect();
        assert_eq!(with_art.len(), 1);
        assert_eq!(with_art[0].title, "With Art");
        let file = on_stick(stick.path(), &with_art[0].artwork_path);
        assert!(file.is_file(), "{}", file.display());
    }

    // OneLibrary holds the same tracks and lists under the same ids.
    let no_art = |lib: &check::Library| -> Vec<check::Track> {
        lib.tracks
            .iter()
            .map(|t| check::Track {
                artwork_path: String::new(),
                ..t.clone()
            })
            .collect()
    };
    assert_eq!(no_art(&one), no_art(&pdb));
    assert_eq!(one.playlists, pdb.playlists);

    // The tree and its order.
    for lib in [&pdb, &one] {
        let gigs = lib.playlist("Gigs");
        let year = lib.playlist("2026");
        let friday = lib.playlist("Friday");
        let sunday = lib.playlist("Domingo Ñ");
        assert!(gigs.folder && year.folder && !friday.folder);
        assert_eq!(gigs.parent_id, 0);
        assert_eq!(year.parent_id, gigs.id);
        assert_eq!((friday.parent_id, sunday.parent_id), (year.id, year.id));
        assert!(friday.sort_order < sunday.sort_order);
        let want: Vec<String> = tracks[..20].iter().map(|t| t.title.clone()).collect();
        assert_eq!(lib.titles("Friday"), want);
        let want: Vec<String> = tracks.iter().rev().map(|t| t.title.clone()).collect();
        assert_eq!(lib.titles("Domingo Ñ"), want);
        assert_eq!(lib.titles("List 07"), ["Track 07", "With Art"]);
        let top: Vec<&str> = lib
            .playlists
            .iter()
            .filter(|p| p.parent_id == 0)
            .map(|p| p.name.as_str())
            .collect();
        let mut want = vec!["Gigs".to_string()];
        want.extend((0..25).map(|n| format!("List {n:02}")));
        assert_eq!(top, want);
    }

    // Every track's analysis: grid, waveforms at the players' rates, and the
    // audio path, read as the CDJ-3000 simulator loads them.
    for t in &pdb.tracks {
        let (a, paths) = analysis_of(stick.path(), t);
        assert_eq!(paths, [t.path.as_str(); 3], "track {}", t.id);
        assert!(
            a.beats.len() >= 7,
            "track {}: {} beats",
            t.id,
            a.beats.len()
        );
        for (i, b) in a.beats.iter().enumerate() {
            assert!(
                (b.bpm - 120.0).abs() < 1.0,
                "track {} beat {i}: {b:?}",
                t.id
            );
            assert!(
                (1..=4).contains(&b.number),
                "track {} beat {i}: {b:?}",
                t.id
            );
            if i > 0 {
                let before = a.beats[i - 1];
                assert!(b.time_ms > before.time_ms, "track {} beat {i}", t.id);
                assert_eq!(b.number, before.number % 4 + 1, "track {} beat {i}", t.id);
            }
        }
        assert!(a.beats.last().unwrap().time_ms < 4000);
        let entries = |tag: &str| {
            a.waveform(tag)
                .unwrap_or_else(|| panic!("track {}: no {tag}", t.id))
                .entries()
        };
        assert_eq!(entries("PWAV"), 400, "track {}", t.id);
        assert_eq!(entries("PWV2"), 100, "track {}", t.id);
        assert_eq!(entries("PWV4"), 1200, "track {}", t.id);
        for tag in ["PWV3", "PWV5", "PWV7"] {
            // 150 columns a second.
            assert_eq!(entries(tag), 600, "track {} {tag}", t.id);
        }
    }

    // Hot cues A to H with names, colours and loops; memory cues too.
    let (a, _) = analysis_of(stick.path(), &pdb.tracks[0]);
    let drop = a.hot_cue(1).unwrap();
    assert_eq!((drop.time_ms, drop.is_loop), (500, false));
    assert_eq!(drop.name, "Drop");
    assert_eq!((drop.color_code, drop.color), (0x2a, [255, 0xff, 0, 0]));
    let lp = a.hot_cue(2).unwrap();
    assert!(lp.is_loop && lp.extended);
    assert_eq!((lp.time_ms, lp.end_ms), (1000, 2000));
    assert_eq!((lp.loop_numerator, lp.loop_denominator), (2, 1));
    assert_eq!(lp.color_code, 0x01);
    assert_eq!(a.hot_cue(5).unwrap().time_ms, 2500);
    let memories: Vec<_> = a.cues.iter().filter(|c| c.hot_cue == 0).collect();
    assert_eq!(memories.len(), 2, "{memories:?}");
    let point = memories.iter().find(|c| !c.is_loop).unwrap();
    assert_eq!(point.time_ms, 3000);
    let mem_loop = memories.iter().find(|c| c.is_loop).unwrap();
    assert_eq!((mem_loop.time_ms, mem_loop.end_ms), (1000, 1500));
    assert_eq!(mem_loop.color_id, 5, "green");
    assert_eq!(a.cues.len(), 5);
}

#[test]
fn the_independent_reader_refuses_broken_files() {
    let music = tempfile::tempdir().unwrap();
    let stick = tempfile::tempdir().unwrap();
    std::env::set_var("DJSUGAR_CACHE", music.path().join("cache"));
    let path = music.path().join("One.wav");
    click_track(&path, 3.0);
    let mut t = scan::read_track(1, &path);
    t.title = "One".into();
    let lists = vec![List {
        name: "L".into(),
        tracks: vec![0],
        ..List::default()
    }];
    let options = Options {
        root: stick.path().to_path_buf(),
        name: "TEST".into(),
        device_library: true,
        one_library: false,
        engine_dj: false,
        camelot: true,
    };
    export(&[t], &lists, &options, &mut |_| true).unwrap();
    let pdb = fs::read(stick.path().join("PIONEER/rekordbox/export.pdb")).unwrap();
    assert!(check::pdb::parse(&pdb).is_ok());
    // Cut short, or a page that says it belongs to another table.
    assert!(check::pdb::parse(&pdb[..pdb.len() - 100]).is_err());
    let mut wrong = pdb.clone();
    let first_track_page = u32::from_le_bytes(pdb[28 + 8..28 + 12].try_into().unwrap()) as usize;
    wrong[first_track_page * 4096 + 8] = 9;
    assert!(check::pdb::parse(&wrong).is_err());

    let lib = check::pdb::parse(&pdb).unwrap();
    let dat = on_stick(stick.path(), &lib.tracks[0].analysis_path);
    let anlz = fs::read(&dat).unwrap();
    assert!(check::anlz::parse(&anlz).is_ok());
    // A section that runs past the end of the file.
    let mut long = anlz.clone();
    let first = u32::from_be_bytes(anlz[4..8].try_into().unwrap()) as usize;
    long[first + 8..first + 12].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(check::anlz::parse(&long).is_err());
}

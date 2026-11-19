//! Traktor's history: a `History` folder beside `collection.nml` with one
//! NML per session, `history_2023y01m05d_21h30m12s.nml` for one begun then.
//! Each has the tracks played in its own `COLLECTION` and a playlist of
//! them in order, every `ENTRY` with an `EXTENDEDDATA` saying when it
//! started: `STARTDATE` packs the day as year << 16 | month << 8 | day,
//! and `STARTTIME` is seconds since that midnight, on the DJ's clock.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDate, TimeZone};
use djsugar_engine::import::{ImportedEntry, ImportedListKind};
use roxmltree::Node;

use super::{child, collection, document, load, location};
use crate::ImportedList;

/// The session files, oldest first.
pub fn files(collection: &Path) -> Vec<PathBuf> {
    let dir = collection.parent().unwrap_or(Path::new("")).join("History");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|e| e.eq_ignore_ascii_case("nml"))
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("history_"))
        })
        .collect();
    files.sort();
    files
}

/// A local day and second of it as a Unix time.
fn local(y: i32, m: u32, d: u32, seconds: u32) -> Option<i64> {
    let at = NaiveDate::from_ymd_opt(y, m, d)?.and_hms_opt(0, 0, 0)?
        + chrono::Duration::seconds(i64::from(seconds));
    Some(Local.from_local_datetime(&at).earliest()?.timestamp())
}

/// "history_2023y01m05d_21h30m12s" as when it began.
fn began(stem: &str) -> Option<i64> {
    let digits: Vec<u32> = stem
        .strip_prefix("history_")?
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().ok())
        .collect::<Option<_>>()?;
    match digits[..] {
        [y, mo, d, h, mi, s, ..] => local(y as i32, mo, d, h * 3600 + mi * 60 + s),
        [y, mo, d, ..] => local(y as i32, mo, d, 0),
        _ => None,
    }
}

/// When an entry started, from its `EXTENDEDDATA`.
fn started(entry: Node) -> Option<i64> {
    let x = child(entry, "EXTENDEDDATA")?;
    let date: u32 = x.attribute("STARTDATE")?.trim().parse().ok()?;
    let time: u32 = x.attribute("STARTTIME")?.trim().parse().ok()?;
    local((date >> 16) as i32, (date >> 8) & 0xff, date & 0xff, time)
}

/// A session, if any track was played in it.
pub fn session(path: &Path, exists: &impl Fn(&Path) -> bool) -> Option<ImportedList> {
    let text = load(path).ok()?;
    let doc = document(&text).ok()?;
    let files: HashMap<String, PathBuf> = collection(&doc)
        .filter_map(|n| location(n, exists))
        .collect();
    let entries: Vec<ImportedEntry> = doc
        .descendants()
        .filter(|n| n.has_tag_name("PLAYLIST"))
        .flat_map(|p| p.children().filter(|e| e.has_tag_name("ENTRY")))
        .filter_map(|e| {
            let key = child(e, "PRIMARYKEY")?.attribute("KEY")?;
            let file = files.get(key)?.clone();
            Some(ImportedEntry::new(file, started(e)))
        })
        .collect();
    if entries.is_empty() {
        return None;
    }
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    let start = entries
        .iter()
        .filter_map(|e| e.played_at)
        .min()
        .or_else(|| began(&stem))
        .unwrap_or(0);
    let name = chrono::DateTime::from_timestamp(start, 0)
        .filter(|_| start > 0)
        .map(|t| t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or(stem);
    Some(ImportedList {
        folders: Vec::new(),
        name,
        kind: ImportedListKind::History { started: start },
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SESSION: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="no" ?>
<NML VERSION="19"><HEAD COMPANY="www.native-instruments.com" PROGRAM="Traktor"></HEAD>
<COLLECTION ENTRIES="2">
<ENTRY TITLE="Glue" ARTIST="Bicep">
<LOCATION DIR="/:Music/:" FILE="glue.mp3" VOLUME="C:"></LOCATION>
</ENTRY>
<ENTRY TITLE="Other" ARTIST="X">
<LOCATION DIR="/:Music/:" FILE="other.mp3" VOLUME="C:"></LOCATION>
</ENTRY>
</COLLECTION>
<PLAYLISTS><NODE TYPE="FOLDER" NAME="$ROOT"><SUBNODES COUNT="1">
<NODE TYPE="PLAYLIST" NAME="HistoryPlaylist"><PLAYLIST ENTRIES="2" TYPE="LIST" UUID="x">
<ENTRY><PRIMARYKEY TYPE="TRACK" KEY="C:/:Music/:other.mp3"></PRIMARYKEY>
<EXTENDEDDATA DECK="1" DURATION="300.0" EXTENDEDTYPE="HistoryData" PLAYEDPUBLIC="1" STARTDATE="132579589" STARTTIME="77500"></EXTENDEDDATA></ENTRY>
<ENTRY><PRIMARYKEY TYPE="TRACK" KEY="C:/:Music/:glue.mp3"></PRIMARYKEY>
<EXTENDEDDATA DECK="0" DURATION="250.0" EXTENDEDTYPE="HistoryData" PLAYEDPUBLIC="1" STARTDATE="132579589" STARTTIME="77800"></EXTENDEDDATA></ENTRY>
</PLAYLIST></NODE></SUBNODES></NODE></PLAYLISTS></NML>"#;

    #[test]
    fn reads_a_session() {
        let dir = std::env::temp_dir().join(format!("djs-traktor-h-{}", std::process::id()));
        let history = dir.join("History");
        std::fs::create_dir_all(&history).unwrap();
        let file = history.join("history_2023y01m05d_21h30m12s.nml");
        std::fs::write(&file, SESSION).unwrap();
        std::fs::write(history.join("notes.txt"), "x").unwrap();
        let found = files(&dir.join("collection.nml"));
        let list = session(&file, &|_: &Path| true).unwrap();
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(found, [file]);
        // 132579589 is 2023 << 16 | 1 << 8 | 5; 77500 s is 21:31:40.
        let first = local(2023, 1, 5, 77_500).unwrap();
        assert_eq!(list.kind, ImportedListKind::History { started: first });
        assert_eq!(list.name, "2023-01-05 21:31");
        let got: Vec<(PathBuf, Option<i64>)> = list
            .entries
            .iter()
            .map(|e| (e.path.clone(), e.played_at))
            .collect();
        assert_eq!(
            got,
            [
                (PathBuf::from("C:/Music/other.mp3"), Some(first)),
                (PathBuf::from("C:/Music/glue.mp3"), Some(first + 300)),
            ]
        );
        assert_eq!(
            began("history_2023y01m05d_21h30m12s"),
            local(2023, 1, 5, 21 * 3600 + 30 * 60 + 12)
        );
    }
}

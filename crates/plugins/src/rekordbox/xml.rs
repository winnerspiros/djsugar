//! A rekordbox XML export (File › Export Collection in xml format), for
//! rekordbox 5 and when `master.db` can't be read: tracks with their tags,
//! beat grid (`TEMPO`), cues and loops (`POSITION_MARK`), and the playlist
//! tree. It has no history, intelligent playlists or phrases.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use djsugar_engine::beats::{BeatGrid, GridMarker};
use djsugar_engine::import::{ImportedEntry, ImportedList, ImportedListKind, StreamedTrack};
use djsugar_engine::{Hotcue, LibraryInfo, Source, Track, MEMORY_CUE};
use roxmltree::{Document, Node};

use super::PLAYLISTS;
use crate::ImportedLibrary;

fn parse(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("can't read {}: {e}", path.display()))
}

fn document(text: &str) -> Result<Document<'_>, String> {
    let doc = Document::parse(text).map_err(|e| e.to_string())?;
    if !doc.root_element().has_tag_name("DJ_PLAYLISTS") {
        return Err("not a rekordbox XML file".into());
    }
    Ok(doc)
}

pub fn preview(path: &Path) -> Result<String, String> {
    let text = parse(path)?;
    let doc = document(&text)?;
    let tracks = collection(&doc).count();
    let lists = playlists(&doc, &HashMap::new());
    Ok(super::summary(tracks, lists.len(), 0, 0))
}

pub fn read(
    path: &Path,
    groups: &[&str],
    exists: impl Fn(&Path) -> bool,
) -> Result<ImportedLibrary, String> {
    let text = parse(path)?;
    let doc = document(&text)?;
    let mut ids = HashMap::new();
    let mut tracks = Vec::new();
    for n in collection(&doc) {
        let Some(entry) = entry(n) else {
            continue;
        };
        if let Some(id) = n.attribute("TrackID") {
            ids.insert(id.to_string(), entry.clone());
        }
        if entry.streamed.is_none() && exists(&entry.path) {
            tracks.push(track(n, entry.path));
        }
    }
    let lists = if groups.contains(&PLAYLISTS) {
        playlists(&doc, &ids)
    } else {
        Vec::new()
    };
    Ok(ImportedLibrary {
        tracks,
        lists,
        written: None,
    })
}

pub fn lists(path: &Path, groups: &[&str]) -> Result<Vec<ImportedList>, String> {
    Ok(read(path, groups, |_| false)?.lists)
}

fn collection<'a>(doc: &'a Document) -> impl Iterator<Item = Node<'a, 'a>> {
    doc.root_element()
        .children()
        .filter(|n| n.has_tag_name("COLLECTION"))
        .flat_map(|c| c.children().filter(|n| n.has_tag_name("TRACK")))
}

/// A collection track as a list entry: its file, or the streaming
/// service's track its location names, with what the export knows of it.
fn entry(n: Node) -> Option<ImportedEntry> {
    let url = n.attribute("Location")?;
    let Some(mut s) =
        StreamedTrack::parse(url).or_else(|| StreamedTrack::parse(&percent_decode(url)))
    else {
        return location(url).map(|path| ImportedEntry::new(path, None));
    };
    s.artist = text(n, "Artist");
    s.title = text(n, "Name");
    s.duration = number(n, "TotalTime").unwrap_or(0.0);
    s.isrc = text(n, "ISRC");
    Some(ImportedEntry {
        path: PathBuf::from(&s.uri),
        played_at: None,
        streamed: Some(s),
    })
}

/// A file from a `file://localhost/…` location.
pub(crate) fn location(url: &str) -> Option<PathBuf> {
    let rest = url
        .strip_prefix("file://localhost")
        .or_else(|| url.strip_prefix("file://"))?;
    let path = percent_decode(rest);
    // Windows: /C:/Music/… is C:/Music/…
    let path = match path.as_bytes() {
        [b'/', d, b':', ..] if d.is_ascii_alphabetic() => path[1..].to_string(),
        _ => path,
    };
    (!path.is_empty()).then(|| PathBuf::from(path))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 3 <= bytes.len() {
            if let Some(b) = std::str::from_utf8(&bytes[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn text(n: Node, name: &str) -> String {
    n.attribute(name).unwrap_or_default().trim().to_string()
}

fn number(n: Node, name: &str) -> Option<f64> {
    n.attribute(name)?.trim().parse().ok()
}

fn track(n: Node, file: PathBuf) -> Track {
    let bpm = number(n, "AverageBpm").unwrap_or(0.0);
    let markers: Vec<GridMarker> = n
        .children()
        .filter(|c| c.has_tag_name("TEMPO"))
        .filter_map(|c| {
            Some(GridMarker {
                seconds: number(c, "Inizio")?,
                bpm: number(c, "Bpm")?,
            })
        })
        .collect();
    let mut hotcues = Vec::new();
    let mut memory = MEMORY_CUE;
    for m in n.children().filter(|c| c.has_tag_name("POSITION_MARK")) {
        let Some(seconds) = number(m, "Start") else {
            continue;
        };
        let end = number(m, "End").filter(|e| *e > seconds);
        let color = match (number(m, "Red"), number(m, "Green"), number(m, "Blue")) {
            (Some(r), Some(g), Some(b)) => Some(((r as u32) << 16) | ((g as u32) << 8) | b as u32),
            _ => None,
        };
        let num = number(m, "Num").unwrap_or(-1.0);
        let number = if num >= 0.0 {
            num as usize
        } else {
            memory += 1;
            memory - 1
        };
        if !hotcues.iter().any(|h: &Hotcue| h.number == number) {
            hotcues.push(Hotcue {
                number,
                seconds,
                name: text(m, "Name"),
                end,
                color,
            });
        }
    }
    hotcues.sort_by_key(|h| h.number);
    let mut t = Track {
        id: 0,
        artist: text(n, "Artist"),
        title: text(n, "Name"),
        album: text(n, "Album"),
        genre: text(n, "Genre"),
        // 0 to 255, 51 a star.
        rating: (number(n, "Rating").unwrap_or(0.0) / 51.0)
            .round()
            .clamp(0.0, 5.0) as u8,
        bpm,
        key: text(n, "Tonality"),
        duration: number(n, "TotalTime").unwrap_or(0.0),
        first_beat: markers.first().map_or(0.0, |m| m.seconds),
        grid: Default::default(),
        source: Source::File(file),
        waveform: Arc::default(),
        audio: None,
        cover: None,
        info: LibraryInfo {
            comment: text(n, "Comments"),
            grouping: text(n, "Grouping"),
            color: n
                .attribute("Colour")
                .and_then(|c| u32::from_str_radix(c.trim_start_matches("0x"), 16).ok()),
            play_count: number(n, "PlayCount").unwrap_or(0.0) as u32,
            hotcues,
            fixed_grid: !markers.is_empty(),
            year: Some(text(n, "Year"))
                .filter(|y| y != "0")
                .unwrap_or_default(),
            track_number: number(n, "TrackNumber").unwrap_or(0.0) as u32,
            disc_number: number(n, "DiscNumber").unwrap_or(0.0) as u32,
            label: text(n, "Label"),
            composer: text(n, "Composer"),
            remixer: text(n, "Remixer"),
            remix: text(n, "Mix"),
            added: n
                .attribute("DateAdded")
                .and_then(crate::virtualdj::lists::day_start)
                .unwrap_or(0),
            ..Default::default()
        },
    };
    if !markers.is_empty() {
        t.set_grid(BeatGrid::new(markers));
    }
    t
}

/// The playlist tree under `ROOT`: `Type="0"` is a folder, `1` a list whose
/// `TRACK`s name collection tracks by id (or by location, `KeyType="1"`).
fn playlists(doc: &Document, ids: &HashMap<String, ImportedEntry>) -> Vec<ImportedList> {
    let mut out = Vec::new();
    let Some(root) = doc
        .root_element()
        .children()
        .find(|n| n.has_tag_name("PLAYLISTS"))
        .and_then(|p| p.children().find(|n| n.has_tag_name("NODE")))
    else {
        return out;
    };
    walk(root, &mut Vec::new(), ids, &mut out);
    out
}

fn walk(
    node: Node,
    folders: &mut Vec<String>,
    ids: &HashMap<String, ImportedEntry>,
    out: &mut Vec<ImportedList>,
) {
    for n in node.children().filter(|n| n.has_tag_name("NODE")) {
        let name = text(n, "Name");
        match n.attribute("Type") {
            Some("0") => {
                folders.push(name);
                walk(n, folders, ids, out);
                folders.pop();
            }
            Some("1") => {
                let by_location = n.attribute("KeyType") == Some("1");
                let entries = n
                    .children()
                    .filter(|t| t.has_tag_name("TRACK"))
                    .filter_map(|t| {
                        let key = t.attribute("Key")?;
                        if by_location {
                            match StreamedTrack::parse(key) {
                                Some(_) => Some(ImportedEntry::new(key, None)),
                                None => location(key).map(|p| ImportedEntry::new(p, None)),
                            }
                        } else {
                            ids.get(key).cloned()
                        }
                    })
                    .collect();
                out.push(ImportedList {
                    folders: folders.clone(),
                    name,
                    kind: ImportedListKind::List,
                    entries,
                });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<DJ_PLAYLISTS Version="1.0.0">
  <PRODUCT Name="rekordbox" Version="6.6.2" Company="AlphaTheta"/>
  <COLLECTION Entries="2">
    <TRACK TrackID="1" Name="Glue" Artist="Bicep" Album="Bicep" Genre="Electronic"
           TotalTime="269" Year="2017" AverageBpm="120.00" DateAdded="2022-04-09"
           Comments="nice" PlayCount="7" Rating="204" Tonality="Fm" Label="Ninja Tune"
           Mix="Original Mix" Colour="0xFF007F"
           Location="file://localhost/Users/dj/Music/Bicep%20-%20Glue.mp3">
      <TEMPO Inizio="0.025" Bpm="120.00" Metro="4/4" Battito="1"/>
      <TEMPO Inizio="64.025" Bpm="121.00" Metro="4/4" Battito="1"/>
      <POSITION_MARK Name="Drop" Type="0" Start="32.025" Num="0" Red="230" Green="40" Blue="40"/>
      <POSITION_MARK Name="" Type="4" Start="64.025" End="72.025" Num="1" Red="40" Green="226" Blue="20"/>
      <POSITION_MARK Name="" Type="0" Start="0.025" Num="-1"/>
    </TRACK>
    <TRACK TrackID="2" Name="Other" Artist="X"
           Location="file://localhost/C:/Music/other.mp3"/>
  </COLLECTION>
  <PLAYLISTS>
    <NODE Type="0" Name="ROOT" Count="2">
      <NODE Type="0" Name="Sets" Count="1">
        <NODE Name="Friday" Type="1" KeyType="0" Entries="2">
          <TRACK Key="1"/>
          <TRACK Key="2"/>
        </NODE>
      </NODE>
      <NODE Name="By file" Type="1" KeyType="1" Entries="1">
        <TRACK Key="file://localhost/Users/dj/Music/Bicep%20-%20Glue.mp3"/>
      </NODE>
    </NODE>
  </PLAYLISTS>
</DJ_PLAYLISTS>"#;

    #[test]
    fn reads_a_rekordbox_xml() {
        let path = std::env::temp_dir().join(format!("djsugar-rb-{}.xml", std::process::id()));
        std::fs::write(&path, XML).unwrap();
        let lib = read(&path, &[PLAYLISTS], |p| p.starts_with("/Users")).unwrap();
        assert_eq!(lib.tracks.len(), 1);
        let t = &lib.tracks[0];
        assert!(
            matches!(&t.source, Source::File(p) if p == Path::new("/Users/dj/Music/Bicep - Glue.mp3"))
        );
        assert_eq!((t.rating, t.key.as_str(), t.info.play_count), (4, "Fm", 7));
        assert_eq!(t.info.color, Some(0xFF007F));
        assert_eq!(t.info.remix, "Original Mix");
        assert_eq!(t.grid.markers().len(), 2);
        assert_eq!(t.info.hotcues.len(), 3);
        assert_eq!(t.info.hotcues[0].color, Some(0xE62828));
        assert_eq!(t.info.hotcues[1].end, Some(72.025));
        assert!(t.info.hotcues[2].is_memory());
        assert_eq!(lib.lists.len(), 2);
        assert_eq!(lib.lists[0].folders, ["Sets"]);
        assert_eq!(
            lib.lists[0].entries[1].path,
            PathBuf::from("C:/Music/other.mp3")
        );
        assert_eq!(lib.lists[1].entries.len(), 1);
        assert!(preview(&path).unwrap().starts_with("2 tracks, 2 playlists"));
        std::fs::remove_file(path).ok();
    }
}

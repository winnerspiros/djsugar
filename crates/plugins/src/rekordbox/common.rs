//! What the two ways of reading rekordbox share: its colours, hot cue
//! letters, phrase names and beat grids, and its intelligent playlists in
//! DJ Sugar's search syntax.

use djsugar_engine::beats::{BeatGrid, GridMarker};
use djsugar_engine::{Marker, MarkerKind};

/// The 16 colours of rekordbox's hot cue menu, by the code it keeps for
/// each, as rekordbox shows them (the same table djsugar-export writes).
const HOT_CUE_COLOURS: [(i64, u32); 16] = [
    (0x01, 0x305AFF),
    (0x05, 0x50B4FF),
    (0x09, 0x00E0FF),
    (0x0e, 0x1FA392),
    (0x12, 0x10B176),
    (0x16, 0x28E214),
    (0x1a, 0xA5E116),
    (0x1e, 0xB4BE04),
    (0x20, 0xC3AF04),
    (0x26, 0xE0641B),
    (0x2a, 0xE62828),
    (0x2d, 0xFF127B),
    (0x31, 0xDE44CF),
    (0x38, 0xB432FF),
    (0x3c, 0xAA72FF),
    (0x3e, 0x6473FF),
];

/// A hot cue's colour from rekordbox's code for it.
pub fn hot_cue_colour(code: i64) -> Option<u32> {
    HOT_CUE_COLOURS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, rgb)| *rgb)
}

/// rekordbox's eight track colours by id (1 pink to 8 purple), which memory
/// cues use too.
pub fn track_colour(id: i64) -> Option<u32> {
    const COLOURS: [u32; 8] = [
        0xFF007F, 0xFF0000, 0xFFA500, 0xFFFF00, 0x00FF00, 0x25FDE9, 0x0000FF, 0x660099,
    ];
    usize::try_from(id)
        .ok()
        .and_then(|i| i.checked_sub(1))
        .and_then(|i| COLOURS.get(i).copied())
}

/// DJ Sugar's hot cue number (from 0) for rekordbox's cue kind in master.db:
/// 1 to 3 are A to C, and D to H are kept as 5 to 9.
pub fn hot_cue_number(kind: i64) -> Option<usize> {
    match kind {
        1..=3 => Some(kind as usize - 1),
        4 => Some(3),
        5..=9 => Some(kind as usize - 2),
        _ => None,
    }
}

/// A grid from rekordbox's beats (seconds and tempo of each): a marker
/// where it starts and wherever the tempo changes.
pub fn grid_from_beats(beats: &[(f64, f64)]) -> BeatGrid {
    let mut markers: Vec<GridMarker> = Vec::new();
    for &(seconds, bpm) in beats {
        if markers.last().is_none_or(|m| (m.bpm - bpm).abs() > 0.005) {
            markers.push(GridMarker { seconds, bpm });
        }
    }
    BeatGrid::new(markers)
}

/// rekordbox's phrase moods, which name the phrase kinds differently.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mood {
    High,
    Mid,
    Low,
}

/// The name rekordbox shows for a phrase kind in `mood`.
pub fn phrase_name(mood: Mood, kind: u16) -> &'static str {
    match (mood, kind) {
        (_, 1) => "Intro",
        (Mood::High, 2) => "Up",
        (Mood::High, 3) => "Down",
        (Mood::High, 5) => "Chorus",
        (Mood::High, 6) => "Outro",
        (Mood::Mid, 2) => "Verse 1",
        (Mood::Mid, 3) => "Verse 2",
        (Mood::Mid, 4) => "Verse 3",
        (Mood::Mid, 5) => "Verse 4",
        (Mood::Mid, 6) => "Verse 5",
        (Mood::Mid, 7) => "Verse 6",
        (Mood::Low, 2..=4) => "Verse 1",
        (Mood::Low, 5..=7) => "Verse 2",
        (Mood::Mid | Mood::Low, 8) => "Bridge",
        (Mood::Mid | Mood::Low, 9) => "Chorus",
        (Mood::Mid | Mood::Low, 10) => "Outro",
        _ => "Phrase",
    }
}

/// Phrase markers from rekordbox's phrases (the beat each starts on, from
/// 1, and its kind) up to `end_beat`, placed with the track's beats.
pub fn phrases(mood: Mood, phrases: &[(u16, u16)], end_beat: u16, beats: &[f64]) -> Vec<Marker> {
    let at = |beat: u16| {
        beats
            .get(usize::from(beat).saturating_sub(1))
            .copied()
            .or_else(|| {
                (beat as usize > beats.len())
                    .then(|| beats.last().copied())
                    .flatten()
            })
    };
    phrases
        .iter()
        .enumerate()
        .filter_map(|(i, &(beat, kind))| {
            let next = phrases.get(i + 1).map_or(end_beat, |p| p.0);
            Some(Marker {
                kind: MarkerKind::Phrase,
                seconds: at(beat)?,
                end: at(next).filter(|_| next > beat),
                name: phrase_name(mood, kind).to_string(),
                color: None,
                detected: false,
            })
        })
        .collect()
}

/// One condition of an intelligent playlist: the property, rekordbox's
/// operator number, its unit and its values.
pub struct Condition<'a> {
    pub property: &'a str,
    pub operator: i64,
    pub unit: &'a str,
    pub left: &'a str,
    pub right: &'a str,
}

/// An intelligent playlist in DJ Sugar's search syntax, where DJ Sugar can say
/// the same: `any` joins the conditions with `|`. `tag_name` gives a My Tag's
/// name from its id. None when a condition has no equivalent here.
pub fn smart_query(
    conditions: &[Condition],
    any: bool,
    tag_name: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let terms: Vec<String> = conditions
        .iter()
        .map(|c| term(c, &tag_name))
        .collect::<Option<_>>()?;
    if terms.is_empty() {
        return None;
    }
    Some(terms.join(if any { " | " } else { " " }))
}

fn quoted(v: &str) -> String {
    if v.contains(char::is_whitespace) || v.is_empty() {
        format!("\"{}\"", v.replace('"', ""))
    } else {
        v.to_string()
    }
}

fn term(c: &Condition, tag_name: &impl Fn(&str) -> Option<String>) -> Option<String> {
    // 1 =, 2 ≠, 3 >, 4 <, 5 in range, 6 in the last, 7 not in the last,
    // 8 contains, 9 doesn't contain, 10 starts with, 11 ends with.
    let text = match c.property {
        "artist" => Some("artist"),
        "album" => Some("album"),
        "genre" => Some("genre"),
        "comments" => Some("comment"),
        "label" => Some("publisher"),
        "remixedBy" => Some("remixer"),
        "name" => Some("title"),
        "grouping" => Some("grouping"),
        "key" => Some("key"),
        "producer" => Some("composer"),
        "mytag" => Some("tag"),
        _ => None,
    };
    if let Some(field) = text {
        let value = if c.property == "mytag" {
            tag_name(c.left)?
        } else {
            c.left.to_string()
        };
        let v = quoted(&value);
        return match c.operator {
            1 if field == "key" || field == "tag" => Some(format!("{field}:{v}")),
            1 => Some(format!("{field}:={v}")),
            2 => Some(format!("-{field}:={v}")),
            8 => Some(format!("{field}:{v}")),
            9 => Some(format!("-{field}:{v}")),
            _ => None,
        };
    }
    let number = match c.property {
        "bpm" => Some("bpm"),
        "rating" => Some("rating"),
        "year" => Some("year"),
        "duration" => Some("duration"),
        "counter" => Some("played"),
        _ => None,
    };
    if let Some(field) = number {
        let n = |v: &str| -> Option<String> {
            let x: f64 = v.trim().parse().ok()?;
            // Ratings may come as 0 to 255.
            let x = if field == "rating" && x > 5.0 {
                (x / 51.0).round()
            } else {
                x
            };
            Some(format!("{x}"))
        };
        return match c.operator {
            1 => Some(format!("{field}:{}", n(c.left)?)),
            2 => Some(format!("-{field}:{}", n(c.left)?)),
            3 => Some(format!("{field}:>{}", n(c.left)?)),
            4 => Some(format!("{field}:<{}", n(c.left)?)),
            5 => Some(format!("{field}:{}-{}", n(c.left)?, n(c.right)?)),
            _ => None,
        };
    }
    if c.property == "dateCreated" || c.property == "stockDate" {
        let count: i64 = c.left.trim().parse().ok()?;
        let days = match c.unit {
            "day" | "days" | "" => count,
            "week" | "weeks" => count * 7,
            "month" | "months" => count * 30,
            "year" | "years" => count * 365,
            _ => return None,
        };
        return match c.operator {
            6 => Some(format!("added:<{days}d")),
            7 => Some(format!("added:>{days}d")),
            _ => None,
        };
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_and_cue_letters() {
        assert_eq!(hot_cue_colour(0x2a), Some(0xE62828));
        assert_eq!(hot_cue_colour(0x02), None);
        assert_eq!(track_colour(1), Some(0xFF007F));
        assert_eq!(track_colour(0), None);
        assert_eq!(hot_cue_number(1), Some(0));
        assert_eq!(hot_cue_number(3), Some(2));
        assert_eq!(hot_cue_number(5), Some(3));
        assert_eq!(hot_cue_number(9), Some(7));
        assert_eq!(hot_cue_number(0), None);
    }

    #[test]
    fn a_grid_marks_each_tempo_change() {
        let beats: Vec<(f64, f64)> = (0..8)
            .map(|i| (0.025 + i as f64 * 0.5, 120.0))
            .chain((0..8).map(|i| (4.025 + i as f64 * 60.0 / 124.0, 124.0)))
            .collect();
        let grid = grid_from_beats(&beats);
        assert_eq!(grid.markers().len(), 2);
        assert_eq!(grid.markers()[1].bpm, 124.0);
        assert!((grid.first_beat() - 0.025).abs() < 1e-9);
    }

    #[test]
    fn phrases_get_rekordboxs_names_and_ends() {
        let beats: Vec<f64> = (0..64).map(|i| i as f64 * 0.5).collect();
        let m = phrases(Mood::Mid, &[(1, 1), (17, 2), (33, 9)], 64, &beats);
        let names: Vec<&str> = m.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Intro", "Verse 1", "Chorus"]);
        assert_eq!((m[1].seconds, m[1].end), (8.0, Some(16.0)));
        assert_eq!(m[2].end, Some(31.5));
        assert_eq!(phrase_name(Mood::High, 2), "Up");
        assert_eq!(phrase_name(Mood::Low, 6), "Verse 2");
    }

    #[test]
    fn intelligent_playlists_in_djsugars_search() {
        let c = |property, operator, left, right| Condition {
            property,
            operator,
            unit: "",
            left,
            right,
        };
        let q = smart_query(
            &[
                c("genre", 8, "Deep House", ""),
                c("bpm", 5, "120", "126"),
                c("rating", 3, "153", ""),
                c("mytag", 1, "42", ""),
            ],
            false,
            |id| (id == "42").then(|| "Warm up".to_string()),
        );
        assert_eq!(
            q.as_deref(),
            Some("genre:\"Deep House\" bpm:120-126 rating:>3 tag:\"Warm up\"")
        );
        let any = smart_query(
            &[
                c("artist", 1, "Bicep", ""),
                c("artist", 1, "Floating Points", ""),
            ],
            true,
            |_| None,
        );
        assert_eq!(
            any.as_deref(),
            Some("artist:=Bicep | artist:=\"Floating Points\"")
        );
        assert_eq!(
            smart_query(&[c("filename", 8, "x", "")], false, |_| None),
            None
        );
        assert_eq!(
            smart_query(&[c("artist", 10, "Bi", "")], false, |_| None),
            None
        );
    }
}

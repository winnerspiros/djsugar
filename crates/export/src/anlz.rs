//! A track's analysis files on the stick (`ANLZ0000.DAT`, `.EXT` and
//! `.2EX`): where the audio is, the beat grid, cues and loops, and the
//! waveforms, written with rbox.
//!
//! Section order and the cue layouts follow what rekordbox 7 writes, as
//! Bake'n Deck's `expressport` (MIT licence, Copyright (c) Masanari Higashi)
//! recorded them from real exports: hot cues A to C go in the `.DAT` cue
//! list, D to H in the `.EXT` one, and the `.EXT`'s extended lists carry all
//! of them with names and colours.

use std::path::Path;

use djsugar_engine::track::Hotcue;
use rbox_anlz::{
    Anlz, AnlzString, AnlzTag, Beat, BeatGrid, ColorIndex, Cue, CueList, CueListType, CueStatus,
    CueType, ExtendedCue, ExtendedCueList, Header, VBR,
};

use crate::wave::Waveforms;
use crate::Error;

/// Hot cues A to H.
pub const HOT_CUES: usize = 8;
/// A to C go in the `.DAT` cue list.
const DAT_HOT_CUES: usize = 3;
const NO_LOOP: u32 = 0xFFFF_FFFF;

/// What goes into one track's analysis files.
pub struct TrackAnalysis<'a> {
    /// Where the audio is on the stick, e.g. `/Contents/A/B/x.mp3`.
    pub stick_path: &'a str,
    pub beats: Vec<Beat>,
    /// Tempo for working out loop lengths in beats; 0 when unknown.
    pub bpm: f64,
    /// Hot cues (numbers 0..8) and memory cues; others are left out.
    pub cues: &'a [Hotcue],
    pub waveforms: Waveforms,
    /// MPEG audio frames of an MP3, for the seek section.
    pub mp3_frames: Option<u32>,
}

fn ms(seconds: f64) -> u32 {
    (seconds.max(0.0) * 1000.0).round() as u32
}

/// The 16 colours of rekordbox's hot cue menu: the code the `.EXT` stores,
/// the colour rekordbox shows, and the colour bytes the player lights.
const HOT_CUE_COLOURS: [(u8, u32, [u8; 3]); 16] = [
    (0x01, 0x305AFF, [0x00, 0x00, 0xff]),
    (0x05, 0x50B4FF, [0x00, 0x70, 0xff]),
    (0x09, 0x00E0FF, [0x00, 0xe0, 0xff]),
    (0x0e, 0x1FA392, [0x00, 0xff, 0xa3]),
    (0x12, 0x10B176, [0x00, 0xff, 0x47]),
    (GREEN, 0x28E214, [0x1a, 0xff, 0x00]),
    (0x1a, 0xA5E116, [0x80, 0xff, 0x00]),
    (0x1e, 0xB4BE04, [0xe6, 0xff, 0x00]),
    (0x20, 0xC3AF04, [0xff, 0xe8, 0x00]),
    (0x26, 0xE0641B, [0xff, 0x5e, 0x00]),
    (0x2a, 0xE62828, [0xff, 0x00, 0x00]),
    (0x2d, 0xFF127B, [0xff, 0x00, 0x45]),
    (0x31, 0xDE44CF, [0xff, 0x00, 0xa1]),
    (0x38, 0xB432FF, [0xb3, 0x00, 0xff]),
    (0x3c, 0xAA72FF, [0x4d, 0x00, 0xff]),
    (0x3e, 0x6473FF, [0x1a, 0x00, 0xff]),
];
const GREEN: u8 = 0x16;

fn distance(a: u32, b: u32) -> i32 {
    let d = |shift: u32| {
        let x = ((a >> shift) & 0xff) as i32 - ((b >> shift) & 0xff) as i32;
        x * x
    };
    d(16) + d(8) + d(0)
}

/// A hot cue's colour code and bytes, the nearest of rekordbox's 16. Green
/// and no colour go out as code 0 with green bytes, as rekordbox writes them.
fn hot_colour(color: Option<u32>) -> (u8, (u8, u8, u8)) {
    let none = (0, (0x1a, 0xff, 0x00));
    let Some(rgb) = color else {
        return none;
    };
    let &(code, _, [r, g, b]) = HOT_CUE_COLOURS
        .iter()
        .min_by_key(|(_, shown, _)| distance(rgb, *shown))
        .expect("16 colours");
    if code == GREEN {
        none
    } else {
        (code, (r, g, b))
    }
}

/// The track colours rekordbox and the players know, by id: pink, red,
/// orange, yellow, green, aqua, blue and purple.
pub const TRACK_COLOURS: [(i64, &str, u32); 8] = [
    (1, "Pink", 0xFF007F),
    (2, "Red", 0xFF0000),
    (3, "Orange", 0xFFA500),
    (4, "Yellow", 0xFFFF00),
    (5, "Green", 0x00FF00),
    (6, "Aqua", 0x25FDE9),
    (7, "Blue", 0x0000FF),
    (8, "Purple", 0x660099),
];

/// The nearest track colour's id, 0 for none.
pub fn colour_id(color: Option<u32>) -> i64 {
    color.map_or(0, |rgb| {
        TRACK_COLOURS
            .iter()
            .min_by_key(|(_, _, c)| distance(rgb, *c))
            .map_or(0, |(id, _, _)| *id)
    })
}

/// The hot cue's number on the player: 1 for A up to 8 for H, 0 for a memory cue.
fn hot_number(c: &Hotcue) -> u32 {
    if c.is_memory() {
        0
    } else {
        c.number as u32 + 1
    }
}

fn cue_type(c: &Hotcue) -> CueType {
    if c.end.is_some() {
        CueType::Loop
    } else {
        CueType::Point
    }
}

fn loop_ms(c: &Hotcue) -> u32 {
    c.end.map_or(NO_LOOP, ms)
}

/// A cue list (`PCOB`). Hot cues aren't chained; memory cues are, by
/// their place in the list.
fn cue_list(list: CueListType, cues: &[&Hotcue]) -> CueList {
    let hot = list == CueListType::HotCues;
    let n = cues.len();
    let entries = cues
        .iter()
        .enumerate()
        .map(|(i, c)| Cue {
            header: Header {
                tag: AnlzTag::Cue,
                size: 0x1c,
                total_size: 0x38,
            },
            hot_cue: hot_number(c),
            status: CueStatus::Disabled,
            order_first: if hot || i == 0 {
                0xFFFF
            } else {
                (i - 1) as u16
            },
            order_last: if hot || i + 1 == n {
                0xFFFF
            } else {
                (i + 1) as u16
            },
            cue_type: cue_type(c),
            time: ms(c.seconds),
            loop_time: loop_ms(c),
            u4: 0,
            u5: 0,
            u6: 0,
            u7: 0,
        })
        .collect();
    let mut out = CueList::new(list.clone(), entries);
    if !hot && n > 0 {
        out.memory_count = n as i32 - 1;
    }
    out
}

/// Loop length in whole beats, when it is one.
fn loop_fraction(c: &Hotcue, bpm: f64) -> (u16, u16) {
    match c.end {
        Some(end) if bpm > 0.0 => {
            let beats = (end - c.seconds) * bpm / 60.0;
            let rounded = beats.round();
            if rounded >= 1.0 && (beats - rounded).abs() < 0.02 {
                (rounded as u16, 1)
            } else {
                (0, 0)
            }
        }
        _ => (0, 0),
    }
}

/// An extended cue list (`PCO2`), with names and colours.
fn extended_list(list: CueListType, cues: &[&Hotcue], bpm: f64) -> ExtendedCueList {
    let hot = list == CueListType::HotCues;
    let entries = cues
        .iter()
        .map(|c| {
            let (num, den) = loop_fraction(c, bpm);
            let (code, rgb) = if hot {
                hot_colour(c.color)
            } else {
                (0, (0, 0, 0))
            };
            let memory_colour = if hot { 0 } else { colour_id(c.color) as u8 };
            let comment = if c.name.is_empty() {
                AnlzString::with_padding(String::new(), 0)
            } else {
                AnlzString::with_padding(c.name.clone(), 1)
            };
            ExtendedCue {
                header: Header {
                    tag: AnlzTag::ExtendedCue,
                    size: 0x10,
                    total_size: 0,
                },
                hot_cue: hot_number(c),
                cue_type: cue_type(c),
                time: ms(c.seconds),
                loop_time: loop_ms(c),
                color: ColorIndex::from(memory_colour),
                u3: 1,
                u4: 0,
                u5: 0,
                loop_numerator: num,
                loop_denominator: den,
                comment,
                hot_cue_color_index: code,
                hot_cue_color_rgb: rgb,
                trailing_data: vec![0; 40],
            }
        })
        .collect();
    ExtendedCueList::new(list, entries)
}

/// `PQT2` as rekordbox writes it without an extended grid.
fn empty_extended_grid() -> Vec<u8> {
    let mut s = Vec::with_capacity(56);
    s.extend_from_slice(b"PQT2");
    s.extend_from_slice(&0x38u32.to_be_bytes());
    s.extend_from_slice(&0x38u32.to_be_bytes());
    let mut payload = [0u8; 44];
    payload[4..8].copy_from_slice(&[0x01, 0x00, 0x00, 0x02]);
    s.extend_from_slice(&payload);
    s
}

/// Hot cues A to H, by letter, and memory cues by time. Hot cues past H
/// don't fit on a player and are left out.
fn split(cues: &[Hotcue]) -> (Vec<&Hotcue>, Vec<&Hotcue>) {
    let mut hot: Vec<&Hotcue> = cues
        .iter()
        .filter(|c| !c.is_memory() && c.number < HOT_CUES)
        .collect();
    hot.sort_by_key(|c| c.number);
    let mut memory: Vec<&Hotcue> = cues.iter().filter(|c| c.is_memory()).collect();
    memory.sort_by(|a, b| a.seconds.total_cmp(&b.seconds));
    (hot, memory)
}

/// The three files, built in memory for `stem` (a path without extension).
pub fn build(stem: &Path, t: TrackAnalysis) -> Result<[Anlz; 3], Error> {
    let (hot, memory) = split(t.cues);
    let (dat_hot, ext_hot): (Vec<&Hotcue>, Vec<&Hotcue>) =
        hot.iter().partition(|c| c.number < DAT_HOT_CUES);
    let w = t.waveforms;

    let mut dat = Anlz::new_dat(stem)?;
    dat.set_path(t.stick_path)?;
    dat.set_vbr(VBR {
        u1: 0,
        index: [0; 400],
        u2: t.mp3_frames.map_or(0, |f| f.wrapping_mul(1152)),
    })?;
    dat.set_beat_grid(BeatGrid::new(t.beats))?;
    dat.set_waveform_preview(w.preview)?;
    dat.set_tiny_waveform_preview(w.tiny)?;
    dat.set_hot_cues(cue_list(CueListType::HotCues, &dat_hot))?;
    dat.set_memory_cues(cue_list(CueListType::MemoryCues, &memory))?;

    let mut ext = Anlz::new_ext(stem)?;
    ext.set_path(t.stick_path)?;
    ext.set_waveform_detail(w.detail)?;
    ext.set_hot_cues(cue_list(CueListType::HotCues, &ext_hot))?;
    ext.set_memory_cues(cue_list(CueListType::MemoryCues, &[]))?;
    ext.set_extended_hot_cues(extended_list(CueListType::HotCues, &hot, t.bpm))?;
    ext.set_extended_memory_cues(extended_list(CueListType::MemoryCues, &memory, t.bpm))?;
    ext.set_section_bytes(empty_extended_grid())?;
    ext.set_waveform_color_detail(w.colour_detail)?;
    ext.set_waveform_color_preview(w.colour_preview)?;

    let mut two = Anlz::new_2ex(stem)?;
    two.set_path(t.stick_path)?;
    two.set_waveform_3band_detail(w.band_detail)?;
    two.set_waveform_3band_preview(w.band_preview)?;
    two.set_waveform_3band_calibration(w.band_gains)?;

    Ok([dat, ext, two])
}

/// The grid as the players keep it: every beat, numbered 1 to 4 in its bar
/// from the grid's first beat, with the tempo there.
pub fn beats(grid: &djsugar_engine::BeatGrid, duration: f64) -> Vec<Beat> {
    if grid.is_empty() || duration <= 0.0 {
        return Vec::new();
    }
    let first = grid.beat_at(0.0).ceil();
    let mut out = Vec::new();
    let mut beat = first;
    loop {
        let seconds = grid.seconds_at(beat);
        if !seconds.is_finite() || seconds >= duration {
            break;
        }
        if seconds >= 0.0 {
            let bar = (beat - grid.beat_at(grid.first_beat()).round()).rem_euclid(4.0);
            out.push(Beat {
                beat_number: bar as u16 + 1,
                tempo: (grid.bpm_at(seconds) * 100.0).round().clamp(0.0, 65535.0) as u16,
                time: ms(seconds),
            });
        }
        beat += 1.0;
        if out.len() > 100_000 {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wave;
    use djsugar_engine::track::Waveform;

    fn cue(number: usize, seconds: f64) -> Hotcue {
        Hotcue {
            number,
            seconds,
            name: String::new(),
            end: None,
            color: None,
        }
    }

    fn files(cues: &[Hotcue]) -> (tempfile::TempDir, [Anlz; 3]) {
        let dir = tempfile::tempdir().unwrap();
        let waveform = Waveform::from_finest(1000.0, vec![[80; 8]; 2000]);
        let stem = dir.path().join("ANLZ0000");
        let files = build(
            &stem,
            TrackAnalysis {
                stick_path: "/Contents/A/B/c.mp3",
                beats: beats(&djsugar_engine::BeatGrid::constant(120.0, 0.1), 2.0),
                bpm: 120.0,
                cues,
                waveforms: wave::waveforms(&waveform, 2.0),
                mp3_frames: Some(38),
            },
        )
        .unwrap();
        (dir, files)
    }

    #[test]
    fn sections_are_in_rekordbox_order() {
        let (dir, [mut dat, mut ext, mut two]) = files(&[cue(0, 0.1)]);
        dat.dump().unwrap();
        ext.dump().unwrap();
        two.dump().unwrap();
        let tags = |ext: &str| section_tags(dir.path().join(format!("ANLZ0000.{ext}")));
        assert_eq!(
            tags("DAT"),
            ["PPTH", "PVBR", "PQTZ", "PWAV", "PWV2", "PCOB", "PCOB"]
        );
        assert_eq!(
            tags("EXT"),
            ["PPTH", "PWV3", "PCOB", "PCOB", "PCO2", "PCO2", "PQT2", "PWV5", "PWV4"]
        );
        assert_eq!(tags("2EX"), ["PPTH", "PWV7", "PWV6", "PWVC"]);
    }

    /// The tags of a written file's sections, walking their lengths.
    fn section_tags(path: std::path::PathBuf) -> Vec<String> {
        let bytes = std::fs::read(path).unwrap();
        let be = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        assert_eq!(&bytes[..4], b"PMAI");
        assert_eq!(be(8), bytes.len(), "file length field");
        let mut at = be(4);
        let mut tags = Vec::new();
        while at < bytes.len() {
            tags.push(String::from_utf8_lossy(&bytes[at..at + 4]).into_owned());
            at += be(at + 8);
        }
        tags
    }

    /// Bytes from a rekordbox export: hot cues B at 15.280 s and C at 30.280 s.
    #[test]
    fn hot_cue_bytes_match_rekordbox() {
        let cues = [cue(1, 15.280), cue(2, 30.280)];
        let (dir, [mut dat, mut ext, _]) = files(&cues);
        dat.dump().unwrap();
        ext.dump().unwrap();
        let bytes = std::fs::read(dir.path().join("ANLZ0000.DAT")).unwrap();
        let pcob = hex("50434f4200000018000000880000000100000002ffffffff504350540000001c00000038000000020000000000010000ffffffff010003e800003bb0ffffffff00000000000000000000000000000000504350540000001c00000038000000030000000000010000ffffffff010003e800007648ffffffff00000000000000000000000000000000");
        assert!(find(&bytes, &pcob), "PCOB not as rekordbox writes it");
        let bytes = std::fs::read(dir.path().join("ANLZ0000.EXT")).unwrap();
        let pcp2 = hex("50435032000000100000005800000003010003e800007648ffffffff00010000000000000000000000000000001aff0000000000000000000000000000000000000000000000000000000000000000000000000000000000");
        assert!(find(&bytes, &pcp2), "PCP2 not as rekordbox writes it");
    }

    /// From a rekordbox export: one memory cue named "1.1Bars" at 1.547 s.
    #[test]
    fn memory_cue_bytes_match_rekordbox() {
        let mut memory = cue(djsugar_engine::track::MEMORY_CUE, 1.547);
        memory.name = "1.1Bars".into();
        let (dir, [mut dat, _, _]) = files(&[memory]);
        dat.dump().unwrap();
        let bytes = std::fs::read(dir.path().join("ANLZ0000.DAT")).unwrap();
        let pcob = hex("50434f420000001800000050000000000000000100000000504350540000001c00000038000000000000000000010000ffffffff010003e80000060bffffffff00000000000000000000000000000000");
        assert!(find(&bytes, &pcob));
    }

    #[test]
    fn grid_beats_count_bars_from_the_first_beat() {
        let b = beats(&djsugar_engine::BeatGrid::constant(120.0, 0.25), 2.25);
        let got: Vec<_> = b.iter().map(|b| (b.beat_number, b.tempo, b.time)).collect();
        assert_eq!(
            got,
            [
                (1, 12000, 250),
                (2, 12000, 750),
                (3, 12000, 1250),
                (4, 12000, 1750)
            ]
        );
    }

    #[test]
    fn colours_snap_to_the_players_palette() {
        assert_eq!(hot_colour(Some(0xE62828)).0, 0x2a);
        assert_eq!(hot_colour(Some(0x28E214)).0, 0);
        assert_eq!(colour_id(Some(0xFF1010)), 2);
        assert_eq!(colour_id(None), 0);
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    fn find(hay: &[u8], needle: &[u8]) -> bool {
        hay.windows(needle.len()).any(|w| w == needle)
    }
}

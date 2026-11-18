//! Reads Serato metadata from audio file tags.
//!
//! Serato stores its per-track metadata (cues, loops, beatgrid, color, BPM lock,
//! auto-gain) in custom tag frames:
//! - MP3/AIFF/WAV: ID3v2.4 GEOB frames
//! - MP4/M4A: `----:com.serato.dj:*` atoms (base64)
//! - FLAC/Ogg Vorbis: Vorbis comments with `SERATO_*` keys (base64)
//!
//! The GEOB/atom/comment data format:
//! - `application/octet-stream\0` (MIME type)
//! - `\0` (null byte)
//! - Tag name (e.g., `Serato Markers2`, `Serato BeatGrid`, `Serato Autotags`)
//! - Base64-encoded binary payload (may have `\n` every 72 chars)
//!
//! The payloads are documented in Holzhaus/serato-tags (MIT).

use std::collections::HashMap;

use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use djsugar_engine::beats::{BeatGrid, GridMarker};
use djsugar_engine::{Hotcue, LibraryInfo, Marker, MarkerKind, Track, MEMORY_CUE};

/// Reads all Serato tags from an audio file and applies them to the track.
pub fn read_serato_tags(track: &mut Track, path: &std::path::Path) {
    let tags = lofty::read_from_path(path);
    if tags.is_err() {
        return;
    }
    let tagged = tags.unwrap();

    // Collect all Serato GEOB/atom/comment data
    let mut markers2_data = None;
    let mut beatgrid_data = None;
    let mut autotags_data = None;

    for tag in tagged.tags() {
        // ID3v2 GEOB frames
        if let Some(id3_tag) = tag.as_id3v2() {
            for frame in id3_tag.frames() {
                if frame.id() == "GEOB" {
                    if let Some(geob) = frame.as_geob() {
                        let mime = geob.mime_type();
                        let desc = geob.description();
                        let data = geob.data();
                        if mime == "application/octet-stream" {
                            if desc == "Serato Markers2" || desc == "Serato Markers_" {
                                markers2_data = Some(format!(
                                    "application/octet-stream\0{}\0{}",
                                    desc,
                                    STANDARD_NO_PAD.encode(data)
                                ));
                            } else if desc == "Serato BeatGrid" {
                                beatgrid_data = Some(format!(
                                    "application/octet-stream\0{}\0{}",
                                    desc,
                                    STANDARD_NO_PAD.encode(data)
                                ));
                            } else if desc == "Serato Autotags" {
                                autotags_data = Some(format!(
                                    "application/octet-stream\0{}\0{}",
                                    desc,
                                    STANDARD_NO_PAD.encode(data)
                                ));
                            }
                        }
                    }
                }
            }
        }

        // MP4 atoms
        if let Some(mp4_tag) = tag.as_mp4() {
            for item in mp4_tag.items() {
                if item.key().starts_with("----:com.serato.dj:") {
                    let key = item.key().strip_prefix("----:com.serato.dj:").unwrap();
                    if let Some(data) = item.data().first() {
                        let decoded = STANDARD_NO_PAD.decode(data).ok();
                        if let Some(decoded) = decoded {
                            let geob_data = format!(
                                "application/octet-stream\0{}\0{}",
                                key,
                                STANDARD_NO_PAD.encode(&decoded)
                            );
                            match key {
                                "markersv2" | "markers" => markers2_data = Some(geob_data),
                                "beatgrid" => beatgrid_data = Some(geob_data),
                                "autgain" => autotags_data = Some(geob_data),
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        // Vorbis comments (FLAC/Ogg)
        if let Some(vorbis_tag) = tag.as_vorbis_comments() {
            for comment in vorbis_tag.comments() {
                let key = comment.key();
                let value = comment.value();
                if key == "SERATO_MARKERS_V2" || key == "SERATO_MARKERS" {
                    markers2_data = Some(format!(
                        "application/octet-stream\0Serato Markers2\0{}",
                        value
                    ));
                } else if key == "SERATO_BEATGRID" {
                    beatgrid_data = Some(format!(
                        "application/octet-stream\0Serato BeatGrid\0{}",
                        value
                    ));
                } else if key == "SERATO_AUTOGAIN" {
                    autotags_data = Some(format!(
                        "application/octet-stream\0Serato Autotags\0{}",
                        value
                    ));
                }
            }
        }
    }

    // Parse and apply tags
    if let Some(data) = markers2_data {
        if let Ok((color, bpm_locked, cues, loops)) = parse_markers2(&data) {
            if let Some(c) = color {
                track.info.color = Some(c);
            }
            if bpm_locked {
                track.info.grid_locked = true;
                track.info.grid_from = "serato".into();
            }
            apply_cues(track, cues);
            apply_loops(track, loops);
        }
    }

    if let Some(data) = beatgrid_data {
        if let Ok(grid) = parse_beatgrid(&data) {
            track.grid = grid;
            track.first_beat = grid.first_beat();
            track.bpm = grid.main_bpm(track.duration);
            track.info.fixed_grid = true;
            track.info.grid_from = "serato".into();
        }
    }

    if let Some(data) = autotags_data {
        if let Ok((bpm, auto_gain, gain_db)) = parse_autotags(&data) {
            if track.bpm == 0.0 {
                track.bpm = bpm;
            }
            track.info.gain_db = Some(gain_db);
            // Store auto_gain as a tag or in comment
            if auto_gain != 0.0 {
                track.info.comment = format!(
                    "{}; Serato AutoGain: {:.1}dB",
                    track.info.comment, auto_gain
                );
            }
        }
    }
}

/// Parses the `Serato Markers2` payload.
/// Returns (color, bpm_locked, cues, loops).
pub fn parse_markers2(
    geob_data: &str,
) -> Result<(Option<u32>, bool, Vec<CueData>, Vec<LoopData>), String> {
    // Find the base64 payload after "application/octet-stream\0<tag>\0"
    let parts: Vec<&str> = geob_data.splitn(3, '\0').collect();
    if parts.len() != 3 {
        return Err("invalid GEOB format".into());
    }
    let b64 = parts[2].replace('\n', "");
    let payload = STANDARD_NO_PAD.decode(&b64).map_err(|e| e.to_string())?;

    if payload.len() < 2 || payload[0] != 0x01 || payload[1] != 0x01 {
        return Err("invalid Markers2 header".into());
    }

    let mut cursor = 2;
    let mut color = None;
    let mut bpm_locked = false;
    let mut cues = Vec::new();
    let mut loops = Vec::new();

    while cursor < payload.len() {
        // Read null-terminated type string
        let type_end = payload[cursor..]
            .iter()
            .position(|&b| b == 0)
            .ok_or("unterminated type")?;
        let type_str = std::str::from_utf8(&payload[cursor..cursor + type_end])?;
        cursor += type_end + 1;

        if cursor + 4 > payload.len() {
            break;
        }
        let length = u32::from_be_bytes([
            payload[cursor],
            payload[cursor + 1],
            payload[cursor + 2],
            payload[cursor + 3],
        ]) as usize;
        cursor += 4;

        if cursor + length > payload.len() {
            break;
        }
        let entry_data = &payload[cursor..cursor + length];
        cursor += length;

        match type_str {
            "COLOR" => {
                if entry_data.len() == 4 && entry_data[0] == 0 {
                    color = Some(
                        ((entry_data[1] as u32) << 16)
                            | ((entry_data[2] as u32) << 8)
                            | (entry_data[3] as u32),
                    );
                }
            }
            "BPMLOCK" => {
                if entry_data.len() == 1 {
                    bpm_locked = entry_data[0] != 0;
                }
            }
            "CUE" => {
                if let Ok(cue) = parse_cue_entry(entry_data) {
                    cues.push(cue);
                }
            }
            "LOOP" => {
                if let Ok(loop_) = parse_loop_entry(entry_data) {
                    loops.push(loop_);
                }
            }
            "FLIP" => {
                // TODO: parse flip data
            }
            _ => {
                // Unknown entry type, skip
            }
        }
    }

    Ok((color, bpm_locked, cues, loops))
}

#[derive(Debug, Clone)]
pub struct CueData {
    pub index: u32,    // 1-8 for hot cues, >8 for memory cues
    pub position: f64, // seconds
    pub color: Option<u32>,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct LoopData {
    pub start: f64,
    pub end: f64,
    pub color: Option<u32>,
    pub name: String,
}

fn parse_cue_entry(data: &[u8]) -> Result<CueData, String> {
    // CUE entry format (from serato-tags):
    // index (u32), position (f64), color (4 bytes), name (UTF-8 null-terminated)
    let mut cursor = 0;
    if data.len() < 4 + 8 + 4 {
        return Err("cue entry too short".into());
    }
    let index = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    cursor += 4;

    let pos_bits = u64::from_be_bytes([
        data[cursor],
        data[cursor + 1],
        data[cursor + 2],
        data[cursor + 3],
        data[cursor + 4],
        data[cursor + 5],
        data[cursor + 6],
        data[cursor + 7],
    ]);
    let position = f64::from_bits(pos_bits);
    cursor += 8;

    let color = if data[cursor] == 0 && cursor + 3 < data.len() {
        Some(
            ((data[cursor + 1] as u32) << 16)
                | ((data[cursor + 2] as u32) << 8)
                | (data[cursor + 3] as u32),
        )
    } else {
        None
    };
    cursor += 4;

    // Name is null-terminated UTF-8
    let name_end = data[cursor..]
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(data.len() - cursor);
    let name = std::str::from_utf8(&data[cursor..cursor + name_end])?.to_string();

    Ok(CueData {
        index,
        position,
        color,
        name,
    })
}

fn parse_loop_entry(data: &[u8]) -> Result<LoopData, String> {
    // LOOP entry format:
    // start (f64), end (f64), color (4 bytes), name (UTF-8 null-terminated)
    let mut cursor = 0;
    if data.len() < 8 + 8 + 4 {
        return Err("loop entry too short".into());
    }

    let start_bits = u64::from_be_bytes([
        data[cursor],
        data[cursor + 1],
        data[cursor + 2],
        data[cursor + 3],
        data[cursor + 4],
        data[cursor + 5],
        data[cursor + 6],
        data[cursor + 7],
    ]);
    let start = f64::from_bits(start_bits);
    cursor += 8;

    let end_bits = u64::from_be_bytes([
        data[cursor],
        data[cursor + 1],
        data[cursor + 2],
        data[cursor + 3],
        data[cursor + 4],
        data[cursor + 5],
        data[cursor + 6],
        data[cursor + 7],
    ]);
    let end = f64::from_bits(end_bits);
    cursor += 8;

    let color = if data[cursor] == 0 && cursor + 3 < data.len() {
        Some(
            ((data[cursor + 1] as u32) << 16)
                | ((data[cursor + 2] as u32) << 8)
                | (data[cursor + 3] as u32),
        )
    } else {
        None
    };
    cursor += 4;

    let name_end = data[cursor..]
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(data.len() - cursor);
    let name = std::str::from_utf8(&data[cursor..cursor + name_end])?.to_string();

    Ok(LoopData {
        start,
        end,
        color,
        name,
    })
}

/// Applies parsed cues to the track's hotcues and markers.
fn apply_cues(track: &mut Track, cues: Vec<CueData>) {
    for cue in cues {
        if cue.index >= 1 && cue.index <= 8 {
            // Hot cue
            track.info.hotcues.push(Hotcue {
                index: cue.index as u8,
                position: cue.position,
                color: cue.color,
                name: cue.name,
                kind: djsugar_engine::HotcueKind::Cue,
            });
        } else {
            // Memory cue
            track.info.markers.push(Marker {
                position: cue.position,
                kind: MarkerKind::Cue,
                label: cue.name,
                color: cue.color,
            });
        }
    }
    // Sort hotcues by index
    track.info.hotcues.sort_by_key(|h| h.index);
    // Sort markers by position
    track
        .info
        .markers
        .sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
}

/// Applies parsed loops to the track's hotcues (as saved loops) and markers.
fn apply_loops(track: &mut Track, loops: Vec<LoopData>) {
    for (i, loop_) in loops.iter().enumerate() {
        let index = (9 + i) as u8; // Saved loops start at index 9
        track.info.hotcues.push(Hotcue {
            index,
            position: loop_.start,
            color: loop_.color,
            name: loop_.name.clone(),
            kind: djsugar_engine::HotcueKind::Loop { end: loop_.end },
        });
        track.info.markers.push(Marker {
            position: loop_.start,
            kind: MarkerKind::LoopStart,
            label: loop_.name.clone(),
            color: loop_.color,
        });
        track.info.markers.push(Marker {
            position: loop_.end,
            kind: MarkerKind::LoopEnd,
            label: loop_.name.clone(),
            color: loop_.color,
        });
    }
    track
        .info
        .markers
        .sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
}

/// Parses the `Serato BeatGrid` payload.
pub fn parse_beatgrid(geob_data: &str) -> Result<BeatGrid, String> {
    let parts: Vec<&str> = geob_data.splitn(3, '\0').collect();
    if parts.len() != 3 {
        return Err("invalid GEOB format".into());
    }
    let b64 = parts[2].replace('\n', "");
    let payload = STANDARD_NO_PAD.decode(&b64).map_err(|e| e.to_string())?;

    if payload.len() < 6 {
        return Err("beatgrid payload too short".into());
    }

    // Header: 2 bytes (0x01 0x00 typically)
    // Num markers: 4 bytes (u32 BE)
    let num_markers = u32::from_be_bytes([payload[2], payload[3], payload[4], payload[5]]) as usize;

    let mut cursor = 6;
    let mut markers = Vec::new();

    for _ in 0..num_markers {
        if cursor + 8 > payload.len() {
            break;
        }
        // Each marker: position (f32 BE), bpm (f32 BE)
        let pos = f32::from_be_bytes([
            payload[cursor],
            payload[cursor + 1],
            payload[cursor + 2],
            payload[cursor + 3],
        ]) as f64;
        cursor += 4;
        let bpm = f32::from_be_bytes([
            payload[cursor],
            payload[cursor + 1],
            payload[cursor + 2],
            payload[cursor + 3],
        ]) as f64;
        cursor += 4;

        markers.push(GridMarker { seconds: pos, bpm });
    }

    // Footer byte (0x37) - ignore

    Ok(BeatGrid::new(markers))
}

/// Parses the `Serato Autotags` payload.
/// Returns (bpm, auto_gain_db, gain_db).
pub fn parse_autotags(geob_data: &str) -> Result<(f64, f64, f64), String> {
    let parts: Vec<&str> = geob_data.splitn(3, '\0').collect();
    if parts.len() != 3 {
        return Err("invalid GEOB format".into());
    }
    let b64 = parts[2].replace('\n', "");
    let payload = STANDARD_NO_PAD.decode(&b64).map_err(|e| e.to_string())?;

    if payload.len() < 2 || payload[0] != 0x01 || payload[1] != 0x01 {
        return Err("invalid Autotags header".into());
    }

    // After header, three null-terminated strings: BPM, Auto Gain, Gain dB
    let mut cursor = 2;
    let mut strings = Vec::new();

    for _ in 0..3 {
        let end = payload[cursor..]
            .iter()
            .position(|&b| b == 0)
            .ok_or("unterminated string")?;
        let s = std::str::from_utf8(&payload[cursor..cursor + end])?;
        strings.push(s.to_string());
        cursor += end + 1;
    }

    let bpm = strings.get(0).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let auto_gain = strings.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let gain_db = strings.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);

    Ok((bpm, auto_gain, gain_db))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};

    #[test]
    fn test_parse_markers2_color_bpmlock() {
        let mut payload = Vec::new();
        payload.extend_from_slice(b"\x01\x01");
        payload.extend_from_slice(b"COLOR\0");
        payload.extend_from_slice(&[0, 0, 0, 4]);
        payload.extend_from_slice(&[0, 0xFF, 0x00, 0x00]);
        payload.extend_from_slice(b"BPMLOCK\0");
        payload.extend_from_slice(&[0, 0, 0, 1]);
        payload.extend_from_slice(&[1]);
        payload.push(0);

        let encoded = STANDARD_NO_PAD.encode(&payload);
        let geob = format!("application/octet-stream\0Serato Markers2\0{}", encoded);

        let (color, bpm_locked, cues, loops) = parse_markers2(&geob).unwrap();
        assert_eq!(color, Some(0xFF0000));
        assert!(bpm_locked);
        assert!(cues.is_empty());
        assert!(loops.is_empty());
    }

    #[test]
    fn test_parse_markers2_cue() {
        let mut payload = Vec::new();
        payload.extend_from_slice(b"\x01\x01");
        // CUE: index=1, position=30.5, color=blue, name="Drop"
        payload.extend_from_slice(b"CUE\0");
        let mut cue_data = Vec::new();
        cue_data.extend_from_slice(&[0, 0, 0, 1]); // index
        cue_data.extend_from_slice(&(30.5f64).to_bits().to_be_bytes()); // position
        cue_data.extend_from_slice(&[0, 0x00, 0x00, 0xFF]); // color
        cue_data.extend_from_slice(b"Drop\0");
        payload.extend_from_slice(&(cue_data.len() as u32).to_be_bytes());
        payload.extend_from_slice(&cue_data);
        payload.push(0);

        let encoded = STANDARD_NO_PAD.encode(&payload);
        let geob = format!("application/octet-stream\0Serato Markers2\0{}", encoded);

        let (_, _, cues, _) = parse_markers2(&geob).unwrap();
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].index, 1);
        assert!((cues[0].position - 30.5).abs() < 0.001);
        assert_eq!(cues[0].color, Some(0x0000FF));
        assert_eq!(cues[0].name, "Drop");
    }

    #[test]
    fn test_parse_markers2_loop() {
        let mut payload = Vec::new();
        payload.extend_from_slice(b"\x01\x01");
        // LOOP: start=60.0, end=90.0, color=green, name="Loop 1"
        payload.extend_from_slice(b"LOOP\0");
        let mut loop_data = Vec::new();
        loop_data.extend_from_slice(&(60.0f64).to_bits().to_be_bytes());
        loop_data.extend_from_slice(&(90.0f64).to_bits().to_be_bytes());
        loop_data.extend_from_slice(&[0, 0x00, 0xFF, 0x00]); // color
        loop_data.extend_from_slice(b"Loop 1\0");
        payload.extend_from_slice(&(loop_data.len() as u32).to_be_bytes());
        payload.extend_from_slice(&loop_data);
        payload.push(0);

        let encoded = STANDARD_NO_PAD.encode(&payload);
        let geob = format!("application/octet-stream\0Serato Markers2\0{}", encoded);

        let (_, _, _, loops) = parse_markers2(&geob).unwrap();
        assert_eq!(loops.len(), 1);
        assert!((loops[0].start - 60.0).abs() < 0.001);
        assert!((loops[0].end - 90.0).abs() < 0.001);
        assert_eq!(loops[0].color, Some(0x00FF00));
        assert_eq!(loops[0].name, "Loop 1");
    }

    #[test]
    fn test_parse_beatgrid() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&[0x01, 0x00]); // header
        payload.extend_from_slice(&[0x00, 0x00, 0x00, 0x02]); // 2 markers
                                                              // Marker 1: position=1.5, bpm=128.0
        payload.extend_from_slice(&1.5f32.to_be_bytes());
        payload.extend_from_slice(&128.0f32.to_be_bytes());
        // Marker 2: position=120.0, bpm=126.0
        payload.extend_from_slice(&120.0f32.to_be_bytes());
        payload.extend_from_slice(&126.0f32.to_be_bytes());
        payload.push(0x37); // footer

        let encoded = STANDARD_NO_PAD.encode(&payload);
        let geob = format!("application/octet-stream\0Serato BeatGrid\0{}", encoded);

        let grid = parse_beatgrid(&geob).unwrap();
        assert_eq!(grid.markers.len(), 2);
        assert!((grid.markers[0].seconds - 1.5).abs() < 0.001);
        assert!((grid.markers[0].bpm - 128.0).abs() < 0.001);
        assert!((grid.markers[1].seconds - 120.0).abs() < 0.001);
        assert!((grid.markers[1].bpm - 126.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_autotags() {
        let mut payload = Vec::new();
        payload.extend_from_slice(b"\x01\x01");
        payload.extend_from_slice(b"128.00\0");
        payload.extend_from_slice(b"-3.257\0");
        payload.extend_from_slice(b"0.000\0");

        let encoded = STANDARD_NO_PAD.encode(&payload);
        let geob = format!("application/octet-stream\0Serato Autotags\0{}", encoded);

        let (bpm, auto_gain, gain_db) = parse_autotags(&geob).unwrap();
        assert!((bpm - 128.0).abs() < 0.01);
        assert!((auto_gain - (-3.257)).abs() < 0.001);
        assert!((gain_db - 0.0).abs() < 0.001);
    }
}

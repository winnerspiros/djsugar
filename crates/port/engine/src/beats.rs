//! Beat grids whose tempo can change partway through a track
//! (live drums, edits, old disco).
//!
//! A grid is a list of markers. Each marker is a beat, and the beats carry on
//! at the marker's tempo until the next marker. Every marker falls on a whole
//! beat (beat numbers count from the first marker), so bars and phase stay
//! continuous across tempo changes: [`BeatGrid::new`] nudges each section's
//! tempo so a whole number of beats fits between its markers. Before the
//! first marker and after the last the grid carries on at their tempo.
//!
//! The lookups only read the markers, so the audio thread can use them.

/// A change to a loaded track's beat grid, asked for from a deck or a
/// controller, made at the deck's play position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridEdit {
    /// Moves the grid so a beat falls here (`beats_translate_curpos`).
    BeatHere,
    /// Moves the grid by this many seconds (`beats_translate_earlier`/`_later`).
    Shift(f64),
    /// Moves the grid by this many seconds as a drag of the waveform
    /// (`beats_translate_by`): one drag is one undo step.
    Drag(f64),
    /// Changes the tempo here by this many BPM (`beats_adjust_faster`/`_slower`).
    Tempo(f64),
    /// Sets the tempo here to this many BPM (a deck's tapped `bpm_tap`).
    SetTempo(f64),
    /// Starts a new tempo section on the beat here (`beats_section_add`).
    AddSection,
    /// Removes the tempo change that starts the section here (`beats_section_remove`).
    RemoveSection,
    /// Halves or doubles the tempo of the whole grid (`beats_scale_half`/`_double`).
    Scale(f64),
    /// Puts a beat here and makes it the first of a bar (`beats_downbeat_here`).
    BarHere,
    /// Takes back the last edit (`beats_undo_adjustment`).
    Undo,
    /// Locks or unlocks the grid against edits and analysis (`bpmlock`).
    Lock(bool),
    /// Goes back to the grid analysis found (`beats_reset`).
    Reset,
}

impl BeatGrid {
    /// The grid after `edit` at `seconds` (`Undo`, `Lock` and `Reset` are the
    /// caller's to handle).
    pub fn edited(&self, edit: GridEdit, seconds: f64) -> Self {
        match edit {
            GridEdit::BeatHere => self.with_beat_at(seconds),
            GridEdit::BarHere => self.with_beat_at(seconds).with_bar_at(seconds),
            GridEdit::Shift(by) | GridEdit::Drag(by) => self.shifted(by),
            GridEdit::Tempo(by) => self.with_bpm_at(seconds, self.bpm_at(seconds) + by),
            GridEdit::SetTempo(bpm) => self.with_bpm_at(seconds, bpm),
            GridEdit::AddSection => self.with_marker(seconds, self.bpm_at(seconds)),
            GridEdit::RemoveSection => self.without_marker_at(seconds),
            GridEdit::Scale(f) => Self::new(
                self.markers
                    .iter()
                    .map(|m| GridMarker {
                        seconds: m.seconds,
                        bpm: m.bpm * f,
                    })
                    .collect(),
            ),
            GridEdit::Undo | GridEdit::Lock(_) | GridEdit::Reset => self.clone(),
        }
    }
}

/// A beat where the tempo (re)starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridMarker {
    pub seconds: f64,
    pub bpm: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BeatGrid {
    markers: Vec<GridMarker>,
    /// The beat number of each marker.
    index: Vec<f64>,
}

/// Shortest section the grid keeps, in beats; markers closer than this to the
/// one before are dropped.
const MIN_SECTION_BEATS: f64 = 4.0;

impl BeatGrid {
    /// A grid through `markers` (in any order). Markers with no usable tempo
    /// are dropped, as are markers that would start a section shorter than
    /// a few beats.
    pub fn new(mut markers: Vec<GridMarker>) -> Self {
        markers.retain(|m| m.seconds.is_finite() && m.bpm.is_finite() && m.bpm > 1.0);
        markers.sort_by(|a, b| a.seconds.total_cmp(&b.seconds));
        let mut kept: Vec<GridMarker> = Vec::with_capacity(markers.len());
        let mut index = Vec::with_capacity(markers.len());
        for m in markers {
            let Some(prev) = kept.last_mut() else {
                kept.push(m);
                index.push(0.0);
                continue;
            };
            let span = m.seconds - prev.seconds;
            let beats = (span * prev.bpm / 60.0).round();
            if beats < MIN_SECTION_BEATS {
                continue;
            }
            // Fit a whole number of beats between the two markers.
            prev.bpm = beats * 60.0 / span;
            let at = index.last().unwrap() + beats;
            kept.push(m);
            index.push(at);
        }
        BeatGrid {
            markers: kept,
            index,
        }
    }

    /// A grid at one tempo, with a beat at `first_beat`.
    pub fn constant(bpm: f64, first_beat: f64) -> Self {
        Self::new(vec![GridMarker {
            seconds: first_beat,
            bpm,
        }])
    }

    pub fn is_empty(&self) -> bool {
        self.markers.is_empty()
    }

    pub fn markers(&self) -> &[GridMarker] {
        &self.markers
    }

    /// Whether the tempo changes anywhere.
    pub fn is_dynamic(&self) -> bool {
        self.markers.len() > 1
    }

    /// Which section `seconds` falls in.
    fn section(&self, seconds: f64) -> usize {
        self.markers
            .partition_point(|m| m.seconds <= seconds)
            .saturating_sub(1)
    }

    /// The beat number at `seconds`, fractional between beats. The first
    /// marker is beat 0; earlier times give negative beats.
    pub fn beat_at(&self, seconds: f64) -> f64 {
        if self.markers.is_empty() {
            return 0.0;
        }
        let k = self.section(seconds);
        let m = self.markers[k];
        self.index[k] + (seconds - m.seconds) * m.bpm / 60.0
    }

    /// When beat number `beat` falls, in seconds.
    pub fn seconds_at(&self, beat: f64) -> f64 {
        if self.markers.is_empty() {
            return 0.0;
        }
        let k = self.index.partition_point(|&i| i <= beat).saturating_sub(1);
        let m = self.markers[k];
        m.seconds + (beat - self.index[k]) * 60.0 / m.bpm
    }

    /// The tempo at `seconds`.
    pub fn bpm_at(&self, seconds: f64) -> f64 {
        if self.markers.is_empty() {
            return 0.0;
        }
        self.markers[self.section(seconds)].bpm
    }

    /// The tempo that runs longest within `duration` seconds: the one to show
    /// in the library and sort by.
    pub fn main_bpm(&self, duration: f64) -> f64 {
        let mut best = (0.0, 0.0);
        for (k, m) in self.markers.iter().enumerate() {
            let end = self.markers.get(k + 1).map_or(duration, |n| n.seconds);
            let span = end.min(duration) - m.seconds.max(0.0);
            if span > best.0 || k == 0 {
                best = (span.max(best.0), m.bpm);
            }
        }
        best.1
    }

    /// The first beat at or after the start of the track.
    pub fn first_beat(&self) -> f64 {
        if self.markers.is_empty() {
            return 0.0;
        }
        let b = self.beat_at(0.0).ceil();
        self.seconds_at(b).max(0.0)
    }

    /// The nearest beat to `seconds` (or the one before, with `floor`).
    pub fn snap(&self, seconds: f64, floor: bool) -> f64 {
        if self.markers.is_empty() {
            return seconds;
        }
        let b = self.beat_at(seconds);
        // A hair of tolerance so a time on a beat doesn't floor to the one before.
        self.seconds_at(if floor { (b + 1e-6).floor() } else { b.round() })
    }

    /// The same grid moved by `seconds`.
    pub fn shifted(&self, seconds: f64) -> Self {
        BeatGrid {
            markers: self
                .markers
                .iter()
                .map(|m| GridMarker {
                    seconds: m.seconds + seconds,
                    bpm: m.bpm,
                })
                .collect(),
            index: self.index.clone(),
        }
    }

    /// The grid moved so the beat nearest `seconds` lands exactly there.
    pub fn with_beat_at(&self, seconds: f64) -> Self {
        let nearest = self.snap(seconds, false);
        self.shifted(seconds - nearest)
    }

    /// The grid renumbered so the beat nearest `seconds` starts a bar (bars
    /// are four beats from beat 0). Beats stay where they are: a marker a
    /// few beats before the first one takes its place.
    pub fn with_bar_at(&self, seconds: f64) -> Self {
        let Some(&first) = self.markers.first() else {
            return self.clone();
        };
        let to_bar = (4.0 - self.beat_at(seconds).round().rem_euclid(4.0)).rem_euclid(4.0);
        if to_bar == 0.0 {
            return self.clone();
        }
        let mut markers = self.markers.clone();
        markers.push(GridMarker {
            seconds: first.seconds - to_bar * 60.0 / first.bpm,
            bpm: first.bpm,
        });
        Self::new(markers)
    }

    /// The grid with a new tempo section starting on the beat nearest
    /// `seconds`, at `bpm`. Later markers are kept.
    pub fn with_marker(&self, seconds: f64, bpm: f64) -> Self {
        if self.markers.is_empty() {
            return Self::constant(bpm, seconds);
        }
        let at = self.snap(seconds, false);
        let mut markers: Vec<GridMarker> = self
            .markers
            .iter()
            .copied()
            .filter(|m| (m.seconds - at).abs() > 1e-6)
            .collect();
        markers.push(GridMarker { seconds: at, bpm });
        Self::new(markers)
    }

    /// The grid with the tempo of the section around `seconds` set to `bpm`.
    /// The section keeps its first beat and its number of beats, so later
    /// sections move with its end.
    pub fn with_bpm_at(&self, seconds: f64, bpm: f64) -> Self {
        if self.markers.is_empty() || bpm.is_nan() || bpm <= 1.0 {
            return self.clone();
        }
        let k = self.section(seconds);
        let mut markers = self.markers.clone();
        if k + 1 < markers.len() {
            let beats = self.index[k + 1] - self.index[k];
            let end = markers[k].seconds + beats * 60.0 / bpm;
            let shift = end - markers[k + 1].seconds;
            for m in &mut markers[k + 1..] {
                m.seconds += shift;
            }
        }
        markers[k].bpm = bpm;
        Self::new(markers)
    }

    /// The grid without the tempo change that starts the section around
    /// `seconds`: the section before it carries on. The first marker stays.
    pub fn without_marker_at(&self, seconds: f64) -> Self {
        let k = self.section(seconds);
        if k == 0 {
            return self.clone();
        }
        let mut markers = self.markers.clone();
        markers.remove(k);
        Self::new(markers)
    }

    /// Up to `N` sections around the window from `from` to `to` seconds: each
    /// one's start, tempo and beat number, for drawing.
    pub fn sections_in<const N: usize>(&self, from: f64, to: f64) -> [Option<(f64, f64, f64)>; N] {
        let mut out = [None; N];
        if self.markers.is_empty() {
            return out;
        }
        let first = self.section(from);
        for (slot, k) in out.iter_mut().zip(first..self.markers.len()) {
            let m = self.markers[k];
            if k > first && m.seconds > to {
                break;
            }
            *slot = Some((m.seconds, m.bpm, self.index[k]));
        }
        out
    }

    /// Stores the grid as text: `seconds:bpm` pairs separated by spaces.
    pub fn to_text(&self) -> String {
        self.markers
            .iter()
            .map(|m| format!("{}:{}", m.seconds, m.bpm))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Reads a grid stored with [`BeatGrid::to_text`].
    pub fn from_text(text: &str) -> Self {
        Self::new(
            text.split_whitespace()
                .filter_map(|pair| {
                    let (s, b) = pair.split_once(':')?;
                    Some(GridMarker {
                        seconds: s.parse().ok()?,
                        bpm: b.parse().ok()?,
                    })
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn a_constant_grid_counts_beats_both_ways() {
        let g = BeatGrid::constant(120.0, 0.25);
        assert!(close(g.beat_at(0.25), 0.0));
        assert!(close(g.beat_at(2.25), 4.0));
        assert!(close(g.beat_at(0.0), -0.5));
        assert!(close(g.seconds_at(8.0), 4.25));
        assert!(close(g.first_beat(), 0.25));
        assert!(close(g.snap(1.0, false), 1.25));
        assert!(close(g.snap(1.0, true), 0.75));
        assert!(close(g.snap(0.75, true), 0.75));
    }

    #[test]
    fn markers_fall_on_whole_beats_and_tempo_changes_at_them() {
        // 120 BPM for 16 beats (8 s), then 126 BPM. The second marker is a
        // little off the 120 grid; the first section's tempo is refitted.
        let g = BeatGrid::new(vec![
            GridMarker {
                seconds: 8.05,
                bpm: 126.0,
            },
            GridMarker {
                seconds: 0.05 + 0.01,
                bpm: 120.0,
            },
        ]);
        assert_eq!(g.markers().len(), 2);
        assert!(close(g.beat_at(8.05), 16.0));
        assert!(close(g.seconds_at(16.0), 8.05));
        assert!(close(g.bpm_at(4.0), 16.0 * 60.0 / 7.99));
        assert!(close(g.bpm_at(9.0), 126.0));
        assert!(close(g.seconds_at(17.0), 8.05 + 60.0 / 126.0));
        assert!(g.is_dynamic());
        // Beat numbers and times round-trip across sections.
        for b in [-3.0, 0.0, 5.5, 15.9, 16.0, 40.25] {
            assert!(close(g.beat_at(g.seconds_at(b)), b));
        }
    }

    #[test]
    fn bar_here_puts_bar_one_on_the_beat_and_keeps_the_beats() {
        let grid = BeatGrid::constant(120.0, 0.25);
        // 2.25 s is beat 4 (a downbeat already), 2.75 s is beat 5.
        let moved = grid.edited(GridEdit::BarHere, 2.76);
        assert!((moved.beat_at(2.76) - 8.0).abs() < 1e-9, "{:?}", moved);
        assert!((moved.snap(10.26, false) - 10.26).abs() < 1e-9);
        // A grid in sections keeps its later section.
        let two = BeatGrid::new(vec![
            GridMarker {
                seconds: 0.0,
                bpm: 120.0,
            },
            GridMarker {
                seconds: 16.0,
                bpm: 125.0,
            },
        ]);
        let bar = two.with_bar_at(1.5);
        assert_eq!(bar.beat_at(1.5).round().rem_euclid(4.0), 0.0);
        assert!((bar.bpm_at(20.0) - 125.0).abs() < 1e-9);
        assert!((bar.snap(16.0, false) - 16.0).abs() < 1e-9);
        assert_eq!(bar.markers().len(), 2);
    }

    #[test]
    fn tiny_sections_are_dropped() {
        let g = BeatGrid::new(vec![
            GridMarker {
                seconds: 0.0,
                bpm: 120.0,
            },
            // One beat later: too short.
            GridMarker {
                seconds: 0.5,
                bpm: 130.0,
            },
        ]);
        assert_eq!(g.markers().len(), 1);
    }

    #[test]
    fn main_tempo_is_the_one_that_runs_longest() {
        let g = BeatGrid::new(vec![
            GridMarker {
                seconds: 0.0,
                bpm: 100.0,
            },
            GridMarker {
                seconds: 24.0,
                bpm: 128.0,
            },
        ]);
        assert!(close(g.main_bpm(200.0), 128.0));
        assert!(close(g.main_bpm(30.0), 100.0));
    }

    #[test]
    fn edits_keep_the_grid_on_whole_beats() {
        let g = BeatGrid::constant(120.0, 0.0);
        let moved = g.with_beat_at(1.1);
        assert!(close(moved.snap(1.1, false), 1.1));
        let split = g.with_marker(10.1, 124.0);
        assert!(close(split.markers()[1].seconds, 10.0));
        assert!(close(split.bpm_at(11.0), 124.0));
        // Speeding up the first section keeps its 20 beats and pulls the
        // next section in.
        let faster = split.with_bpm_at(3.0, 125.0);
        assert!(close(faster.markers()[1].seconds, 20.0 * 60.0 / 125.0));
        assert!(close(faster.beat_at(faster.markers()[1].seconds), 20.0));
        assert!(close(faster.bpm_at(11.0), 124.0));
        assert_eq!(split.without_marker_at(11.0), g);
        assert_eq!(split.without_marker_at(3.0), split);
        let text = split.to_text();
        assert_eq!(BeatGrid::from_text(&text), split);
        let sections = split.sections_in::<4>(9.0, 11.0);
        assert_eq!(sections[0], Some((0.0, 120.0, 0.0)));
        assert_eq!(sections[1], Some((10.0, 124.0, 20.0)));
        assert_eq!(sections[2], None);
    }
}

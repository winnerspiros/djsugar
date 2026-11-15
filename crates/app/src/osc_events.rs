//! OSC events for visuals and lights: `/beat` and `/phrase` from the
//! master deck, `/deck/N/track` when a track loads and `/deck/N/cue` when
//! a hot cue fires.
//!
//! [`Follower`] works out the events from where the decks are; [`spawn`]
//! runs it on a thread of its own about 200 times a second while OSC is
//! on, from the same play positions the waveforms are drawn at, so a beat
//! goes out within a few milliseconds of the audio buffer that plays it.
//! It only reads: no lock the audio thread takes, nothing written.

use std::sync::Arc;
use std::time::{Duration, Instant};

use djsugar_control::{ControlHandle, ControlRegistry};
use djsugar_engine::{LoadedTracks, Track, VisualPositions, NUM_DECKS};
use djsugar_osc::{OscMessage, OscType};
use djsugar_waveform::PhraseKind;

use crate::beat_counter;

/// How often the decks are looked at.
const PERIOD: Duration = Duration::from_millis(5);
/// Hot cues watched per deck (those the registry has).
const HOT_CUES: usize = 36;

/// What one deck is doing at a moment.
#[derive(Clone, Default)]
pub struct DeckNow {
    pub track: Option<Arc<Track>>,
    /// Track seconds and speed (1 at the track's own tempo, 0 paused).
    pub seconds: f64,
    pub speed: f64,
    pub playing: bool,
}

/// A track and its phrases, found once.
type TrackPhrases = (Arc<Track>, Vec<(f64, PhraseKind)>);

/// A hot cue's activate control and the version last seen.
struct Cue {
    number: usize,
    handle: ControlHandle,
    seen: u32,
}

pub struct Follower {
    /// The master deck: the one playing, or with several, the one that
    /// started first (as Ableton Link follows).
    master: Option<usize>,
    /// The master's last beat (deck, beat number from the first beat).
    beat: Option<(usize, i64)>,
    /// The master's last phrase (deck, phrase number from 1).
    phrase: Option<(usize, i64)>,
    loaded: [Option<Arc<Track>>; NUM_DECKS],
    phrases: [Option<TrackPhrases>; NUM_DECKS],
    cues: Vec<Vec<Cue>>,
}

impl Follower {
    /// Starts from what's loaded now, so only later loads are announced.
    pub fn new(registry: &ControlRegistry, loaded: [Option<Arc<Track>>; NUM_DECKS]) -> Self {
        let cues = (0..NUM_DECKS)
            .map(|d| {
                (1..=HOT_CUES)
                    .filter_map(|n| {
                        let handle = registry.get(
                            &format!("[Channel{}]", d + 1),
                            &format!("hotcue_{n}_activate"),
                        )?;
                        Some(Cue {
                            number: n,
                            seen: handle.version(),
                            handle,
                        })
                    })
                    .collect()
            })
            .collect();
        Self {
            master: None,
            beat: None,
            phrase: None,
            loaded,
            phrases: Default::default(),
            cues,
        }
    }

    /// The events since the last step.
    pub fn step(&mut self, decks: &[DeckNow; NUM_DECKS]) -> Vec<OscMessage> {
        let mut out = Vec::new();
        for (d, now) in decks.iter().enumerate() {
            let changed = match (&self.loaded[d], &now.track) {
                (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
                (None, None) => false,
                _ => true,
            };
            if changed {
                self.loaded[d] = now.track.clone();
                if let Some(t) = &now.track {
                    out.push(message(
                        &format!("/deck/{}/track", d + 1),
                        vec![
                            OscType::String(t.title.clone()),
                            OscType::String(t.artist.clone()),
                            OscType::Float(t.bpm as f32),
                            OscType::String(t.key.clone()),
                        ],
                    ));
                }
            }
            for cue in &mut self.cues[d] {
                let v = cue.handle.version();
                if v != cue.seen {
                    // Pressed now, or pressed and let go since the last look.
                    let pressed = cue.handle.get_bool() || v.wrapping_sub(cue.seen) >= 2;
                    cue.seen = v;
                    if pressed {
                        out.push(message(
                            &format!("/deck/{}/cue", d + 1),
                            vec![OscType::Int(cue.number as i32)],
                        ));
                    }
                }
            }
        }
        let playing = |d: usize| {
            let n = &decks[d];
            n.playing && n.track.as_ref().is_some_and(|t| t.bpm > 0.0)
        };
        let master = self
            .master
            .filter(|&d| playing(d))
            .or_else(|| (0..NUM_DECKS).find(|&d| playing(d)));
        let switched = master != self.master;
        self.master = master;
        let Some(d) = master else {
            self.beat = None;
            self.phrase = None;
            return out;
        };
        let now = &decks[d];
        let Some(t) = now.track.clone() else {
            return out;
        };
        let phrases = match &self.phrases[d] {
            Some((cached, p)) if Arc::ptr_eq(cached, &t) => p.clone(),
            _ => {
                let p = beat_counter::track_phrases(&t);
                self.phrases[d] = Some((t.clone(), p.clone()));
                p
            }
        };
        let Some(count) = beat_counter::count(&t, now.seconds, &phrases) else {
            return out;
        };
        let beat = (t.beat_at(now.seconds) + 1e-6).floor() as i64;
        // Only forward steps of the deck that was already master: a jump,
        // a new master or a scratch backwards isn't a beat heard.
        let last = self.beat.replace((d, beat));
        let fresh =
            !switched && last.is_some_and(|(ld, lb)| ld == d && beat > lb && beat - lb <= 2);
        if fresh {
            let bpm = t.bpm * now.speed.abs();
            out.push(message(
                "/beat",
                vec![
                    OscType::Int(count.beat as i32 + 1),
                    OscType::Int(count.bar_in_phrase.max(0) as i32),
                    OscType::Float(bpm as f32),
                ],
            ));
        }
        let last_phrase = self.phrase.replace((d, count.phrase));
        let new_phrase = fresh
            && count.phrase > 0
            && count.bar_in_phrase == 1
            && count.beat == 0
            && last_phrase.is_some_and(|(ld, lp)| ld == d && lp != count.phrase);
        if new_phrase {
            let name = if phrases.is_empty() {
                "Phrase"
            } else {
                phrases
                    .get(count.phrase as usize - 1)
                    .map_or("Phrase", |&(_, kind)| phrase_name(kind))
            };
            out.push(message(
                "/phrase",
                vec![
                    OscType::String(name.into()),
                    OscType::Int(count.bars_left as i32),
                ],
            ));
        }
        out
    }
}

/// The phrase name sent with `/phrase`.
pub fn phrase_name(kind: PhraseKind) -> &'static str {
    match kind {
        PhraseKind::Intro => "Intro",
        PhraseKind::Build => "Build",
        PhraseKind::Drop => "Drop",
        PhraseKind::Breakdown => "Breakdown",
        PhraseKind::Outro => "Outro",
        PhraseKind::Chorus => "Chorus",
        PhraseKind::Verse => "Verse",
    }
}

fn message(address: &str, args: Vec<OscType>) -> OscMessage {
    OscMessage {
        addr: address.into(),
        args,
    }
}

/// Runs a [`Follower`] while OSC is on, handing its events to `send`.
pub fn spawn(
    registry: Arc<ControlRegistry>,
    loaded: LoadedTracks,
    visual: Arc<VisualPositions>,
    running: impl Fn() -> bool + Send + 'static,
    send: impl Fn(Vec<OscMessage>) + Send + 'static,
) {
    let play: Vec<ControlHandle> = (0..NUM_DECKS)
        .map(|d| registry.toggle(&format!("[Channel{}]", d + 1), "play"))
        .collect();
    let _ = std::thread::Builder::new()
        .name("osc events".into())
        .spawn(move || {
            let mut follower: Option<Follower> = None;
            loop {
                if !running() {
                    follower = None;
                    std::thread::sleep(Duration::from_millis(250));
                    continue;
                }
                let tracks = loaded.read().map(|t| t.clone()).unwrap_or_default();
                let now = Instant::now();
                let decks: [DeckNow; NUM_DECKS] = std::array::from_fn(|d| {
                    let (seconds, speed) = visual.position(d, now);
                    DeckNow {
                        track: tracks[d].clone(),
                        seconds,
                        speed,
                        playing: play[d].get_bool(),
                    }
                });
                let f = follower.get_or_insert_with(|| Follower::new(&registry, tracks.clone()));
                let events = f.step(&decks);
                if !events.is_empty() {
                    send(events);
                }
                std::thread::sleep(PERIOD);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use djsugar_control::Behavior;

    /// 120 BPM from 0 s: a beat every half second, a bar every two.
    fn track(title: &str) -> Arc<Track> {
        let mut t = (*djsugar_engine::sample_library()[0]).clone();
        t.title = title.into();
        t.bpm = 120.0;
        t.first_beat = 0.0;
        t.grid = Default::default();
        t.duration = 600.0;
        t.waveform = Default::default();
        Arc::new(t)
    }

    fn decks(track: &Arc<Track>, seconds: f64, playing: bool) -> [DeckNow; NUM_DECKS] {
        let mut d: [DeckNow; NUM_DECKS] = Default::default();
        d[0] = DeckNow {
            track: Some(track.clone()),
            seconds,
            speed: 1.0,
            playing,
        };
        d
    }

    fn addresses(m: &[OscMessage]) -> Vec<&str> {
        m.iter().map(|m| m.addr.as_str()).collect()
    }

    #[test]
    fn a_load_is_announced_once() {
        let reg = ControlRegistry::new();
        let mut f = Follower::new(&reg, Default::default());
        let t = track("Song");
        let out = f.step(&decks(&t, 0.0, false));
        assert_eq!(addresses(&out), ["/deck/1/track"]);
        assert_eq!(out[0].args[0], OscType::String("Song".into()));
        assert!(f.step(&decks(&t, 0.0, false)).is_empty());
    }

    #[test]
    fn beats_and_eight_bar_phrases_follow_the_playing_deck() {
        let reg = ControlRegistry::new();
        let t = track("Song");
        let mut loaded: [Option<Arc<Track>>; NUM_DECKS] = Default::default();
        loaded[0] = Some(t.clone());
        let mut f = Follower::new(&reg, loaded);
        // Paused: nothing.
        assert!(f.step(&decks(&t, 0.2, false)).is_empty());
        let mut beats = Vec::new();
        let mut phrases = Vec::new();
        // Play 17 seconds in 5 ms steps: 34 beats, into the second phrase.
        let mut s = 0.2;
        while s < 17.2 {
            for m in f.step(&decks(&t, s, true)) {
                match m.addr.as_str() {
                    "/beat" => beats.push(m.args),
                    "/phrase" => phrases.push(m.args),
                    other => panic!("unexpected {other}"),
                }
            }
            s += 0.005;
        }
        assert_eq!(beats.len(), 34);
        assert_eq!(
            beats[0],
            vec![OscType::Int(2), OscType::Int(1), OscType::Float(120.0)]
        );
        // Beat 1 of bar 1 of phrase 2 at 16 s, and the phrase with it.
        assert_eq!(
            phrases,
            vec![vec![OscType::String("Phrase".into()), OscType::Int(8)]]
        );
    }

    #[test]
    fn a_jump_is_not_a_beat() {
        let reg = ControlRegistry::new();
        let t = track("Song");
        let mut loaded: [Option<Arc<Track>>; NUM_DECKS] = Default::default();
        loaded[0] = Some(t.clone());
        let mut f = Follower::new(&reg, loaded);
        f.step(&decks(&t, 10.1, true));
        assert!(f.step(&decks(&t, 30.1, true)).is_empty());
        assert!(f.step(&decks(&t, 5.1, true)).is_empty());
        assert_eq!(addresses(&f.step(&decks(&t, 5.5, true))), ["/beat"]);
    }

    #[test]
    fn a_hot_cue_press_is_sent() {
        let reg = ControlRegistry::new();
        let cue = reg.create(
            "[Channel2]",
            "hotcue_3_activate",
            0.0,
            0.0,
            1.0,
            Behavior::Push,
        );
        let mut f = Follower::new(&reg, Default::default());
        let none: [DeckNow; NUM_DECKS] = Default::default();
        assert!(f.step(&none).is_empty());
        cue.set(1.0);
        let out = f.step(&none);
        assert_eq!(addresses(&out), ["/deck/2/cue"]);
        assert_eq!(out[0].args, vec![OscType::Int(3)]);
        // Let go: nothing more. Pressed and let go between looks: sent.
        cue.set(0.0);
        assert!(f.step(&none).is_empty());
        cue.set(1.0);
        cue.set(0.0);
        assert_eq!(addresses(&f.step(&none)), ["/deck/2/cue"]);
    }
}

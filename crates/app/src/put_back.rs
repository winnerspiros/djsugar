//! Put back: an unloaded deck offers what it held, so a stray ⏏ costs one
//! click. Whatever empties a deck (the cover's ⏏, the Mixer's, its menu, a
//! shortcut, a controller's eject) is caught the same way: the deck held
//! something on one frame and nothing on the next. The empty deck then
//! shows "Put back <name>" until something else goes on it.
//!
//! A track comes back at the place it was unloaded from; a Groove Box,
//! Looper or live input (the Mixer and instruments too) is turned back on.

use std::sync::Arc;

use djsugar_control::{ControlHandle, ControlRegistry};
use djsugar_engine::{deck_group, AudioEngine, Track, NUM_DECKS};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::{AppWindow, DeckActions};

/// What a deck held.
#[derive(Clone)]
enum Held {
    /// A track and where it was, 0 to 1 of the way through. Kept for Put
    /// back without its audio, which the loader reads again.
    Track(Arc<Track>, f64),
    Groove,
    Looper,
    /// A live input: `[ChannelN],live_input` (the Mixer is one of them).
    Live(f64),
}

struct Deck {
    loaded: ControlHandle,
    duration: ControlHandle,
    playposition: ControlHandle,
    live_input: ControlHandle,
    groove: ControlHandle,
    looper: ControlHandle,
    /// On the last frame.
    now: Option<Held>,
    /// Before the deck was emptied, offered as Put back.
    last: Option<Held>,
    /// A put back track's place, set once it is on the deck.
    seek: Option<f64>,
}

pub struct PutBack {
    decks: Vec<Deck>,
    names: [String; NUM_DECKS],
}

impl PutBack {
    pub fn new(reg: &ControlRegistry) -> Self {
        let decks = (0..NUM_DECKS)
            .map(|d| {
                let g = deck_group(d);
                let c = |group: &str, k: &str| {
                    reg.get(group, k)
                        .unwrap_or_else(|| panic!("missing control {group},{k}"))
                };
                Deck {
                    loaded: c(&g, "track_loaded"),
                    duration: c(&g, "duration"),
                    playposition: c(&g, "playposition"),
                    live_input: c(&g, "live_input"),
                    groove: c(&format!("[Groove{}]", d + 1), "enabled"),
                    looper: c(&djsugar_engine::looper::looper_group(d), "enabled"),
                    now: None,
                    last: None,
                    seek: None,
                }
            })
            .collect();
        Self {
            decks,
            names: Default::default(),
        }
    }

    /// Called every frame with the tracks on the decks.
    pub fn tick(&mut self, app: &AppWindow, tracks: &[Option<Arc<Track>>]) {
        if self.update(tracks) {
            let names: Vec<SharedString> = self.names.iter().map(|n| n.into()).collect();
            app.global::<DeckActions>()
                .set_put_back_names(ModelRc::new(VecModel::from(names)));
        }
    }

    /// Follows what is on the decks; true when a Put back name changed.
    fn update(&mut self, tracks: &[Option<Arc<Track>>]) -> bool {
        let mut changed = false;
        for (d, deck) in self.decks.iter_mut().enumerate() {
            let track = tracks.get(d).cloned().flatten();
            // A put back track goes to its place once the engine has it.
            if let Some(at) = deck.seek {
                if deck.loaded.get_bool() && deck.duration.get() > 0.0 {
                    deck.playposition.set(at);
                    deck.seek = None;
                }
            }
            let now = held(deck, track);
            match (&deck.now, &now) {
                (Some(before), None) => deck.last = Some(without_audio(before)),
                (_, Some(_)) => deck.last = None,
                _ => {}
            }
            deck.now = now;
            let name = deck.last.as_ref().map_or(String::new(), name);
            if self.names[d] != name {
                self.names[d] = name;
                changed = true;
            }
        }
        changed
    }

    /// Puts back what `deck` held before it was unloaded.
    pub fn put_back(&mut self, deck: usize, engine: &AudioEngine) {
        let Some(d) = self.decks.get_mut(deck) else {
            return;
        };
        if d.now.is_some() {
            return;
        }
        match d.last.clone() {
            Some(Held::Track(t, at)) => {
                if engine.load_track(deck, t) {
                    d.seek = Some(at);
                }
            }
            Some(Held::Groove) => d.groove.set_bool(true),
            Some(Held::Looper) => d.looper.set_bool(true),
            Some(Held::Live(input)) => d.live_input.set(input),
            None => {}
        }
    }
}

/// What `deck` holds now, as the engine's controls say.
fn held(deck: &Deck, track: Option<Arc<Track>>) -> Option<Held> {
    if deck.loaded.get_bool() {
        // Still on its way to the list of loaded tracks: as before.
        let Some(t) = track else {
            return deck.now.clone();
        };
        return Some(Held::Track(t, deck.playposition.get()));
    }
    if deck.groove.get_bool() {
        Some(Held::Groove)
    } else if deck.looper.get_bool() {
        Some(Held::Looper)
    } else if deck.live_input.get() > 0.0 {
        Some(Held::Live(deck.live_input.get()))
    } else {
        None
    }
}

/// `held` to keep for Put back: a track's audio is freed with the deck.
fn without_audio(held: &Held) -> Held {
    match held {
        Held::Track(t, at) => Held::Track(
            Arc::new(Track {
                audio: None,
                ..(**t).clone()
            }),
            *at,
        ),
        other => other.clone(),
    }
}

/// The name Put back shows.
fn name(held: &Held) -> String {
    match held {
        Held::Track(t, _) if !t.title.is_empty() => t.title.clone(),
        Held::Track(..) => "the track".into(),
        Held::Groove => "the Groove Box".into(),
        Held::Looper => "the Looper".into(),
        Held::Live(input) if *input as usize == djsugar_engine::inputs::MIXER => "the Mixer".into(),
        Held::Live(input) if *input as usize == 1 => "the mic".into(),
        Held::Live(input) if *input as usize == 3 => "the instrument".into(),
        Held::Live(_) => "the input".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use djsugar_engine::engine::Engine;
    use djsugar_engine::sample_library;

    struct Rig {
        reg: Arc<ControlRegistry>,
        _engine: Engine,
        put_back: PutBack,
    }

    impl Rig {
        fn new() -> Self {
            let reg = ControlRegistry::new();
            let (engine, _input) = Engine::new(&reg, 48_000.0);
            let put_back = PutBack::new(&reg);
            Self {
                reg,
                _engine: engine,
                put_back,
            }
        }

        fn c(&self, group: &str, key: &str) -> ControlHandle {
            self.reg.get(group, key).unwrap()
        }

        fn frame(&mut self, tracks: &[Option<Arc<Track>>]) {
            self.put_back.update(tracks);
        }
    }

    fn track() -> Arc<Track> {
        let mut t = (*sample_library()[0]).clone();
        t.title = "Midnight Circuit".into();
        t.duration = 252.0;
        Arc::new(t)
    }

    #[test]
    fn an_unloaded_track_is_offered_back_where_it_was() {
        let mut r = Rig::new();
        let t = track();
        r.c("[Channel1]", "track_loaded").set_bool(true);
        r.c("[Channel1]", "duration").set(252.0);
        r.c("[Channel1]", "playposition").set(0.5);
        r.frame(&[Some(t.clone()), None, None, None]);
        assert_eq!(r.put_back.names[0], "");
        r.c("[Channel1]", "track_loaded").set_bool(false);
        r.frame(&[None, None, None, None]);
        assert_eq!(r.put_back.names[0], "Midnight Circuit");
        match &r.put_back.decks[0].last {
            Some(Held::Track(kept, at)) => {
                assert!(kept.audio.is_none());
                assert_eq!(*at, 0.5);
            }
            _ => panic!("no track to put back"),
        }
        // Something else on the deck: nothing to put back.
        r.c("[Groove1]", "enabled").set_bool(true);
        r.frame(&[None, None, None, None]);
        assert_eq!(r.put_back.names[0], "");
    }

    #[test]
    fn a_live_deck_is_offered_back_by_its_name() {
        let mut r = Rig::new();
        let mixer = djsugar_engine::inputs::MIXER as f64;
        r.c("[Channel2]", "live_input").set(mixer);
        r.frame(&[None, None, None, None]);
        r.c("[Channel2]", "live_input").set(0.0);
        r.frame(&[None, None, None, None]);
        assert_eq!(r.put_back.names[1], "the Mixer");
        r.c("[Groove1]", "enabled").set_bool(true);
        r.frame(&[None, None, None, None]);
        r.c("[Groove1]", "enabled").set_bool(false);
        r.frame(&[None, None, None, None]);
        assert_eq!(r.put_back.names[0], "the Groove Box");
    }

    #[test]
    fn a_track_still_reaching_the_list_keeps_what_was_there() {
        let mut r = Rig::new();
        r.c("[Channel1]", "live_input").set(1.0);
        r.frame(&[None, None, None, None]);
        // The engine has the track before the list of loaded tracks does.
        r.c("[Channel1]", "live_input").set(0.0);
        r.c("[Channel1]", "track_loaded").set_bool(true);
        r.frame(&[None, None, None, None]);
        assert_eq!(r.put_back.names[0], "");
    }
}

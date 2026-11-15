//! Player's sleep timer (the moon on the Player bar): after 15, 30, 45 or
//! 60 minutes the music fades out over a few seconds and stops; or it stops
//! when the track playing now ends, with Automix switched off so nothing
//! mixes in after it. Runs on the UI thread a few times a second.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use djsugar_control::ControlRegistry;
use djsugar_engine::{LoadedTracks, NUM_DECKS};
use slint::ComponentHandle;

use crate::{broadcast, AppWindow, SleepUi};

/// The menu's choices, in minutes; the last is "End of track".
pub const MINUTES: [u64; 4] = [15, 30, 45, 60];
/// How long the music takes to fade out.
const FADE: Duration = Duration::from_secs(10);

enum Timer {
    Off,
    /// Stops at this time, fading from FADE before it.
    At(Instant),
    /// Stops when the track on this deck stops or changes.
    EndOfTrack(usize, usize),
}

struct Sleep {
    registry: Arc<ControlRegistry>,
    loaded: LoadedTracks,
    timer: Timer,
    /// The master level before the fade, to put back after.
    gain: Option<f64>,
}

/// "23 min", or "45 s" in the last minute.
pub fn left(d: Duration) -> String {
    let s = d.as_secs();
    if s < 60 {
        format!("{s} s")
    } else {
        format!("{} min", s.div_ceil(60))
    }
}

/// How loud the music is `into` the fade (0 at the start, FADE at the end).
fn fade_level(into: Duration) -> f64 {
    (1.0 - into.as_secs_f64() / FADE.as_secs_f64()).clamp(0.0, 1.0)
}

impl Sleep {
    fn control(&self, group: &str, key: &str) -> Option<djsugar_control::ControlHandle> {
        self.registry.get(group, key)
    }

    fn set(&mut self, choice: i32) {
        self.restore();
        self.timer = match choice {
            c if (1..=MINUTES.len() as i32).contains(&c) => {
                Timer::At(Instant::now() + Duration::from_secs(MINUTES[c as usize - 1] * 60))
            }
            c if c == MINUTES.len() as i32 + 1 => {
                match broadcast::audible_deck(&self.registry, &self.loaded) {
                    Some((deck, t)) => {
                        // Nothing mixes in after this track.
                        if let Some(c) = self.control("[AutoDJ]", "enabled") {
                            c.set(0.0);
                        }
                        Timer::EndOfTrack(deck, t.id)
                    }
                    None => Timer::Off,
                }
            }
            _ => Timer::Off,
        };
    }

    /// Puts the master level back after a fade.
    fn restore(&mut self) {
        if let (Some(g), Some(c)) = (self.gain.take(), self.control("[Master]", "gain")) {
            c.set(g);
        }
    }

    /// Stops every deck and Automix, and puts the level back.
    fn stop(&mut self) {
        if let Some(c) = self.control("[AutoDJ]", "enabled") {
            c.set(0.0);
        }
        for d in 0..NUM_DECKS {
            if let Some(c) = self.control(&format!("[Channel{}]", d + 1), "play") {
                c.set(0.0);
            }
        }
        self.restore();
        self.timer = Timer::Off;
    }

    /// Follows the timer; what the button shows.
    fn tick(&mut self) -> (bool, String) {
        match self.timer {
            Timer::Off => (false, String::new()),
            Timer::At(until) => {
                let now = Instant::now();
                if now >= until {
                    self.stop();
                    return (false, String::new());
                }
                let rest = until - now;
                if rest <= FADE {
                    let gain = self.control("[Master]", "gain");
                    if let Some(c) = gain {
                        let start = *self.gain.get_or_insert_with(|| c.get());
                        c.set(start * fade_level(FADE - rest));
                    }
                }
                (true, format!("Stops in {}", left(rest)))
            }
            Timer::EndOfTrack(deck, id) => {
                let playing = self
                    .control(&format!("[Channel{}]", deck + 1), "play")
                    .is_some_and(|c| c.get() > 0.5);
                let same = self
                    .loaded
                    .read()
                    .ok()
                    .and_then(|t| t.get(deck).cloned().flatten())
                    .is_some_and(|t| t.id == id);
                if !playing || !same {
                    self.stop();
                    return (false, String::new());
                }
                (true, "Stops after this track".into())
            }
        }
    }
}

/// Wires the Player bar's sleep menu; keep the returned timer alive.
pub fn install(
    app: &AppWindow,
    registry: Arc<ControlRegistry>,
    loaded: LoadedTracks,
) -> slint::Timer {
    let sleep = Rc::new(RefCell::new(Sleep {
        registry,
        loaded,
        timer: Timer::Off,
        gain: None,
    }));
    let ui = app.global::<SleepUi>();
    let s = sleep.clone();
    let weak = app.as_weak();
    ui.on_set(move |choice| {
        s.borrow_mut().set(choice);
        if let Some(app) = weak.upgrade() {
            let (on, text) = s.borrow_mut().tick();
            let ui = app.global::<SleepUi>();
            ui.set_on(on);
            ui.set_left(text.into());
        }
    });
    let weak = app.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(250),
        move || {
            let (on, text) = sleep.borrow_mut().tick();
            if let Some(app) = weak.upgrade() {
                let ui = app.global::<SleepUi>();
                if ui.get_on() != on {
                    ui.set_on(on);
                }
                if ui.get_left() != text.as_str() {
                    ui.set_left(text.into());
                }
            }
        },
    );
    timer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_left() {
        assert_eq!(left(Duration::from_secs(23 * 60 - 10)), "23 min");
        assert_eq!(left(Duration::from_secs(61)), "2 min");
        assert_eq!(left(Duration::from_secs(45)), "45 s");
    }

    #[test]
    fn the_fade() {
        assert_eq!(fade_level(Duration::ZERO), 1.0);
        assert!((fade_level(FADE / 2) - 0.5).abs() < 1e-9);
        assert_eq!(fade_level(FADE * 2), 0.0);
    }
}

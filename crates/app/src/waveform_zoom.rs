//! The large waveforms' zoom: twenty steps of 5%, shared by every large scrolling
//! waveform in every layout, Prep's included, so beats sit the same
//! distance apart on every deck and the zoom stays put when the layout
//! changes. Deck overviews and mini waveforms keep showing the whole track.
//!
//! The steps are where the zoom stops, not how it moves: each change glides
//! there over [`GLIDE`], the waveforms stretching as it goes. The wheel,
//! trackpad and the edge bar's drag move the zoom smoothly between steps
//! ([`seek`]); once they stop for [`SETTLE`], it glides to the nearest step. Every step
//! multiplies the width by the same [`STEP`], so each feels the same size;
//! step [`DEFAULT_LEVEL`] shows [`DEFAULT_SECONDS`], as before there was a
//! zoom.
//!
//! The mouse wheel or trackpad over a waveform, its edge bar, its right-click
//! menu, ⌘= / ⌘− / ⌘0 and View all go through [`set`] (from main.slint's
//! `WaveZoom`). Controllers use our control names on each deck's group:
//! `waveform_zoom` (1 to 20, higher is tighter), `waveform_zoom_up`,
//! `waveform_zoom_down` and `waveform_zoom_set_default`. Every deck's
//! controls set the one shared zoom.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use djsugar_control::{Behavior, ControlHandle, ControlRegistry};
use slint::ComponentHandle;

use crate::{settings, AppWindow, WaveZoom, NUM_DECKS};

/// The steps, 1 (widest) to `LEVELS` (tightest), shown as 5% to 100%.
pub const LEVELS: i32 = 20;
/// The step it starts on, and goes back to on reset: 40%.
pub const DEFAULT_LEVEL: i32 = 8;
/// Track seconds across a large waveform at [`DEFAULT_LEVEL`].
pub const DEFAULT_SECONDS: f64 = 10.0;
/// How much wider each step out is (the square root of 1.5, so two steps
/// are the 1.5 of the first version's ten): about 34 s across at step 1,
/// under a second at step 20, tight enough to set a grid on a kick by eye.
const STEP: f64 = 1.224_744_871_391_589;
/// How long the waveforms take to stretch to a new step.
const GLIDE: Duration = Duration::from_millis(150);
/// How quickly the zoom follows the wheel, trackpad or a drag: short, so it
/// keeps up, but long enough to smooth out a wheel's notches.
const FOLLOW: Duration = Duration::from_millis(90);
/// How long the wheel, trackpad or a drag must rest before the zoom settles
/// on the nearest step.
const SETTLE: Duration = Duration::from_millis(180);
const SETTING: &str = "waveform_zoom_steps";
/// The first version's ten steps, each two of these.
const OLD_SETTING: &str = "waveform_zoom";

/// Track seconds across a large waveform at `level`, at normal speed;
/// between steps while gliding.
pub fn seconds(level: f64) -> f64 {
    DEFAULT_SECONDS * STEP.powf(DEFAULT_LEVEL as f64 - level)
}

/// The zoom on its way to a step. Gliding in steps rather than seconds keeps
/// the stretch even: each step's share of the glide changes the width by
/// the same factor.
#[derive(Clone, Copy, Debug)]
struct Glide {
    from: f64,
    to: f64,
    start: Option<Instant>,
    length: Duration,
    /// When the wheel, trackpad or a drag last moved it, while it rests
    /// between steps.
    sought: Option<Instant>,
}

impl Glide {
    fn new(level: i32) -> Self {
        Self {
            from: level as f64,
            to: level as f64,
            start: None,
            length: GLIDE,
            sought: None,
        }
    }

    fn go(&mut self, level: i32, now: Instant) {
        self.glide(level as f64, GLIDE, now);
        self.sought = None;
    }

    /// Follows the wheel, trackpad or a drag to a point between steps.
    fn seek(&mut self, level: f64, now: Instant) {
        self.glide(level.clamp(1.0, LEVELS as f64), FOLLOW, now);
        self.sought = Some(now);
    }

    fn glide(&mut self, to: f64, length: Duration, now: Instant) {
        self.from = self.at(now);
        self.to = to;
        self.start = Some(now);
        self.length = length;
    }

    /// The step to settle on, once the zoom has rested between steps for
    /// [`SETTLE`].
    fn settle(&self, now: Instant) -> Option<i32> {
        let sought = self.sought?;
        (now.saturating_duration_since(sought) >= SETTLE).then(|| self.to.round() as i32)
    }

    /// Where it is now: quick at first, settling into the step.
    fn at(&self, now: Instant) -> f64 {
        let Some(start) = self.start else {
            return self.to;
        };
        let t = now.saturating_duration_since(start).as_secs_f64() / self.length.as_secs_f64();
        if t >= 1.0 {
            return self.to;
        }
        let eased = 1.0 - (1.0 - t).powi(3);
        self.from + (self.to - self.from) * eased
    }

    fn gliding(&self, now: Instant) -> bool {
        self.sought.is_some()
            || self
                .start
                .is_some_and(|s| now.saturating_duration_since(s) < self.length)
    }
}

thread_local! {
    /// The zoom, read by the renderer each frame (both on the UI thread).
    static ZOOM: RefCell<Glide> = RefCell::new(Glide::new(DEFAULT_LEVEL));
    /// Each deck's controller controls, and the versions last seen.
    static CONTROLS: RefCell<Vec<DeckControls>> = const { RefCell::new(Vec::new()) };
}

/// The seconds across the large waveforms now, and whether the zoom is
/// still gliding (so the next frame should come at once).
pub fn now(now: Instant) -> (f64, bool) {
    ZOOM.with(|z| {
        let z = z.borrow();
        (seconds(z.at(now)), z.gliding(now))
    })
}

/// One deck's zoom controls.
struct DeckControls {
    zoom: ControlHandle,
    up: ControlHandle,
    down: ControlHandle,
    reset: ControlHandle,
    seen: [u32; 4],
}

impl DeckControls {
    fn versions(&self) -> [u32; 4] {
        [
            self.zoom.version(),
            self.up.version(),
            self.down.version(),
            self.reset.version(),
        ]
    }
}

/// The deck groups' zoom controls, for `--list-controls` and controllers.
pub fn create_controls(registry: &ControlRegistry) -> Vec<[ControlHandle; 4]> {
    (1..=NUM_DECKS)
        .map(|d| {
            let g = format!("[Channel{d}]");
            [
                registry.create(
                    &g,
                    "waveform_zoom",
                    DEFAULT_LEVEL as f64,
                    1.0,
                    LEVELS as f64,
                    Behavior::Linear,
                ),
                registry.push(&g, "waveform_zoom_up"),
                registry.push(&g, "waveform_zoom_down"),
                registry.push(&g, "waveform_zoom_set_default"),
            ]
        })
        .collect()
}

/// Reads the saved step, shows it, and takes changes from the window and
/// controllers. Keep the returned timer: it watches the controllers.
pub fn install(app: &AppWindow, registry: &ControlRegistry) -> slint::Timer {
    let level = clamp(saved(&settings::load()));
    ZOOM.with(|z| *z.borrow_mut() = Glide::new(level));
    let ui = app.global::<WaveZoom>();
    ui.set_steps(LEVELS);
    ui.set_default_level(DEFAULT_LEVEL);
    ui.set_level(level);
    ui.set_seconds(seconds(level as f64) as f32);
    let decks = create_controls(registry)
        .into_iter()
        .map(|[zoom, up, down, reset]| {
            zoom.set(level as f64);
            let mut c = DeckControls {
                zoom,
                up,
                down,
                reset,
                seen: [0; 4],
            };
            c.seen = c.versions();
            c
        })
        .collect();
    CONTROLS.with(|c| *c.borrow_mut() = decks);
    let weak = app.as_weak();
    ui.on_set_level(move |level| {
        if let Some(app) = weak.upgrade() {
            set(&app, level);
        }
    });
    let weak = app.as_weak();
    ui.on_seek(move |level| {
        if let Some(app) = weak.upgrade() {
            seek(&app, level as f64);
        }
    });
    let weak = app.as_weak();
    ui.on_nudge(move |by| {
        if let Some(app) = weak.upgrade() {
            let to = ZOOM.with(|z| z.borrow().to);
            seek(&app, to + by as f64);
        }
    });
    let timer = slint::Timer::default();
    let weak = app.as_weak();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(16),
        move || {
            let Some(app) = weak.upgrade() else {
                return;
            };
            if let Some(level) = from_controllers() {
                set(&app, level);
            } else if let Some(level) = ZOOM.with(|z| z.borrow().settle(Instant::now())) {
                settle(&app, level);
            }
        },
    );
    timer
}

fn clamp(level: i32) -> i32 {
    level.clamp(1, LEVELS)
}

/// The saved step: the twenty-step one, else the first version's ten-step
/// one doubled (its 40% is this one's 40%).
fn saved(settings: &serde_json::Value) -> i32 {
    if let Some(n) = settings[SETTING].as_f64() {
        return n.round() as i32;
    }
    settings[OLD_SETTING]
        .as_f64()
        .map_or(DEFAULT_LEVEL, |n| n.round() as i32 * 2)
}

fn level() -> i32 {
    ZOOM.with(|z| z.borrow().to.round() as i32)
}

/// A step a controller asked for since the last look, if any.
fn from_controllers() -> Option<i32> {
    CONTROLS.with(|c| {
        let mut asked = None;
        for d in c.borrow_mut().iter_mut() {
            let now = d.versions();
            let seen = std::mem::replace(&mut d.seen, now);
            let pressed = |i: usize, h: &ControlHandle| now[i] != seen[i] && h.get_bool();
            if now[0] != seen[0] {
                asked = Some(d.zoom.get().round() as i32);
            }
            let current = asked.unwrap_or_else(level);
            if pressed(1, &d.up) {
                asked = Some(current + 1);
            } else if pressed(2, &d.down) {
                asked = Some(current - 1);
            } else if pressed(3, &d.reset) {
                asked = Some(DEFAULT_LEVEL);
            }
        }
        asked.map(clamp).filter(|l| *l != level())
    })
}

/// Zooms every large waveform to `level`: glides there, shows it, saves it
/// and tells controllers.
pub fn set(app: &AppWindow, level: i32) {
    let level = clamp(level);
    let resting = ZOOM.with(|z| {
        let z = z.borrow();
        z.sought.is_none() && z.to == level as f64
    });
    if resting {
        return;
    }
    settle(app, level);
}

/// Glides to `level` from wherever the zoom is, step or not, and shows,
/// saves and passes on the step.
fn settle(app: &AppWindow, level: i32) {
    ZOOM.with(|z| z.borrow_mut().go(level, Instant::now()));
    let ui = app.global::<WaveZoom>();
    ui.set_level(level);
    ui.set_changes(ui.get_changes().wrapping_add(1));
    settings::set(SETTING, serde_json::json!(level));
    CONTROLS.with(|c| {
        for d in c.borrow_mut().iter_mut() {
            d.zoom.set(level as f64);
            d.seen = d.versions();
        }
    });
    app.window().request_redraw();
}

/// Follows the wheel, trackpad or a drag to `level`, which may fall between
/// steps; the zoom settles on the nearest step once they stop.
pub fn seek(app: &AppWindow, level: f64) {
    ZOOM.with(|z| z.borrow_mut().seek(level, Instant::now()));
    let ui = app.global::<WaveZoom>();
    let shown = clamp(level.round() as i32);
    if ui.get_level() != shown {
        ui.set_level(shown);
    }
    ui.set_changes(ui.get_changes().wrapping_add(1));
    app.window().request_redraw();
}

/// One step in (1) or out (-1).
pub fn step(app: &AppWindow, by: i32) {
    set(app, level() + by);
}

pub fn reset(app: &AppWindow) {
    set(app, DEFAULT_LEVEL);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_step_shows_ten_seconds() {
        assert_eq!(seconds(DEFAULT_LEVEL as f64), 10.0);
    }

    #[test]
    fn the_first_versions_zoom_keeps_its_width() {
        assert_eq!(saved(&serde_json::json!({ "waveform_zoom": 6 })), 12);
        assert_eq!(
            saved(&serde_json::json!({ "waveform_zoom": 6, "waveform_zoom_steps": 9 })),
            9
        );
        assert_eq!(saved(&serde_json::Value::Null), DEFAULT_LEVEL);
    }

    #[test]
    fn a_seek_settles_on_the_nearest_step_once_it_rests() {
        let t0 = Instant::now();
        let mut g = Glide::new(8);
        g.seek(9.4, t0);
        assert!(g.gliding(t0 + SETTLE * 2));
        assert_eq!(g.settle(t0 + SETTLE / 2), None);
        assert_eq!(g.settle(t0 + SETTLE), Some(9));
        g.go(9, t0 + SETTLE);
        assert_eq!(g.settle(t0 + SETTLE * 3), None);
        assert_eq!(g.at(t0 + SETTLE + GLIDE), 9.0);
    }

    #[test]
    fn each_step_in_is_the_same_factor_tighter() {
        for l in 1..LEVELS {
            let ratio = seconds(l as f64) / seconds(l as f64 + 1.0);
            assert!((ratio - STEP).abs() < 1e-9);
        }
        assert!(seconds(1.0) > 30.0);
        assert!(seconds(LEVELS as f64) < 1.0);
    }

    #[test]
    fn a_glide_reaches_its_step_and_stops() {
        let t0 = Instant::now();
        let mut g = Glide::new(4);
        g.go(6, t0);
        assert!(g.gliding(t0));
        let mid = g.at(t0 + GLIDE / 2);
        assert!(mid > 4.0 && mid < 6.0, "{mid}");
        assert_eq!(g.at(t0 + GLIDE), 6.0);
        assert!(!g.gliding(t0 + GLIDE));
    }

    #[test]
    fn a_new_step_mid_glide_starts_where_the_zoom_is() {
        let t0 = Instant::now();
        let mut g = Glide::new(4);
        g.go(5, t0);
        let t1 = t0 + GLIDE / 3;
        let there = g.at(t1);
        g.go(6, t1);
        assert_eq!(g.at(t1), there);
        assert_eq!(g.at(t1 + GLIDE), 6.0);
    }
}

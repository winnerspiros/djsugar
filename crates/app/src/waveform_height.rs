//! How tall the large scrolling waveforms are: ten steps, 1 to 10, with 3
//! the height they always had. Each layout keeps its own step (tall
//! waveforms in 2-Deck don't make 4-Deck taller), saved as one list by
//! layout number.
//!
//! The window works out the heights each step means in each layout, and
//! shows a lower step while the chosen one would leave the library too
//! little room (main.slint). Here is only the choice: from the grip under
//! the waveforms, the waveforms button's menu, a waveform's right-click
//! menu, ⌥⌘↑ / ⌥⌘↓, View, and controllers through `[Skin],waveform_height`
//! (1 to 10), `waveform_height_up`, `waveform_height_down` and
//! `waveform_height_set_default`, which act on the layout showing.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use djsugar_control::{Behavior, ControlHandle, ControlRegistry};
use serde_json::Value;
use slint::{ComponentHandle, Model, ModelRc, VecModel};

use crate::{settings, AppWindow, WaveHeight};

/// The steps, 1 (lowest) to `LEVELS` (tallest).
pub const LEVELS: i32 = 10;
/// The height the waveforms had before they could change.
pub const DEFAULT_LEVEL: i32 = 3;
/// Layout numbers 0 to 4 (main.slint's `layout`); Player (1) has no large
/// waveforms but keeps its place in the list.
const LAYOUTS: usize = 5;
const SETTING: &str = "waveform_height_steps";
/// The first version's four sizes (Small, Medium, Large, Extra large), and
/// the steps they became.
const OLD_SETTING: &str = "waveform_height";
const OLD_SIZES: [i32; 4] = [1, 3, 6, 10];

fn clamp(level: i32) -> i32 {
    level.clamp(1, LEVELS)
}

/// Each layout's step from the saved settings: the ten-step list, else the
/// first version's four sizes moved to their steps.
fn saved(settings: &Value) -> [i32; LAYOUTS] {
    if settings[SETTING].is_array() {
        return parse(&settings[SETTING]);
    }
    let mut levels = [DEFAULT_LEVEL; LAYOUTS];
    if let Some(list) = settings[OLD_SETTING].as_array() {
        for (slot, v) in levels.iter_mut().zip(list) {
            if let Some(n) = v.as_f64() {
                *slot = OLD_SIZES[(n.round() as i32).clamp(1, 4) as usize - 1];
            }
        }
    }
    levels
}

/// Each layout's step from the saved list; 3 where it has none.
fn parse(saved: &Value) -> [i32; LAYOUTS] {
    let mut levels = [DEFAULT_LEVEL; LAYOUTS];
    if let Some(list) = saved.as_array() {
        for (slot, v) in levels.iter_mut().zip(list) {
            if let Some(n) = v.as_f64() {
                *slot = clamp(n.round() as i32);
            }
        }
    }
    levels
}

/// The controller controls, for `--list-controls` and controllers.
pub fn create_controls(registry: &ControlRegistry) -> [ControlHandle; 4] {
    [
        registry.create(
            "[Skin]",
            "waveform_height",
            DEFAULT_LEVEL as f64,
            1.0,
            LEVELS as f64,
            Behavior::Linear,
        ),
        registry.push("[Skin]", "waveform_height_up"),
        registry.push("[Skin]", "waveform_height_down"),
        registry.push("[Skin]", "waveform_height_set_default"),
    ]
}

thread_local! {
    /// The controls, so a change from the window shows on them.
    static HEIGHT_CONTROL: RefCell<Option<ControlHandle>> = const { RefCell::new(None) };
}

/// Reads the saved sizes, shows them, and takes changes from the window
/// and controllers. Keep the returned timer: it watches the controllers.
pub fn install(app: &AppWindow, registry: &ControlRegistry) -> slint::Timer {
    let levels = saved(&settings::load());
    let ui = app.global::<WaveHeight>();
    ui.set_steps(LEVELS);
    ui.set_default_level(DEFAULT_LEVEL);
    ui.set_levels(ModelRc::from(Rc::new(VecModel::from(levels.to_vec()))));
    let weak = app.as_weak();
    ui.on_set_level(move |layout, level| {
        if let Some(app) = weak.upgrade() {
            set(&app, layout, level);
        }
    });

    let [height, up, down, reset] = create_controls(registry);
    HEIGHT_CONTROL.with(|c| *c.borrow_mut() = Some(height.clone()));
    let versions = |h: &ControlHandle, u: &ControlHandle, d: &ControlHandle, r: &ControlHandle| {
        [h.version(), u.version(), d.version(), r.version()]
    };
    // The layout the controls last showed, so a layout change shows its size.
    let shown_layout = Cell::new(-1);
    let seen = Cell::new([0u32; 4]);
    let timer = slint::Timer::default();
    let weak = app.as_weak();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(16),
        move || {
            let Some(app) = weak.upgrade() else {
                return;
            };
            let layout = app.global::<WaveHeight>().get_layout();
            if shown_layout.replace(layout) != layout {
                height.set(level(&app, layout) as f64);
                seen.set(versions(&height, &up, &down, &reset));
                return;
            }
            let now = versions(&height, &up, &down, &reset);
            let before = seen.replace(now);
            if now == before {
                return;
            }
            let current = level(&app, layout);
            let pressed = |i: usize, h: &ControlHandle| now[i] != before[i] && h.get_bool();
            let asked = if now[0] != before[0] {
                height.get().round() as i32
            } else if pressed(1, &up) {
                current + 1
            } else if pressed(2, &down) {
                current - 1
            } else if pressed(3, &reset) {
                DEFAULT_LEVEL
            } else {
                current
            };
            set(&app, layout, asked);
            // Back in range on the control if it was asked for too much.
            height.set(level(&app, layout) as f64);
            seen.set(versions(&height, &up, &down, &reset));
        },
    );
    timer
}

/// The step `layout` has now.
fn level(app: &AppWindow, layout: i32) -> i32 {
    let levels = app.global::<WaveHeight>().get_levels();
    usize::try_from(layout)
        .ok()
        .and_then(|l| levels.row_data(l))
        .unwrap_or(DEFAULT_LEVEL)
}

/// Sets `layout`'s step: shows it, saves it and tells controllers.
pub fn set(app: &AppWindow, layout: i32, level: i32) {
    let level = clamp(level);
    let Ok(index) = usize::try_from(layout) else {
        return;
    };
    if index >= LAYOUTS || self::level(app, layout) == level {
        return;
    }
    let ui = app.global::<WaveHeight>();
    let levels = ui.get_levels();
    levels.set_row_data(index, level);
    ui.set_changes(ui.get_changes().wrapping_add(1));
    let all: Vec<i32> = levels.iter().collect();
    settings::set(SETTING, serde_json::json!(all));
    if ui.get_layout() == layout {
        HEIGHT_CONTROL.with(|c| {
            if let Some(c) = c.borrow().as_ref() {
                c.set(level as f64);
            }
        });
    }
}

/// One step taller (1) or shorter (-1) in the layout showing.
pub fn step(app: &AppWindow, by: i32) {
    let layout = app.global::<WaveHeight>().get_layout();
    set(app, layout, level(app, layout) + by);
}

/// Back to step 3 in the layout showing.
pub fn reset(app: &AppWindow) {
    let layout = app.global::<WaveHeight>().get_layout();
    set(app, layout, DEFAULT_LEVEL);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_saved_is_medium_everywhere() {
        assert_eq!(parse(&Value::Null), [DEFAULT_LEVEL; LAYOUTS]);
    }

    #[test]
    fn saved_steps_come_back_by_layout() {
        let saved = serde_json::json!([10, 3, 1, 7, 5]);
        assert_eq!(parse(&saved), [10, 3, 1, 7, 5]);
    }

    #[test]
    fn odd_saved_values_are_kept_in_range() {
        let saved = serde_json::json!([19, -1, "big", 2.6]);
        assert_eq!(parse(&saved), [10, 1, DEFAULT_LEVEL, 3, DEFAULT_LEVEL]);
    }

    #[test]
    fn the_four_sizes_move_to_their_steps() {
        let settings = serde_json::json!({ "waveform_height": [4, 2, 1, 3, 2] });
        assert_eq!(saved(&settings), [10, 3, 1, 6, 3]);
    }

    #[test]
    fn the_ten_steps_win_over_the_four_sizes() {
        let settings = serde_json::json!({
            "waveform_height": [4, 4, 4, 4, 4],
            "waveform_height_steps": [8, 3, 3, 3, 3],
        });
        assert_eq!(saved(&settings), [8, 3, 3, 3, 3]);
    }
}

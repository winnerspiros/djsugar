//! Settings › Appearance: Dark, Light or Match system.
//!
//! Sets `Theme.light`, which every Theme colour reads, and the window's
//! native appearance so the title bar matches. Under Match system the
//! computer's setting is followed, also when it changes while DJ Sugar runs
//! (winit's `ThemeChanged`, sent on macOS and Windows; elsewhere the
//! system's look is unknown and Dark is used).

use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use serde_json::json;
use slint::winit_030::winit::window::Theme as WindowTheme;
use slint::winit_030::WinitWindowAccessor;
use slint::ComponentHandle;

use crate::{settings, AppWindow, LayoutsUi, Theme};

pub const KEY: &str = "appearance";
/// In Settings order; the index is what's saved.
pub const LABELS: [&str; 3] = ["Dark", "Light", "Match system"];
const DARK: u8 = 0;
const LIGHT: u8 = 1;
const SYSTEM: u8 = 2;

static MODE: AtomicU8 = AtomicU8::new(DARK);

/// The saved choice, as an index into [`LABELS`].
pub fn mode() -> usize {
    MODE.load(Ordering::Relaxed) as usize
}

/// Reads the saved choice and applies it, again once the window is up, as
/// the system's look can only be asked of a window that exists.
pub fn install(app: &AppWindow) {
    let saved = settings::number(KEY, 0.0).clamp(0.0, SYSTEM as f64) as u8;
    MODE.store(saved, Ordering::Relaxed);
    apply(app);
    let weak = app.as_weak();
    slint::Timer::single_shot(Duration::ZERO, move || {
        if let Some(app) = weak.upgrade() {
            apply(&app);
        }
    });
}

/// Picked in Settings or the layout menu.
pub fn choose(app: &AppWindow, index: usize) {
    let m = index.min(SYSTEM as usize) as u8;
    MODE.store(m, Ordering::Relaxed);
    settings::set(KEY, json!(m));
    apply(app);
}

/// The computer switched between light and dark.
pub fn system_changed(app: &AppWindow, theme: WindowTheme) {
    if MODE.load(Ordering::Relaxed) == SYSTEM {
        app.global::<Theme>().set_light(theme == WindowTheme::Light);
    }
}

fn apply(app: &AppWindow) {
    let m = MODE.load(Ordering::Relaxed);
    let light = app
        .window()
        .with_winit_window(|w| {
            w.set_theme(match m {
                DARK => Some(WindowTheme::Dark),
                LIGHT => Some(WindowTheme::Light),
                _ => None,
            });
            match m {
                DARK => false,
                LIGHT => true,
                _ => w.theme() == Some(WindowTheme::Light),
            }
        })
        .unwrap_or(m == LIGHT);
    app.global::<Theme>().set_light(light);
    app.global::<LayoutsUi>().set_appearance(m as i32);
}

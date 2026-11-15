//! How the decks' waveforms look: the colour scheme, the stacked bands'
//! palette, whether they are drawn whole or from the bottom up, and where
//! the large waveforms' play line (the needle) is.
//!
//! Picked in the waveforms button's drop-down and kept here, so the renderer can read it
//! every frame without reading the settings file; [`version`] changes with
//! every pick so the renderer applies it once.

use std::sync::atomic::{AtomicU32, Ordering};

use djsugar_waveform::{Palette, Style, BAND_PALETTES};

use crate::settings;

pub const COLOR_KEY: &str = "waveform_color";
pub const PALETTE_KEY: &str = "waveform_palette";
/// Full or Half, for the large waveforms and the overviews alike (the key
/// keeps its first name, from when only the overview could be half).
pub const HALF_KEY: &str = "waveform_overview_half";
/// The needle: 0 in the middle, 1 a quarter in from the left.
pub const NEEDLE_KEY: &str = "waveform_needle";
/// Settings › Appearance › Size follows gain, on at first: a deck's gain
/// knob sizes its large waveform (not the overview).
pub const GAIN_KEY: &str = "waveform_follows_gain";
/// Vocals, in the waveforms button's drop-down: what runs along the bottom
/// of the large waveforms, one of [`VOCALS_LABELS`]. Lyrics at first.
pub const VOCALS_KEY: &str = "waveform_vocals";
/// The switches Vocals took over: Show vocals (the cyan strip) and Show
/// lyrics. Read once, when Vocals has never been picked.
const OLD_BAR_KEY: &str = "waveform_vocal_tint";
const OLD_LYRICS_KEY: &str = "waveform_lyrics";
/// What runs along the bottom of the large waveforms, never both: nothing,
/// a cyan bar where someone sings, or the timed lyrics.
pub const VOCALS_LABELS: [&str; 3] = ["Off", "Bar", "Lyrics"];
const VOCALS_BAR: u32 = 1;
const VOCALS_LYRICS: u32 = 2;
/// Phrases, in the waveforms button's drop-down: the overviews' phrase bar
/// and the large waveforms' phrase flags, each kept under its own key.
pub const PHRASES_LABELS: [&str; 4] = ["Off", "Overview", "Waveform", "Both"];
const PHRASE_BAR_KEY: &str = "phrases_visible";
const PHRASE_FLAGS_KEY: &str = "phrase_flags_visible";

/// The color schemes, in the drop-down's order.
pub const COLOR_LABELS: [&str; 3] = ["RGB", "3-Band", "Stems"];
/// What each scheme is saved as: Stems kept the 3 it had beside the Blue
/// scheme (2), which is gone; a saved Blue comes back as RGB.
const COLOR_SAVED: [u32; 3] = [0, 1, 3];
/// The stacked bands' palettes, in [`BAND_PALETTES`] order.
pub const PALETTE_LABELS: [&str; BAND_PALETTES.len()] = ["Blue-Orange-White", "Lilac-Cyan-White"];
pub const SHAPE_LABELS: [&str; 2] = ["Full", "Half"];
/// Where the needle is, as a fraction of the waveform's width, for Center
/// and Left.
pub const NEEDLE_AT: [f32; 2] = [0.5, 0.25];

static COLOR: AtomicU32 = AtomicU32::new(0);
static PALETTE: AtomicU32 = AtomicU32::new(0);
static HALF: AtomicU32 = AtomicU32::new(0);
static NEEDLE: AtomicU32 = AtomicU32::new(0);
static VERSION: AtomicU32 = AtomicU32::new(1);
static GAIN: AtomicU32 = AtomicU32::new(1);
static VOCALS: AtomicU32 = AtomicU32::new(VOCALS_LYRICS);

/// Reads the saved choices; called once at start.
pub fn load() {
    let read = |key, len: usize| (settings::number(key, 0.0).max(0.0) as usize).min(len - 1) as u32;
    let saved = settings::number(COLOR_KEY, 0.0).max(0.0) as u32;
    COLOR.store(color_from_saved(saved) as u32, Ordering::Relaxed);
    PALETTE.store(read(PALETTE_KEY, PALETTE_LABELS.len()), Ordering::Relaxed);
    HALF.store(read(HALF_KEY, SHAPE_LABELS.len()), Ordering::Relaxed);
    NEEDLE.store(read(NEEDLE_KEY, NEEDLE_AT.len()), Ordering::Relaxed);
    GAIN.store(settings::flag(GAIN_KEY, true) as u32, Ordering::Relaxed);
    VOCALS.store(saved_vocals(), Ordering::Relaxed);
    crate::browser::set_wave_lyrics(vocals() == VOCALS_LYRICS as usize);
    VERSION.fetch_add(1, Ordering::Relaxed);
}

fn pick(cell: &AtomicU32, key: &str, index: usize, len: usize) {
    let i = index.min(len - 1) as u32;
    cell.store(i, Ordering::Relaxed);
    settings::set(key, serde_json::json!(i));
    VERSION.fetch_add(1, Ordering::Relaxed);
}

/// Vocals as saved, or from the two switches it replaced: the bar if Show
/// vocals was on (it was off at first), else the lyrics unless Show lyrics
/// was switched off.
fn saved_vocals() -> u32 {
    let saved = settings::load();
    if let Some(n) = saved[VOCALS_KEY].as_u64() {
        return (n as usize).min(VOCALS_LABELS.len() - 1) as u32;
    }
    vocals_from_switches(
        saved[OLD_BAR_KEY].as_bool().unwrap_or(false),
        saved[OLD_LYRICS_KEY].as_bool().unwrap_or(true),
    )
}

fn vocals_from_switches(bar: bool, lyrics: bool) -> u32 {
    match (bar, lyrics) {
        (true, _) => VOCALS_BAR,
        (false, true) => VOCALS_LYRICS,
        (false, false) => 0,
    }
}

/// The drop-down's scheme for a saved value: RGB for anything unknown,
/// and for the Blue scheme that was dropped.
fn color_from_saved(saved: u32) -> usize {
    COLOR_SAVED.iter().position(|&v| v == saved).unwrap_or(0)
}

pub fn choose_color(index: usize) {
    let i = index.min(COLOR_LABELS.len() - 1);
    COLOR.store(i as u32, Ordering::Relaxed);
    settings::set(COLOR_KEY, serde_json::json!(COLOR_SAVED[i]));
    VERSION.fetch_add(1, Ordering::Relaxed);
}

pub fn choose_palette(index: usize) {
    pick(&PALETTE, PALETTE_KEY, index, PALETTE_LABELS.len());
}

pub fn choose_half(index: usize) {
    pick(&HALF, HALF_KEY, index, SHAPE_LABELS.len());
}

pub fn choose_needle(index: usize) {
    pick(&NEEDLE, NEEDLE_KEY, index, NEEDLE_AT.len());
}

/// Size follows gain was switched; saved by the caller.
pub fn set_follows_gain(on: bool) {
    GAIN.store(on as u32, Ordering::Relaxed);
}

pub fn follows_gain() -> bool {
    GAIN.load(Ordering::Relaxed) == 1
}

/// Picks what runs along the bottom of the large waveforms, and saves it.
pub fn choose_vocals(index: usize) {
    let i = index.min(VOCALS_LABELS.len() - 1) as u32;
    VOCALS.store(i, Ordering::Relaxed);
    settings::set(VOCALS_KEY, serde_json::json!(i));
    crate::browser::set_wave_lyrics(i == VOCALS_LYRICS);
    VERSION.fetch_add(1, Ordering::Relaxed);
}

/// Which of [`VOCALS_LABELS`] is in use.
pub fn vocals() -> usize {
    VOCALS.load(Ordering::Relaxed) as usize
}

/// Whether the large waveforms mark the sung parts with a cyan bar.
pub fn show_vocals() -> bool {
    VOCALS.load(Ordering::Relaxed) == VOCALS_BAR
}

/// Which of [`PHRASES_LABELS`] the overviews' phrase bar and the large
/// waveforms' flags make.
fn phrases_index(bar: bool, flags: bool) -> usize {
    bar as usize + 2 * flags as usize
}

/// How tall a deck's waveforms are drawn for its gain knob (1 at the
/// middle, 0 to 4): unchanged unless Size follows gain is on, and then kept
/// between a quarter and twice the height so a waveform never vanishes.
pub fn gain_scale(pregain: f64) -> f32 {
    if follows_gain() {
        pregain.clamp(0.25, 2.0) as f32
    } else {
        1.0
    }
}

/// Which of [`COLOR_LABELS`] is in use.
pub fn color() -> usize {
    COLOR.load(Ordering::Relaxed) as usize
}

/// Which of [`PALETTE_LABELS`] is in use.
pub fn palette_index() -> usize {
    PALETTE.load(Ordering::Relaxed) as usize
}

/// Whether the waveforms are drawn from the bottom up.
pub fn half() -> bool {
    HALF.load(Ordering::Relaxed) == 1
}

/// Where the large waveforms' play line is, as a fraction of their width.
pub fn needle() -> f32 {
    NEEDLE_AT[(NEEDLE.load(Ordering::Relaxed) as usize).min(NEEDLE_AT.len() - 1)]
}

/// Shows the choices in the waveforms button's drop-down and takes its
/// picks.
pub fn install(app: &crate::AppWindow) {
    use slint::ComponentHandle;
    let ui = app.global::<crate::WaveLook>();
    show(&ui);
    let weak = app.as_weak();
    let after = move |choose: &dyn Fn()| {
        choose();
        if let Some(app) = weak.upgrade() {
            show(&app.global::<crate::WaveLook>());
            app.window().request_redraw();
        }
    };
    let a = after.clone();
    ui.on_pick_color(move |i| a(&|| choose_color(i.max(0) as usize)));
    let a = after.clone();
    ui.on_pick_palette(move |i| a(&|| choose_palette(i.max(0) as usize)));
    let a = after.clone();
    ui.on_pick_half(move |half| a(&|| choose_half(half as usize)));
    let a = after.clone();
    ui.on_pick_needle_left(move |left| a(&|| choose_needle(left as usize)));
    let a = after.clone();
    ui.on_pick_vocals(move |i| a(&|| choose_vocals(i.max(0) as usize)));
    // Phrases: the overviews' bar and the large waveforms' flags.
    let phrases = app.global::<crate::PhraseView>();
    phrases.set_visible(settings::flag(PHRASE_BAR_KEY, true));
    phrases.set_waveform_visible(settings::flag(PHRASE_FLAGS_KEY, true));
    ui.set_phrases(phrases_index(phrases.get_visible(), phrases.get_waveform_visible()) as i32);
    let weak = app.as_weak();
    ui.on_pick_phrases(move |i| {
        let i = (i.max(0) as usize).min(PHRASES_LABELS.len() - 1);
        let (bar, flags) = (i & 1 == 1, i & 2 == 2);
        settings::set(PHRASE_BAR_KEY, serde_json::json!(bar));
        settings::set(PHRASE_FLAGS_KEY, serde_json::json!(flags));
        if let Some(app) = weak.upgrade() {
            let phrases = app.global::<crate::PhraseView>();
            phrases.set_visible(bar);
            phrases.set_waveform_visible(flags);
            app.global::<crate::WaveLook>().set_phrases(i as i32);
            app.window().request_redraw();
        }
    });
}

fn show(ui: &crate::WaveLook) {
    ui.set_color(color() as i32);
    ui.set_palette(palette_index() as i32);
    ui.set_half(half());
    ui.set_needle(needle());
    ui.set_vocals(vocals() as i32);
}

/// Changes whenever a choice does.
pub fn version() -> u32 {
    VERSION.load(Ordering::Relaxed)
}

/// The waveform style for the color scheme in use.
pub fn style() -> Style {
    match color() {
        1 => Style::Bands,
        2 => Style::Stems,
        _ => Style::Rgb,
    }
}

/// The band colours: RGB keeps its red, amber and blue; 3-Band uses the
/// palette picked.
pub fn palette() -> Palette {
    match style() {
        Style::Bands => Palette::bands(palette_index()),
        _ => Palette::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheme_sets_style_and_palette() {
        COLOR.store(1, Ordering::Relaxed);
        PALETTE.store(1, Ordering::Relaxed);
        assert_eq!(style(), Style::Bands);
        assert_eq!(palette(), Palette::bands(1));
        COLOR.store(0, Ordering::Relaxed);
        assert_eq!(style(), Style::Rgb);
        assert_eq!(palette(), Palette::default());
    }

    #[test]
    fn saved_schemes_come_back_without_blue() {
        assert_eq!(color_from_saved(0), 0);
        assert_eq!(color_from_saved(1), 1);
        // The dropped Blue scheme comes back as RGB; Stems keeps its 3.
        assert_eq!(color_from_saved(2), 0);
        assert_eq!(color_from_saved(3), 2);
        assert_eq!(color_from_saved(9), 0);
        COLOR.store(2, Ordering::Relaxed);
        assert_eq!(style(), Style::Stems);
        COLOR.store(0, Ordering::Relaxed);
    }

    #[test]
    fn needle_is_center_or_a_quarter_in() {
        NEEDLE.store(0, Ordering::Relaxed);
        assert_eq!(needle(), 0.5);
        NEEDLE.store(1, Ordering::Relaxed);
        assert_eq!(needle(), 0.25);
        NEEDLE.store(7, Ordering::Relaxed);
        assert_eq!(needle(), 0.25);
        NEEDLE.store(0, Ordering::Relaxed);
    }

    #[test]
    fn vocals_come_from_the_old_switches() {
        // Show vocals on wins: the bar and the lyrics never show together.
        assert_eq!(vocals_from_switches(true, true), VOCALS_BAR);
        assert_eq!(vocals_from_switches(true, false), VOCALS_BAR);
        assert_eq!(vocals_from_switches(false, true), VOCALS_LYRICS);
        assert_eq!(vocals_from_switches(false, false), 0);
    }

    #[test]
    fn phrases_pick_bar_flags_or_both() {
        assert_eq!(PHRASES_LABELS[phrases_index(false, false)], "Off");
        assert_eq!(PHRASES_LABELS[phrases_index(true, false)], "Overview");
        assert_eq!(PHRASES_LABELS[phrases_index(false, true)], "Waveform");
        assert_eq!(PHRASES_LABELS[phrases_index(true, true)], "Both");
    }

    #[test]
    fn gain_sizes_only_when_on() {
        set_follows_gain(false);
        assert_eq!(gain_scale(2.0), 1.0);
        set_follows_gain(true);
        assert_eq!(gain_scale(1.5), 1.5);
        assert_eq!(gain_scale(0.0), 0.25);
        assert_eq!(gain_scale(4.0), 2.0);
        set_follows_gain(false);
    }
}

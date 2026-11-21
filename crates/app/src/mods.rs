//! Mods in the app (see the `djsugar-mods` crate): the app's own, shipped in
//! its `mods` folder, and the DJ's, in `mods` in the settings folder. Both
//! are watched: adding, editing or removing a mod takes effect within a
//! second, without a restart.
//!
//! What a mod can set so far: any property of the `Theme` global, as
//! `theme.<name>` (both appearances), `theme.dark.<name>` or
//! `theme.light.<name>`. Colours are `"#rrggbb"` or `"#rrggbbaa"`, lengths
//! are numbers of logical pixels.

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use djsugar_mods::{Registry, Value};
use slint::ComponentHandle;

use crate::{AppWindow, Theme};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Color,
    Length,
    Bool,
    Text,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Prop {
    Color(slint::Color),
    Length(f32),
    Bool(bool),
    Text(slint::SharedString),
}

mod theme_props {
    use super::{Kind, Prop};
    include!(concat!(env!("OUT_DIR"), "/theme_props.rs"));
}

/// The app's own mods: `DJSUGAR_MODS`, else beside the program as each
/// system's package puts them, else the source tree's `res/mods`.
pub fn bundled_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("DJSUGAR_MODS") {
        return Some(PathBuf::from(dir));
    }
    let mut candidates = Vec::new();
    if let Some(exe) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(PathBuf::from))
    {
        candidates.push(exe.join("../Resources/mods"));
        candidates.push(exe.join("mods"));
        candidates.push(exe.join("../share/djsugar/mods"));
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../res/mods"));
    candidates.into_iter().find(|d| d.is_dir())
}

/// The DJ's own mods, made on first use.
pub fn user_dir() -> Option<PathBuf> {
    let dir = crate::settings::config_dir()?.join("mods");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir)
}

fn folders() -> Vec<PathBuf> {
    bundled_dir().into_iter().chain(user_dir()).collect()
}

/// Mods the DJ turned off (Settings › Mods), by id.
fn disabled() -> BTreeSet<String> {
    crate::settings::load()["mods_disabled"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// `#rgb`, `#rrggbb` or `#rrggbbaa`.
pub fn parse_color(s: &str) -> Option<slint::Color> {
    let hex = s.trim().strip_prefix('#')?;
    let digit = |i: usize, n: usize| u8::from_str_radix(hex.get(i..i + n)?, 16).ok();
    match hex.len() {
        3 => {
            let d = |i| digit(i, 1).map(|v| v * 17);
            Some(slint::Color::from_rgb_u8(d(0)?, d(1)?, d(2)?))
        }
        6 => Some(slint::Color::from_rgb_u8(
            digit(0, 2)?,
            digit(2, 2)?,
            digit(4, 2)?,
        )),
        8 => Some(slint::Color::from_argb_u8(
            digit(6, 2)?,
            digit(0, 2)?,
            digit(2, 2)?,
            digit(4, 2)?,
        )),
        _ => None,
    }
}

fn convert(kind: Kind, v: &Value) -> Option<Prop> {
    Some(match kind {
        Kind::Color => Prop::Color(parse_color(v.text()?)?),
        Kind::Length => Prop::Length(v.number()? as f32),
        Kind::Bool => Prop::Bool(v.flag()?),
        Kind::Text => Prop::Text(v.text()?.into()),
    })
}

/// The Theme property a mod sets for the appearance in use, if any.
fn wanted<'a>(reg: &'a Registry, name: &str, light: bool) -> Option<&'a Value> {
    let mode = if light { "light" } else { "dark" };
    reg.get(&format!("theme.{mode}.{name}"))
        .or_else(|| reg.get(&format!("theme.{name}")))
}

/// State the app's appearance is the DJ's to switch, not a mod's.
const NOT_MODDABLE: [&str; 2] = ["light", "player"];

struct Applied {
    /// Every property's own value, Dark and Light, before any mod.
    defaults: HashMap<&'static str, (Prop, Prop)>,
    /// Properties a mod has set, to put back when it stops setting them.
    touched: BTreeSet<&'static str>,
}

impl Applied {
    fn capture(theme: &Theme<'_>) -> Applied {
        let was = theme.get_light();
        let read = |light: bool| -> HashMap<&'static str, Prop> {
            theme.set_light(light);
            theme_props::PROPS
                .iter()
                .filter_map(|(n, _)| theme_props::get(theme, n).map(|p| (*n, p)))
                .collect()
        };
        let (dark, mut light) = (read(false), read(true));
        theme.set_light(was);
        let defaults = dark
            .into_iter()
            .filter_map(|(n, d)| light.remove(n).map(|l| (n, (d, l))))
            .collect();
        Applied {
            defaults,
            touched: BTreeSet::new(),
        }
    }

    fn apply(&mut self, theme: &Theme<'_>, reg: &Registry) {
        let light = theme.get_light();
        let mut set = 0;
        for &(name, kind) in theme_props::PROPS {
            if NOT_MODDABLE.contains(&name) {
                continue;
            }
            let value = match wanted(reg, name, light) {
                Some(v) => {
                    match convert(kind, v) {
                        Some(p) => Some(p),
                        None => {
                            eprintln!("mods: theme.{name} has a value of the wrong kind ({kind:?} wanted)");
                            None
                        }
                    }
                }
                None => None,
            };
            match value {
                Some(p) => {
                    theme_props::set(theme, name, &p);
                    self.touched.insert(name);
                    set += 1;
                }
                None if self.touched.contains(name) => {
                    if let Some((d, l)) = self.defaults.get(name) {
                        theme_props::set(theme, name, if light { l } else { d });
                    }
                    self.touched.remove(name);
                }
                None => {}
            }
        }
        eprintln!("mods: {} in use, {set} look settings", reg.mods().len());
    }
}

fn build() -> Registry {
    let (found, mut problems) = djsugar_mods::discover(&folders());
    let mut reg = Registry::build(found, &disabled());
    reg.problems.append(&mut problems);
    for p in &reg.problems {
        eprintln!("mods: {} left out: {}", p.id, p.reason);
    }
    reg
}

/// Loads the mods and keeps them applied: when a mod changes, and when the
/// DJ switches between Dark and Light.
pub fn install(app: &AppWindow) {
    let theme = app.global::<Theme>();
    let mut applied = Applied::capture(&theme);
    let mut reg = build();
    applied.apply(&theme, &reg);
    let mut stamp = djsugar_mods::fingerprint(&folders());
    let mut light = theme.get_light();
    let mut ticks = 0u32;
    let weak = app.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(250),
        move || {
            let Some(app) = weak.upgrade() else {
                return;
            };
            let theme = app.global::<Theme>();
            ticks = ticks.wrapping_add(1);
            let mut again = theme.get_light() != light;
            if ticks.is_multiple_of(4) {
                let now = djsugar_mods::fingerprint(&folders());
                if now != stamp {
                    stamp = now;
                    reg = build();
                    again = true;
                }
            }
            if again {
                light = theme.get_light();
                applied.apply(&theme, &reg);
            }
        },
    );
    // Runs as long as the app does.
    std::mem::forget(timer);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_read_in_every_form() {
        assert_eq!(
            parse_color("#ff2e93"),
            Some(slint::Color::from_rgb_u8(255, 46, 147))
        );
        assert_eq!(
            parse_color("#f0a"),
            Some(slint::Color::from_rgb_u8(255, 0, 170))
        );
        assert_eq!(
            parse_color("#ff2e9380"),
            Some(slint::Color::from_argb_u8(128, 255, 46, 147))
        );
        assert_eq!(parse_color("ff2e93"), None);
        assert_eq!(parse_color("#ff2e9"), None);
        assert_eq!(parse_color("#gg0000"), None);
    }

    #[test]
    fn every_theme_property_is_moddable() {
        assert!(theme_props::PROPS.len() > 150);
        for name in [
            "background",
            "accent",
            "lilac",
            "deck1",
            "radius",
            "font-normal",
        ] {
            assert!(theme_props::PROPS.iter().any(|(n, _)| *n == name), "{name}");
        }
    }

    #[test]
    fn the_shipped_look_only_names_real_properties() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../res/mods");
        let (found, problems) = djsugar_mods::discover(&[dir]);
        assert!(problems.is_empty(), "{problems:?}");
        let reg = Registry::build(found, &BTreeSet::new());
        assert!(reg.mods().iter().any(|m| m.id == "sugar.look"));
        for (slot, value) in reg.under("theme") {
            let name = slot
                .trim_start_matches("theme.")
                .trim_start_matches("dark.")
                .trim_start_matches("light.");
            let kind = theme_props::PROPS
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, k)| *k);
            let kind = kind.unwrap_or_else(|| panic!("{slot} isn't a Theme property"));
            assert!(
                convert(kind, value).is_some(),
                "{slot}: wrong kind of value"
            );
        }
    }
}

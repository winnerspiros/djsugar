//! Keyboard shortcuts: a key, written like `cmd+1`, runs an DJSugarScript
//! line. The defaults are here; the DJ's own replace them key by key
//! (settings `keyboard_shortcuts`). Space, ⌘comma, ⌘A and Esc stay fixed in
//! main.slint and never reach this.

use djsugar_control::ControlRegistry;
use djsugar_script::{query_control, run_control, Call, Error, Host, Kind};

use crate::browser::Browser;
use crate::settings;

pub struct Shortcut {
    pub key: &'static str,
    pub script: &'static str,
}

pub const DEFAULTS: &[Shortcut] = &[
    Shortcut {
        key: "cmd+1",
        script: "rating 1",
    },
    Shortcut {
        key: "cmd+2",
        script: "rating 2",
    },
    Shortcut {
        key: "cmd+3",
        script: "rating 3",
    },
    Shortcut {
        key: "cmd+4",
        script: "rating 4",
    },
    Shortcut {
        key: "cmd+5",
        script: "rating 5",
    },
    Shortcut {
        key: "cmd+i",
        script: "show_info",
    },
    Shortcut {
        key: "g",
        script: "edit_grid",
    },
    // Unload deck 1 or 2, as Serato's keys.
    Shortcut {
        key: "alt+shift+left",
        script: "deck 1 eject",
    },
    Shortcut {
        key: "alt+shift+right",
        script: "deck 2 eject",
    },
];

/// Modifiers in the order keys are written.
const MODIFIERS: [&str; 3] = ["cmd", "alt", "shift"];

/// Keys without a character of their own, as Slint reports them.
fn special(name: &str) -> Option<char> {
    use slint::platform::Key;
    let key = match name {
        "space" => return Some(' '),
        "return" => Key::Return,
        "tab" => Key::Tab,
        "escape" => Key::Escape,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "up" => Key::UpArrow,
        "down" => Key::DownArrow,
        "left" => Key::LeftArrow,
        "right" => Key::RightArrow,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        _ => return None,
    };
    Some(char::from(key))
}

const SPECIAL_NAMES: [&str; 26] = [
    "space",
    "return",
    "tab",
    "escape",
    "backspace",
    "delete",
    "up",
    "down",
    "left",
    "right",
    "home",
    "end",
    "pageup",
    "pagedown",
    "f1",
    "f2",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    "f10",
    "f11",
    "f12",
];

/// The key a press names, as shortcuts write it: `cmd+alt+shift+k`. `cmd`
/// is ⌘ on macOS and Ctrl elsewhere (Slint reports both as control).
pub fn key_name(text: &str, cmd: bool, alt: bool, shift: bool) -> Option<String> {
    let mut chars = text.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    let name = match SPECIAL_NAMES.iter().find(|n| special(n) == Some(c)) {
        Some(n) => n.to_string(),
        // Shift turns digits into symbols on a US keyboard; name the digit.
        None => match ")!@#$%^&*(".find(c).filter(|_| shift) {
            Some(digit) => digit.to_string(),
            None if c.is_control() => return None,
            None => c.to_lowercase().to_string(),
        },
    };
    let mut parts: Vec<&str> = MODIFIERS
        .iter()
        .zip([cmd, alt, shift])
        .filter(|(_, on)| *on)
        .map(|(m, _)| *m)
        .collect();
    parts.push(&name);
    Some(parts.join("+"))
}

/// A key as someone wrote it ("Shift+Cmd+F1", "⌘1") in the form
/// [`key_name`] gives, so the two compare equal.
pub fn normalize(key: &str) -> Option<String> {
    let key = key
        .trim()
        .to_lowercase()
        .replace('⌘', "cmd+")
        .replace('⌥', "alt+")
        .replace('⇧', "shift+");
    let mut on = [false; 3];
    let mut name = None;
    for part in key.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        let modifier = match part {
            "cmd" | "command" | "ctrl" | "control" => Some(0),
            "alt" | "option" | "opt" => Some(1),
            "shift" => Some(2),
            _ => None,
        };
        match modifier {
            Some(i) => on[i] = true,
            None if name.is_none() => name = Some(part.to_string()),
            None => return None,
        }
    }
    let name = name?;
    if name.chars().count() != 1 && !SPECIAL_NAMES.contains(&name.as_str()) {
        return None;
    }
    let mut parts: Vec<&str> = MODIFIERS
        .iter()
        .zip(on)
        .filter(|(_, on)| *on)
        .map(|(m, _)| *m)
        .collect();
    parts.push(&name);
    Some(parts.join("+"))
}

/// The line a key runs: the DJ's own for it, else the default.
pub fn script_for(key: &str) -> Option<String> {
    script_for_in(&settings::keyboard_shortcuts(), key)
}

fn script_for_in(own: &[(String, String)], key: &str) -> Option<String> {
    match find(own, key).map(|i| &own[i].1) {
        Some(script) if script.trim().is_empty() => None,
        Some(script) => Some(script.clone()),
        None => DEFAULTS
            .iter()
            .find(|s| s.key == key)
            .map(|s| s.script.to_string()),
    }
}

/// The key that runs exactly `script`, as the DJ reads it, for a menu line's
/// hint; "" when no key does.
pub fn key_for(script: &str) -> String {
    key_for_in(&settings::keyboard_shortcuts(), script)
}

fn key_for_in(own: &[(String, String)], script: &str) -> String {
    rows(own)
        .iter()
        .find(|r| r.script.trim() == script)
        .map(|r| display(&r.key))
        .unwrap_or_default()
}

/// Keys main.slint keeps for itself; a shortcut on them would never run.
pub const FIXED: [&str; 4] = ["space", "cmd+,", "cmd+a", "escape"];

/// One shortcut as the editor lists it.
#[derive(Debug, PartialEq)]
pub struct Row {
    pub key: String,
    /// Empty when the key does nothing (a default turned off, or a new
    /// shortcut not filled in yet).
    pub script: String,
    /// The key has a default shortcut.
    pub default: bool,
    /// It runs something other than its default.
    pub changed: bool,
}

fn default_for(key: &str) -> Option<&'static str> {
    DEFAULTS.iter().find(|s| s.key == key).map(|s| s.script)
}

fn find(own: &[(String, String)], key: &str) -> Option<usize> {
    own.iter()
        .position(|(k, _)| normalize(k).as_deref() == Some(key))
}

/// Every shortcut: the defaults in their order, then the DJ's own.
pub fn rows(own: &[(String, String)]) -> Vec<Row> {
    let mut rows: Vec<Row> = DEFAULTS
        .iter()
        .map(|s| match find(own, s.key) {
            Some(i) => Row {
                key: s.key.to_string(),
                script: own[i].1.clone(),
                default: true,
                changed: own[i].1 != s.script,
            },
            None => Row {
                key: s.key.to_string(),
                script: s.script.to_string(),
                default: true,
                changed: false,
            },
        })
        .collect();
    for (k, script) in own {
        let Some(key) = normalize(k) else { continue };
        if rows.iter().any(|r| r.key == key) {
            continue;
        }
        rows.push(Row {
            key,
            script: script.clone(),
            default: false,
            changed: true,
        });
    }
    rows
}

/// What `key` runs now, for the editor: "" when nothing.
fn current(own: &[(String, String)], key: &str) -> String {
    rows(own)
        .into_iter()
        .find(|r| r.key == key)
        .map(|r| r.script)
        .unwrap_or_default()
}

/// Makes `key` run `script`. Back to its default, the DJ's own entry goes.
pub fn set_script(own: &mut Vec<(String, String)>, key: &str, script: &str) {
    let at = find(own, key);
    if default_for(key) == Some(script) {
        if let Some(i) = at {
            own.remove(i);
        }
        return;
    }
    let entry = (key.to_string(), script.to_string());
    match at {
        Some(i) => own[i] = entry,
        None => own.push(entry),
    }
}

/// Takes `key` off the list; a default is turned off instead.
pub fn remove(own: &mut Vec<(String, String)>, key: &str) {
    if default_for(key).is_some() {
        set_script(own, key, "");
    } else if let Some(i) = find(own, key) {
        own.remove(i);
    }
}

/// Puts a default back.
pub fn reset(own: &mut Vec<(String, String)>, key: &str) {
    if let Some(i) = find(own, key) {
        own.remove(i);
    }
}

/// A new shortcut on `key`, not filled in yet. False when the key already
/// has a row.
pub fn add(own: &mut Vec<(String, String)>, key: &str) -> bool {
    if rows(own).iter().any(|r| r.key == key) {
        return false;
    }
    own.push((key.to_string(), String::new()));
    true
}

/// Moves what `old` runs to `new`. Answers what `new` ran before, when it
/// ran something else and that is now replaced.
pub fn rebind(own: &mut Vec<(String, String)>, old: &str, new: &str) -> Option<String> {
    if old == new {
        return None;
    }
    let script = current(own, old);
    let replaced = Some(current(own, new)).filter(|s| !s.trim().is_empty());
    remove(own, old);
    set_script(own, new, &script);
    replaced
}

/// A key as the DJ reads it: `⌘⇧1` on macOS, `Ctrl+Shift+1` elsewhere.
/// Saves imported shortcuts as a new set named after `file` and starts
/// using it; keys DJ Sugar keeps for itself are left out. Returns its name.
pub fn import_set(file: &str, keys: &[(String, String)]) -> String {
    let mut own = Vec::new();
    for (key, script) in keys {
        if let Some(k) = normalize(key) {
            if !FIXED.contains(&k.as_str()) {
                set_script(&mut own, &k, script);
            }
        }
    }
    let name = file
        .rsplit_once('.')
        .map_or(file, |(stem, _)| stem)
        .trim_start_matches("KEYBOARD - ")
        .to_string();
    settings::add_keyboard_set(&name, &own)
}

pub fn display(key: &str) -> String {
    let mac = cfg!(target_os = "macos");
    let mut parts: Vec<&str> = key.split('+').collect();
    let name = parts.pop().unwrap_or_default();
    let name = match name {
        "space" => "Space".to_string(),
        "return" => if mac { "↩" } else { "Enter" }.to_string(),
        "tab" => "Tab".to_string(),
        "escape" => "Esc".to_string(),
        "backspace" => if mac { "⌫" } else { "Backspace" }.to_string(),
        "delete" => if mac { "⌦" } else { "Delete" }.to_string(),
        "up" => "↑".to_string(),
        "down" => "↓".to_string(),
        "left" => "←".to_string(),
        "right" => "→".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        "pageup" => "Page Up".to_string(),
        "pagedown" => "Page Down".to_string(),
        n => n.to_uppercase(),
    };
    let modifier = |m: &str| match (m, mac) {
        ("cmd", true) => "⌘",
        ("alt", true) => "⌥",
        ("shift", true) => "⇧",
        ("cmd", false) => "Ctrl+",
        ("alt", false) => "Alt+",
        _ => "Shift+",
    };
    // macOS writes modifiers in the order ⌥⇧⌘.
    let order: &[&str] = if mac {
        &["alt", "shift", "cmd"]
    } else {
        &MODIFIERS
    };
    let mut out: String = order
        .iter()
        .filter(|m| parts.contains(m))
        .map(|m| modifier(m))
        .collect();
    out.push_str(&name);
    out
}

/// One line on what a script does, from its verbs' help, or why it
/// doesn't parse (the bool is true then).
pub fn describe(script: &str) -> (String, bool) {
    if script.trim().is_empty() {
        return (
            "Off. Type an action, or pick one from Actions.".into(),
            false,
        );
    }
    if let Err(e) = djsugar_script::parse(script) {
        return (e.message, true);
    }
    let mut helps: Vec<&str> = Vec::new();
    for word in script.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        if let Some(v) = djsugar_script::verb(word) {
            if !helps.contains(&v.help) {
                helps.push(v.help);
            }
        }
    }
    (helps.join(" · "), false)
}

/// A verb as it's written with its arguments, for the editor's list:
/// `loop [beats]`, `hot_cue <number>`.
pub fn usage(v: &djsugar_script::Verb) -> String {
    let mut out = v.name.to_string();
    for a in v.args {
        match a.default {
            Some(_) => out.push_str(&format!(" [{}]", a.name)),
            None => out.push_str(&format!(" <{}>", a.name)),
        }
    }
    out
}

/// A verb ready to run: arguments it needs get a sample value.
pub fn sample(v: &djsugar_script::Verb) -> String {
    use djsugar_script::ArgKind;
    let mut out = v.name.to_string();
    for a in v.args.iter().filter(|a| a.default.is_none()) {
        out.push(' ');
        out.push_str(match a.kind {
            ArgKind::Number { .. } => "1",
            ArgKind::Level => "100%",
            ArgKind::Control => "'play'",
            ArgKind::Text => "'x'",
        });
    }
    out
}

/// Runs DJSugarScript in the app: control verbs on the registry, `rating`
/// on the browser.
pub struct AppHost<'a> {
    pub registry: &'a ControlRegistry,
    pub browser: &'a mut Browser,
    /// The rows picked in the track list, first and last; -1 for none.
    pub picked: (i32, i32),
    /// 1 to 4.
    pub master: u8,
}

impl Host for AppHost<'_> {
    fn master_deck(&self) -> u8 {
        self.master
    }

    fn run(&mut self, call: &Call) -> Result<(), Error> {
        if call.verb.kind != Kind::App {
            return run_control(self.registry, call);
        }
        match call.verb.name {
            "rating" => {
                let stars = call.args[0].number().round() as u8;
                match call.deck {
                    Some(deck) => {
                        if !self.browser.rate_deck(deck as usize - 1, stars) {
                            return Err(Error::new(format!("Deck {deck} has no track")));
                        }
                    }
                    None => {
                        let (first, last) = self.picked;
                        if first < 0 {
                            return Err(Error::new("Pick a track in the browser first"));
                        }
                        self.browser
                            .rate_rows(first as usize, last.max(first) as usize, stars);
                    }
                }
                Ok(())
            }
            "show_info" => {
                let (first, _) = self.picked;
                if first < 0 {
                    return Err(Error::new("Pick a track in the browser first"));
                }
                self.browser.show_info(first);
                Ok(())
            }
            other => Err(Error::new(format!("'{other}' isn't available here"))),
        }
    }

    fn query(&mut self, call: &Call) -> Result<bool, Error> {
        query_control(self.registry, call)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_named_the_same_way_they_are_written() {
        assert_eq!(key_name("1", true, false, false).as_deref(), Some("cmd+1"));
        assert_eq!(
            key_name("!", true, false, true).as_deref(),
            Some("cmd+shift+1")
        );
        assert_eq!(
            key_name("K", false, true, true).as_deref(),
            Some("alt+shift+k")
        );
        let f1 = char::from(slint::platform::Key::F1).to_string();
        assert_eq!(key_name(&f1, false, false, false).as_deref(), Some("f1"));
        for (written, named) in [
            ("Cmd+1", "cmd+1"),
            ("Shift+Cmd+F1", "cmd+shift+f1"),
            ("⌘⇧2", "cmd+shift+2"),
            ("ctrl + alt + x", "cmd+alt+x"),
        ] {
            assert_eq!(normalize(written).as_deref(), Some(named), "{written}");
        }
        assert_eq!(normalize("cmd+ab"), None);
        assert_eq!(normalize("cmd"), None);
    }

    #[test]
    fn the_default_shortcuts_parse_and_are_written_normally() {
        for s in DEFAULTS {
            assert_eq!(normalize(s.key).as_deref(), Some(s.key));
            djsugar_script::parse(s.script).unwrap();
        }
    }

    fn own(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, s)| (k.to_string(), s.to_string()))
            .collect()
    }

    #[test]
    fn rows_put_the_djs_own_over_the_defaults() {
        let list = own(&[("Cmd+2", "deck 1 play"), ("f1", "sync")]);
        let rows = rows(&list);
        assert_eq!(rows.len(), DEFAULTS.len() + 1);
        assert_eq!(rows[0].script, "rating 1");
        assert!(!rows[0].changed);
        assert_eq!(rows[1].script, "deck 1 play");
        assert!(rows[1].changed && rows[1].default);
        let last = rows.last().unwrap();
        assert_eq!((last.key.as_str(), last.default), ("f1", false));
    }

    #[test]
    fn editing_keeps_only_what_differs_from_the_defaults() {
        let mut list = Vec::new();
        set_script(&mut list, "cmd+1", "rating 5");
        assert_eq!(list, own(&[("cmd+1", "rating 5")]));
        set_script(&mut list, "cmd+1", "rating 1");
        assert!(list.is_empty());
        remove(&mut list, "cmd+3");
        assert_eq!(list, own(&[("cmd+3", "")]));
        assert_eq!(script_for_in(&list, "cmd+3"), None);
        reset(&mut list, "cmd+3");
        assert!(list.is_empty());
        assert!(add(&mut list, "f2"));
        assert!(!add(&mut list, "f2"));
        assert!(!add(&mut list, "cmd+4"));
        remove(&mut list, "f2");
        assert!(list.is_empty());
    }

    #[test]
    fn rebinding_moves_the_action_and_says_what_it_replaced() {
        let mut list = Vec::new();
        assert_eq!(rebind(&mut list, "cmd+1", "f1"), None);
        assert_eq!(list, own(&[("cmd+1", ""), ("f1", "rating 1")]));
        let replaced = rebind(&mut list, "f1", "cmd+2");
        assert_eq!(replaced.as_deref(), Some("rating 2"));
        let rows = rows(&list);
        assert_eq!(rows[1].script, "rating 1");
        assert!(rows.iter().all(|r| r.key != "f1"));
    }

    #[test]
    fn scripts_are_described_or_their_mistake_shown() {
        assert_eq!(
            describe("rating 3"),
            (
                djsugar_script::verb("rating").unwrap().help.to_string(),
                false
            )
        );
        assert!(describe("ratin 3").1);
        assert!(!describe("").1);
        for v in djsugar_script::VERBS {
            djsugar_script::parse(&sample(v)).unwrap_or_else(|e| panic!("{}: {e}", sample(v)));
        }
    }

    #[test]
    fn menu_hints_name_the_key_that_runs_the_line() {
        assert_eq!(key_for_in(&[], "show_info"), display("cmd+i"));
        let moved = own(&[("cmd+i", ""), ("f2", "show_info")]);
        assert_eq!(key_for_in(&moved, "show_info"), display("f2"));
        assert_eq!(key_for_in(&own(&[("cmd+i", "")]), "show_info"), "");
    }

    #[test]
    fn keys_display_for_the_platform() {
        let shown = display("cmd+shift+1");
        if cfg!(target_os = "macos") {
            assert_eq!(shown, "⇧⌘1");
        } else {
            assert_eq!(shown, "Ctrl+Shift+1");
        }
        assert!(display("left").ends_with('←'));
    }
}

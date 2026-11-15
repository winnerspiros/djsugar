//! Settings › Keyboard: shortcuts and capturing a new key.

use super::*;

/// A shortcut's row on the Keyboard page.
pub(super) fn shortcut_item(r: &shortcuts::Row) -> PrefItem {
    let (detail, wrong) = shortcuts::describe(&r.script);
    let verbs: Vec<&str> = djsugar_script::VERBS.iter().map(|v| v.name).collect();
    PrefItem {
        value: r.script.as_str().into(),
        on: wrong,
        choices: strings(&verbs),
        action: match (r.default, r.changed) {
            (true, true) => "Reset",
            (true, false) => "Turn Off",
            (false, _) => "Remove",
        }
        .into(),
        ..item(
            &format!("shortcut:{}", r.key),
            SHORTCUT,
            &shortcuts::display(&r.key),
            &detail,
        )
    }
}

impl Prefs {
    pub(super) fn keyboard_page(&self) -> Vec<PrefItem> {
        let fixed = if cfg!(target_os = "macos") {
            "Space plays or pauses the master deck, ⌘, opens Settings and Esc closes them; those stay as they are."
        } else {
            "Space plays or pauses the master deck, Ctrl+, opens Settings and Esc closes them; those stay as they are."
        };
        let mut items = vec![
            item(
                "",
                NOTE,
                "",
                &format!(
                    "Each key runs a line of DJSugarScript, such as “rating 3” or “deck 2 sync & play”. \
                     Click a key to change it, type the action, or pick one from Actions. {fixed}"
                ),
            ),
            item("", HEADING, "SET", ""),
        ];
        let sets = settings::keyboard_sets();
        let current = settings::keyboard_set();
        let mut choices = sets.clone();
        choices.push("New set from this one".into());
        choices.push("Import…".into());
        let default = current == settings::DEFAULT_KEYBOARD_SET;
        if !default {
            choices.push("Delete this set".into());
        }
        items.push(PrefItem {
            choices: strings(&choices),
            chosen: sets.iter().position(|n| *n == current).unwrap_or(0) as i32,
            ..item(
                "keyboard_set",
                MENU,
                "Shortcut set",
                "Keep several sets of shortcuts and switch between them. Import reads a VirtualDJ keyboard mapping.",
            )
        });
        if !default {
            items.push(PrefItem {
                value: current.clone().into(),
                ..item("keyboard_set_name", TEXT, "Name", "")
            });
        }
        items.push(item("", HEADING, "SHORTCUTS", ""));
        if !self.shortcut_note.is_empty() {
            items.push(item("", NOTE, "", &self.shortcut_note));
        }
        let own = settings::keyboard_shortcuts();
        items.extend(shortcuts::rows(&own).iter().map(shortcut_item));
        items.push(PrefItem {
            detail: "Click Add a key, then press the keys for a new shortcut.".into(),
            ..item("shortcut_new", SHORTCUT, "", "")
        });
        items.push(PrefItem {
            action: "Reset All".into(),
            ..item(
                "shortcuts_reset_all",
                BUTTON,
                "Reset all shortcuts",
                &format!(
                    "Back to DJ Sugar's own: {} to {} rate the picked tracks.",
                    shortcuts::display("cmd+1"),
                    shortcuts::display("cmd+5")
                ),
            )
        });
        items.push(item("", HEADING, "ACTIONS", ""));
        items.push(item(
            "",
            NOTE,
            "",
            "A verb alone acts on the master deck; “deck 2 play” on deck 2. “&” runs one action after \
             another, “play ? pause : play” asks first, and “toggle 'slip_enabled'” reaches any control by its name.",
        ));
        items.extend(djsugar_script::VERBS.iter().map(|v| {
            item(
                &format!("verb:{}", v.name),
                NOTE,
                &shortcuts::usage(v),
                v.help,
            )
        }));
        items
    }

    /// The DJ typed in a shortcut: its row is updated where it is, so the
    /// field keeps its focus.
    pub(super) fn update_shortcut_row(&self, id: &str) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let own = settings::keyboard_shortcuts();
        let Some(row) = shortcuts::rows(&own)
            .into_iter()
            .find(|r| id.strip_prefix("shortcut:") == Some(r.key.as_str()))
        else {
            return;
        };
        let items = app.global::<Preferences>().get_items();
        for i in 0..slint::Model::row_count(&items) {
            let Some(old) = slint::Model::row_data(&items, i) else {
                continue;
            };
            if old.id == id {
                let mut new = shortcut_item(&row);
                // The field shows what was typed, not a saved copy.
                new.value = old.value;
                slint::Model::set_row_data(&items, i, new);
            }
        }
    }

    /// A key pressed on a shortcut's key button. False to keep listening.
    pub(super) fn key_captured(
        &mut self,
        id: &str,
        text: &str,
        cmd: bool,
        alt: bool,
        shift: bool,
    ) -> bool {
        let Some(key) = shortcuts::key_name(text, cmd, alt, shift) else {
            return false;
        };
        let shown = shortcuts::display(&key);
        let mut own = settings::keyboard_shortcuts();
        if shortcuts::FIXED.contains(&key.as_str())
            || shortcuts::normalize(&key).as_deref() != Some(&key)
        {
            self.shortcut_note = format!("{shown} can't be a shortcut.");
        } else if let Some(old) = id.strip_prefix("shortcut:") {
            let script = |key: &str| {
                shortcuts::rows(&own)
                    .into_iter()
                    .find(|r| r.key == key)
                    .map(|r| r.script)
                    .unwrap_or_default()
            };
            // Moving a key that does nothing onto one in use would only
            // turn that one off.
            let taken = script(&key);
            if script(old).trim().is_empty() && !taken.trim().is_empty() && old != key {
                self.shortcut_note = format!("{shown} already runs “{taken}”.");
                self.refresh();
                return true;
            }
            self.shortcut_note = match shortcuts::rebind(&mut own, old, &key) {
                Some(was) => format!("{shown} ran “{was}”; it now runs this one instead."),
                None => String::new(),
            };
        } else if !shortcuts::add(&mut own, &key) {
            self.shortcut_note = format!("{shown} is already on the list.");
        } else {
            self.shortcut_note.clear();
        }
        settings::set_keyboard_shortcuts(&own);
        self.refresh();
        true
    }
}

/// Settings › Player › MEDIA KEYS: whether the media keys reach the DJ
/// layouts too.
pub(super) fn media_key_items() -> Vec<PrefItem> {
    vec![
        item("", HEADING, "MEDIA KEYS", ""),
        switch(
            crate::media_controls::EVERYWHERE,
            "Media keys in every layout",
            "Play/pause, next and previous keys, headphone buttons and the system's Now Playing always work in Player. On, they also play and pause the master deck in the DJ layouts, where a headphone button could stop a deck mid-set.",
            settings::flag(crate::media_controls::EVERYWHERE, false),
        ),
    ]
}

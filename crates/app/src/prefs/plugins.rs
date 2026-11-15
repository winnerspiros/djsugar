//! Settings › Plugins (CLAP, VST3 and AU) and each add-on's own settings page.

use super::*;

/// How the plugin list is narrowed; kept while Settings is open.
#[derive(Default)]
pub(super) struct PluginView {
    /// The format chips that are on ("CLAP", "VST3", "AU"); none is all.
    pub(super) formats: Vec<String>,
    pub(super) search: String,
    /// Makers opened in the list.
    pub(super) open: std::collections::HashSet<String>,
}

/// The formats in `list`, in the chips' order.
pub(super) fn plugin_formats(list: &[Found]) -> Vec<String> {
    ["CLAP", "VST3", "AU"]
        .into_iter()
        .filter(|f| list.iter().any(|p| p.format == *f))
        .map(String::from)
        .collect()
}

/// `list` narrowed to `formats` (none is all) and to plugins whose name or
/// maker has every word of `search`, by maker, makers and plugins A to Z.
pub(super) fn plugin_groups(
    list: &[Found],
    formats: &[String],
    search: &str,
) -> Vec<(String, Vec<Found>)> {
    let words: Vec<String> = search.split_whitespace().map(str::to_lowercase).collect();
    // By the maker's name in lower case, so "FabFilter" and "fabfilter"
    // are one maker, shown as first spelt.
    let mut groups: std::collections::BTreeMap<String, (String, Vec<Found>)> = Default::default();
    for f in list {
        if !formats.is_empty() && !formats.iter().any(|x| x == f.format) {
            continue;
        }
        let maker = match f.vendor.trim() {
            "" => "Other".to_string(),
            v => v.to_string(),
        };
        let text = format!("{} {}", f.name, maker).to_lowercase();
        if !words.iter().all(|w| text.contains(w.as_str())) {
            continue;
        }
        groups
            .entry(maker.to_lowercase())
            .or_insert_with(|| (maker, Vec::new()))
            .1
            .push(f.clone());
    }
    groups
        .into_values()
        .map(|(maker, mut fs)| {
            fs.sort_by(|a, b| {
                (a.name.to_lowercase(), a.format).cmp(&(b.name.to_lowercase(), b.format))
            });
            (maker, fs)
        })
        .collect()
}

impl Prefs {
    /// Settings › Plugins. Effects and Instruments: Scan, then the plugins
    /// by maker, narrowed by format chips and a search, each with a tick.
    /// Folders: where plugins are looked for, Scan, and files that couldn't
    /// be read, with Retry. `tab` None is everything, one row per plugin,
    /// for a search of all of Settings.
    pub(super) fn audio_plugin_page(&self, tab: Option<usize>) -> Vec<PrefItem> {
        let p = self.audio_plugins.borrow();
        if !p.available() {
            return vec![item(
                "",
                NOTE,
                "Plugins",
                "This copy of DJ Sugar came without its plugin helper (djsugar-plugin-host), so CLAP, VST3 and Audio Unit plugins can't run.",
            )];
        }
        let effects = p.effects();
        let instruments = p.instruments();
        let failures = p.failures();
        let shown = p.shown();
        let note = if !p.note.is_empty() {
            p.note.clone()
        } else if effects.is_empty() && instruments.is_empty() {
            "Nothing is looked at until you press Scan.".to_string()
        } else {
            format!(
                "{} effects and {} instruments known from the last scan.",
                effects.len(),
                instruments.len()
            )
        };
        let scan = PrefItem {
            action: if p.scanning() { "" } else { "Scan" }.into(),
            value: if p.scanning() {
                // Just the count: the file's name is in the line below.
                note.split(':').next().unwrap_or_default().into()
            } else {
                "".into()
            },
            ..item("audio_plugins_scan", BUTTON, "Scan for plugins", &note)
        };
        let mut items = Vec::new();
        if tab.is_none_or(|t| t == FOLDERS_TAB) {
            items.push(folder_list(
                PLUGIN_FOLDERS_LIST,
                "Plugin folders",
                "Where DJ Sugar looks for .clap and .vst3 plugins, besides the system's own plugin \
                 folders.",
                &p.folders(),
                false,
                false,
            ));
            items.push(scan.clone());
            if !failures.is_empty() {
                let n = failures.len();
                items.push(PrefItem {
                    action: "Details…".into(),
                    ..item(
                        "audio_plugin_errors",
                        BUTTON,
                        &format!(
                            "{n} {} couldn't be loaded",
                            if n == 1 { "plugin" } else { "plugins" }
                        ),
                        "",
                    )
                });
            }
        }
        let Some(tab) = tab else {
            for e in effects.iter().chain(&instruments) {
                items.push(PrefItem {
                    on: shown.contains(&e.key),
                    ..item(
                        &format!("audio_plugin_show:{}", e.key),
                        TICK,
                        &format!("{} · {} · {}", e.name, e.vendor, e.format),
                        "",
                    )
                });
            }
            return items;
        };
        if tab == FOLDERS_TAB {
            return items;
        }
        let instrument_tab = tab == 1;
        let list = if instrument_tab {
            &instruments
        } else {
            &effects
        };
        items.push(PrefItem {
            detail: if p.scanning() {
                note.clone()
            } else if list.is_empty() {
                format!("{note} Plugin folders are on the Folders tab.")
            } else if instrument_tab {
                "Ticked instruments are offered by the Instrument input in the MIC panel.".into()
            } else {
                "Ticked effects are offered in every FX list: decks, Beat FX, Mic and Aux.".into()
            }
            .into(),
            ..scan
        });
        if list.is_empty() {
            return items;
        }
        let v = &self.plugin_view;
        let formats = plugin_formats(list);
        items.push(PrefItem {
            choices: strings(&formats),
            chosen: formats
                .iter()
                .enumerate()
                .filter(|(_, f)| v.formats.contains(f))
                .map(|(i, _)| 1 << i)
                .sum(),
            value: v.search.as_str().into(),
            ..item(
                "audio_plugin_filter",
                FILTER_BAR,
                "",
                if instrument_tab {
                    "Search instruments and makers"
                } else {
                    "Search effects and makers"
                },
            )
        });
        let groups = plugin_groups(list, &v.formats, &v.search);
        if groups.is_empty() {
            items.push(item("", NOTE, "", "Nothing matches."));
        }
        let searching = !v.search.trim().is_empty();
        for (maker, plugins) in &groups {
            let ticked = plugins.iter().filter(|f| shown.contains(&f.key)).count();
            let open = searching || v.open.contains(maker);
            items.push(PrefItem {
                on: ticked == plugins.len(),
                value: if ticked > 0 && ticked < plugins.len() {
                    "some".into()
                } else {
                    "".into()
                },
                chosen: 1,
                action: if open { "▾" } else { "▸" }.into(),
                ..item(
                    &format!("audio_plugin_maker:{maker}"),
                    TICK,
                    maker,
                    &if ticked > 0 {
                        format!("{ticked} of {} ticked", plugins.len())
                    } else {
                        plugins.len().to_string()
                    },
                )
            });
            if !open {
                continue;
            }
            for f in plugins {
                items.push(PrefItem {
                    on: shown.contains(&f.key),
                    chosen: 2,
                    ..item(
                        &format!("audio_plugin_show:{}", f.key),
                        TICK,
                        &f.name,
                        f.format,
                    )
                });
            }
        }
        items
    }

    /// The plugin list shown: instruments on Settings › Plugins'
    /// Instruments tab, otherwise effects.
    pub(super) fn plugin_list(&self) -> Vec<Found> {
        let tab = self
            .app
            .upgrade()
            .map_or(0, |a| a.global::<Preferences>().get_tab());
        let p = self.audio_plugins.borrow();
        if tab == 1 {
            p.instruments()
        } else {
            p.effects()
        }
    }

    /// Keyboard shortcuts, each an DJSugarScript line (see shortcuts.rs).
    pub(super) fn audio_plugin_pressed(&mut self, id: &str) -> bool {
        if let Some(maker) = id
            .strip_prefix("audio_plugin_maker:")
            .and_then(|m| m.strip_suffix("#fold"))
        {
            let open = &mut self.plugin_view.open;
            if !open.remove(maker) {
                open.insert(maker.to_string());
            }
            self.refresh();
            return true;
        }
        let mut p = self.audio_plugins.borrow_mut();
        if id == "audio_plugins_scan" {
            p.start_scan();
        } else if id == "audio_plugin_folder_add" {
            choose(id, "Choose a plugin folder", true, self.chosen_tx.clone());
        } else if let Some(i) = id
            .strip_prefix("audio_plugin_folder_remove:")
            .and_then(|i| i.parse::<usize>().ok())
        {
            let mut folders = p.folders();
            if i < folders.len() {
                folders.remove(i);
            }
            p.set_folders(&folders);
        } else if id == "audio_plugin_errors" {
            let failures = p.failures();
            let text: Vec<String> = failures
                .iter()
                .map(|(path, why)| format!("{}\n    {why}", path.display()))
                .collect();
            if let Some(app) = self.app.upgrade() {
                let g = app.global::<PluginErrors>();
                g.set_title(
                    format!(
                        "{} {} couldn't be loaded",
                        failures.len(),
                        if failures.len() == 1 {
                            "plugin"
                        } else {
                            "plugins"
                        }
                    )
                    .into(),
                );
                g.set_text(text.join("\n\n").into());
                g.set_open(true);
            }
        } else if id == "audio_plugin_errors_close" {
            if let Some(app) = self.app.upgrade() {
                app.global::<PluginErrors>().set_open(false);
            }
        } else if id == "audio_plugin_retry_all" {
            for (path, _) in p.failures() {
                p.retry(&path);
            }
            if let Some(app) = self.app.upgrade() {
                app.global::<PluginErrors>().set_open(false);
            }
        } else {
            return false;
        }
        drop(p);
        self.refresh();
        true
    }

    /// Plugin `index`'s page: its own rows, ids `plugin:<index>:<key>`.
    pub(super) fn plugin_page(&self, index: usize) -> Vec<PrefItem> {
        let Some(plugin) = self.plugins.get(index) else {
            return Vec::new();
        };
        let config = settings::plugin_config(plugin.manifest().id);
        plugin
            .settings()
            .into_iter()
            .map(|s| {
                let id = format!("plugin:{index}:{}", s.key);
                match s.kind {
                    SettingKind::Note => item(&id, NOTE, &s.title, &s.detail),
                    SettingKind::Switch(default) => PrefItem {
                        on: config.is_on(s.key, default),
                        ..item(&id, SWITCH, &s.title, &s.detail)
                    },
                    SettingKind::Folder => PrefItem {
                        value: match config.get(s.key) {
                            "" => "Default".into(),
                            set => set.into(),
                        },
                        action: "Choose…".into(),
                        ..item(&id, PATH, &s.title, &s.detail)
                    },
                    kind => PrefItem {
                        value: config.get(s.key).into(),
                        on: kind == SettingKind::Secret,
                        ..item(&id, TEXT, &s.title, &s.detail)
                    },
                }
            })
            .collect()
    }

    /// A value on a plugin's page (`plugin:<index>:<key>`) changed: it's
    /// saved, and the plugin hears of it.
    pub(super) fn set_plugin_value(&mut self, id: &str, value: &str) {
        let mut parts = id.splitn(3, ':');
        let (Some("plugin"), Some(index), Some(key)) = (parts.next(), parts.next(), parts.next())
        else {
            return;
        };
        let Some(plugin) = index
            .parse()
            .ok()
            .and_then(|i: usize| self.plugins.get_mut(i))
        else {
            return;
        };
        let plugin_id = plugin.manifest().id;
        settings::set_plugin_value(plugin_id, key, value);
        plugin.configure(&settings::plugin_config(plugin_id));
        (self.plugin_changed)(plugin_id);
    }
}

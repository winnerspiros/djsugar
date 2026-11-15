//! Settings › Sources: where music comes from, and which sources the rail shows.

use super::*;

impl Prefs {
    /// Settings › Sources: the cache every source shares, then each source
    /// by kind with its status and In rail switch. The open source shows
    /// its settings under it; a search (`all`) shows every source's.
    pub(super) fn sources_page(&self, all: bool) -> Vec<PrefItem> {
        let (used, each) = crate::sources::cache_usage();
        let limit = settings::number(crate::sources::CACHE_GB, crate::sources::DEFAULT_CACHE_GB);
        let limits: Vec<String> = SOURCES_CACHE_GB
            .iter()
            .map(|&gb| {
                if gb == crate::sources::DEFAULT_CACHE_GB {
                    format!("{gb} GB (default)")
                } else {
                    format!("{gb} GB")
                }
            })
            .collect();
        let mut cache_rows = vec![PrefItem {
            choices: strings(&limits),
            chosen: SOURCES_CACHE_GB
                .iter()
                .position(|&gb| gb == limit)
                .unwrap_or(2) as i32,
            ..item(
                "sources_cache_gb",
                MENU,
                "Cache for played tracks",
                &format!(
                    "{} used by tracks played from your servers; the oldest go first. \
                         Manage under Collection › Storage clears them.",
                    if used == 0 {
                        "Nothing".to_string()
                    } else {
                        size(used)
                    },
                ),
            )
        }];
        if !self.sources_note.is_empty() {
            cache_rows.push(item("", NOTE, "", &self.sources_note));
        }
        let rail = self.rail_shown();
        let shown = |i: i32| rail.get(i as usize).copied().unwrap_or(true);
        let place = |name: &str| {
            self.rail_names
                .iter()
                .position(|n| n == name)
                .map(|i| i as i32)
        };
        let open = |i: i32| all || self.open_source == Some(i);
        let fold = |i: i32| if open(i) { "▾" } else { "▸" };
        let row = |i: i32, title: &str, status: i32, detail: &str, arrow: &str| PrefItem {
            on: shown(i),
            chosen: status,
            action: arrow.into(),
            ..item(&format!("source:{i}"), SOURCE_ROW, title, detail)
        };

        let mut items = Vec::new();
        // DJ Sugar's own sources, then everything outside it, as in the
        // rail's "..." menu.
        items.push(item("", HEADING, "BUILT-IN", ""));
        for name in ["Collection", "Files", "Albums", "Recordings", "Jam"] {
            if let Some(i) = place(name) {
                items.push(row(i, name, 0, "", ""));
            }
        }
        // Everything else in one section: the streaming services, the DJ's
        // own servers, free music (the stores come later) and add-ons.
        items.push(item("", HEADING, "EXTERNAL", ""));
        if crate::browser::STREAMING_ENABLED {
            for &(source, name, id) in crate::browser::STREAMS.iter() {
                let signed_in = crate::spotify::Saved::load(&format!("{id}.json"))
                    .is_some_and(|s| !s.refresh_token.is_empty());
                let (status, detail) = if signed_in {
                    (1, "Signed in".to_string())
                } else {
                    (0, "Not signed in".to_string())
                };
                items.push(row(source, name, status, &detail, fold(source)));
                if open(source) {
                    items.push(item(
                        "",
                        NOTE,
                        "",
                        &format!(
                            "Sign in from {name} in the rail. Its playlists show there, and \
                             tracks also in your collection can be played. {name} is browse \
                             only: nothing downloads and nothing is cached."
                        ),
                    ));
                    if signed_in {
                        items.push(PrefItem {
                            action: "Sign out".into(),
                            ..item(
                                &format!("stream_sign_out:{source}"),
                                BUTTON,
                                &format!("Sign out of {name}"),
                                "Your playlists there are forgotten here until you sign in again.",
                            )
                        });
                    }
                }
            }
        }

        // Plugin sources, by kind: the DJ's own servers, built-in sources,
        // then add-ons.
        let builtin: Vec<&str> = djsugar_plugins::builtin()
            .iter()
            .map(|p| p.manifest().id)
            .collect();
        let kind = |n: usize| {
            if self.plugin_plays[n] {
                0
            } else if builtin.contains(&self.plugins[n].manifest().id) {
                1
            } else {
                2
            }
        };
        let mut addons = Vec::new();
        for k in 0..3 {
            let members: Vec<usize> = (0..self.plugins.len()).filter(|&n| kind(n) == k).collect();
            if members.is_empty() {
                continue;
            }
            // Servers and free music follow the streaming services; add-ons
            // come last.
            let items = if k == 2 { &mut addons } else { &mut items };
            for n in members {
                let plugin = &self.plugins[n];
                let name = plugin.manifest().name;
                let config = settings::plugin_config(plugin.manifest().id);
                let set_up = !crate::sources::needs_setup(&plugin.settings(), &config);
                let mut detail = if !set_up {
                    "Not set up".to_string()
                } else if self.plugin_plays[n] {
                    "Set up".to_string()
                } else {
                    "Ready".to_string()
                };
                let cached = each
                    .get(&crate::sources::cache_key(name))
                    .copied()
                    .unwrap_or(0);
                if cached > 0 {
                    detail.push_str(&format!(" · {} cached", size(cached)));
                }
                let folder = match config.get(DOWNLOAD_FOLDER).trim() {
                    "" => format!("Downloaded/{name}"),
                    set => set.to_string(),
                };
                if plugin.settings().iter().any(|s| s.key == DOWNLOAD_FOLDER) {
                    // A server's files reach the collection with Add to
                    // Collection; other sources' with Download.
                    let verb = if self.plugin_plays[n] {
                        "adds to"
                    } else {
                        "downloads to"
                    };
                    detail.push_str(&format!(" · {verb} {folder}"));
                }
                match place(name) {
                    Some(i) => {
                        items.push(row(i, name, set_up as i32, &detail, fold(i)));
                        if open(i) {
                            items.extend(self.plugin_page(n));
                        }
                    }
                    // Settings with no source in the rail: always open.
                    None => {
                        items.push(item("", HEADING, &name.to_uppercase(), ""));
                        items.extend(self.plugin_page(n));
                    }
                }
            }
        }

        items.extend(addons);
        items.extend(cache_rows);
        items
    }

    /// Whether each source shows in the rail, in the rail's order.
    pub(super) fn rail_shown(&self) -> Vec<bool> {
        self.app
            .upgrade()
            .map(|app| slint::Model::iter(&app.get_sources_shown()).collect())
            .unwrap_or_default()
    }

    /// Shows or hides source `i` in the rail, as its right-click menu does.
    pub(super) fn set_rail_shown(&self, i: i32, on: bool) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let shown = app.get_sources_shown();
        if i < 0 || i as usize >= slint::Model::row_count(&shown) {
            return;
        }
        slint::Model::set_row_data(&shown, i as usize, on);
        app.invoke_source_shown_changed(i, on);
    }
}

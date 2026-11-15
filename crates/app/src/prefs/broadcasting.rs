//! Settings › Broadcast: the stations to stream to, the loudness meter
//! target and the now-playing feed; the recording limiter and
//! scrobbling, which other pages show.

use super::*;

/// A station's row on Settings › Broadcast: `bc:<index>:<field>`.
pub(super) fn station_field(id: &str) -> Option<(usize, &str)> {
    let rest = id.strip_prefix("bc:")?;
    let (i, field) = rest.split_once(':')?;
    Some((i.parse().ok()?, field))
}

/// Settings › Broadcast › Stations: where the loudness meter in the ON AIR
/// panel shows green.
pub(super) fn loudness_target_item() -> PrefItem {
    let target = settings::number("loudness_target", meter::DEFAULT_LOUDNESS_TARGET);
    PrefItem {
            choices: strings(&LOUDNESS_TARGETS.map(|t| t.1)),
            chosen: LOUDNESS_TARGETS
                .iter()
                .position(|t| (t.0 - target).abs() < 1e-6)
                .unwrap_or(0) as i32,
            ..item(
                "loudness_target",
                MENU,
                "Loudness target",
                "Where the loudness meter in the ON AIR panel shows green. It doesn't change any level.",
            )
        }
}

/// Settings › Audio › Recording: the limiter on recordings and streams.
pub(super) fn air_ceiling_item() -> PrefItem {
    let ceiling = settings::number("air_ceiling", engine::DEFAULT_AIR_CEILING_DB);
    PrefItem {
            choices: strings(&AIR_CEILINGS.map(|t| t.1)),
            chosen: AIR_CEILINGS
                .iter()
                .position(|t| (t.0 - ceiling).abs() < 1e-6)
                .unwrap_or(0) as i32,
            ..item(
                "air_ceiling",
                MENU,
                "Limiter while recording or on air",
                "Keeps peaks, also those between samples, under this level so MP3, AAC and Opus files and streams don't clip.",
            )
        }
}

/// Settings › Broadcast › NOW PLAYING FEED: the track the audience hears,
/// for OBS and other streaming software.
pub(super) fn now_playing_items() -> Vec<PrefItem> {
    use crate::now_playing as np;
    let on = settings::flag(np::ENABLED, false);
    let mut items = vec![
        item("", HEADING, "NOW PLAYING FEED", ""),
        switch(
            np::ENABLED,
            "Now playing for streaming software",
            "Writes the track heard to now-playing.txt and now-playing.json, and serves an \
             overlay page for OBS or Streamlabs. Works with or without broadcasting.",
            on,
        ),
    ];
    if !on {
        return items;
    }
    let status = np::status();
    if !status.is_empty() {
        let overlay = status.starts_with("http");
        items.push(PrefItem {
            action: if overlay { "Open".into() } else { "".into() },
            ..item(
                "now_playing_open",
                if overlay { BUTTON } else { NOTE },
                if overlay { "Overlay page" } else { "" },
                &if overlay {
                    format!("{status}  ·  Add it as a Browser Source, 1920 × 1080.")
                } else {
                    status
                },
            )
        });
    }
    items.push(PrefItem {
        choices: strings(&["Lower third", "Corner", "Ticker"]),
        chosen: settings::number(np::STYLE, 0.0).clamp(0.0, 2.0) as i32,
        ..item(np::STYLE, SEGMENTED, "Overlay style", "")
    });
    items.push(folder_list(
        np::FOLDER,
        "Files folder",
        "Where now-playing.txt and now-playing.json are kept, for a Text source in OBS.",
        &[np::folder()],
        true,
        false,
    ));
    items.push(PrefItem {
        value: settings::string(np::TEMPLATE).into(),
        ..item(
            np::TEMPLATE,
            TEXT,
            "Text file line",
            "{artist}, {title}, {album}, {label}, {genre}, {bpm} and {key} are filled in. \
             Empty is {artist} - {title}.",
        )
    });
    let holds = [0.0, 5.0, 10.0, 20.0, 30.0];
    let hold = settings::number(np::HOLD, np::DEFAULT_HOLD);
    items.push(PrefItem {
        choices: strings(&["At once", "5 s", "10 s", "20 s", "30 s"]),
        chosen: holds
            .iter()
            .position(|&h| (h - hold).abs() < 0.5)
            .unwrap_or(2) as i32,
        ..item(
            np::HOLD,
            MENU,
            "Changes after",
            "How long a track must be the one heard before it shows, so a quick cut or a \
             long blend doesn't flip the title back and forth.",
        )
    });
    items.push(switch(
        np::NETWORK,
        "Open to other devices",
        "Lets a second computer running OBS open the overlay page over your network. Off, \
         only this computer can.",
        settings::flag(np::NETWORK, false),
    ));
    items
}

/// Settings › Player › SCROBBLING: the tracks heard, sent to the DJ's
/// ListenBrainz profile.
pub(super) fn scrobble_items() -> Vec<PrefItem> {
    use crate::scrobble as sc;
    let on = settings::flag(sc::ENABLED, false);
    let mut items = vec![
        item("", HEADING, "SCROBBLING", ""),
        switch(
            sc::ENABLED,
            "Send plays to ListenBrainz",
            "Each track the audience hears goes to your ListenBrainz profile once half of it \
             (or four minutes) has played, from any layout. Tracks cued in the headphones \
             don't count.",
            on,
        ),
    ];
    if !on {
        return items;
    }
    items.push(PrefItem {
        value: settings::string(sc::TOKEN).into(),
        on: true,
        ..item(
            sc::TOKEN,
            TEXT,
            "ListenBrainz token",
            "Your user token, from listenbrainz.org under Settings.",
        )
    });
    let status = sc::status();
    if !status.is_empty() {
        items.push(item("", NOTE, "", &status));
    }
    items
}

/// Settings › Player › SCROBBLING, Last.fm: the DJ's own API account
/// and signing in.
pub(super) fn lastfm_items() -> Vec<PrefItem> {
    use crate::lastfm as fm;
    let on = settings::flag(fm::ENABLED, false);
    let mut items = vec![switch(
        fm::ENABLED,
        "Send plays to Last.fm",
        "The same plays go to Last.fm. It needs an API account of your own (free, at \
         last.fm/api/account/create): paste its key and shared secret, then sign in.",
        on,
    )];
    if !on {
        return items;
    }
    items.push(PrefItem {
        value: settings::string(fm::API_KEY).into(),
        ..item(fm::API_KEY, TEXT, "Last.fm API key", "")
    });
    items.push(PrefItem {
        value: settings::string(fm::SECRET).into(),
        on: true,
        ..item(fm::SECRET, TEXT, "Last.fm shared secret", "")
    });
    let user = settings::string(fm::USER);
    let signed_in = !settings::string(fm::SESSION).is_empty() && !user.is_empty();
    let status = fm::status();
    let detail = if !status.is_empty() {
        status
    } else if signed_in {
        format!("Signed in as {user}")
    } else {
        "Opens Last.fm in the browser to allow DJ Sugar.".to_string()
    };
    items.push(PrefItem {
        action: if fm::signing_in() {
            "Waiting…".into()
        } else if signed_in {
            "Sign out".into()
        } else {
            "Sign in".into()
        },
        ..item(
            if signed_in {
                "lastfm_sign_out"
            } else {
                "lastfm_sign_in"
            },
            BUTTON,
            "Last.fm account",
            &detail,
        )
    });
    items
}

impl Prefs {
    /// Settings › Broadcast: the stations to stream to, the now-playing
    /// text, and what a public station needs.
    pub(super) fn broadcast_page(&mut self) -> Vec<PrefItem> {
        use djsugar_broadcast::{Format, ServerKind};
        let Some(b) = self.broadcast.clone() else {
            return vec![item(
                "",
                NOTE,
                "",
                "Broadcasting isn't available: the audio engine didn't start.",
            )];
        };
        let stations = broadcast::stations();
        let mut items = vec![
            switch(
                broadcast::ENABLED,
                "Broadcasting",
                "Shows ON AIR in the top bar, once a station below is set up and switched on.",
                settings::flag(broadcast::ENABLED, false),
            ),
            item("broadcast", HEADING, "STATIONS", ""),
        ];
        if stations.is_empty() {
            items.push(item(
                "",
                NOTE,
                "",
                "Stream your mix to an internet radio: add the Icecast or Shoutcast server your \
                 station or host gave you, then press ON AIR in the top bar.",
            ));
        }
        for (i, s) in stations.iter().enumerate() {
            let id = |field: &str| format!("bc:{i}:{field}");
            let set_up = !s.host.trim().is_empty();
            items.push(switch(
                &id("enabled"),
                &s.name,
                &if set_up {
                    format!("{}. Streams when you go on air.", s.summary())
                } else {
                    "Not set up yet: fill in its server.".to_string()
                },
                s.enabled,
            ));
            if self.editing_station != Some(i) {
                items.push(PrefItem {
                    action: "Edit".into(),
                    ..item(
                        &id("edit"),
                        BUTTON,
                        "Server and stream",
                        self.station_notes.get(&s.id).map_or("", |n| n.as_str()),
                    )
                });
                continue;
            }
            let text = |field: &str, title: &str, detail: &str, value: &str| PrefItem {
                value: value.into(),
                ..item(&id(field), TEXT, title, detail)
            };
            let icecast = s.server == ServerKind::Icecast;
            items.push(text("name", "Name", "What DJ Sugar calls it.", &s.name));
            items.push(PrefItem {
                choices: strings(&["Icecast 2", "Shoutcast 1"]),
                chosen: if icecast { 0 } else { 1 },
                ..item(
                    &id("server"),
                    SEGMENTED,
                    "Server",
                    "Most hosts run Icecast. Shoutcast 2 servers take Shoutcast 1 streams.",
                )
            });
            items.push(text(
                "host",
                "Address",
                "The server's host name or IP address, such as radio.example.org.",
                &s.host,
            ));
            items.push(text("port", "Port", "", &s.port.to_string()));
            if icecast {
                items.push(text(
                    "mount",
                    "Mount point",
                    "Where listeners find the stream, such as /live.",
                    &s.mount,
                ));
                items.push(text("user", "User", "Usually source.", &s.user));
            }
            items.push(PrefItem {
                value: b.password(&s.id).into(),
                on: true,
                ..item(
                    &id("password"),
                    TEXT,
                    "Password",
                    "The source password. Kept in your system's keychain, not in DJ Sugar's \
                     settings.",
                )
            });
            items.push(switch(
                &id("tls"),
                "Secure connection (TLS)",
                "Only if the server takes source connections over https.",
                s.tls,
            ));
            if icecast {
                items.push(PrefItem {
                    choices: strings(&["MP3", "Opus"]),
                    chosen: if s.format == Format::Opus { 1 } else { 0 },
                    ..item(
                        &id("format"),
                        SEGMENTED,
                        "Format",
                        "MP3 plays everywhere and shows each track's title. Opus sounds \
                         better at low bitrates.",
                    )
                });
            }
            let rates: Vec<String> = BITRATES.iter().map(|r| format!("{r} kb/s")).collect();
            items.push(PrefItem {
                choices: strings(&rates),
                chosen: BITRATES.iter().position(|&r| r == s.bitrate).unwrap_or(2) as i32,
                ..item(
                    &id("bitrate"),
                    MENU,
                    "Bitrate",
                    "Higher sounds better and needs a faster upload. 128 kb/s MP3 or 96 kb/s \
                     Opus suit most radios.",
                )
            });
            if s.effective_format() == Format::Opus {
                items.push(switch(
                    &id("opus_titles"),
                    "Send each new title in the stream",
                    "Opus carries titles in the stream itself, by starting it again at each \
                     track. Browsers and VLC follow; some players stop. Off, listeners see the \
                     first title only.",
                    s.opus_titles,
                ));
            }
            items.push(text(
                "stream_name",
                "Stream name",
                "Shown to listeners; the station's name when empty.",
                &s.stream_name,
            ));
            items.push(text("genre", "Genre", "", &s.genre));
            items.push(text("website", "Website", "", &s.website));
            items.push(text("description", "Description", "", &s.description));
            items.push(switch(
                &id("public"),
                "List in the directory",
                "Asks the server to list the stream in its public directory.",
                s.public,
            ));
            items.push(PrefItem {
                action: "Test".into(),
                ..item(
                    &id("test"),
                    BUTTON,
                    "Test the connection",
                    self.station_notes.get(&s.id).map_or(
                        "Connects with these settings and hangs up, without going on air.",
                        |n| n.as_str(),
                    ),
                )
            });
            let confirm = self.removing.as_deref() == Some(id("remove").as_str());
            items.push(PrefItem {
                action: if confirm { "Remove" } else { "Remove…" }.into(),
                ..item(
                    &id("remove"),
                    BUTTON,
                    "Remove this station",
                    if confirm {
                        "Press Remove again to remove it and its password."
                    } else {
                        ""
                    },
                )
            });
            items.push(PrefItem {
                action: "Done".into(),
                ..item(&id("edit"), BUTTON, "", "")
            });
        }
        items.push(PrefItem {
            action: "Add".into(),
            ..item("bc_add", BUTTON, "Add a station", "")
        });
        items.push(item("", HEADING, "BEFORE YOU GO ON AIR", ""));
        items.push(item(
            "",
            NOTE,
            "",
            "While on air, the limiter keeps peaks under the ceiling set in Settings › Audio › \
             Recording, and the ON AIR panel's loudness meter shows how loud the stream is \
             against your target.",
        ));
        items.push(item(
            "",
            NOTE,
            "",
            "Playing music to the public needs licences in most countries, from bodies such as \
             PRS, GEMA, SACEM, ASCAP, BMI and SoundExchange. Many hosts include them; check \
             yours before you stream other people's music.",
        ));
        if let Some(trouble) = b.keychain_trouble() {
            items.push(item(
                "",
                NOTE,
                "",
                &format!(
                    "Passwords can't be kept in your system's keychain ({trouble}), so DJ Sugar \
                     remembers them until it quits."
                ),
            ));
        }
        items
    }

    /// Settings › Broadcast › Now playing: the title sent to listeners and
    /// the now-playing feed for streaming software.
    pub(super) fn now_playing_page(&self) -> Vec<PrefItem> {
        let mut items = vec![item("", HEADING, "STREAM TITLE", "")];
        items.push(PrefItem {
            value: settings::string(broadcast::TITLE_TEMPLATE).into(),
            ..item(
                broadcast::TITLE_TEMPLATE,
                TEXT,
                "Title sent to listeners",
                "The track heard most, a couple of seconds after it takes over. {artist}, \
                 {title} and {dj} are filled in; plain text is sent as it is. Empty is \
                 {artist} - {title}.",
            )
        });
        items.push(PrefItem {
            value: settings::string(broadcast::DJ_NAME).into(),
            ..item(
                broadcast::DJ_NAME,
                TEXT,
                "DJ name",
                "For {dj} in the title.",
            )
        });
        items.extend(now_playing_items());
        items
    }

    /// Changes station `i` and tells the broadcaster.
    pub(super) fn edit_station(
        &mut self,
        i: usize,
        f: impl FnOnce(&mut djsugar_broadcast::Station),
    ) {
        if let Some(b) = &self.broadcast {
            b.edit(i, f);
        }
    }

    /// Settings › Broadcast's buttons; false if `id` isn't one of them.
    pub(super) fn broadcast_pressed(&mut self, id: &str) -> bool {
        let Some(b) = self.broadcast.clone() else {
            return false;
        };
        if id == "bc_add" {
            self.editing_station = Some(b.add());
            self.refresh();
            return true;
        }
        let Some((i, field)) = station_field(id) else {
            return false;
        };
        match field {
            "edit" => {
                self.editing_station = (self.editing_station != Some(i)).then_some(i);
                self.removing = None;
            }
            "remove" => {
                if self.removing.as_deref() == Some(id) {
                    b.remove(i);
                    self.removing = None;
                    self.editing_station = None;
                } else {
                    self.removing = Some(id.to_string());
                }
            }
            "test" => {
                let Some(station) = broadcast::stations().into_iter().nth(i) else {
                    return true;
                };
                self.station_notes
                    .insert(station.id.clone(), "Connecting…".into());
                let tx = self.tested_tx.clone();
                let sid = station.id.clone();
                b.test(i, move |result| {
                    let note = match result {
                        Ok(()) => {
                            "Connected: the server took the stream. Ready to go on air.".to_string()
                        }
                        Err(e) => e,
                    };
                    let _ = tx.send((sid, note));
                });
            }
            _ => return false,
        }
        self.refresh();
        true
    }
}

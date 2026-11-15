//! Sends one of the DJ's lists or a history session to Spotify as a new
//! private playlist, its tracks in the list's order (a session as it was
//! played). Tracks Spotify doesn't have are left out and named.

use djsugar_library::ListEntry;

use super::{Browser, Contents};
use crate::spotify::{self, Request, Wanted};
use crate::LibraryEdit;

impl Browser {
    pub(super) fn install_spotify_export(
        edit: &LibraryEdit,
        with: &(impl Fn(&dyn Fn(&mut Browser)) + Clone + 'static),
    ) {
        let w = with.clone();
        edit.on_export_spotify(move |row| w(&|b| b.export_to_spotify(row as usize)));
    }

    /// Exports the tree's row `row` to Spotify.
    fn export_to_spotify(&mut self, row: usize) {
        let Some(&node) = self.visible_nodes.get(row) else {
            return;
        };
        let Contents::Playlist(list) = self.nodes[node].contents else {
            return;
        };
        let name = self.node_title(node);
        let history = self.nodes[node].history;
        let Some(st) = self.streams.iter().find(|s| s.id == "spotify") else {
            return;
        };
        if !st.state.connected {
            return self.export_status(
                list,
                "Sign in to Spotify (Spotify on the left) to export this list.".into(),
            );
        }
        let tracks = self.wanted(list);
        if tracks.is_empty() {
            return self.export_status(list, "Nothing to export: the list is empty.".into());
        }
        let description = if history {
            format!("Played with DJ Sugar on {name}, in the order played.")
        } else {
            "Made with DJ Sugar.".into()
        };
        let count = tracks.len();
        st.service.send(Request::Export {
            list,
            name,
            description,
            tracks,
        });
        self.export_status(list, format!("Sending {count} tracks to Spotify…"));
    }

    /// The list's tracks, in order, as Spotify is to find them.
    fn wanted(&self, list: i64) -> Vec<Wanted> {
        let Some(db) = &self.db else {
            return Vec::new();
        };
        db.playlist_entries(list)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|e| match e {
                ListEntry::Track(id) => {
                    let t = &self.library[*self.index_of.get(&id)?];
                    Some(Wanted {
                        id: String::new(),
                        isrc: t.info.isrc.clone(),
                        artist: t.artist.clone(),
                        title: t.title.clone(),
                        duration: t.duration,
                    })
                }
                ListEntry::Streamed(s) => Some(Wanted {
                    id: if s.service == "spotify" {
                        s.id.clone()
                    } else {
                        String::new()
                    },
                    isrc: s.isrc,
                    artist: s.artist,
                    title: s.title,
                    duration: s.duration,
                }),
            })
            .collect()
    }

    /// Spotify made the playlist: say how it went and open it.
    pub(super) fn exported(&mut self, list: i64, url: &str, added: usize, missing: &[String]) {
        let mut text = format!("On Spotify: {added} tracks, in order");
        if !missing.is_empty() {
            let names: Vec<&str> = missing.iter().take(3).map(String::as_str).collect();
            let more = missing.len().saturating_sub(names.len());
            text = format!(
                "{text}. Not on Spotify ({}): {}{}",
                missing.len(),
                names.join(", "),
                if more > 0 {
                    format!(" and {more} more")
                } else {
                    String::new()
                }
            );
            eprintln!("Not found on Spotify: {}", missing.join("; "));
        }
        spotify::open_in_browser(url);
        self.export_status(list, text);
    }

    pub(super) fn export_status(&mut self, list: i64, text: String) {
        self.export_status = Some((list, text));
        if self.current_list() == Some(list) {
            self.refresh_rows();
        }
    }

    /// What exporting the list shown did, for its header.
    pub(super) fn export_note(&self) -> Option<&str> {
        let (list, text) = self.export_status.as_ref()?;
        (self.current_list() == Some(*list)).then_some(text.as_str())
    }

    /// The DJ's list or history session shown, if one is.
    fn current_list(&self) -> Option<i64> {
        match self.nodes.get(self.current_node)?.contents {
            Contents::Playlist(id) => Some(id),
            _ => None,
        }
    }
}

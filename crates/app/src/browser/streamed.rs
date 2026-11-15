//! Streamed tracks standing in lists brought in from another app (see
//! [`djsugar_library::ListEntry::Streamed`]): shown greyed in their place,
//! matched to the collection as files arrive (the same ISRC, else the same
//! artist and title at the same length) or by hand with Change Match…, and
//! opened on their service's page. While the DJ is signed in to TIDAL or
//! Spotify, their ISRCs are fetched from there, so a download matches surely.

use std::collections::HashMap;

use djsugar_engine::import::StreamedTrack;
use djsugar_library::ListEntry;

use super::columns::InList;
use super::{Browser, Contents, Manual};
use crate::spotify::{Request, StreamTrack};

/// An entry of the imported list shown: a collection track (an index into
/// `library`) or a streamed track (an index into [`Placeholders::tracks`]).
#[derive(Clone, Copy)]
pub(super) enum Placed {
    Track(usize),
    Streamed(usize),
}

#[derive(Default)]
pub(super) struct Placeholders {
    /// The list shown, when streamed tracks stand in it.
    pub entries: Option<Vec<Placed>>,
    /// Where each entry is in the list, and when it was played in a
    /// history session.
    pub lists: Vec<InList>,
    /// Those tracks, with their service ("tidal").
    pub tracks: Vec<(String, StreamTrack)>,
    /// Matches the DJ set by hand, by service and then by
    /// [`StreamTrack::match_key`]: the collection track's id, or None.
    pub manual: HashMap<String, Manual>,
}

/// How a streamed track shows in the list. Its service stands where the
/// album would, so the DJ sees where it is.
fn stream_track(s: &StreamedTrack) -> StreamTrack {
    StreamTrack {
        id: s.id.clone(),
        page: s.page_url().unwrap_or_default(),
        isrc: s.isrc.clone(),
        title: s.title.clone(),
        artists: if s.artist.is_empty() {
            Vec::new()
        } else {
            vec![s.artist.clone()]
        },
        album: s.service_name().to_string(),
        duration: s.duration,
    }
}

impl Browser {
    /// Reads the shown list again when streamed tracks stand in it.
    pub(super) fn load_placed(&mut self) {
        self.placed.entries = None;
        self.placed.lists.clear();
        self.placed.tracks.clear();
        let Some(&Contents::Playlist(id)) = self.nodes.get(self.current_node).map(|n| &n.contents)
        else {
            return;
        };
        let Some(db) = &self.db else { return };
        let Ok(entries) = db.playlist_entries(id) else {
            return;
        };
        if !entries.iter().any(|e| matches!(e, ListEntry::Streamed(_))) {
            return;
        }
        let history = self.nodes[self.current_node].history;
        let times = if history {
            db.playlist_times(id).unwrap_or_default()
        } else {
            Vec::new()
        };
        let mut placed = Vec::with_capacity(entries.len());
        for (n, e) in entries.into_iter().enumerate() {
            let list = InList {
                order: n + 1,
                played_at: times.get(n).copied().flatten(),
                set_start: false,
            };
            match e {
                ListEntry::Track(t) => {
                    if let Some(&i) = self.index_of.get(&t) {
                        placed.push(Placed::Track(i));
                        self.placed.lists.push(list);
                    }
                }
                ListEntry::Streamed(s) => {
                    if !self.placed.manual.contains_key(&s.service) {
                        let manual = db.stream_matches(&s.service).unwrap_or_default();
                        self.placed.manual.insert(s.service.clone(), manual);
                    }
                    placed.push(Placed::Streamed(self.placed.tracks.len()));
                    self.placed.lists.push(list);
                    self.placed
                        .tracks
                        .push((s.service.clone(), stream_track(&s)));
                }
            }
        }
        self.placed.entries = Some(placed);
    }

    /// The collection track (an index into `library`) each streamed track
    /// shown is, if the collection has it now.
    pub(super) fn placed_matches(&self) -> Vec<Option<usize>> {
        let tracks: Vec<(&StreamTrack, Option<&Manual>)> = self
            .placed
            .tracks
            .iter()
            .map(|(service, t)| (t, self.placed.manual.get(service)))
            .collect();
        self.match_streamed(&tracks, true)
    }

    /// The streamed track at `row` of the imported list shown, with its
    /// service.
    pub(super) fn placed_at(&self, row: usize) -> Option<&(String, StreamTrack)> {
        self.placed.entries.as_ref()?;
        self.placed.tracks.get((*self.stream_rows.get(row)?)?)
    }

    /// Asks a streaming service the DJ just signed in to for the ISRCs of
    /// its tracks standing in lists, so a download matches its placeholder
    /// surely.
    pub(super) fn look_up_placed(&self, stream: usize) {
        let st = &self.streams[stream];
        let Some(db) = &self.db else { return };
        let ids = db.streamed_without_isrc(st.id).unwrap_or_default();
        if !ids.is_empty() {
            st.service.send(Request::Lookup(ids));
        }
    }

    /// Keeps what the service said of tracks asked for, and shows it.
    pub(super) fn found_placed(&mut self, stream: usize, found: Vec<StreamTrack>) {
        let service = self.streams[stream].id;
        let Some(db) = &self.db else { return };
        for t in found {
            let s = StreamedTrack {
                service: service.to_string(),
                id: t.id.clone(),
                artist: t.artist(),
                title: t.title.clone(),
                duration: t.duration,
                isrc: t.isrc.clone(),
                ..Default::default()
            };
            if let Err(e) = db.describe_streamed(&s) {
                eprintln!("can't keep what {service} said of a track: {e}");
                return;
            }
        }
        if self.placed.entries.is_some() {
            self.refresh_rows();
        }
    }
}

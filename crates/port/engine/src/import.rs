//! What an add-on brings in from another DJ app's library: its tracks, and
//! the playlists, folders and history it keeps.

use std::path::PathBuf;

use crate::track::Track;

/// Another app's library, as read by an importer.
#[derive(Clone, Debug, Default)]
pub struct ImportedLibrary {
    /// Local files only, with what the app knows about them.
    pub tracks: Vec<Track>,
    /// Lists in the groups the DJ chose, in the app's order.
    pub lists: Vec<ImportedList>,
    /// When a USB stick's database was written (Unix seconds). Its copies
    /// of the library's own files then carry edits made on the players,
    /// which come back when they are newer than the DJ's here.
    pub written: Option<i64>,
}

/// A playlist, history session or smart list from another app.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedList {
    /// The folders it sits in, outermost first ("GY Pro", "GY Sets").
    pub folders: Vec<String>,
    pub name: String,
    pub kind: ImportedListKind,
    /// Its tracks in order. Streamed tracks stay as placeholders; files
    /// the library doesn't have (files that have gone) are left out when
    /// it is saved.
    pub entries: Vec<ImportedEntry>,
}

impl ImportedList {
    /// Names the list among its app's, for the DJ's choice of which come
    /// in: its kind, folders and name ("list:Sets/2023/Friday").
    pub fn key(&self) -> String {
        let kind = match self.kind {
            ImportedListKind::List => "list",
            ImportedListKind::History { .. } => "history",
            ImportedListKind::Smart { .. } => "smart",
        };
        let mut path = self.folders.clone();
        path.push(self.name.clone());
        format!("{kind}:{}", path.join("/"))
    }

    /// Whether it comes in when the DJ hasn't said: all but smart lists,
    /// whose searches may not mean the same here.
    pub fn wanted_by_default(&self) -> bool {
        !matches!(self.kind, ImportedListKind::Smart { .. })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ImportedListKind {
    List,
    /// A session of played tracks; `started` is a Unix time.
    History {
        started: i64,
    },
    /// A saved search in DJ Sugar's search syntax.
    Smart {
        query: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportedEntry {
    pub path: PathBuf,
    /// When it was played, for a history session (Unix time).
    pub played_at: Option<i64>,
    /// A track on a streaming service rather than a file: it stands in the
    /// list until the collection has the song.
    pub streamed: Option<StreamedTrack>,
}

impl ImportedEntry {
    /// The entry for `path` as another app wrote it: a streamed track when
    /// it names one (see [`StreamedTrack::parse`]), else a file.
    pub fn new(path: impl Into<PathBuf>, played_at: Option<i64>) -> Self {
        let path = path.into();
        let streamed = path.to_str().and_then(StreamedTrack::parse);
        Self {
            path,
            played_at,
            streamed,
        }
    }
}

/// A track on a streaming service in another app's list: the service, its
/// id there and what the app knew of it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StreamedTrack {
    /// "tidal", "spotify", "soundcloud", "beatport", "beatsource",
    /// "deezer" or "applemusic"; "online" when the app didn't say which.
    pub service: String,
    /// The service's id for the track; empty when not known.
    pub id: String,
    /// As the other app wrote it ("netsearch://td172486431").
    pub uri: String,
    pub artist: String,
    pub title: String,
    /// Seconds; 0 when not known.
    pub duration: f64,
    pub isrc: String,
}

/// The services a streamed track can be on, by the names other apps use.
const SERVICES: [&str; 6] = [
    "tidal",
    "spotify",
    "soundcloud",
    "beatport",
    "beatsource",
    "deezer",
];

/// VirtualDJ's `netsearch://` prefixes. `td` is TIDAL, as seen in real
/// libraries; the others follow VirtualDJ's catalogue names.
const NETSEARCH: [(&str, &str); 5] = [
    ("td", "tidal"),
    ("sc", "soundcloud"),
    ("bp", "beatport"),
    ("bs", "beatsource"),
    ("dz", "deezer"),
];

impl StreamedTrack {
    /// The streamed track `uri` names, if it names one rather than a file:
    /// VirtualDJ's `netsearch://td123`, a `tidal:tracks:123` or
    /// `spotify:track:abc` URI, a service's web address, or any other
    /// `scheme://` address (an "online" track of unknown service).
    pub fn parse(uri: &str) -> Option<Self> {
        let uri = uri.trim();
        let streamed = |service: &str, id: &str| {
            Some(Self {
                service: service.to_string(),
                id: id.to_string(),
                uri: uri.to_string(),
                ..Default::default()
            })
        };
        // rekordbox XML puts a service URI where a file's address would be.
        let bare = uri
            .strip_prefix("file://localhost/")
            .or_else(|| uri.strip_prefix("file:///"))
            .unwrap_or(uri);
        if let Some(rest) = bare.strip_prefix("netsearch://") {
            let rest = rest.trim_matches('/');
            for (prefix, service) in NETSEARCH {
                if let Some(id) = rest.strip_prefix(prefix) {
                    return streamed(service, id);
                }
            }
            return streamed("online", rest);
        }
        // service:kind:id ("tidal:tracks:123", "spotify:track:abc").
        let parts: Vec<&str> = bare.split(':').collect();
        if parts.len() >= 3 {
            let service = parts[0].to_ascii_lowercase();
            if SERVICES.contains(&service.as_str()) {
                return streamed(&service, parts[parts.len() - 1]);
            }
        }
        if let Some(rest) = bare
            .strip_prefix("https://")
            .or_else(|| bare.strip_prefix("http://"))
        {
            let mut segments = rest.split(['/', '?', '#']).filter(|s| !s.is_empty());
            let host = segments.next().unwrap_or_default().to_ascii_lowercase();
            let path: Vec<&str> = segments.collect();
            let service = SERVICES
                .iter()
                .find(|s| host == format!("{s}.com") || host.ends_with(&format!(".{s}.com")));
            if let Some(service) = service {
                // The id follows "track": tidal.com/browse/track/123,
                // open.spotify.com/track/abc, beatport.com/track/name/123.
                let id = match path.iter().position(|s| *s == "track") {
                    Some(i) if *service == "beatport" || *service == "beatsource" => {
                        path.get(i + 2).or(path.get(i + 1))
                    }
                    Some(i) => path.get(i + 1),
                    None => None,
                };
                return streamed(service, id.copied().unwrap_or_default());
            }
            return streamed("online", "");
        }
        if bare.contains("://") {
            return streamed("online", "");
        }
        None
    }

    /// The service's name as people write it.
    pub fn service_name(&self) -> &str {
        match self.service.as_str() {
            "tidal" => "TIDAL",
            "spotify" => "Spotify",
            "soundcloud" => "SoundCloud",
            "beatport" => "Beatport",
            "beatsource" => "Beatsource",
            "deezer" => "Deezer",
            "applemusic" => "Apple Music",
            _ => "Online",
        }
    }

    /// The track's page on its service, where there is one to link to.
    pub fn page_url(&self) -> Option<String> {
        if self.id.is_empty() {
            return None;
        }
        let id = &self.id;
        Some(match self.service.as_str() {
            "tidal" => format!("https://tidal.com/browse/track/{id}"),
            "spotify" => format!("https://open.spotify.com/track/{id}"),
            "deezer" => format!("https://www.deezer.com/track/{id}"),
            "beatport" => format!("https://www.beatport.com/track/-/{id}"),
            "beatsource" => format!("https://www.beatsource.com/track/-/{id}"),
            _ => return None,
        })
    }
}

/// A kind of list an importer can bring in, which the DJ can turn on or
/// off in Settings > Library.
#[derive(Clone, Debug, PartialEq)]
pub struct ListGroup {
    /// Stable, saved in the settings ("playlists", "history").
    pub id: &'static str,
    pub title: &'static str,
    pub detail: &'static str,
    pub on_by_default: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(uri: &str) -> Option<(String, String)> {
        StreamedTrack::parse(uri).map(|s| (s.service, s.id))
    }

    #[test]
    fn reads_streamed_tracks_from_the_ways_apps_write_them() {
        let s = |a: &str, b: &str| Some((a.to_string(), b.to_string()));
        assert_eq!(parsed("netsearch://td172486431"), s("tidal", "172486431"));
        assert_eq!(parsed("netsearch://zz9"), s("online", "zz9"));
        assert_eq!(parsed("tidal:tracks:123"), s("tidal", "123"));
        assert_eq!(parsed("spotify:track:4uLU6h"), s("spotify", "4uLU6h"));
        assert_eq!(parsed("file://localhost/tidal:tracks:55"), s("tidal", "55"));
        assert_eq!(
            parsed("https://tidal.com/browse/track/77?u"),
            s("tidal", "77")
        );
        assert_eq!(
            parsed("https://open.spotify.com/track/abc"),
            s("spotify", "abc")
        );
        assert_eq!(
            parsed("https://www.beatport.com/track/glue/123"),
            s("beatport", "123")
        );
        assert_eq!(parsed("https://example.com/x"), s("online", ""));
        // Files are files, Windows drives included.
        assert_eq!(parsed("/Music/a.mp3"), None);
        assert_eq!(parsed("C:/Music/a:b:c.mp3"), None);
    }

    #[test]
    fn links_to_the_track_on_its_service() {
        let t = StreamedTrack::parse("netsearch://td5").unwrap();
        assert_eq!(t.service_name(), "TIDAL");
        assert_eq!(
            t.page_url().as_deref(),
            Some("https://tidal.com/browse/track/5")
        );
        assert_eq!(
            StreamedTrack::parse("netsearch://zz9").unwrap().page_url(),
            None
        );
    }
}

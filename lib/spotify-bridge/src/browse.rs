//! Browsing through Spotify's own client endpoints (spclient), on the
//! librespot session: Liked Songs, the user's playlists, playlist tracks
//! and search. Not the public Web API: its quota is per client id, and the
//! client id librespot logs in with is shared by every librespot app, so
//! it answers 429 (rate limited) nearly always.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use futures_util::stream::{self, StreamExt};
use librespot_core::error::ErrorKind;
use librespot_core::session::Session;
use librespot_core::spotify_id::SpotifyId;
use librespot_core::spotify_uri::SpotifyUri;
use librespot_metadata::image::Images;
use librespot_metadata::{Metadata, Playlist, Track};
use librespot_protocol::context::Context;
use librespot_protocol::context_page::ContextPage;
use librespot_protocol::playlist4_external::SelectedListContent;
use protobuf::Message;
use serde_json::{json, Value};

/// Parallel metadata requests
const CONCURRENCY: usize = 12;
/// Context pages followed at most (Liked Songs of ~10000 tracks)
const MAX_PAGES: usize = 200;
const IMAGE_URL: &str = "https://i.scdn.co/image/";

/// The track uris of the lists opened, so a next page needs no new lookup
fn lists() -> &'static Mutex<HashMap<String, Vec<String>>> {
    static LISTS: OnceLock<Mutex<HashMap<String, Vec<String>>>> = OnceLock::new();
    LISTS.get_or_init(Default::default)
}

fn image_url(images: &Images) -> String {
    // The smallest one of at least 64 px suits a row
    let mut sorted: Vec<_> = images.iter().collect();
    sorted.sort_by_key(|image| image.width);
    sorted
        .iter()
        .find(|image| image.width >= 64)
        .or_else(|| sorted.last())
        .and_then(|image| image.id.to_base16().ok())
        .map(|hex| format!("{IMAGE_URL}{hex}"))
        .unwrap_or_default()
}

fn track_json(track: &Track) -> Option<Value> {
    let id = track.id.to_id().ok()?;
    let artists: Vec<&str> = track.artists.iter().map(|a| a.name.as_str()).collect();
    Some(json!({
        "id": id,
        "uri": format!("spotify:track:{id}"),
        "title": track.name,
        "artist": artists.join(", "),
        "album": track.album.name,
        "durationSec": (track.duration + 500) / 1000,
        "artwork": image_url(&track.album.covers),
        "explicit": track.is_explicit,
    }))
}

/// The tracks of `uris`, in their order; tracks Spotify has no data for are
/// left out. Any other failure (connection, login) fails the page, so it is
/// not skipped for good
async fn tracks(session: &Session, uris: &[String]) -> Result<Vec<Value>, String> {
    let fetched: Vec<Option<Track>> =
        stream::iter(uris.iter().filter_map(|uri| SpotifyUri::from_uri(uri).ok()))
            .map(|uri| async move {
                match Track::get(session, &uri).await {
                    Ok(track) => Ok(Some(track)),
                    Err(e)
                        if matches!(
                            e.kind,
                            ErrorKind::NotFound
                                | ErrorKind::InvalidArgument
                                | ErrorKind::FailedPrecondition
                        ) =>
                    {
                        Ok(None)
                    }
                    Err(e) => Err(e.to_string()),
                }
            })
            .buffered(CONCURRENCY)
            .collect::<Vec<Result<Option<Track>, String>>>()
            .await
            .into_iter()
            .collect::<Result<_, _>>()?;
    Ok(fetched
        .iter()
        .flatten()
        .filter_map(track_json)
        .collect())
}

fn page_track_uris(page: &ContextPage, out: &mut Vec<String>) {
    out.extend(
        page.tracks
            .iter()
            .map(|t| t.uri().to_owned())
            .filter(|uri| uri.starts_with("spotify:track:")),
    );
}

/// The track uris of a context (Liked Songs, search), following its pages
async fn context_tracks(session: &Session, uri: &str) -> Result<Vec<String>, String> {
    let spclient = session.spclient();
    let context: Context = spclient
        .get_context(uri)
        .await
        .map_err(|e| format!("Spotify: {e}"))?;
    let mut uris = Vec::new();
    let mut next: Vec<String> = Vec::new();
    for page in &context.pages {
        page_track_uris(page, &mut uris);
        if page.tracks.is_empty() && !page.page_url().is_empty() {
            next.push(page.page_url().to_owned());
        } else if !page.next_page_url().is_empty() {
            next.push(page.next_page_url().to_owned());
        }
    }
    let mut followed = 0;
    while let Some(url) = next.pop() {
        followed += 1;
        if followed > MAX_PAGES {
            break;
        }
        let bytes = spclient
            .get_next_page(&url)
            .await
            .map_err(|e| format!("Spotify: {e}"))?;
        let text = String::from_utf8_lossy(&bytes);
        let page: ContextPage =
            protobuf_json_mapping::parse_from_str_with_options(&text, &lenient())
                .map_err(|e| format!("Spotify page: {e}"))?;
        page_track_uris(&page, &mut uris);
        if !page.next_page_url().is_empty() {
            next.push(page.next_page_url().to_owned());
        }
    }
    Ok(uris)
}

fn lenient() -> protobuf_json_mapping::ParseOptions {
    protobuf_json_mapping::ParseOptions {
        ignore_unknown_fields: true,
        ..Default::default()
    }
}

async fn playlist_tracks(session: &Session, id: &str) -> Result<Vec<String>, String> {
    let id = SpotifyId::from_base62(id).map_err(|e| format!("Playlist id: {e}"))?;
    let playlist = Playlist::get(session, &SpotifyUri::Playlist { id, user: None })
        .await
        .map_err(|e| format!("Spotify playlist: {e}"))?;
    Ok(playlist
        .tracks()
        .filter(|uri| matches!(uri, SpotifyUri::Track { .. }))
        .filter_map(|uri| uri.to_uri().ok())
        .collect())
}

fn list_json(name: &str, length: i32, picture: &[u8]) -> (String, i32, String) {
    let artwork = if picture.is_empty() {
        String::new()
    } else {
        let hex: String = picture.iter().map(|b| format!("{b:02x}")).collect();
        format!("{IMAGE_URL}{hex}")
    };
    (name.to_owned(), length, artwork)
}

/// The user's playlists (folders flattened), in their order
async fn playlists(session: &Session) -> Result<Vec<Value>, String> {
    let spclient = session.spclient();
    let bytes = spclient
        .get_rootlist(0, Some(10_000))
        .await
        .map_err(|e| format!("Spotify playlists: {e}"))?;
    let rootlist = SelectedListContent::parse_from_bytes(&bytes)
        .map_err(|e| format!("Spotify playlists: {e}"))?;
    let contents = rootlist.contents.get_or_default();
    // Decorated with the attributes: name and length come with the list
    let aligned = contents.meta_items.len() == contents.items.len();
    // Owned up front: the futures below run on another thread
    let known: Vec<(String, Option<(String, i32, String)>)> = contents
        .items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            let id = item.uri().strip_prefix("spotify:playlist:")?.to_owned();
            let known = aligned
                .then(|| {
                    let m = &contents.meta_items[i];
                    let a = m.attributes.get_or_default();
                    list_json(a.name(), m.length(), a.picture())
                })
                .filter(|(name, ..)| !name.is_empty());
            Some((id, known))
        })
        .collect();
    let entries = known.into_iter().map(|(id, known)| async move {
        let (name, count, artwork) = match known {
            Some(known) => known,
            None => {
                let pid = SpotifyId::from_base62(&id).ok()?;
                let bytes = session.spclient().get_playlist(&pid).await.ok()?;
                let list = SelectedListContent::parse_from_bytes(&bytes).ok()?;
                let a = list.attributes.get_or_default();
                list_json(a.name(), list.length(), a.picture())
            }
        };
        Some(json!({ "id": id, "name": name, "count": count, "artwork": artwork }))
    });
    Ok(stream::iter(entries)
        .buffered(CONCURRENCY)
        .filter_map(|entry| async move { entry })
        .collect()
        .await)
}

/// The account's display name, else its username
async fn profile(session: &Session) -> Value {
    let user = session.username();
    let name = session
        .spclient()
        .get_user_profile(&user, Some(0), Some(0))
        .await
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|j| j["name"].as_str().map(str::to_owned))
        .filter(|name| !name.is_empty())
        .unwrap_or(user);
    json!({ "name": name })
}

/// `request`: "profile", "playlists", "liked", "playlist:<id>" or
/// "search:<query>". Track lists come `limit` at a time from `offset`:
/// {"items": [...], "next": <offset of the next page, -1 at the end>}
pub async fn browse(
    session: &Session,
    request: &str,
    offset: usize,
    limit: usize,
) -> Result<Value, String> {
    match request {
        "profile" => return Ok(profile(session).await),
        "playlists" => return Ok(json!({ "items": playlists(session).await?, "next": -1 })),
        _ => {}
    }
    let cached = (offset > 0)
        .then(|| lists().lock().unwrap().get(request).cloned())
        .flatten();
    let uris = match cached {
        Some(uris) => uris,
        None => {
            let uris = if request == "liked" {
                let user = session.username();
                context_tracks(session, &format!("spotify:user:{user}:collection")).await?
            } else if let Some(id) = request.strip_prefix("playlist:") {
                playlist_tracks(session, id).await?
            } else if let Some(query) = request.strip_prefix("search:") {
                let words: Vec<String> = query.split_whitespace().map(search_word).collect();
                context_tracks(session, &format!("spotify:search:{}", words.join("+"))).await?
            } else {
                return Err(format!("Unknown list: {request}"));
            };
            let mut lists = lists().lock().unwrap();
            lists.clear(); // one list open at a time
            lists.insert(request.to_owned(), uris.clone());
            uris
        }
    };
    let end = uris.len().min(offset + limit);
    let page = uris.get(offset..end).unwrap_or_default();
    let next = if end < uris.len() { end as i64 } else { -1 };
    Ok(json!({ "items": tracks(session, page).await?, "next": next }))
}

/// A search word inside a spotify:search: uri (no ':' or '+' of its own)
fn search_word(word: &str) -> String {
    word.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_words_are_escaped() {
        assert_eq!(search_word("daft"), "daft");
        assert_eq!(search_word("a:b+c"), "a%3Ab%2Bc");
        assert_eq!(search_word("é"), "%C3%A9");
    }

    #[test]
    fn context_page_json_parses() {
        let page: ContextPage = protobuf_json_mapping::parse_from_str_with_options(
            r#"{"tracks":[{"uri":"spotify:track:4uLU6hMCjMI75M1A2tKUQC","uid":"x",
               "metadata":{"k":"v"}},{"uri":"spotify:episode:abc"}],
               "next_page_url":"hm://collection/next","unknown":1}"#,
            &lenient(),
        )
        .unwrap();
        let mut uris = Vec::new();
        page_track_uris(&page, &mut uris);
        assert_eq!(uris, ["spotify:track:4uLU6hMCjMI75M1A2tKUQC"]);
        assert_eq!(page.next_page_url(), "hm://collection/next");
    }

    #[test]
    fn playlist_picture_is_an_image_url() {
        let (_, _, url) = list_json("x", 3, &[0xab, 0x01]);
        assert_eq!(url, "https://i.scdn.co/image/ab01");
    }
}

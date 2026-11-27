//! A YouTube Music account: its liked songs, its playlists and its channels, asked for the way the web player
//! asks and signed with the cookies of a sign-in made on YouTube's own page.

use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde_json::{Map, Value, json};
use sha1::{Digest, Sha1};

use crate::http::{agent, read};
use crate::youtube::continuation;
use crate::{Error, Hit, Result};

const ORIGIN: &str = "https://music.youtube.com";
/// The web player's version when its page does not tell; an old one gets empty lists.
const VERSION: &str = "1.20261104.00.00";
const AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36";
const TIMEOUT: Duration = Duration::from_secs(20);
/// Playlists come a page at a time; a library rarely has more.
const LIST_PAGES: usize = 10;
/// The search that gives songs, the releases, and not videos.
const SONGS: &str = "EgWKAQIIAWoMEA4QChADEAQQCRAF";
/// The liked songs, the playlists, and what a playlist's ID starts with among the hits.
pub const LIKED: &str = "liked";
pub const LISTS: &str = "playlists";
pub const LIST: &str = "playlist:";
const AGAIN: &str = "YouTube wants a new sign-in";

/// Where the sign-in starts, the host whose cookies it leaves, and the cookie that tells it is done.
pub const SIGN_IN: (&str, &str, &str) = (
    "https://accounts.google.com/ServiceLogin?ltmpl=music&service=youtube&passive=true&continue=https%3A%2F%2Fmusic.youtube.com%2F",
    "music.youtube.com",
    "SAPISID",
);

#[derive(Debug, Default)]
pub struct Page {
    pub hits: Vec<Hit>,
    pub more: Option<String>,
}

/// One of the account's channels; the main one has no page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Channel {
    pub name: String,
    pub handle: String,
    pub page: String,
}

/// What is kept between runs: the cookies, then the chosen channel's page and name.
#[derive(Debug, Default)]
struct Kept {
    cookies: String,
    page: String,
    name: String,
}

#[derive(Clone, Debug)]
struct Player {
    version: String,
    visitor: String,
}

#[derive(Debug)]
pub struct Account {
    file: PathBuf,
    kept: Mutex<Kept>,
    player: Mutex<Option<Player>>,
}

impl Account {
    /// The account kept in `file`, signed out when there is none.
    pub fn open(file: PathBuf) -> Self {
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        let mut lines = text.lines().map(|l| l.trim().to_string());
        let mut next = || lines.next().unwrap_or_default();
        let kept = Kept { cookies: next(), page: next(), name: next() };
        Self { file, kept: Mutex::new(kept), player: Mutex::new(None) }
    }

    fn kept(&self) -> std::sync::MutexGuard<'_, Kept> {
        self.kept.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn signed_in(&self) -> bool {
        !sapisid(&self.kept().cookies).is_empty()
    }

    pub fn name(&self) -> String {
        self.kept().name.clone()
    }

    /// The chosen channel's page; empty for the main one.
    pub fn page(&self) -> String {
        self.kept().page.clone()
    }

    /// Takes the cookies of a sign-in; false when they are not of a signed in account.
    pub fn sign_in(&self, cookies: &str) -> bool {
        if sapisid(cookies).is_empty() {
            return false;
        }
        *self.kept() = Kept { cookies: cookies.trim().to_string(), ..Kept::default() };
        self.save();
        true
    }

    pub fn sign_out(&self) {
        let _ = std::fs::remove_file(&self.file);
        *self.kept() = Kept::default();
    }

    /// Asks from now on as one of the account's channels.
    pub fn choose(&self, channel: &Channel) {
        {
            let mut k = self.kept();
            k.page.clone_from(&channel.page);
            k.name.clone_from(&channel.name);
        }
        self.save();
    }

    fn save(&self) {
        let text = {
            let k = self.kept();
            format!("{}\n{}\n{}", k.cookies, k.page, k.name)
        };
        if let Some(dir) = self.file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if std::fs::write(&self.file, text).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&self.file, std::fs::Permissions::from_mode(0o600));
            }
        }
    }

    /// The web player's version and the visitor it gives; read from its page once, without the account's cookies.
    fn player(&self) -> Player {
        if let Some(p) = self.player.lock().unwrap_or_else(PoisonError::into_inner).clone() {
            return p;
        }
        let req = agent().get(format!("{ORIGIN}/")).header("User-Agent", AGENT).header("Cookie", "SOCS=CAI");
        let html = read(req.config().timeout_global(Some(TIMEOUT)).build().call()).map(|(_, b)| b).unwrap_or_default();
        let quoted = |key: &str| {
            let rest = html.split(&format!("\"{key}\":\"")).nth(1)?;
            rest.split('"').next().filter(|v| !v.is_empty()).map(str::to_string)
        };
        let found = quoted("INNERTUBE_CLIENT_VERSION");
        let p = Player {
            version: found.clone().unwrap_or_else(|| VERSION.to_string()),
            visitor: quoted("VISITOR_DATA").unwrap_or_default(),
        };
        if found.is_some() {
            *self.player.lock().unwrap_or_else(PoisonError::into_inner) = Some(p.clone());
        }
        p
    }

    /// Falls back to the version built in; false when that is the one in use.
    fn older_player(&self) -> bool {
        let mut p = self.player.lock().unwrap_or_else(PoisonError::into_inner);
        match p.as_mut() {
            Some(p) if p.version != VERSION => {
                p.version = VERSION.to_string();
                true
            }
            _ => false,
        }
    }

    fn post(&self, endpoint: &str, mut body: Value, signed: bool) -> Result<Value> {
        let player = self.player();
        let mut client = json!({ "clientName": "WEB_REMIX", "clientVersion": player.version, "hl": "en" });
        let mut req = agent()
            .post(format!("{ORIGIN}/youtubei/v1/{endpoint}?prettyPrint=false"))
            .config()
            .timeout_global(Some(TIMEOUT))
            .build()
            .header("Content-Type", "application/json")
            .header("User-Agent", AGENT)
            .header("Origin", ORIGIN)
            .header("X-Origin", ORIGIN)
            .header("X-Goog-AuthUser", "0")
            .header("X-YouTube-Client-Name", "67")
            .header("X-YouTube-Client-Version", &player.version)
            .header("Referer", format!("{ORIGIN}/"));
        if !player.visitor.is_empty() {
            req = req.header("X-Goog-Visitor-Id", &player.visitor);
            client["visitorData"] = json!(player.visitor);
        }
        let mut context = json!({ "client": client });
        if signed {
            let (cookies, page) = {
                let k = self.kept();
                (k.cookies.clone(), k.page.clone())
            };
            if sapisid(&cookies).is_empty() {
                return Err(Error::Refused("Not signed in".into()));
            }
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            req = req.header("Cookie", &cookies).header("Authorization", authorization(&cookies, now));
            if !page.is_empty() {
                req = req.header("X-Goog-PageId", &page);
                context["user"] = json!({ "onBehalfOfUser": page });
            }
        }
        body["context"] = context;
        let (status, text) = read(req.send(body.to_string()))?;
        match status {
            401 | 403 if signed => {
                self.sign_out();
                Err(Error::Refused(AGAIN.into()))
            }
            200..=299 => serde_json::from_str(&text).map_err(|_| Error::Parse("not JSON")),
            s => Err(Error::Status(s)),
        }
    }

    /// One of the account's lists: the liked songs, the playlists, or a playlist; `more` continues one.
    pub fn list(&self, list: &str, more: Option<&str>) -> Result<Page> {
        if list == LISTS {
            return self.playlists();
        }
        if let Some(token) = more {
            let v = self.post("browse", json!({ "continuation": token }), true)?;
            return Ok(Page { hits: tracks(&v), more: continuation(&v) });
        }
        let id = if list == LIKED { "VLLM" } else { list.strip_prefix(LIST).unwrap_or(list) };
        let mut v = self.post("browse", json!({ "browseId": id }), true)?;
        // Likes given on YouTube itself are not among YouTube Music's: its liked videos then.
        if id == "VLLM" && tracks(&v).is_empty() && signed_in_page(&v) {
            v = self.post("browse", json!({ "browseId": "VLLL" }), true)?;
        }
        let hits = tracks(&v);
        if hits.is_empty() && !signed_in_page(&v) {
            if self.older_player() {
                return self.list(list, None);
            }
            return Err(Error::Refused("YouTube did not accept the sign-in. Sign out, then sign in again".into()));
        }
        Ok(Page { hits, more: continuation(&v) })
    }

    fn playlists(&self) -> Result<Page> {
        let mut v = self.post("browse", json!({ "browseId": "FEmusic_liked_playlists" }), true)?;
        let mut hits = Vec::new();
        for _ in 0..LIST_PAGES {
            playlists(&v, &mut hits);
            let Some(token) = continuation(&v) else { break };
            v = self.post("browse", json!({ "continuation": token }), true)?;
        }
        Ok(Page { hits, more: None })
    }

    /// The account's channels, the main one among them.
    pub fn channels(&self) -> Result<Vec<Channel>> {
        self.post("account/accounts_list", json!({}), true).map(|v| channels(&v))
    }

    /// Whose account it is, kept for the next run; empty when YouTube does not say.
    pub fn whose(&self) -> Result<String> {
        // The account's menu can fail right after a sign-in; the channels still name it.
        let menu = self.post("account/account_menu", json!({}), true);
        if matches!(&menu, Err(Error::Refused(_))) {
            return menu.map(|_| String::new());
        }
        let mut name = String::new();
        if let Ok(v) = &menu {
            walk(v, &mut |o| {
                name = o.get("accountName").map(text).unwrap_or_default();
                !name.is_empty()
            });
            // Anything that is not YouTube's own reply, a hotel's login page say, is no verdict.
            if name.is_empty() && v.get("responseContext").is_some() && !signed_in_page(v) {
                self.sign_out();
                return Err(Error::Refused(AGAIN.into()));
            }
        }
        if name.is_empty() {
            let all = self.channels().unwrap_or_default();
            let page = self.page();
            let mine = all.iter().find(|c| c.page == page).or(all.first().filter(|_| all.len() == 1));
            name = mine.map(|c| c.name.clone()).unwrap_or_default();
        }
        if !name.is_empty() && self.signed_in() {
            self.kept().name.clone_from(&name);
            self.save();
        }
        Ok(name)
    }

    /// The song that `query` names: the first found whose length is about `seconds`, or the top one if not far off.
    pub fn song(&self, query: &str, seconds: u32) -> Option<String> {
        let v = self.post("search", json!({ "query": query, "params": SONGS }), false).ok()?;
        let found = tracks(&v);
        let near = |h: &&Hit, by: u32| seconds == 0 || h.seconds.abs_diff(seconds) <= by;
        found.iter().find(|h| near(h, 6)).or(found.first().filter(|h| near(h, 30))).map(|h| h.id.clone())
    }
}

/// A video as the song YouTube Music takes it for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Song {
    pub title: String,
    pub artist: String,
    /// Its words, without times.
    pub words: Option<String>,
}

fn ask(endpoint: &str, mut body: Value) -> Result<Value> {
    body["context"] = json!({ "client": { "clientName": "WEB_REMIX", "clientVersion": VERSION, "hl": "en" } });
    let req = agent()
        .post(format!("{ORIGIN}/youtubei/v1/{endpoint}?prettyPrint=false"))
        .config()
        .timeout_global(Some(TIMEOUT))
        .build()
        .header("Content-Type", "application/json")
        .header("User-Agent", AGENT)
        .header("Origin", ORIGIN)
        .header("X-YouTube-Client-Name", "67")
        .header("X-YouTube-Client-Version", VERSION);
    match read(req.send(body.to_string()))? {
        (200..=299, text) => serde_json::from_str(&text).map_err(|_| Error::Parse("not JSON")),
        (s, _) => Err(Error::Status(s)),
    }
}

/// The song a watch page plays and the page that holds its lyrics.
fn playing(next: &Value) -> (Song, Option<String>) {
    let (mut song, mut lyrics) = (Song::default(), None);
    walk(next, &mut |m| {
        if let Some(v) = m.get("playlistPanelVideoRenderer").filter(|_| song.title.is_empty()) {
            song.title = text(&v["title"]);
            song.artist = v["longBylineText"]["runs"][0]["text"].as_str().unwrap_or_default().to_owned();
        }
        let page = m.get("browseEndpoint").and_then(|b| b["browseId"].as_str()).filter(|id| id.starts_with("MPLY"));
        lyrics = page.map(str::to_owned).or(lyrics.take());
        false
    });
    (song, lyrics)
}

/// The words on a lyrics page.
fn words(page: &Value) -> Option<String> {
    let mut out = None;
    walk(page, &mut |m| {
        if let Some(shelf) = m.get("musicDescriptionShelfRenderer") {
            let lines = text(&shelf["description"]).replace('\r', "").replace(['[', ']'], "");
            out = Some(lines.trim().to_owned()).filter(|t| !t.is_empty());
        }
        out.is_some()
    });
    out
}

/// What YouTube Music calls `video` as a song, with its words when it has them. Nobody need be signed in.
pub fn song_of(video: &str) -> Result<Song> {
    let (mut song, lyrics) = playing(&ask("next", json!({ "videoId": video }))?);
    if let Some(page) = lyrics {
        song.words = ask("browse", json!({ "browseId": page })).ok().and_then(|p| words(&p));
    }
    Ok(song)
}

fn cookie<'a>(cookies: &'a str, name: &str) -> &'a str {
    cookies.split(';').find_map(|p| p.trim().strip_prefix(name)?.strip_prefix('=')).unwrap_or("")
}

/// The cookie the account's requests are signed with.
fn sapisid(cookies: &str) -> &str {
    match cookie(cookies, "SAPISID") {
        "" => cookie(cookies, "__Secure-3PAPISID"),
        v => v,
    }
}

/// Signed as the web player signs: the time and the SHA-1 of time, cookie and origin, once for each cookie it has.
fn authorization(cookies: &str, now: u64) -> String {
    let sign = |secret: &str| {
        let hash = Sha1::digest(format!("{now} {secret} {ORIGIN}"));
        let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
        format!("{now}_{hex}")
    };
    let mut out = format!("SAPISIDHASH {}", sign(sapisid(cookies)));
    for (name, scheme) in [("__Secure-1PAPISID", "SAPISID1PHASH"), ("__Secure-3PAPISID", "SAPISID3PHASH")] {
        let secret = cookie(cookies, name);
        if !secret.is_empty() {
            out.push_str(&format!(" {scheme} {}", sign(secret)));
        }
    }
    out
}

/// Calls `visit` on every object inside `v`, in order; one it says yes to is not looked into.
fn walk(v: &Value, visit: &mut dyn FnMut(&Map<String, Value>) -> bool) {
    match v {
        Value::Object(m) => {
            if !visit(m) {
                m.values().for_each(|x| walk(x, visit));
            }
        }
        Value::Array(a) => a.iter().for_each(|x| walk(x, visit)),
        _ => {}
    }
}

fn text(v: &Value) -> String {
    if let Some(s) = v["simpleText"].as_str() {
        return s.to_string();
    }
    v["runs"].as_array().map(|r| r.iter().filter_map(|x| x["text"].as_str()).collect()).unwrap_or_default()
}

/// "3:45" or "1:02:03" in seconds; 0 for anything else.
fn clock(s: &str) -> u32 {
    s.split(':').try_fold(0u32, |all, p| Some(all * 60 + p.trim().parse::<u32>().ok()?)).unwrap_or(0)
}

fn column(item: &Value, n: usize) -> String {
    text(&item["flexColumns"][n]["musicResponsiveListItemFlexColumnRenderer"]["text"])
}

/// The largest picture inside `v`.
fn picture(v: &Value) -> String {
    let mut url = String::new();
    walk(v, &mut |o| {
        let last = o.get("thumbnails").and_then(Value::as_array).and_then(|t| t.last());
        url = last.and_then(|t| t["url"].as_str()).unwrap_or_default().to_string();
        !url.is_empty()
    });
    url
}

/// The tracks of a page that can be played; those that can not have no video.
fn tracks(page: &Value) -> Vec<Hit> {
    let mut out: Vec<Hit> = Vec::new();
    walk(page, &mut |o| {
        let Some(item) = o.get("musicResponsiveListItemRenderer").filter(|i| i.is_object()) else { return false };
        let mut id = item["playlistItemData"]["videoId"].as_str().unwrap_or_default().to_string();
        if id.is_empty() {
            walk(&item["overlay"], &mut |o| {
                id = o.get("watchEndpoint").and_then(|w| w["videoId"].as_str()).unwrap_or_default().to_string();
                !id.is_empty()
            });
        }
        if id.is_empty() || out.iter().any(|h| h.id == id) {
            return true;
        }
        let artist = column(item, 1);
        let fixed = text(&item["fixedColumns"][0]["musicResponsiveListItemFixedColumnRenderer"]["text"]);
        // A search's rows end in the length instead: "Song • Artist • Album • 3:45".
        let seconds = match clock(&fixed) {
            0 => artist.rsplit(' ').next().map_or(0, clock),
            n => n,
        };
        out.push(Hit { id, title: column(item, 0), artist, seconds, art: picture(&item["thumbnail"]) });
        true
    });
    out
}

/// The playlists of a page, as hits to open; not the liked songs, which have their own button, nor the buttons
/// that make new ones.
fn playlists(page: &Value, out: &mut Vec<Hit>) {
    walk(page, &mut |o| {
        for (kind, wide) in [("musicTwoRowItemRenderer", true), ("musicResponsiveListItemRenderer", false)] {
            let Some(item) = o.get(kind).filter(|i| i.is_object()) else { continue };
            let id = item["navigationEndpoint"]["browseEndpoint"]["browseId"].as_str().unwrap_or_default();
            if id.starts_with("VL") && !["VLLM", "VLLL", "VLSE"].contains(&id) {
                let (title, artist, art) = if wide {
                    (text(&item["title"]), text(&item["subtitle"]), picture(&item["thumbnailRenderer"]))
                } else {
                    (column(item, 0), column(item, 1), picture(&item["thumbnail"]))
                };
                out.push(Hit { id: format!("{LIST}{id}"), title, artist, seconds: 0, art });
            }
            return true;
        }
        false
    });
}

fn channels(page: &Value) -> Vec<Channel> {
    let mut out = Vec::new();
    walk(page, &mut |o| {
        let Some(item) = o.get("accountItem").filter(|i| i.is_object()) else { return false };
        // The page is in a token somewhere in the item, or else the first half of "<page>||<user>".
        let (mut page, mut sync) = (String::new(), String::new());
        walk(item, &mut |o| {
            if let Some(id) = o.get("pageIdToken").and_then(|t| t["pageId"].as_str()).filter(|id| !id.is_empty()) {
                page = id.to_string();
                return true;
            }
            if let Some(s) = o.get("datasyncIdToken").and_then(|t| t["datasyncIdToken"].as_str())
                && sync.is_empty()
            {
                sync = s.to_string();
            }
            false
        });
        if page.is_empty()
            && let Some((first, second)) = sync.split_once("||")
            && !second.is_empty()
            && first != second
        {
            page = first.to_string();
        }
        out.push(Channel { name: text(&item["accountName"]), handle: text(&item["channelHandle"]), page });
        true
    });
    out
}

/// Whether YouTube answered as to a signed in user.
fn signed_in_page(page: &Value) -> bool {
    let mut yes = false;
    walk(&page["responseContext"], &mut |o| {
        let found = o.get("key").and_then(Value::as_str) == Some("logged_in");
        if found {
            yes = o.get("value").and_then(Value::as_str) == Some("1");
        }
        found
    });
    yes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_signed_once_for_each_cookie() {
        let cookies = "A=1; SAPISID=abc/def; __Secure-3PAPISID=xyz";
        assert_eq!((sapisid(cookies), sapisid("__Secure-3PAPISID=xyz"), sapisid("SAPISIDX=1")), ("abc/def", "xyz", ""));
        let auth = authorization(cookies, 1_700_000_000);
        let parts: Vec<&str> = auth.split(' ').collect();
        assert_eq!((parts[0], parts[2], parts.len()), ("SAPISIDHASH", "SAPISID3PHASH", 4));
        assert!(parts[1].starts_with("1700000000_") && parts[1].len() == 11 + 40);
        assert_ne!(parts[1], parts[3]);
    }

    #[test]
    fn a_page_gives_its_playable_tracks_once() {
        let row = |id: &str, length: &str| {
            json!({"musicResponsiveListItemRenderer": {
                "playlistItemData": {"videoId": id},
                "thumbnail": {"musicThumbnailRenderer": {"thumbnail": {"thumbnails": [{"url": "small"}, {"url": "large"}]}}},
                "flexColumns": [
                    {"musicResponsiveListItemFlexColumnRenderer": {"text": {"runs": [{"text": "Song"}]}}},
                    {"musicResponsiveListItemFlexColumnRenderer": {"text": {"runs": [{"text": "Artist"}, {"text": " • 2:05"}]}}}],
                "fixedColumns": [{"musicResponsiveListItemFixedColumnRenderer": {"text": {"simpleText": length}}}]}})
        };
        let page = json!({"contents": [row("a", "3:45"), row("a", "3:45"), row("", "1:00"), row("b", "")],
            "responseContext": {"serviceTrackingParams": [{"params": [{"key": "logged_in", "value": "1"}]}]}});
        let found = tracks(&page);
        assert_eq!(found.len(), 2);
        assert_eq!((found[0].id.as_str(), found[0].title.as_str(), found[0].seconds), ("a", "Song", 225));
        assert_eq!((found[0].art.as_str(), found[1].seconds), ("large", 125), "a search's row tells its length last");
        assert!(signed_in_page(&page) && !signed_in_page(&json!({"responseContext": {}})));
    }

    #[test]
    fn playlists_leave_out_the_liked_songs() {
        let list = |id: &str| {
            json!({"musicTwoRowItemRenderer": {"title": {"runs": [{"text": "Mix"}]}, "subtitle": {"simpleText": "9 songs"},
                "navigationEndpoint": {"browseEndpoint": {"browseId": id}}}})
        };
        let mut out = Vec::new();
        playlists(&json!([list("VLLM"), list("VLPL1"), list("FEmusic_new"), list("VLSE")]), &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(
            (out[0].id.as_str(), out[0].title.as_str(), out[0].artist.as_str()),
            ("playlist:VLPL1", "Mix", "9 songs")
        );
    }

    #[test]
    fn a_watch_page_names_its_song_and_its_lyrics() {
        let next = json!({ "tabs": [
            { "content": [{ "playlistPanelVideoRenderer": { "title": { "runs": [{ "text": "Song" }] },
                "longBylineText": { "runs": [{ "text": "Singer" }, { "text": " \u{2022} " }, { "text": "1M views" }] } } }] },
            { "endpoint": { "browseEndpoint": { "browseId": "MPLYt_abc" } } },
            { "endpoint": { "browseEndpoint": { "browseId": "MPTRt_abc" } } }] });
        let (song, lyrics) = playing(&next);
        assert_eq!((song.title.as_str(), song.artist.as_str()), ("Song", "Singer"));
        assert_eq!(lyrics.as_deref(), Some("MPLYt_abc"));
        let page = json!({ "x": { "musicDescriptionShelfRenderer": { "description": { "runs": [{ "text": "One\r\nTwo" }] } } } });
        assert_eq!(words(&page).as_deref(), Some("One\nTwo"));
        assert_eq!(words(&json!({})), None);
    }

    #[test]
    #[ignore = "network"]
    fn names_a_real_video() {
        let song = song_of("TUVcZfQe-Kw").unwrap();
        assert_eq!(song.artist, "Dua Lipa");
        assert!(song.title.starts_with("Levitating"), "{}", song.title);
        assert!(song.words.is_some_and(|w| w.lines().count() > 20));
    }

    #[test]
    fn channels_tell_their_page() {
        let item = |name: &str, extra: Value| json!({"accountItem": {"accountName": {"simpleText": name}, "x": extra}});
        let page = json!([
            item("Main", json!({"datasyncIdToken": {"datasyncIdToken": "111||"}})),
            item("Brand", json!({"datasyncIdToken": {"datasyncIdToken": "222||111"}})),
            item("Other", json!({"deep": {"pageIdToken": {"pageId": "333"}}})),
        ]);
        let pages: Vec<(String, String)> = channels(&page).into_iter().map(|c| (c.name, c.page)).collect();
        let want = [("Main", ""), ("Brand", "222"), ("Other", "333")].map(|(n, p)| (n.to_string(), p.to_string()));
        assert_eq!(pages, want);
    }

    #[test]
    fn the_account_is_kept_between_runs() {
        let file = std::env::temp_dir().join(format!("djsugar-ytmusic-{}", std::process::id())).join("account");
        let a = Account::open(file.clone());
        assert!(!a.signed_in() && !a.sign_in("NID=1"), "cookies without a sign-in are not one");
        assert!(a.sign_in("SAPISID=s; NID=1"));
        a.choose(&Channel { name: "Brand".into(), handle: String::new(), page: "222".into() });
        let again = Account::open(file.clone());
        assert_eq!((again.signed_in(), again.name().as_str(), again.page().as_str()), (true, "Brand", "222"));
        again.sign_out();
        assert!(!Account::open(file.clone()).signed_in());
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }
}

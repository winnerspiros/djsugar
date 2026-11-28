//! Genius: plain words, no key. Its site turns programs away, and the service its phone app reads answers anyone who
//! names the app's version: a search by artist and title, then the song with its words as plain text.

use std::time::Duration;

use serde_json::Value;

use crate::http::{agent, read};
use crate::{Error, Result};

const API: &str = "https://api.genius.com";
/// The header the service wants, and an app version to put in it.
const VERSION: (&str, &str) = ("X-Genius-iOS-Version", "7.7.0");
const TIMEOUT: Duration = Duration::from_secs(12);

fn get(path: &str) -> Result<Value> {
    let call = agent().get(&format!("{API}{path}")).header(VERSION.0, VERSION.1);
    match read(call.config().timeout_global(Some(TIMEOUT)).build().call())? {
        (200..=299, body) => serde_json::from_str(&body).map_err(|_| Error::Parse("not JSON")),
        (s, _) => Err(Error::Status(s)),
    }
}

/// A name in small letters and digits, its words one space apart.
fn plain(name: &str) -> String {
    let kept: String = name.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect();
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The found song that is this one: of the same name, by an artist sharing a word with ours, and with words.
fn pick(found: &Value, artist: &str, title: &str) -> Option<u64> {
    let (artist, title) = (plain(artist), plain(title));
    let ours = |h: &&Value| {
        let song = &h["result"];
        let by = plain(song["artist_names"].as_str().unwrap_or_default());
        plain(song["title"].as_str().unwrap_or_default()) == title
            && song["instrumental"].as_bool() != Some(true)
            && artist.split(' ').filter(|w| w.chars().count() > 1).any(|w| by.split(' ').any(|k| k == w))
    };
    found["response"]["hits"].as_array()?.iter().find(ours)?["result"]["id"].as_u64()
}

/// The words as sung: headings such as `[Chorus]` left out, one empty line between verses.
fn words(text: &str) -> Option<String> {
    let mut kept: Vec<&str> = Vec::new();
    for l in text.lines().map(str::trim).filter(|l| !(l.starts_with('[') && l.ends_with(']'))) {
        if !(l.is_empty() && kept.last().is_none_or(|k| k.is_empty())) {
            kept.push(l);
        }
    }
    let words = kept.join("\n").trim().to_string();
    (words.lines().count() > 2).then_some(words)
}

/// A song's words from Genius; None when it has no such song, or not its words.
pub fn find(artist: &str, title: &str) -> Result<Option<String>> {
    if artist.trim().is_empty() || title.trim().is_empty() {
        return Ok(None);
    }
    let q = crate::soundcloud::encode(&format!("{} {}", artist.trim(), title.trim()));
    let Some(id) = pick(&get(&format!("/search?q={q}"))?, artist, title) else { return Ok(None) };
    let song = get(&format!("/songs/{id}?text_format=plain"))?;
    Ok(song["response"]["song"]["lyrics"]["plain"].as_str().and_then(words))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_song_and_drops_the_headings() {
        let hit = |id: u64, title: &str, by: &str, instrumental: bool| serde_json::json!({"result": {"id": id, "title": title, "artist_names": by, "instrumental": instrumental}});
        let found = serde_json::json!({"response": {"hits": [
            hit(1, "One More Time (Translation)", "Genius Translations", false),
            hit(2, "One More Time", "Someone Else", false),
            hit(3, "One More Time", "Daft Punk (Ft. Romanthony)", true),
            hit(4, "One More Time", "Daft Punk (Ft. Romanthony)", false),
        ]}});
        assert_eq!(pick(&found, "Daft Punk", "one more time"), Some(4));
        assert_eq!(pick(&found, "Daft Punk", "Aerodynamic"), None);
        assert_eq!(words("[Intro]\n One\n Two\n \n [Verse 1]\n Three").unwrap(), "One\nTwo\n\nThree");
        assert_eq!(words("[Instrumental]"), None);
    }

    #[test]
    #[ignore = "network"]
    fn live_lookup() {
        let l = find("Daft Punk", "One More Time").unwrap().unwrap();
        assert!(l.contains("One more time") && !l.contains('['), "{l}");
        let greek = find("ΛΕΞ", "Τίποτα στον κόσμο").unwrap();
        assert!(greek.is_none_or(|g| !g.contains('<')));
    }
}

//! lyrics.ovh: free plain lyrics, no key. The last place asked, for words nobody has timed.

use std::time::Duration;

use crate::http::{agent, read};
use crate::{Error, Result};

const API: &str = "https://api.lyrics.ovh/v1";
const TIMEOUT: Duration = Duration::from_secs(10);

/// A track's words without times; None when the service has none.
pub fn find(artist: &str, title: &str) -> Result<Option<String>> {
    let (a, t) = (artist.trim(), title.trim());
    if a.is_empty() || t.is_empty() {
        return Ok(None);
    }
    let url = format!("{API}/{}/{}", crate::soundcloud::encode(a), crate::soundcloud::encode(t));
    match read(agent().get(&url).config().timeout_global(Some(TIMEOUT)).build().call())? {
        (200..=299, body) => {
            let v: serde_json::Value = serde_json::from_str(&body).map_err(|_| Error::Parse("not JSON"))?;
            Ok(v["lyrics"].as_str().map(plain).filter(|t| !t.is_empty()))
        }
        (404, _) => Ok(None),
        (s, _) => Err(Error::Status(s)),
    }
}

/// The words a line each, without the heading some entries start with or brackets that would read as times.
fn plain(text: &str) -> String {
    let lines = text.lines().map(str::trim).filter(|l| !l.starts_with("Paroles de la chanson"));
    lines.map(|l| l.replace(['[', ']'], "")).collect::<Vec<_>>().join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_and_brackets_go() {
        assert_eq!(plain("Paroles de la chanson X par Y\r\nOne\r\n\r\n[Chorus]\r\nTwo"), "One\n\nChorus\nTwo");
    }

    #[test]
    #[ignore = "network"]
    fn live_lookup() {
        let l = find("Daft Punk", "One More Time").unwrap().unwrap();
        assert!(l.contains("One more time"), "{l}");
    }
}

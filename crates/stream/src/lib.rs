//! Streaming hub: services whose audio may be played on the decks.
//!
//! Every service answers searches with [`Hit`]s and turns a hit into a
//! [`Stream`]: a plain HTTP URL with the headers to fetch it. The app
//! downloads it into its stream cache and decodes it like any file. Calls
//! block; the app runs them on its worker thread.
//!
//! Browse-only services (Spotify, TIDAL, SoundCloud, Mixcloud) live in the
//! app; their tracks play by matching the collection or one of these.

pub mod audius;
pub mod subsonic;
pub mod url;
pub mod youtube;

use std::io::Read;
use std::path::Path;
use std::time::Duration;

/// Which service a hit came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Service {
    YouTube,
    Audius,
    Subsonic,
    Url,
}

impl Service {
    pub const fn name(self) -> &'static str {
        match self {
            Self::YouTube => "YouTube",
            Self::Audius => "Audius",
            Self::Subsonic => "Navidrome",
            Self::Url => "Link",
        }
    }
}

/// One search result.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hit {
    /// The service's own ID, enough to ask for the stream again.
    pub id: String,
    pub title: String,
    pub artist: String,
    pub seconds: Option<u32>,
    pub artwork: Option<String>,
}

/// Where the audio is and how to ask for it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stream {
    pub url: String,
    pub headers: Vec<(String, String)>,
    /// File extension the cache should use, so the decoder probes the right format.
    pub ext: &'static str,
}

#[derive(Debug)]
pub enum Error {
    Http(String),
    Parse(&'static str),
    /// The service refused: a message for the DJ.
    Refused(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(e) => write!(f, "network: {e}"),
            Self::Parse(what) => write!(f, "unexpected reply: {what}"),
            Self::Refused(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .http_status_as_error(false)
        .build()
        .new_agent()
}

pub(crate) fn json(
    resp: std::result::Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> Result<serde_json::Value> {
    let mut resp = resp.map_err(|e| Error::Http(e.to_string()))?;
    let status = resp.status();
    let body = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| Error::Http(e.to_string()))?;
    if !status.is_success() {
        return Err(Error::Http(format!("HTTP {}", status.as_u16())));
    }
    serde_json::from_str(&body).map_err(|_| Error::Parse("not JSON"))
}

pub(crate) fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Downloads a stream to `to` (written beside it, then renamed), calling
/// `progress(done, total)` as it goes. Returns the bytes written.
pub fn download(
    stream: &Stream,
    to: &Path,
    mut progress: impl FnMut(u64, Option<u64>),
) -> Result<u64> {
    let agent = agent();
    let mut req = agent.get(&stream.url);
    for (k, v) in &stream.headers {
        req = req.header(k, v);
    }
    let mut resp = req.call().map_err(|e| Error::Http(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(Error::Http(format!("HTTP {}", resp.status().as_u16())));
    }
    let total = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok()?.parse().ok());
    let tmp = to.with_extension("part");
    let mut file = std::fs::File::create(&tmp).map_err(|e| Error::Http(e.to_string()))?;
    let mut body = resp.body_mut().with_config().limit(u64::MAX).reader();
    let mut buf = vec![0u8; 1 << 16];
    let mut done = 0u64;
    loop {
        let n = body
            .read(&mut buf)
            .map_err(|e| Error::Http(e.to_string()))?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n]).map_err(|e| Error::Http(e.to_string()))?;
        done += n as u64;
        progress(done, total);
    }
    drop(file);
    std::fs::rename(&tmp, to).map_err(|e| Error::Http(e.to_string()))?;
    Ok(done)
}

#[cfg(test)]
mod tests {
    #[test]
    fn encodes_queries() {
        assert_eq!(
            super::enc("Daft Punk & co/é"),
            "Daft%20Punk%20%26%20co%2F%C3%A9"
        );
    }
}

//! Streaming services: search them and fetch their audio for the decks.
//! Calls block, so the app runs them on worker threads. Pure Rust down to TLS.

pub mod archive;
pub mod audius;
pub mod botguard;
pub mod cipher;
pub mod hls;
pub mod http;
pub mod kugou;
pub mod lrclib;
pub mod lyricsovh;
pub mod netease;
pub mod qq;
pub mod said;
pub mod soundcloud;
pub mod sponsorblock;
pub mod youtube;
pub mod ytmusic;

/// One search result.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hit {
    /// The service's own ID, enough to fetch the audio later.
    pub id: String,
    pub title: String,
    pub artist: String,
    pub seconds: u32,
    /// The artwork's address, when the service has one.
    pub art: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// No connection, timeout, TLS failure.
    Network(String),
    /// The service answered with an HTTP error.
    Status(u16),
    /// The reply was not what the service normally sends.
    Parse(&'static str),
    /// The service turned the request down; the message is for the DJ.
    Refused(String),
    /// The DJ called the download off.
    Stopped,
    /// The server holding the file doesn't answer from this connection.
    Unreachable,
}

/// Told how much of a download has arrived, of how much; answers false to call it off.
pub type Progress<'a> = &'a (dyn Fn(u64, u64) -> bool + Sync);

static NOTES: std::sync::OnceLock<fn(&str)> = std::sync::OnceLock::new();

/// Where the notes of what the services answered go: the app's log, when it keeps one.
pub fn notes_to(sink: fn(&str)) {
    let _ = NOTES.set(sink);
}

pub(crate) fn note(text: &str) {
    if let Some(sink) = NOTES.get() {
        sink(text);
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(e) => write!(f, "network: {e}"),
            Self::Status(s) => write!(f, "HTTP {s}"),
            Self::Parse(what) => write!(f, "unexpected reply: {what}"),
            Self::Refused(why) => f.write_str(why),
            Self::Stopped => f.write_str("stopped"),
            Self::Unreachable => f.write_str("the server can't be reached on this connection"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

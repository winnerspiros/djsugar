//! Streaming services: search them and fetch their audio for the decks.
//! Calls block, so the app runs them on worker threads. Pure Rust down to TLS.

pub mod archive;
pub mod audius;
pub mod botguard;
pub mod cipher;
pub mod http;
pub mod lrclib;
pub mod soundcloud;
pub mod youtube;

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
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(e) => write!(f, "network: {e}"),
            Self::Status(s) => write!(f, "HTTP {s}"),
            Self::Parse(what) => write!(f, "unexpected reply: {what}"),
            Self::Refused(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

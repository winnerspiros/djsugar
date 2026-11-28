//! A shared HTTP agent and a parallel ranged downloader.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use crate::{Error, Result};

/// One agent for the whole app: its connection pool is reused by every call.
pub fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        let tls = Arc::new(rustls_rustcrypto::provider());
        let config = ureq::Agent::config_builder()
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .provider(ureq::tls::TlsProvider::Rustls)
                    .unversioned_rustls_crypto_provider(tls)
                    .root_certs(ureq::tls::RootCerts::new_with_certs(&roots()))
                    .build(),
            )
            .timeout_connect(Some(Duration::from_secs(10)))
            .http_status_as_error(false)
            .build();
        ureq::Agent::new_with_config(config)
    })
}

/// Mozilla's root certificates, built in so no platform store is needed.
fn roots() -> Vec<ureq::tls::Certificate<'static>> {
    webpki_root_certs::TLS_SERVER_ROOT_CERTS.iter().map(|c| ureq::tls::Certificate::from_der(c.as_ref())).collect()
}

pub(crate) fn net(e: impl std::fmt::Display) -> Error {
    Error::Network(e.to_string())
}

/// Status and body of a reply, the body as JSON when it parses.
pub(crate) fn read(resp: std::result::Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<(u16, String)> {
    let mut resp = resp.map_err(net)?;
    let status = resp.status().as_u16();
    let body = resp.body_mut().with_config().limit(32 << 20).read_to_string().map_err(net)?;
    Ok((status, body))
}

/// Size of a file at `url`, from a one byte ranged request.
pub fn length(url: &str, headers: &[(&str, &str)]) -> Result<Option<u64>> {
    let mut req = agent().get(url).header("Range", "bytes=0-0");
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let resp = req.config().timeout_global(Some(ANSWER)).build().call().map_err(net)?;
    let status = resp.status().as_u16();
    if status >= 400 {
        return Err(Error::Status(status));
    }
    let total = resp.headers().get("content-range").and_then(|v| v.to_str().ok()?.rsplit('/').next()?.parse().ok());
    Ok(total.or_else(|| resp.headers().get("content-length").and_then(|v| v.to_str().ok()?.parse().ok())))
}

/// How long a server may take to answer a request for a piece, and to send the piece. A piece that runs out of
/// time is asked for again from where it stopped, so a slow line still gets there and a dead one gives up.
const ANSWER: Duration = Duration::from_secs(15);
const SENDING: Duration = Duration::from_secs(40);
/// How long connecting to the server holding a file may take: where another holds it too, and where none does.
const REACHING: Duration = Duration::from_secs(2);
const REACHING_LAST: Duration = Duration::from_secs(6);

/// The server a link names, for the log: never the link itself, which can carry a signature.
fn host(url: &str) -> &str {
    url.split("://").nth(1).unwrap_or(url).split(['/', '?', ':']).next().unwrap_or_default()
}

/// Servers that could not be connected to lately, with when: asked last by the downloads that follow.
static UNREACHED: std::sync::Mutex<Vec<(String, std::time::Instant)>> = std::sync::Mutex::new(Vec::new());
const UNREACHED_FOR: Duration = Duration::from_secs(10 * 60);

/// The machine a link names: the part of its host after the front that picked it.
fn machine(url: &str) -> &str {
    let host = host(url);
    host.split_once("---").map_or(host, |(_, m)| m)
}

fn unreached(url: &str) -> bool {
    let mut seen = UNREACHED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    seen.retain(|(_, at)| at.elapsed() < UNREACHED_FOR);
    seen.iter().any(|(m, _)| m == machine(url))
}

fn note_unreached(url: &str) {
    UNREACHED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push((machine(url).to_owned(), std::time::Instant::now()));
}

/// The most one request asks for; streaming hosts cut longer replies short.
pub const PIECE: u64 = 1 << 20;
/// And the least worth a connection of its own.
const SMALLEST: u64 = 1 << 18;

/// Downloads `url` to `to` in [`PIECE`] ranges on `workers` threads, each resumed
/// up to four times; `size` skips asking the length. A short file is an error.
pub fn download(
    url: &str,
    headers: &[(&str, &str)],
    to: &Path,
    size: Option<u64>,
    workers: u64,
    progress: crate::Progress<'_>,
) -> Result<u64> {
    download_any(&[url], headers, to, size, workers, progress)
}

/// [`download`] of a file several servers hold, `urls` its link on each: one that can't be connected to is
/// left for the next by every thread at once, in the middle of the file as well.
pub fn download_any(
    urls: &[&str],
    headers: &[(&str, &str)],
    to: &Path,
    size: Option<u64>,
    workers: u64,
    progress: crate::Progress<'_>,
) -> Result<u64> {
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed};
    if urls.is_empty() {
        return Err(Error::Parse("no link"));
    }
    // Servers that could not be reached a moment ago are left for last.
    let mut ordered: Vec<&str> = urls.iter().copied().filter(|u| !unreached(u)).collect();
    ordered.extend(urls.iter().copied().filter(|u| unreached(u)));
    let urls = &ordered[..];
    let url = urls[0];
    let server = AtomicUsize::new(0);
    let part = to.with_extension(format!("{}.part", to.extension().and_then(|e| e.to_str()).unwrap_or("")));
    let total = match size {
        Some(n) => n,
        None => length(url, headers)?.ok_or(Error::Parse("the server gave no length"))?,
    };
    if total == 0 {
        return Err(Error::Parse("empty stream"));
    }
    let file = std::fs::File::create(&part).map_err(net)?;
    file.set_len(total).map_err(net)?;
    let done = AtomicU64::new(0);
    let next = AtomicU64::new(0);
    // A host that holds each connection to a set speed is faster over more of them: a small file is cut
    // finer, to keep every thread busy.
    let piece = (total / workers.max(1)).clamp(SMALLEST, PIECE);
    let pieces = total.div_ceil(piece);
    let fetch = |from: u64, end: u64| -> Result<()> {
        let mut at = from;
        let mut tries = 0;
        loop {
            // Asked between tries too: a server that can't be reached sends nothing to be stopped over.
            if !progress(done.load(Relaxed), total) {
                return Err(Error::Stopped);
            }
            let asked = server.load(Relaxed);
            let url = urls[asked.min(urls.len() - 1)];
            let reaching = if asked + 1 < urls.len() { REACHING } else { REACHING_LAST };
            let mut req = agent().get(url).header("Range", &format!("bytes={at}-{end}"));
            for (k, v) in headers {
                req = req.header(*k, *v);
            }
            let before = at;
            let run = |at: &mut u64| -> Result<()> {
                let timed = req
                    .config()
                    .timeout_connect(Some(reaching))
                    .timeout_recv_response(Some(ANSWER))
                    .timeout_recv_body(Some(SENDING))
                    .build();
                // A server that can't be connected to is not asked again: another may hold the file. One that
                // goes quiet midway is.
                let mut resp = timed.call().map_err(|e| match e {
                    ureq::Error::Timeout(ureq::Timeout::Connect | ureq::Timeout::Resolve)
                    | ureq::Error::HostNotFound
                    | ureq::Error::ConnectionFailed => Error::Unreachable,
                    e => net(e),
                })?;
                let status = resp.status().as_u16();
                if status >= 400 {
                    return Err(Error::Status(status));
                }
                // A server ignoring the range sends the whole file from byte 0.
                if status == 200 && *at > 0 {
                    return Err(Error::Parse("the server ignored the range"));
                }
                let mut body = resp.body_mut().with_config().limit(u64::MAX).reader();
                // A clone would share one position with the other threads.
                let mut out = std::fs::OpenOptions::new().write(true).open(&part).map_err(net)?;
                out.seek(SeekFrom::Start(*at)).map_err(net)?;
                let mut buf = vec![0u8; 1 << 16];
                while *at <= end {
                    let want = buf.len().min((end + 1 - *at) as usize);
                    let n = body.read(&mut buf[..want]).map_err(net)?;
                    if n == 0 {
                        break;
                    }
                    out.write_all(&buf[..n]).map_err(net)?;
                    *at += n as u64;
                    if !progress(done.fetch_add(n as u64, Relaxed) + n as u64, total) {
                        return Err(Error::Stopped);
                    }
                }
                Ok(())
            };
            match run(&mut at) {
                _ if at > end => return Ok(()),
                Err(Error::Status(s)) if s < 500 => return Err(Error::Status(s)),
                Err(Error::Stopped) => return Err(Error::Stopped),
                // Said once, by the thread that moves all of them on.
                Err(Error::Unreachable) if asked + 1 < urls.len() => {
                    if server.compare_exchange(asked, asked + 1, Relaxed, Relaxed).is_ok() {
                        crate::note(&format!("{} can't be reached", host(url)));
                        note_unreached(url);
                    }
                }
                // The last of them is given up on at once only where nothing has come: a line can drop midway.
                Err(Error::Unreachable) if done.load(Relaxed) == 0 || tries >= 4 => {
                    crate::note(&format!("{} can't be reached", host(url)));
                    return Err(Error::Unreachable);
                }
                Err(Error::Unreachable) => {
                    tries += 1;
                    std::thread::sleep(Duration::from_millis(500));
                }
                // A piece that moved on is not a failed try.
                _ if at > before => tries = 0,
                r if tries >= 4 => return r.and(Err(Error::Parse("download ended early"))),
                r => {
                    if let Err(e) = r {
                        crate::note(&format!("piece at {at} again: {e}"));
                    }
                    tries += 1;
                }
            }
        }
    };
    let result = std::thread::scope(|s| {
        let jobs: Vec<_> = (0..workers.clamp(1, pieces))
            .map(|_| {
                s.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, Relaxed);
                        if i >= pieces {
                            return Ok(());
                        }
                        fetch(i * piece, ((i + 1) * piece).min(total) - 1)?;
                    }
                })
            })
            .collect();
        jobs.into_iter().try_for_each(|j| j.join().unwrap_or(Err(Error::Parse("download thread"))))
    });
    let result = result
        .and_then(|()| if done.load(Relaxed) < total { Err(Error::Parse("download ended early")) } else { Ok(()) });
    drop(file);
    match result {
        Ok(()) => {
            std::fs::rename(&part, to).map_err(net)?;
            Ok(done.into_inner())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            Err(e)
        }
    }
}

/// Saves the artwork at `url` beside the audio file `track`, named after it, so it is found as its cover.
pub fn save_art(url: &str, track: &std::path::Path) -> Result<()> {
    let to = track.with_extension("jpg");
    if url.is_empty() || to.is_file() {
        return Ok(());
    }
    std::fs::write(&to, picture(url)?).map_err(net)
}

/// The picture at `url`, as the file it is.
pub fn picture(url: &str) -> Result<Vec<u8>> {
    let mut resp = agent()
        .get(url)
        .config()
        .timeout_global(Some(std::time::Duration::from_secs(15)))
        .build()
        .call()
        .map_err(net)?;
    if resp.status().as_u16() != 200 {
        return Err(Error::Status(resp.status().as_u16()));
    }
    resp.body_mut().with_config().limit(8 << 20).read_to_vec().map_err(net)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};

    /// Serves `data` by range, but never more than `cut` bytes per reply.
    fn serve(data: Vec<u8>, cut: usize) -> String {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        std::thread::spawn(move || {
            for c in l.incoming().flatten() {
                let data = data.clone();
                std::thread::spawn(move || {
                    let mut r = BufReader::new(c.try_clone().unwrap());
                    let mut range = (0, data.len() - 1);
                    let mut line = String::new();
                    while r.read_line(&mut line).is_ok_and(|n| n > 2) {
                        if let Some(v) = line.to_lowercase().strip_prefix("range: bytes=") {
                            let (a, b) = v.trim().split_once('-').unwrap();
                            range.0 = a.parse().unwrap();
                            range.1 = b.parse().unwrap_or(data.len() - 1).min(data.len() - 1);
                        }
                        line.clear();
                    }
                    let (a, b) = range;
                    let head = format!(
                        "HTTP/1.1 206 Partial\r\nContent-Range: bytes {a}-{b}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        data.len(),
                        b + 1 - a
                    );
                    let mut c = c;
                    let _ = c.write_all(head.as_bytes());
                    let _ = c.write_all(&data[a..=b.min(a + cut - 1)]);
                });
            }
        });
        format!("http://{addr}/a")
    }

    #[test]
    fn cut_replies_still_give_the_whole_file() {
        let data: Vec<u8> = (0..3_500_000u32).map(|i| (i * 7 % 251) as u8).collect();
        let url = serve(data.clone(), 300_000);
        let dir = std::env::temp_dir().join(format!("dl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let to = dir.join("a.bin");
        for size in [Some(data.len() as u64), None] {
            let _ = std::fs::remove_file(&to);
            let n = download(&url, &[], &to, size, 4, &|_, _| true);
            assert!(n.is_ok(), "{n:?}");
            assert_eq!(std::fs::read(&to).unwrap(), data);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The SHA-256 of a file, as lowercase hex.
pub fn sha256(path: &std::path::Path) -> std::io::Result<String> {
    use sha2::Digest;
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hash = sha2::Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod digest {
    #[test]
    fn hashes_a_file() {
        let p = std::env::temp_dir().join(format!("sugar-sha-{}", std::process::id()));
        std::fs::write(&p, b"abc").unwrap();
        let got = super::sha256(&p).unwrap();
        let _ = std::fs::remove_file(&p);
        assert_eq!(got, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}

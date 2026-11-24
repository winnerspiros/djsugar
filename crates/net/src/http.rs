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
    let resp = req.call().map_err(net)?;
    let status = resp.status().as_u16();
    if status >= 400 {
        return Err(Error::Status(status));
    }
    let total = resp.headers().get("content-range").and_then(|v| v.to_str().ok()?.rsplit('/').next()?.parse().ok());
    Ok(total.or_else(|| resp.headers().get("content-length").and_then(|v| v.to_str().ok()?.parse().ok())))
}

/// The most one request asks for; streaming hosts cut longer replies short.
pub const PIECE: u64 = 1 << 20;

/// Downloads `url` to `to` in [`PIECE`] ranges on `workers` threads, each resumed
/// up to four times; `size` skips asking the length. A short file is an error.
pub fn download(
    url: &str,
    headers: &[(&str, &str)],
    to: &Path,
    size: Option<u64>,
    workers: u64,
    progress: &(dyn Fn(u64, u64) + Sync),
) -> Result<u64> {
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
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
    let pieces = total.div_ceil(PIECE);
    let fetch = |from: u64, end: u64| -> Result<()> {
        let mut at = from;
        let mut tries = 0;
        loop {
            let mut req = agent().get(url).header("Range", &format!("bytes={at}-{end}"));
            for (k, v) in headers {
                req = req.header(*k, *v);
            }
            let run = |at: &mut u64| -> Result<()> {
                let mut resp = req.call().map_err(net)?;
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
                    progress(done.fetch_add(n as u64, Relaxed) + n as u64, total);
                }
                Ok(())
            };
            match run(&mut at) {
                _ if at > end => return Ok(()),
                Err(Error::Status(s)) if s < 500 => return Err(Error::Status(s)),
                r if tries >= 4 => return r.and(Err(Error::Parse("download ended early"))),
                _ => tries += 1,
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
                        fetch(i * PIECE, ((i + 1) * PIECE).min(total) - 1)?;
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
            let n = download(&url, &[], &to, size, 4, &|_, _| {});
            assert!(n.is_ok(), "{n:?}");
            assert_eq!(std::fs::read(&to).unwrap(), data);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

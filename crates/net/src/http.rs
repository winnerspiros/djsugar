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

/// Downloads `url` to `to` through a `.part` file, renamed when complete.
///
/// Files of `chunk_from` bytes or more are fetched as `chunks` ranged
/// requests at once, each resumed up to four times where it stopped; this
/// is what makes long mixes arrive in seconds. `progress(done, total)` is
/// called from the download threads.
pub fn download(
    url: &str,
    headers: &[(&str, &str)],
    to: &Path,
    chunk_from: u64,
    chunks: u64,
    progress: &(dyn Fn(u64, u64) + Sync),
) -> Result<u64> {
    let part = to.with_extension(format!("{}.part", to.extension().and_then(|e| e.to_str()).unwrap_or("")));
    let total = length(url, headers)?;
    let file = std::fs::File::create(&part).map_err(net)?;
    let done = std::sync::atomic::AtomicU64::new(0);
    let total_n = total.unwrap_or(0);
    let fetch = |from: u64, to_byte: Option<u64>| -> Result<()> {
        let mut at = from;
        let mut tries = 0;
        loop {
            let range = match to_byte {
                Some(end) => format!("bytes={at}-{end}"),
                None => format!("bytes={at}-"),
            };
            let mut req = agent().get(url).header("Range", &range);
            for (k, v) in headers {
                req = req.header(*k, *v);
            }
            let run = || -> Result<()> {
                let mut resp = req.call().map_err(net)?;
                let status = resp.status().as_u16();
                if status >= 400 {
                    return Err(Error::Status(status));
                }
                let mut body = resp.body_mut().with_config().limit(u64::MAX).reader();
                let mut out = file.try_clone().map_err(net)?;
                out.seek(SeekFrom::Start(at)).map_err(net)?;
                let mut buf = vec![0u8; 1 << 16];
                loop {
                    let n = body.read(&mut buf).map_err(net)?;
                    if n == 0 {
                        return Ok(());
                    }
                    out.write_all(&buf[..n]).map_err(net)?;
                    at += n as u64;
                    let d = done.fetch_add(n as u64, std::sync::atomic::Ordering::Relaxed) + n as u64;
                    progress(d, total_n);
                }
            };
            match run() {
                Ok(()) if to_byte.is_none_or(|end| at > end) => return Ok(()),
                Err(Error::Status(s)) if s < 500 => return Err(Error::Status(s)),
                r if tries >= 4 => return r.and(Err(Error::Parse("download ended early"))),
                _ => tries += 1,
            }
        }
    };
    let result = match total {
        Some(n) if n >= chunk_from && chunks > 1 => {
            file.set_len(n).map_err(net)?;
            let size = n.div_ceil(chunks);
            std::thread::scope(|s| {
                let jobs: Vec<_> = (0..chunks)
                    .map(|i| (i * size, ((i + 1) * size).min(n) - 1))
                    .filter(|(a, b)| a <= b)
                    .map(|(a, b)| s.spawn(move || fetch(a, Some(b))))
                    .collect();
                jobs.into_iter().try_for_each(|j| j.join().unwrap_or(Err(Error::Parse("download thread"))))
            })
        }
        _ => fetch(0, None),
    };
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

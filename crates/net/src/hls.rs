//! Segmented streams: the sound of a playlist of short pieces, joined into one file.

use crate::http::{agent, net};
use crate::{Error, Result};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::time::Duration;

const ANSWER: Duration = Duration::from_secs(20);
const WORKERS: usize = 6;
const TRIES: usize = 4;

fn get(url: &str, headers: &[(&str, &str)]) -> Result<Vec<u8>> {
    let mut req = agent().get(url);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let mut resp = req.config().timeout_global(Some(ANSWER)).build().call().map_err(net)?;
    let status = resp.status().as_u16();
    if status >= 400 {
        return Err(Error::Status(status));
    }
    resp.body_mut().with_config().limit(64 << 20).read_to_vec().map_err(net)
}

fn attribute<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let mut from = 0;
    while let Some(at) = line[from..].find(key) {
        let at = from + at;
        let rest = &line[at + key.len()..];
        if (at == 0 || matches!(line.as_bytes()[at - 1], b',' | b':')) && rest.starts_with('=') {
            let rest = &rest[1..];
            return Some(match rest.strip_prefix('"') {
                Some(quoted) => quoted.split('"').next().unwrap_or_default(),
                None => rest.split(',').next().unwrap_or_default(),
            });
        }
        from = at + key.len();
    }
    None
}

fn joined(base: &str, link: &str) -> String {
    if link.contains("://") {
        return link.to_string();
    }
    let dir = base.split('?').next().unwrap_or(base);
    format!("{}/{link}", dir.rsplit_once('/').map_or(dir, |d| d.0))
}

/// The playlist to take the sound from: a sound-only one when the list has any, else the smallest stream
/// whose sound is plain AAC, else the smallest.
pub fn sound(master: &str, base: &str) -> Option<String> {
    let only: Vec<&str> =
        master.lines().filter(|l| l.starts_with("#EXT-X-MEDIA:") && l.contains("TYPE=AUDIO")).collect();
    let first = only.iter().find(|l| attribute(l, "DEFAULT") == Some("YES")).or(only.first());
    if let Some(link) = first.and_then(|l| attribute(l, "URI")) {
        return Some(joined(base, link));
    }
    let mut streams: Vec<(bool, u64, &str)> = Vec::new();
    let mut lines = master.lines();
    while let Some(line) = lines.next() {
        if line.starts_with("#EXT-X-STREAM-INF:") {
            let link = lines.by_ref().find(|l| !l.starts_with('#') && !l.trim().is_empty())?;
            let plain = attribute(line, "CODECS").is_some_and(|c| c.contains("mp4a.40.2"));
            streams.push((!plain, attribute(line, "BANDWIDTH").and_then(|b| b.parse().ok()).unwrap_or(0), link.trim()));
        }
    }
    streams.sort();
    streams.first().map(|s| joined(base, s.2))
}

/// The start piece a playlist names, when it has one, and its pieces in order.
pub fn pieces(list: &str, base: &str) -> (Option<String>, Vec<String>) {
    let start =
        list.lines().find(|l| l.starts_with("#EXT-X-MAP:")).and_then(|l| attribute(l, "URI")).map(|l| joined(base, l));
    let all = list.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(|l| joined(base, l));
    (start, all.collect())
}

/// Takes the AAC sound out of transport stream packets, as it lies in them: frames with their own headers.
#[derive(Debug, Default)]
pub struct Sound {
    table: Option<u16>,
    sound: Option<u16>,
}

impl Sound {
    pub fn take(&mut self, packets: &[u8], out: &mut Vec<u8>) {
        for p in packets.as_chunks::<188>().0.iter().filter(|p| p[0] == 0x47) {
            let id = u16::from(p[1] & 0x1f) << 8 | u16::from(p[2]);
            let starts = p[1] & 0x40 != 0;
            let mut at = 4;
            if p[3] & 0x20 != 0 {
                at += 1 + usize::from(p[4]);
            }
            if p[3] & 0x10 == 0 || at >= 188 {
                continue;
            }
            let body = &p[at..];
            if id == 0 || Some(id) == self.table {
                // A table starts after a one byte skip count.
                let Some(t) = starts.then(|| body.get(1 + usize::from(body[0])..)).flatten() else { continue };
                if t.len() < 12 {
                    continue;
                }
                let end = (3 + (usize::from(t[1] & 0x0f) << 8 | usize::from(t[2]))).saturating_sub(4).min(t.len());
                if id == 0 {
                    let programs = t.get(8..end).unwrap_or_default().as_chunks::<4>().0.iter();
                    self.table = programs
                        .filter(|e| e[0] != 0 || e[1] != 0)
                        .map(|e| u16::from(e[2] & 0x1f) << 8 | u16::from(e[3]))
                        .next();
                } else {
                    let mut i = 12 + (usize::from(t[10] & 0x0f) << 8 | usize::from(t[11]));
                    while i + 5 <= end {
                        if t[i] == 0x0f {
                            self.sound = Some(u16::from(t[i + 1] & 0x1f) << 8 | u16::from(t[i + 2]));
                            break;
                        }
                        i += 5 + (usize::from(t[i + 3] & 0x0f) << 8 | usize::from(t[i + 4]));
                    }
                }
            } else if Some(id) == self.sound {
                if starts {
                    if body.len() > 9 && body[..3] == [0, 0, 1] {
                        out.extend_from_slice(body.get(9 + usize::from(body[8])..).unwrap_or_default());
                    }
                } else {
                    out.extend_from_slice(body);
                }
            }
        }
    }
}

/// Downloads the sound of the stream at `manifest` to `stem` with the ending its kind takes; returns the file.
pub fn fetch(manifest: &str, headers: &[(&str, &str)], stem: &Path, progress: crate::Progress<'_>) -> Result<PathBuf> {
    let text = |url: &str| get(url, headers).map(|b| String::from_utf8_lossy(&b).into_owned());
    let first = text(manifest)?;
    let (list, base) = if first.contains("#EXTINF") {
        (first, manifest.to_string())
    } else {
        let link = sound(&first, manifest).ok_or(Error::Parse("no stream in the playlist"))?;
        (text(&link)?, link)
    };
    let (start, links) = pieces(&list, &base);
    if links.is_empty() {
        return Err(Error::Parse("an empty playlist"));
    }
    let start = start.map(|link| get(&link, headers)).transpose()?;
    let got: Vec<Mutex<Vec<u8>>> = links.iter().map(|_| Mutex::default()).collect();
    let (next, done, bytes) = (AtomicUsize::new(0), AtomicUsize::new(0), AtomicU64::new(0));
    let failed: Mutex<Option<Error>> = Mutex::default();
    std::thread::scope(|s| {
        for _ in 0..WORKERS.min(links.len()) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Relaxed);
                    if i >= links.len() || failed.lock().is_ok_and(|f| f.is_some()) {
                        return;
                    }
                    let mut tries = 0;
                    let piece = loop {
                        // The whole size is a guess from the pieces so far.
                        let n = done.load(Relaxed).max(1) as u64;
                        let so_far = bytes.load(Relaxed);
                        if !progress(so_far, (so_far / n * links.len() as u64).max(so_far + 1)) {
                            break Err(Error::Stopped);
                        }
                        match get(&links[i], headers) {
                            Err(Error::Network(_)) if tries < TRIES => tries += 1,
                            other => break other,
                        }
                    };
                    match piece {
                        Ok(b) => {
                            bytes.fetch_add(b.len() as u64, Relaxed);
                            done.fetch_add(1, Relaxed);
                            if let Ok(mut slot) = got[i].lock() {
                                *slot = b;
                            }
                        }
                        Err(e) => {
                            if let Ok(mut f) = failed.lock() {
                                f.get_or_insert(e);
                            }
                            return;
                        }
                    }
                }
            });
        }
    });
    if let Some(e) = failed.into_inner().ok().flatten() {
        return Err(e);
    }
    let got: Vec<Vec<u8>> = got.into_iter().map(|m| m.into_inner().unwrap_or_default()).collect();
    let packets = start.is_none() && got[0].first() == Some(&0x47);
    let mut out = start.unwrap_or_default();
    let mut sound = Sound::default();
    for piece in &got {
        if packets {
            sound.take(piece, &mut out);
        } else {
            out.extend_from_slice(piece);
        }
    }
    if out.is_empty() {
        return Err(Error::Parse("no sound in the stream"));
    }
    let to = stem.with_extension(if packets { "aac" } else { "m4a" });
    let part = to.with_extension("part");
    std::fs::write(&part, &out).and_then(|()| std::fs::rename(&part, &to)).map_err(net)?;
    Ok(to)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_sound_of_a_list() {
        let both = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=75506,CODECS=\"mp4a.40.5,avc1.4D400C\"\nhttps://a/low\n\
                    #EXT-X-STREAM-INF:BANDWIDTH=572569,CODECS=\"mp4a.40.2,avc1.640028\"\nhttps://a/high\n\
                    #EXT-X-STREAM-INF:BANDWIDTH=195534,CODECS=\"mp4a.40.2,avc1.4D401E\"\nmid/list.m3u8\n";
        assert_eq!(sound(both, "https://a/b/master.m3u8?x=1/2").as_deref(), Some("https://a/b/mid/list.m3u8"));
        let only = format!(
            "#EXT-X-MEDIA:URI=\"https://a/233\",TYPE=AUDIO,GROUP-ID=\"233\",NAME=\"Default\",DEFAULT=YES\n{both}"
        );
        assert_eq!(sound(&only, "https://a/m").as_deref(), Some("https://a/233"));
        let (start, all) =
            pieces("#EXTM3U\n#EXT-X-MAP:URI=\"init\"\n#EXTINF:5.0,\ns1\n#EXTINF:5.0,\nhttps://b/s2\n", "https://a/l");
        assert_eq!(start.as_deref(), Some("https://a/init"));
        assert_eq!(all, ["https://a/s1", "https://b/s2"]);
    }

    fn packet(id: u16, starts: bool, body: &[u8]) -> Vec<u8> {
        let fill = 184 - body.len();
        let mut p = vec![0x47, u8::from(starts) << 6 | (id >> 8) as u8, id as u8, if fill > 0 { 0x30 } else { 0x10 }];
        if fill > 0 {
            p.push((fill - 1) as u8);
            p.resize(4 + fill, 0xff);
        }
        p.extend_from_slice(body);
        p
    }

    #[test]
    fn takes_the_sound_out_of_packets() {
        let programs = [0, 0, 0xb0, 13, 0, 1, 0xc1, 0, 0, 0, 1, 0xf0, 0x00, 0, 0, 0, 0];
        let streams = [
            0, 2, 0xb0, 23, 0, 1, 0xc1, 0, 0, 0xe1, 0, 0xf0, 0, 0x1b, 0xe1, 0, 0xf0, 0, 0x0f, 0xe1, 1, 0xf0, 0, 0, 0,
            0, 0,
        ];
        let mut ts = packet(0, true, &programs);
        ts.extend(packet(0x1000, true, &streams));
        ts.extend(packet(0x101, true, &[0, 0, 1, 0xc0, 0, 0, 0x80, 0x80, 5, 1, 2, 3, 4, 5, 0xff, 0xf1, 7]));
        ts.extend(packet(0x100, false, &[9; 184]));
        ts.extend(packet(0x101, false, &[8, 8]));
        let mut out = Vec::new();
        Sound::default().take(&ts, &mut out);
        assert_eq!(out, [0xff, 0xf1, 7, 8, 8]);
    }

    /// `DJSUGAR_HLS=<a playlist link> cargo test -p sugar-net -- reads_a_real_stream --ignored --nocapture`
    #[test]
    #[ignore = "network"]
    fn reads_a_real_stream() {
        let Ok(link) = std::env::var("DJSUGAR_HLS") else { return };
        let dir = std::env::temp_dir().join("djsugar-hls");
        std::fs::create_dir_all(&dir).unwrap();
        let to = fetch(link.trim(), &[], &dir.join("stream"), &|_, _| true).unwrap();
        println!("{} {} bytes", to.display(), std::fs::metadata(&to).unwrap().len());
    }
}

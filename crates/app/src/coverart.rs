//! Looking for a track's album art, for the cover dialog of the track info
//! panel: the picture in its tags, an image in its folder, and covers found
//! online through public APIs (MusicBrainz with the Cover Art Archive,
//! iTunes Search, and Discogs when the DJ gave a Discogs token). Each source
//! runs on its own thread and reports covers as it finds them. Analyze
//! Track also takes the first online cover on its own, see [`first_online`].

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use djsugar_engine::{cover, tags};
use djsugar_plugins::{encode, USER_AGENT};
use serde_json::Value;

/// An online source: the image links it has for what is wanted.
type Finder = fn(&Wanted) -> Vec<String>;

/// Covers each online source offers at most.
const PER_SOURCE: usize = 3;
/// Longest side of the picture shown in the dialog.
const THUMB: u32 = 240;
/// The smallest cover Analyze Track takes on its own, on its shorter side.
pub const AUTO_MIN: u32 = 600;

/// The online sources, in the order [`first_online`] asks them: iTunes
/// first, as it is the quickest to answer and so the dialog's first cover.
const ONLINE: [(&str, Finder); 3] = [
    ("iTunes", itunes),
    ("Cover Art Archive", cover_art_archive),
    ("Discogs", discogs),
];

/// What a search is for.
#[derive(Clone, Debug, Default)]
pub struct Wanted {
    pub path: Option<PathBuf>,
    pub artist: String,
    pub album: String,
    pub title: String,
    pub discogs_token: String,
}

/// A cover found: where, the image file's bytes, and its size.
pub struct Found {
    pub source: String,
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Scaled down for showing, RGBA.
    pub thumb: cover::Cover,
}

pub enum Event {
    Found(Found),
    /// Every source has answered.
    Done,
}

/// Starts looking; covers arrive on the returned channel, then `Done`.
pub fn search(wanted: Wanted) -> Receiver<Event> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("cover-search".into())
        .spawn(move || {
            std::thread::scope(|s| {
                let w = &wanted;
                let local = tx.clone();
                s.spawn(move || {
                    if let Some(path) = &w.path {
                        if let Some(data) = tags::embedded_cover(path) {
                            send(&local, "Tag", data);
                        }
                        if let Some(file) = cover::beside(path, &w.album) {
                            if let Ok(data) = std::fs::read(file) {
                                send(&local, "Folder", data);
                            }
                        }
                    }
                });
                for (name, find) in ONLINE {
                    let tx = tx.clone();
                    s.spawn(move || {
                        for url in find(w).into_iter().take(PER_SOURCE) {
                            if let Some(data) = download(&url, w) {
                                send(&tx, name, data);
                            }
                        }
                    });
                }
            });
            let _ = tx.send(Event::Done);
        })
        .expect("spawn cover search");
    rx
}

/// Longest side of a cover the batch embeds; bigger ones are scaled down.
pub const BATCH_SIZE: u32 = 1024;

/// The largest cover the online sources have for what is wanted, as JPEG
/// or PNG data no bigger than [`BATCH_SIZE`] on its longest side. Waits
/// for every source.
pub fn best_cover(w: &Wanted) -> Option<Vec<u8>> {
    let found: Vec<Vec<u8>> = std::thread::scope(|s| {
        let jobs: Vec<_> = ONLINE
            .iter()
            .map(|(_, find)| {
                s.spawn(move || {
                    find(w)
                        .into_iter()
                        .take(PER_SOURCE)
                        .filter_map(|url| download(&url, w))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        jobs.into_iter()
            .flat_map(|j| j.join().unwrap_or_default())
            .collect()
    });
    largest(found)
}

/// The biggest of these images, scaled down to [`BATCH_SIZE`] when bigger.
fn largest(found: Vec<Vec<u8>>) -> Option<Vec<u8>> {
    let (data, image) = found
        .into_iter()
        .filter_map(|d| image::load_from_memory(&d).ok().map(|i| (d, i)))
        .max_by_key(|(_, i)| i.width() as u64 * i.height() as u64)?;
    if image.width().max(image.height()) <= BATCH_SIZE {
        return Some(data);
    }
    let small = image.thumbnail(BATCH_SIZE, BATCH_SIZE).to_rgb8();
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90)
        .encode_image(&small)
        .ok()?;
    Some(out)
}

/// The first cover the online sources have for what is wanted, as the
/// image file's bytes, when it is at least `min` pixels on its shorter
/// side; None when there is none or the first is smaller. Waits for the
/// answers, so it is for a background thread.
pub fn first_online(wanted: &Wanted, min: u32) -> Option<Vec<u8>> {
    let data = ONLINE.iter().find_map(|(_, find)| {
        let url = find(wanted).into_iter().next()?;
        download(&url, wanted)
    })?;
    big_enough(&data, min).then_some(data)
}

/// Whether the image is at least `min` pixels on its shorter side.
fn big_enough(data: &[u8], min: u32) -> bool {
    image::ImageReader::new(std::io::Cursor::new(data))
        .with_guessed_format()
        .ok()
        .and_then(|r| r.into_dimensions().ok())
        .is_some_and(|(w, h)| w.min(h) >= min)
}

fn send(tx: &Sender<Event>, source: &str, data: Vec<u8>) {
    if let Some(f) = found(source, data) {
        let _ = tx.send(Event::Found(f));
    }
}

/// The picture in the file at `path` as a cover the DJ picked, scaled
/// down to [`BATCH_SIZE`] when bigger, so it doesn't bloat every track it
/// goes into; Err says why it can't be one.
pub fn from_file(path: &Path) -> Result<Found, String> {
    let data = std::fs::read(path).map_err(|e| format!("Couldn't read the file: {e}"))?;
    largest(vec![data])
        .and_then(|data| found("File", data))
        .ok_or_else(|| "That file isn't a JPEG or PNG picture.".to_string())
}

/// `data` (an image file's bytes) as a cover found at `source`, with its
/// size and a thumbnail; `None` if it isn't a picture.
pub fn found(source: &str, data: Vec<u8>) -> Option<Found> {
    let image = image::load_from_memory(&data).ok()?;
    let (width, height) = (image.width(), image.height());
    let small = if width.max(height) > THUMB {
        image.thumbnail(THUMB, THUMB)
    } else {
        image
    };
    let rgba = small.to_rgba8();
    Some(Found {
        source: source.into(),
        data,
        width,
        height,
        thumb: cover::Cover {
            width: rgba.width(),
            height: rgba.height(),
            rgba: rgba.into_raw(),
        },
    })
}

/// A small square picture from `url` (a result's cover in a source's
/// list), as RGBA pixels with its side length.
pub fn thumbnail(url: &str, side: u32) -> Option<(u32, Vec<u8>)> {
    let data = agent()
        .get(url)
        .call()
        .ok()?
        .body_mut()
        .with_config()
        .limit(8 << 20)
        .read_to_vec()
        .ok()?;
    let image = image::load_from_memory(&data).ok()?;
    let small = image.resize_to_fill(side, side, image::imageops::FilterType::Triangle);
    Some((side, small.to_rgba8().into_raw()))
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(12)))
        .user_agent(USER_AGENT)
        .build()
        .into()
}

pub(crate) fn get_json(url: &str, auth: Option<&str>) -> Option<Value> {
    try_json(url, auth).ok()
}

/// Why a request for JSON gave nothing back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Failed {
    /// The service answered that there is no such thing.
    NotThere,
    /// No answer, a busy service, or one that couldn't be read.
    Unreachable,
}

/// Like [`get_json`], telling apart "not there" from "couldn't ask".
pub(crate) fn try_json(url: &str, auth: Option<&str>) -> Result<Value, Failed> {
    let mut req = agent().get(url).header("Accept", "application/json");
    if let Some(a) = auth {
        req = req.header("Authorization", a);
    }
    match req.call() {
        Ok(mut r) => r.body_mut().read_json().map_err(|_| Failed::Unreachable),
        Err(ureq::Error::StatusCode(404)) => Err(Failed::NotThere),
        Err(_) => Err(Failed::Unreachable),
    }
}

fn download(url: &str, w: &Wanted) -> Option<Vec<u8>> {
    let mut req = agent().get(url);
    if url.contains("discogs.com") && !w.discogs_token.is_empty() {
        req = req.header(
            "Authorization",
            &format!("Discogs token={}", w.discogs_token),
        );
    }
    req.call()
        .ok()?
        .body_mut()
        .with_config()
        .limit(20 << 20)
        .read_to_vec()
        .ok()
}

/// What to search the album by: its name, or the track's title for a
/// single.
fn release(w: &Wanted) -> &str {
    if w.album.trim().is_empty() {
        w.title.trim()
    } else {
        w.album.trim()
    }
}

/// Lucene's special characters, escaped for MusicBrainz's search.
pub(crate) fn lucene(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            let special = "+-&|!(){}[]^\"~*?:\\/".contains(c);
            special.then_some('\\').into_iter().chain(Some(c))
        })
        .collect()
}

fn cover_art_archive(w: &Wanted) -> Vec<String> {
    let title = release(w);
    if title.is_empty() {
        return Vec::new();
    }
    let mut query = format!("release:\"{}\"", lucene(title));
    if !w.artist.trim().is_empty() {
        query += &format!(" AND artist:\"{}\"", lucene(w.artist.trim()));
    }
    let url = format!(
        "https://musicbrainz.org/ws/2/release/?fmt=json&limit=10&query={}",
        encode(&query)
    );
    get_json(&url, None)
        .map(|v| cover_art_archive_urls(&v))
        .unwrap_or_default()
}

/// One cover per release group (an album's editions share theirs), best
/// matches first.
fn cover_art_archive_urls(v: &Value) -> Vec<String> {
    let mut groups: Vec<&str> = Vec::new();
    for r in v["releases"].as_array().into_iter().flatten() {
        if r["score"].as_i64().unwrap_or(100) < 80 {
            continue;
        }
        if let Some(id) = r["release-group"]["id"].as_str() {
            if !groups.contains(&id) {
                groups.push(id);
            }
        }
    }
    groups
        .into_iter()
        .map(|id| format!("https://coverartarchive.org/release-group/{id}/front-1200"))
        .collect()
}

fn itunes(w: &Wanted) -> Vec<String> {
    let term = format!("{} {}", w.artist.trim(), release(w));
    if term.trim().is_empty() {
        return Vec::new();
    }
    let entity = if w.album.trim().is_empty() {
        "song"
    } else {
        "album"
    };
    let url = format!(
        "https://itunes.apple.com/search?media=music&entity={entity}&limit={PER_SOURCE}&term={}",
        encode(term.trim())
    );
    get_json(&url, None)
        .map(|v| itunes_urls(&v))
        .unwrap_or_default()
}

/// The artwork links, asked for at 1200 pixels instead of 100.
fn itunes_urls(v: &Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for r in v["results"].as_array().into_iter().flatten() {
        if let Some(url) = r["artworkUrl100"].as_str() {
            let big = url.replace("100x100bb", "1200x1200bb");
            if !out.contains(&big) {
                out.push(big);
            }
        }
    }
    out
}

fn discogs(w: &Wanted) -> Vec<String> {
    // Discogs only answers searches with a token.
    if w.discogs_token.trim().is_empty() || release(w).is_empty() {
        return Vec::new();
    }
    let mut url = format!(
        "https://api.discogs.com/database/search?type=release&per_page=10&release_title={}",
        encode(release(w))
    );
    if !w.artist.trim().is_empty() {
        url += &format!("&artist={}", encode(w.artist.trim()));
    }
    let auth = format!("Discogs token={}", w.discogs_token.trim());
    get_json(&url, Some(&auth))
        .map(|v| discogs_urls(&v))
        .unwrap_or_default()
}

fn discogs_urls(v: &Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for r in v["results"].as_array().into_iter().flatten() {
        // Releases without art point at a placeholder ("spacer.gif").
        if let Some(url) = r["cover_image"].as_str().filter(|u| !u.ends_with(".gif")) {
            if !out.iter().any(|o| o == url) {
                out.push(url.to_string());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_musicbrainz_answers() {
        let v = json!({"releases": [
            {"id": "r1", "score": 100, "release-group": {"id": "g1"}},
            {"id": "r2", "score": 98, "release-group": {"id": "g1"}},
            {"id": "r3", "score": 90, "release-group": {"id": "g2"}},
            {"id": "r4", "score": 40, "release-group": {"id": "g3"}},
        ]});
        assert_eq!(
            cover_art_archive_urls(&v),
            [
                "https://coverartarchive.org/release-group/g1/front-1200",
                "https://coverartarchive.org/release-group/g2/front-1200"
            ]
        );
    }

    #[test]
    fn reads_itunes_and_discogs_answers() {
        let v = json!({"resultCount": 2, "results": [
            {"artworkUrl100": "https://is1-ssl.mzstatic.com/image/thumb/a/100x100bb.jpg"},
            {"artworkUrl100": "https://is1-ssl.mzstatic.com/image/thumb/a/100x100bb.jpg"},
        ]});
        assert_eq!(
            itunes_urls(&v),
            ["https://is1-ssl.mzstatic.com/image/thumb/a/1200x1200bb.jpg"]
        );
        let v = json!({"results": [
            {"cover_image": "https://i.discogs.com/a.jpeg"},
            {"cover_image": "https://st.discogs.com/images/spacer.gif"},
        ]});
        assert_eq!(discogs_urls(&v), ["https://i.discogs.com/a.jpeg"]);
    }

    #[test]
    fn keeps_the_largest_cover_scaled_down() {
        let png = |w, h| {
            let mut out = std::io::Cursor::new(Vec::new());
            image::RgbImage::new(w, h)
                .write_to(&mut out, image::ImageFormat::Png)
                .unwrap();
            out.into_inner()
        };
        let small = png(300, 300);
        let got = largest(vec![small.clone(), b"junk".to_vec()]).unwrap();
        assert_eq!(got, small);
        let big = largest(vec![small, png(2000, 1500)]).unwrap();
        let i = image::load_from_memory(&big).unwrap();
        assert_eq!((i.width(), i.height()), (1024, 768));
    }

    #[test]
    fn escapes_searches() {
        assert_eq!(lucene("AC/DC: Live!"), "AC\\/DC\\: Live\\!");
        let w = Wanted {
            title: "Single".into(),
            ..Default::default()
        };
        assert_eq!(release(&w), "Single");
    }

    #[test]
    fn takes_only_big_enough_covers() {
        let png = |w, h| {
            let mut out = Vec::new();
            image::RgbaImage::from_pixel(w, h, image::Rgba([1, 2, 3, 255]))
                .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
                .unwrap();
            out
        };
        assert!(big_enough(&png(600, 600), AUTO_MIN));
        assert!(big_enough(&png(1200, 1000), AUTO_MIN));
        assert!(!big_enough(&png(600, 599), AUTO_MIN));
        assert!(!big_enough(b"not an image", AUTO_MIN));
    }

    #[test]
    fn finds_the_tag_and_folder_covers() {
        let dir = std::env::temp_dir().join(format!("djsugar-coverart-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("song.mp3");
        std::fs::write(&path, b"not really audio").unwrap();
        image::RgbaImage::from_pixel(600, 500, image::Rgba([1, 2, 3, 255]))
            .save(dir.join("cover.png"))
            .unwrap();
        let rx = search(Wanted {
            path: Some(path),
            ..Default::default()
        });
        let mut found = Vec::new();
        for e in rx {
            match e {
                Event::Found(f) => found.push((f.source, f.width, f.height, f.thumb.width)),
                Event::Done => break,
            }
        }
        assert_eq!(found, [("Folder".to_string(), 600, 500, 240)]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reads_a_picked_picture_file() {
        let dir = std::env::temp_dir().join(format!("djsugar-coverfile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let big = dir.join("big.png");
        image::RgbaImage::from_pixel(2048, 1024, image::Rgba([1, 2, 3, 255]))
            .save(&big)
            .unwrap();
        let f = from_file(&big).unwrap();
        assert_eq!((f.source.as_str(), f.width, f.height), ("File", 1024, 512));
        let small = dir.join("small.png");
        image::RgbaImage::from_pixel(500, 500, image::Rgba([1, 2, 3, 255]))
            .save(&small)
            .unwrap();
        let f = from_file(&small).unwrap();
        assert_eq!(f.data, std::fs::read(&small).unwrap());
        let text = dir.join("notes.txt");
        std::fs::write(&text, b"not a picture").unwrap();
        assert!(from_file(&text).is_err());
        assert!(from_file(&dir.join("gone.jpg")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

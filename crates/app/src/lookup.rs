//! Looking up a track's tags online: its audio fingerprint (Chromaprint,
//! as AcoustID uses) finds the recording on MusicBrainz when the DJ gave
//! an AcoustID key; otherwise, or when the sound isn't known, MusicBrainz
//! is searched by artist and title: as tagged first, then without the
//! "(Original Mix)", "[Label]" and "feat." parts that DJ files carry and
//! MusicBrainz doesn't. Genres come from MusicBrainz (the recording's, or
//! failing that its artist's) unless Discogs, with the DJ's token, gives a
//! style. Discogs also adds the label and year. Requests are paced as each
//! service asks, and a busy service is asked once more before the track
//! counts as unreachable. Nothing is saved here: what is found goes to the
//! clean-up's review table.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::Engine as _;
use djsugar_engine::decode::Decoder;
use djsugar_plugins::encode;
use rusty_chromaprint::{Configuration, Fingerprinter};
use serde_json::Value;

use crate::coverart::{lucene, try_json, Failed};

/// Seconds of audio fingerprinted, as `fpcalc` does.
const FINGERPRINT_SECONDS: usize = 120;

/// A search result at least this sure ends the search: the looser
/// searches after it are only for tracks the exact one missed.
const GOOD_ENOUGH: f32 = 0.9;

/// Search results less sure than this aren't a match at all.
const AT_LEAST: f32 = 0.5;

/// What to look up a track by.
#[derive(Clone, Debug, Default)]
pub struct Query {
    pub path: Option<PathBuf>,
    pub artist: String,
    pub title: String,
    /// The artist and title came from the file's name, not its tags.
    pub by_file_name: bool,
    /// Seconds.
    pub duration: f64,
    pub acoustid_key: String,
    pub discogs_token: String,
}

/// What was found for a track, and how sure the match is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    pub artist: String,
    pub title: String,
    pub album: String,
    pub year: String,
    pub label: String,
    pub genre: String,
    /// The genre is the artist's, not this recording's: a guess.
    pub genre_guessed: bool,
    /// From 0 to 1.
    pub score: f32,
    /// Where the match came from: "AcoustID" or "MusicBrainz".
    pub source: &'static str,
    /// MusicBrainz ids of the recording and its first artist.
    pub recording_id: String,
    pub artist_id: String,
}

/// Why nothing came back for a track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Miss {
    /// Asked, and nothing matched.
    NotFound,
    /// MusicBrainz or AcoustID couldn't be reached, or stayed busy.
    Unreachable,
}

/// Looks a track up.
pub fn look_up(q: &Query) -> Result<Found, Miss> {
    let mut unreachable = false;
    let by_sound = match by_fingerprint(q) {
        Ok(f) => Some(f),
        Err(Miss::Unreachable) => {
            unreachable = true;
            None
        }
        Err(Miss::NotFound) => None,
    };
    let mut found = match by_sound {
        Some(f) => f,
        None => match by_name(q) {
            Ok(f) => f,
            Err(Miss::NotFound) if unreachable => return Err(Miss::Unreachable),
            Err(e) => return Err(e),
        },
    };
    if !q.discogs_token.trim().is_empty() {
        if let Some(d) = discogs(q, &found) {
            if found.year.is_empty() {
                found.year = d.year;
            }
            found.label = d.label;
            if !d.genre.is_empty() {
                found.genre = d.genre;
                found.genre_guessed = false;
            }
        }
    }
    if found.genre.is_empty() && !found.artist_id.is_empty() {
        let url = format!(
            "https://musicbrainz.org/ws/2/artist/{}?inc=genres&fmt=json",
            found.artist_id
        );
        if let Ok(v) = MUSICBRAINZ.get(&url, None) {
            found.genre = top_genre(&v);
            found.genre_guessed = !found.genre.is_empty();
        }
    }
    Ok(found)
}

/// Keeps requests to one service apart, and asks a busy one again once.
struct Paced {
    last: Mutex<Option<Instant>>,
    gap: Duration,
}

/// MusicBrainz asks apps for one request a second.
static MUSICBRAINZ: Paced = Paced::new(1100);
/// AcoustID allows three a second.
static ACOUSTID: Paced = Paced::new(340);
/// Discogs allows 60 a minute with a token.
static DISCOGS: Paced = Paced::new(1000);

impl Paced {
    const fn new(ms: u64) -> Self {
        Paced {
            last: Mutex::new(None),
            gap: Duration::from_millis(ms),
        }
    }

    fn wait(&self) {
        let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = *last {
            let since = t.elapsed();
            if since < self.gap {
                std::thread::sleep(self.gap - since);
            }
        }
        *last = Some(Instant::now());
    }

    fn get(&self, url: &str, auth: Option<&str>) -> Result<Value, Miss> {
        for attempt in 0..2 {
            if attempt > 0 {
                // A busy service (MusicBrainz answers 503) gets a breather.
                std::thread::sleep(Duration::from_secs(3));
            }
            self.wait();
            match try_json(url, auth) {
                Ok(v) => return Ok(v),
                Err(Failed::NotThere) => return Err(Miss::NotFound),
                Err(Failed::Unreachable) => {}
            }
        }
        Err(Miss::Unreachable)
    }
}

/// The recording AcoustID knows by the track's fingerprint, with its
/// releases and genres from MusicBrainz.
fn by_fingerprint(q: &Query) -> Result<Found, Miss> {
    if q.acoustid_key.trim().is_empty() {
        return Err(Miss::NotFound);
    }
    let Some((fingerprint, seconds)) = q.path.as_ref().and_then(|p| fingerprint(p)) else {
        return Err(Miss::NotFound);
    };
    let duration = if q.duration > 0.0 {
        q.duration
    } else {
        seconds
    };
    let url = format!(
        "https://api.acoustid.org/v2/lookup?client={}&meta=recordings&duration={}&fingerprint={}",
        encode(q.acoustid_key.trim()),
        duration.round() as u64,
        fingerprint
    );
    let (id, score) = acoustid_best(&ACOUSTID.get(&url, None)?).ok_or(Miss::NotFound)?;
    let mut found = recording_by_id(&id)?;
    found.score = score;
    found.source = "AcoustID";
    Ok(found)
}

/// A MusicBrainz recording with its artist, releases and genres.
fn recording_by_id(id: &str) -> Result<Found, Miss> {
    let v = MUSICBRAINZ.get(
        &format!(
            "https://musicbrainz.org/ws/2/recording/{id}?inc=artist-credits+releases+genres&fmt=json"
        ),
        None,
    )?;
    recording(&v).ok_or(Miss::NotFound)
}

/// The recording MusicBrainz finds by artist and title: as they are, then
/// without the parts MusicBrainz doesn't have, then by their words in any
/// order. The surest of these wins.
fn by_name(q: &Query) -> Result<Found, Miss> {
    let (artist, title) = (q.artist.trim(), q.title.trim());
    if artist.is_empty() || title.is_empty() {
        return Err(Miss::NotFound);
    }
    let (bare_artist, bare_title) = (search_artist(artist), search_title(title));
    let phrase =
        |t: &str, a: &str| format!("recording:\"{}\" AND artist:\"{}\"", lucene(t), lucene(a));
    let mut queries = vec![phrase(title, artist)];
    if (bare_title.as_str(), bare_artist.as_str()) != (title, artist) && !bare_title.is_empty() {
        queries.push(phrase(&bare_title, &bare_artist));
    }
    if !bare_title.is_empty() && !bare_artist.is_empty() {
        queries.push(format!(
            "recording:({}) AND artist:({})",
            words(&bare_title),
            words(&bare_artist)
        ));
    }
    let mut best: Option<Found> = None;
    let mut reached = false;
    for query in queries {
        let url = format!(
            "https://musicbrainz.org/ws/2/recording?fmt=json&limit=5&query={}",
            encode(&query)
        );
        let Ok(v) = MUSICBRAINZ.get(&url, None) else {
            continue;
        };
        reached = true;
        if let Some(mut f) = musicbrainz_best(&v, q.duration) {
            // A loose search can rank a different song first: a match
            // whose name isn't the one asked for is a lot less sure.
            if !agrees(&f, &bare_title, &bare_artist) {
                f.score *= 0.6;
            }
            if best.as_ref().is_none_or(|b| f.score > b.score) {
                best = Some(f);
            }
        }
        if best.as_ref().is_some_and(|b| b.score >= GOOD_ENOUGH) {
            break;
        }
    }
    let Some(mut found) = best.filter(|f| f.score >= AT_LEAST) else {
        return Err(if reached {
            Miss::NotFound
        } else {
            Miss::Unreachable
        });
    };
    // Search results carry no genres: the recording itself does.
    if !found.recording_id.is_empty() {
        if let Ok(full) = recording_by_id(&found.recording_id) {
            found.genre = full.genre;
        }
    }
    if q.by_file_name {
        found.source = "MusicBrainz, by file name";
    }
    Ok(found)
}

/// A title as MusicBrainz is likely to have it: without parts in
/// brackets ("(Original Mix)", "[Label]"), a guest ("feat. X") or
/// underscores.
pub fn search_title(title: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for c in title.replace('_', " ").chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let out = cut_guest(&out);
    out.trim()
        .trim_end_matches(['-', '–', ' '])
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// An artist without a guest ("A feat. B" is A) or underscores.
pub fn search_artist(artist: &str) -> String {
    let a = artist.replace('_', " ");
    cut_guest(&a)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Text up to a " feat.", " ft." or " featuring".
fn cut_guest(s: &str) -> String {
    let lower = s.to_lowercase();
    let cut = [" feat. ", " feat ", " ft. ", " ft ", " featuring "]
        .iter()
        .filter_map(|w| lower.find(w))
        .min()
        .unwrap_or(s.len());
    // The lower-case text has the same byte positions for these words.
    s.get(..cut).unwrap_or(s).to_string()
}

/// Whether a match's title and artist are the ones searched for, ignoring
/// case, punctuation and bracketed parts; one may hold the other ("Bonobo"
/// in "Bonobo & X").
fn agrees(f: &Found, title: &str, artist: &str) -> bool {
    let key = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect()
    };
    let alike =
        |a: String, b: String| !a.is_empty() && !b.is_empty() && (a.contains(&b) || b.contains(&a));
    alike(key(&search_title(&f.title)), key(title)) && alike(key(&f.artist), key(artist))
}

/// Words for a Lucene query, each escaped, in any order.
fn words(s: &str) -> String {
    s.split_whitespace()
        .map(lucene)
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The genre most voted for in a MusicBrainz answer, in title case.
fn top_genre(v: &Value) -> String {
    let Some(best) = v["genres"]
        .as_array()
        .and_then(|g| g.iter().max_by_key(|g| g["count"].as_i64().unwrap_or(0)))
        .and_then(|g| g["name"].as_str())
    else {
        return String::new();
    };
    best.split(' ')
        .map(|w| {
            if ["uk", "us", "edm", "idm", "ebm", "r&b", "dnb", "lo-fi"].contains(&w) {
                return w.to_uppercase().replace("LO-FI", "Lo-Fi");
            }
            w.split('-')
                .map(|p| {
                    let mut c = p.chars();
                    c.next()
                        .map(|f| f.to_uppercase().chain(c).collect::<String>())
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join("-")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The track's Chromaprint fingerprint, compressed and encoded as
/// AcoustID takes it, and the file's length in seconds.
pub fn fingerprint(path: &Path) -> Option<(String, f64)> {
    let audio = Decoder::open(path).ok()?.run(|_| true).ok()?;
    let rate = audio.sample_rate;
    let config = Configuration::preset_test2();
    let mut printer = Fingerprinter::new(&config);
    printer.start(rate, 2).ok()?;
    let frames = audio.len().min(FINGERPRINT_SECONDS * rate as usize);
    let mut samples = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let [l, r] = audio.frame(i);
        samples.push((l.clamp(-1.0, 1.0) * 32767.0) as i16);
        samples.push((r.clamp(-1.0, 1.0) * 32767.0) as i16);
    }
    printer.consume(&samples);
    printer.finish();
    let compressed =
        rusty_chromaprint::FingerprintCompressor::from(&config).compress(printer.fingerprint());
    let text = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(compressed);
    Some((text, audio.duration()))
}

/// The best recording in an AcoustID answer: its MusicBrainz id and score.
fn acoustid_best(v: &Value) -> Option<(String, f32)> {
    v["results"]
        .as_array()?
        .iter()
        .filter_map(|r| {
            let score = r["score"].as_f64()? as f32;
            let id = r["recordings"].as_array()?.first()?["id"].as_str()?;
            Some((id.to_string(), score))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
}

/// A MusicBrainz recording's artist, title, and its first official album
/// (the earliest), from a lookup or a search result.
fn recording(r: &Value) -> Option<Found> {
    let title = r["title"].as_str()?.to_string();
    let artist: String = r["artist-credit"]
        .as_array()?
        .iter()
        .map(|c| {
            format!(
                "{}{}",
                c["name"].as_str().unwrap_or(""),
                c["joinphrase"].as_str().unwrap_or("")
            )
        })
        .collect();
    let mut releases: Vec<&Value> = r["releases"].as_array().into_iter().flatten().collect();
    // Official albums first, then the earliest.
    releases.sort_by_key(|rel| {
        let official = rel["status"].as_str() == Some("Official");
        let date = rel["date"]
            .as_str()
            .filter(|d| !d.is_empty())
            .unwrap_or("9999");
        (!official, date.to_string())
    });
    let first = releases.first();
    let album = first
        .and_then(|rel| rel["title"].as_str())
        .unwrap_or("")
        .to_string();
    // The recording's first release, when MusicBrainz gives it, beats
    // the album's date: a reissue's album is often the only one listed.
    let year = r["first-release-date"]
        .as_str()
        .filter(|d| d.len() >= 4)
        .or_else(|| first.and_then(|rel| rel["date"].as_str()))
        .and_then(|d| d.get(..4))
        .unwrap_or("")
        .to_string();
    let artist_id = r["artist-credit"][0]["artist"]["id"]
        .as_str()
        .unwrap_or("")
        .to_string();
    Some(Found {
        artist: artist.trim().to_string(),
        title,
        album,
        year,
        genre: top_genre(r),
        source: "MusicBrainz",
        recording_id: r["id"].as_str().unwrap_or("").to_string(),
        artist_id,
        ..Default::default()
    })
}

/// The best search result: the highest score, then the one closest in
/// length. A length more than 10 seconds off counts against it.
fn musicbrainz_best(v: &Value, duration: f64) -> Option<Found> {
    let best = v["recordings"].as_array()?.iter().max_by_key(|r| {
        let mut score = r["score"].as_i64().unwrap_or(0);
        if let (Some(ms), true) = (r["length"].as_f64(), duration > 0.0) {
            if (ms / 1000.0 - duration).abs() > 10.0 {
                score -= 30;
            }
        }
        score
    })?;
    let mut found = recording(best)?;
    let mut score = best["score"].as_f64().unwrap_or(0.0) / 100.0;
    if let (Some(ms), true) = (best["length"].as_f64(), duration > 0.0) {
        if (ms / 1000.0 - duration).abs() > 10.0 {
            score *= 0.7;
        }
    }
    found.score = score as f32;
    Some(found)
}

/// What Discogs adds: the year, label and style of the release.
fn discogs(q: &Query, found: &Found) -> Option<Found> {
    let artist = if found.artist.is_empty() {
        &q.artist
    } else {
        &found.artist
    };
    let mut url = format!(
        "https://api.discogs.com/database/search?type=release&per_page=5&artist={}",
        encode(artist.trim())
    );
    if !found.album.is_empty() {
        url += &format!("&release_title={}", encode(&found.album));
    } else {
        url += &format!("&track={}", encode(&found.title));
    }
    let auth = format!("Discogs token={}", q.discogs_token.trim());
    discogs_first(&DISCOGS.get(&url, Some(&auth)).ok()?)
}

fn discogs_first(v: &Value) -> Option<Found> {
    let r = v["results"].as_array()?.first()?;
    let first = |key: &str| {
        r[key]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string()
    };
    let style = first("style");
    Some(Found {
        year: r["year"].as_str().unwrap_or("").to_string(),
        label: first("label"),
        genre: if style.is_empty() {
            first("genre")
        } else {
            style
        },
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_acoustid_and_musicbrainz_answers() {
        let v = json!({"status": "ok", "results": [
            {"score": 0.42, "recordings": [{"id": "a"}]},
            {"score": 0.97, "recordings": [{"id": "b"}]},
            {"score": 0.99}
        ]});
        assert_eq!(acoustid_best(&v), Some(("b".into(), 0.97)));
        let r = json!({
            "title": "Kerala",
            "artist-credit": [{"name": "Bonobo", "joinphrase": " feat. "}, {"name": "X"}],
            "releases": [
                {"title": "Kerala (Remixes)", "date": "2017-06-02", "status": "Official"},
                {"title": "Migration", "date": "2017-01-13", "status": "Official"},
                {"title": "Bootleg", "date": "2016", "status": "Bootleg"}
            ]
        });
        let f = recording(&r).unwrap();
        assert_eq!(
            (
                f.artist.as_str(),
                f.title.as_str(),
                f.album.as_str(),
                f.year.as_str()
            ),
            ("Bonobo feat. X", "Kerala", "Migration", "2017")
        );
    }

    #[test]
    fn prefers_matches_of_the_right_length() {
        let v = json!({"recordings": [
            {"score": 100, "title": "Clouds", "length": 600000, "artist-credit": [{"name": "A"}]},
            {"score": 95, "title": "Clouds", "length": 271000, "artist-credit": [{"name": "A"}]}
        ]});
        let f = musicbrainz_best(&v, 271.5).unwrap();
        assert!((f.score - 0.95).abs() < 1e-6);
        let v = json!({"recordings": [
            {"score": 100, "title": "Clouds", "length": 600000, "artist-credit": [{"name": "A"}]}
        ]});
        assert!(musicbrainz_best(&v, 271.5).unwrap().score < 0.8);
    }

    #[test]
    fn reads_discogs_styles() {
        let v = json!({"results": [{"year": "2014", "label": ["Ninja Tune", "x"],
            "genre": ["Electronic"], "style": ["Downtempo", "Ambient"]}]});
        let d = discogs_first(&v).unwrap();
        assert_eq!(
            (d.year.as_str(), d.label.as_str(), d.genre.as_str()),
            ("2014", "Ninja Tune", "Downtempo")
        );
    }

    #[test]
    fn fingerprints_audio() {
        let dir = std::env::temp_dir().join(format!("djsugar-fp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tone.wav");
        let rate = 44100u32;
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for i in 0..rate * 10 {
            let t = i as f32 / rate as f32;
            let s = ((t * 440.0 * std::f32::consts::TAU).sin() * (t * 3.0).sin() * 12000.0) as i16;
            w.write_sample(s).unwrap();
            w.write_sample(s).unwrap();
        }
        w.finalize().unwrap();
        let (fp, seconds) = fingerprint(&path).unwrap();
        assert!(fp.len() > 20, "{fp}");
        assert!((seconds - 10.0).abs() < 0.1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn searches_without_what_musicbrainz_lacks() {
        assert_eq!(search_title("Kerala (Original Mix) [Ninja Tune]"), "Kerala");
        assert_eq!(search_title("Clouds feat. Someone (Extended)"), "Clouds");
        assert_eq!(search_title("Night_Drive - "), "Night Drive");
        assert_eq!(search_title("(Intro)"), "");
        assert_eq!(search_artist("Bonobo ft. Szjerdene"), "Bonobo");
        assert_eq!(search_artist("Simon & Garfunkel"), "Simon & Garfunkel");
        assert_eq!(words("a+b c"), "a\\+b c");
    }

    #[test]
    fn reads_genres_ids_and_first_release() {
        let r = json!({
            "id": "rec",
            "title": "Kerala",
            "first-release-date": "2016-11-01",
            "artist-credit": [{"name": "Bonobo", "artist": {"id": "art"}}],
            "releases": [{"title": "Migration", "date": "2017-01-13", "status": "Official"}],
            "genres": [{"name": "downtempo", "count": 3}, {"name": "uk garage", "count": 5}]
        });
        let f = recording(&r).unwrap();
        assert_eq!(f.year, "2016");
        assert_eq!(f.genre, "UK Garage");
        assert_eq!(
            (f.recording_id.as_str(), f.artist_id.as_str()),
            ("rec", "art")
        );
        assert_eq!(
            top_genre(&json!({"genres": [{"name": "deep house", "count": 1}]})),
            "Deep House"
        );
        assert_eq!(top_genre(&json!({})), "");
    }

    #[test]
    fn a_match_agrees_by_name_not_by_case() {
        let f = Found {
            title: "Kerala".into(),
            artist: "Bonobo & X".into(),
            ..Default::default()
        };
        assert!(agrees(&f, "kerala", "Bonobo"));
        assert!(!agrees(&f, "Cirrus", "Bonobo"));
    }
}

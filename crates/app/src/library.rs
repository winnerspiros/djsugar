use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::state::SAMPLES;

const AUDIO: [&str; 9] = ["mp3", "flac", "wav", "ogg", "opus", "m4a", "aac", "aif", "aiff"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Sample(&'static str),
    File(Arc<Path>),
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub source: Source,
    pub title: Box<str>,
    pub artist: Box<str>,
    pub folder: Box<str>,
    key: Box<str>,
    loose: Box<str>,
}

impl Entry {
    fn new(source: Source, name: &str, folder: &str) -> Self {
        let (artist, title) = name.split_once(" - ").map_or(("", name), |(a, t)| (a.trim(), t.trim()));
        let title = title.replace('_', " ");
        let key = fold(&format!("{title} {artist} {folder}"));
        let loose = key.replace('h', "");
        Self {
            source,
            title: title.into(),
            artist: artist.into(),
            folder: folder.into(),
            key: key.into(),
            loose: loose.into(),
        }
    }

    pub fn extension(&self) -> &str {
        let name = match &self.source {
            Source::Sample(s) => s,
            Source::File(p) => p.to_str().unwrap_or(""),
        };
        name.rsplit_once('.').map_or("", |(_, e)| e)
    }
}

fn split(path: &str) -> (&str, &str) {
    let (dir, file) = path.rsplit_once('/').unwrap_or(("samples", path));
    (dir.rsplit('/').next().unwrap_or(dir), file.rsplit_once('.').map_or(file, |(s, _)| s))
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Meta {
    pub bpm: f32,
    pub key: Option<sugar_analysis::Key>,
    pub secs: f32,
}

impl Meta {
    pub fn of(t: &sugar_engine::Track) -> Self {
        Self {
            bpm: sugar_analysis::grid(t.rate, &t.frames).map_or(0.0, |g| crate::settings::fold(g).bpm as f32),
            key: sugar_analysis::key(t.rate, &t.frames),
            secs: t.frames.len() as f32 / t.rate as f32,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Column {
    #[default]
    Index,
    Title,
    Artist,
    Bpm,
    Key,
    Time,
    Rating,
}

pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(char::to_lowercase) {
        let latin = match c {
            'α' | 'ά' => "a",
            'β' => "v",
            'γ' => "g",
            'δ' => "d",
            'ε' | 'έ' => "e",
            'ζ' => "z",
            'η' | 'ή' | 'ι' | 'ί' | 'ϊ' | 'ΐ' | 'υ' | 'ύ' | 'ϋ' | 'ΰ' => "i",
            'θ' => "th",
            'κ' => "k",
            'λ' => "l",
            'μ' => "m",
            'ν' => "n",
            'ξ' => "x",
            'ο' | 'ό' | 'ω' | 'ώ' => "o",
            'π' => "p",
            'ρ' => "r",
            'σ' | 'ς' => "s",
            'τ' => "t",
            'φ' => "f",
            'χ' => "ch",
            'ψ' => "ps",
            '_' | '-' | '.' => " ",
            _ => {
                out.push(c);
                continue;
            }
        };
        out.push_str(latin);
    }
    out
}

pub fn matches(entry: &Entry, query: &str) -> bool {
    query.split_whitespace().all(|w| {
        let (negate, w) = w.strip_prefix('-').map_or((false, w), |r| (true, r));
        let w = fold(w);
        let w = w.trim();
        let hit = w.is_empty() || entry.key.contains(w) || entry.loose.contains(&w.replace('h', ""));
        hit != negate || w.is_empty()
    })
}

pub fn samples() -> Vec<Entry> {
    SAMPLES
        .iter()
        .map(|&p| {
            let (dir, title) = split(p);
            Entry::new(Source::Sample(p), title, if dir == "samples" { "Samples" } else { dir })
        })
        .collect()
}

pub fn roots() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> =
        std::env::var_os("DJSUGAR_MUSIC").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if let Some(home) = std::env::var_os("HOME") {
        v.push(Path::new(&home).join("Music"));
    }
    if cfg!(target_os = "android") {
        // The phone's own storage and any card in it, wherever the music was put.
        v.push("/storage/emulated/0".into());
        let cards = std::fs::read_dir("/storage").into_iter().flatten().flatten().map(|e| e.path());
        v.extend(cards.filter(|p| !p.ends_with("emulated") && !p.ends_with("self")));
    }
    v.retain(|p| p.is_dir());
    v
}

/// A downloaded stream, named by the service's own title and artist.
pub fn file(path: PathBuf, title: &str, artist: &str, folder: &str) -> Entry {
    let name =
        if artist.is_empty() || title.contains(" - ") { title.to_string() } else { format!("{artist} - {title}") };
    Entry::new(Source::File(path.into()), &name, folder)
}

/// Folders a phone keeps sounds in that are not music, and other apps' own data.
const NOT_MUSIC: [&str; 6] = ["Android", "Ringtones", "Notifications", "Alarms", "Recordings", "Podcasts"];

pub fn scan(roots: &[PathBuf]) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = roots.to_vec();
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(kind) = e.file_type() else { continue };
            if kind.is_dir() {
                let name = e.file_name();
                let name = name.to_string_lossy();
                if !name.starts_with('.') && !NOT_MUSIC.contains(&&*name) {
                    stack.push(p);
                }
                continue;
            }
            let ext = p.extension().and_then(|x| x.to_str()).map(str::to_ascii_lowercase);
            if !ext.is_some_and(|x| AUDIO.contains(&x.as_str())) {
                continue;
            }
            let title = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let folder = dir.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            out.push(Entry::new(Source::File(p.into()), &title, &folder));
        }
    }
    out.sort_by(|a, b| a.folder.cmp(&b.folder).then_with(|| a.title.cmp(&b.title)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(title: &str, folder: &str) -> Entry {
        Entry::new(Source::Sample("x.ogg"), title, folder)
    }

    #[test]
    fn search_is_word_and_case_insensitive() {
        let t = e("Amen_Break", "Breaks");
        assert!(matches(&t, "amen"));
        assert!(matches(&t, "BREAK amen"));
        assert!(!matches(&t, "amen kick"));
        assert!(matches(&t, ""));
        assert!(!matches(&t, "-break"));
        assert!(matches(&t, "amen -kick"));
    }

    #[test]
    fn greek_titles_match_greeklish() {
        let t = e("Θέλω να σε δω", "Λαϊκά");
        assert!(matches(&t, "thelo"));
        assert!(matches(&t, "θέλω"));
        assert!(matches(&t, "telo"));
        assert!(matches(&t, "laika"));
    }

    #[test]
    fn scan_finds_audio_and_skips_hidden() {
        let root = std::env::temp_dir().join(format!("djsugar-scan-{}", std::process::id()));
        std::fs::create_dir_all(root.join("House/.cache")).unwrap();
        std::fs::create_dir_all(root.join("Ringtones")).unwrap();
        for f in ["House/a.mp3", "House/b.FLAC", "House/notes.txt", "House/.cache/c.mp3", "d.wav", "Ringtones/e.mp3"] {
            std::fs::write(root.join(f), b"").unwrap();
        }
        let found = scan(std::slice::from_ref(&root));
        let names: Vec<_> = found.iter().map(|e| &*e.title).collect();
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(names.len(), 3, "{names:?}");
        assert!(names.contains(&"b"));
    }
}

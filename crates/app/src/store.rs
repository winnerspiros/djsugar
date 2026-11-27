//! Per-track memory: beat grid, lock, hot cues and their colours, kept in one
//! tab separated file in the app data folder and rewritten on change.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static DIR: OnceLock<PathBuf> = OnceLock::new();

#[cfg_attr(not(target_os = "android"), allow(dead_code, reason = "Android passes its app folder"))]
pub fn set_dir(dir: PathBuf) {
    let _ = DIR.set(dir);
}

pub fn dir() -> Option<PathBuf> {
    if let Some(d) = DIR.get() {
        return Some(d.clone());
    }
    if let Some(d) = std::env::var_os("DJSUGAR_DATA") {
        return Some(d.into());
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".local/share")))?;
    Some(base.join("djsugar"))
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Saved {
    pub grid: Option<(f64, f64)>,
    pub locked: bool,
    pub cues: [Option<f64>; 8],
    pub colours: [u8; 8],
    /// Stars, 0 to 5.
    pub rating: u8,
    /// The track's colour, 1 to 8; 0 for none.
    pub colour: u8,
    /// Where the track is in preparation: 0 none, then Inbox, In Prep, Play Ready.
    pub stage: u8,
}

impl Saved {
    fn line(&self, key: &str) -> String {
        let grid = self.grid.map_or_else(|| "-\t-".to_string(), |(b, f)| format!("{b}\t{f}"));
        let cues: Vec<String> = self.cues.iter().map(|c| c.map_or_else(|| "-".into(), |v| format!("{v:.0}"))).collect();
        let colours: Vec<String> = self.colours.iter().map(u8::to_string).collect();
        format!(
            "{key}\t{grid}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            u8::from(self.locked),
            cues.join(","),
            colours.join(","),
            self.rating,
            self.colour,
            self.stage
        )
    }

    fn parse(line: &str) -> Option<(String, Self)> {
        let mut f = line.split('\t');
        let key = f.next()?.to_string();
        let bpm = f.next()?.parse().ok();
        let first = f.next()?.parse().ok();
        let locked = f.next()? == "1";
        let mut s = Self { grid: bpm.zip(first), locked, ..Self::default() };
        for (slot, v) in s.cues.iter_mut().zip(f.next()?.split(',')) {
            *slot = v.parse().ok();
        }
        for (slot, v) in s.colours.iter_mut().zip(f.next()?.split(',')) {
            *slot = v.parse().unwrap_or(0);
        }
        // Rating and colour came later; older lines have neither.
        s.rating = f.next().and_then(|v| v.parse().ok()).unwrap_or(0).min(5);
        s.colour = f.next().and_then(|v| v.parse().ok()).unwrap_or(0).min(8);
        s.stage = f.next().and_then(|v| v.parse().ok()).unwrap_or(0).min(3);
        Some((key, s))
    }
}

#[derive(Debug, Default)]
pub struct Store {
    tracks: HashMap<String, Saved>,
}

impl Store {
    pub fn open() -> Self {
        let text = dir().and_then(|d| std::fs::read_to_string(d.join("tracks.tsv")).ok()).unwrap_or_default();
        Self::parse(&text)
    }

    fn parse(text: &str) -> Self {
        Self { tracks: text.lines().filter_map(Saved::parse).collect() }
    }

    pub fn get(&self, key: &str) -> Option<Saved> {
        self.tracks.get(key).copied()
    }

    pub fn put(&mut self, key: &str, s: Saved) {
        if self.tracks.get(key) == Some(&s) {
            return;
        }
        self.tracks.insert(key.to_string(), s);
        let text = self.text();
        if let Some(d) = dir() {
            std::thread::spawn(move || {
                let _ = std::fs::create_dir_all(&d);
                let tmp = d.join("tracks.tsv.new");
                if std::fs::write(&tmp, text).is_ok() {
                    let _ = std::fs::rename(tmp, d.join("tracks.tsv"));
                }
            });
        }
    }

    fn text(&self) -> String {
        let mut keys: Vec<&String> = self.tracks.keys().collect();
        keys.sort();
        keys.into_iter().map(|k| self.tracks[k].line(k)).collect()
    }
}

/// The user's own pad sounds: every audio file in the `samples` folder of the app's files, by name.
pub fn sounds() -> Vec<(String, PathBuf)> {
    let Some(d) = dir() else { return Vec::new() };
    let mut found: Vec<_> = std::fs::read_dir(d.join("samples"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && djsugar_engine::scan::is_audio(p))
        .filter_map(|p| Some((p.file_stem()?.to_str()?.to_string(), p)))
        .collect();
    found.sort();
    found
}

/// What a pad's sample is written as: a built-in one its number, one of the user's (`built` and up) its file's name
/// after a `/`, which no number and no file name starts with.
fn pad_word(i: usize, built: usize, sounds: &[(String, PathBuf)]) -> String {
    let file = i.checked_sub(built).and_then(|n| sounds.get(n)).and_then(|(_, p)| p.file_name()?.to_str());
    match file {
        Some(f) if !f.contains([',', '\n']) => format!("/{f}"),
        Some(_) => "0".into(),
        None => i.min(built.saturating_sub(1)).to_string(),
    }
}

fn pad_of(word: &str, built: usize, sounds: &[(String, PathBuf)]) -> Option<usize> {
    match word.strip_prefix('/') {
        // A sound since taken away leaves the pad on the first sample.
        Some(file) => Some(
            sounds
                .iter()
                .position(|(_, p)| p.file_name().and_then(|f| f.to_str()) == Some(file))
                .map_or(0, |n| built + n),
        ),
        None => word.parse().ok().filter(|i| *i < built),
    }
}

/// Sample assigned to every pad of both decks, one line per deck.
pub fn save_pads<const N: usize>(pads: &[[usize; N]; 2], built: usize, sounds: &[(String, PathBuf)]) {
    let Some(d) = dir() else { return };
    let text: String = pads
        .iter()
        .map(|row| row.iter().map(|i| pad_word(*i, built, sounds)).collect::<Vec<_>>().join(",") + "\n")
        .collect();
    let _ = std::fs::create_dir_all(&d);
    let _ = std::fs::write(d.join("pads.txt"), text);
}

pub fn load_pads<const N: usize>(built: usize, sounds: &[(String, PathBuf)]) -> Option<[[usize; N]; 2]> {
    let text = std::fs::read_to_string(dir()?.join("pads.txt")).ok()?;
    let mut out = [[0; N]; 2];
    for (row, line) in out.iter_mut().zip(text.lines()) {
        for (slot, v) in row.iter_mut().zip(line.split(',')) {
            *slot = pad_of(v, built, sounds)?;
        }
    }
    Some(out)
}

/// Playlists, one per line: the name, then the tracks, tab separated.
pub fn save_playlists(lists: &[(String, Vec<String>)]) {
    save_lists("playlists.tsv", lists);
}

pub fn load_playlists() -> Vec<(String, Vec<String>)> {
    load_lists("playlists.tsv")
}

/// Automix's list, a track per line: its key, then its title and artist.
pub fn save_queue(queue: &[(String, Vec<String>)]) {
    save_lists("automix.tsv", queue);
}

pub fn load_queue() -> Vec<(String, Vec<String>)> {
    load_lists("automix.tsv")
}

/// The Sidelist, kept as Automix's list is.
pub fn save_sidelist(list: &[(String, Vec<String>)]) {
    save_lists("sidelist.tsv", list);
}

pub fn load_sidelist() -> Vec<(String, Vec<String>)> {
    load_lists("sidelist.tsv")
}

/// Tracks linked by hand: each track's key, then its partners'.
pub fn save_links(links: &[(String, Vec<String>)]) {
    save_lists("links.tsv", links);
}

pub fn load_links() -> Vec<(String, Vec<String>)> {
    load_lists("links.tsv")
}

/// Tracks played one after another, kept as links are.
pub fn save_played(played: &[(String, Vec<String>)]) {
    save_lists("played.tsv", played);
}

pub fn load_played() -> Vec<(String, Vec<String>)> {
    load_lists("played.tsv")
}

/// Tags, kept as playlists are: each a name and the tracks it is on.
pub fn save_tags(tags: &[(String, Vec<String>)]) {
    save_lists("tags.tsv", tags);
}

pub fn load_tags() -> Vec<(String, Vec<String>)> {
    load_lists("tags.tsv")
}

/// The keys moved from where they shipped: each the shipped key and the key its action is on now.
pub fn save_keys(binds: &[(char, char)]) {
    let lists: Vec<_> = binds.iter().map(|b| (b.0.to_string(), vec![b.1.to_string()])).collect();
    save_lists("keys.tsv", &lists);
}

pub fn load_keys() -> Vec<(char, char)> {
    let pair = |(a, b): (String, Vec<String>)| Some((a.chars().next()?, b.first()?.chars().next()?));
    load_lists("keys.tsv").into_iter().filter_map(pair).collect()
}

/// Tracks got from the streaming services, each a file with its title, artist and service.
pub fn save_streamed(tracks: &[(String, Vec<String>)]) {
    save_lists("streamed.tsv", tracks);
}

pub fn load_streamed() -> Vec<(String, Vec<String>)> {
    load_lists("streamed.tsv")
}

/// A playlist as M3U text: each track's artist and title, then where its file is.
fn m3u(tracks: &[(&str, &str, &Path)]) -> String {
    let mut text = String::from("#EXTM3U\n");
    for (artist, title, path) in tracks {
        let by = if artist.is_empty() { String::new() } else { format!("{artist} - ") };
        text += &format!("#EXTINF:-1,{by}{title}\n{}\n", path.display());
    }
    text
}

/// Writes a playlist into the `exports` folder of the app's files, named after it; the file written.
pub fn export(name: &str, tracks: &[(&str, &str, &Path)]) -> Option<PathBuf> {
    let d = dir()?.join("exports");
    std::fs::create_dir_all(&d).ok()?;
    let safe: String =
        name.chars().map(|c| if c.is_control() || "/\\:*?\"<>|".contains(c) { '_' } else { c }).collect();
    let file = d.join(format!("{}.m3u8", safe.trim()));
    std::fs::write(&file, m3u(tracks)).ok()?;
    Some(file)
}

fn save_lists(file: &str, lists: &[(String, Vec<String>)]) {
    let Some(d) = dir() else { return };
    let text: String = lists
        .iter()
        .map(|(n, keys)| {
            std::iter::once(n.as_str()).chain(keys.iter().map(String::as_str)).collect::<Vec<_>>().join("\t") + "\n"
        })
        .collect();
    let _ = std::fs::create_dir_all(&d);
    let _ = std::fs::write(d.join(file), text);
}

fn load_lists(file: &str) -> Vec<(String, Vec<String>)> {
    let text = dir().and_then(|d| std::fs::read_to_string(d.join(file)).ok()).unwrap_or_default();
    text.lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            Some((f.next()?.to_string(), f.map(str::to_string).collect()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_playlist_exports_as_m3u() {
        let tracks = [("Daft", "Around", Path::new("/m/a.flac")), ("", "Untitled", Path::new("/m/b c.mp3"))];
        assert_eq!(m3u(&tracks), "#EXTM3U\n#EXTINF:-1,Daft - Around\n/m/a.flac\n#EXTINF:-1,Untitled\n/m/b c.mp3\n");
    }

    #[test]
    fn pads_keep_their_sounds_by_name() {
        let sounds = |names: &[&str]| -> Vec<(String, PathBuf)> {
            names.iter().map(|n| (n.to_string(), PathBuf::from(format!("/s/{n}.wav")))).collect()
        };
        let had = sounds(&["horn", "siren"]);
        assert_eq!(pad_word(7, 10, &had), "7");
        assert_eq!(pad_word(11, 10, &had), "/siren.wav");
        // Another sound sorted in before it moves it one along; taken away, the pad falls back to the first sample.
        assert_eq!(pad_of("/siren.wav", 10, &sounds(&["air", "horn", "siren"])), Some(12));
        assert_eq!(pad_of("/siren.wav", 10, &sounds(&["horn"])), Some(0));
        assert_eq!(pad_of("7", 10, &had), Some(7));
        assert_eq!(pad_of("12", 10, &had), None);
    }

    #[test]
    fn round_trips() {
        let mut cues = [None; 8];
        cues[0] = Some(44100.0);
        cues[5] = Some(9.0);
        let s = Saved {
            grid: Some((124.5, 812.25)),
            locked: true,
            cues,
            colours: [1, 2, 3, 4, 5, 6, 7, 0],
            rating: 4,
            colour: 6,
            stage: 2,
        };
        let mut st = Store::default();
        st.tracks.insert("a/b c.flac".into(), s);
        let back = Store::parse(&st.text());
        assert_eq!(back.get("a/b c.flac"), Some(s));
        // A line saved before ratings reads with none.
        let old = "x\t-\t-\t0\t-,-,-,-,-,-,-,-\t0,0,0,0,0,0,0,0\n";
        assert_eq!(Store::parse(old).get("x"), Some(Saved::default()));
    }
}

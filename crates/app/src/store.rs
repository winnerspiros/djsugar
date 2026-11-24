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
}

impl Saved {
    fn line(&self, key: &str) -> String {
        let grid = self.grid.map_or_else(|| "-\t-".to_string(), |(b, f)| format!("{b}\t{f}"));
        let cues: Vec<String> = self.cues.iter().map(|c| c.map_or_else(|| "-".into(), |v| format!("{v:.0}"))).collect();
        let colours: Vec<String> = self.colours.iter().map(u8::to_string).collect();
        format!(
            "{key}\t{grid}\t{}\t{}\t{}\t{}\t{}\n",
            u8::from(self.locked),
            cues.join(","),
            colours.join(","),
            self.rating,
            self.colour
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

/// Sample assigned to every pad of both decks, one line per deck.
pub fn save_pads<const N: usize>(pads: &[[usize; N]; 2]) {
    let Some(d) = dir() else { return };
    let text: String =
        pads.iter().map(|row| row.iter().map(usize::to_string).collect::<Vec<_>>().join(",") + "\n").collect();
    let _ = std::fs::create_dir_all(&d);
    let _ = std::fs::write(d.join("pads.txt"), text);
}

pub fn load_pads<const N: usize>() -> Option<[[usize; N]; 2]> {
    let text = std::fs::read_to_string(dir()?.join("pads.txt")).ok()?;
    let mut out = [[0; N]; 2];
    for (row, line) in out.iter_mut().zip(text.lines()) {
        for (slot, v) in row.iter_mut().zip(line.split(',')) {
            *slot = v.parse().ok()?;
        }
    }
    Some(out)
}

/// Playlists, one per line: the name, then the tracks, tab separated.
pub fn save_playlists(lists: &[(String, Vec<String>)]) {
    let Some(d) = dir() else { return };
    let text: String = lists
        .iter()
        .map(|(n, keys)| {
            std::iter::once(n.as_str()).chain(keys.iter().map(String::as_str)).collect::<Vec<_>>().join("\t") + "\n"
        })
        .collect();
    let _ = std::fs::create_dir_all(&d);
    let _ = std::fs::write(d.join("playlists.tsv"), text);
}

pub fn load_playlists() -> Vec<(String, Vec<String>)> {
    let text = dir().and_then(|d| std::fs::read_to_string(d.join("playlists.tsv")).ok()).unwrap_or_default();
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

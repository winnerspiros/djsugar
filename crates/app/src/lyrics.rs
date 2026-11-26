use djsugar_engine::lyrics::{self as lrc, Line};

/// A track's lyrics: LRC, enhanced LRC timed by word, or plain text.
#[derive(Debug, Default)]
pub struct Lyrics {
    pub lines: Vec<Line>,
}

impl Lyrics {
    pub fn parse(text: &str) -> Self {
        Self { lines: lrc::parse(text).unwrap_or_default() }
    }

    /// The line sung at `seconds`.
    pub fn current(&self, seconds: f64) -> Option<usize> {
        lrc::line_at(&self.lines, seconds)
    }

    /// Words with no times at all, which [`time`] can give some.
    pub fn untimed(&self) -> bool {
        !self.lines.is_empty() && self.lines.iter().all(|l| l.start.is_none())
    }

    /// Line `n` split for karaoke at `seconds`.
    pub fn fill(&self, n: usize, seconds: f64) -> lrc::Fill {
        let until = self.lines.get(n + 1).and_then(|l| l.start);
        self.lines[n].fill_at(seconds, until)
    }
}

/// Lyrics found online: LRCLIB's, or NetEase's when LRCLIB has none or only untimed words and NetEase has them timed.
fn fetch(artist: &str, title: &str, seconds: f64) -> Option<String> {
    let first = sugar_net::lrclib::find(artist, title, seconds).ok().flatten();
    if first.as_ref().is_some_and(|t| t.contains('[')) {
        return first;
    }
    sugar_net::netease::find(artist, title, seconds).ok().flatten().or(first)
}

/// Lyrics for a file: its `.lrc` beside it, else what is found online, saved beside it for next time.
pub fn find(path: &std::path::Path, artist: &str, title: &str, seconds: f64, online: bool) -> Lyrics {
    let lrc = path.with_extension("lrc");
    if let Ok(t) = std::fs::read_to_string(&lrc) {
        return Lyrics::parse(&t);
    }
    if !online {
        return Lyrics::default();
    }
    match fetch(artist, title, seconds) {
        Some(t) => {
            let _ = std::fs::write(&lrc, &t);
            Lyrics::parse(&t)
        }
        _ => Lyrics::default(),
    }
}

/// The vocal level, of 255, from which a moment counts as sung.
const SUNG: u8 = 128;

/// Gives untimed lines times from where the track is sung: `levels` are its vocal levels, `rate` a second. The lines are laid
/// over the sung stretches in order, each taking a share of them by its length. False, with nothing changed, when nothing is sung.
pub fn spread(lines: &mut [Line], levels: &[u8], rate: f64) -> bool {
    let sung: Vec<usize> = levels.iter().enumerate().filter(|(_, v)| **v >= SUNG).map(|(i, _)| i).collect();
    let all: usize = lines.iter().map(|l| l.text.chars().count() + 1).sum();
    if sung.is_empty() || all == 0 || rate <= 0.0 {
        return false;
    }
    let mut before = 0;
    for l in lines {
        l.start = Some(sung[before * sung.len() / all] as f64 / rate);
        before += l.text.chars().count() + 1;
    }
    true
}

/// Times a file's untimed lyrics by listening for its vocals, and keeps them beside it as an `.lrc`.
pub fn time(path: &std::path::Path, mut lyrics: Lyrics) -> Result<Lyrics, String> {
    let levels = djsugar_engine::vocals::analyse(path, None, || true)?;
    if !spread(&mut lyrics.lines, &levels, djsugar_engine::vocals::RATE) {
        return Err("No vocals were heard to time the lyrics by".into());
    }
    let _ = std::fs::write(path.with_extension("lrc"), lrc::to_lrc(&lyrics.lines));
    Ok(lyrics)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run with `DJSUGAR_TRACK` naming a sung track that has untimed words in the `.lrc` beside it.
    #[test]
    #[ignore = "network, a model download and a track of one's own"]
    fn times_a_real_track() {
        let Some(path) = std::env::var_os("DJSUGAR_TRACK").map(std::path::PathBuf::from) else { return };
        crate::models::install();
        let l = find(&path, "", "", 0.0, false);
        assert!(l.untimed());
        let l = time(&path, l).unwrap();
        let at: Vec<f64> = l.lines.iter().map(|l| l.start.unwrap()).collect();
        assert!(at.is_sorted() && at[0] < at[at.len() - 1], "{at:?}");
        println!("{at:?}");
    }

    #[test]
    fn untimed_lines_follow_the_singing() {
        let mut l = Lyrics::parse("one\ntwo\nsix");
        assert!(l.untimed());
        // Sung from 2 s to 5 s and from 10 s to 13 s, at one level a second.
        let levels: Vec<u8> =
            (0..20).map(|i| if (2..5).contains(&i) || (10..13).contains(&i) { 255 } else { 0 }).collect();
        assert!(spread(&mut l.lines, &levels, 1.0));
        let at: Vec<f64> = l.lines.iter().map(|l| l.start.unwrap()).collect();
        assert_eq!(at, [2.0, 4.0, 11.0]);
        assert!(!l.untimed());
        assert!(!spread(&mut l.lines, &[0; 20], 1.0));
    }

    #[test]
    fn parses_repeats_offset_and_finds_line() {
        let l = Lyrics::parse("[ar:Someone]\n[offset:500]\n[00:01.50][00:10.00]Chorus\n[00:05.25]Verse");
        assert_eq!(l.lines.len(), 3);
        assert_eq!(l.lines[0].text, "Chorus");
        assert!((l.lines[0].start.unwrap() - 1.0).abs() < 1e-9);
        assert_eq!(l.current(0.5), None);
        assert_eq!(l.current(5.0), Some(1));
        assert_eq!(l.current(60.0), Some(2));
    }

    #[test]
    fn word_fill() {
        let l = Lyrics::parse("[00:10.00]<00:10.00>Hold <00:10.40>the <00:10.70>line<00:11.50>");
        let f = l.fill(0, 10.5);
        assert_eq!((f.sung.as_str(), f.word.as_str()), ("Hold ", "the"));
    }
}

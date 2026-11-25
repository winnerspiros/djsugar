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

    /// Line `n` split for karaoke at `seconds`.
    pub fn fill(&self, n: usize, seconds: f64) -> lrc::Fill {
        let until = self.lines.get(n + 1).and_then(|l| l.start);
        self.lines[n].fill_at(seconds, until)
    }
}

/// Lyrics for a file: its `.lrc` beside it, else LRCLIB's, saved beside it for next time.
pub fn find(path: &std::path::Path, artist: &str, title: &str, seconds: f64, online: bool) -> Lyrics {
    let lrc = path.with_extension("lrc");
    if let Ok(t) = std::fs::read_to_string(&lrc) {
        return Lyrics::parse(&t);
    }
    if !online {
        return Lyrics::default();
    }
    match sugar_net::lrclib::find(artist, title, seconds) {
        Ok(Some(t)) => {
            let _ = std::fs::write(&lrc, &t);
            Lyrics::parse(&t)
        }
        _ => Lyrics::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

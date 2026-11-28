use djsugar_engine::lyrics::{self as lrc, Line};
use djsugar_engine::word_timing::CHECKED;

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

/// Words in a title's brackets that say which version of the song it is, and so stay.
const VERSIONS: [&str; 8] = ["remix", "mix", "edit", "bootleg", "rework", "flip", "dub", "version"];

/// A title without what a video adds to it: brackets that are not a version, guests, and what follows a bar.
fn bare(title: &str) -> String {
    let mut out = String::new();
    let mut depth = 0u32;
    let mut held = String::new();
    for c in title.chars() {
        match c {
            '(' | '[' | '\u{3010}' => depth += 1,
            ')' | ']' | '\u{3011}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    let low = held.to_lowercase();
                    if VERSIONS.iter().any(|v| low.split(|c: char| !c.is_alphanumeric()).any(|w| w == *v)) {
                        out.push_str(&format!("({held})"));
                    }
                    held.clear();
                }
            }
            c if depth > 0 => held.push(c),
            c => out.push(c),
        }
    }
    let low = out.to_lowercase();
    let said = [
        " ft.",
        " ft ",
        " feat.",
        " feat ",
        " featuring ",
        " | ",
        " // ",
        " - official",
        " - music video",
        " - lyric video",
        " - lyrics",
    ];
    let cut = said.iter().filter_map(|m| low.find(m)).min();
    // The lower-case copy can be longer or shorter in bytes; only cut where both agree.
    let out = match cut {
        Some(n) if low.len() == out.len() => &out[..n],
        _ => &out,
    };
    out.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c: char| c == '-' || c.is_whitespace())
        .to_string()
}

/// An artist without what a channel adds to the name.
fn act(artist: &str) -> String {
    let mut a = artist.trim();
    for tail in [" - Topic", "VEVO", "Vevo", " Official", " Music", " TV"] {
        a = a.strip_suffix(tail).unwrap_or(a).trim();
    }
    a.to_string()
}

/// The names to ask the services for, likeliest first: a video called "Artist - Song (Official Video)" on a channel
/// of another name is asked for as that artist's song before it is asked for as the channel's.
pub fn names(artist: &str, title: &str) -> Vec<(String, String)> {
    let (a, t) = (act(artist), bare(title));
    let mut out = Vec::new();
    for dash in [" - ", " \u{2013} ", " \u{2014} ", ": "] {
        if let Some((left, right)) = t.split_once(dash) {
            let right = right.trim_matches(|c: char| c == '"' || c == '\'' || c.is_whitespace());
            out.push((act(left), right.to_string()));
            break;
        }
    }
    out.push((a.clone(), t.clone()));
    out.push((artist.trim().to_string(), title.trim().to_string()));
    out.push((String::new(), t));
    let mut seen: Vec<(String, String)> = Vec::new();
    out.retain(|n| {
        let low = (n.0.to_lowercase(), n.1.to_lowercase());
        !n.1.is_empty() && !seen.contains(&low) && {
            seen.push(low);
            true
        }
    });
    out
}

/// One service's lookup, by artist, title and length.
type Finder = fn(&str, &str, f64) -> sugar_net::Result<Option<String>>;

fn timed(text: &Option<String>) -> bool {
    text.as_ref().is_some_and(|t| t.contains('['))
}

/// The YouTube video a downloaded file is of, from its name.
pub fn video(path: &std::path::Path) -> Option<&str> {
    let stem = path.file_stem()?.to_str()?;
    let made = stem.len() == 11 && stem.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    (made && matches!(path.extension()?.to_str()?, "m4a" | "webm" | "aac")).then_some(stem)
}

/// Lyrics found online, timed ones wanted most. A downloaded video goes by its YouTube Music name first; LRCLIB,
/// NetEase and a signed-in Spotify are asked for timed lines. Plain words are the last resort: YouTube Music's,
/// those services', lyrics.ovh's, or what is written under the video.
fn fetch(video: Option<&str>, artist: &str, title: &str, seconds: f64) -> Option<String> {
    let mut names = names(artist, title);
    let song = video.and_then(|v| sugar_net::ytmusic::song_of(v).ok()).unwrap_or_default();
    if !song.title.is_empty() && !song.artist.is_empty() {
        crate::log::note(&format!("lyrics: the video is \"{}\" by \"{}\"", song.title, song.artist));
        let known = self::names(&song.artist, &song.title);
        names.retain(|n| !known.contains(n));
        names.splice(0..0, known);
    }
    let mut plain: Option<String> = song.words;
    let mut asked = 0;
    for length in [seconds, 0.0] {
        for (a, t) in &names {
            let from: [(&str, Finder); 2] =
                [("lrclib", sugar_net::lrclib::find), ("netease", sugar_net::netease::find)];
            for (who, find) in from {
                // NetEase takes its first hit when no length is given, which is too loose to trust.
                if length <= 0.0 && (who == "netease" || seconds <= 0.0) {
                    continue;
                }
                asked += 1;
                let got = find(a, t, length);
                if let Err(e) = &got {
                    crate::log::note(&format!("lyrics {who} \"{a}\" \"{t}\": {e}"));
                }
                let got = got.ok().flatten();
                if timed(&got) {
                    crate::log::note(&format!("lyrics timed from {who} as \"{a}\" \"{t}\" after {asked} asks"));
                    return got;
                }
                plain = plain.or(got);
            }
        }
    }
    if let Some((a, t)) = names.first() {
        let got = crate::spotify::words(a, t, seconds);
        if timed(&got) {
            crate::log::note(&format!("lyrics timed from spotify as \"{a}\" \"{t}\""));
            return got;
        }
        plain = plain.or(got);
    }
    let under = video.and_then(|v| sugar_net::said::find(v).ok().flatten());
    if timed(&under) {
        crate::log::note("lyrics timed from what is written under the video");
        return under;
    }
    if plain.is_none() {
        plain = names.iter().find_map(|(a, t)| sugar_net::lyricsovh::find(a, t).ok().flatten()).or(under);
    }
    let what = if plain.is_some() { "plain words only" } else { "nothing" };
    crate::log::note(&format!("lyrics for \"{artist}\" \"{title}\": {what} after {asked} asks"));
    plain
}

/// Lyrics for a file: its `.lrc` beside it, else what is found online, saved beside it for next time. Words kept
/// without times are asked for again, in case somebody has timed them since.
pub fn find(path: &std::path::Path, artist: &str, title: &str, seconds: f64, online: bool) -> Lyrics {
    let lrc = path.with_extension("lrc");
    let kept = std::fs::read_to_string(&lrc).ok();
    // Words this app wrote down by ear give way to a person's, once those are found with times.
    let ours = kept.as_deref().is_some_and(lrc::is_auto);
    if !online || (timed(&kept) && !ours) {
        return kept.map(|t| Lyrics::parse(&t)).unwrap_or_default();
    }
    let found = fetch(video(path), artist, title, seconds);
    if found.is_some() && (kept.is_none() || timed(&found)) {
        let _ = std::fs::write(&lrc, found.as_deref().unwrap_or_default());
        return found.map(|t| Lyrics::parse(&t)).unwrap_or_default();
    }
    kept.map(|t| Lyrics::parse(&t)).unwrap_or_default()
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

/// Where listening to a track says how far along it is.
pub type Said = std::sync::Arc<dyn Fn(String) + Send + Sync>;

fn report(said: Said) -> djsugar_engine::word_timing::Report {
    use djsugar_engine::word_timing::Stage;
    std::sync::Arc::new(move |stage| {
        said(match stage {
            Stage::Downloading => "Getting ready to listen".to_owned(),
            Stage::Listening(part) => format!("Listening {part}%"),
        });
    })
}

/// The file each deck holds: only those are worth listening to.
static WANTED: std::sync::Mutex<Vec<(usize, std::path::PathBuf)>> = std::sync::Mutex::new(Vec::new());

/// Notes that `deck` now holds the file at `path`.
pub fn want(deck: usize, path: &std::path::Path) {
    let mut wanted = WANTED.lock().unwrap_or_else(|e| e.into_inner());
    wanted.retain(|(d, _)| *d != deck);
    wanted.push((deck, path.to_owned()));
}

/// Gives a file's lyrics what they lack by listening to it, one track at a time: times for words that have none,
/// or the words themselves. None when the track has left its deck meanwhile, or nothing sung was made out.
pub fn by_ear(path: &std::path::Path, lyrics: Lyrics, said: Said) -> Option<Lyrics> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    if !WANTED.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|(_, p)| p == path) {
        return None;
    }
    let heard = if lyrics.lines.is_empty() {
        write(path, said).map(Some)
    } else if lyrics.untimed() {
        time(path, lyrics, said).map(Some)
    } else if unchecked(path) {
        check(path, &lyrics, said)
    } else {
        Ok(None)
    };
    heard.map_err(|why| crate::log::note(&format!("lyrics not heard: {why}"))).ok().flatten()
}

/// Whether the timed lyrics kept beside the file at `path` have yet to be held against it: those found online
/// or put there by hand. What this app heard itself, it need not check.
pub fn unchecked(path: &std::path::Path) -> bool {
    std::fs::read_to_string(path.with_extension("lrc")).is_ok_and(|kept| {
        timed(&Some(kept.clone())) && !lrc::is_auto(&kept) && !kept.lines().take(20).any(|l| l.trim() == CHECKED)
    })
}

/// Holds a file's timed lyrics against the singing, once: Some, with every word where it is heard, when they
/// were off. The `.lrc` is marked either way, so the track is not listened to again.
fn check(path: &std::path::Path, lyrics: &Lyrics, said: Said) -> Result<Option<Lyrics>, String> {
    let began = std::time::Instant::now();
    // Lyrics timed to the song are late in its video by what the video adds; viewers may have marked that.
    let parts = crate::skips::marked(path);
    let on = |at: f64| crate::skips::within(&parts, at);
    let moved: Vec<Line> = lyrics
        .lines
        .iter()
        .map(|l| Line {
            start: l.start.map(on),
            end: l.end.map(on),
            words: l.words.iter().map(|w| lrc::Word { start: on(w.start), text: w.text.clone() }).collect(),
            text: l.text.clone(),
        })
        .collect();
    let moved = (!parts.is_empty()).then_some(&moved[..]);
    let mended = djsugar_engine::word_timing::check_words(path, &lyrics.lines, moved, report(said))?;
    let what = if mended.is_some() { "were off and are set by ear" } else { "are left as they came" };
    crate::log::note(&format!("lyrics checked in {:.0} s: the times {what}", began.elapsed().as_secs_f64()));
    let lrc = path.with_extension("lrc");
    let kept = match &mended {
        Some(lines) => lrc::to_lrc(lines),
        None => std::fs::read_to_string(&lrc).unwrap_or_default(),
    };
    let _ = std::fs::write(lrc, format!("{CHECKED}\n{kept}"));
    Ok(mended.map(|lines| Lyrics { lines }))
}

/// Times a file's untimed lyrics and keeps them beside it as an `.lrc`: each word where it is heard sung, or, when
/// the words can't be made out, the lines spread over the sung stretches.
pub fn time(path: &std::path::Path, mut lyrics: Lyrics, said: Said) -> Result<Lyrics, String> {
    let began = std::time::Instant::now();
    match djsugar_engine::word_timing::time_words(path, &lyrics.lines, report(said)) {
        Ok(lines) => lyrics.lines = lines,
        Err(why) => {
            crate::log::note(&format!("lyrics not timed by word: {why}"));
            let levels = djsugar_engine::vocals::analyse(path, None, || true)?;
            if !spread(&mut lyrics.lines, &levels, djsugar_engine::vocals::RATE) {
                return Err("No vocals were heard to time the lyrics by".into());
            }
        }
    }
    crate::log::note(&format!("lyrics timed in {:.0} s", began.elapsed().as_secs_f64()));
    let _ = std::fs::write(path.with_extension("lrc"), format!("{CHECKED}\n{}", lrc::to_lrc(&lyrics.lines)));
    Ok(lyrics)
}

/// Writes down the words sung in a file that has no lyrics, timed by word, and keeps them beside it as an `.lrc`
/// marked as written by ear.
pub fn write(path: &std::path::Path, said: Said) -> Result<Lyrics, String> {
    let began = std::time::Instant::now();
    let lines = djsugar_engine::word_timing::transcribe(path, report(said))?;
    crate::log::note(&format!("{} lines written by ear in {:.0} s", lines.len(), began.elapsed().as_secs_f64()));
    let _ = std::fs::write(path.with_extension("lrc"), format!("{}\n{}", lrc::AUTO, lrc::to_lrc(&lines)));
    Ok(Lyrics { lines })
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
        let l = time(&path, l, std::sync::Arc::new(|now| println!("{now}"))).unwrap();
        let at: Vec<f64> = l.lines.iter().map(|l| l.start.unwrap()).collect();
        assert!(at.is_sorted() && at[0] < at[at.len() - 1], "{at:?}");
        println!("{}", lrc::to_lrc(&l.lines));
    }

    /// Run with `DJSUGAR_TRACK` naming a sung track.
    #[test]
    #[ignore = "network, a model download and a track of one's own"]
    fn writes_a_real_track_down() {
        let Some(path) = std::env::var_os("DJSUGAR_TRACK").map(std::path::PathBuf::from) else { return };
        crate::models::install();
        let l = write(&path, std::sync::Arc::new(|_| ())).unwrap();
        println!("{}", lrc::to_lrc(&l.lines));
        assert!(l.lines.len() > 4 && !l.untimed());
    }

    #[test]
    fn video_names_become_song_names() {
        let greek = names("Katerina Lioliou", "Κατερίνα Λιόλιου - Λογαριασμός - Official Music Video");
        assert_eq!(greek[0], ("Κατερίνα Λιόλιου".to_string(), "Λογαριασμός".to_string()));
        let n = names("DuaLipaVEVO", "Dua Lipa - Levitating (Official Video) [4K]");
        assert_eq!(n[0], ("Dua Lipa".to_string(), "Levitating".to_string()));
        assert!(n.contains(&(String::new(), "Dua Lipa - Levitating".to_string())));
        let n = names("Daft Punk - Topic", "One More Time (Club Mix) ft. Romanthony");
        assert_eq!(n[0], ("Daft Punk".to_string(), "One More Time (Club Mix)".to_string()));
        let n = names("A", "Song");
        assert_eq!(n, [("A".to_string(), "Song".to_string()), (String::new(), "Song".to_string())]);
    }

    #[test]
    #[ignore = "network"]
    fn finds_timed_words_for_a_video_name() {
        let l = fetch(None, "DuaLipaVEVO", "Dua Lipa - Levitating (Official Video)", 203.0).unwrap();
        assert!(l.contains("[0"), "{l}");
    }

    #[test]
    fn downloaded_videos_are_known_by_their_names() {
        assert_eq!(video(std::path::Path::new("/c/TUVcZfQe-Kw.m4a")), Some("TUVcZfQe-Kw"));
        assert_eq!(video(std::path::Path::new("/c/fJ9rUzIMcZQ.webm")), Some("fJ9rUzIMcZQ"));
        assert_eq!(video(std::path::Path::new("/music/Some song.m4a")), None);
        assert_eq!(video(std::path::Path::new("/music/elevenchars.mp3")), None);
    }

    #[test]
    #[ignore = "network"]
    fn a_video_with_an_odd_name_is_found_by_what_it_is() {
        let l = fetch(Some("TUVcZfQe-Kw"), "", "dl lev mv", 231.0).unwrap();
        assert!(l.contains("[0"), "{l}");
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

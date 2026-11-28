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
    let mut words: Vec<&str> = out.split_whitespace().collect();
    // What a video says of itself at the end of its name, with no brackets or dash to tell it by.
    let tail = words.iter().rev().take_while(|w| SAID.contains(&w.to_lowercase().as_str())).count();
    let tells = |w: &&str| !MAYBE.contains(&w.to_lowercase().as_str());
    if tail < words.len() && words[words.len() - tail..].iter().any(tells) {
        words.truncate(words.len() - tail);
    }
    words.join(" ").trim_matches(|c: char| c == '-' || c.is_whitespace()).to_string()
}

/// Words a video ends its name with that are no part of the song's; [`MAYBE`] those a song may end with too,
/// which go only along with one of the others.
const SAID: [&str; 18] = [
    "official",
    "music",
    "video",
    "audio",
    "new",
    "clip",
    "lyric",
    "lyrics",
    "hd",
    "hq",
    "4k",
    "videoclip",
    "visualizer",
    "visualiser",
    "m/v",
    "mv",
    "επίσημο",
    "στίχοι",
];
const MAYBE: [&str; 5] = ["music", "video", "audio", "new", "clip"];

/// The words of lyrics, as told apart when two sets of them are compared: without times, marks or accents,
/// Greek in Latin letters.
fn worded_as(text: &str) -> std::collections::HashSet<String> {
    let mut bare = String::with_capacity(text.len());
    let mut depth = 0u32;
    for c in text.chars() {
        match c {
            '[' | '<' => depth += 1,
            ']' | '>' => depth = depth.saturating_sub(1),
            c if depth == 0 => bare.push(if c.is_alphanumeric() { c } else { ' ' }),
            _ => {}
        }
    }
    crate::library::fold(&bare).split_whitespace().filter(|w| w.chars().count() > 2).map(str::to_owned).collect()
}

/// How far two sets of lyrics are of the same song, of 1: the share of the shorter's words the longer has.
/// Two songs in one language share the small words, so only well over half says they are one.
fn alike(a: &str, b: &str) -> f64 {
    let (a, b) = (worded_as(a), worded_as(b));
    let least = a.len().min(b.len());
    if least < 8 {
        return 0.0;
    }
    a.intersection(&b).count() as f64 / least as f64
}

/// The writing most of a text's letters are in: 0 for Latin and whatever is not told apart.
fn writing(text: &str) -> u8 {
    let mut seen = [0usize; 5];
    for c in text.chars().filter(|c| c.is_alphabetic()) {
        seen[match c as u32 {
            0x370..=0x3ff | 0x1f00..=0x1fff => 1,
            0x400..=0x52f => 2,
            0x3040..=0x30ff | 0x3400..=0x9fff => 3,
            0xac00..=0xd7af => 4,
            _ => 0,
        }] += 1;
    }
    (0..5).max_by_key(|&i| (seen[i], 5 - i)).unwrap_or(0) as u8
}

/// Whether lyrics may be those of a song so named: a name not in Latin letters is of a song in its own.
fn spoken(name: &str, text: &str) -> bool {
    let (named, sung) = (writing(name), writing(&text.replace(['[', '<'], "\n")));
    named == 0 || named == sung
}

/// The services that take the hit of the right length whatever it is called: what they give is unsure
/// until something bears it out.
const LOOSE: [&str; 3] = ["netease", "kugou", "qq"];

/// From how alike two sets of lyrics are one song's.
const SAME: f64 = 0.6;

/// The tag of lyrics found by a title alone, with nothing to bear them out: listening may show them another
/// song's. And what follows a file's name where such were shown to be, so that they are not taken again.
const UNSURE: &str = "[re:DJ Sugar unsure]";
const REFUSED: &str = "lrc.not";

/// An artist without what a channel adds to the name.
fn act(artist: &str) -> String {
    let mut a = artist.trim();
    for tail in [" - Topic", "VEVO", "Vevo", " Official", " Music", " TV"] {
        a = a.strip_suffix(tail).unwrap_or(a).trim();
    }
    a.to_string()
}

/// A Greek name as it is written in Latin letters.
fn latin(name: &str) -> String {
    let mut low = name.to_lowercase();
    for (greek, latin) in [("ου", "ou"), ("ού", "ou"), ("ευ", "ev"), ("εύ", "ev"), ("αυ", "av"), ("αύ", "av")]
    {
        low = low.replace(greek, latin);
    }
    crate::library::fold(&low)
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
    // Greek names are kept in Latin letters by some.
    let latin = (latin(&out[0].0), latin(&out[0].1));
    if latin.1.is_ascii() && !out[0].1.is_ascii() {
        out.push(latin);
    }
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

/// Whether lyrics have a time ahead of each word.
fn worded(text: &Option<String>) -> bool {
    text.as_deref().is_some_and(|t| t.lines().filter(|l| l.contains("]<")).count() * 2 > t.lines().count())
}

/// One site's page of plain words, by artist and title.
type Page = fn(&str, &str) -> sugar_net::Result<Option<String>>;

fn timed(text: &Option<String>) -> bool {
    text.as_ref().is_some_and(|t| t.contains('['))
}

/// The YouTube video a downloaded file is of, from its name.
pub fn video(path: &std::path::Path) -> Option<&str> {
    let stem = path.file_stem()?.to_str()?;
    let made = stem.len() == 11 && stem.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    (made && matches!(path.extension()?.to_str()?, "m4a" | "webm" | "aac")).then_some(stem)
}

/// What was found online for a track.
#[derive(Default)]
struct Found {
    text: Option<String>,
    /// Found by a title alone, and borne out by nothing else.
    unsure: bool,
    /// What YouTube Music calls the song a video is of.
    song: Option<(String, String)>,
}

/// How long the services are waited for, at each round of asking: one that hangs is left behind.
const WAIT: std::time::Duration = std::time::Duration::from_secs(7);

/// Something to be asked, on a thread of its own.
type Job<T> = Box<dyn FnOnce() -> Option<T> + Send>;

/// Runs every one of `jobs` at once, and gives what each came back with inside [`WAIT`], in their order.
fn at_once<T: Send + 'static>(jobs: Vec<Job<T>>) -> Vec<Option<T>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut out: Vec<Option<T>> = jobs.iter().map(|_| None).collect();
    for (i, job) in jobs.into_iter().enumerate() {
        let tx = tx.clone();
        std::thread::spawn(move || tx.send((i, job())));
    }
    drop(tx);
    let until = std::time::Instant::now() + WAIT;
    while let Ok((i, got)) = rx.recv_timeout(until.saturating_duration_since(std::time::Instant::now())) {
        out[i] = got;
    }
    out
}

/// Asks each of `from` for lyrics by each of `names`, all at once: the answers, with which name and who.
fn ask(from: &[(&'static str, Finder)], names: &[(String, String)], length: f64) -> Vec<(usize, &'static str, String)> {
    let mut jobs: Vec<Job<(usize, &'static str, String)>> = Vec::new();
    for (n, (a, t)) in names.iter().enumerate() {
        for &(who, find) in from {
            let (a, t) = (a.clone(), t.clone());
            jobs.push(Box::new(move || match find(&a, &t, length) {
                Ok(got) => Some((n, who, got?)),
                Err(e) => {
                    crate::log::note(&format!("lyrics {who} \"{a}\" \"{t}\": {e}"));
                    None
                }
            }));
        }
    }
    at_once(jobs).into_iter().flatten().collect()
}

/// How many other uploads of a song are read for its lyrics, and how far from the track's length, in
/// seconds, one may be and still be the same recording.
const UPLOADS: usize = 3;
const SAME_LENGTH: u32 = 2;

/// What is written under a video and in its captions, and under and in the other uploads of the song: timed
/// lines where anybody has them, else the plain words under the video itself.
fn written(video: Option<&str>, name: Option<&(String, String)>, seconds: f64) -> Option<String> {
    static YOUTUBE: std::sync::OnceLock<sugar_net::youtube::YouTube> = std::sync::OnceLock::new();
    let youtube = || YOUTUBE.get_or_init(|| sugar_net::youtube::YouTube::new("US", ""));
    let mut videos: Vec<String> = video.iter().map(|v| v.to_string()).collect();
    let long = seconds.round() as u32;
    if let Some((a, t)) = name.filter(|_| long > 0)
        && let Ok(page) = youtube().search(&format!("{a} {t}"), None)
    {
        // Another upload of the same length is the same recording, and what is timed for it fits this one.
        let same = page.hits.into_iter().filter(|h| h.seconds.abs_diff(long) <= SAME_LENGTH);
        let others: Vec<String> = same.map(|h| h.id).filter(|id| !videos.contains(id)).take(UPLOADS).collect();
        videos.extend(others);
    }
    let mut jobs: Vec<Job<String>> = Vec::new();
    for v in &videos {
        let (under, shown) = (v.clone(), v.clone());
        jobs.push(Box::new(move || sugar_net::said::find(&under).ok().flatten()));
        jobs.push(Box::new(move || youtube().captions(&shown).ok().flatten()));
    }
    let got = at_once(jobs);
    let timed = got.iter().flatten().find(|text| text.starts_with('['));
    // Plain words under another upload may be of another version; only the video's own are taken.
    timed.or(got.first().and_then(Option::as_ref).filter(|_| video.is_some())).cloned()
}

/// Lyrics found online, those timed word by word wanted most, then timed lines. A downloaded video goes by its
/// YouTube Music name first, and its words there are what the others' are held against; those like `refused`
/// are passed over. Plain words are the last resort.
fn fetch(video: Option<&str>, artist: &str, title: &str, seconds: f64, refused: Option<&str>) -> Found {
    let began = std::time::Instant::now();
    let mut names = names(artist, title);
    let song = video.and_then(|v| sugar_net::ytmusic::song_of(v).ok()).unwrap_or_default();
    let mut found = Found::default();
    if !song.title.is_empty() && !song.artist.is_empty() {
        crate::log::note(&format!("lyrics: the video is \"{}\" by \"{}\"", song.title, song.artist));
        let known = self::names(&song.artist, &song.title);
        found.song = known.first().cloned();
        names.retain(|n| !known.contains(n));
        // The video's own name goes first where it names the artist: YouTube Music may have a channel's.
        let at = usize::from(names.first().is_some_and(|n| !n.0.is_empty() && bare(title).contains(" - ")));
        names.splice(at..at, known);
        names.sort_by_key(|n| n.0.is_empty());
    }
    let sure = song.words;
    let fits = |who: &str, text: &str| {
        let other = sure.as_deref().is_some_and(|sure| alike(text, sure) < SAME && worded_as(sure).len() >= 8);
        let again = refused.is_some_and(|r| alike(text, r) >= SAME);
        let foreign = !spoken(title, text);
        if other || again || foreign {
            crate::log::note(&format!("lyrics from {who} are another song's"));
        }
        !(other || again || foreign)
    };
    let mut plain: Option<String> = sure.clone();
    // Everything any service has said, for telling whether two of them agree.
    let mut seen: Vec<String> = Vec::new();
    let mut asked = 0;
    let all: [(&str, Finder); 6] = [
        ("lrclib", sugar_net::lrclib::find),
        ("netease", sugar_net::netease::find),
        ("kugou", sugar_net::kugou::find),
        ("qq", sugar_net::qq::find),
        ("lrc.red", sugar_net::lrcred::find),
        ("deezer", sugar_net::deezer::find),
    ];
    // The likeliest name alone, as it mostly is enough; then the others together; then all with no length
    // given, of the one service that still goes by the name then. The others take their first hit.
    let most = names.len().min(1);
    let rounds =
        [(&names[..most], seconds, &all[..]), (&names[most..], seconds, &all[..]), (&names[..], 0.0, &all[..1])];
    for (round, length, from) in rounds {
        // With no length to go by at all, nobody's first hit is to be trusted.
        if round.is_empty() || seconds <= 0.0 {
            continue;
        }
        asked += round.len() * from.len();
        let mut got = ask(from, round, length);
        got.retain(|(_, who, text)| fits(who, text));
        let agreed = |text: &str| {
            let all = got.iter().map(|g| &g.2).chain(seen.iter());
            all.filter(|other| alike(text, other) >= SAME).count()
        };
        // The likelier name first; then words a person timed one by one over lines; then those more agree with.
        let rank = |g: &&(usize, &str, String)| (std::cmp::Reverse(g.0), worded(&Some(g.2.clone())), agreed(&g.2));
        if let Some((n, who, text)) = got.iter().filter(|g| g.2.contains('[')).max_by_key(rank) {
            let (a, t) = &round[*n];
            // Itself is one of those it agrees with.
            found.unsure = (a.is_empty() || LOOSE.contains(who)) && sure.is_none() && agreed(text) < 2;
            let how = if worded(&Some(text.clone())) { "timed by word" } else { "timed" };
            let doubt = if found.unsure { ", unsure" } else { "" };
            let took = began.elapsed().as_millis();
            crate::log::note(&format!(
                "lyrics {how} from {who} as \"{a}\" \"{t}\" after {asked} asks, {took} ms{doubt}"
            ));
            found.text = Some(text.clone());
            return found;
        }
        plain = plain.or_else(|| got.first().map(|g| g.2.clone()));
        seen.extend(got.into_iter().map(|g| g.2));
    }
    if let Some((a, t)) = names.first() {
        let got = crate::spotify::words(a, t, seconds).filter(|text| fits("spotify", text));
        if timed(&got) {
            crate::log::note(&format!("lyrics timed from spotify as \"{a}\" \"{t}\""));
            found.text = got;
            return found;
        }
        plain = plain.or(got);
    }
    let under = written(video, names.first().filter(|n| !n.0.is_empty()), seconds).filter(|text| fits("YouTube", text));
    if timed(&under) {
        crate::log::note("lyrics timed from what is written under an upload of the song, or in its captions");
        found.text = under;
        return found;
    }
    if plain.is_none() {
        let pages: [(&'static str, Page); 8] = [
            ("genius", sugar_net::genius::find),
            ("lyrics.ovh", sugar_net::lyricsovh::find),
            ("azlyrics", sugar_net::pages::azlyrics),
            ("lyricstranslate", sugar_net::pages::lyricstranslate),
            ("stixoi.info", sugar_net::pages::stixoi),
            ("greeklyrics", sugar_net::pages::greeklyrics),
            ("hiphoplyrics", sugar_net::pages::hiphoplyrics),
            ("j-lyric", sugar_net::pages::jlyric),
        ];
        // Each site is asked by the likeliest two names, all at once; the first in this order that has it wins.
        let mut jobs: Vec<Job<String>> = Vec::new();
        for (_, find) in pages {
            let names: Vec<_> = names.iter().take(2).cloned().collect();
            jobs.push(Box::new(move || names.iter().find_map(|(a, t)| find(a, t).ok().flatten())));
        }
        let got = at_once(jobs);
        let had =
            got.into_iter().zip(pages).find_map(|(text, (who, _))| Some((who, text.filter(|t| spoken(title, t))?)));
        if let Some((who, text)) = had {
            crate::log::note(&format!("lyrics: plain words from {who}"));
            plain = Some(text);
        }
        plain = plain.or(under);
    }
    let what = if plain.is_some() { "plain words only" } else { "nothing" };
    let took = began.elapsed().as_millis();
    crate::log::note(&format!("lyrics for \"{artist}\" \"{title}\": {what} after {asked} asks, {took} ms"));
    found.text = plain;
    found
}

/// The artist and title a stream is to be kept by, from the name its service gave it and what YouTube Music
/// calls the song: what the video's name says of itself first, as that keeps a Greek name in Greek.
pub fn known(artist: &str, title: &str, song: Option<(String, String)>) -> (String, String) {
    let own = names(artist, title).swap_remove(0);
    if bare(title).contains(" - ") { own } else { song.unwrap_or(own) }
}

/// Lyrics for a file: its `.lrc` beside it, else what is found online, saved beside it for next time. Words kept
/// without times are asked for again, in case somebody has timed them since.
pub fn find(path: &std::path::Path, artist: &str, title: &str, seconds: f64, online: bool) -> Lyrics {
    find_named(path, artist, title, seconds, online).0
}

/// [`find`], and with it what YouTube Music calls the song, where it was asked.
pub fn find_named(
    path: &std::path::Path,
    artist: &str,
    title: &str,
    seconds: f64,
    online: bool,
) -> (Lyrics, Option<(String, String)>) {
    let lrc = path.with_extension("lrc");
    let kept = std::fs::read_to_string(&lrc).ok();
    // Words this app wrote down by ear give way to a person's, once those are found with times.
    let ours = kept.as_deref().is_some_and(lrc::is_auto);
    if !online || (timed(&kept) && !ours) {
        return (kept.map(|t| Lyrics::parse(&t)).unwrap_or_default(), None);
    }
    let refused = std::fs::read_to_string(path.with_extension(REFUSED)).ok();
    let found = fetch(video(path), artist, title, seconds, refused.as_deref());
    if let Some(text) = &found.text
        && (kept.is_none() || text.contains('['))
    {
        let mark = if found.unsure { format!("{UNSURE}\n") } else { String::new() };
        let _ = std::fs::write(&lrc, format!("{mark}{text}"));
        return (Lyrics::parse(text), found.song);
    }
    (kept.map(|t| Lyrics::parse(&t)).unwrap_or_default(), found.song)
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

/// Files whose listening was called off: left alone until the app is started again.
static CALLED_OFF: std::sync::Mutex<Vec<std::path::PathBuf>> = std::sync::Mutex::new(Vec::new());

/// Calls off the listening to the file at `path`.
pub fn call_off(path: &std::path::Path) {
    CALLED_OFF.lock().unwrap_or_else(|e| e.into_inner()).push(path.to_owned());
    djsugar_engine::word_timing::stop();
}

fn called_off(path: &std::path::Path) -> bool {
    CALLED_OFF.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|p| p == path)
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
    if called_off(path) || !WANTED.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|(_, p)| p == path) {
        return None;
    }
    let heard = if lyrics.lines.is_empty() {
        write(path, said).map(Some)
    } else if lyrics.untimed() {
        time(path, lyrics, said).map(Some)
    } else if unchecked(path) {
        check(path, &lyrics, said.clone()).and_then(|checked| match checked {
            Some(lines) => Ok(lines),
            None => write(path, said).map(Some),
        })
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

/// Holds a file's timed lyrics against the singing, once: Some of Some, with every word where it is heard,
/// when they were off. The `.lrc` is marked, so the track is not listened to again. None when they were found
/// by a title alone and are not what is sung: they are put aside, never to be taken for this file again.
fn check(path: &std::path::Path, lyrics: &Lyrics, said: Said) -> Result<Option<Option<Lyrics>>, String> {
    use djsugar_engine::word_timing::Checked;
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
    let checked = djsugar_engine::word_timing::check_words(path, &lyrics.lines, moved, report(said))?;
    let lrc = path.with_extension("lrc");
    let kept = std::fs::read_to_string(&lrc).unwrap_or_default();
    let unsure = kept.lines().next().is_some_and(|l| l.trim() == UNSURE);
    let what = match &checked {
        Checked::Mended(_) => "the times were off and are set by ear",
        Checked::Wrong if unsure => "they are another song's, and are put aside",
        Checked::Wrong => "little of them was made out, and they are left as they came",
        Checked::Right => "the times are left as they came",
    };
    crate::log::note(&format!("lyrics checked in {:.0} s: {what}", began.elapsed().as_secs_f64()));
    if checked == Checked::Wrong && unsure {
        let _ = std::fs::rename(&lrc, path.with_extension(REFUSED));
        return Ok(None);
    }
    let mended = if let Checked::Mended(lines) = checked { Some(lines) } else { None };
    let kept = match &mended {
        Some(lines) => lrc::to_lrc(lines),
        None => kept.strip_prefix(UNSURE).unwrap_or(&kept).trim_start().to_owned(),
    };
    let _ = std::fs::write(lrc, format!("{CHECKED}\n{kept}"));
    Ok(Some(mended.map(|lines| Lyrics { lines })))
}

/// Times a file's untimed lyrics and keeps them beside it as an `.lrc`: each word where it is heard sung, or, when
/// the words can't be made out, the lines spread over the sung stretches.
pub fn time(path: &std::path::Path, mut lyrics: Lyrics, said: Said) -> Result<Lyrics, String> {
    let began = std::time::Instant::now();
    match djsugar_engine::word_timing::time_words(path, &lyrics.lines, report(said)) {
        Ok(lines) => lyrics.lines = lines,
        Err(why) if called_off(path) => return Err(why),
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

    #[test]
    fn a_file_called_off_is_not_listened_to_again() {
        let path = std::path::Path::new("/nowhere/called-off.m4a");
        want(0, path);
        call_off(path);
        let said: Said = std::sync::Arc::new(|now| panic!("listened: {now}"));
        assert!(by_ear(path, Lyrics { lines: Vec::new() }, said).is_none());
    }

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
        let l = fetch(None, "DuaLipaVEVO", "Dua Lipa - Levitating (Official Video)", 203.0, None).text.unwrap();
        assert!(l.contains("[0"), "{l}");
    }

    #[test]
    fn what_a_video_says_of_itself_is_no_part_of_the_song() {
        assert_eq!(bare("Levitating Official Music Video"), "Levitating");
        assert_eq!(bare("Το Γράμμα Official Video HD"), "Το Γράμμα");
        assert_eq!(bare("Λογαριασμός στίχοι"), "Λογαριασμός");
        assert_eq!(bare("Something New"), "Something New");
        assert_eq!(bare("Music Video"), "Music Video");
        assert_eq!(bare("Lyrics"), "Lyrics");
        let n = names("Mazonakis Official", "Γιώργος Μαζωνάκης - Οινόπνευμα Φτηνό - Official Music Video");
        assert!(n.contains(&("giorgos mazonakis".to_string(), "oinopnevma ftino".to_string())), "{n:?}");
        assert_eq!(latin("Που είσαι αύριο"), "pou eisai avrio");
        assert!(n.last().unwrap().0.is_empty());
    }

    #[test]
    fn lyrics_of_one_song_are_told_from_another() {
        let one = "[00:01.00]<00:01.00>Θέλω <00:01.40>να σε δω ξανά\n[00:05.00]μέσα στη βροχή περπατώ μόνος\nκαι ψάχνω τα μάτια σου";
        let same = "thelo na se do xana\nmesa sti vrochi perpato monos\nkai psachno ta matia sou paντού";
        let other = "You said the rain would never fall on summer nights like these\nand walking home alone again";
        assert!(alike(one, same) >= SAME, "{}", alike(one, same));
        assert!(alike(one, other) < SAME);
        assert!(alike("la la", "la la") < SAME);
        assert!(!spoken("Το Γράμμα", "[00:24.600]欠二两笔墨债\n[00:27.390]闲听梅雨窗外"));
        assert!(spoken("Το Γράμμα", "[00:19.23] Ήρθες εχθές μετά από χρόνια"));
        assert!(spoken("Lemon", "[00:01.00]夢ならばどれほどよかったでしょう"));
    }

    #[test]
    fn a_stream_is_kept_by_what_its_name_says() {
        let song = Some(("Giorgos Mazonakis".to_string(), "Inopnevma Ftino".to_string()));
        let k = known("Mazonakis", "Γιώργος Μαζωνάκης - Οινόπνευμα Φτηνό - Official Music Video", song.clone());
        assert_eq!(k, ("Γιώργος Μαζωνάκης".to_string(), "Οινόπνευμα Φτηνό".to_string()));
        assert_eq!(known("LightVEVO", "Baba Yaga", Some(("Light".into(), "Baba Yaga".into()))).0, "Light");
        assert_eq!(
            known("LightVEVO", "Baba Yaga (Official Video)", None),
            ("Light".to_string(), "Baba Yaga".to_string())
        );
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
        let found = fetch(Some("TUVcZfQe-Kw"), "", "dl lev mv", 231.0, None);
        assert_eq!(found.song, Some(("Dua Lipa".to_string(), "Levitating".to_string())));
        let l = found.text.unwrap();
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

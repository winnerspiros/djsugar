//! Lyrics read off the pages of sites that keep them as plain words: AZLyrics, whose pages go by the artist's and the
//! song's names, and LyricsTranslate, J-Lyric, GreekLyrics, stixoi.info and a Greek hip hop blog, which are searched.
//! None times its lines.

use std::time::Duration;

use crate::http::{agent, read};
use crate::{Error, Result};

/// AZLyrics and the Greek sites answer a browser.
const BROWSER: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
/// LyricsTranslate turns away what calls itself a browser and is none, and answers what says what it is.
const OURS: &str = "djsugar (https://github.com/winnerspiros/djsugar)";
const TIMEOUT: Duration = Duration::from_secs(12);
/// How many songs of one name stixoi.info is asked for.
const SAME: usize = 3;
const GREEK: &str = "https://www.greeklyrics.gr";
/// What AZLyrics puts right ahead of a song's words.
const AHEAD: &str = "<!-- Usage of azlyrics.com content";
/// What GreekLyrics opens each part of a song's words with.
const PART: &str = "class=\"lyrics-content";

/// A page's text; None when the site has no such page.
fn page(url: &str, who: &str) -> Result<Option<String>> {
    let call = agent().get(url).header("User-Agent", who).config().timeout_global(Some(TIMEOUT)).build().call();
    match read(call)? {
        (200..=299, body) => Ok(Some(body)),
        (404, _) => Ok(None),
        (s, _) => Err(Error::Status(s)),
    }
}

/// A piece of a page as the lines it shows.
fn shown(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    // Only the page's own breaks end a line: one for a line, two around a verse.
    let text = |out: &mut String, t: &str| out.extend(t.chars().filter(|c| !matches!(c, '\n' | '\r')));
    while let Some(at) = rest.find('<') {
        text(&mut out, &rest[..at]);
        let Some(end) = rest[at..].find('>') else { break };
        let tag = rest[at + 1..at + end].trim_start_matches('/');
        if tag.starts_with("br") {
            out.push('\n');
        } else if tag == "p" {
            out.push_str("\n\n");
        }
        rest = &rest[at + end + 1..];
    }
    text(&mut out, rest);
    for (code, c) in [
        ("&nbsp;", " "),
        ("&quot;", "\""),
        ("&#039;", "'"),
        ("&#x27;", "'"),
        ("&#39;", "'"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&amp;", "&"),
    ] {
        out = out.replace(code, c);
    }
    let lines: Vec<&str> = out.lines().map(str::trim).collect();
    // One empty line between verses, none at the ends.
    let mut kept: Vec<&str> = Vec::new();
    for l in lines {
        if !(l.is_empty() && kept.last().is_none_or(|k| k.is_empty())) {
            kept.push(l);
        }
    }
    kept.join("\n").trim().to_string()
}

/// A name as AZLyrics spells it in a page's address: its Latin letters and digits, lower case.
fn spelled(name: &str) -> String {
    name.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect()
}

/// The words of a song on its AZLyrics page.
fn az_words(html: &str) -> Option<String> {
    let (_, rest) = html.split_once(AHEAD)?;
    let (_, rest) = rest.split_once("-->")?;
    let words = shown(rest.split_once("</div>")?.0);
    (!words.is_empty()).then_some(words)
}

/// A song's words from AZLyrics; None when it has no page by these names.
pub fn azlyrics(artist: &str, title: &str) -> Result<Option<String>> {
    let low = artist.trim().to_lowercase();
    let (a, t) = (spelled(low.strip_prefix("the ").unwrap_or(&low)), spelled(title));
    // A name in other letters spells to nothing, or to too little to be the song.
    if a.is_empty() || t.is_empty() || t.len() * 2 < title.trim().chars().filter(|c| c.is_alphanumeric()).count() {
        return Ok(None);
    }
    Ok(page(&format!("https://www.azlyrics.com/lyrics/{a}/{t}.html"), BROWSER)?.as_deref().and_then(az_words))
}

/// A Greek name without its accents and its case, letters and digits only.
fn plain(name: &str) -> String {
    let bare = |c: char| match c {
        'ά' => 'α',
        'έ' => 'ε',
        'ή' => 'η',
        'ί' | 'ϊ' | 'ΐ' => 'ι',
        'ό' => 'ο',
        'ύ' | 'ϋ' | 'ΰ' => 'υ',
        'ώ' => 'ω',
        'ς' => 'σ',
        c => c,
    };
    name.to_lowercase()
        .chars()
        .map(bare)
        .filter(|c| c.is_alphanumeric() || *c == ' ')
        .collect::<String>()
        .trim()
        .to_string()
}

/// What a GreekLyrics search found: each song's page, name and artist.
fn rows(html: &str) -> Vec<(&str, String, String)> {
    let mut out = Vec::new();
    for row in html.split("<tr").skip(1) {
        let Some((_, rest)) = row.split_once("href=\"/songs/") else { continue };
        let Some((path, rest)) = rest.split_once('"') else { continue };
        let Some((_, rest)) = rest.split_once('>') else { continue };
        let Some((name, rest)) = rest.split_once("</a>") else { continue };
        let artist =
            rest.split_once("<td").and_then(|(_, r)| r.split_once('>')).and_then(|(_, r)| r.split_once("</td>"));
        if path.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            out.push((path, shown(name), artist.map(|a| shown(a.0)).unwrap_or_default()));
        }
    }
    out
}

/// The found song that is this one: of the same name, and by the artist when more than one is.
fn pick<'a>(rows: &[(&'a str, String, String)], artist: &str, title: &str) -> Option<&'a str> {
    let same: Vec<_> = rows.iter().filter(|r| plain(&r.1) == plain(title)).collect();
    let artist = plain(artist);
    let theirs = |r: &&&(&str, String, String)| {
        let known = format!("{} {}", plain(&r.2), r.0.to_lowercase().replace('-', " "));
        artist.split(' ').filter(|w| w.chars().count() > 2).any(|w| known.split(' ').any(|k| k == w))
    };
    match same.iter().find(theirs) {
        Some(r) => Some(r.0),
        None if same.len() == 1 && artist.is_empty() => Some(same[0].0),
        None => None,
    }
}

/// The words of a song on its GreekLyrics page, which shows them in parts.
fn greek_words(html: &str) -> Option<String> {
    let parts: Vec<String> =
        html.split(PART).skip(1).filter_map(|p| Some(shown(p.split_once('>')?.1.split_once("</div>")?.0))).collect();
    let words = parts.join("\n\n");
    (!words.trim().is_empty()).then_some(words)
}

/// A song's words from GreekLyrics; asked only for names in Greek letters.
pub fn greeklyrics(artist: &str, title: &str) -> Result<Option<String>> {
    if !greek(title) {
        return Ok(None);
    }
    let q = crate::soundcloud::encode(title.trim());
    let Some(found) = page(&format!("{GREEK}/search?q={q}"), BROWSER)? else { return Ok(None) };
    let Some(path) = pick(&rows(&found), artist, title) else { return Ok(None) };
    Ok(page(&format!("{GREEK}/songs/{path}"), BROWSER)?.as_deref().and_then(greek_words))
}

/// What a LyricsTranslate search found: each song's page, name and artist.
fn lt_rows(html: &str) -> Vec<(&str, String, String)> {
    let mut out = Vec::new();
    for row in html.split("class=\"table__trow\"").skip(1) {
        let Some((_, rest)) = row.split_once("data-href=\"/en/") else { continue };
        let Some((path, rest)) = rest.split_once('"') else { continue };
        let Some((_, rest)) = rest.split_once("class=\"title-text\">") else { continue };
        let Some((name, rest)) = rest.split_once("</span>") else { continue };
        let artist = rest.split_once("__author\">").and_then(|(_, r)| r.split_once("</div>"));
        // A name in other letters is followed by how it reads in Latin ones.
        let name = shown(name);
        let name = name.split_once(" (").map_or(name.as_str(), |n| n.0).to_string();
        if path.ends_with("-lyrics.html") && path.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.%".contains(&b)) {
            out.push((path, name, artist.map(|a| shown(a.0)).unwrap_or_default()));
        }
    }
    out
}

/// The words of a song on its LyricsTranslate page; None where they were taken down.
fn lt_words(html: &str) -> Option<String> {
    let (_, rest) = html.split_once("id=\"song-body\">")?;
    let body = rest.split_once("<div id=\"song-transliteration\"")?.0;
    // Each line is a block of its own, and each verse a block of those.
    let words = shown(&body.replace("</div>", "<br>"));
    (words.lines().count() > 2).then_some(words)
}

/// A song's words from LyricsTranslate; None when it has no such song, or not its words.
pub fn lyricstranslate(artist: &str, title: &str) -> Result<Option<String>> {
    if artist.trim().is_empty() || title.trim().is_empty() {
        return Ok(None);
    }
    let (a, t) = (crate::soundcloud::encode(artist.trim()), crate::soundcloud::encode(title.trim()));
    let search = format!("https://lyricstranslate.com/en/songs/0/{a}/{t}/0/none/0?order=relevance");
    let Some(found) = page(&search, OURS)? else { return Ok(None) };
    let Some(path) = pick(&lt_rows(&found), artist, title) else { return Ok(None) };
    Ok(page(&format!("https://lyricstranslate.com/en/{path}"), OURS)?.as_deref().and_then(lt_words))
}

/// What a stixoi.info search found: each song's number and name.
fn stixoi_rows(html: &str) -> Vec<(&str, String)> {
    let mut out = Vec::new();
    for row in html.split("href=\"/songs/").skip(1) {
        let Some((id, rest)) = row.split_once('"') else { continue };
        let Some((_, rest)) = rest.split_once("class=\"font-medium\">") else { continue };
        let Some((name, _)) = rest.split_once("</span>") else { continue };
        if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) {
            out.push((id, shown(name)));
        }
    }
    out
}

/// The words of a song on its stixoi.info page, which shows each line apart.
fn stixoi_words(html: &str) -> Option<String> {
    let line =
        |l: &str| Some(l.split_once("whitespace-pre-wrap")?.1.split_once('>')?.1.split_once("</span>")?.0.to_string());
    let lines: Vec<String> = html.split("data-line-index=").skip(1).filter_map(line).collect();
    let words = shown(&lines.join("<br>"));
    (words.lines().count() > 2).then_some(words)
}

/// A song's words from stixoi.info; asked only for names in Greek letters.
pub fn stixoi(artist: &str, title: &str) -> Result<Option<String>> {
    if !greek(title) {
        return Ok(None);
    }
    let q = crate::soundcloud::encode(title.trim());
    let Some(found) = page(&format!("https://stixoi.info/search?q={q}&scope=songs"), BROWSER)? else { return Ok(None) };
    let rows = stixoi_rows(&found);
    let same: Vec<&str> = rows.iter().filter(|r| plain(&r.1) == plain(title)).map(|r| r.0).take(SAME).collect();
    // The search names who wrote a song, not who sings it: the singer is looked for on each page of that name.
    let singer = plain(artist);
    let mut only = None;
    for id in &same {
        let Some(song) = page(&format!("https://stixoi.info/songs/{id}"), BROWSER)? else { continue };
        let known = plain(&shown(&song));
        if singer.split(' ').filter(|w| w.chars().count() > 2).any(|w| known.split(' ').any(|k| k == w)) {
            return Ok(stixoi_words(&song));
        }
        only = stixoi_words(&song);
    }
    Ok(only.filter(|_| same.len() == 1))
}

/// Whether a name has Greek letters in it.
fn greek(name: &str) -> bool {
    name.chars().any(|c| ('\u{370}'..='\u{3ff}').contains(&c))
}

/// Whether a name has Japanese letters in it: kana, or the characters shared with Chinese.
fn japanese(name: &str) -> bool {
    name.chars().any(|c| matches!(c, '\u{3040}'..='\u{30ff}' | '\u{4e00}'..='\u{9fff}'))
}

/// What a J-Lyric search found: each song's page and name.
fn jlyric_rows(html: &str) -> Vec<(&str, String)> {
    let mut out = Vec::new();
    for row in html.split("<p class=\"mid\"><a href=\"/artist/").skip(1) {
        let Some((path, rest)) = row.split_once('"') else { continue };
        let Some((_, rest)) = rest.split_once('>') else { continue };
        let Some((name, _)) = rest.split_once("</a>") else { continue };
        if path.ends_with(".html") && path.bytes().all(|b| b.is_ascii_alphanumeric() || b"/.".contains(&b)) {
            out.push((path, shown(name)));
        }
    }
    out
}

/// A song's words from J-Lyric, which is searched by song and singer; asked only for names in Japanese letters.
pub fn jlyric(artist: &str, title: &str) -> Result<Option<String>> {
    if artist.trim().is_empty() || !(japanese(title) || japanese(artist)) {
        return Ok(None);
    }
    let (a, t) = (crate::soundcloud::encode(artist.trim()), crate::soundcloud::encode(title.trim()));
    let search = format!("https://j-lyric.net/search.php?kt={t}&ct=2&ka={a}&ca=2&kl=&cl=2");
    let Some(found) = page(&search, BROWSER)? else { return Ok(None) };
    let rows = jlyric_rows(&found);
    let Some(path) = rows.iter().find(|r| plain(&r.1) == plain(title)).map(|r| r.0) else { return Ok(None) };
    let song = page(&format!("https://j-lyric.net/artist/{path}"), BROWSER)?;
    let words = song.as_deref().and_then(|s| Some(shown(s.split_once("id=\"Lyric\">")?.1.split_once("</p>")?.0)));
    Ok(words.filter(|w| w.lines().count() > 2))
}

/// The post of a Blogger search that is this song's: its words. Posts are headed "artist - song".
fn post(feed: &serde_json::Value, artist: &str, title: &str) -> Option<String> {
    let singer = plain(artist);
    let ours = |e: &&serde_json::Value| {
        let head = e["title"]["$t"].as_str().unwrap_or_default();
        let Some((by, name)) = head.rsplit_once(" - ") else { return false };
        let by = plain(by);
        plain(name) == plain(title)
            && singer.split(' ').filter(|w| w.chars().count() > 2).any(|w| by.split(' ').any(|k| k == w))
    };
    let body = feed["feed"]["entry"].as_array()?.iter().find(ours)?["content"]["$t"].as_str()?;
    // The words are kept as typed, a line to a line.
    let words = shown(&body.replace('\n', "<br>").replace('\u{feff}', ""));
    (words.lines().count() > 2).then_some(words)
}

/// A song's words from the Greek hip hop lyrics blog; asked only for names in Greek letters.
pub fn hiphoplyrics(artist: &str, title: &str) -> Result<Option<String>> {
    if artist.trim().is_empty() || !greek(title) {
        return Ok(None);
    }
    let q = crate::soundcloud::encode(title.trim());
    let search = format!("https://hiphoplyrics.blogspot.com/feeds/posts/default?alt=json&max-results=8&q={q}");
    let Some(found) = page(&search, OURS)? else { return Ok(None) };
    let feed = serde_json::from_str(&found).map_err(|_| Error::Parse("not JSON"))?;
    Ok(post(&feed, artist, title))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_words_off_an_azlyrics_page() {
        let html = "<div class=\"ringtone\"></div><div>\n<!-- Usage of azlyrics.com content by x. -->\nOne more time<br>\nWe&#39;re gonna celebrate<br>\n<br>\n<i>[Chorus]</i><br>\nOh yeah\n</div><div>after</div>";
        assert_eq!(az_words(html).unwrap(), "One more time\nWe're gonna celebrate\n\n[Chorus]\nOh yeah");
        assert_eq!(az_words("<div>no song</div>"), None);
        assert_eq!(spelled("Don't Stop (Me) Now!"), "dontstopmenow");
    }

    #[test]
    fn picks_the_greek_song_by_name_and_artist() {
        let html = "<table><tr><td><a class=\"x\" href=\"/songs/lidakis-manolis-to-zeibekiko\">Το ζεϊμπέκικο</a></td><td class=\"y\">Λιδάκης Μανώλης</td></tr>\
            <tr><td><a href=\"/songs/dalaras-giorgos-to-zeibekiko\">Το ζεϊμπέκικο</a></td><td>Νταλάρας Γιώργος</td></tr></table>";
        let found = rows(html);
        assert_eq!(found.len(), 2);
        let title = "το Ζειμπεκικο";
        // By the artist in Greek, by the artist in Latin letters as the page's address has it, and by nobody.
        assert_eq!(pick(&found, "Γιώργος Νταλάρας", title), Some("dalaras-giorgos-to-zeibekiko"));
        assert_eq!(pick(&found, "Giorgos Dalaras", title), Some("dalaras-giorgos-to-zeibekiko"));
        assert_eq!(pick(&found, "", title), None);
        assert_eq!(pick(&found[..1], "", title), Some("lidakis-manolis-to-zeibekiko"));
    }

    #[test]
    fn joins_the_parts_of_a_greek_page() {
        let html = "<div class=\"lyrics-content leading-tight\"><p>One\n<br/>Two</p><p>Three</p></div></div><div class=\"ad\"></div><div><div class=\"lyrics-content leading-tight\"><p>Four</p></div>";
        assert_eq!(greek_words(html).unwrap(), "One\nTwo\n\nThree\n\nFour");
    }

    #[test]
    fn reads_a_lyricstranslate_search_and_page() {
        let html = "<div class=\"table__trow\"><div data-href=\"/en/a-b-song-lyrics.html\"><a class=\"title-link\"><span class=\"title-text\">Το τραγούδι (To tragoudi)</span></a><div class=\"block-1-table__author-wrap\"><span class=\"block-1-table__author\"><span class=\"text\">Some Singer</span></span></div></div></div>";
        let found = lt_rows(html);
        assert_eq!(found, [("a-b-song-lyrics.html", "Το τραγούδι".to_string(), "Some Singer".to_string())]);
        assert_eq!(pick(&found, "Some Singer", "το τραγουδι"), Some("a-b-song-lyrics.html"));
        let song = "<div class=\"x\" id=\"song-body\">\n<div class=\"ltf\"><div class='par'><div class=\"ll-0-1\">One</div><div class=\"ll-0-2\">\nTwo&#039;s</div></div><div class='emptyline'>&nbsp;</div><div class='par'><div class=\"ll-1-1\">Three</div></div></div>\n</div>\n<div id=\"song-transliteration\" data-nosnippet></div>";
        assert_eq!(lt_words(song).unwrap(), "One\nTwo's\n\nThree");
        let gone = "<div id=\"song-body\">\n<p>Lyrics removed on request of copyright owners</p></div><div id=\"song-transliteration\">";
        assert_eq!(lt_words(gone), None);
    }

    #[test]
    fn reads_a_stixoi_search_and_page() {
        let html = "<li><a class=\"block\" href=\"/songs/1165\"><div><span><span class=\"font-medium\"><mark>Συννεφιασμένη</mark> <mark>Κυριακή</mark></span><span class=\"ml-2\">· Τσιτσάνης</span></span></div></a></li>";
        assert_eq!(stixoi_rows(html), [("1165", "Συννεφιασμένη Κυριακή".to_string())]);
        let line = |i: usize, c: &str, t: &str| {
            format!(
                "<div class=\"group\" data-line-index=\"{i}\"><div><span class=\"flex-1 whitespace-pre-wrap {c}\">{t}</span></div></div>"
            )
        };
        let song = [line(0, "", "One"), line(1, "", "Two"), line(2, "opacity-0", " "), line(3, "", "Three")].concat();
        assert_eq!(stixoi_words(&song).unwrap(), "One\nTwo\n\nThree");
    }

    #[test]
    fn reads_a_jlyric_search_and_a_blog_post() {
        let html = "<div class=\"bdy\"><p class=\"mid\"><a href=\"/artist/a061652/l050f8a.html\" title=\"x\">夜に駆ける</a></p><p class=\"sml\">歌：YOASOBI</p></div>";
        assert_eq!(jlyric_rows(html), [("a061652/l050f8a.html", "夜に駆ける".to_string())]);
        assert!(japanese("夜に駆ける") && japanese("アイドル") && !japanese("Idol") && !greek("Idol") && greek("ΛΕΞ"));
        let feed = serde_json::json!({"feed": {"entry": [
            {"title": {"$t": "Other - Ένα Αστέρι"}, "content": {"$t": "no"}},
            {"title": {"$t": "Eversor x ΛΕΞ - Ένα Αστέρι"}, "content": {"$t": "<div>\n<iframe src=\"x\"></iframe></div>\n<span>One,\nTwo\n\nThree\u{feff}</span>"}},
        ]}});
        assert_eq!(post(&feed, "ΛΕΞ", "ένα αστέρι").unwrap(), "One,\nTwo\n\nThree");
        assert_eq!(post(&feed, "Someone", "Ένα Αστέρι"), None);
    }

    #[test]
    #[ignore = "network"]
    fn live_lookups() {
        let jl = jlyric("YOASOBI", "夜に駆ける").unwrap().unwrap();
        assert!(jl.starts_with("沈むように") && jl.lines().count() > 30, "{jl}");
        let hh = hiphoplyrics("ΛΕΞ", "Ένα Αστέρι από Τσιμέντο").unwrap().unwrap();
        assert!(hh.starts_with("Με μάτια") && !hh.contains('<'), "{hh}");
        let lt = lyricstranslate("Frixos Gazepides", "Το τραγούδι του Χατζηαβάτη").unwrap().unwrap();
        assert!(lt.starts_with("Αμάν") && !lt.contains('<'), "{lt}");
        assert!(lyricstranslate("Stromae", "Papaoutai").unwrap().is_none());
        let sx = stixoi("Βασίλης Τσιτσάνης", "Συννεφιασμένη Κυριακή").unwrap().unwrap();
        assert!(sx.lines().count() > 6 && !sx.contains('<'), "{sx}");
        let az = azlyrics("Daft Punk", "One More Time").unwrap().unwrap();
        assert!(az.starts_with("One more time") && !az.contains('<'), "{az}");
        assert!(azlyrics("Daft Punk", "No Such Song Of Theirs").unwrap().is_none());
        let title = "Το ζεϊμπέκικο του Νότου";
        let gr = greeklyrics("Giorgos Dalaras", title).unwrap().unwrap();
        assert!(gr.lines().count() > 10 && !gr.contains('<'), "{gr}");
    }
}

//! Lyrics read off the pages of sites that keep them as plain words: AZLyrics, whose pages go by the artist's and the
//! song's names, and GreekLyrics, which is searched by the song's name. Neither times its lines.

use std::time::Duration;

use crate::http::{agent, read};
use crate::{Error, Result};

const AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const TIMEOUT: Duration = Duration::from_secs(12);
const GREEK: &str = "https://www.greeklyrics.gr";
/// What AZLyrics puts right ahead of a song's words.
const AHEAD: &str = "<!-- Usage of azlyrics.com content";
/// What GreekLyrics opens each part of a song's words with.
const PART: &str = "class=\"lyrics-content";

/// A page's text; None when the site has no such page.
fn page(url: &str) -> Result<Option<String>> {
    let call = agent().get(url).header("User-Agent", AGENT).config().timeout_global(Some(TIMEOUT)).build().call();
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
    for (code, c) in [("&quot;", "\""), ("&#x27;", "'"), ("&#39;", "'"), ("&lt;", "<"), ("&gt;", ">"), ("&amp;", "&")] {
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
    Ok(page(&format!("https://www.azlyrics.com/lyrics/{a}/{t}.html"))?.as_deref().and_then(az_words))
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
        let known = format!("{} {}", plain(&r.2), r.0.replace('-', " "));
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
    if !title.chars().any(|c| ('Ͱ'..='Ͽ').contains(&c)) {
        return Ok(None);
    }
    let q = crate::soundcloud::encode(title.trim());
    let Some(found) = page(&format!("{GREEK}/search?q={q}"))? else { return Ok(None) };
    let Some(path) = pick(&rows(&found), artist, title) else { return Ok(None) };
    Ok(page(&format!("{GREEK}/songs/{path}"))?.as_deref().and_then(greek_words))
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
    #[ignore = "network"]
    fn live_lookups() {
        let az = azlyrics("Daft Punk", "One More Time").unwrap().unwrap();
        assert!(az.starts_with("One more time") && !az.contains('<'), "{az}");
        assert!(azlyrics("Daft Punk", "No Such Song Of Theirs").unwrap().is_none());
        let title = "Το ζεϊμπέκικο του Νότου";
        let gr = greeklyrics("Giorgos Dalaras", title).unwrap().unwrap();
        assert!(gr.lines().count() > 10 && !gr.contains('<'), "{gr}");
    }
}

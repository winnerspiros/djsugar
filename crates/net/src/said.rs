//! Lyrics people have written under a YouTube video: in its description or its most liked comments. Asked last, and
//! only a long run of lines that read as verses is taken. Lines that start with a time become timed lines.

use std::time::Duration;

use serde_json::{Value, json};

use crate::http::{agent, read};
use crate::{Error, Result};

const ORIGIN: &str = "https://www.youtube.com";
const VERSION: &str = "2.20260708.00.00";
const AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const TIMEOUT: Duration = Duration::from_secs(12);
/// The fewest lines in a row that count as a song's words.
const VERSES: usize = 12;
/// A line longer than this is prose, not a verse.
const LONGEST: usize = 90;
/// The fewest lines that must start with a time for the times to be kept.
const TIMED: usize = 8;
/// How many pages of comments are read, most liked first.
const PAGES: usize = 4;

fn post(body: &Value) -> Result<Value> {
    let mut body = body.clone();
    body["context"] = json!({ "client": { "clientName": "WEB", "clientVersion": VERSION, "hl": "en" } });
    let req = agent()
        .post(format!("{ORIGIN}/youtubei/v1/next?prettyPrint=false"))
        .config()
        .timeout_global(Some(TIMEOUT))
        .build()
        .header("Content-Type", "application/json")
        .header("User-Agent", AGENT)
        .header("Origin", ORIGIN);
    match read(req.send(body.to_string()))? {
        (200..=299, text) => serde_json::from_str(&text).map_err(|_| Error::Parse("not JSON")),
        (s, _) => Err(Error::Status(s)),
    }
}

fn walk(v: &Value, visit: &mut dyn FnMut(&serde_json::Map<String, Value>)) {
    match v {
        Value::Object(m) => {
            visit(m);
            m.values().for_each(|x| walk(x, visit));
        }
        Value::Array(a) => a.iter().for_each(|x| walk(x, visit)),
        _ => {}
    }
}

/// The video's description and where its comments continue.
fn page(next: &Value) -> (String, Option<String>) {
    let (mut told, mut more) = (String::new(), None);
    walk(next, &mut |m| {
        if let Some(t) = m.get("attributedDescription").and_then(|d| d["content"].as_str()) {
            told = t.to_owned();
        }
        if m.get("sectionIdentifier").and_then(Value::as_str) == Some("comment-item-section") {
            let token =
                &m["contents"][0]["continuationItemRenderer"]["continuationEndpoint"]["continuationCommand"]["token"];
            more = token.as_str().map(str::to_owned).or(more.take());
        }
    });
    (told, more)
}

/// What each comment of a page says, and where the comments go on.
fn comments(page: &Value) -> (Vec<String>, Option<String>) {
    let (mut out, mut more) = (Vec::new(), None);
    walk(page, &mut |m| {
        if let Some(t) = m.get("commentEntityPayload").and_then(|c| c["properties"]["content"]["content"].as_str()) {
            out.push(t.to_owned());
        }
        // The page ends with its own continuation; each comment's replies have one inside the comment.
        if let Some(items) = m.get("continuationItems").and_then(Value::as_array) {
            let token = &items.last().unwrap_or(&Value::Null)["continuationItemRenderer"]["continuationEndpoint"]["continuationCommand"]
                ["token"];
            more = token.as_str().map(str::to_owned).or(more.take());
        }
    });
    (out, more)
}

/// A line of a list: "3. Name", "3) Name" or a tab after the number.
fn listed(line: &str) -> bool {
    let rest = line.trim_start_matches(|c: char| c.is_ascii_digit());
    rest.len() < line.len() && rest.starts_with(['.', ')', '\t'])
}

/// "1:05 words" as its seconds and words.
fn stamped(line: &str) -> Option<(u32, &str)> {
    let line = line.trim_start_matches(['[', '(']);
    let (clock, rest) = line.split_once([' ', ']', ')'])?;
    let (m, s) = clock.split_once(':')?;
    let (m, s): (u32, u32) = (m.parse().ok()?, s.parse().ok().filter(|_| s.len() == 2)?);
    Some((m * 60 + s, rest.trim_start_matches([' ', '-', ']', ')']).trim()))
}

/// Whether a line could be sung: short, and nothing of a link, a tag or a credit about it.
fn sung(line: &str) -> bool {
    let credit = line.split_once(": ").is_some_and(|(_, said)| !said.is_empty());
    let marks = ["http", "www.", "@", "#", "\u{a9}", "\u{2117}", "\u{2022}"];
    line.chars().count() <= LONGEST && !credit && !marks.iter().any(|m| line.contains(m))
}

/// The song's words in `text`, when it holds a long enough run of verses: LRC lines when most carry a time,
/// plain lines otherwise.
pub fn verses(text: &str) -> Option<String> {
    let mut best: Vec<&str> = Vec::new();
    let mut run: Vec<&str> = Vec::new();
    for line in text.lines().map(str::trim).chain([LINK]) {
        let body = stamped(line).map_or(line, |(_, words)| words);
        if sung(body) {
            run.push(line);
            continue;
        }
        if run.iter().filter(|l| !l.is_empty()).count() > best.iter().filter(|l| !l.is_empty()).count() {
            best = std::mem::take(&mut run);
        }
        run.clear();
    }
    let mut lines: Vec<&str> = best.into_iter().filter(|l| !l.is_empty()).collect();
    // A heading such as "Lyrics:" is not sung.
    if lines.first().is_some_and(|l| l.ends_with(':')) {
        lines.remove(0);
    }
    // A list of tracks or of moments is short lines too, but numbered ones.
    if lines.len() < VERSES || lines.iter().filter(|l| listed(l)).count() * 2 > lines.len() {
        return None;
    }
    let times: Vec<(u32, &str)> = lines.iter().filter_map(|l| stamped(l)).filter(|(_, w)| !w.is_empty()).collect();
    if times.len() >= TIMED && times.len() * 2 >= lines.len() {
        let lrc = times.iter().map(|(at, words)| format!("[{:02}:{:02}.00]{words}", at / 60, at % 60));
        return Some(lrc.collect::<Vec<_>>().join("\n"));
    }
    Some(lines.iter().map(|l| l.replace(['[', ']'], "")).collect::<Vec<_>>().join("\n"))
}

/// A line no verse holds, to close the last run.
const LINK: &str = "http";

/// The words written under `video`, from its description or else its most liked comments.
pub fn find(video: &str) -> Result<Option<String>> {
    let (told, more) = page(&post(&json!({ "videoId": video }))?);
    if let Some(words) = verses(&told) {
        return Ok(Some(words));
    }
    let Some(more) = more else { return Ok(None) };
    // Timed words from any page beat plain ones; of plain ones the longest is the likeliest to be whole.
    let (mut next, mut plain) = (Some(more), None::<String>);
    for _ in 0..PAGES {
        let Some(token) = next.take() else { break };
        let Ok(page) = post(&json!({ "continuation": token })) else { break };
        let (said, more) = comments(&page);
        for words in said.iter().filter_map(|c| verses(c)) {
            if words.starts_with('[') {
                return Ok(Some(words));
            }
            if plain.as_ref().is_none_or(|p| words.len() > p.len()) {
                plain = Some(words);
            }
        }
        next = more;
    }
    Ok(plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credits_and_links_are_not_verses() {
        let told = "Official video\nhttps://example.com/buy\nDirector: Someone\nFollow @someone\n#pop";
        assert_eq!(verses(told), None);
        assert_eq!(verses("Producer: how many?\nSinger: yes"), None);
    }

    #[test]
    fn a_long_run_of_lines_is_taken_whole() {
        let words: Vec<String> = (0..14).map(|i| format!("Line number {i} of the song")).collect();
        let told = format!("Listen: https://example.com\n\n{}\n\nSubscribe: https://example.com", words.join("\n"));
        assert_eq!(verses(&told), Some(words.join("\n")));
    }

    #[test]
    fn a_list_of_tracks_is_not_a_song_and_a_heading_is_not_sung() {
        let list: Vec<String> = (1..=15).map(|i| format!("{i}.\t\"Track {i}\"")).collect();
        assert_eq!(verses(&list.join("\n")), None);
        let words: Vec<String> = (0..13).map(|i| format!("Line number {i} of the song")).collect();
        assert_eq!(verses(&format!("letra:\n{}", words.join("\n"))), Some(words.join("\n")));
    }

    #[test]
    fn lines_with_times_become_timed() {
        let words: Vec<String> = (0..12).map(|i| format!("{}:{:02} Sing line {i}", i / 6, i * 10 % 60)).collect();
        let got = verses(&words.join("\n")).unwrap();
        assert!(got.starts_with("[00:00.00]Sing line 0\n[00:10.00]Sing line 1"), "{got}");
        assert!(got.ends_with("[01:50.00]Sing line 11"), "{got}");
    }

    #[test]
    fn replies_give_the_description_and_the_comments() {
        let next = json!({ "a": { "attributedDescription": { "content": "Told" } }, "b": [{
            "sectionIdentifier": "comment-item-section",
            "contents": [{ "continuationItemRenderer": { "continuationEndpoint": { "continuationCommand": { "token": "T" } } } }] }] });
        assert_eq!(page(&next), ("Told".to_owned(), Some("T".to_owned())));
        let more = json!({ "m": [{ "payload": { "commentEntityPayload": { "properties": { "content": { "content": "Hi" } } } } }] });
        assert_eq!(comments(&more), (vec!["Hi".to_owned()], None));
        let more = json!({ "continuationItems": [
            { "replies": { "continuationItemRenderer": { "continuationEndpoint": { "continuationCommand": { "token": "R" } } } } },
            { "continuationItemRenderer": { "continuationEndpoint": { "continuationCommand": { "token": "N" } } } }] });
        assert_eq!(comments(&more).1.as_deref(), Some("N"));
    }

    #[test]
    #[ignore = "network"]
    fn asks_a_real_video() {
        let words = find("fJ9rUzIMcZQ").unwrap().unwrap();
        assert!(words.to_lowercase().contains("is this the real life"), "{words}");
    }
}

//! A track's lyrics as Spotify has them, written as LRC text.

use serde_json::Value;

/// A number Spotify writes as text or as a number.
fn ms(v: &Value) -> Option<f64> {
    v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_f64())
}

fn stamp(ms: f64) -> String {
    let cs = (ms / 10.0).round().max(0.0) as u64;
    format!("{:02}:{:02}.{:02}", cs / 6000, cs / 100 % 60, cs % 100)
}

/// Spotify's reply as LRC: a time before each line when it has them, and before each syllable when it has those
/// too; the bare words otherwise. None for a reply with no words in it.
pub fn to_lrc(reply: &Value) -> Option<String> {
    let lyrics = &reply["lyrics"];
    let timed = lyrics["syncType"].as_str().is_some_and(|t| t != "UNSYNCED");
    let mut out = String::new();
    for line in lyrics["lines"].as_array()? {
        let words = line["words"].as_str().unwrap_or_default();
        let Some(start) = ms(&line["startTimeMs"]).filter(|_| timed) else {
            out += words;
            out.push('\n');
            continue;
        };
        out += &format!("[{}]", stamp(start));
        let mut rest = words.chars();
        let mut by_word = false;
        for s in line["syllables"].as_array().map(Vec::as_slice).unwrap_or_default() {
            let (Some(at), Some(n)) = (ms(&s["startTimeMs"]), s["numChars"].as_u64()) else { continue };
            out += &format!("<{}>", stamp(at));
            out.extend(rest.by_ref().take(n as usize));
            by_word = true;
        }
        out.extend(rest);
        if let Some(end) = ms(&line["endTimeMs"]).filter(|e| by_word && *e > start) {
            out += &format!("<{}>", stamp(end));
        }
        out.push('\n');
    }
    (!out.trim().is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn lines_get_their_times() {
        let reply = json!({ "lyrics": { "syncType": "LINE_SYNCED", "lines": [
            { "startTimeMs": "960", "words": "Hold the line", "syllables": [], "endTimeMs": "0" },
            { "startTimeMs": "61504", "words": "\u{266a}", "syllables": [], "endTimeMs": "0" } ] } });
        assert_eq!(to_lrc(&reply).unwrap(), "[00:00.96]Hold the line\n[01:01.50]\u{266a}\n");
    }

    #[test]
    fn syllables_get_their_times_and_the_line_its_end() {
        let reply = json!({ "lyrics": { "syncType": "SYLLABLE_SYNCED", "lines": [ { "startTimeMs": "10000",
            "words": "Hold on", "endTimeMs": "11500", "syllables": [
                { "startTimeMs": "10000", "numChars": 5 }, { "startTimeMs": "10400", "numChars": 2 } ] } ] } });
        assert_eq!(to_lrc(&reply).unwrap(), "[00:10.00]<00:10.00>Hold <00:10.40>on<00:11.50>\n");
    }

    #[test]
    fn untimed_words_stay_bare_and_nothing_is_nothing() {
        let reply = json!({ "lyrics": { "syncType": "UNSYNCED", "lines": [
            { "startTimeMs": "0", "words": "One" }, { "startTimeMs": "0", "words": "Two" } ] } });
        assert_eq!(to_lrc(&reply).unwrap(), "One\nTwo\n");
        assert_eq!(to_lrc(&json!({ "lyrics": { "syncType": "LINE_SYNCED", "lines": [] } })), None);
        assert_eq!(to_lrc(&json!({})), None);
    }
}

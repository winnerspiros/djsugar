//! Fix Names: tools that change the names of many tracks at once (find and
//! replace, change case, remove what's in brackets, swap artist and title).
//! Each gives the changes it would make, for the review table; nothing is
//! written here.

use djsugar_engine::Track;
use regex::{NoExpand, Regex, RegexBuilder};

use crate::cleanup::{Change, Field};

/// The names a tool works on when told "all names".
pub const NAMES: [Field; 4] = [Field::Title, Field::Artist, Field::Remix, Field::Album];

/// How Change Case writes a name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    /// "The Night Is Young", small words lower, "DJ" and "UK" kept.
    Title,
    /// "The night is young".
    Sentence,
    Upper,
    Lower,
}

/// A tool and how it is set.
#[derive(Clone, Debug)]
pub enum Tool {
    Replace {
        find: String,
        with: String,
        match_case: bool,
        whole_words: bool,
        /// `find` is a regular expression, and `with` can use `$1`.
        pattern: bool,
    },
    Case(Case),
    /// Takes out what's in round and square brackets.
    RemoveBrackets,
    /// Artist and title the other way round.
    Swap,
}

/// Words that stay lower case inside a title-cased name.
const SMALL: [&str; 24] = [
    "a", "an", "and", "as", "at", "but", "by", "for", "from", "in", "into", "nor", "of", "on",
    "or", "the", "to", "with", "vs", "vs.", "feat.", "ft.", "x", "&",
];

/// Words that stay in capitals.
const CAPITALS: [&str; 14] = [
    "DJ", "MC", "UK", "USA", "NYC", "EP", "LP", "VIP", "TV", "BPM", "II", "III", "IV", "XL",
];

/// The expression `Replace` finds, or why it can't be used.
fn finder(find: &str, match_case: bool, whole_words: bool, pattern: bool) -> Result<Regex, String> {
    let mut expr = if pattern {
        find.to_string()
    } else {
        regex::escape(find)
    };
    if whole_words {
        expr = format!(r"\b(?:{expr})\b");
    }
    RegexBuilder::new(&expr)
        .case_insensitive(!match_case)
        .build()
        .map_err(|e| {
            let why = e.to_string();
            let last = why.lines().last().unwrap_or("").trim().to_string();
            format!("The pattern doesn't work: {last}")
        })
}

/// A word written with capitals inside it on purpose ("McCartney",
/// "deadmau5" is not), which case changes leave alone.
fn mixed(word: &str) -> bool {
    let letters: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).collect();
    letters.len() > 1
        && letters[1..].iter().any(|c| c.is_uppercase())
        && letters.iter().any(|c| c.is_lowercase())
}

/// `word` with its first letter (after any brackets or quotes) upper case
/// and the rest lower.
fn capitalised(word: &str) -> String {
    let mut out = String::new();
    let mut first = true;
    for c in word.chars() {
        if first && c.is_alphabetic() {
            out.extend(c.to_uppercase());
            first = false;
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out
}

/// The word as it is written in capitals, when it is one of them.
fn capitals(word: &str) -> Option<String> {
    let bare: String = word.chars().filter(|c| c.is_alphanumeric()).collect();
    CAPITALS
        .iter()
        .find(|c| c.eq_ignore_ascii_case(&bare))
        .map(|c| word.replacen(&bare, c, 1))
}

/// `s` in case `case`.
pub fn change_case(s: &str, case: Case) -> String {
    match case {
        Case::Upper => s.to_uppercase(),
        Case::Lower => s.to_lowercase(),
        Case::Title | Case::Sentence => {
            let mut out = Vec::new();
            // A new part starts the name, and after an opening bracket or
            // a dash: its first word is capitalised.
            let mut starts = true;
            for word in s.split(' ') {
                let lw = word.to_lowercase();
                let w = if word.is_empty() {
                    String::new()
                } else if mixed(word) {
                    word.to_string()
                } else if let Some(c) = capitals(word) {
                    c
                } else if starts || word.starts_with(['(', '[']) {
                    capitalised(word)
                } else if case == Case::Sentence || SMALL.contains(&lw.as_str()) {
                    lw
                } else {
                    capitalised(word)
                };
                if !word.is_empty() {
                    starts = word == "-" || word.ends_with(['(', '[', ':']);
                }
                out.push(w);
            }
            out.join(" ")
        }
    }
}

/// `s` without what's in round and square brackets.
pub fn remove_brackets(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for c in s.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A tool made ready to run on many tracks.
pub struct Ready {
    tool: Tool,
    finder: Option<Regex>,
}

impl Ready {
    /// Gets `tool` ready, or says why it can't run.
    pub fn new(tool: Tool) -> Result<Self, String> {
        let finder = match &tool {
            Tool::Replace {
                find,
                match_case,
                whole_words,
                pattern,
                ..
            } => {
                if find.is_empty() {
                    return Err("Type what to find.".into());
                }
                Some(finder(find, *match_case, *whole_words, *pattern)?)
            }
            _ => None,
        };
        Ok(Self { tool, finder })
    }

    /// What the tool makes of `value`.
    fn fix(&self, value: &str) -> String {
        match (&self.tool, &self.finder) {
            (Tool::Replace { with, pattern, .. }, Some(re)) => {
                let out = if *pattern {
                    re.replace_all(value, with.as_str())
                } else {
                    re.replace_all(value, NoExpand(with))
                };
                // Taking out a word leaves no double spaces behind.
                out.split(' ')
                    .filter(|w| !w.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")
                    .trim()
                    .to_string()
            }
            (Tool::Case(case), _) => change_case(value, *case),
            (Tool::RemoveBrackets, _) => remove_brackets(value),
            _ => value.to_string(),
        }
    }

    /// The changes the tool makes to `fields` of `t`.
    pub fn changes(&self, t: &Track, fields: &[Field]) -> Vec<Change> {
        if let Tool::Swap = self.tool {
            let (artist, title) = (t.artist.trim(), t.title.trim());
            if artist == title || artist.is_empty() || title.is_empty() {
                return Vec::new();
            }
            return vec![
                Change {
                    field: Field::Artist,
                    old: t.artist.clone(),
                    new: title.to_string(),
                },
                Change {
                    field: Field::Title,
                    old: t.title.clone(),
                    new: artist.to_string(),
                },
            ];
        }
        fields
            .iter()
            .filter_map(|&field| {
                let old = field.get(t);
                let new = self.fix(&old);
                // Nothing to change, and a title or artist is never emptied.
                let emptied = new.is_empty() && matches!(field, Field::Title | Field::Artist);
                (new != old && !emptied).then(|| Change {
                    field,
                    old: old.to_string(),
                    new,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use djsugar_engine::Source;

    fn track(artist: &str, title: &str) -> Track {
        Track {
            id: 1,
            artist: artist.into(),
            title: title.into(),
            album: String::new(),
            genre: String::new(),
            rating: 0,
            bpm: 0.0,
            key: String::new(),
            duration: 0.0,
            first_beat: 0.0,
            source: Source::Synth { seed: 1 },
            waveform: Default::default(),
            audio: None,
            cover: None,
            grid: Default::default(),
            info: Default::default(),
        }
    }

    fn replace(find: &str, with: &str, match_case: bool, whole: bool, pattern: bool) -> Ready {
        Ready::new(Tool::Replace {
            find: find.into(),
            with: with.into(),
            match_case,
            whole_words: whole,
            pattern,
        })
        .unwrap()
    }

    #[test]
    fn finds_and_replaces() {
        let r = replace("(Original Mix)", "", false, false, false);
        assert_eq!(r.fix("Road (original mix) [Live]"), "Road [Live]");
        let r = replace("mix", "Edit", true, false, false);
        assert_eq!(r.fix("Club mix Mix"), "Club Edit Mix", "match case");
        let r = replace("in", "IN", false, true, false);
        assert_eq!(r.fix("Lost in Spinning"), "Lost IN Spinning", "whole words");
        let r = replace(r"^(\d+)\s*-\s*", "", false, false, true);
        assert_eq!(r.fix("01 - Home"), "Home", "pattern");
        let r = replace(r"(\w+) & (\w+)", "$2 & $1", false, false, true);
        assert_eq!(r.fix("Tycho & Bonobo"), "Bonobo & Tycho", "groups");
        let r = replace("$1", "x", false, false, false);
        assert_eq!(r.fix("a $1 b"), "a x b", "plain text is literal");
        assert!(Ready::new(Tool::Replace {
            find: "(".into(),
            with: String::new(),
            match_case: false,
            whole_words: false,
            pattern: true,
        })
        .is_err());
        assert!(Ready::new(Tool::Replace {
            find: String::new(),
            with: String::new(),
            match_case: false,
            whole_words: false,
            pattern: false,
        })
        .is_err());
    }

    #[test]
    fn changes_case() {
        assert_eq!(
            change_case("THE NIGHT IS YOUNG (DJ KOZE REMIX)", Case::Title),
            "The Night Is Young (DJ Koze Remix)"
        );
        assert_eq!(
            change_case("a tribe called quest feat. the roots", Case::Title),
            "A Tribe Called Quest feat. the Roots"
        );
        assert_eq!(
            change_case("McCartney in the uk", Case::Title),
            "McCartney in the UK"
        );
        assert_eq!(
            change_case("Lost In The Night - The Remix", Case::Sentence),
            "Lost in the night - The remix"
        );
        assert_eq!(change_case("Road", Case::Upper), "ROAD");
        assert_eq!(change_case("ROAD", Case::Lower), "road");
    }

    #[test]
    fn removes_brackets() {
        assert_eq!(remove_brackets("Road (Original Mix) [Live]"), "Road");
        assert_eq!(remove_brackets("A (b (c)) d"), "A d");
        assert_eq!(remove_brackets("No brackets"), "No brackets");
    }

    #[test]
    fn proposes_changes_never_emptying_names() {
        let t = track("Road", "Lane 8");
        let swap = Ready::new(Tool::Swap).unwrap();
        let c = swap.changes(&t, &[]);
        assert_eq!(c.len(), 2);
        assert_eq!((c[0].new.as_str(), c[1].new.as_str()), ("Lane 8", "Road"));
        let t = track("Lane 8", "(Intro)");
        let brackets = Ready::new(Tool::RemoveBrackets).unwrap();
        assert!(brackets.changes(&t, &NAMES).is_empty(), "the title stays");
        let t = track("lane 8", "road");
        let title = Ready::new(Tool::Case(Case::Title)).unwrap();
        let fields: Vec<Field> = title.changes(&t, &NAMES).iter().map(|c| c.field).collect();
        assert_eq!(fields, [Field::Title, Field::Artist]);
    }
}

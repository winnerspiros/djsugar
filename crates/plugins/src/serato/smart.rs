//! Serato's smart crates, `SmartCrates/*.scrate` next to `database V2`:
//! the same runs of fields as a crate, with the rules first. `rart` says
//! whether every rule must match (else any one), and each `rurt` is a rule:
//! `trft` how it compares (`cond_con_str` contains, `cond_is_str` is,
//! `cond_greq_uint` at least, `cond_aft_time` after a date, …), `urkt`
//! the field by number, and the value as text (`trpt`), a date (`trtt`,
//! month/day/year) or a number. The tracks Serato last found are saved
//! after the rules as `otrk` records, as in a crate.
//!
//! A smart crate comes in as a smart list: the rules as a search in DJ
//! Sugar's syntax. One whose rules the search can't say comes in as a plain
//! list of the tracks Serato last found for it.

use std::path::{Path, PathBuf};

use djsugar_engine::import::{ImportedEntry, ImportedListKind};

use super::{fields, file, number, records, text};
use crate::ImportedList;

/// The `.scrate` files in the `SmartCrates` folder next to `database`.
pub fn files(database: &Path) -> Vec<PathBuf> {
    let dir = database
        .parent()
        .unwrap_or(Path::new(""))
        .join("SmartCrates");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "scrate"))
        .collect();
    files.sort();
    files
}

/// A smart crate, its folders from the file's name (`House%%Deep`).
pub fn list(path: &Path, disk: &Path) -> Option<ImportedList> {
    let stem = path.file_stem()?.to_string_lossy().into_owned();
    let mut folders: Vec<String> = stem.split("%%").map(str::to_string).collect();
    let name = folders.pop()?;
    let data = std::fs::read(path).ok()?;
    let kind = match query(&data) {
        Some(query) => ImportedListKind::Smart { query },
        None => ImportedListKind::List,
    };
    let entries = if matches!(kind, ImportedListKind::List) {
        records(&data, b"otrk")
            .filter_map(|r| {
                let (_, v) = fields(r).find(|(n, _)| *n == b"ptrk")?;
                Some(ImportedEntry::new(file(&text(v), disk), None))
            })
            .collect()
    } else {
        Vec::new()
    };
    Some(ImportedList {
        folders,
        name,
        kind,
        entries,
    })
}

/// The rules as one search, or `None` if one of them can't be said.
pub fn query(data: &[u8]) -> Option<String> {
    let all = records(data, b"rart")
        .next()
        .and_then(|r| fields(r).find(|(n, _)| *n == b"brut"))
        .is_none_or(|(_, v)| v.first() != Some(&0));
    let terms: Vec<String> = records(data, b"rurt").map(rule).collect::<Option<_>>()?;
    if terms.is_empty() {
        return None;
    }
    Some(terms.join(if all { " " } else { " | " }))
}

/// Serato's field numbers, as the search names them.
fn field(n: u32) -> Option<&'static str> {
    Some(match n {
        4 => "location",
        6 => "title",
        7 => "artist",
        8 => "album",
        9 => "genre",
        15 => "bpm",
        17 => "comment",
        19 => "grouping",
        20 => "remixer",
        21 => "publisher",
        22 => "composer",
        23 => "year",
        25 => "added",
        51 => "key",
        79 => "played",
        _ => return None,
    })
}

/// "5/13/2020" (or already "2020-05-13") as 2020-05-13.
fn date(v: &str) -> Option<String> {
    let v = v.trim();
    let parts: Vec<u32> = v
        .split(['/', '-', '.'])
        .map(|p| p.trim().parse().ok())
        .collect::<Option<_>>()?;
    let (y, m, d) = match parts[..] {
        [y, m, d] if y > 31 => (y, m, d),
        [m, d, y] => (y, m, d),
        _ => return None,
    };
    ((1..=12).contains(&m) && (1..=31).contains(&d) && y > 1900)
        .then(|| format!("{y:04}-{m:02}-{d:02}"))
}

fn quoted(v: &str) -> String {
    let v = v.replace('"', "");
    if v.is_empty() || v.contains(char::is_whitespace) {
        format!("\"{v}\"")
    } else {
        v
    }
}

fn rule(rec: &[u8]) -> Option<String> {
    let mut how = String::new();
    let mut what = None;
    let mut value = None;
    let mut when = None;
    for (name, v) in fields(rec) {
        match name {
            b"trft" => how = text(v),
            b"urkt" => what = number(v),
            b"trpt" => value = Some(text(v)),
            b"trtt" => when = Some(text(v)),
            [b'u', ..] => value = value.or_else(|| number(v).map(|n| n.to_string())),
            _ => {}
        }
    }
    let f = field(what?)?;
    let numeric = matches!(f, "bpm" | "year" | "played");
    let v = || value.clone().filter(|v| !v.trim().is_empty());
    match how.as_str() {
        "cond_con_str" if !numeric => Some(format!("{f}:{}", quoted(&v()?))),
        "cond_dnc_str" if !numeric => Some(format!("-{f}:{}", quoted(&v()?))),
        "cond_is_str" if numeric || f == "key" => Some(format!("{f}:{}", v()?.trim())),
        "cond_isn_str" if numeric || f == "key" => Some(format!("-{f}:{}", v()?.trim())),
        "cond_is_str" => Some(format!("{f}:={}", quoted(&v()?))),
        "cond_isn_str" => Some(format!("-{f}:={}", quoted(&v()?))),
        "cond_greq_uint" if numeric => Some(format!("{f}:>={}", v()?.trim())),
        "cond_lseq_uint" if numeric => Some(format!("{f}:<={}", v()?.trim())),
        "cond_bef_time" | "cond_bef_str" if f == "added" => {
            Some(format!("added:<{}", date(&when.or_else(v)?)?))
        }
        "cond_aft_time" | "cond_aft_str" if f == "added" => {
            Some(format!("added:>{}", date(&when.or_else(v)?)?))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::export::{field as put, utf16};
    use super::*;

    fn rule(how: &str, what: u32, value: &[u8]) -> Vec<u8> {
        let mut r = put(b"trft", &utf16(how));
        r.extend(put(b"urkt", &what.to_be_bytes()));
        r.extend(value);
        put(b"rurt", &r)
    }

    /// Laid out as the smart crate Serato 3 saved that serato-tools keeps
    /// as test data: match all, then four rules, then the tracks.
    fn scrate(all: bool, rules: &[Vec<u8>], tracks: &[&str]) -> Vec<u8> {
        let mut d = put(b"vrsn", &utf16("1.0/Serato ScratchLive Smart Crate"));
        d.extend(put(b"rart", &put(b"brut", &[u8::from(all)])));
        d.extend(put(b"rlut", &put(b"brut", &[1])));
        for r in rules {
            d.extend(r);
        }
        for t in tracks {
            d.extend(put(b"otrk", &put(b"ptrk", &utf16(t))));
        }
        d
    }

    #[test]
    fn broken_smart_crates_are_turned_down() {
        let rules = [
            rule("cond_aft_time", 25, &put(b"trtt", &utf16("5/13/2020"))),
            rule("cond_greq_uint", 15, &put(b"urpt", &124u32.to_be_bytes())),
        ];
        crate::garble::check(&[scrate(true, &rules, &["a.mp3"])], |b| {
            let _ = query(b);
        });
    }

    #[test]
    fn rules_as_a_search() {
        let rules = [
            rule("cond_aft_time", 25, &put(b"trtt", &utf16("5/13/2020"))),
            rule("cond_dnc_str", 19, &put(b"trpt", &utf16("GRP"))),
            rule("cond_is_str", 23, &put(b"trpt", &utf16("2000"))),
            rule("cond_con_str", 21, &put(b"trpt", &utf16("Big Beat"))),
            rule("cond_greq_uint", 15, &put(b"urpt", &124u32.to_be_bytes())),
        ];
        assert_eq!(
            query(&scrate(true, &rules, &[])).as_deref(),
            Some("added:>2020-05-13 -grouping:GRP year:2000 publisher:\"Big Beat\" bpm:>=124")
        );
        assert_eq!(
            query(&scrate(false, &rules[1..3], &[])).as_deref(),
            Some("-grouping:GRP | year:2000")
        );
        // A field the search has no name for.
        let odd = [rule("cond_con_str", 999, &put(b"trpt", &utf16("x")))];
        assert_eq!(query(&scrate(true, &odd, &[])), None);
    }

    #[test]
    fn reads_the_folder() {
        let dir = std::env::temp_dir().join(format!("djs-scrate-{}", std::process::id()));
        let serato = dir.join("_Serato_");
        let smart = serato.join("SmartCrates");
        std::fs::create_dir_all(&smart).unwrap();
        let rules = [rule("cond_con_str", 9, &put(b"trpt", &utf16("House")))];
        std::fs::write(
            smart.join("Genres%%House.scrate"),
            scrate(true, &rules, &["Music/a.mp3"]),
        )
        .unwrap();
        let odd = [rule("cond_con_str", 999, &put(b"trpt", &utf16("x")))];
        std::fs::write(
            smart.join("Odd.scrate"),
            scrate(true, &odd, &["Music/b.mp3"]),
        )
        .unwrap();
        let database = serato.join("database V2");
        let found = files(&database);
        let lists: Vec<ImportedList> = found.iter().filter_map(|f| list(f, &dir)).collect();
        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(lists.len(), 2);
        assert_eq!(lists[0].folders, ["Genres"]);
        assert_eq!(lists[0].name, "House");
        assert_eq!(
            lists[0].kind,
            ImportedListKind::Smart {
                query: "genre:House".into()
            }
        );
        assert!(lists[0].entries.is_empty());
        assert_eq!(lists[1].kind, ImportedListKind::List);
        assert_eq!(lists[1].entries[0].path, dir.join("Music").join("b.mp3"));
    }
}

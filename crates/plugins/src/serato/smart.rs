//! Parses Serato's `.scrate` files (smart crates in `SmartCrates/`).
//!
//! Smart crates are saved searches with rules that match tracks by metadata
//! fields. They use the same TLV format.

use std::path::{Path, PathBuf};

use crate::serato::database::read_utf16be;
use crate::{ImportedEntry, ImportedList, ImportedListKind};
use std::collections::HashMap;

/// A smart crate rule.
#[derive(Debug, Clone)]
pub struct SmartRule {
    /// Field to match on (see `SmartCrate::FIELD_NAMES`).
    pub field: u32,
    /// Operator: 0=contains, 1=is, 2=does not contain, 3=is not.
    pub op: u32,
    /// Value to match.
    pub value: String,
}

/// A parsed smart crate.
#[derive(Debug, Clone)]
pub struct SmartCrate {
    pub name: String,
    pub rules: Vec<SmartRule>,
    pub match_all: bool, // true = AND, false = OR
}

/// Field indices used in smart crate rules (from Serato's UI).
pub const FIELD_NAMES: [&str; 14] = [
    "Added",    // 0
    "Album",    // 1
    "Artist",   // 2
    "BPM",      // 3
    "Comment",  // 4
    "Composer", // 5
    "Filename", // 6
    "Genre",    // 7
    "Grouping", // 8
    "Key",      // 9
    "Label",    // 10
    "Remixer",  // 11
    "Song",     // 12
    "Year",     // 13
];

/// Operator indices.
pub const OP_NAMES: [&str; 4] = ["contains", "is", "does not contain", "is not"];

/// Reads all `.scrate` files in `SmartCrates/`.
pub fn read_smart_crates(root: &Path) -> Result<Vec<SmartCrate>, String> {
    let smart_dir = root.join("SmartCrates");
    if !smart_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut crates = Vec::new();
    for entry in std::fs::read_dir(&smart_dir)
        .map_err(|e| e.to_string())?
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("scrate") {
            if let Ok(sc) = parse_smart_crate(&path) {
                crates.push(sc);
            }
        }
    }
    crates.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(crates)
}

fn parse_smart_crate(path: &Path) -> Result<SmartCrate, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let mut cursor = Cursor::new(&data);

    let mut name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let mut rules = Vec::new();
    let mut match_all = true;

    while cursor.remaining() >= 8 {
        let tag = cursor.read_tag()?;
        let len = cursor.read_u32()? as usize;
        if cursor.remaining() < len {
            break;
        }
        let value = cursor.read_bytes(len)?;

        match &tag {
            b"vrsn" => {} // version
            b"scnm" => name = read_utf16be(value),
            b"scma" => {
                // Match all (AND) vs any (OR)
                if value.len() == 4 {
                    match_all = u32::from_be_bytes([value[0], value[1], value[2], value[3]]) != 0;
                }
            }
            b"scrl" => {
                // Rule count (we parse rules via scr tags)
            }
            b"scr" => {
                let rule = parse_rule(value)?;
                rules.push(rule);
            }
            _ => {}
        }
    }

    Ok(SmartCrate {
        name,
        rules,
        match_all,
    })
}

fn parse_rule(data: &[u8]) -> Result<SmartRule, String> {
    let mut cursor = Cursor::new(data);
    let mut field = 0;
    let mut op = 0;
    let mut value = String::new();

    while cursor.remaining() >= 8 {
        let tag = cursor.read_tag()?;
        let len = cursor.read_u32()? as usize;
        if cursor.remaining() < len {
            break;
        }
        let val = cursor.read_bytes(len)?;

        match &tag {
            b"scrf" => {
                if val.len() == 4 {
                    field = u32::from_be_bytes([val[0], val[1], val[2], val[3]]);
                }
            }
            b"scro" => {
                if val.len() == 4 {
                    op = u32::from_be_bytes([val[0], val[1], val[2], val[3]]);
                }
            }
            b"scrv" => value = read_utf16be(val),
            _ => {}
        }
    }

    Ok(SmartRule { field, op, value })
}

/// Converts a smart crate to an imported list (as a smart list with a query).
pub fn smart_crate_to_list(
    sc: &SmartCrate,
    track_by_path: &HashMap<PathBuf, usize>,
) -> Result<Vec<ImportedList>, String> {
    let query = build_smart_query(sc);
    let entries = Vec::new(); // Smart lists don't have static entries

    Ok(vec![ImportedList {
        folders: Vec::new(),
        name: sc.name.clone(),
        kind: ImportedListKind::Smart { query },
        entries,
    }])
}

fn build_smart_query(sc: &SmartCrate) -> String {
    let mut parts = Vec::new();
    for rule in &sc.rules {
        let field = FIELD_NAMES.get(rule.field as usize).unwrap_or(&"Song");
        let op = OP_NAMES.get(rule.op as usize).unwrap_or(&"contains");
        let value = &rule.value;

        let query_part = match op {
            "contains" => format!("{}:{}", field.to_lowercase(), escape_query(value)),
            "is" => format!("{}:=\"{}\"", field.to_lowercase(), escape_query(value)),
            "does not contain" => format!("-{}:{}", field.to_lowercase(), escape_query(value)),
            "is not" => format!("-{}:=\"{}\"", field.to_lowercase(), escape_query(value)),
            _ => format!("{}:{}", field.to_lowercase(), escape_query(value)),
        };
        parts.push(query_part);
    }

    if sc.match_all {
        parts.join(" ")
    } else {
        parts.join(" OR ")
    }
}

fn escape_query(s: &str) -> String {
    if s.contains(' ') || s.contains(':') || s.contains('"') {
        format!("\"{}\"", s.replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn read_tag(&mut self) -> Result<[u8; 4], String> {
        if self.remaining() < 4 {
            return Err("truncated tag".into());
        }
        let tag = [
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ];
        self.pos += 4;
        Ok(tag)
    }

    fn read_u32(&mut self) -> Result<u32, String> {
        if self.remaining() < 4 {
            return Err("truncated u32".into());
        }
        let v = u32::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], String> {
        if self.remaining() < len {
            return Err("truncated value".into());
        }
        let start = self.pos;
        self.pos += len;
        Ok(&self.data[start..self.pos])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    fn be_u32(v: u32) -> [u8; 4] {
        v.to_be_bytes()
    }

    fn utf16be(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(|c| c.to_be_bytes()).collect()
    }

    fn tlv(tag: &[u8; 4], value: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(tag);
        out.extend_from_slice(&be_u32(value.len() as u32));
        out.extend_from_slice(value);
        out
    }

    #[test]
    fn test_smart_crate() {
        let dir = tempdir().unwrap();
        let smart_dir = dir.path().join("SmartCrates");
        std::fs::create_dir(&smart_dir).unwrap();
        let sc_path = smart_dir.join("House Tracks.scrate");

        let mut data = Vec::new();
        data.extend_from_slice(&tlv(b"vrsn", &utf16be("2.0")));
        data.extend_from_slice(&tlv(b"scnm", &utf16be("House Tracks")));
        data.extend_from_slice(&tlv(b"scma", &be_u32(1))); // match all
        data.extend_from_slice(&tlv(b"scrl", &be_u32(2))); // rule count
                                                           // Rule 1: Genre contains House
        let mut rule1 = Vec::new();
        rule1.extend_from_slice(&tlv(b"scrf", &be_u32(7))); // Genre
        rule1.extend_from_slice(&tlv(b"scro", &be_u32(0))); // contains
        rule1.extend_from_slice(&tlv(b"scrv", &utf16be("House")));
        data.extend_from_slice(&tlv(b"scr", &rule1));
        // Rule 2: BPM is 120-130 (simplified as contains "12")
        let mut rule2 = Vec::new();
        rule2.extend_from_slice(&tlv(b"scrf", &be_u32(3))); // BPM
        rule2.extend_from_slice(&tlv(b"scro", &be_u32(0))); // contains
        rule2.extend_from_slice(&tlv(b"scrv", &utf16be("12")));
        data.extend_from_slice(&tlv(b"scr", &rule2));

        std::fs::write(&sc_path, &data).unwrap();

        let crates = read_smart_crates(dir.path()).unwrap();
        assert_eq!(crates.len(), 1);
        assert_eq!(crates[0].name, "House Tracks");
        assert_eq!(crates[0].rules.len(), 2);
        assert!(crates[0].match_all);
    }

    #[test]
    fn test_query_building() {
        let sc = SmartCrate {
            name: "Test".into(),
            rules: vec![
                SmartRule {
                    field: 7,
                    op: 0,
                    value: "House".into(),
                }, // Genre contains House
                SmartRule {
                    field: 3,
                    op: 1,
                    value: "128".into(),
                }, // BPM is 128
            ],
            match_all: true,
        };
        let query = build_smart_query(&sc);
        assert_eq!(query, "genre:House bpm:=\"128\"");
    }
}

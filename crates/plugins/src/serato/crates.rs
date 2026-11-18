//! Parses Serato's `.crate` files (regular crates in `Subcrates/`).
//!
//! Crate files use the same TLV format as `database V2` but with `ptrk`
//! (playlist track) instead of `pfil` for track references. They also have
//! `osrt` for sort order and `ovct` for visible columns.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::serato::database::read_utf16be;
use crate::{ImportedEntry, ImportedList, ImportedListKind};

/// A parsed crate.
#[derive(Debug, Clone)]
pub struct Crate {
    pub name: String,
    pub tracks: Vec<String>,  // volume-relative paths (ptrk values)
    pub folders: Vec<String>, // folder path from %% in filename
}

/// Reads all `.crate` files in `Subcrates/`, building the folder tree from
/// `%%` in filenames (e.g., "Genre%%House.crate" -> folder "Genre", crate "House").
pub fn read_crates(root: &Path) -> Result<Vec<Crate>, String> {
    let subcrate_dir = root.join("Subcrates");
    if !subcrate_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut crates = Vec::new();
    for entry in std::fs::read_dir(&subcrate_dir)
        .map_err(|e| e.to_string())?
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("crate") {
            if let Ok(krate) = parse_crate(&path) {
                crates.push(krate);
            }
        }
    }
    crates.sort_by(|a, b| a.folders.cmp(&b.folders).then_with(|| a.name.cmp(&b.name)));
    Ok(crates)
}

fn parse_crate(path: &Path) -> Result<Crate, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let mut cursor = Cursor::new(&data);

    let mut name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let mut tracks = Vec::new();

    // Parse folder from %% in filename
    let folders = if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        stem.split("%%").map(|s| s.to_string()).collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    // The actual crate name is the last component after %%
    if let Some(last) = folders.last() {
        name = last.clone();
    }

    while cursor.remaining() >= 8 {
        let tag = cursor.read_tag()?;
        let len = cursor.read_u32()? as usize;
        if cursor.remaining() < len {
            break;
        }
        let value = cursor.read_bytes(len)?;

        match &tag {
            b"otrk" => {
                // Parse ptrk inside otrk
                let mut sub = Cursor::new(value);
                while sub.remaining() >= 8 {
                    let stag = sub.read_tag()?;
                    let slen = sub.read_u32()? as usize;
                    if sub.remaining() < slen {
                        break;
                    }
                    let sval = sub.read_bytes(slen)?;
                    if stag == *b"ptrk" {
                        tracks.push(read_utf16be(sval));
                    }
                }
            }
            _ => {}
        }
    }

    Ok(Crate {
        name,
        tracks,
        folders: if folders.len() > 1 {
            folders[..folders.len() - 1].to_vec()
        } else {
            Vec::new()
        },
    })
}

/// Converts a crate to imported lists.
pub fn crate_to_lists(
    krate: &Crate,
    track_by_path: &HashMap<PathBuf, usize>,
    root: &Path,
) -> Result<Vec<ImportedList>, String> {
    let mut entries = Vec::new();
    for ptrk in &krate.tracks {
        let path = PathBuf::from(ptrk);
        // Resolve relative to the volume root (parent of _Serato_)
        let volume_root = root.parent().unwrap_or(root);
        let full_path = volume_root.join(&path);
        if let Some(&id) = track_by_path.get(&full_path) {
            entries.push(ImportedEntry::new(full_path, None));
        } else {
            // Track not found locally, add as placeholder
            entries.push(ImportedEntry::new(full_path, None));
        }
    }

    Ok(vec![ImportedList {
        folders: krate.folders.clone(),
        name: krate.name.clone(),
        kind: ImportedListKind::List,
        entries,
    }])
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
    fn test_crate_with_folders() {
        let dir = tempdir().unwrap();
        let subcrate_dir = dir.path().join("Subcrates");
        std::fs::create_dir(&subcrate_dir).unwrap();
        let crate_path = subcrate_dir.join("Electronic%%House.crate");

        let mut data = Vec::new();
        data.extend_from_slice(&tlv(b"vrsn", &utf16be("2.0")));
        data.extend_from_slice(&tlv(b"osrt", &be_u32(0)));
        let mut track_data = Vec::new();
        track_data.extend_from_slice(&tlv(b"ptrk", &utf16be("Music/Track1.mp3")));
        track_data.extend_from_slice(&tlv(b"ptrk", &utf16be("Music/Track2.mp3")));
        data.extend_from_slice(&tlv(b"otrk", &track_data));

        std::fs::write(&crate_path, &data).unwrap();

        let crates = read_crates(dir.path()).unwrap();
        assert_eq!(crates.len(), 1);
        assert_eq!(crates[0].name, "House");
        assert_eq!(crates[0].folders, vec!["Electronic"]);
        assert_eq!(crates[0].tracks.len(), 2);
    }

    #[test]
    fn test_crate_no_folders() {
        let dir = tempdir().unwrap();
        let subcrate_dir = dir.path().join("Subcrates");
        std::fs::create_dir(&subcrate_dir).unwrap();
        let crate_path = subcrate_dir.join("My Crate.crate");

        let mut data = Vec::new();
        data.extend_from_slice(&tlv(b"vrsn", &utf16be("2.0")));
        let mut track_data = Vec::new();
        track_data.extend_from_slice(&tlv(b"ptrk", &utf16be("Music/Track.mp3")));
        data.extend_from_slice(&tlv(b"otrk", &track_data));

        std::fs::write(&crate_path, &data).unwrap();

        let crates = read_crates(dir.path()).unwrap();
        assert_eq!(crates.len(), 1);
        assert_eq!(crates[0].name, "My Crate");
        assert!(crates[0].folders.is_empty());
    }
}

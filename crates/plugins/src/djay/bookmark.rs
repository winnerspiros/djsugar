//! The file a macOS bookmark (`CFURLBookmarkData`) points at, read from the
//! path it saved, without asking macOS. djay Pro on a Mac keeps where each
//! track is as a bookmark.
//!
//! A bookmark is `book`, its length, a version and the size of its header;
//! after the header, the offset of its table of contents. The table lists
//! items by key, each at an offset from the end of the header: an item is
//! its length, its type and its bytes. Key `0x1004` is the path, an array
//! (type `0x0601`) of offsets to its folder and file names, each a string
//! (type `0x0101`).

use std::path::PathBuf;

const PATH: u32 = 0x1004;
const STRING: u32 = 0x0101;
const ARRAY: u32 = 0x0601;
const TOC_MAGIC: u32 = 0xFFFF_FFFE;

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    let bytes = b.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

/// An item: its type and bytes.
fn item(data: &[u8], offset: usize) -> Option<(u32, &[u8])> {
    let len = u32_at(data, offset)? as usize;
    let kind = u32_at(data, offset + 4)?;
    let start = offset + 8;
    Some((kind, data.get(start..start.checked_add(len)?)?))
}

/// The path a bookmark saved, if it is one.
pub fn path(bookmark: &[u8]) -> Option<PathBuf> {
    if bookmark.get(..4)? != b"book" {
        return None;
    }
    let header = u32_at(bookmark, 12)? as usize;
    let data = bookmark.get(header..)?;
    let mut toc = u32_at(data, 0)? as usize;
    // Tables chain to the next; a bookmark has one or two.
    for _ in 0..8 {
        if u32_at(data, toc + 4)? != TOC_MAGIC {
            return None;
        }
        let next = u32_at(data, toc + 12)? as usize;
        let count = u32_at(data, toc + 16)? as usize;
        for i in 0..count.min(256) {
            let entry = toc + 20 + i * 12;
            if u32_at(data, entry)? != PATH {
                continue;
            }
            let (kind, list) = item(data, u32_at(data, entry + 4)? as usize)?;
            if kind != ARRAY {
                return None;
            }
            let mut path = PathBuf::from("/");
            for at in list.as_chunks::<4>().0 {
                let (kind, name) = item(data, u32::from_le_bytes(*at) as usize)?;
                if kind != STRING {
                    return None;
                }
                path.push(std::str::from_utf8(name).ok()?);
            }
            return Some(path);
        }
        if next == 0 {
            break;
        }
        toc = next;
    }
    None
}

/// Makes a bookmark holding `parts` as its path, for tests.
#[cfg(test)]
pub fn make(parts: &[&str]) -> Vec<u8> {
    let mut data: Vec<u8> = vec![0; 4];
    let mut offsets = Vec::new();
    let pad = |d: &mut Vec<u8>| {
        while !d.len().is_multiple_of(4) {
            d.push(0);
        }
    };
    for p in parts {
        offsets.push(data.len() as u32);
        data.extend_from_slice(&(p.len() as u32).to_le_bytes());
        data.extend_from_slice(&STRING.to_le_bytes());
        data.extend_from_slice(p.as_bytes());
        pad(&mut data);
    }
    let array = data.len() as u32;
    data.extend_from_slice(&((offsets.len() * 4) as u32).to_le_bytes());
    data.extend_from_slice(&ARRAY.to_le_bytes());
    for o in &offsets {
        data.extend_from_slice(&o.to_le_bytes());
    }
    let toc = data.len() as u32;
    data[..4].copy_from_slice(&toc.to_le_bytes());
    for v in [20 + 12, TOC_MAGIC, 1, 0, 1, PATH, array, 0] {
        data.extend_from_slice(&v.to_le_bytes());
    }
    let mut out = b"book".to_vec();
    out.extend_from_slice(&((48 + data.len()) as u32).to_le_bytes());
    out.extend_from_slice(&0x1004_0000u32.to_le_bytes());
    out.extend_from_slice(&48u32.to_le_bytes());
    out.resize(48, 0);
    out.extend_from_slice(&data);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_path() {
        let b = make(&["Users", "dj", "Music", "Bicep - Glue.mp3"]);
        assert_eq!(
            path(&b),
            Some(PathBuf::from("/Users/dj/Music/Bicep - Glue.mp3"))
        );
        assert_eq!(path(b"bookjunk"), None);
        assert_eq!(path(b"not a bookmark"), None);
        crate::garble::check(&[b], |b| {
            let _ = path(b);
        });
    }
}

//! A plugin's own presets as files: VST3 `.vstpreset` and AU `.aupreset`, in the folders each format names for them, made into the sound [`crate::Connection::load_state`] takes. - VST3 keeps its
//! presets in `<root>/<maker>/<plugin>/`, the roots being `~/Library/Audio/Presets` on macOS, `Documents/VST3 Presets` on Windows and `~/.vst3/presets` on Linux, plus the machine-wide ones. - AU
//! keeps `.aupreset` files in `~/Library/Audio/Presets/<maker>/<name>/` (and `/Library/...`). One is the unit's class info, as the AU helper saves its sound, so the file is the sound itself.

use std::path::{Path, PathBuf};

/// How deep under a plugin's folder presets are looked for: makers sort
/// them into folders by kind, a level or two down.
const DEPTH: usize = 3;
/// At most this many presets for one plugin.
const MOST: usize = 2000;

/// A plugin's own preset, as found.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// The file's name without its type.
    pub name: String,
    pub path: PathBuf,
}

/// The two parts of a VST3 sound, component then controller, as one block.
pub fn pack_states(component: &[u8], controller: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + component.len() + controller.len());
    for part in [component, controller] {
        out.extend_from_slice(&(part.len() as u32).to_le_bytes());
        out.extend_from_slice(part);
    }
    out
}

/// [`pack_states`] undone.
pub fn unpack_states(data: &[u8]) -> Option<(&[u8], &[u8])> {
    let part = |d: &[u8]| -> Option<(usize, usize)> {
        let n = u32::from_le_bytes(d.get(..4)?.try_into().ok()?) as usize;
        (d.len() >= 4 + n).then_some((4, 4 + n))
    };
    let (a, b) = part(data)?;
    let component = &data[a..b];
    let rest = &data[b..];
    let (c, d) = part(rest)?;
    Some((component, &rest[c..d]))
}

/// The folders a format keeps presets in, before the maker's and plugin's.
fn roots(format: &str) -> Vec<PathBuf> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    let mut out = Vec::new();
    if cfg!(target_os = "macos") {
        if let Some(h) = &home {
            out.push(h.join("Library/Audio/Presets"));
        }
        out.push(PathBuf::from("/Library/Audio/Presets"));
    } else if format == "vst3" && cfg!(windows) {
        if let Some(h) = &home {
            out.push(h.join("Documents").join("VST3 Presets"));
        }
        for var in ["APPDATA", "PROGRAMDATA"] {
            if let Some(d) = std::env::var_os(var) {
                out.push(PathBuf::from(d).join("VST3 Presets"));
            }
        }
    } else if format == "vst3" {
        if let Some(h) = &home {
            out.push(h.join(".vst3/presets"));
        }
        out.push(PathBuf::from("/usr/share/vst3/presets"));
        out.push(PathBuf::from("/usr/local/share/vst3/presets"));
    }
    out
}

/// A name as the formats write it into a folder name: `/` and the like
/// are not allowed there.
fn folder_name(name: &str) -> String {
    name.chars().map(|c| if r#"/\:*?"<>|"#.contains(c) { '_' } else { c }).collect()
}

/// The presets of the plugin `name` by `maker`, in `format` ("vst3" or
/// "au"), by name. `class` is a VST3 plugin's class id: a file made for
/// another class in the same folder is left out.
pub fn find(format: &str, maker: &str, name: &str, class: &str) -> Vec<Found> {
    find_in(&roots(format), format, maker, name, class)
}

fn find_in(roots: &[PathBuf], format: &str, maker: &str, name: &str, class: &str) -> Vec<Found> {
    let ext = match format {
        "vst3" => "vstpreset",
        "au" => "aupreset",
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for root in roots {
        let dir = root.join(folder_name(maker)).join(folder_name(name));
        walk(&dir, ext, DEPTH, &mut out);
    }
    if format == "vst3" {
        out.retain(|f| preset_class(&f.path).is_none_or(|c| same_class(&c, class)));
    }
    out.sort_by_cached_key(|f| (f.name.to_lowercase(), f.path.clone()));
    out.dedup_by(|a, b| a.path == b.path);
    out.truncate(MOST);
    out
}

fn walk(dir: &Path, ext: &str, depth: usize, out: &mut Vec<Found>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        if out.len() >= MOST {
            return;
        }
        let path = e.path();
        if path.is_dir() {
            if depth > 0 {
                walk(&path, ext, depth - 1, out);
            }
        } else if path.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case(ext)) {
            let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            out.push(Found { name, path });
        }
    }
}

/// A `.vstpreset`'s class id, from its header.
fn preset_class(path: &Path) -> Option<String> {
    use std::io::Read;
    let mut head = [0u8; 40];
    std::fs::File::open(path).ok()?.read_exact(&mut head).ok()?;
    (&head[..4] == b"VST3").then(|| String::from_utf8_lossy(&head[8..40]).into_owned())
}

/// Whether two class ids name one class: as written, or with the first
/// three groups' bytes the other way round, as Windows writes them.
fn same_class(a: &str, b: &str) -> bool {
    if a.eq_ignore_ascii_case(b) {
        return true;
    }
    let swap = |s: &str| -> Option<String> {
        let bytes = |r: std::ops::Range<usize>| s.get(r);
        let rev = |p: &str| -> String {
            let pairs: Vec<&str> = (0..p.len()).step_by(2).filter_map(|i| p.get(i..i + 2)).collect();
            pairs.into_iter().rev().collect()
        };
        Some(format!("{}{}{}{}", rev(bytes(0..8)?), rev(bytes(8..12)?), rev(bytes(12..16)?), bytes(16..32)?))
    };
    swap(a).is_some_and(|s| s.eq_ignore_ascii_case(b))
}

/// The sound a `.vstpreset` file holds, as the VST3 helper takes it.
pub fn vst3_state(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let damaged = || "the preset file is damaged".to_string();
    if bytes.get(..4) != Some(b"VST3") {
        return Err("it isn't a VST3 preset".into());
    }
    let int = |at: usize| -> Option<i64> { Some(i64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?)) };
    let list = usize::try_from(int(40).ok_or_else(damaged)?).map_err(|_| damaged())?;
    if bytes.get(list..list + 4) != Some(b"List") {
        return Err(damaged());
    }
    let count =
        i32::from_le_bytes(bytes.get(list + 4..list + 8).ok_or_else(damaged)?.try_into().map_err(|_| damaged())?);
    let (mut component, mut controller) = (None, None);
    for i in 0..usize::try_from(count).map_err(|_| damaged())? {
        let at = list + 8 + i * 20;
        let id = bytes.get(at..at + 4).ok_or_else(damaged)?;
        let (Some(offset), Some(size)) = (int(at + 4), int(at + 12)) else {
            return Err(damaged());
        };
        let (Ok(offset), Ok(size)) = (usize::try_from(offset), usize::try_from(size)) else {
            return Err(damaged());
        };
        let data = bytes.get(offset..offset.checked_add(size).ok_or_else(damaged)?).ok_or_else(damaged)?;
        match id {
            b"Comp" => component = Some(data),
            b"Cont" => controller = Some(data),
            _ => {}
        }
    }
    let component = component.ok_or("the preset holds no sound")?;
    Ok(pack_states(component, controller.unwrap_or_default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `.vstpreset` for `class` holding `comp` and `cont`.
    fn vstpreset(class: &str, comp: &[u8], cont: &[u8]) -> Vec<u8> {
        let mut out = b"VST3".to_vec();
        out.extend_from_slice(&1i32.to_le_bytes());
        out.extend_from_slice(class.as_bytes());
        let list = 48 + comp.len() + cont.len();
        out.extend_from_slice(&(list as i64).to_le_bytes());
        out.extend_from_slice(comp);
        out.extend_from_slice(cont);
        out.extend_from_slice(b"List");
        out.extend_from_slice(&2i32.to_le_bytes());
        for (id, at, len) in [(b"Comp", 48, comp.len()), (b"Cont", 48 + comp.len(), cont.len())] {
            out.extend_from_slice(id);
            out.extend_from_slice(&(at as i64).to_le_bytes());
            out.extend_from_slice(&(len as i64).to_le_bytes());
        }
        out
    }

    const CLASS: &str = "1BA8A477EE0A4A2D80F50D1413D2EAA0";

    #[test]
    fn a_vstpreset_reads_as_the_sound_the_helper_takes() {
        let file = vstpreset(CLASS, b"component", b"ctl");
        let state = vst3_state(&file).unwrap();
        assert_eq!(unpack_states(&state), Some((&b"component"[..], &b"ctl"[..])));
        assert!(vst3_state(b"nope").is_err());
        assert!(vst3_state(&file[..50]).is_err());
    }

    #[test]
    fn a_plugins_presets_are_found_in_its_makers_folder() {
        let root = std::env::temp_dir().join(format!("djsugar-presets-{}", std::process::id()));
        let dir = root.join("Maker Co").join("Synth_One");
        std::fs::create_dir_all(dir.join("Bass")).unwrap();
        std::fs::write(dir.join("Bass/Deep.vstpreset"), vstpreset(CLASS, b"a", b"")).unwrap();
        std::fs::write(dir.join("Airy.VSTPRESET"), vstpreset(CLASS, b"b", b"")).unwrap();
        let other = "00000000000000000000000000000001";
        std::fs::write(dir.join("Other.vstpreset"), vstpreset(other, b"c", b"")).unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        let found = find_in(std::slice::from_ref(&root), "vst3", "Maker Co", "Synth/One", CLASS);
        let names: Vec<&str> = found.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["Airy", "Deep"]);
        // Windows writes the class with its first groups' bytes reversed.
        let windows = "77A4A81B0AEE2D4A80F50D1413D2EAA0";
        assert_eq!(find_in(std::slice::from_ref(&root), "vst3", "Maker Co", "Synth/One", windows).len(), 2);
        assert!(find_in(std::slice::from_ref(&root), "au", "Maker Co", "Synth/One", "").is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}

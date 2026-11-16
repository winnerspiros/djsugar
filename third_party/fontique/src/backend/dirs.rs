// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Fonts on Linux and BSD without fontconfig: the fonts in the XDG data
//! directories, generic families from well-known family names, and fallback
//! by which fonts cover a script's sample text.

use alloc::{string::String, sync::Arc, vec::Vec};
use std::path::PathBuf;

use hashbrown::HashMap;

use super::{
    FallbackKey, FamilyId, FamilyInfo, FamilyNameMap, GenericFamily, GenericFamilyMap, Script,
    ScriptExt, scan,
};

const SANS: &[&str] = &[
    "Noto Sans",
    "DejaVu Sans",
    "Liberation Sans",
    "Cantarell",
    "Ubuntu",
    "Roboto",
    "Open Sans",
    "Inter",
    "FreeSans",
    "Arial",
];
const SERIF: &[&str] = &[
    "Noto Serif",
    "DejaVu Serif",
    "Liberation Serif",
    "FreeSerif",
    "Times New Roman",
];
const MONO: &[&str] = &[
    "Noto Sans Mono",
    "DejaVu Sans Mono",
    "Liberation Mono",
    "Ubuntu Mono",
    "Hack",
    "FreeMono",
    "Courier New",
];

const GENERIC: &[(GenericFamily, &[&str])] = &[
    (GenericFamily::SansSerif, SANS),
    (GenericFamily::Serif, SERIF),
    (GenericFamily::Monospace, MONO),
    (GenericFamily::Cursive, SERIF),
    (GenericFamily::Fantasy, SERIF),
    (GenericFamily::SystemUi, SANS),
    (GenericFamily::UiSansSerif, SANS),
    (GenericFamily::UiSerif, SERIF),
    (GenericFamily::UiMonospace, MONO),
    (GenericFamily::UiRounded, SANS),
    (
        GenericFamily::Emoji,
        &["Noto Color Emoji", "Twemoji", "JoyPixels", "Noto Emoji"],
    ),
    (
        GenericFamily::Math,
        &["Noto Sans Math", "STIX Two Math", "DejaVu Math TeX Gyre"],
    ),
    (GenericFamily::FangSong, &["FangSong", "AR PL UKai CN"]),
];

/// Where fonts live: `$XDG_DATA_HOME/fonts`, each `$XDG_DATA_DIRS/fonts`
/// and `~/.fonts`.
fn font_dirs() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home.as_ref().map(|h| h.join(".local/share")));
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| String::from("/usr/local/share:/usr/share"));
    let mut dirs: Vec<PathBuf> = data_home.into_iter().map(|d| d.join("fonts")).collect();
    dirs.extend(
        data_dirs
            .split(':')
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .map(|d| d.join("fonts")),
    );
    dirs.extend(home.map(|h| h.join(".fonts")));
    let mut seen = Vec::with_capacity(dirs.len());
    dirs.retain(|d| {
        let d = d.canonicalize().unwrap_or_else(|_| d.clone());
        let new = d.is_dir() && !seen.contains(&d);
        seen.push(d);
        new
    });
    dirs
}

pub(crate) struct SystemFonts {
    pub(crate) name_map: Arc<FamilyNameMap>,
    pub(crate) generic_families: Arc<GenericFamilyMap>,
    families: HashMap<FamilyId, FamilyInfo>,
    /// Families in fallback order: the generic sans-serif ones, then Noto
    /// Sans ones, then the rest by name.
    order: Vec<FamilyId>,
    fallbacks: HashMap<FallbackKey, Option<FamilyId>>,
}

impl SystemFonts {
    pub(crate) fn new() -> Self {
        let scan::ScannedCollection {
            family_names: name_map,
            families,
            ..
        } = scan::ScannedCollection::from_paths(font_dirs(), 8);
        let mut generic_families = GenericFamilyMap::default();
        for (generic, names) in GENERIC {
            generic_families.set(
                *generic,
                names.iter().filter_map(|n| name_map.get(n)).map(|n| n.id()),
            );
        }
        let sans = generic_families.get(GenericFamily::SansSerif);
        let mut order: Vec<(u8, &str, FamilyId)> = families
            .values()
            .map(|f| {
                let rank = match sans.iter().position(|id| *id == f.id()) {
                    Some(_) => 0,
                    None if f.name().starts_with("Noto Sans") => 1,
                    None if f.name().starts_with("Noto") => 2,
                    None => 3,
                };
                (rank, f.name(), f.id())
            })
            .collect();
        order.sort_by(|a, b| {
            let pos = |id| sans.iter().position(|s| *s == id).unwrap_or(usize::MAX);
            (a.0, pos(a.2), a.1).cmp(&(b.0, pos(b.2), b.1))
        });
        let order = order.into_iter().map(|(_, _, id)| id).collect();
        Self {
            name_map: Arc::new(name_map),
            generic_families: Arc::new(generic_families),
            families,
            order,
            fallbacks: HashMap::default(),
        }
    }

    pub(crate) fn family(&mut self, id: FamilyId) -> Option<FamilyInfo> {
        self.families.get(&id).cloned()
    }

    pub(crate) fn fallback(&mut self, key: impl Into<FallbackKey>) -> Option<FamilyId> {
        let key: FallbackKey = key.into();
        if let Some(found) = self.fallbacks.get(&key) {
            return *found;
        }
        let found = self.find_fallback(&key);
        self.fallbacks.insert(key, found);
        found
    }

    fn find_fallback(&self, key: &FallbackKey) -> Option<FamilyId> {
        let sample: Vec<char> = key
            .script()
            .sample()
            .map(|s| s.chars().filter(|c| !c.is_whitespace()).collect())
            .unwrap_or_default();
        if sample.is_empty() {
            return self.order.first().copied();
        }
        let hint = key.locale_str().and_then(cjk_hint);
        let covering = self
            .order
            .iter()
            .filter(|id| self.families.get(*id).is_some_and(|f| covers(f, &sample)));
        match hint {
            Some(hint) => {
                let mut first = None;
                for id in covering {
                    first.get_or_insert(*id);
                    if self.families[id].name().contains(hint) {
                        return Some(*id);
                    }
                }
                first
            }
            None => covering.copied().next(),
        }
    }
}

/// The family-name tag CJK fonts use for a locale's glyph shapes.
fn cjk_hint(locale: &str) -> Option<&'static str> {
    let l = locale.to_ascii_lowercase();
    Some(match () {
        _ if l.starts_with("ja") => "JP",
        _ if l.starts_with("ko") => "KR",
        _ if l.contains("hk") => "HK",
        _ if l.contains("hant") || l.contains("tw") || l.contains("mo") => "TC",
        _ if l.starts_with("zh") => "SC",
        _ => return None,
    })
}

/// Whether the family's default font maps every character of the sample.
fn covers(family: &FamilyInfo, sample: &[char]) -> bool {
    let Some(font) = family.default_font() else {
        return false;
    };
    let Some(data) = font.load(None) else {
        return false;
    };
    let Some(charmap) = font.charmap_index().charmap(data.as_ref()) else {
        return false;
    };
    sample
        .iter()
        .all(|c| charmap.map(*c).is_some_and(|g| g != 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cjk_locales_pick_their_glyph_shapes() {
        assert_eq!(cjk_hint("ja-JP"), Some("JP"));
        assert_eq!(cjk_hint("zh-Hant-TW"), Some("TC"));
        assert_eq!(cjk_hint("zh-HK"), Some("HK"));
        assert_eq!(cjk_hint("zh-CN"), Some("SC"));
        assert_eq!(cjk_hint("ko"), Some("KR"));
        assert_eq!(cjk_hint("en-US"), None);
    }

    /// Whatever fonts this machine has, Latin text falls back to one that
    /// covers it, and it is the first generic sans-serif family when there
    /// is one.
    #[test]
    fn latin_falls_back_to_a_covering_family() {
        let mut fonts = SystemFonts::new();
        if fonts.families.is_empty() {
            assert!(font_dirs().is_empty(), "font folders but no fonts");
            return;
        }
        let latin = Script::from_bytes(*b"Latn");
        let found = fonts.fallback(FallbackKey::new(latin, None));
        let id = found.expect("a Latin font");
        let sample: Vec<char> = latin
            .sample()
            .unwrap()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(covers(&fonts.families[&id], &sample));
        if let Some(sans) = fonts.generic_families.get(GenericFamily::SansSerif).first() {
            if covers(&fonts.families[sans], &sample) {
                assert_eq!(id, *sans);
            }
        }
        assert_eq!(fonts.fallback(FallbackKey::new(latin, None)), found);
    }
}

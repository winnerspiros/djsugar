//! The interface font. On macOS it is the system font, San Francisco;
//! elsewhere the bundled Inter Tabular. `DJSUGAR_UI_FONT` names
//! another font file to use.
//!
//! Running times and clocks need digits of one width, which San Francisco
//! has only as an OpenType feature (`tnum`) that Slint can't turn on. So
//! the font is read from the system, its digits are pointed at their
//! tabular forms, and the result is registered with Slint under its own
//! name. Nothing is written to disk or shipped: the system's own font is
//! used, changed only in memory.

use std::collections::HashMap;
use std::sync::Arc;

use read_fonts::tables::gsub::{SingleSubst, SubstitutionSubtables};
use read_fonts::types::{GlyphId, GlyphId16, NameId, Tag};
use read_fonts::{FontRef, TableProvider};
use skrifa::MetadataProvider;
use write_fonts::tables::cmap::Cmap;
use write_fonts::tables::name::{Name, NameRecord};
use write_fonts::{FontBuilder, OffsetMarker};

/// The family the adapted font is registered as.
pub const FAMILY: &str = "DJ Sugar UI";

/// The macOS system font.
const SYSTEM_FONT: &str = "/System/Library/Fonts/SFNS.ttf";

/// Registers the interface font, if there is one to adapt, and returns the
/// family to use; `None` keeps the bundled font.
pub fn install() -> Option<&'static str> {
    let path = std::env::var_os("DJSUGAR_UI_FONT")
        .map(std::path::PathBuf::from)
        .or_else(|| cfg!(target_os = "macos").then(|| SYSTEM_FONT.into()))?;
    let data = std::fs::read(&path).ok()?;
    let font = match tabular(&data, FAMILY) {
        Ok(font) => font,
        Err(e) => {
            eprintln!("can't use {} as the interface font: {e}", path.display());
            return None;
        }
    };
    use slint::fontique_011::fontique;
    let mut collection = slint::fontique_011::shared_collection();
    let fonts = collection.register_fonts(fontique::Blob::new(Arc::new(font)), None);
    (!fonts.is_empty()).then_some(FAMILY)
}

/// `data` (a TrueType or OpenType font) with tabular digits by default,
/// renamed to `family`.
pub fn tabular(data: &[u8], family: &str) -> Result<Vec<u8>, String> {
    let font = FontRef::new(data).map_err(|e| e.to_string())?;
    let tnum = tnum_glyphs(&font).unwrap_or_default();
    if tnum.is_empty() {
        return Err("it has no tabular digits".into());
    }
    let mappings = font.charmap().mappings().map(|(c, g)| {
        let c = char::from_u32(c).unwrap_or('\0');
        let g = if c.is_ascii_digit() {
            tnum.get(&g).copied().unwrap_or(g)
        } else {
            g
        };
        (c, g)
    });
    let cmap =
        Cmap::from_mappings(mappings.filter(|(c, _)| *c != '\0')).map_err(|e| format!("{e:?}"))?;
    let name = renamed(&font, family).map_err(|e| e.to_string())?;
    let mut builder = FontBuilder::new();
    builder.add_table(&cmap).map_err(|e| e.to_string())?;
    builder.add_table(&name).map_err(|e| e.to_string())?;
    builder.copy_missing_tables(font);
    Ok(builder.build())
}

/// The glyphs the `tnum` feature swaps in, by the glyph they replace.
fn tnum_glyphs(font: &FontRef) -> Result<HashMap<GlyphId, GlyphId>, read_fonts::ReadError> {
    let gsub = font.gsub()?;
    let features = gsub.feature_list()?;
    let lookups = gsub.lookup_list()?;
    let mut map = HashMap::new();
    for record in features.feature_records() {
        if record.feature_tag() != Tag::new(b"tnum") {
            continue;
        }
        let feature = record.feature(features.offset_data())?;
        for index in feature.lookup_list_indices() {
            let lookup = lookups.lookups().get(index.get() as usize)?;
            let SubstitutionSubtables::Single(subtables) = lookup.subtables()? else {
                continue;
            };
            for subtable in subtables.iter() {
                match subtable? {
                    SingleSubst::Format1(f) => {
                        let delta = f.delta_glyph_id() as i32;
                        for g in f.coverage()?.iter() {
                            let to = (g.to_u32() as i32 + delta) as u16;
                            map.entry(g.into()).or_insert(GlyphId16::new(to).into());
                        }
                    }
                    SingleSubst::Format2(f) => {
                        for (g, to) in f.coverage()?.iter().zip(f.substitute_glyph_ids()) {
                            map.entry(g.into()).or_insert(to.get().into());
                        }
                    }
                }
            }
        }
    }
    Ok(map)
}

/// The font's name table with its family names replaced by `family`.
fn renamed(font: &FontRef, family: &str) -> Result<Name, read_fonts::ReadError> {
    let name = font.name()?;
    let mut records = Vec::new();
    for r in name.name_record() {
        let id = r.name_id();
        let text = if id == NameId::FAMILY_NAME || id == NameId::TYPOGRAPHIC_FAMILY_NAME {
            family.to_string()
        } else if id == NameId::FULL_NAME {
            format!("{family} Regular")
        } else if id == NameId::POSTSCRIPT_NAME {
            family.replace(' ', "")
        } else {
            r.string(name.string_data())?.to_string()
        };
        records.push(NameRecord::new(
            r.platform_id(),
            r.encoding_id(),
            r.language_id(),
            id,
            OffsetMarker::new(text),
        ));
    }
    Ok(Name::new(records))
}

#[cfg(test)]
mod tests {
    use super::*;

    const INTER: &[u8] = include_bytes!("../ui/fonts/InterTabular-Regular.otf");

    #[test]
    fn digits_become_one_width_and_the_family_is_renamed() {
        // Stock Inter, where installed, has proportional digits to fix.
        let stock = std::fs::read("/usr/share/fonts/opentype/inter/Inter-Regular.otf");
        let input = stock.as_deref().unwrap_or(INTER);
        let out = tabular(input, "Test Family").unwrap();
        let font = FontRef::new(&out).unwrap();
        let charmap = font.charmap();
        let metrics = font.glyph_metrics(
            skrifa::instance::Size::unscaled(),
            skrifa::instance::LocationRef::default(),
        );
        let widths: Vec<f32> = ('0'..='9')
            .map(|c| metrics.advance_width(charmap.map(c).unwrap()).unwrap())
            .collect();
        assert!(widths.iter().all(|w| *w == widths[0]), "{widths:?}");
        let family = font
            .localized_strings(NameId::FAMILY_NAME)
            .english_or_first()
            .unwrap()
            .to_string();
        assert_eq!(family, "Test Family");
        // Letters are untouched.
        assert_eq!(
            charmap.map('a'),
            FontRef::new(input).unwrap().charmap().map('a')
        );
    }
}

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

use skrifa::font::Font;
use skrifa::{DrawMemory, GlyphId, PathSink, ScalerFont};

use crate::raster::Raster;

pub const ATLAS: usize = 2048;
const WEIGHTS: [f32; 3] = [400.0, 600.0, 800.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Regular,
    Semibold,
    Bold,
}

#[derive(Default)]
struct IdHasher(u64);

impl Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, _: &[u8]) {
        unreachable!()
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = n.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
}

#[derive(Clone, Copy)]
struct Slot {
    uv: [u16; 4],
    left: i16,
    top: i16,
}

pub struct Fonts {
    faces: [ScalerFont; 3],
    scale_unit: f32,
    cache: HashMap<u64, Option<Slot>, BuildHasherDefault<IdHasher>>,
    raster: Raster,
    pub(crate) pixels: Vec<u8>,
    shelf: (u16, u16, u16),
    pub(crate) dirty: Option<(usize, usize)>,
}

impl std::fmt::Debug for Fonts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fonts").field("glyphs", &self.cache.len()).finish()
    }
}

struct Outline<'a> {
    r: &'a mut Raster,
    s: f32,
    ox: f32,
    oy: f32,
    start: (f32, f32),
    at: (f32, f32),
}

impl Outline<'_> {
    fn p(&self, x: f32, y: f32) -> (f32, f32) {
        (x * self.s + self.ox, self.oy - y * self.s)
    }
}

impl PathSink for Outline<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.start = self.p(x, y);
        self.at = self.start;
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.p(x, y);
        self.r.line(self.at, p);
        self.at = p;
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (c, p) = (self.p(x1, y1), self.p(x, y));
        self.r.quad(self.at, c, p);
        self.at = p;
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (c1, c2, p) = (self.p(x1, y1), self.p(x2, y2), self.p(x, y));
        self.r.cubic(self.at, c1, c2, p);
        self.at = p;
    }
    fn close(&mut self) {
        if self.at != self.start {
            self.r.line(self.at, self.start);
        }
        self.at = self.start;
    }
}

pub(crate) struct Run {
    pub uv: [u16; 4],
    pub x: f32,
    pub y: f32,
}

impl Fonts {
    pub fn new(data: &'static [u8]) -> Self {
        let font = Font::new(data, 0).expect("bundled font parses");
        let faces =
            WEIGHTS.map(|w| ScalerFont::unhinted(font.instance_builder().variations([("wght", w)]).build(), None));
        let scale_unit = f32::from(font.units_per_em()).recip();
        Self {
            faces,
            scale_unit,
            cache: HashMap::default(),
            raster: Raster::new(),
            pixels: vec![0; ATLAS * ATLAS],
            shelf: (1, 1, 0),
            dirty: None,
        }
    }

    pub fn width(&self, text: &str, px: f32, weight: Weight) -> f32 {
        let f = &self.faces[weight as usize];
        let (map, metrics) = (f.font().charmap(), f.linear_glyph_metrics());
        text.chars().filter_map(|c| map.map_unicode(c)).map(|g| metrics.h_advance(g)).sum::<f32>()
            * px
            * self.scale_unit
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn layout(
        &mut self,
        text: &str,
        x: f32,
        baseline: f32,
        px: f32,
        weight: Weight,
        max: f32,
        mut emit: impl FnMut(Run),
    ) -> f32 {
        let full = self.width(text, px, weight);
        let (limit, ellipsis) =
            if full <= max { (f32::INFINITY, false) } else { (max - self.width("\u{2026}", px, weight), true) };
        let s = px * self.scale_unit;
        let mut pen = x;
        for c in text.chars() {
            let adv = self.glyph(c, x, pen, baseline, px, s, weight, &mut emit, limit);
            match adv {
                Some(a) => pen += a,
                None => break,
            }
        }
        if ellipsis {
            pen += self.glyph('\u{2026}', x, pen, baseline, px, s, weight, &mut emit, f32::INFINITY).unwrap_or(0.0);
        }
        pen - x
    }

    #[allow(clippy::too_many_arguments)]
    fn glyph(
        &mut self,
        c: char,
        x: f32,
        pen: f32,
        baseline: f32,
        px: f32,
        s: f32,
        weight: Weight,
        emit: &mut impl FnMut(Run),
        limit: f32,
    ) -> Option<f32> {
        let f = &self.faces[weight as usize];
        let Some(g) = f.font().charmap().map_unicode(c) else { return Some(0.0) };
        let adv = f.linear_glyph_metrics().h_advance(g) * s;
        if pen + adv - x > limit {
            return None;
        }
        let key = u64::from(g.to_u32()) | ((px * 4.0).round() as u64) << 16 | (weight as u64) << 40;
        let slot = match self.cache.get(&key) {
            Some(&slot) => slot,
            None => {
                let slot = self.rasterize(weight, g, s);
                self.cache.insert(key, slot);
                slot
            }
        };
        if let Some(sl) = slot {
            emit(Run { uv: sl.uv, x: pen.round() + f32::from(sl.left), y: baseline.round() - f32::from(sl.top) });
        }
        Some(adv)
    }

    fn rasterize(&mut self, weight: Weight, g: GlyphId, s: f32) -> Option<Slot> {
        let f = &self.faces[weight as usize];
        let e = f.linear_glyph_metrics().extents(g)?;
        if e.width <= 0.0 || e.height <= 0.0 {
            return None;
        }
        let x0 = (e.x_bearing * s).floor() - 1.0;
        let x1 = ((e.x_bearing + e.width) * s).ceil() + 1.0;
        let y0 = ((e.y_bearing - e.height) * s).floor() - 1.0;
        let y1 = (e.y_bearing * s).ceil() + 1.0;
        let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
        if w >= ATLAS || h >= ATLAS {
            return None;
        }
        let (mut sx, mut sy, mut sh) = self.shelf;
        if usize::from(sx) + w + 1 > ATLAS {
            sx = 1;
            sy += sh + 1;
            sh = 0;
        }
        if usize::from(sy) + h + 1 > ATLAS {
            self.cache.clear();
            self.pixels.fill(0);
            (sx, sy, sh) = (1, 1, 0);
            self.dirty = Some((0, ATLAS));
        }
        self.raster.reset(w, h);
        let outline = f.outline(g)?;
        outline
            .draw(
                DrawMemory::Scratch,
                &mut Outline { r: &mut self.raster, s, ox: -x0, oy: y1, start: (0.0, 0.0), at: (0.0, 0.0) },
            )
            .ok()?;
        let at = usize::from(sy) * ATLAS + usize::from(sx);
        self.raster.write(&mut self.pixels[at..], ATLAS);
        let rows = (usize::from(sy), usize::from(sy) + h);
        self.dirty = Some(self.dirty.map_or(rows, |(a, b)| (a.min(rows.0), b.max(rows.1))));
        let slot = Slot { uv: [sx, sy, w as u16, h as u16], left: x0 as i16, top: y1 as i16 };
        self.shelf = (sx + w as u16 + 1, sy, sh.max(h as u16));
        Some(slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static FONT: &[u8] = include_bytes!("../../../res/fonts/Inter.ttf");

    #[test]
    fn glyphs_are_cached_once() {
        let mut f = Fonts::new(FONT);
        let mut n = 0;
        f.layout("DJ Sugar", 0.0, 20.0, 16.0, Weight::Regular, f32::INFINITY, |_| n += 1);
        let cached = f.cache.len();
        f.layout("DJ Sugar", 0.0, 20.0, 16.0, Weight::Regular, f32::INFINITY, |_| n += 1);
        assert_eq!(n, 14);
        assert_eq!(f.cache.len(), cached);
        assert!(f.pixels.contains(&255));
    }

    #[test]
    fn long_text_is_cut_with_an_ellipsis() {
        let mut f = Fonts::new(FONT);
        let full = f.width("we drive until the city sleeps", 16.0, Weight::Bold);
        let w = f.layout("we drive until the city sleeps", 0.0, 20.0, 16.0, Weight::Bold, full * 0.5, |_| {});
        assert!(w <= full * 0.5 + 0.01, "{w} > {}", full * 0.5);
        assert!(w > full * 0.3);
    }

    #[test]
    fn bold_is_wider() {
        let f = Fonts::new(FONT);
        assert!(f.width("Sugar", 16.0, Weight::Bold) > f.width("Sugar", 16.0, Weight::Regular));
    }
}

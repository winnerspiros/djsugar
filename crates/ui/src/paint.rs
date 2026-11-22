use bytemuck::{Pod, Zeroable};

use crate::text::{ATLAS, Fonts, Weight};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Quad {
    pub rect: [f32; 4],
    pub radius: f32,
    pub border: f32,
    pub fill: u32,
    pub stroke: u32,
}

pub const fn rgb(hex: u32) -> u32 {
    rgba(hex << 8 | 0xff)
}

pub const fn rgba(hex: u32) -> u32 {
    hex.swap_bytes()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub px: f32,
    pub weight: Weight,
    pub colour: u32,
}

impl Style {
    pub const fn new(px: f32, weight: Weight, colour: u32) -> Self {
        Self { px, weight, colour }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Glyph {
    pub pos: [f32; 2],
    pub uv: [u16; 4],
    pub colour: u32,
}

#[derive(Debug)]
pub struct Painter {
    quads: Vec<Quad>,
    glyphs: Vec<Glyph>,
    scale: f32,
    pub fonts: Fonts,
}

impl Painter {
    pub fn new(fonts: Fonts) -> Self {
        Self { quads: Vec::new(), glyphs: Vec::new(), scale: 1.0, fonts }
    }

    pub fn begin(&mut self, scale: f32) {
        self.quads.clear();
        self.glyphs.clear();
        self.scale = scale;
    }

    pub fn text(&mut self, x: f32, baseline: f32, style: Style, text: &str) -> f32 {
        self.text_in(x, baseline, f32::INFINITY, style, text)
    }

    pub fn text_in(&mut self, x: f32, baseline: f32, max: f32, style: Style, text: &str) -> f32 {
        let s = self.scale;
        let glyphs = &mut self.glyphs;
        let colour = style.colour;
        let w = self.fonts.layout(text, x * s, baseline * s, style.px * s, style.weight, max * s, |r| {
            glyphs.push(Glyph { pos: [r.x, r.y], uv: r.uv, colour });
        });
        w / s
    }

    pub fn text_width(&self, style: Style, text: &str) -> f32 {
        self.fonts.width(text, style.px, style.weight)
    }

    pub fn glyphs(&self) -> &[Glyph] {
        &self.glyphs
    }

    pub(crate) fn atlas_update(&mut self) -> Option<(usize, &[u8])> {
        let (a, b) = self.fonts.dirty.take()?;
        Some((a, &self.fonts.pixels[a * ATLAS..b * ATLAS]))
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, fill: u32) {
        self.outlined(x, y, w, h, radius, fill, 0.0, 0);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn outlined(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, fill: u32, border: f32, stroke: u32) {
        let s = self.scale;
        self.quads.push(Quad {
            rect: [x * s, y * s, w * s, h * s],
            radius: radius * s,
            border: border * s,
            fill,
            stroke,
        });
    }

    pub fn quads(&self) -> &[Quad] {
        &self.quads
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_pack_as_rgba_bytes() {
        assert_eq!(rgb(0x112233).to_le_bytes(), [0x11, 0x22, 0x33, 0xff]);
        assert_eq!(rgba(0x11223344).to_le_bytes(), [0x11, 0x22, 0x33, 0x44]);
    }

    #[test]
    fn quad_is_32_bytes() {
        assert_eq!(size_of::<Quad>(), 32);
    }

    #[test]
    fn glyph_is_20_bytes() {
        assert_eq!(size_of::<Glyph>(), 20);
    }

    #[test]
    fn begin_keeps_capacity() {
        let mut p = Painter::new(Fonts::new(include_bytes!("../../../res/fonts/Inter.ttf")));
        p.begin(2.0);
        p.rect(1.0, 2.0, 3.0, 4.0, 0.0, 0);
        let cap = p.quads.capacity();
        p.begin(1.0);
        assert!(p.quads().is_empty());
        assert_eq!(p.quads.capacity(), cap);
    }
}

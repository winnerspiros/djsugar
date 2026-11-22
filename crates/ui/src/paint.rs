use bytemuck::{Pod, Zeroable};

use crate::text::{ATLAS, Fonts, Weight};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Quad {
    pub rect: [f32; 4],
    pub radius: f32,
    pub border: f32,
    pub soft: f32,
    pub radial: f32,
    pub fill: u32,
    pub stroke: u32,
    pub fill2: u32,
    pub arc: u32,
}

/// Surface textures drawn by the shader, with a lit bevel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Material {
    Brushed = 2,
    Vinyl = 3,
    Knurled = 4,
    Grain = 5,
}

fn turns(a: f32) -> u32 {
    (a.rem_euclid(1.0) * 65535.0).round() as u32
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
    layers: Vec<(u32, u32)>,
    scale: f32,
    pub fonts: Fonts,
}

impl Painter {
    pub fn new(fonts: Fonts) -> Self {
        Self { quads: Vec::new(), glyphs: Vec::new(), layers: Vec::new(), scale: 1.0, fonts }
    }

    pub fn begin(&mut self, scale: f32) {
        self.quads.clear();
        self.glyphs.clear();
        self.layers.clear();
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

    pub fn layer(&mut self) {
        self.layers.push((self.quads.len() as u32, self.glyphs.len() as u32));
    }

    pub(crate) fn spans(&self) -> impl Iterator<Item = (std::ops::Range<u32>, std::ops::Range<u32>)> + '_ {
        let ends = self.layers.iter().copied().chain([(self.quads.len() as u32, self.glyphs.len() as u32)]);
        let starts = [(0, 0)].into_iter().chain(self.layers.iter().copied());
        starts.zip(ends).map(|(a, b)| (a.0..b.0, a.1..b.1))
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
            ..Quad::default()
        });
    }

    /// Vertical gradient from `top` to `bottom`.
    #[allow(clippy::too_many_arguments)]
    pub fn gradient(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        top: u32,
        bottom: u32,
        border: f32,
        stroke: u32,
    ) {
        self.outlined(x, y, w, h, radius, top, border, stroke);
        self.last().fill2 = bottom | u32::from(bottom == 0);
    }

    /// Radial gradient disc from `centre` colour to `edge` colour.
    pub fn disc(&mut self, cx: f32, cy: f32, r: f32, centre: u32, edge: u32) {
        self.rect(cx - r, cy - r, r * 2.0, r * 2.0, r, centre);
        let q = self.last();
        q.fill2 = edge | u32::from(edge == 0);
        q.radial = 1.0;
    }

    /// Soft blurred shape: shadows and neon glow.
    #[allow(clippy::too_many_arguments)]
    pub fn glow(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, blur: f32, colour: u32) {
        self.rect(x, y, w, h, radius, colour);
        self.last().soft = blur * self.scale;
    }

    /// Ring segment of width `thick`, starting at `start` turns clockwise from 12 o'clock for `sweep` turns.
    #[allow(clippy::too_many_arguments)]
    pub fn arc(&mut self, cx: f32, cy: f32, r: f32, thick: f32, start: f32, sweep: f32, colour: u32) {
        if sweep <= 0.0 {
            return;
        }
        self.outlined(cx - r, cy - r, r * 2.0, r * 2.0, r, 0, thick, colour);
        self.last().arc = turns(start) | ((sweep.min(1.0) * 65535.0).round() as u32) << 16;
    }

    /// Filled pie wedge, as `arc` but solid.
    pub fn pie(&mut self, cx: f32, cy: f32, r: f32, start: f32, sweep: f32, colour: u32) {
        if sweep <= 0.0 {
            return;
        }
        self.rect(cx - r, cy - r, r * 2.0, r * 2.0, r, colour);
        self.last().arc = turns(start) | ((sweep.min(1.0) * 65535.0).round() as u32) << 16;
    }

    /// Straight line of width `w` between two points, drawn as a rotated-free thin quad series.
    pub fn line(&mut self, (x0, y0): (f32, f32), (x1, y1): (f32, f32), w: f32, colour: u32) {
        let (dx, dy) = (x1 - x0, y1 - y0);
        let n = (dx.abs().max(dy.abs()) / (w * 0.75).max(0.5)).ceil().max(1.0) as usize;
        for i in 0..=n {
            let t = i as f32 / n as f32;
            self.rect(x0 + dx * t - w * 0.5, y0 + dy * t - w * 0.5, w, w, w * 0.5, colour);
        }
    }

    /// Gives the last shape a surface texture.
    pub fn texture(&mut self, m: Material) {
        self.last().radial = m as u8 as f32;
    }

    fn last(&mut self) -> &mut Quad {
        self.quads.last_mut().expect("pushed")
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
    fn quad_is_48_bytes() {
        assert_eq!(size_of::<Quad>(), 48);
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

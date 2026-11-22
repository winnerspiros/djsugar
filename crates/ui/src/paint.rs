use bytemuck::{Pod, Zeroable};

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

#[derive(Debug, Default)]
pub struct Painter {
    quads: Vec<Quad>,
    scale: f32,
}

impl Painter {
    pub fn begin(&mut self, scale: f32) {
        self.quads.clear();
        self.scale = scale;
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, fill: u32) {
        self.outlined(x, y, w, h, radius, fill, 0.0, 0);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn outlined(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        fill: u32,
        border: f32,
        stroke: u32,
    ) {
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
    fn begin_keeps_capacity() {
        let mut p = Painter::default();
        p.begin(2.0);
        p.rect(1.0, 2.0, 3.0, 4.0, 0.0, 0);
        let cap = p.quads.capacity();
        p.begin(1.0);
        assert!(p.quads().is_empty());
        assert_eq!(p.quads.capacity(), cap);
    }
}

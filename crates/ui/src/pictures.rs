//! Pictures (covers, thumbnails): kept in one texture of square cells, the one drawn longest ago making way.

/// The most a picture can measure on a side, in pixels.
pub const CELL: usize = 256;
/// Cells on a side of the texture.
pub(crate) const SIDE: usize = 8;

#[derive(Debug, Default)]
struct Slot {
    key: u64,
    w: u16,
    h: u16,
    seen: u64,
    pixels: Vec<u8>,
    stale: bool,
}

#[derive(Debug)]
pub struct Pictures {
    slots: Vec<Slot>,
    frame: u64,
}

impl Default for Pictures {
    fn default() -> Self {
        Self { slots: (0..SIDE * SIDE).map(|_| Slot::default()).collect(), frame: 1 }
    }
}

impl Pictures {
    pub(crate) fn tick(&mut self) {
        self.frame += 1;
    }

    pub fn has(&self, key: u64) -> bool {
        self.slots.iter().any(|s| s.key == key && s.w > 0)
    }

    /// Keeps `rgba`, `w` by `h` pixels and no larger than [`CELL`] either way, to be drawn by its `key`.
    pub fn set(&mut self, key: u64, w: usize, h: usize, rgba: &[u8]) {
        if w == 0 || h == 0 || w > CELL || h > CELL || rgba.len() != w * h * 4 {
            return;
        }
        let at = self.slots.iter().position(|s| s.key == key && s.w > 0).unwrap_or_else(|| {
            (0..self.slots.len()).min_by_key(|&i| (self.slots[i].w > 0, self.slots[i].seen)).unwrap_or(0)
        });
        let s = &mut self.slots[at];
        s.pixels.clear();
        s.pixels.extend_from_slice(rgba);
        *s = Slot {
            key,
            w: w as u16,
            h: h as u16,
            seen: self.frame,
            pixels: std::mem::take(&mut s.pixels),
            stale: true,
        };
    }

    /// Where the picture is in the texture: left, top, width, height.
    pub(crate) fn find(&mut self, key: u64) -> Option<[u16; 4]> {
        let at = self.slots.iter().position(|s| s.key == key && s.w > 0)?;
        let s = &mut self.slots[at];
        s.seen = self.frame;
        Some([(at % SIDE * CELL) as u16, (at / SIDE * CELL) as u16, s.w, s.h])
    }

    /// Hands over the pictures the texture does not hold yet (all of them when it is `fresh`), as left, top, width, height and pixels.
    pub(crate) fn pending(&mut self, fresh: bool, mut put: impl FnMut([u32; 4], &[u8])) {
        for (at, s) in self.slots.iter_mut().enumerate() {
            if s.w > 0 && (s.stale || fresh) {
                put([(at % SIDE * CELL) as u32, (at / SIDE * CELL) as u32, u32::from(s.w), u32::from(s.h)], &s.pixels);
                s.stale = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picture_drawn_longest_ago_makes_way() {
        let mut p = Pictures::default();
        let dot = [0u8; 4];
        for key in 1..=(SIDE * SIDE) as u64 {
            p.set(key, 1, 1, &dot);
            p.tick();
        }
        assert!(p.find(1).is_some());
        p.set(1000, 1, 1, &dot);
        assert!(p.has(1) && p.has(1000) && !p.has(2));
        let mut sent = 0;
        p.pending(false, |_, _| sent += 1);
        assert_eq!(sent, SIDE * SIDE);
        p.pending(false, |_, _| sent += 1);
        assert_eq!(sent, SIDE * SIDE);
        p.set(7, CELL + 1, 1, &[0; (CELL + 1) * 4]);
        p.pending(false, |_, _| sent += 1);
        assert_eq!(sent, SIDE * SIDE);
    }
}

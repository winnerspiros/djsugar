use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Minor,
    Major,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub camelot: u8,
    pub mode: Mode,
}

impl Key {
    pub fn new(camelot: u8, mode: Mode) -> Option<Self> {
        (1..=12)
            .contains(&camelot)
            .then_some(Self { camelot, mode })
    }

    fn wheel_distance(self, other: Key) -> u8 {
        let d = (i16::from(self.camelot) - i16::from(other.camelot)).rem_euclid(12) as u8;
        d.min(12 - d)
    }

    pub fn fit(self, other: Key) -> f32 {
        match (self.wheel_distance(other), self.mode == other.mode) {
            (0, true) => 1.0,
            (0, false) | (1, true) => 0.9,
            (2, true) => 0.6,
            (1, false) => 0.4,
            _ => 0.0,
        }
    }

    pub fn transpose(self, semitones: i32) -> Key {
        let c = (i32::from(self.camelot) - 1 + semitones * 7).rem_euclid(12) + 1;
        Key {
            camelot: c as u8,
            mode: self.mode,
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = if self.mode == Mode::Minor { 'A' } else { 'B' };
        write!(f, "{}{m}", self.camelot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(c: u8, m: Mode) -> Key {
        Key::new(c, m).unwrap()
    }

    #[test]
    fn fit_follows_the_wheel() {
        assert_eq!(k(8, Mode::Minor).fit(k(8, Mode::Minor)), 1.0);
        assert_eq!(k(8, Mode::Minor).fit(k(8, Mode::Major)), 0.9);
        assert_eq!(k(12, Mode::Minor).fit(k(1, Mode::Minor)), 0.9);
        assert_eq!(k(8, Mode::Minor).fit(k(2, Mode::Minor)), 0.0);
    }

    #[test]
    fn transpose_wraps_and_displays() {
        assert_eq!(k(8, Mode::Minor).transpose(1).to_string(), "3A");
        assert_eq!(k(8, Mode::Minor).transpose(12), k(8, Mode::Minor));
        assert!(Key::new(13, Mode::Major).is_none());
    }
}

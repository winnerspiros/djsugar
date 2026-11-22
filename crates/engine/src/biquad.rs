use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, Default)]
pub struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    z: [[f32; 2]; 2],
}

#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Low,
    High,
    LowShelf(f32),
    HighShelf(f32),
    Peak(f32),
}

impl Biquad {
    pub fn identity() -> Self {
        Self { b: [1.0, 0.0, 0.0], ..Self::default() }
    }

    pub fn set(&mut self, kind: Kind, freq: f32, q: f32, rate: f32) {
        let w = 2.0 * PI * (freq / rate).clamp(1e-5, 0.49);
        let (s, c) = w.sin_cos();
        let alpha = s / (2.0 * q);
        let (b, a) = match kind {
            Kind::Low => ([(1.0 - c) * 0.5, 1.0 - c, (1.0 - c) * 0.5], [1.0 + alpha, -2.0 * c, 1.0 - alpha]),
            Kind::High => ([(1.0 + c) * 0.5, -(1.0 + c), (1.0 + c) * 0.5], [1.0 + alpha, -2.0 * c, 1.0 - alpha]),
            Kind::Peak(g) => {
                let a = g.sqrt();
                ([1.0 + alpha * a, -2.0 * c, 1.0 - alpha * a], [1.0 + alpha / a, -2.0 * c, 1.0 - alpha / a])
            }
            Kind::LowShelf(g) | Kind::HighShelf(g) => {
                let a = g.sqrt();
                let r = 2.0 * a.sqrt() * alpha;
                let sign = if matches!(kind, Kind::LowShelf(_)) { 1.0 } else { -1.0 };
                let (p, m) = (a + 1.0, a - 1.0);
                (
                    [a * (p - sign * m * c + r), sign * 2.0 * a * (m - sign * p * c), a * (p - sign * m * c - r)],
                    [p + sign * m * c + r, -sign * 2.0 * (m + sign * p * c), p + sign * m * c - r],
                )
            }
        };
        let n = a[0].recip();
        self.b = b.map(|v| v * n);
        self.a = [a[1] * n, a[2] * n];
    }

    #[inline]
    pub fn run(&mut self, ch: usize, x: f32) -> f32 {
        let z = &mut self.z[ch];
        let y = self.b[0] * x + z[0];
        z[0] = self.b[1] * x - self.a[0] * y + z[1];
        z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

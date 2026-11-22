pub struct Raster {
    w: usize,
    h: usize,
    acc: Vec<f32>,
}

impl Raster {
    pub fn new() -> Self {
        Self { w: 0, h: 0, acc: Vec::new() }
    }

    pub fn reset(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.acc.clear();
        self.acc.resize(w * h + 4, 0.0);
    }

    pub fn line(&mut self, (x0, y0): (f32, f32), (x1, y1): (f32, f32)) {
        if (y0 - y1).abs() <= f32::EPSILON {
            return;
        }
        let (dir, (ax, ay), (bx, by)) = if y0 < y1 { (1.0, (x0, y0), (x1, y1)) } else { (-1.0, (x1, y1), (x0, y0)) };
        let dxdy = (bx - ax) / (by - ay);
        let mut x = ax;
        if ay < 0.0 {
            x -= ay * dxdy;
        }
        let end = (by.ceil() as usize).min(self.h);
        let w = self.w;
        let acc = &mut self.acc;
        for y in (ay.max(0.0) as usize)..end {
            let row = y * w;
            let dy = ((y + 1) as f32).min(by) - (y as f32).max(ay);
            let xn = x + dxdy * dy;
            let d = dy * dir;
            let (l, r) = if x < xn { (x, xn) } else { (xn, x) };
            let lf = l.floor();
            let li = lf.max(0.0) as usize;
            let rc = r.ceil();
            let ri = rc.max(0.0) as usize;
            if ri <= li + 1 {
                let m = 0.5 * (x + xn) - lf;
                acc[row + li] += d - d * m;
                acc[row + li + 1] += d * m;
            } else {
                let s = (r - l).recip();
                let lx = l - lf;
                let a0 = 0.5 * s * (1.0 - lx) * (1.0 - lx);
                let rx = r - rc + 1.0;
                let am = 0.5 * s * rx * rx;
                acc[row + li] += d * a0;
                if ri == li + 2 {
                    acc[row + li + 1] += d * (1.0 - a0 - am);
                } else {
                    let a1 = s * (1.5 - lx);
                    acc[row + li + 1] += d * (a1 - a0);
                    for v in &mut acc[row + li + 2..row + ri - 1] {
                        *v += d * s;
                    }
                    let a2 = a1 + (ri - li - 3) as f32 * s;
                    acc[row + ri - 1] += d * (1.0 - a2 - am);
                }
                acc[row + ri] += d * am;
            }
            x = xn;
        }
    }

    pub fn quad(&mut self, p0: (f32, f32), p1: (f32, f32), p2: (f32, f32)) {
        let dev = (p0.0 - 2.0 * p1.0 + p2.0).hypot(p0.1 - 2.0 * p1.1 + p2.1);
        let n = 1 + (dev * 3.0).sqrt() as usize;
        let step = (n as f32).recip();
        let mut prev = p0;
        for i in 1..=n {
            let t = i as f32 * step;
            let u = 1.0 - t;
            let p =
                (u * u * p0.0 + 2.0 * u * t * p1.0 + t * t * p2.0, u * u * p0.1 + 2.0 * u * t * p1.1 + t * t * p2.1);
            self.line(prev, p);
            prev = p;
        }
    }

    pub fn cubic(&mut self, p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)) {
        let d1 = (p0.0 - 2.0 * p1.0 + p2.0).hypot(p0.1 - 2.0 * p1.1 + p2.1);
        let d2 = (p1.0 - 2.0 * p2.0 + p3.0).hypot(p1.1 - 2.0 * p2.1 + p3.1);
        let n = 1 + (d1.max(d2) * 4.5).sqrt() as usize;
        let step = (n as f32).recip();
        let mut prev = p0;
        for i in 1..=n {
            let t = i as f32 * step;
            let u = 1.0 - t;
            let (a, b, c, e) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            let p = (a * p0.0 + b * p1.0 + c * p2.0 + e * p3.0, a * p0.1 + b * p1.1 + c * p2.1 + e * p3.1);
            self.line(prev, p);
            prev = p;
        }
    }

    pub fn write(&self, out: &mut [u8], stride: usize) {
        let mut sum = 0.0f32;
        for y in 0..self.h {
            let src = &self.acc[y * self.w..(y + 1) * self.w];
            let dst = &mut out[y * stride..y * stride + self.w];
            for (o, &a) in dst.iter_mut().zip(src) {
                sum += a;
                *o = (sum.abs().min(1.0) * 255.0 + 0.5) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_covers_its_pixels_exactly() {
        let mut r = Raster::new();
        r.reset(4, 4);
        let pts = [(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 3.0), (1.0, 1.0)];
        for p in pts.windows(2) {
            r.line(p[0], p[1]);
        }
        let mut out = [0u8; 16];
        r.write(&mut out, 4);
        assert_eq!(out, [0, 0, 0, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn half_pixel_edge_is_half_covered() {
        let mut r = Raster::new();
        r.reset(2, 1);
        let pts = [(0.5, 0.0), (2.0, 0.0), (2.0, 1.0), (0.5, 1.0), (0.5, 0.0)];
        for p in pts.windows(2) {
            r.line(p[0], p[1]);
        }
        let mut out = [0u8; 2];
        r.write(&mut out, 2);
        assert_eq!(out, [128, 255]);
    }
}

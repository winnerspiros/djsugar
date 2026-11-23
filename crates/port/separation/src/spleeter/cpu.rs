//! Spleeter's U-Nets on the CPU: each convolution is one matrix product of
//! input patches with the weights (`matrixmultiply`), split between threads.
//! Feature maps are channels-last: `data[(y * w + x) * c + channel]`.

use safetensors::SafeTensors;

use super::{tensor, F, STEMS, T};

#[derive(Clone)]
struct Map {
    h: usize,
    w: usize,
    c: usize,
    data: Vec<f32>,
}

struct Layer {
    /// Convolution: [(dy, dx, in), out]. Transposed: [in, (dy, dx, out)].
    weights: Vec<f32>,
    bias: Vec<f32>,
    inp: usize,
    out: usize,
    scale: Vec<f32>,
    shift: Vec<f32>,
}

struct UNet {
    enc: Vec<Layer>,
    dec: Vec<Layer>,
    /// [out = 2][dy][dx], one input channel.
    out_weights: Vec<f32>,
    out_bias: Vec<f32>,
}

const K: usize = 5;

/// Threads to use; 0 means one per core.
static THREADS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Limits the threads a separation uses, e.g. to leave cores free for
/// audio while a deck plays; 0 is one per core.
pub fn set_threads(n: usize) {
    THREADS.store(n, std::sync::atomic::Ordering::Relaxed);
}

fn threads() -> usize {
    match THREADS.load(std::sync::atomic::Ordering::Relaxed) {
        0 => std::thread::available_parallelism().map_or(1, |n| n.get()),
        n => n,
    }
}

/// `c = a · b` for row-major `a` (m × k) and `b` (k × n).
fn matmul(a: &[f32], b: &[f32], c: &mut [f32], m: usize, k: usize, n: usize) {
    // SAFETY: the slices hold m × k, k × n and m × n values (checked by the
    // asserts), and the strides describe dense row-major matrices.
    assert!(a.len() >= m * k && b.len() >= k * n && c.len() >= m * n);
    unsafe {
        matrixmultiply::sgemm(
            m,
            k,
            n,
            1.0,
            a.as_ptr(),
            k as isize,
            1,
            b.as_ptr(),
            n as isize,
            1,
            0.0,
            c.as_mut_ptr(),
            n as isize,
            1,
        );
    }
}

fn elu(x: f32) -> f32 {
    if x > 0.0 {
        x
    } else {
        x.exp_m1()
    }
}

impl Layer {
    /// A layer from burn-layout 5×5 weights: [out, in, kh, kw] for a
    /// convolution, [in, out, kh, kw] for a transposed one (`shape` is the
    /// first two).
    fn new(
        w: &[f32],
        shape: [usize; 2],
        transposed: bool,
        bias: Vec<f32>,
        scale: Vec<f32>,
        shift: Vec<f32>,
    ) -> Layer {
        let (inp, out) = if transposed {
            (shape[0], shape[1])
        } else {
            (shape[1], shape[0])
        };
        let mut weights = vec![0.0f32; w.len()];
        for a in 0..shape[0] {
            for b in 0..shape[1] {
                for dy in 0..K {
                    for dx in 0..K {
                        let v = w[((a * shape[1] + b) * K + dy) * K + dx];
                        let at = if transposed {
                            // a is the input channel, b the output channel.
                            (a * K * K + dy * K + dx) * out + b
                        } else {
                            // a is the output channel, b the input channel.
                            ((dy * K + dx) * inp + b) * out + a
                        };
                        weights[at] = v;
                    }
                }
            }
        }
        Layer {
            weights,
            bias,
            inp,
            out,
            scale,
            shift,
        }
    }

    fn norm(&self, x: &mut Map) {
        for px in x.data.chunks_exact_mut(x.c) {
            for ((v, s), b) in px.iter_mut().zip(&self.scale).zip(&self.shift) {
                *v = *v * s + b;
            }
        }
    }

    /// 5×5 convolution, stride 2, TensorFlow "same" padding (1 before, 2
    /// after), plus bias. Output rows are shared out between threads.
    fn down(&self, x: &Map) -> Map {
        let (oh, ow) = (x.h / 2, x.w / 2);
        let depth = K * K * self.inp;
        let mut out = vec![0.0f32; oh * ow * self.out];
        let rows = oh.div_ceil(threads());
        std::thread::scope(|scope| {
            for (part, block) in out.chunks_mut(rows * ow * self.out).enumerate() {
                scope.spawn(move || {
                    let n = block.len() / self.out;
                    let mut patches = vec![0.0f32; n * depth];
                    for (p, patch) in patches.chunks_exact_mut(depth).enumerate() {
                        let (oy, ox) = (part * rows + p / ow, p % ow);
                        for dy in 0..K {
                            let iy = (2 * oy + dy) as isize - 1;
                            for dx in 0..K {
                                let ix = (2 * ox + dx) as isize - 1;
                                let dst = &mut patch[(dy * K + dx) * self.inp..][..self.inp];
                                if (0..x.h as isize).contains(&iy)
                                    && (0..x.w as isize).contains(&ix)
                                {
                                    let at = (iy as usize * x.w + ix as usize) * x.c;
                                    dst.copy_from_slice(&x.data[at..at + x.c]);
                                } else {
                                    dst.fill(0.0);
                                }
                            }
                        }
                    }
                    matmul(&patches, &self.weights, block, n, depth, self.out);
                    for px in block.chunks_exact_mut(self.out) {
                        for (v, b) in px.iter_mut().zip(&self.bias) {
                            *v += b;
                        }
                    }
                });
            }
        });
        Map {
            h: oh,
            w: ow,
            c: self.out,
            data: out,
        }
    }

    /// 5×5 transposed convolution, stride 2, "same" (output 2n, the full
    /// output's rows and columns from 1), plus bias.
    fn up(&self, x: &Map) -> Map {
        let (oh, ow) = (x.h * 2, x.w * 2);
        let spread = K * K * self.out;
        let pixels = x.h * x.w;
        // Each input pixel's contribution to its 5×5 neighbourhood.
        let mut cols = vec![0.0f32; pixels * spread];
        let rows = pixels.div_ceil(threads());
        std::thread::scope(|scope| {
            for (part, block) in cols.chunks_mut(rows * spread).enumerate() {
                let n = block.len() / spread;
                let src = &x.data[part * rows * x.c..][..n * x.c];
                scope.spawn(move || matmul(src, &self.weights, block, n, self.inp, spread));
            }
        });
        let mut out = vec![0.0f32; oh * ow * self.out];
        for (p, col) in cols.chunks_exact(spread).enumerate() {
            let (iy, ix) = (p / x.w, p % x.w);
            for dy in 0..K {
                let oy = (2 * iy + dy) as isize - 1;
                if !(0..oh as isize).contains(&oy) {
                    continue;
                }
                for dx in 0..K {
                    let ox = (2 * ix + dx) as isize - 1;
                    if !(0..ow as isize).contains(&ox) {
                        continue;
                    }
                    let dst = &mut out[(oy as usize * ow + ox as usize) * self.out..][..self.out];
                    let add = &col[(dy * K + dx) * self.out..][..self.out];
                    for (d, a) in dst.iter_mut().zip(add) {
                        *d += a;
                    }
                }
            }
        }
        for px in out.chunks_exact_mut(self.out) {
            for (v, b) in px.iter_mut().zip(&self.bias) {
                *v += b;
            }
        }
        Map {
            h: oh,
            w: ow,
            c: self.out,
            data: out,
        }
    }
}

fn concat(a: &Map, b: &Map) -> Map {
    let c = a.c + b.c;
    let mut data = Vec::with_capacity(a.h * a.w * c);
    for (pa, pb) in a.data.chunks_exact(a.c).zip(b.data.chunks_exact(b.c)) {
        data.extend_from_slice(pa);
        data.extend_from_slice(pb);
    }
    Map {
        h: a.h,
        w: a.w,
        c,
        data,
    }
}

impl UNet {
    /// The stem's estimated magnitudes for `x` (T × F × 2).
    fn forward(&self, x: &Map) -> Vec<f32> {
        let mut convs: Vec<Map> = Vec::with_capacity(6);
        let mut h: Option<Map> = None;
        for layer in &self.enc {
            let c = layer.down(h.as_ref().unwrap_or(x));
            let mut a = c.clone();
            layer.norm(&mut a);
            a.data.iter_mut().for_each(|v| *v = elu(*v));
            convs.push(c);
            h = Some(a);
        }
        // Spleeter's decoder starts from the last convolution before its
        // batch norm and activation, and skips connect pre-norm outputs.
        let mut h = convs.pop().expect("six encoder layers");
        for layer in &self.dec {
            let mut u = layer.up(&h);
            u.data.iter_mut().for_each(|v| *v = elu(*v));
            layer.norm(&mut u);
            h = match convs.pop() {
                Some(skip) => concat(&skip, &u),
                None => u,
            };
        }
        // 4×4 convolution, dilation 2, "same" (3 each side), sigmoid, then
        // the mask times the input.
        let mut est = vec![0.0f32; T * F * 2];
        for y in 0..T {
            for xx in 0..F {
                for o in 0..2 {
                    let mut s = self.out_bias[o];
                    for dy in 0..4 {
                        let iy = (y + 2 * dy) as isize - 3;
                        if !(0..T as isize).contains(&iy) {
                            continue;
                        }
                        for dx in 0..4 {
                            let ix = (xx + 2 * dx) as isize - 3;
                            if (0..F as isize).contains(&ix) {
                                s += h.data[iy as usize * F + ix as usize]
                                    * self.out_weights[(o * 4 + dy) * 4 + dx];
                            }
                        }
                    }
                    let i = (y * F + xx) * 2 + o;
                    est[i] = x.data[i] / (1.0 + (-s).exp());
                }
            }
        }
        est
    }
}

pub struct Net {
    nets: Vec<UNet>,
}

impl Net {
    /// Loads the weights file made by `scripts/models/convert_spleeter.py`.
    pub fn load(bytes: &[u8]) -> Result<Self, String> {
        let st = SafeTensors::deserialize(bytes).map_err(|e| e.to_string())?;
        let get = |name: &str| tensor(&st, name);
        let layer = |p: &str, transposed: bool| -> Result<Layer, String> {
            let (w, s) = get(&format!("{p}.weight"))?;
            Ok(Layer::new(
                &w,
                [s[0], s[1]],
                transposed,
                get(&format!("{p}.bias"))?.0,
                get(&format!("{p}.bn_scale"))?.0,
                get(&format!("{p}.bn_shift"))?.0,
            ))
        };
        let nets = STEMS
            .iter()
            .map(|stem| {
                Ok(UNet {
                    enc: (0..6)
                        .map(|k| layer(&format!("{stem}.enc{k}"), false))
                        .collect::<Result<_, String>>()?,
                    dec: (0..6)
                        .map(|k| layer(&format!("{stem}.dec{k}"), true))
                        .collect::<Result<_, String>>()?,
                    out_weights: get(&format!("{stem}.out.weight"))?.0,
                    out_bias: get(&format!("{stem}.out.bias"))?.0,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Net { nets })
    }
}

/// `[channel][frame][bin]` magnitudes as the networks' channels-last input.
fn input(magnitudes: &[f32]) -> Map {
    let mut data = vec![0.0f32; T * F * 2];
    for (j, v) in magnitudes.iter().enumerate() {
        let (c, i) = (j / (T * F), j % (T * F));
        data[i * 2 + c] = *v;
    }
    Map {
        h: T,
        w: F,
        c: 2,
        data,
    }
}

impl super::OneStem for Net {
    fn estimate_stem(&self, stem: usize, magnitudes: &[f32]) -> Vec<f32> {
        let est = self.nets[stem].forward(&input(magnitudes));
        let mut out = vec![0.0f32; 2 * T * F];
        for (i, [l, r]) in est.as_chunks::<2>().0.iter().enumerate() {
            out[i] = *l;
            out[T * F + i] = *r;
        }
        out
    }
}

impl super::Net for Net {
    fn estimate(&self, magnitudes: Vec<f32>) -> Vec<f32> {
        let x = input(&magnitudes);
        let mut out = vec![0.0f32; STEMS.len() * 2 * T * F];
        for (s, net) in self.nets.iter().enumerate() {
            let est = net.forward(&x);
            let dst = &mut out[s * 2 * T * F..][..2 * T * F];
            for (i, [l, r]) in est.as_chunks::<2>().0.iter().enumerate() {
                dst[i] = *l;
                dst[T * F + i] = *r;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(n: usize, seed: u32) -> Vec<f32> {
        let mut x = seed.wrapping_mul(2_654_435_761).max(1);
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                (x % 2001) as f32 / 1000.0 - 1.0
            })
            .collect()
    }

    fn map(h: usize, w: usize, c: usize) -> Map {
        Map {
            h,
            w,
            c,
            data: noise(h * w * c, 7),
        }
    }

    fn at(x: &Map, y: isize, xx: isize, c: usize) -> f32 {
        if (0..x.h as isize).contains(&y) && (0..x.w as isize).contains(&xx) {
            x.data[(y as usize * x.w + xx as usize) * x.c + c]
        } else {
            0.0
        }
    }

    #[test]
    fn strided_convolution_pads_like_tensorflow_same() {
        let (inp, out) = (3, 4);
        let w = noise(out * inp * K * K, 1);
        let bias = noise(out, 2);
        let layer = Layer::new(&w, [out, inp], false, bias.clone(), vec![], vec![]);
        let x = map(8, 6, inp);
        let y = layer.down(&x);
        assert_eq!((y.h, y.w, y.c), (4, 3, out));
        for oy in 0..4 {
            for ox in 0..3 {
                for o in 0..out {
                    let mut s = bias[o];
                    for i in 0..inp {
                        for dy in 0..K {
                            for dx in 0..K {
                                let v = at(
                                    &x,
                                    (2 * oy + dy) as isize - 1,
                                    (2 * ox + dx) as isize - 1,
                                    i,
                                );
                                s += v * w[((o * inp + i) * K + dy) * K + dx];
                            }
                        }
                    }
                    let got = y.data[(oy * 3 + ox) * out + o];
                    assert!((got - s).abs() < 1e-4, "{got} vs {s}");
                }
            }
        }
    }

    #[test]
    fn transposed_convolution_crops_like_tensorflow_same() {
        let (inp, out) = (3, 2);
        let w = noise(inp * out * K * K, 3);
        let bias = noise(out, 4);
        let layer = Layer::new(&w, [inp, out], true, bias.clone(), vec![], vec![]);
        let x = map(3, 4, inp);
        let y = layer.up(&x);
        assert_eq!((y.h, y.w, y.c), (6, 8, out));
        // The full transposed convolution, then rows and columns from 1.
        for oy in 0..6 {
            for ox in 0..8 {
                for o in 0..out {
                    let mut s = bias[o];
                    for iy in 0..3 {
                        for ix in 0..4 {
                            for ky in 0..K {
                                for kx in 0..K {
                                    if 2 * iy + ky == oy + 1 && 2 * ix + kx == ox + 1 {
                                        for i in 0..inp {
                                            s += at(&x, iy as isize, ix as isize, i)
                                                * w[((i * out + o) * K + ky) * K + kx];
                                        }
                                    }
                                }
                            }
                        }
                    }
                    let got = y.data[(oy * 8 + ox) * out + o];
                    assert!((got - s).abs() < 1e-4, "{got} vs {s}");
                }
            }
        }
    }
}

//! Broken inputs for the parsers of other apps' files, which come from
//! disks DJ Sugar doesn't control: every cut of a good file, good files
//! with bytes changed (lengths most of all), and noise. A parser may turn
//! any of them down, but must not panic, hang or ask for huge memory.

/// A small fixed-seed generator, so a failure repeats.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Runs `parse` on broken versions of each of `good`.
pub fn check(good: &[Vec<u8>], mut parse: impl FnMut(&[u8])) {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for g in good {
        parse(g);
        for cut in 0..g.len() {
            parse(&g[..cut]);
        }
        for _ in 0..400 {
            let mut b = g.clone();
            for _ in 0..1 + rng.below(4) {
                if b.is_empty() {
                    break;
                }
                let at = rng.below(b.len());
                b[at] = match rng.below(4) {
                    0 => 0xFF,
                    1 => 0,
                    2 => 0x80,
                    _ => rng.next() as u8,
                };
            }
            parse(&b);
        }
    }
    for _ in 0..300 {
        let len = rng.below(300);
        let noise: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
        parse(&noise);
    }
}

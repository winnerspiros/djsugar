//! Keep going's picker: which tracks follow the end of Up Next when the
//! DJ's own list runs out in Player mode (see `browser/keep_going.rs`).
//! A pick mixes with the track before it, the way Smooth flow orders the
//! Automix list: the fewest Camelot steps, then the smallest tempo change.
//! A little chance among the best few keeps two runs from being the same.

use djsugar_engine::key::{Key, Mixing};

use crate::automix_order::{bpm_jump, key_steps};

/// What picking needs to know about a track.
#[derive(Clone, Debug, Default)]
pub struct Item {
    /// 0 when unknown.
    pub bpm: f64,
    pub key: Option<Key>,
    /// The main artist, lower case ("bicep" for "Bicep, Hammer").
    pub artist: String,
}

/// How many of the best tracks a pick is drawn from.
const BEST: usize = 4;
/// How much worse than the best a track may be and still be drawn.
const SLACK: f64 = 0.75;

/// What following `a` with `b` costs: key steps, plus the tempo change
/// with 6% counting as one step, plus the same artist again.
pub fn cost(a: &Item, b: &Item, mixing: Mixing) -> f64 {
    let key = match (a.key, b.key) {
        (Some(x), Some(y)) => f64::from(key_steps(x, y, mixing)),
        _ => 3.0,
    };
    let tempo = bpm_jump(a.bpm, b.bpm).map_or(2.0, |j| (j.abs() / 6.0).min(6.0));
    let same = !a.artist.is_empty() && a.artist == b.artist;
    key + tempo + if same { 2.0 } else { 0.0 }
}

/// Up to `n` picks from `pool` (indices into it), each following the one
/// before, the first following `after` (anything, when None).
/// `random` gives numbers for the draw among the best.
pub fn picks(
    pool: &[Item],
    after: Option<&Item>,
    n: usize,
    mixing: Mixing,
    random: &mut dyn FnMut() -> u64,
) -> Vec<usize> {
    let mut left: Vec<usize> = (0..pool.len()).collect();
    let mut out = Vec::new();
    let mut last = after.cloned();
    while out.len() < n && !left.is_empty() {
        let at = match &last {
            None => (random() % left.len() as u64) as usize,
            Some(prev) => {
                let mut scored: Vec<(f64, usize)> = left
                    .iter()
                    .enumerate()
                    .map(|(at, &i)| (cost(prev, &pool[i], mixing), at))
                    .collect();
                scored.sort_by(|x, y| x.0.total_cmp(&y.0));
                let best = scored[0].0;
                let near = scored
                    .iter()
                    .take(BEST)
                    .take_while(|(c, _)| *c <= best + SLACK)
                    .count();
                scored[(random() % near as u64) as usize].1
            }
        };
        let i = left.swap_remove(at);
        last = Some(pool[i].clone());
        out.push(i);
    }
    out
}

/// Numbers for the draw, from a random seed (xorshift64*).
pub fn random() -> impl FnMut() -> u64 {
    let mut seed = [0u8; 8];
    let _ = getrandom::fill(&mut seed);
    let mut state = u64::from_le_bytes(seed) | 1;
    move || {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        state.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(bpm: f64, key: &str, artist: &str) -> Item {
        Item {
            bpm,
            key: Key::parse(key),
            artist: artist.into(),
        }
    }

    #[test]
    fn picks_what_mixes() {
        let pool = [
            item(140.0, "3B", "a"),
            item(123.0, "8A", "b"),
            item(124.0, "1A", "c"),
        ];
        let after = item(124.0, "8A", "d");
        let mut r = || 0;
        // Every draw is the first of the best: 8A, then what mixes with it.
        let got = picks(&pool, Some(&after), 3, Mixing::Harmonic, &mut r);
        assert_eq!(got[0], 1);
        assert_eq!(got.len(), 3);
    }

    #[test]
    fn half_time_counts_as_the_same_tempo() {
        let a = item(140.0, "8A", "x");
        assert!(cost(&a, &item(70.0, "8A", "y"), Mixing::Harmonic) < 0.01);
    }

    #[test]
    fn avoids_the_same_artist_twice() {
        let after = item(124.0, "8A", "bicep");
        let same = item(124.0, "8A", "bicep");
        let other = item(125.0, "9A", "floating points");
        let m = Mixing::Harmonic;
        assert!(cost(&after, &other, m) < cost(&after, &same, m));
    }

    #[test]
    fn draws_only_among_the_best() {
        let pool = [
            item(124.0, "8A", "a"),
            item(124.0, "8A", "b"),
            item(170.0, "2B", "c"),
        ];
        let after = item(124.0, "8A", "d");
        for n in 0..10u64 {
            let mut r = move || n;
            let got = picks(&pool, Some(&after), 1, Mixing::Harmonic, &mut r);
            assert_ne!(got, [2], "the far one is never drawn");
        }
    }

    #[test]
    fn empty_pool_picks_nothing() {
        let mut r = || 0;
        assert!(picks(&[], None, 3, Mixing::Harmonic, &mut r).is_empty());
    }
}

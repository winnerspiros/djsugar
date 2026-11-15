//! Putting the Automix list in order: shuffled, by tempo, or along a
//! harmonic path where each track mixes into the next.
//!
//! An order is worked out once, when it is picked; dragging tracks around
//! afterwards keeps working as before. The first `fixed` tracks (the one
//! playing and the one waiting on a deck) stay where they are.

use djsugar_engine::key::{Key, Mixing};

/// The orders the Automix panel's Order menu offers, in its order.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Order {
    Shuffle,
    BpmUp,
    BpmDown,
    /// Each track's key mixes into the next one's where it can.
    Harmonic,
    /// Key and tempo together: the smoothest mixes overall.
    Flow,
    Reverse,
}

pub const ORDERS: [Order; 6] = [
    Order::Shuffle,
    Order::BpmUp,
    Order::BpmDown,
    Order::Harmonic,
    Order::Flow,
    Order::Reverse,
];

impl Order {
    pub fn name(self) -> &'static str {
        match self {
            Order::Shuffle => "Shuffle",
            Order::BpmUp => "BPM low to high",
            Order::BpmDown => "BPM high to low",
            Order::Harmonic => "Harmonic path",
            Order::Flow => "Smooth flow",
            Order::Reverse => "Reverse",
        }
    }
}

/// What ordering needs to know about a track.
#[derive(Clone, Copy, Debug, Default)]
pub struct Item {
    /// 0 when unknown.
    pub bpm: f64,
    pub key: Option<Key>,
}

/// The Camelot number (1..=12) and whether the key is minor (A).
fn camelot(k: Key) -> (i32, bool) {
    let s = k.camelot();
    let n = s[..s.len() - 1].parse().unwrap_or(1);
    (n, k.minor)
}

/// Steps around the Camelot wheel from `a` to `b`: 0 for the same key,
/// a neighbouring number or the relative key count as one step (when
/// they mix by the rule picked), each further number or letter one more.
pub fn key_steps(a: Key, b: Key, mixing: Mixing) -> u32 {
    if a == b {
        return 0;
    }
    if a.mixes(b, mixing) {
        return 1;
    }
    let ((na, ma), (nb, mb)) = (camelot(a), camelot(b));
    let d = (na - nb).rem_euclid(12);
    let dn = d.min(12 - d) as u32;
    dn + u32::from(ma != mb)
}

/// The tempo change from `a` to `b` in percent, counting half and double
/// time as the same tempo; None when either is unknown.
pub fn bpm_jump(a: f64, b: f64) -> Option<f64> {
    if a <= 0.0 || b <= 0.0 {
        return None;
    }
    [b, b * 2.0, b / 2.0]
        .into_iter()
        .map(|b| (b / a - 1.0) * 100.0)
        .min_by(|x, y| x.abs().total_cmp(&y.abs()))
}

/// What a mix from `a` into `b` costs for a path order: key steps, plus the
/// tempo jump (6% counting as one key step) for Smooth flow, or as a mere
/// tie-break for Harmonic path.
fn cost(a: &Item, b: &Item, order: Order, mixing: Mixing) -> f64 {
    let key = match (a.key, b.key) {
        (Some(x), Some(y)) => key_steps(x, y, mixing) as f64,
        _ => 4.0,
    };
    let tempo = bpm_jump(a.bpm, b.bpm).map_or(3.0, |j| (j.abs() / 6.0).min(5.0));
    match order {
        Order::Flow => key + tempo,
        _ => key + 0.01 * tempo,
    }
}

/// A new order for `items`: indices into it, the first `fixed` unchanged.
/// `random` gives numbers for shuffling.
pub fn order(
    items: &[Item],
    fixed: usize,
    order: Order,
    mixing: Mixing,
    random: &mut dyn FnMut() -> u64,
) -> Vec<usize> {
    let fixed = fixed.min(items.len());
    let mut out: Vec<usize> = (0..fixed).collect();
    let mut rest: Vec<usize> = (fixed..items.len()).collect();
    match order {
        Order::Shuffle => {
            for i in (1..rest.len()).rev() {
                let j = (random() % (i as u64 + 1)) as usize;
                rest.swap(i, j);
            }
        }
        Order::Reverse => rest.reverse(),
        Order::BpmUp | Order::BpmDown => {
            // Unknown tempos go last, in the order they were.
            let (mut known, unknown): (Vec<usize>, Vec<usize>) =
                rest.into_iter().partition(|&i| items[i].bpm > 0.0);
            let key_number = |i: usize| items[i].key.map_or(99, |k| camelot(k).0);
            known.sort_by(|&a, &b| {
                let by_bpm = items[a].bpm.total_cmp(&items[b].bpm);
                let by_bpm = if order == Order::BpmDown {
                    by_bpm.reverse()
                } else {
                    by_bpm
                };
                by_bpm.then(key_number(a).cmp(&key_number(b)))
            });
            known.extend(unknown);
            rest = known;
        }
        Order::Harmonic | Order::Flow => {
            // Tracks with nothing to go on go last, in the order they were.
            let (known, unknown): (Vec<usize>, Vec<usize>) = rest.into_iter().partition(|&i| {
                items[i].key.is_some() || (order == Order::Flow && items[i].bpm > 0.0)
            });
            let mut path = path(items, out.last().copied(), known, order, mixing);
            path.extend(unknown);
            rest = path;
        }
    }
    out.extend(rest);
    out
}

/// Orders `todo` so each mix costs as little as it can: always the cheapest
/// next track from where the path is (nearest neighbour), then improved by
/// turning stretches around while that lowers the total (2-opt). The path
/// starts after `from`, or with the first of `todo` when there is none.
fn path(
    items: &[Item],
    from: Option<usize>,
    mut todo: Vec<usize>,
    order: Order,
    mixing: Mixing,
) -> Vec<usize> {
    let c = |a: usize, b: usize| cost(&items[a], &items[b], order, mixing);
    let mut path: Vec<usize> = Vec::with_capacity(todo.len() + 1);
    // The track the path leads on from: fixed, so not part of the result.
    let anchored = from.is_some();
    match from {
        Some(f) => path.push(f),
        None if !todo.is_empty() => path.push(todo.remove(0)),
        None => return Vec::new(),
    }
    while !todo.is_empty() {
        let last = *path.last().unwrap();
        // The first of equally good ones, so ties keep the list's order.
        let (i, _) = todo
            .iter()
            .enumerate()
            .min_by(|(_, &a), (_, &b)| c(last, a).total_cmp(&c(last, b)))
            .unwrap();
        path.push(todo.remove(i));
    }
    // 2-opt on an open path whose first track stays: reversing path[i..=j]
    // swaps the edges (i-1, i) and (j, j+1) for (i-1, j) and (i, j+1).
    let n = path.len();
    let mut better = true;
    let mut rounds = 0;
    while better && rounds < 50 {
        better = false;
        rounds += 1;
        for i in 1..n.saturating_sub(1) {
            for j in i + 1..n {
                let before = c(path[i - 1], path[i])
                    + if j + 1 < n {
                        c(path[j], path[j + 1])
                    } else {
                        0.0
                    };
                let after = c(path[i - 1], path[j])
                    + if j + 1 < n {
                        c(path[i], path[j + 1])
                    } else {
                        0.0
                    };
                if after + 1e-9 < before {
                    path[i..=j].reverse();
                    better = true;
                }
            }
        }
    }
    if anchored {
        path.remove(0);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(bpm: f64, key: &str) -> Item {
        Item {
            bpm,
            key: Key::parse(key),
        }
    }

    fn no_random() -> u64 {
        0
    }

    #[test]
    fn camelot_steps() {
        let k = |s| Key::parse(s).unwrap();
        let h = Mixing::Harmonic;
        assert_eq!(key_steps(k("8A"), k("8A"), h), 0);
        assert_eq!(key_steps(k("8A"), k("9A"), h), 1);
        assert_eq!(key_steps(k("8A"), k("8B"), h), 1);
        assert_eq!(key_steps(k("12A"), k("1A"), h), 1, "round the wheel");
        assert_eq!(key_steps(k("8A"), k("10A"), h), 2);
        assert_eq!(key_steps(k("8A"), k("9B"), h), 2);
        assert_eq!(key_steps(k("8A"), k("9B"), Mixing::Fuzzy), 1);
        assert_eq!(key_steps(k("8A"), k("2A"), h), 6);
    }

    #[test]
    fn bpm_orders_keep_unknown_tempos_last() {
        let items = [
            item(128.0, "8A"),
            item(0.0, "8A"),
            item(120.0, "1A"),
            item(124.0, "2A"),
        ];
        let mut r = no_random;
        assert_eq!(
            order(&items, 0, Order::BpmUp, Mixing::Harmonic, &mut r),
            [2, 3, 0, 1]
        );
        assert_eq!(
            order(&items, 0, Order::BpmDown, Mixing::Harmonic, &mut r),
            [0, 3, 2, 1]
        );
        assert_eq!(
            order(&items, 1, Order::BpmUp, Mixing::Harmonic, &mut r),
            [0, 2, 3, 1]
        );
    }

    #[test]
    fn harmonic_path_walks_the_wheel() {
        // Fixed first track 8A; the rest scrambled.
        let items = [
            item(124.0, "8A"),
            item(124.0, "11A"),
            item(124.0, "9A"),
            item(124.0, "10A"),
            item(124.0, ""),
            item(124.0, "12A"),
        ];
        let got = order(&items, 1, Order::Harmonic, Mixing::Harmonic, &mut no_random);
        assert_eq!(got[0], 0);
        assert_eq!(*got.last().unwrap(), 4, "no key goes last");
        // Every mix is a single step around the wheel.
        let total: u32 = got[..5]
            .windows(2)
            .map(|w| {
                key_steps(
                    items[w[0]].key.unwrap(),
                    items[w[1]].key.unwrap(),
                    Mixing::Harmonic,
                )
            })
            .sum();
        assert_eq!(total, 4, "{got:?}");
    }

    #[test]
    fn smooth_flow_also_avoids_tempo_jumps() {
        // Same key throughout: only tempo decides.
        let items = [
            item(120.0, "8A"),
            item(140.0, "8A"),
            item(121.0, "8A"),
            item(139.0, "8A"),
        ];
        let got = order(&items, 1, Order::Flow, Mixing::Harmonic, &mut no_random);
        assert_eq!(got, [0, 2, 3, 1]);
    }

    #[test]
    fn half_and_double_time_count_as_the_same_tempo() {
        assert!(bpm_jump(140.0, 70.0).unwrap().abs() < 1e-9);
        assert!((bpm_jump(100.0, 103.0).unwrap() - 3.0).abs() < 1e-9);
        assert_eq!(bpm_jump(0.0, 120.0), None);
    }

    #[test]
    fn shuffle_and_reverse_keep_the_fixed_tracks() {
        let items = [Item::default(); 6];
        let mut n = 12345u64;
        let mut r = move || {
            n = n
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            n >> 33
        };
        let got = order(&items, 2, Order::Shuffle, Mixing::Harmonic, &mut r);
        assert_eq!(&got[..2], &[0, 1]);
        let mut sorted = got.clone();
        sorted.sort();
        assert_eq!(sorted, [0, 1, 2, 3, 4, 5]);
        assert_eq!(
            order(&items, 2, Order::Reverse, Mixing::Harmonic, &mut r),
            [0, 1, 5, 4, 3, 2]
        );
    }
}

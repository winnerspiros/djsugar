//! Tracks that go together: the ones linked by hand, and the ones played one after another. Each list holds a
//! track's key and the keys of its partners, both ways round.

pub type Pairs = Vec<(String, Vec<String>)>;

/// Pairs two tracks; false when they already were, or are the same track.
pub fn pair(pairs: &mut Pairs, a: &str, b: &str) -> bool {
    if a == b || paired(pairs, a, b) {
        return false;
    }
    for (from, to) in [(a, b), (b, a)] {
        match pairs.iter_mut().find(|(k, _)| k == from) {
            Some((_, with)) => with.push(to.to_string()),
            None => pairs.push((from.to_string(), vec![to.to_string()])),
        }
    }
    true
}

pub fn unpair(pairs: &mut Pairs, a: &str, b: &str) {
    for (from, to) in [(a, b), (b, a)] {
        if let Some((_, with)) = pairs.iter_mut().find(|(k, _)| k == from) {
            with.retain(|k| k != to);
        }
    }
    pairs.retain(|(_, with)| !with.is_empty());
}

pub fn paired(pairs: &Pairs, a: &str, b: &str) -> bool {
    partners(pairs, a).iter().any(|k| k == b)
}

/// The partners of a track, oldest first.
pub fn partners<'a>(pairs: &'a Pairs, key: &str) -> &'a [String] {
    pairs.iter().find(|(k, _)| k == key).map_or(&[], |(_, with)| with.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pair_holds_both_ways_until_it_is_undone() {
        let mut pairs = Pairs::new();
        assert!(pair(&mut pairs, "a", "b"));
        assert!(pair(&mut pairs, "a", "c"));
        assert!(!pair(&mut pairs, "b", "a"), "already paired");
        assert!(!pair(&mut pairs, "a", "a"), "not with itself");
        assert_eq!(partners(&pairs, "a"), ["b", "c"]);
        assert_eq!(partners(&pairs, "c"), ["a"]);
        unpair(&mut pairs, "b", "a");
        assert_eq!(partners(&pairs, "a"), ["c"]);
        assert!(partners(&pairs, "b").is_empty());
        assert_eq!(pairs.len(), 2, "a track with no partner is dropped");
    }
}

/// The most tracks of the collection suggested for one track.
pub const ALIKE: usize = 12;
/// How far apart two tempos may be, as a part of the slower one, and how many steps round the key wheel two
/// keys, for the tracks to mix without effort.
const TEMPO: f32 = 0.06;
const STEPS: f32 = 1.0;

/// What is known of a track for suggesting others: its artist in lower case, its tempo and its key.
pub type Known<'a> = (&'a str, f32, Option<sugar_analysis::Key>);

/// How well a track goes with another, higher the better; None when nothing ties them. The same artist counts
/// most; a tempo within reach and a key next on the wheel count together.
pub fn alike(of: Known<'_>, other: Known<'_>) -> Option<f32> {
    let same = !of.0.is_empty() && of.0 == other.0;
    let apart = if of.1 > 0.0 && other.1 > 0.0 { (of.1 - other.1).abs() / of.1.min(other.1) } else { 1.0 };
    let steps = crate::automix::key_steps(of.2, other.2);
    let mixes = apart <= TEMPO && steps <= STEPS;
    if !same && !mixes {
        return None;
    }
    Some(if same { 2.0 } else { 0.0 } + if mixes { 1.0 - apart / TEMPO * 0.5 - steps * 0.25 } else { 0.0 })
}

#[cfg(test)]
mod alike_tests {
    use super::*;

    #[test]
    fn same_artist_first_then_what_mixes() {
        let key = |tonic| Some(sugar_analysis::Key { tonic, minor: false });
        let of = ("a", 124.0, key(0));
        let same = alike(of, ("a", 90.0, key(5))).unwrap();
        let mixes = alike(of, ("b", 125.0, key(7))).unwrap();
        assert!(same > mixes && mixes > 0.0);
        assert!(alike(of, ("a", 124.0, key(0))).unwrap() > same, "same artist and mixes");
        assert_eq!(alike(of, ("b", 140.0, key(0))), None, "too fast");
        assert_eq!(alike(of, ("b", 124.0, key(3))), None, "clashing key");
        assert_eq!(alike(("", 0.0, None), ("", 0.0, None)), None, "nothing known");
    }
}

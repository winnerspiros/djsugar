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

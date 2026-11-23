//! Musical variation for the Groove Box's tracks: new grooves in a genre's
//! style, small mutations, busier and calmer tracks and even spacing. Not
//! plain dice: each move is weighted by how strong a step is in the bar
//! (downbeats first, as in Mutable Instruments' Grids) and by how often the
//! genre's library grooves hit that step with that sound.
//!
//! Everything works on a track's three 16-bit masks ([`Track`]) and a small
//! random number generator, so it runs on the audio thread without
//! allocating. The genre statistics ([`Habits`]) are built once, when the
//! engine starts.

use crate::groove::STEPS;
use crate::kits::ROLES;
use crate::patterns::{self, Genre};

/// One track of one groove: its hits and which of them are soft or accented.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Track {
    pub hits: u32,
    pub soft: u32,
    pub accent: u32,
    /// Hits that play only half the time.
    pub maybe: u32,
}

impl Track {
    fn has(&self, step: usize) -> bool {
        self.hits & (1 << step) != 0
    }

    fn count(&self) -> u32 {
        (self.hits & 0xffff).count_ones()
    }

    /// Takes the hit off `step`, levels and all.
    fn clear(&mut self, step: usize) {
        let m = !(1u32 << step);
        self.hits &= m;
        self.soft &= m;
        self.accent &= m;
        self.maybe &= m;
    }

    /// Moves the hit on `from` (with its level) to `to`.
    fn shift(&mut self, from: usize, to: usize) {
        let carry = |mask: &mut u32| {
            if *mask & (1 << from) != 0 {
                *mask = (*mask & !(1 << from)) | 1 << to;
            }
        };
        carry(&mut self.soft);
        carry(&mut self.accent);
        carry(&mut self.maybe);
        self.hits = (self.hits & !(1 << from)) | 1 << to;
    }
}

/// What a variation does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    /// A new track in the genre's style.
    New,
    /// One or two hits move to a nearby step, now and then one more or less.
    Mutate,
    /// One more hit, on the strongest free step.
    Busier,
    /// One hit fewer, the weakest goes.
    Calmer,
    /// The same number of hits, spread evenly (a Euclidean rhythm), starting
    /// where the first hit was.
    Even,
    /// Every hit a step later or earlier.
    RotateRight,
    RotateLeft,
}

impl Move {
    /// `[GrooveN],dice` numbers them from 1.
    pub fn from_number(n: u8) -> Option<Move> {
        Some(match n {
            1 => Move::New,
            2 => Move::Mutate,
            3 => Move::Busier,
            4 => Move::Calmer,
            5 => Move::Even,
            6 => Move::RotateRight,
            7 => Move::RotateLeft,
            _ => return None,
        })
    }
}

/// How strong a step is in the bar: the downbeat, then beat 3, beats 2 and
/// 4, the off-beat 8ths, then the 16ths in between.
pub fn metric(step: usize) -> f32 {
    match step % STEPS {
        0 => 1.0,
        8 => 0.85,
        4 | 12 => 0.75,
        2 | 6 | 10 | 14 => 0.5,
        _ => 0.25,
    }
}

/// How often each sound hits each step, and follows a hit or a rest with a
/// hit, in one genre's library grooves.
#[derive(Clone, Debug, Default)]
pub struct Habit {
    /// Grooves that use the sound at all.
    grooves: u16,
    /// Hits on each step.
    on: [u16; STEPS],
    /// [step][previous step hit?]: (times, of which hits).
    after: [[(u16, u16); 2]; STEPS],
    /// Soft and accented hits.
    soft: u16,
    accent: u16,
    hits: u16,
}

impl Habit {
    /// How likely a hit is on `step` (0-1).
    fn chance(&self, step: usize) -> f32 {
        if self.grooves == 0 {
            return 0.0;
        }
        self.on[step] as f32 / self.grooves as f32
    }

    /// How likely a hit is on `step` after a hit (or rest) on the step
    /// before, smoothed toward the step's own chance.
    fn chance_after(&self, step: usize, prev: bool) -> f32 {
        let (n, h) = self.after[step][prev as usize];
        (h as f32 + 2.0 * self.chance(step)) / (n as f32 + 2.0)
    }
}

/// [`Habit`]s for every genre and sound, built from the groove library.
pub struct Habits {
    genres: Vec<[Habit; ROLES.len()]>,
}

impl Habits {
    pub fn from_library() -> Habits {
        Habits::from(patterns::library())
    }

    /// The library's habits, built once and shared by every Groove Box.
    pub fn shared() -> std::sync::Arc<Habits> {
        static SHARED: std::sync::OnceLock<std::sync::Arc<Habits>> = std::sync::OnceLock::new();
        SHARED
            .get_or_init(|| std::sync::Arc::new(Habits::from_library()))
            .clone()
    }

    pub fn from(library: &[Genre]) -> Habits {
        let genres = library
            .iter()
            .map(|genre| {
                let mut h: [Habit; ROLES.len()] = Default::default();
                for groove in &genre.grooves {
                    for (role, steps) in groove.rows.iter().enumerate() {
                        let bits = steps.hits & 0xffff;
                        if bits == 0 {
                            continue;
                        }
                        let habit = &mut h[role];
                        habit.grooves += 1;
                        habit.hits += bits.count_ones() as u16;
                        habit.soft += (steps.soft & bits).count_ones() as u16;
                        habit.accent += (steps.accent & bits).count_ones() as u16;
                        for s in 0..STEPS {
                            let hit = bits & (1 << s) != 0;
                            let prev = bits & (1 << ((s + STEPS - 1) % STEPS)) != 0;
                            if hit {
                                habit.on[s] += 1;
                            }
                            let slot = &mut habit.after[s][prev as usize];
                            slot.0 += 1;
                            slot.1 += hit as u16;
                        }
                    }
                }
                h
            })
            .collect();
        Habits { genres }
    }

    fn get(&self, genre: usize, role: usize) -> Option<&Habit> {
        self.genres.get(genre)?.get(role).filter(|h| h.grooves > 0)
    }
}

/// A small, fast random number generator (xorshift32).
#[derive(Clone, Copy, Debug)]
pub struct Rng(pub u32);

impl Rng {
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// 0 to 1.
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Picks an index with a chance in proportion to its weight; None when
    /// every weight is 0.
    fn pick(&mut self, weights: &[f32; STEPS]) -> Option<usize> {
        let total: f32 = weights.iter().sum();
        if total <= 0.0 {
            return None;
        }
        let mut at = self.unit() * total;
        for (i, &w) in weights.iter().enumerate() {
            if w > 0.0 && at < w {
                return Some(i);
            }
            at -= w;
        }
        weights.iter().rposition(|&w| w > 0.0)
    }
}

/// How much a step suits `role` in this genre: half how strong the step
/// is for that kind of sound, half how often the genre hits it with it.
fn suits(habit: Option<&Habit>, role: usize, step: usize) -> f32 {
    let m = match role {
        // Hats, clap, rim and cowbell sit on the 8ths, the off-beats most.
        2..=6 => match step % 4 {
            2 => 1.0,
            0 => 0.7,
            _ => 0.35,
        },
        _ => metric(step),
    };
    match habit {
        Some(h) => 0.5 * m + 0.5 * h.chance(step),
        None => m,
    }
}

/// Changes `track` (playing the kit's sound `role`, 0-8) by `mv`, in the
/// style of `genre`.
pub fn apply(
    mv: Move,
    track: &mut Track,
    role: usize,
    genre: usize,
    habits: &Habits,
    rng: &mut Rng,
) {
    let habit = habits.get(genre, role);
    match mv {
        Move::New => *track = new_track(habit, role, rng),
        Move::Mutate => mutate(track, habit, role, rng),
        Move::Busier => busier(track, habit, role, rng),
        Move::Calmer => calmer(track, habit, role, rng),
        Move::Even => even(track),
        Move::RotateRight => rotate(track, 1),
        Move::RotateLeft => rotate(track, STEPS - 1),
    }
}

/// A new track: step by step, a hit as often as the genre follows a hit or
/// a rest on the step before with one here; levels as often as the genre
/// uses them. With no habit (a sound the genre never uses), a sparse track
/// on strong steps.
fn new_track(habit: Option<&Habit>, role: usize, rng: &mut Rng) -> Track {
    let mut t = Track::default();
    let Some(h) = habit else {
        let mut w = [0f32; STEPS];
        for (s, w) in w.iter_mut().enumerate() {
            *w = suits(None, role, s).powi(3);
        }
        for _ in 0..2 {
            if let Some(s) = rng.pick(&w) {
                t.hits |= 1 << s;
                w[s] = 0.0;
            }
        }
        return t;
    };
    // Twice round, so step 0 sees the end of the bar before it.
    let mut prev = false;
    for pass in 0..2 {
        for s in 0..STEPS {
            let hit = rng.unit() < h.chance_after(s, prev);
            if pass == 1 && hit {
                t.hits |= 1 << s;
            }
            prev = hit;
        }
    }
    if t.hits == 0 {
        // Never empty: the genre's favourite step.
        let best = (0..STEPS).max_by_key(|&s| h.on[s]).unwrap_or(0);
        t.hits = 1 << best;
    }
    let hits = h.hits.max(1) as f32;
    let hit = t.hits;
    for s in (0..STEPS).filter(|&s| hit & (1 << s) != 0) {
        let r = rng.unit();
        if r < h.accent as f32 / hits {
            t.accent |= 1 << s;
        } else if r < (h.accent + h.soft) as f32 / hits {
            t.soft |= 1 << s;
        }
    }
    t
}

/// Whether a hit is part of the track's backbone and stays put: the kick's
/// downbeat, the snare's backbeat.
fn anchored(role: usize, step: usize) -> bool {
    matches!((role, step), (0, 0) | (1, 4) | (1, 12))
}

fn mutate(t: &mut Track, habit: Option<&Habit>, role: usize, rng: &mut Rng) {
    if t.hits == 0 {
        *t = new_track(habit, role, rng);
        return;
    }
    let moves = 1 + (rng.next_u32() % 2) as usize;
    for _ in 0..moves {
        // A weak hit is likelier to move.
        let mut from = [0f32; STEPS];
        for (s, w) in from.iter_mut().enumerate() {
            if t.has(s) && !anchored(role, s) {
                *w = 1.2 - suits(habit, role, s);
            }
        }
        let Some(f) = rng.pick(&from) else { break };
        // To a free step one or two away, likelier where it suits.
        let mut to = [0f32; STEPS];
        for d in [1usize, 2, STEPS - 1, STEPS - 2] {
            let s = (f + d) % STEPS;
            if !t.has(s) {
                to[s] = suits(habit, role, s) + if d == 1 || d == STEPS - 1 { 0.2 } else { 0.0 };
            }
        }
        if let Some(s) = rng.pick(&to) {
            t.shift(f, s);
        }
    }
    // Now and then one more or one fewer.
    match rng.next_u32() % 4 {
        0 => busier(t, habit, role, rng),
        1 if t.count() > 2 => calmer(t, habit, role, rng),
        _ => {}
    }
}

fn busier(t: &mut Track, habit: Option<&Habit>, role: usize, rng: &mut Rng) {
    let mut w = [0f32; STEPS];
    for (s, w) in w.iter_mut().enumerate() {
        if !t.has(s) {
            // Strongly favour the best few steps.
            *w = suits(habit, role, s).powi(4);
        }
    }
    if let Some(s) = rng.pick(&w) {
        t.hits |= 1 << s;
    }
}

fn calmer(t: &mut Track, habit: Option<&Habit>, role: usize, rng: &mut Rng) {
    let mut w = [0f32; STEPS];
    for (s, w) in w.iter_mut().enumerate() {
        if t.has(s) && !anchored(role, s) {
            *w = (1.05 - suits(habit, role, s)).powi(4);
        }
    }
    if let Some(s) = rng.pick(&w) {
        t.clear(s);
    }
}

/// `k` hits spread as evenly as they go over 16 steps (Bjorklund's result,
/// by the bucket method), the first on step 0.
pub fn euclid(k: u32) -> u32 {
    let k = k.min(STEPS as u32);
    let mut bits = 0;
    for s in 0..STEPS as u32 {
        // A hit wherever the running total of k/16 crosses a whole number.
        if (s * k) % (STEPS as u32) < k {
            bits |= 1 << s;
        }
    }
    bits
}

fn even(t: &mut Track) {
    let k = t.count();
    if k == 0 {
        return;
    }
    let first = (0..STEPS).find(|&s| t.has(s)).unwrap_or(0);
    let bits = euclid(k);
    let rotated = ((bits << first) | (bits >> (STEPS - first))) & 0xffff;
    // Levels don't carry over a respacing.
    *t = Track {
        hits: rotated,
        ..Track::default()
    };
}

fn rotate(t: &mut Track, by: usize) {
    let r = |m: u32| ((m << by) | (m >> (STEPS - by))) & 0xffff;
    *t = Track {
        hits: r(t.hits & 0xffff),
        soft: r(t.soft & 0xffff),
        accent: r(t.accent & 0xffff),
        maybe: r(t.maybe & 0xffff),
    };
}

/// With Vary at `vary` (0-1), whether a hit on `step` of a track playing
/// `role` plays this time: weak steps drop out more, the backbone never.
pub fn keeps(
    vary: f32,
    role: usize,
    step: usize,
    genre: usize,
    habits: &Habits,
    rng: &mut Rng,
) -> bool {
    if vary <= 0.0 || anchored(role, step) {
        return true;
    }
    let drop = vary * 0.45 * (1.0 - suits(habits.get(genre, role), role, step));
    rng.unit() >= drop
}

/// With Vary at `vary`, whether an empty `step` of a track playing `role`
/// gets a soft ghost hit this time: on steps the genre likes for the sound,
/// for snares, hats and percussion only.
pub fn ghost(
    vary: f32,
    role: usize,
    step: usize,
    genre: usize,
    habits: &Habits,
    rng: &mut Rng,
) -> bool {
    if vary <= 0.0 || !(1..=6).contains(&role) {
        return false;
    }
    let likes = habits.get(genre, role).map_or(0.0, |h| h.chance(step));
    rng.unit() < vary * 0.3 * likes
}

/// A hit's loudness with Vary at `vary`: up to a third louder or softer.
pub fn humanize(velocity: f32, vary: f32, rng: &mut Rng) -> f32 {
    velocity * (1.0 + (rng.unit() * 2.0 - 1.0) * 0.33 * vary)
}

/// Undo steps kept.
const UNDO: usize = 16;

/// A Groove Box's variation state: the move waiting for the next bar, the
/// random numbers and the undo steps. Fixed size, so it lives on the audio
/// thread.
pub struct Dice {
    pub habits: std::sync::Arc<Habits>,
    pub rng: Rng,
    /// (move, track: None for every unlocked track), for the next bar.
    pub pending: Option<(Move, Option<usize>)>,
    /// Before each move: the groove it changed and its tracks.
    undo: [(usize, [Track; crate::groove::ROWS]); UNDO],
    undo_len: usize,
    undo_at: usize,
}

impl Dice {
    pub fn new(habits: std::sync::Arc<Habits>, seed: u32) -> Dice {
        Dice {
            habits,
            rng: Rng(seed | 1),
            pending: None,
            undo: [(0, [Track::default(); crate::groove::ROWS]); UNDO],
            undo_len: 0,
            undo_at: 0,
        }
    }

    /// Keeps groove `slot`'s tracks before a move.
    pub fn remember(&mut self, slot: usize, tracks: [Track; crate::groove::ROWS]) {
        self.undo[self.undo_at] = (slot, tracks);
        self.undo_at = (self.undo_at + 1) % UNDO;
        self.undo_len = (self.undo_len + 1).min(UNDO);
    }

    /// The last tracks kept, to put back.
    pub fn take_undo(&mut self) -> Option<(usize, [Track; crate::groove::ROWS])> {
        if self.undo_len == 0 {
            return None;
        }
        self.undo_len -= 1;
        self.undo_at = (self.undo_at + UNDO - 1) % UNDO;
        Some(self.undo[self.undo_at])
    }

    pub fn can_undo(&self) -> bool {
        self.undo_len > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn habits() -> Habits {
        Habits::from_library()
    }

    #[test]
    fn vary_drops_weak_hits_more_and_never_the_backbone() {
        let h = habits();
        let mut rng = Rng(5);
        let kept = |step: usize, rng: &mut Rng| {
            (0..1000)
                .filter(|_| keeps(1.0, 2, step, 0, &h, rng))
                .count()
        };
        assert!(
            kept(1, &mut rng) < kept(2, &mut rng),
            "a 16th drops out more than an off-beat 8th"
        );
        assert!(
            (0..1000).all(|_| keeps(1.0, 0, 0, 0, &h, &mut rng)),
            "the kick's downbeat stays"
        );
        assert!(
            (0..1000).all(|_| keeps(0.0, 2, 1, 0, &h, &mut rng)),
            "no Vary, no change"
        );
        let v: Vec<f32> = (0..1000).map(|_| humanize(1.0, 1.0, &mut rng)).collect();
        assert!(v.iter().all(|&x| (0.66..=1.34).contains(&x)));
        assert!(v.iter().any(|&x| x < 0.9) && v.iter().any(|&x| x > 1.1));
    }

    #[test]
    fn euclid_spreads_hits_evenly() {
        assert_eq!(euclid(4), 0b0001_0001_0001_0001);
        assert_eq!(euclid(0), 0);
        assert_eq!(euclid(16), 0xffff);
        assert_eq!(euclid(3).count_ones(), 3);
        assert_eq!(euclid(5).count_ones(), 5);
        assert_eq!(euclid(7) & 1, 1, "starts on the downbeat");
    }

    #[test]
    fn even_keeps_the_count_and_the_first_hit() {
        let mut t = Track {
            hits: 0b0000_0000_0110_0110,
            ..Track::default()
        };
        even(&mut t);
        assert_eq!(t.count(), 4);
        assert!(t.has(1));
        assert_eq!(t.hits, 0b0010_0010_0010_0010);
    }

    #[test]
    fn rotate_moves_every_hit_and_wraps() {
        let mut t = Track {
            hits: 1 | 1 << 15,
            accent: 1 << 15,
            ..Track::default()
        };
        rotate(&mut t, 1);
        assert_eq!(t.hits, 0b11);
        assert_eq!(t.accent, 1);
        rotate(&mut t, STEPS - 1);
        assert_eq!(t.hits, 1 | 1 << 15);
    }

    #[test]
    fn new_kicks_in_house_mostly_hit_the_beats() {
        let h = habits();
        let lib = patterns::library();
        let house = lib
            .iter()
            .position(|g| g.name.to_lowercase().contains("house"))
            .unwrap();
        let mut rng = Rng(7);
        let (mut on_beat, mut all) = (0, 0);
        for _ in 0..200 {
            let mut t = Track::default();
            apply(Move::New, &mut t, 0, house, &h, &mut rng);
            assert_ne!(t.hits, 0);
            on_beat += (t.hits & 0x1111).count_ones();
            all += t.count();
        }
        assert!(
            on_beat as f32 > all as f32 * 0.6,
            "{on_beat} of {all} on the beat"
        );
    }

    #[test]
    fn busier_and_calmer_add_or_take_one_hit() {
        let h = habits();
        let mut rng = Rng(3);
        let mut t = Track {
            hits: 0x1111,
            ..Track::default()
        };
        apply(Move::Busier, &mut t, 0, 0, &h, &mut rng);
        assert_eq!(t.count(), 5);
        assert_eq!(t.hits & 0x1111, 0x1111, "the old hits stay");
        apply(Move::Calmer, &mut t, 0, 0, &h, &mut rng);
        apply(Move::Calmer, &mut t, 0, 0, &h, &mut rng);
        assert_eq!(t.count(), 3);
        assert!(t.has(0), "the kick keeps its downbeat");
    }

    #[test]
    fn mutate_changes_a_little_and_keeps_the_kick_on_one() {
        let h = habits();
        let mut rng = Rng(11);
        for _ in 0..100 {
            let mut t = Track {
                hits: 0x1111,
                ..Track::default()
            };
            apply(Move::Mutate, &mut t, 0, 0, &h, &mut rng);
            assert!(t.has(0));
            let changed = (t.hits ^ 0x1111).count_ones();
            assert!(changed <= 6, "{changed} steps changed");
        }
    }

    #[test]
    fn levels_move_with_their_hits() {
        let mut t = Track {
            hits: 1 << 3,
            accent: 1 << 3,
            ..Track::default()
        };
        t.shift(3, 5);
        assert_eq!(t.hits, 1 << 5);
        assert_eq!(t.accent, 1 << 5);
    }
}

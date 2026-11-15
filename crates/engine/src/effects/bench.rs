//! The FX benchmark and quality harness: what each effect costs per frame,
//! what the whole rack costs with four decks of effects, how much memory it
//! holds, and checks that the audio thread never allocates and that every
//! effect stays clean and finite at its extremes.
//!
//! The benchmark is ignored by default (it takes a few seconds and only
//! means something in a release build):
//!
//! ```text
//! cargo test --release -p djsugar-engine fx_bench -- --ignored --nocapture
//! ```

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::time::Instant;

use super::*;

/// Counts the allocations made on each thread, so a test can check that a
/// stretch of audio-thread code made none (tests run on several threads).
struct Counting;

thread_local! {
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

// SAFETY: forwards to the system allocator; the counter is a plain
// thread-local `Cell` with const initialisation, which never allocates.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.with(|a| a.set(a.get() + 1));
        // SAFETY: same contract as ours.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: same contract as ours.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.with(|a| a.set(a.get() + 1));
        // SAFETY: same contract as ours.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn allocations() -> usize {
    ALLOCATIONS.with(Cell::get)
}

const SR: f32 = 48_000.0;
/// Frames per audio callback in the rack tests: a typical small buffer.
const BUFFER: usize = 256;

/// White noise from a fixed seed, as a stand-in for music.
fn noise(frames: usize) -> Vec<f32> {
    let mut seed = 0x1234_5678u32;
    (0..frames)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed as f32 / u32::MAX as f32 * 2.0 - 1.0) * 0.5
        })
        .collect()
}

/// An instance of `kind` with parameters `p`, ready to run.
fn instance(kind: Kind, p: [f32; MAX_PARAMETERS]) -> Instance {
    let mut inst = Instance::with_memory(SR);
    inst.kind = Some(kind);
    inst.reset();
    inst.prepare(
        &p,
        Ctx {
            sample_rate: SR,
            beat_seconds: 0.5,
            beat: None,
        },
    );
    inst
}

fn defaults(kind: Kind) -> [f32; MAX_PARAMETERS] {
    let d = MANIFESTS[kind as usize].defaults;
    std::array::from_fn(|k| d.get(k).copied().unwrap_or(0.5) as f32)
}

/// A rack with each deck's unit playing its three default effects (Reverb,
/// Echo, Gate) on its deck, and the Beat FX echoing decks 1 and 2: twelve
/// deck effects and two Beat FX instances.
fn busy_rack(reg: &ControlRegistry) -> EffectRack {
    let rack = EffectRack::new(reg, SR as f64);
    for u in [0, 1, 2, 3, BEAT_FX_UNIT] {
        for s in 0..EFFECTS_PER_UNIT {
            if u == BEAT_FX_UNIT && s > 0 {
                continue;
            }
            let g = effect_group(u, s);
            reg.get(&g, "meta").unwrap().set(0.6);
            reg.get(&g, "enabled").unwrap().set_bool(true);
        }
    }
    rack
}

/// Runs `buffers` callbacks of the busy rack over `input`, as the engine
/// does: each deck, then the main mix, frame by frame.
fn run_rack(rack: &mut EffectRack, input: &[f32], buffers: usize, clock: &mut f64) -> f32 {
    let mut acc = 0.0f32;
    for b in 0..buffers {
        rack.prepare([0.5; TARGETS], *clock);
        *clock += BUFFER as f64 / SR as f64;
        let chunk = &input[(b * BUFFER) % (input.len() - BUFFER)..];
        for (f, &x) in chunk[..BUFFER].iter().enumerate() {
            let mut main = [0.0f32; 2];
            for deck in 0..NUM_DECKS {
                let y = rack.process(deck, [x, x], f, BUFFER);
                main[0] += y[0];
                main[1] += y[1];
            }
            let y = rack.process(MAIN, main, f, BUFFER);
            acc += y[0];
        }
        rack.finish(BUFFER);
    }
    acc
}

#[test]
fn the_rack_never_allocates_on_the_audio_thread() {
    let reg = ControlRegistry::new();
    let mut rack = busy_rack(&reg);
    let input = noise(48_000);
    let mut clock = 0.0;
    // The controls the "user" turns, looked up before counting (looking a
    // control up allocates its name; turning it doesn't).
    let slots: Vec<_> = (0..4)
        .flat_map(|u| (0..EFFECTS_PER_UNIT).map(move |s| effect_group(u, s)))
        .map(|g| {
            (
                reg.get(&g, "next_effect").unwrap(),
                reg.get(&g, "enabled").unwrap(),
            )
        })
        .collect();
    let releases: Vec<_> = (0..4)
        .map(|u| reg.get(&effect_unit_group(u), "release").unwrap())
        .collect();
    // Warm up, so every instance has sounded once.
    run_rack(&mut rack, &input, 20, &mut clock);
    // Switching effects, releasing tails and turning slots on and off all
    // happen inside `prepare`, so do plenty of it.
    let before = allocations();
    for round in 0..40 {
        run_rack(&mut rack, &input, 20, &mut clock);
        let (next, enabled) = &slots[round % slots.len()];
        next.set(1.0);
        run_rack(&mut rack, &input, 2, &mut clock);
        next.set(0.0);
        let release = &releases[round % 4];
        release.set(1.0);
        run_rack(&mut rack, &input, 2, &mut clock);
        release.set(0.0);
        enabled.set_bool(true);
    }
    assert_eq!(allocations() - before, 0, "the audio thread allocated");
}

#[test]
fn the_rack_holds_a_few_buffers_not_one_per_instance() {
    let reg = ControlRegistry::new();
    let rack = EffectRack::new(&reg, SR as f64);
    let mb = rack.memory_bytes() as f64 / 1e6;
    // One two-second buffer for each of the 141 instances was about 108 MB.
    assert!(mb < 40.0, "the rack holds {mb:.1} MB");
}

#[test]
fn an_empty_pool_leaves_the_effect_off_until_memory_comes_back() {
    let reg = ControlRegistry::new();
    let mut rack = EffectRack::with_pool(&reg, SR as f64, 1, 0);
    // Echo on deck 1 and deck 2's units: one long buffer between them.
    for u in 0..2 {
        let g = effect_group(u, 1);
        reg.get(&g, "meta").unwrap().set(1.0);
        reg.get(&g, "enabled").unwrap().set_bool(true);
    }
    let click = |rack: &mut EffectRack, deck: usize| {
        rack.prepare([0.5; TARGETS], 0.0);
        let mut out = Vec::new();
        for f in 0..BUFFER {
            let x = if f == 0 { 1.0 } else { 0.0 };
            out.push(rack.process(deck, [x, x], f, BUFFER)[0]);
        }
        rack.finish(BUFFER);
        out
    };
    click(&mut rack, 0);
    let deck1 = rack.instance(0, 1, 0).unwrap().active();
    let deck2 = rack.instance(1, 1, 1).unwrap().active();
    assert!(deck1 != deck2, "exactly one echo got the buffer");
    // Turn the one that sounds off: once it has faded, the other gets it.
    let (on, off) = if deck1 { (0, 1) } else { (1, 0) };
    reg.get(&effect_group(on, 1), "enabled")
        .unwrap()
        .set_bool(false);
    for _ in 0..3 {
        click(&mut rack, off);
    }
    assert!(!rack.instance(on, 1, on).unwrap().active());
    assert!(rack.instance(off, 1, off).unwrap().active());
}

#[test]
fn every_effect_is_silent_on_silence() {
    for kind in KINDS {
        if kind == Kind::Noise {
            continue;
        }
        let mut inst = instance(kind, defaults(kind));
        for i in 0..SR as usize {
            let y = inst.process([0.0; 2]);
            assert!(y == [0.0; 2], "{kind:?} made {y:?} out of silence at {i}");
        }
    }
}

#[test]
fn every_effect_stays_finite_and_bounded_at_its_extremes() {
    let input = noise(2 * SR as usize);
    for kind in KINDS {
        for corner in 0..16u32 {
            // Every parameter at 0 or 1, in all combinations.
            let p = std::array::from_fn(|k| ((corner >> k) & 1) as f32);
            let mut inst = instance(kind, p);
            let mut peak = 0.0f32;
            for &x in &input {
                // Full scale, both channels different.
                let y = inst.process([x * 2.0, -x * 2.0]);
                assert!(
                    y[0].is_finite() && y[1].is_finite(),
                    "{kind:?} {p:?} blew up"
                );
                peak = peak.max(y[0].abs()).max(y[1].abs());
            }
            assert!(peak < 8.0, "{kind:?} {p:?} peaked at {peak}");
        }
    }
}

#[test]
fn feedback_effects_decay_to_true_silence() {
    // With flush-to-zero on, as on the audio thread, a tail decays to
    // exact zeros instead of lingering in denormals that cost CPU.
    crate::dsp::flush_denormals();
    for kind in [
        Kind::Reverb,
        Kind::Echo,
        Kind::Flanger,
        Kind::Phaser,
        Kind::Spiral,
    ] {
        let mut p = defaults(kind);
        // Long tails: high feedback or decay.
        p[1] = p[1].max(0.8);
        let mut inst = instance(kind, p);
        for _ in 0..4_800 {
            inst.process([0.5, -0.5]);
        }
        let mut last = [1.0f32; 2];
        for _ in 0..60 * SR as usize {
            last = inst.process([0.0; 2]);
        }
        for v in last {
            assert!(v == 0.0 || v.is_normal(), "{kind:?} left a denormal: {v:e}");
            assert!(v.abs() < 1e-4, "{kind:?} still rings after a minute: {v}");
        }
    }
}

/// Nanoseconds per frame of `f`, run over `frames` frames (best of three).
fn time(frames: usize, mut f: impl FnMut(usize)) -> f64 {
    (0..3)
        .map(|_| {
            let start = Instant::now();
            for i in 0..frames {
                f(i);
            }
            start.elapsed().as_nanos() as f64 / frames as f64
        })
        .fold(f64::INFINITY, f64::min)
}

#[test]
#[ignore = "benchmark: run with --release -- --ignored --nocapture"]
fn fx_bench() {
    crate::dsp::flush_denormals();
    let frames = 10 * SR as usize;
    let input = noise(frames);
    // A frame at 48 kHz lasts this long; the share of it an effect takes is
    // the share of one CPU core it needs.
    let budget = 1e9 / SR as f64;
    println!();
    println!("DJ Sugar FX benchmark, 48 kHz stereo, ns per frame (share of a core)");
    println!("{:<14}{:>10}{:>10}", "effect", "ns/frame", "% core");
    for kind in KINDS {
        let mut inst = instance(kind, defaults(kind));
        let ns = time(frames, |i| {
            let x = input[i];
            black_box(inst.process(black_box([x, -x])));
        });
        println!(
            "{:<14}{:>10.1}{:>9.2}%",
            AVAILABLE_EFFECTS[kind as usize],
            ns,
            100.0 * ns / budget
        );
    }

    let reg = ControlRegistry::new();
    let mut rack = busy_rack(&reg);
    let mut clock = 0.0;
    let buffers = frames / BUFFER;
    let mut acc = 0.0;
    let ns = time(1, |_| {
        acc += run_rack(&mut rack, &input, buffers, &mut clock)
    }) / (buffers * BUFFER) as f64;
    black_box(acc);
    println!(
        "rack: 4 decks x Reverb+Echo+Gate, Beat FX Echo on 2 decks: {ns:.1} ns/frame ({:.2}% core)",
        100.0 * ns / budget
    );

    let reg = ControlRegistry::new();
    let mut idle = EffectRack::new(&reg, SR as f64);
    let ns = time(1, |_| {
        acc += run_rack(&mut idle, &input, buffers, &mut clock)
    }) / (buffers * BUFFER) as f64;
    println!(
        "rack, no effects on: {ns:.1} ns/frame ({:.2}% core)",
        100.0 * ns / budget
    );

    let mut color = ColorFx::new(SR as f64);
    for (preset, name) in COLOR_FX.iter().enumerate() {
        color.prepare(preset as f64 + 1.0, 0.8, 0.5);
        let ns = time(frames, |i| {
            let x = input[i];
            black_box(color.process(black_box([x, -x])));
        });
        println!(
            "color fx {name:<10}{ns:>8.1} ns/frame ({:.2}% core)",
            100.0 * ns / budget
        );
    }
    println!("rack memory: {:.1} MB", idle.memory_bytes() as f64 / 1e6);
}

/// The largest step between neighbouring output samples over `frames`.
fn largest_step(inst: &mut Instance, from: usize, frames: usize) -> f32 {
    let mut last = inst.process([tone(from); 2])[0];
    let mut step = 0.0f32;
    for i in from + 1..from + frames {
        let y = inst.process([tone(i); 2])[0];
        step = step.max((y - last).abs());
        last = y;
    }
    step
}

fn tone(i: usize) -> f32 {
    (std::f32::consts::TAU * 220.0 * i as f32 / SR).sin() * 0.5
}

#[test]
fn turning_any_knob_all_the_way_does_not_click() {
    let ctx = Ctx {
        sample_rate: SR,
        beat_seconds: 0.5,
        beat: None,
    };
    let mut clicks = Vec::new();
    // Effects whose knobs are continuous. Gate, Roll, Brake and Bitcrusher
    // make edges or steps on purpose.
    for kind in [
        Kind::Echo,
        Kind::Reverb,
        Kind::Filter,
        Kind::Flanger,
        Kind::Phaser,
        Kind::Noise,
        Kind::Spiral,
        Kind::Drive,
        Kind::Chorus,
        Kind::Ladder,
        Kind::Hall,
        Kind::FilterEcho,
    ] {
        for k in 0..MAX_PARAMETERS {
            // Leave the beat lengths alone: those are switches, and their
            // effects glide (echo time) or change at the next cycle.
            if time_parameter(kind).is_some_and(|(i, ..)| i == k) {
                continue;
            }
            let mut p = defaults(kind);
            if kind == Kind::Ladder && k == 0 {
                // A resonant sweep peaks as it passes the tone, on purpose;
                // without resonance it must sweep cleanly.
                p[1] = 0.0;
            }
            p[k] = 0.0;
            let mut inst = instance(kind, p);
            let settle = 2 * SR as usize;
            let before = largest_step(&mut inst, 0, settle);
            p[k] = 1.0;
            inst.prepare(&p, ctx);
            let during = largest_step(&mut inst, settle, 2_400);
            let after = largest_step(&mut inst, settle + 2_400, settle);
            let bound = 1.5 * before.max(after) + 1e-3;
            if during > bound {
                clicks.push(format!(
                    "{kind:?} parameter {k} jumped by {during} (steady {before} / {after})"
                ));
            }
        }
    }
    assert!(clicks.is_empty(), "{clicks:#?}");
}

#[test]
fn swept_delay_reads_keep_the_highs() {
    // A 10 kHz tone through a delay line read at a sweeping delay: its level
    // stays within 1 dB wherever the read falls between samples.
    let len = 4_096;
    let mut buf = vec![0.0f32; len];
    let mut write = 0;
    let hz = 10_000.0;
    let (mut lo, mut hi) = (f32::INFINITY, 0.0f32);
    let window = 480;
    let mut acc = 0.0f32;
    let mut count = 0;
    for i in 0..48_000 {
        buf[write] = (std::f32::consts::TAU * hz * i as f32 / SR).sin();
        write = (write + 1) % len;
        // The fractional part sweeps slowly, as a flanger's does.
        let delay = 200.0 + 0.5 + 0.5 * (i as f32 / 4_000.0).sin();
        let y = tap(&buf, write, delay);
        if i >= 1_000 {
            acc += y * y;
            count += 1;
            if count == window {
                let level = (acc / window as f32 * 2.0).sqrt();
                lo = lo.min(level);
                hi = hi.max(level);
                acc = 0.0;
                count = 0;
            }
        }
    }
    let db = |v: f32| 20.0 * v.log10();
    assert!(db(lo) > -1.0, "dips to {:.2} dB", db(lo));
    assert!(db(hi) < 0.5, "peaks at {:.2} dB", db(hi));
}

/// Runs `kind` with parameters `p` over `frames` frames of `input`, in
/// 256-frame buffers, with the deck's beat grid starting at beat `start`
/// (two beats a second).
fn run_on_grid(
    kind: Kind,
    p: [f32; MAX_PARAMETERS],
    start: f64,
    frames: usize,
    input: impl Fn(usize) -> f32,
) -> Vec<f32> {
    let mut inst = Instance::with_memory(SR);
    inst.kind = Some(kind);
    inst.reset();
    let mut out = Vec::with_capacity(frames);
    for b in 0..frames.div_ceil(BUFFER) {
        let first = b * BUFFER;
        inst.prepare(
            &p,
            Ctx {
                sample_rate: SR,
                beat_seconds: 0.5,
                beat: Some(start + first as f64 / (0.5 * SR as f64)),
            },
        );
        for i in first..(first + BUFFER).min(frames) {
            let x = input(i);
            out.push(inst.process([x, x])[0]);
        }
    }
    out
}

#[test]
fn gates_open_on_the_beat_grid() {
    // A quarter-beat gate (6 000 frames at 120 BPM) turned on a fifth of
    // the way into a quarter beat: it reopens on the grid's quarter beats,
    // and still does ten seconds later.
    let p = defaults(Kind::Gate);
    let out = run_on_grid(Kind::Gate, p, 10.3, 10 * SR as usize, |_| 0.5);
    let quarter = 6_000;
    // Frame 0 is beat 10.3: the next quarter beat (10.5) is 4 800 frames in.
    let opens: Vec<usize> = (1..out.len())
        .filter(|&i| out[i - 1] < 0.3 && out[i] >= 0.3)
        .collect();
    assert!(opens.len() > 70, "{} openings", opens.len());
    for &i in &opens {
        let off = (i + quarter - 4_800) % quarter;
        let off = off.min(quarter - off);
        // The edge's 2 ms slew crosses half way about 50 frames in.
        assert!(off < 80, "opened {off} frames off the grid at frame {i}");
    }
}

#[test]
fn flanger_and_phaser_sweeps_follow_the_grid() {
    // Started at different points, two flangers on the same grid sweep
    // together once locked.
    for kind in [Kind::Flanger, Kind::Phaser] {
        let p = defaults(kind);
        let input = |i: usize| ((i * 7919) % 1000) as f32 / 1000.0 - 0.5;
        let a = run_on_grid(kind, p, 4.0, SR as usize, input);
        // The same music, but the effect turned on half a second later.
        let half = SR as usize / 2;
        let b = run_on_grid(kind, p, 5.0, half, |i| input(i + half));
        let diff: f32 = a[half + 4_800..]
            .iter()
            .zip(&b[4_800..])
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f32::max);
        assert!(diff < 0.05, "{kind:?} sweeps differ by {diff}");
    }
}

#[test]
fn rolls_start_on_the_grid() {
    // A one-beat roll turned on at beat 10.1 waits for 10.25 (3 600 frames)
    // and repeats from there: a click starting at that frame comes back a
    // beat later (past the 3 ms fade at each seam).
    let mut p = defaults(Kind::Roll);
    p[0] = 0.0;
    let out = run_on_grid(Kind::Roll, p, 10.1, 3 * SR as usize, |i| {
        if (3_600..3_900).contains(&i) {
            0.8
        } else {
            0.0
        }
    });
    let beat = SR as usize / 2;
    assert!(out[3_800] > 0.7, "plays live until the grid");
    assert!(out[3_800 + beat] > 0.7, "repeats a beat later");
    assert!(out[3_800 + 2 * beat] > 0.7, "and again");
}

#[test]
fn no_effect_allocates_when_reset_or_run() {
    let mut inst = Instance::with_memory(SR);
    let ctx = Ctx {
        sample_rate: SR,
        beat_seconds: 0.5,
        beat: Some(3.3),
    };
    let before = allocations();
    for kind in KINDS {
        inst.kind = Some(kind);
        inst.reset();
        inst.prepare(&defaults(kind), ctx);
        for i in 0..1_000 {
            inst.process([tone(i); 2]);
        }
    }
    assert_eq!(allocations() - before, 0, "an effect allocated");
}

#[test]
fn the_flanger_keeps_the_level_when_switched_on() {
    // Mixing in the swept copy used to drop broadband music by up to 3 dB.
    let white = noise(SR as usize * 4);
    let mut acc = 0.0f32;
    let music: Vec<f32> = white
        .iter()
        .enumerate()
        .map(|(i, &w)| {
            acc = acc * 0.98 + w * 0.2;
            acc + 0.3 * (std::f32::consts::TAU * 55.0 * i as f32 / SR).sin()
        })
        .collect();
    let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
    for src in [&white, &music] {
        for (depth, feedback) in [(0.5, 0.5), (1.0, 0.0), (1.0, 0.5)] {
            let mut p = defaults(Kind::Flanger);
            p[1] = depth;
            p[3] = feedback;
            let mut inst = instance(Kind::Flanger, p);
            let out: Vec<f32> = src.iter().map(|&x| inst.process([x, x])[0]).collect();
            let db = 20.0 * (rms(&out[4800..]) / rms(&src[4800..])).log10();
            assert!(
                db.abs() < 1.0,
                "depth {depth} feedback {feedback}: {db:+.2} dB"
            );
        }
    }
}

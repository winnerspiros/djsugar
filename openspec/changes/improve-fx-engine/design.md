# Design

## Context

DJ Sugar's effects are its own Rust code (`crates/engine/src/effects.rs` and `dsp.rs`). The FX library review (`notes/fx-engine-review.md`, 2026-10-27) decided to keep that engine and port MIT-licensed algorithms into it by hand, rather than depend on fundsp, bosai-dsp, Airwindows or Signalsmith DSP. The goal for v0.5 (2026-11-01): high sound quality and low CPU, on four decks at once.

The engine runs everything frame by frame inside the audio callback: each deck's sample goes through EQ, Color FX, Sculpt and the effect rack, then into the main mix, whose effects run before the limiter. Effects read their controls once per buffer in `EffectRack::prepare` and run per frame in `EffectRack::process`.

## Goals

- Every effect sounds clean: no clicks on any knob or switch, no zipper noise, no dulled highs in swept delays, no aliasing where it isn't the point of the effect.
- Every effect is cheap: a busy set (four decks with three effects each, plus Beat FX) costs a small share of one core, measured by a benchmark that ships with the code.
- Nothing allocates, locks or waits on the audio thread, checked by a test.
- Time-based effects lock to the beat, not just to the tempo.
- The same engine serves deck FX, Beat FX, pad FX, the mic's unit and Color FX.

## Non-goals

- Replacing the engine with a library, or adding a DSP dependency.
- Third-party plugin hosting (`add-audio-plugin-hosting`, v2.0).
- Per-stem effects (`add-stem-fx-and-acapella`).

## Decisions

### Measure first

A harness in `crates/engine/src/effects/bench.rs` runs with the engine's tests:

- `fx_bench` (ignored by default; run with `cargo test --release -p djsugar-engine fx_bench -- --ignored --nocapture`) prints ns per frame and the share of a core for each effect, each Color FX, an idle rack and a busy rack, plus the rack's memory.
- A counting global allocator (test builds only) checks that `prepare` and `process` never allocate, while effects are switched, released and turned on and off.
- Quality checks: silence in gives silence out, every effect stays finite and bounded with every parameter at 0 or 1 on full-scale noise, and feedback tails decay to exact zeros with flush-to-zero on.

Every later PR reports the benchmark before and after.

### Delay memory from a pool

Each (unit, slot, target) instance used to own one buffer sized for a two-second stereo echo, about 108 MB for 141 instances. Instances now own no delay memory. The rack keeps a pool of 32 long buffers (two seconds of stereo, for Echo, Roll, Brake, Echo Out and Spiral) and 32 short ones (the reverb's tank, also enough for the flanger), allocated once at start-up. An instance borrows a buffer when it starts sounding and gives it back once it is silent (faded out and any release tail finished). Borrowing and returning only move a pointer between the instance and a pre-sized stack, so nothing allocates. If the pool were ever empty, the effect stays off and tries again next buffer; 32 is well above the most effects a real set sounds at once (about 20 with every deck's FX on, the Beat FX on two decks and pad FX held). Each deck's Color FX keeps its own buffer, since it must be ready the moment the knob moves.

### Smooth everything per sample

Every continuous parameter glides per sample towards the value read once per buffer (one-pole, about 5 ms, matching the filters' cutoff glide), including reverb decay, damping and send, flanger depth, centre and feedback, phaser depth and feedback, gate depth and width, and echo send and ping-pong. Switches that can't glide (phaser stages, bit depth steps) crossfade or change at a zero crossing.

### Better interpolation

Swept delay reads move from linear to 4-point cubic (Catmull-Rom) interpolation, after Signalsmith DSP's interpolators: Echo while its time glides, Flanger, Spiral, Echo Out and Brake. Linear interpolation halfway between samples loses about 2 dB at 10 kHz; cubic keeps it within about half a dB, for a few multiply-adds more per read. The reverb's modulated allpasses stay linear: they swing only a few samples for a slow chorus inside a damped tank, where cubic measured a third more CPU for no audible change. Precomputing the reverb's output tap positions (they were rescaled every sample) more than pays for the smoothing added in the same PR.

### Locked to the beat

`Ctx` gains the target's beat position at the start of the buffer (`EffectRack::set_beats`, from `Track::beat_at`). LFOs and gates (Flanger, Phaser, Gate) take their phase from the deck's position in the beat grid, so a gate opens on the beat and two decks' flangers sweep together. A phase error over 5% of a cycle (a seek, a loop, a newly started effect) snaps; smaller drift is pulled in over about 1,000 frames by nudging the per-frame step, so the sweep never jumps. Roll starts recording on the next division of the grid (its slice, or a quarter beat for longer slices), so the repeat lands on the beat. The main mix follows the deck that leads its tempo (the first playing deck). Stopped decks, the inputs, tap tempo and units with their own `bpm` keep a free-running phase.

### Block processing

Once the above lands, the rack processes each active instance over a block of frames instead of dispatching per frame: the engine renders each deck's chain into a per-deck scratch buffer (sized at start-up for the largest callback), runs the rack over it, then mixes. Parameters still glide per sample inside the block. The benchmark decides whether this is worth its engine restructure: if the busy rack already costs only a few percent of a core, block processing waits.

### New effects

Written for DJ Sugar after published, MIT-licensed designs, each file header naming its source and `NOTICE` listing them (no code copied), in `crates/engine/src/effects/extra.rs`:

- **Drive:** a smooth x/sqrt(1+x²) saturator with a little bias for even harmonics, 4x oversampled through two Kaiser half-band stages and with first-order antiderivative anti-aliasing, then a DC blocker, a tone low-pass and level matching. Drive tops out at 24 dB: beyond that a sine becomes a square whose top harmonics alias whatever is done, and the test (5 kHz at full drive, audible aliasing 60 dB down) fails.
- **Chorus:** three voices swung around 14 ms, a third of a cycle apart, spread in stereo, sweep in beats and locked to the grid.
- **Ladder:** a zero-delay-feedback 4-pole Moog-style low-pass with a saturating input and resonance short of self-oscillation; its cutoff sweep is capped at about 1/120 octave per sample so the resonance can't ring like a click.
- **Hall:** an eight-line feedback delay network mixed by a Hadamard matrix, with four input diffusers, two modulated lines, damping, pre-delay up to 120 ms and decay from 1 to 12 s; level matched to the plate. It fits in a long pool buffer.
- **Rev Roll:** Roll played backwards. (Our Roll already slips: the deck plays on underneath, so there is no separate Slip Roll. Helix is left for later.)
- **Filter Echo:** Echo with a low-pass or high-pass inside its feedback loop, as Pioneer's LPF/HPF Echo; the meta knob opens the send and sweeps the high-pass up together.

New effects join the end of `AVAILABLE_EFFECTS`, so saved `loaded_effect` values and mappings keep their meaning. The pad FX pages are full (eight pads each) and stay as they are.

## Risks

- **A pooled effect could find no memory.** It stays silent rather than glitching; the pool is sized far above real use, and a test covers the empty case.
- **Beat-locked phases jump when the user seeks or loops.** Phases follow the grid, so a jump moves the LFO with it, which is what a DJ expects; gates already slew their edges, so a jump doesn't click.
- **Block processing changes the engine's main loop.** It comes last, behind the benchmark, in its own PR.

## PR plan

1. Benchmark and quality harness, delay memory pool, this design (milestone v0.5).
2. Smooth parameters and cubic interpolation.
3. Beat-locked LFOs, gates and rolls.
4. Block processing, if the benchmark says so.
5. New effects, menus, pad FX pages, notices and the user guide.

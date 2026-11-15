# Proposal

Planned for: **v0.5 · Stems and performance**

## Why

DJ Sugar's effects should be high quality and cheap enough to run on four decks at once. A review of fundsp, bosai-dsp, Airwindows and Signalsmith (`/mnt/project-files/notes/fx-engine-review.md`, 2026-10-27) found that none of them should replace DJ Sugar's own engine. Each one is missing what DJ effects need: beat sync, release tails, click-free switching and Mixxx's controls. The biggest gains are in our own engine:
- The rack reserves about 100 MB of delay memory up front. This is estimated, not measured.
- Effects run one sample at a time.
- Modulated delays use linear interpolation.
- There is no oversampling.
- There is no benchmark.

## What Changes

- An FX benchmark and offline quality checks covering CPU per frame, aliasing, noise floor and denormals.
- Rack memory sized per effect, so only the long delays get 2 s buffers.
- Block processing: each active effect runs over a chunk of frames in a form the compiler can vectorise, with parameters still smoothed per sample.
- Cubic or Lagrange interpolation in the modulated delays (echo, flanger, spiral).
- Real-time safety checked by a test, per-sample smoothing of every parameter, and gates, LFOs and rolls locked to the beat grid.
- New effects, written after MIT-licensed designs with attribution:
  - Drive (oversampled)
  - Chorus
  - Ladder filter
  - Hall reverb
  - Rev Roll (Helix later)
  - Filter Echo
- No new DSP dependency. Signalsmith Stretch stays for key lock.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `effects`: performance budget, new effects

## Out of Scope

- Third-party plugin hosting (`add-audio-plugin-hosting`, v2.0).
- Code under GPL-3.0-only, because DJ Sugar is GPL-2.0-or-later.

## Impact

`crates/engine/src/effects.rs`, `dsp.rs`, a new FX bench, effect menus, pad FX pages, `THIRD_PARTY_NOTICES`.

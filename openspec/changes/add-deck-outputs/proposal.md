# Proposal

Planned for: **v0.6 · Buy it from the artist** (moved from v0.5 at the 0.5.0 release) (2026-11-02: "I've used this to send separate decks to Ableton in the past using 4 channels; it was nice to record decks separately and apply FX in Ableton separately")

## Why

Two kinds of DJ need each deck on its own outputs:

1. **Into a DAW.** Sending each deck to its own channels (on a multi-output interface, or a virtual device such as BlackHole 16ch or Loopback) lets Ableton Live, Bitwig or Logic record every deck on its own track and put its own effects on each one, live or afterwards. DJs have worked this way before with two decks on four channels.
2. **Into a club mixer.** With an external mixer (Pioneer DJM, Allen & Heath Xone, Rane), the hardware does the EQ, faders, crossfader and headphone cue; the software only plays the decks, each into its own mixer channel. Serato, Traktor, rekordbox and VirtualDJ all have this "external mixer" mode; it is also how DVS rigs are wired, so add-dvs builds on it.

DJ Sugar renders only master, headphones and booth today (`MIX_CHANNELS = 6` in `crates/engine/src/output.rs`). Research: notes/ecosystem-standards-review.md in the project files.

## What Changes

- **Deck outputs** in Settings › Audio › Outputs: Deck 1 to Deck 4 can each go to a channel pair of a device, next to Master, Headphones and Booth. The sampler and the Groove Box can have their own pair too.
- **What each deck output carries** (one choice for all decks):
  - **Clean**: the deck after tempo, key lock and stems, before EQ, filter, deck FX and fader. For adding your own effects in the DAW.
  - **Pre-fader** (default): after EQ, filter and deck FX, before the channel fader and crossfader.
  - **Post-fader**: what the deck adds to the master mix, after fader and crossfader. The deck outputs then add up to the master mix before master FX, so the DAW can rebuild the mix exactly.
- In this mode DJ Sugar still mixes as usual: master, headphones, recording, Automix and Link all keep working. This is the "send decks to Ableton" mode.
- **External mixer mode**, a switch under Settings › Audio › Outputs ("Mix on: DJ Sugar / an external mixer"):
  - decks go out pre-fader with no EQ, filter or fader applied (the mixer does those); deck FX, pads, loops, stems and sync stay;
  - DJ Sugar's on-screen mixer (EQ, faders, crossfader, master section, headphone cue buttons) is replaced by slim deck strips with level meters, in 2-Deck and 4-Deck layouts;
  - master, headphones and booth outputs become optional; Automix and the master limiter are off, with a line saying why;
  - controller mappings keep working; mixer controls they send are ignored (most external-mixer setups pair a deck-only controller anyway).
- Setup help: a "Send decks to a DAW" guide page with Ableton, BlackHole/Loopback (macOS), VB-Cable/ASIO (Windows) and PipeWire/JACK (Linux) recipes, and an "External mixer" page.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `audio-io`: deck, sampler and Groove Box outputs, with a choice of tap point
- `mixer`: external mixer mode

## Out of Scope

- Each stem of a deck on its own outputs (a natural follow-up: same mechanism, more channels).
- Recording each deck to its own file inside DJ Sugar (that is add-ableton-set-export layer 1, which will reuse this change's deck taps).
- Hardware mixers' own DVS protocols and timecode (add-dvs).
- Mono deck outputs and per-deck sample rates.

## Difficulty

**Medium overall, about 1 to 1.5 weeks in two PRs.**

1. **Deck outputs with tap points ("send decks to Ableton"): small to medium, 2 to 3 days.** The engine already computes each deck's signal separately before summing (`deck_buf` in `Engine::process`); this adds three tap points, a wider render buffer, and more route rows. Routes on other devices reuse the existing drift-correcting ring buffers.
2. **External mixer mode: medium, 3 to 4 days.** Mostly UI and rules: slim deck strips instead of the mixer in both layouts, which controls stop applying, Automix/limiter/headphone handling, sampler and mic placement, and guide pages.

## Performance

Rule: none of this may cost performance. Design (details in design.md):

- **Zero cost when unused.** No deck route → the render buffer stays six channels and the frame loop is unchanged; the "are deck taps on" check happens once per buffer, not per frame.
- **When used**, each deck tap copies two floats per frame into channels the engine already has in cache: a few hundred extra float copies per 256-frame buffer, well under 1% of what a playing deck already costs (resampling, key lock, EQ, FX). External mixer mode does *less* work than now (no EQ, crossfader, master FX or limiter).
- **No allocation on the audio thread.** The render buffer is sized for the widest layout (6 + 2 × 6 pairs) when the outputs are configured.
- **No added latency.** Deck outputs on the master's device come out of the same callback, sample-aligned with each other and with master. Deck outputs on a second device are drift-corrected like headphones are today; the settings advise keeping all deck outputs on one device (or a macOS aggregate device) so decks stay sample-aligned for a DAW.

## Impact

engine (`output.rs` roles and wider render buffer, tap points in `Engine::process`, external-mode bypass), app (Settings › Audio › Outputs rows and mode switch, slim deck strips in 2-Deck and 4-Deck layouts), controls `[Master],external_mixer`, `[Master],deck_tap`, supported-controls.txt, user guide (Send decks to a DAW, External mixer).

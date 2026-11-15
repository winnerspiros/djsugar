# Proposal

Planned for: **v0.6 · Buy it from the artist** (issue #420, 2026-11-06)

The tasks still open moved to **v0.8 · From the booth to the studio** on 2026-11-12, so v0.7 ships with what is built.

## Why

The Groove Box is played and edited only on screen. Pad controllers (Maschine, Launchpad, Akai pads, Circuit Tracks) are made for this: enter steps, play the sounds live and record them, load a named groove and change it on the fly. Each controller differs only in how it reports presses and takes colours, so the behaviour belongs to DJ Sugar and each mapping stays thin. Research: notes/maschine-mikro-mk3-review.md in the project files.

## What Changes

- **Pad surface in the engine** (`crates/engine/src/groove_pads.rs`), per Groove Box: `[GrooveN],pad1`..`pad16` (pressure 0-1 in), `pad1_color`..`pad16_color` (a small palette out), `pad_mode` (Steps, Play, Grooves), `row_selected`, `record`, `row_clear`.
  - Steps: the selected row's 16 steps, lit with the playhead.
  - Play: pads 1-8 (and again 9-16) play rows 1-8, velocity-sensitive; with `record` the hit goes into the nearest step of the groove playing and isn't heard twice.
  - Grooves: pads 1-4 grooves A-D (on the next bar), 5-8 kits, 9 on the named grooves.
- **Native Instruments Maschine Mikro MK3** HID mapping (`17cc:1700`): pads, mode/transport/row buttons, MUTE/SOLO, encoder and touch strip for level, pitch, decay, swing and tempo, all lights, and its 128 x 32 screen (deck, kit, groove, tempo, row, mode). No NI software or MIDI mode.

## Capabilities

### New Capabilities

- `groove-pads`: drive the Groove Box from a pad controller

### Modified Capabilities

None.

## Out of Scope (later steps)

- Launchpad (Programmer mode) and Akai pad profiles, the Steps pad page on DJ controllers' pads, Groove controls in the mapper's function list.
- 8 rows, accents from pressure and FILL: with add-groove-box-2.

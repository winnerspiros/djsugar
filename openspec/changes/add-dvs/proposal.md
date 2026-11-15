# Proposal

Planned for: **v0.8 · From the booth to the studio**

## Why

Serato, Traktor, rekordbox and VirtualDJ all play digital files from timecode vinyl; Mixxx does it for free.

## What Changes

- Control a deck from Serato, Traktor, rekordbox, MixVibes or djay timecode vinyl or CDs through an audio interface's inputs.
- Decode with a Rust port of the xwax timecoder (GPL-3.0-only crate `djsugar-timecode`) inside the audio callback, and follow the record's position, not only its speed.
- Mixxx `vinylcontrol_*` controls, so DVS mixer mappings work.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `deck-transport`: decks follow timecode vinyl

## Out of Scope

- Hardware mixers' own DVS protocols.

## Impact

engine inputs and deck control; new `djsugar-timecode` crate (GPL-3.0-only, from xwax); Settings › Audio; deck header.

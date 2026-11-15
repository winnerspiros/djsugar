# Proposal

Planned for: **v0.4 · Your collection, cleaned up (in progress: djsugar PR #57)**

## Why

Key lock existed in the engine with no button, and harmonic matching only knew one rule. DJs who mix by key want the looser fuzzy keymixing rules, near-miss hints, and decks that shift into key on load.

## What Changes

- A key lock button above each deck's pitch reset triangle.
- Settings › Analysis keymixing mode: Harmonic (4 compatible keys) or Fuzzy (6 keys, letter ignored, number ±1).
- The browser compares against the key the reference deck actually plays, and marks near matches one semitone away (+1/−1, dimmer lilac) and two (±2, gray).
- Key match on load: Off, ±1 or ±2 semitones (default ±1).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `tempo-and-key`: key lock gets an on-screen button; key matching on load
- `library-search`: Harmonic or Fuzzy keymixing modes and near-miss marks

## Out of Scope

- Energy-boost key moves (+2 / +7).

## Impact

crates/app (deck UI, browser key colouring, settings), crates/library query, engine `keylock` unchanged.

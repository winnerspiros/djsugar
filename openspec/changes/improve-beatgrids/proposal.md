# Proposal

Planned for: **v0.5 · Stems, performance and switching over** (moved from v1.0, 2026-10-29)

## Why

Grids drive sync, loops, quantize and FX. DJ Sugar already has dynamic grids (tempo sections, `crates/engine/src/beats.rs`) and the Mixxx grid controls, but the detected dynamic grids are not good enough. djay Pro is widely said to have the best grids, so we research the options first, then rebuild detection, and make fixing a grid quick on the waveform.

## What Changes

- **Research first** (a notes document before code): how djay Pro, rekordbox, Serato, Traktor and VirtualDJ detect and show grids (constant vs dynamic, how drift is handled, what editing they offer), and which open models and libraries we could use (for example beat_this, BeatNet, madmom, essentia, aubio), with licence, accuracy on a labelled benchmark, speed and model size. Ends with a recommendation.
- Better beat and downbeat detection from that research: bar 1 found reliably, constant grids for steady electronic music, dynamic sections only where the tempo really moves (live drums, edits, old disco); an "unsure" warning when a grid drifts.
- **Saving grids**: an edited grid is saved for the track, can be locked so analysis and imports never overwrite it, shows where it came from (analysis, edited, imported), and can go back to the analysed one. Research whether to also write grids to the file (as Serato and Traktor do) so they survive moving the file.
- **Manual editing in DJ Sugar**: a grid edit mode on the zoomed waveform: drag a beat marker, set the downbeat with one click, tap along to set the tempo, half/double, add or remove a tempo change, undo.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `beatgrids`: grid edit mode, saved and locked grids, unsure warning
- `track-analysis`: better beat and downbeat detection

## Out of Scope

- Grid editing on controllers beyond the existing Mixxx grid controls.

## Impact

notes/beatgrid-research.md, engine analysis and beats, library grid columns, waveform interaction.

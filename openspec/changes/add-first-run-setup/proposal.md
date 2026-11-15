# Proposal

Planned for: **v0.5 · Stems, performance and switching over** (moved from v1.0, 2026-10-29)

## Why

Settings are found by exploring, and a DJ switching over should be playing within minutes. A user request (2026-10-29) asks for a guided tour the first time DJ Sugar opens.

## What Changes

- A first-run guided tour, each step skippable, that can be run again from the Help menu:
  1. **Audio setup**: output device, master and headphones outputs (or the controller's sound card), with a test tone.
  2. **Controller setup**, only if one is connected: shows that it was recognised and which mapping runs, with a "press a button" check; offers Quick map when there's no mapping.
  3. **Library**: pick the music folder(s), and offer to import from VirtualDJ, rekordbox or Mixxx when they are found.
  4. **Simple playback and mixing**: a short hands-on walk through the real window with a highlight and a short tip at each step: load a track, play and cue, headphones cue, sync the second track, bring it in with the volume fader or crossfader and the EQs.
- Uses the same in-app tour engine as the advanced tutorials (`add-tutorials`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `app-layout`: a first-run guided tour

## Out of Scope

- Video tutorials.

## Impact

app UI: a tour overlay (highlight, tip, next/skip) shared with `add-tutorials`; audio, controller and import settings reused as steps.

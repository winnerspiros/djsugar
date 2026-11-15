# Proposal

Planned for: **v0.8 · From the booth to the studio**

## Why

Turning a set into an Ableton Live set lets a DJ edit the mix, make a radio show or a release from it, or rework a set list as a studio arrangement. No other DJ app exports to Live (DJ.Studio, a mix editor, does), and the recorder already logs every control change for this.

## What Changes

- Step 0 (done early): the recorder's control log also holds each loaded deck's place in its track, once a second and whenever it jumps (hot cue, waveform seek, loop going round), and the loop, cue point and hot cue positions. Before this, seeks on the waveform were not in the log, so a set could not be rebuilt past one.
- Step 1: a History set, playlist or crate exported as a Live set (File › Export › Ableton Live Set…): tracks alternate on two audio tracks as warped clips from their beatgrids, overlapping by a set number of bars, with a tempo envelope and a locator per track. Files are referenced where they are.
- Step 2: a recording exported as a Live set: the mix on one audio track, a locator per track from the cue sheet, and the tempo at each track change.
- Step 3: a recording rebuilt in full from its control log: one audio track per deck, a clip per stretch of playback cut at jumps, loops as clip loops, warp markers from beatgrid and pitch (Complex Pro with key lock, Re-Pitch without), automation for channel volume, crossfader (Live's A/B assignment), EQ (EQ Three) and filter (Auto Filter), and the mix on a muted reference track.

The writer produces the Live 11.0 schema (gzipped XML), which Live 11 and every Live 12 release open; Live refuses sets saved by a newer version than itself. The version string and default device XML are copied from a set saved by Live, not computed.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `mix-recording`: deck places and cue positions in the control log; export of recordings and History sets to Live

## Out of Scope

- DJ Sugar effects and plugins recreated as Live devices beyond EQ Three and Auto Filter.
- Per-deck pre-fader audio in recordings (belongs with add-deck-outputs; the rebuilt set uses the original files).
- Reading .als files back. DAWproject (not read by Live) could later reuse the same timeline.

## Impact

engine recorder (step 0), a new .als writer (quick-xml + flate2), File › Export menu entry and History/list right-click entry.

# Proposal

Planned for: **v0.8 · From the booth to the studio**

## Why

The Mixed Set Editor (add-mixed-set-editor, which comes first in v0.8) shows a recorded set as a timeline but changes nothing. DJs want to fix a set after playing it: move a transition, smooth a fader, take out an FX stab, and get a new mix file. Mix editors such as DJ.Studio do this for mixes built from playlists, with no live mode; DJ Sugar can do it for sets played live because it logs every control change beside the recording and can play that log back through its own engine.

## What Changes

- Capture: control changes stamped with the audio frame they took effect on (not the logger's 5 ms poll), plugin parameter changes and instrument / Groove Box notes in the log, an option to record Mic and Aux inputs to their own files, and the deck's exact place in its track every audio block while it is scratched (jog in scratch mode, backspin, brake).
- Editing in the Mixed Set Editor: drag a transition earlier or later (snapping to beats and phrases), stretch it, pick a style, open a transition editor with both tracks on one beatgrid and editable curves (VirtualDJ's Automix editor layout), change a loop's length or repeat count, A/B against the recording, undo.
- Hard moves (scratches, backspins, brakes and anything else the render cannot rebuild from mixer controls) stay one block with two choices, not editable stroke by stroke: **Keep from recording** (the default: that stretch of audio is copied from the recording into the render) or **Bypass** (the deck plays straight through as if the move never happened). DJs fine-tune fader, EQ, filter and FX lines; they keep or drop a scratch whole.
- Render: play the edited set offline through DJ Sugar's engine to a new file beside the recording; the recording is never changed. Mixcloud export and tracklist from the new file.
- The Ableton export (add-ableton-set-export step 3) uses the same timeline, with kept hard moves as rendered audio clips.

Mockups (board 2, and the scratch block on board 1): https://claude.ai/artifact/76fLT6FF4ULjWNcXRSQCzv

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `mix-recording`: frame-accurate log, render an edited set

## Out of Scope

- Building a mix from a playlist without playing it (DJ.Studio's main use); it can reuse the render later.
- Video export, stems placed on the timeline, re-ordering a whole set.
- Recreating third-party plugin state that was not logged.

## Impact

engine recorder (frame stamps, plugin and note lines, input files, place lines during hard moves), editing in the Mixed Set Editor, an offline render path that drives the engine from a log.

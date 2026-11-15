# Design

The timeline model, the reader of the control log and the read-only Mixed Set Editor come from add-mixed-set-editor (earlier in v0.8). This change adds editing, the render and the capture it needs.

## Render

The engine already runs from controls. An offline render creates an engine with no audio device, loads the set's tracks, replays the (edited) event list at each event's frame, and pulls buffers as fast as it can into a WAV or FLAC writer. Two checks decide whether a render is trustworthy:

1. **Null test**: render an unedited log and subtract it from the recording. Target: residual below -60 dBFS outside plugin effects. Needs frame-accurate stamps (Capture) and deterministic DSP (no random seeds per run, key lock with fixed block sizes).
2. **Missing sources**: a track file that moved or a plugin that is not installed makes the render stop before it starts and says which.

Parts the DJ did not edit could later be copied from the recording instead of rendered; not needed for a first version.

## Hard moves

A hard move is a stretch where a deck's place in its track does not follow play, rate and jumps: the jog in scratch mode (`scratch2_enable`), a backspin or a brake. The recorder (step 2) logs the deck's place every audio block while one lasts. The timeline shows it as one block on the clip, from the first to the last block that did not follow normal play, padded to the nearest beat on each side.

- **Keep from recording** (default): the render copies that stretch of the recording's audio, with a 10 ms crossfade at each edge, in place of rendering it. The copy holds the whole mix at that moment, so if the DJ edited another deck's lanes or a transition overlapping the block, the render plays the logged positions back through the engine instead (exact for DJ Sugar's own processing) and says so on the block.
- **Bypass**: the deck plays straight through at its rate from where it was when the move began. The rest of the clip follows from there, so later cue jumps and loops keep their places in the track; the Mixed Set Editor shows the clip shifted where the place differs.

The Ableton export treats a kept hard move the same way: a rendered audio clip on the deck's track in place of warped clips.

## Frame-accurate log

Control changes made from the UI or a controller are applied by the audio thread at a buffer boundary. The audio thread writes (frame, control id, value) into a lock-free ring the logger drains, in place of the logger's 5 ms poll. Plugin parameter changes and instrument notes go through the same ring, and so do the dense place lines during a hard move. Mic and Aux files are written beside the recording with the same frame base.

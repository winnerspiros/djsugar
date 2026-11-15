# Proposal

Planned for: **v0.4 · Your collection, cleaned up** (pulled forward, 2026-10-27; the editing half stays in `add-controller-mapper`)

## Why

A controller can expose hundreds of functions across SHIFT, pad modes and banks, and DJ Sugar runs Mixxx's mappings without showing what they do. Most of each mapping is JavaScript: 214 of the DDJ-FLX4's 319 MIDI rows and every row of the Xone K2 and Midi Fighter Twister are script bindings, so a table of MIDI rows (Mixxx's) cannot say what a control does. Design: the "Simple Controller Editor: design proposal" doc, answered on 2026-10-27.

## What Changes

- A Controller Editor that fills the window, opened from Settings › Controllers (mapping is not done during playback). This change ships it view only.
- Every mapped control listed **by function**: categories (Transport, Cues, Loops, Tempo & sync, Mixer, EQ & filter, Effects, Stems, Samplers, Library, …), each row a function and its target (Deck 1, FX 2, Main) with the hardware control that does it, the layer (SHIFT, pad mode, bank), an icon for its kind (button, pad, knob, fader, encoder, jog) and its MIDI message.
- What script rows do is found by running the mapping in a sandbox: DJ Sugar presses each control once (and again with SHIFT held) against a copy of the engine's controls and records which controls change.
- A Lights tab lists what each LED shows, found the same way.
- An Options tab lists the mapping's own settings (the Twister's 11).
- Pictures of the DDJ-FLX4, Xone K2 and Midi Fighter Twister, drawn by DJ Sugar: click a control to list what it does; pick a row to see the control light up.
- Press to find: touching a control on the connected hardware selects its rows and lights it on the picture.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `controllers`: Controller Editor (view only)

## Out of Scope

- Changing, duplicating or renaming mappings, Map mode and learning (`add-controller-mapper`).
- Pictures of other controllers; they get the list only.
- Long-press and hold-time behaviour in the sandbox (one press and release per control).

## Impact

crates/controllers (descriptions and option definitions from the XML, sandbox probe, function catalogue, controller layouts, a MIDI monitor), crates/app (editor window, Settings › Controllers button), user guide `controllers.md`.

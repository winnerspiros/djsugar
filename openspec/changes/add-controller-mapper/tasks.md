# Tasks

## 1. Own mappings

- [x] 1.1 DJ Sugar mapping file (name, based_on, changes) with create/rename/delete tests
- [x] 1.2 Mapping menu in the Controller Editor: Original, own copies, Duplicate, Rename, Delete
- [x] 1.3 The controller runs the picked copy with its changes applied; Reload Original (a row's Back to original, or Original mapping in the menu)

## 2. Editing

- [x] 2.1 Function menu, target, advanced options (acts on, button, knob type, invert, pick-up) folded with a hint each; screenshot. The first change to the original makes "My …" (2026-10-29)
- [ ] 2.1b LED editing in the Lights tab; range and 14-bit pairing once the runtime applies them
- [x] 2.2 Replace a script row with a plain binding
- [ ] 2.3 Editable mapping options

## 3. Map mode

- [x] 3.1 Map mode over the main window with learn, undo and conflicts (replace, Keep both); MIDI button in the top bar and the editor (2026-10-29). Tried with the FLX4 stand-in; still to try on the Xone K2
- [x] 3.2 Quick map for a controller with no mapping, or from scratch: 2 or 4 decks, walk with Skip, pads 3-8 and decks 2-4 guessed (2026-10-29). Tried with an unknown stand-in device
- [x] 3.3 Update the matching user guide page in website/book/src/guide (controllers.md)

## 4. Fixes after trying it (2026-11-01)

- [x] 4.1 Map mode reaches the FX bar (ON, D/W, slider, ‹ ›), the pad mode tabs (an DJSugarScript row `deck N set 'pad_tab' K`) and headphone cue
- [x] 4.2 The MIDI button shows the MIDI Association wordmark, no socket icon
- [x] 4.3 Quick map becomes the Mapping wizard: asks which controller (buttons, or a press on it); the computer's own ports (Network Session, IAC Driver, Midi Through, GS Wavetable) are never offered or called unmapped
- [x] 4.4 Used with, Deck order and Knobs and faders move from Settings › Controllers to the editor's Options tab
- [x] 4.5 Settings › Controllers lists like Audio: + adds a row with a device menu, × removes; Find controllers and the Mixxx note go
- [x] 4.6 The MIDI button leaves the top bar: Map mode starts from MIDI map in the Controller Editor, which opens again when Map mode is done

## Workflow follow-up

- Archive the change once its PR is merged, so the main specs take its requirements.

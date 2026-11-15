# Design

## Decisions (2026-10-27)

- Organise by function first, not by MIDI message.
- View only first; editing follows in `add-controller-mapper`.
- A large window, since mapping is not done during playback.
- Pictures for the three controllers on hand for testing; others get the list.
- Show MIDI output (LEDs) too, in its own tab so the main list stays clean.

## How script rows get a meaning

Mixxx scripts can't be read statically. The editor loads the mapping into a fresh `ControlRegistry` with every engine control (as the controller tests do, without running the engine) and runs its `init`. For each input message it sends a press and release (notes) or a small move (CC, 14-bit pairs, pitch bend), polls the controller, and records every control whose value changed or that the script tried to write (writes dropped by soft takeover still count). Rows whose script key or description names SHIFT are held down for a second pass, so SHIFT functions of script-driven controllers (K2, Twister) are found too. Rows with no effect keep the mapping's description or the script function's name, under "Controller".

LEDs: each control found above (and each `<output>` row) is set to on and back in the sandbox; the MIDI the controller sends meanwhile is that control's light.

## Controller pictures

DJ Sugar draws its own simple top views from a layout file per controller (`crates/controllers/layouts/*.json`): each control's position, size, shape, kind, label and the MIDI messages it sends with their layer ("SHIFT", "Hot cue", "Bank 2"). A message may leave the channel open (`B? 00`) for the Xone K2, whose channel is set on the device. Mixxx manual drawings and Allen & Heath's sheet were used only as references for positions.

## Press to find

The device thread copies incoming messages to a small queue while the editor is open. The editor matches them to rows and picture controls by status (note off as note on) and number.

# Proposal

Planned for: **v0.5 · Stems, performance and switching over** (moved from v1.0, 2026-10-29; the view-only editor shipped first as `add-controller-viewer`)

## Why

Every DJ app lets you change what a control does. DJ Sugar runs Mixxx's mappings but a DJ can't change one or map an unsupported controller. A user request (2026-10-29): the editor should be easier to use and quick to map with.

## What Changes

- Builds on the Controller Editor from `add-controller-viewer`.
- Each controller has its **Original Mapping** (Mixxx's, read only) and any number of the DJ's own: **Duplicate** and **Rename**; one mapping per controller is in use at a time; **Reload Original** is always there.
- In a mapping of your own: change a row's function from a categorised, searchable menu (target chosen once: Deck 1-4, left/right side, FX unit, Main, Library), add or remove rows, and set advanced options: control type, button behaviour (hold, toggle, trigger), encoder mode, invert, takeover (jump, pick-up, value scaling), range, 14-bit pairing, comment. LEDs are edited in the Lights tab.
- Script-driven rows can be replaced by a plain binding; the script keeps running for the rest.
- Map mode, as in Ableton: the main window tints, click an on-screen control then move a hardware control; Learn stays armed, Enter or ⌘Z undoes the last one; Replace or Add when a control is already used.
- **Quick map** for a controller with no mapping: a guided walk through the usual controls (play, cue, sync, jog, tempo, EQs, filter, volume, crossfader, pads, browse, load) where DJ Sugar highlights each one on screen and the DJ moves the matching hardware control or presses Skip; the result is the DJ's own mapping, ready to fine-tune.
- Easier to use: search rows by name or by pressing the control, clear names instead of Mixxx control keys, and a short hint on each advanced option.
- Mapping options (Mixxx's `<settings>` plus DJ Sugar's own) can be changed per mapping.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `controllers`: own mappings, editing, Map mode

## Out of Scope

- Saving back to Mixxx's format (2026-10-27: Mixxx mappings are read in; DJ Sugar mappings are their own format so they can do more).
- Editing JavaScript.
- HID controllers (much later).

## Impact

crates/controllers (DJ Sugar mapping format, writer, learn), app editor and main window hit-testing for Map mode.

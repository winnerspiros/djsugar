# Proposal

Planned for: **v0.5 · Stems, performance and switching over** (issue #133, with `add-controller-mapper`)

## Why

A DJ switching to DJ Sugar has years of mappings in VirtualDJ, Traktor, rekordbox or djay Pro: LED tricks, shift layers, pad pages, keyboard keys. Making them again by hand is the biggest thing stopping a switch.

## What Changes

- One documented mapping format, `.djsugar-mapping` (website/book/src/dev/mapping-format.md), a superset of Mixxx's `.midi.xml`: plain bindings and JavaScript as Mixxx has them, plus DJSugarScript rows, layers, long and tap presses, script lights with colours and levels, variables, greyed rows and `based_on`.
- Import in the Controller Editor's Mapping menu from VirtualDJ (`.xml`), Traktor (`.tsi`), rekordbox (`.midi.csv`), djay Pro (`.djayMidiMapping`) and DJ Sugar files. Each file becomes a mapping of the DJ's own, based on the controller's Mixxx mapping. What can't come across is kept greyed with the reason, and a report says how many rows came across.
- Export to a self-contained `.djsugar-mapping` and to Mixxx (`.midi.xml` with its scripts), with a report of what Mixxx can't do.
- DJSugarScript gains `adjust 'control' [level]` and `value` (what the controller sent), and lights that send a level (LED rings, VU meters) or a colour by note (Xone K2).
- Keyboard shortcuts come in named sets picked in Settings › Keyboard: new set from the current one, rename, delete, and Import… of a VirtualDJ keyboard mapping.

## Capabilities

### Modified Capabilities

- `controllers`: mapping format, import, export, keyboard shortcut sets

## Out of Scope

- Native Instruments controllers in their own mode (S2, S4 in Traktor): the file only names NI's controls.
- Reading VirtualDJ's encrypted controller definitions: DJ Sugar places VirtualDJ keys with its own tables (FLX4, Xone K2, Midi Fighter Twister first).
- Editing rows (that is `add-controller-mapper`).

## Impact

crates/controllers (format, import/, export), crates/script (verbs), app Controller Editor and Settings › Keyboard.

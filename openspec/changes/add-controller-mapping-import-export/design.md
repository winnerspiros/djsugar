# Design

## Decisions (2026-10-29)

- Bring real-world mappings across first: the Xone K2 with its VU-meter LEDs, the Midi Fighter Twister, the DDJ-FLX4, Traktor and rekordbox files.
- DJ Sugar mappings are a superset of Mixxx's, so a Mixxx mapping reads in with nothing lost; going back, rows that are one control become bindings and the rest are reported.
- Anything that can't come across is a greyed `off` row with `original` and `note`. An `off` row never replaces a base binding, so an import can't make a working control go dead.
- Each imported file is a mapping of its own; several devices on one port in a Traktor file merge into one.
- Keyboard shortcuts are sets chosen by a dropdown; Default stays in `keyboard_shortcuts`, the rest in `keyboard_sets`.

## Readers

- **Traktor**: the base64 `DeviceIO.Config.Controller` frames (DIOM › DEVI › DDAT › DDCB). Command ids map to DJSugarScript; modifier conditions become layers `mN=v`; the condition layout differs before mapping version 3.
- **VirtualDJ**: VDJscript is parsed and translated. `holding` splits into long and tap rows; `device_side`, `action_deck` and similar are settled per control. Keys are placed with DJ Sugar's tables because the definitions are encrypted.
- **rekordbox**: the MIDI Learn CSV columns per deck.
- **djay Pro**: the plist's messages and actions.

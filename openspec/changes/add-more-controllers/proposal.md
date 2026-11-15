# Proposal

Planned for: **v0.5 · Stems, performance and switching over** (2026-10-29)

## Why

Only the DDJ-FLX4, Xone K2 and Midi Fighter Twister are tested with DJ Sugar and drawn in the Controller Editor. Most people switching over own one of a handful of popular entry and mid-range controllers.

## What Changes

- A short list of popular MIDI controllers made "supported": mapping checked, an DJ Sugar layout drawing for the Controller Editor, pad modes and lights working, and a page in the user guide. The final list is a maintainer decision; candidates:
  - With a Mixxx mapping already in `res/controllers` (check, fix, draw): Pioneer DDJ-400, DDJ-200, DDJ-SB3, Numark Mixtrack Pro FX and Platinum FX, Hercules DJControl Inpulse 200/300/500, Denon MC7000, Roland DJ-505.
  - Without one (DJ Sugar's own mapping, from the maker's published MIDI message list): Pioneer DDJ-FLX6, DDJ-FLX2, DDJ-REV1, Numark Party Mix II.
- Supported controllers are marked as such in Settings › Controllers.
- Testers with the hardware are asked for on Reddit and GitHub, since only the three above are on hand for testing.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `controllers`: a supported-controllers list

## Out of Scope

- HID controllers (Traktor S-series and others; much later).
- Standalone players and mixers.

## Impact

crates/controllers layouts and DJ Sugar mappings, website/book controller pages.

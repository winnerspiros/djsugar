# Proposal

Planned for: **v0.5 · Stems, performance and switching over** (2026-11-01)

## Why

Native Instruments' Traktor Kontrol controllers (S2, S3, S4, F1, Z1, MX2) are
USB HID devices, not MIDI ones, so DJ Sugar couldn't use them. They're common
among DJs switching over from Traktor. Mixxx drives them with HID mappings
(`.hid.xml` plus JavaScript), the same way it drives MIDI controllers, so
DJ Sugar can run those mappings as it runs Mixxx's MIDI ones.

## What Changes

- An HID backend in `crates/controllers` (`hid.rs`, with `hidapi`): devices are
  found by the vendor and product IDs their mappings name (and the USB
  interface, or usage page and usage, as Mixxx matches them), opened shared on
  macOS, read and written on a thread of their own.
- Mixxx's HID script API: `incomingData(data, length)` with each input report
  as a `Uint8Array`, `controller.send`, `sendOutputReport` (skipping unchanged
  reports, or FIFO), `getInputReport`, `sendFeatureReport` and
  `getFeatureReport`. Repeated input reports are skipped, as in Mixxx.
- Mixxx's HID mappings for the Traktor range are bundled, unchanged:
  S2 MK1/MK2/MK3, S3, S4 MK2/MK3, F1, Z1, MX2, with
  `common-hid-packet-parser.js`.
- HID controllers are listed in Settings › Controllers with the MIDI ones
  (Automatic or Off). The Controller Editor, Map mode and Quick map leave them
  out: an HID mapping is all script.
- The S2 MK3, S3 and S4 MK3 join the supported list as Beta, with guide pages,
  until testers try them.
- Linux: a udev rule (`res/linux/70-djsugar-hid-controllers.rules`, in the
  download too) so the person at the computer can open them.

## Capabilities

### Modified Capabilities

- `controllers`: HID controllers run Mixxx's HID mappings.

## Out of Scope

- The S4 MK3's screens (Mixxx's `.bulk.xml` mapping and QML screens).
- Other makers' HID mappings (Hercules, Pioneer CDJ HID, EKS Otus); they would
  run the same way once someone can try them.
- Editing or remapping HID controllers in the Controller Editor.
- Traktor Kontrol models Mixxx has no mapping for (X1 MK2/MK3, Z2, S5, S8,
  D2). The first X1 is a MIDI controller and already works.

## Impact

`crates/controllers` (new `hid` module, `Controller::new_hid`, manager scan),
`crates/app` (Settings › Controllers, Map mode and Controller Editor skip HID),
`res/controllers`, `res/linux`, CI installs `libudev-dev`.

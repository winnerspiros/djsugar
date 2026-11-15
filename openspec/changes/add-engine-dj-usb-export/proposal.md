# Proposal

Planned for: **v0.7 · Your library, everywhere** (issue #578)

## Why

Denon DJ (Prime, SC), Numark and Rane standalone players run Engine OS and read a USB stick prepared by Engine DJ. Many clubs and mobile DJs play on them. DJ Sugar's USB export only writes Pioneer's formats, so a DJ on Engine gear has to keep a second library in Engine DJ.

EngineX-OS (<https://github.com/Cd1s/EngineX-OS>, MIT) writes Engine's library on the players themselves and has measured its layout on Engine OS 5 hardware: the `m.db` schema (3.0.2) and the byte layouts of each track's performance data (beat grid, overview waveform, hot cues, loops, track data). With that, Engine DJ fits into the existing export with little new code.

## What Changes

- `djsugar-export` gains an Engine DJ writer: `Engine Library/Database2/m.db` (schema 3.0.2) with tracks, playlists and folders in order, album art in `AlbumArt` and `Engine Library/Artwork/<hash>.jpg`, and each track's `PerformanceData` made from DJ Sugar's own analysis: the beat grid as markers at each tempo change, a 1024-entry low/mid/high overview waveform, hot cues A to H, eight saved loops, the main cue (the first memory cue) and the key.
- The audio goes in the same `/Contents` folder as the Pioneer export (Engine's paths read `../Contents/...`), so one stick can serve both brands with the music on it once.
- The **Export to USB** dialog (File › Export › USB…) gains two player choices: Denon DJ, Numark and Rane (Engine DJ), and Pioneer DJ and Engine DJ together.
- An Engine Library on the stick that DJ Sugar didn't write (from Engine DJ itself) is never replaced: the export stops and says so.
- Marked experimental, like the Pioneer export, until it has been tried on a player.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `usb-export`: Engine DJ players as a target.

## Out of Scope

- Reading an Engine stick or Engine DJ desktop library (import).
- Adding to an Engine library someone else made; each export replaces DJ Sugar's own.
- Smart lists, Prepare lists, history and play counts flowing back.
- Engine's stems and the high-resolution waveform (Engine OS draws it from the audio).
- Engine Library on SD cards or over Engine Link.

## Impact

`crates/export/src/engine/` (writer, schema and performance blobs; adds rusqlite, flate2, sha1, base64 and getrandom to the crate, all already in the workspace). App: `browser/usb.rs`, `ui/usb_export.slint`. Guide page `usb-export.md`. NOTICE credits EngineX-OS.

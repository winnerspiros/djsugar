# Proposal

Planned for: **v0.4 · Your collection, cleaned up** (built early, 2026-10-28; experimental until it has been tried on a player)

## Why

Many DJs play out on Pioneer players that only read a USB stick prepared by rekordbox. Exporting straight from DJ Sugar lets them prepare in DJ Sugar and play anywhere, without keeping a second library in rekordbox. VirtualDJ and Engine DJ already do this.

Earlier this was decided against because there was no player to test on. rbox (<https://github.com/dylanljones/rbox>, Rust, GPL-3.0-only) now writes both of Pioneer's stick formats and their analysis files, so the work is small enough to build and ship marked experimental. Research: `/mnt/project-files/notes/pioneer-usb-export-review.md` (§7 covers rbox).

## What Changes

- A new `djsugar-export` crate writes a stick: the audio, each track's analysis files (`ANLZ0000.DAT/.EXT/.2EX`: beat grid, cues, loops, waveforms), album art, and the library in both formats: the Device Library (`export.pdb`, `exportExt.pdb`) and OneLibrary (`exportLibrary.db`).
- Everything comes from DJ Sugar's own library and analysis cache; tracks never analysed are analysed first.
- **Export to USB…** in the tree's right-click menu, on any list, folder, smart list or stage, opens a dialog to pick the drive and which players the stick is for, then shows progress and a result.
- byte layouts rbox leaves to the caller (waveform encodings, cue colours, the analysis folder hash, MP3 frame counts) follow Bake'n Deck (`baken`, MIT), with its rekordbox test vectors.

## Capabilities

### New Capabilities

- `usb-export`: exporting lists to a USB stick for Pioneer players

### Modified Capabilities

None.

## Out of Scope

- Reading a Pioneer stick or rekordbox library (import stays dropped).
- Keeping several exports side by side on one stick: each export replaces the stick's playlists.
- Players' history and play counts flowing back from the stick.
- Formatting the stick, and exporting to SD cards or computers on Pro DJ Link.
- Phrase (PSSI) and song structure data, and the CDJ-3000's extended (PQT2) beat grid; players fall back to the plain grid.

## Impact

New crate `crates/export` (rbox-devlib, rbox-onelib, rbox-anlz 0.3). OneLibrary is SQLCipher, so the workspace's SQLite becomes SQLCipher (bundled; OpenSSL vendored on Linux, CommonCrypto on macOS); plain databases open as before. App: `browser/usb.rs`, `ui/usb_export.slint`. NOTICE gains rbox and baken.

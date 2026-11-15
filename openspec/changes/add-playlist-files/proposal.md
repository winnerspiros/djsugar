# Proposal

Planned for: **v0.5 · Stems, performance and switching over** (2026-11-02, from the ecosystem standards review)

## Why

M3U8 is the one playlist format every DJ app, media player, car stereo and library tool reads. DJ Sugar reads VirtualDJ's M3U playlists during the VirtualDJ import but can't export any list as a file, and can't take in a loose `.m3u8` someone sends. Research: notes/ecosystem-standards-review.md in the project files.

## What Changes

- **Export:** right-click any playlist, crate, set, folder, History day, Albums or Mixed Sets entry, or the Automix list › **Export as M3U8…**. A folder exports as a folder of `.m3u8` files with the same tree. Options: absolute paths (default) or paths relative to where the file is saved (for a USB stick or a shared folder).
- **Import:** drop `.m3u`, `.m3u8`, `.pls` or `.xspf` files on the Playlists node or the browser, or use **Import Playlist File…** in the Playlists + menu. Each becomes a playlist. Tracks already in the collection are matched; others are added to the collection (Inbox stage); missing files are listed in a short report.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `music-library`: playlists export to and import from M3U8, PLS and XSPF files

## Out of Scope

- Keeping an exported file in sync as the list changes (a later "Keep exported" tick).
- Cues, grids or tags in the file; M3U can't hold them (rekordbox XML export does, see add-rekordbox-xml-export).
- Streaming URLs as playable entries (placeholders are written as comment lines).

## Difficulty

**Small: 1 to 2 days, one PR.** M3U parsing exists in `crates/plugins/src/virtualdj/lists.rs`; PLS and XSPF readers are a few dozen lines each; matching reuses the collection's path index.

## Performance

None on the audio thread. Exports and imports run on a worker thread with a progress line, like Analyze All.

## Impact

library (export/import functions), app (tree and browser menus, drop handling, report dialog), user guide (Playlists page).

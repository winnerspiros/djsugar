# Proposal

Planned for: **v0.7 · Your library, everywhere** (a user request after the EngineX-OS comparison, 2026-11-10; issue #579)

## Why

djay Pro is one of the most used DJ apps on the Mac, iPad and Windows, and DJ Sugar already reads djay Pro controller mappings, but a DJ moving from djay has to set up every hot cue, loop and playlist again. The other imports (VirtualDJ, rekordbox, Mixxx, Traktor) set the pattern: one importer per app, read-only, into DJ Sugar's field superset.

## What Changes

- **Import from djay Pro** in Settings › Collection › Import, with the same three-step wizard as the other apps. DJ Sugar finds `MediaLibrary.db` in `~/Music/djay/djay Media Library.djayMediaLibrary` (Mac) or `Music/djay/djay Media Library` (Windows) and opens it read-only, never writing it.
- Each local track keeps its title, artist, album, length, BPM, key, hot cues 1 to 8 with names and colors, saved loops, Automix start and end (as Automix points), play count, track color and grid anchor (a steady grid from djay's first downbeat when the DJ set one).
- djay's playlists come across as lists in a djay Pro folder, in djay's order. Streamed tracks (TIDAL, Beatport, Apple Music) stay out of lists for now.
- A TSAF reader (djay's record format) and a macOS bookmark reader (where djay keeps each file), credited to the MIT projects they follow: djay-pro-autohotcue, tsaf-parser, whats-now-playing's djay input, and EngineX-OS's djay reader.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library-import`: djay Pro importer

## Out of Scope

- djay's beat grid beyond its first downbeat (djay keeps the rest in its analysis, which DJ Sugar redoes).
- Writing back to djay.

## Impact

`crates/plugins/src/djay` (new), the importers list, NOTICE, the "Your library" guide page.

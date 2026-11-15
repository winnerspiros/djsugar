# Proposal

Planned for: **v0.7 · Your library, everywhere** (a user request, 2026-11-11; issue #630)

## Why

Many DJs keep their music in iTunes or the macOS Music app and build their playlists there. DJ Sugar already brings in VirtualDJ, rekordbox, Mixxx, Traktor and djay Pro libraries, but a DJ coming from iTunes or Music has to add their folders by hand and lose their playlists, stars and play counts.

## What Changes

- **Import from iTunes** in Settings › Collection › Import, with the same three-step wizard as the other apps. DJ Sugar reads the Library XML (an Apple property list): the Music app's shared `Music/Music/Library.xml`, iTunes' `iTunes Library.xml` / `iTunes Music Library.xml` on Mac and Windows, or an exported `Library.xml` in Music or on the Desktop, the newest of those found. When none is found, the wizard says how to export one.
- Each local track keeps its title, artist, album, genre, grouping, composer, comments, year, track and disc number, BPM, stars (not computed album ratings), play count, date added and last played.
- Playlists come in their folders. Music's own lists (Library, Music, Purchased, Genius…) and hidden lists stay out. Smart playlists are a separate group, off by default, coming in as plain lists of what they held.
- Podcasts, audiobooks, videos, voice memos and internet radio stay out. Apple Music and iCloud-only tracks, and protected downloads, stand in lists as streamed-track placeholders (service "applemusic").
- Importers can give a hint for the wizard when their file isn't found (`LibraryImport::missing_hint`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library-import`: iTunes / Music importer

## Out of Scope

- Reading the Music app's own `.musiclibrary` database or the macOS iTunesLibrary framework (no XML needed); possible later on macOS.
- Converting smart playlist rules into DJ Sugar smart lists.
- Writing back to iTunes or Music.

## Impact

`crates/plugins/src/itunes_library.rs` (new), the importers list, the wizard's not-found line, the "Your library" guide page.

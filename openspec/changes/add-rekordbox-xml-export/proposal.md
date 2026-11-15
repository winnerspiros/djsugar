# Proposal

Planned for: **v0.6 · Buy it from the artist** (moved from v0.5 at the 0.5.0 release) (2026-11-02, from the ecosystem standards review)

The tasks still open moved to **v0.8 · From the booth to the studio** on 2026-11-12, so v0.7 ships with what is built.

## Why

rekordbox XML is the exchange format of the DJ library world. rekordbox imports it, and library tools (Lexicon, Rekord Buddy, Mixed In Key, DJ.Studio and others) read it and convert it to Serato, Traktor, Engine DJ and VirtualDJ. One exporter lets a DJ take DJ Sugar's prep (cues, loops, grids, colours, playlists) to almost any other app, which makes switching to DJ Sugar less of a one-way door. DJ Sugar already reads the format (`crates/plugins/src/rekordbox/xml.rs`), so the field mapping is known. Research: notes/ecosystem-standards-review.md in the project files.

## What Changes

- **File › Export › rekordbox XML…** beside the other exports (the list or folder picked in the sidebar, or the whole collection when Collection is picked), and **Export as rekordbox XML…** in the Collection node's menu and on any list or folder (that list's tracks and the list tree only).
- The DJ's tags go into the comment as `/* Tag / Tag */`, as rekordbox's "Add My Tag to comments" writes them; pads past H go out as memory cues; track colours as the nearest of rekordbox's eight.
- Each track carries its tags, rating, colour, comment, date added, play count, beat grid (every grid marker, so variable tempo survives), hot cues with numbers and colours, memory cues, and saved loops. Playlists, crates, sets and folders keep their tree.
- Optional **Keep rekordbox.xml up to date** in Settings › Collection › General: DJ Sugar rewrites the file in the background a minute after the collection changes, so a library tool or rekordbox can be pointed at it once.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `library-import`: the rekordbox XML format also goes out, not only in

## Out of Scope

- Traktor NML and Serato exports (library tools convert from rekordbox XML; native writers can follow later).
- Phrases, stems and DJSugar-only fields (stages, sets' set-list metadata), which rekordbox XML can't hold.
- Writing into rekordbox's own master.db.

## Difficulty

**Small to medium: 2 to 3 days, one PR.** The writer is a straightforward XML tree. The fiddly part is timing: rekordbox measures MP3 positions from a different start than some decoders (the LAME/encoder delay), which tools like Lexicon correct with an offset. The task list includes a round trip through our own reader and a check against rekordbox 7 on a test Mac with a few MP3, AAC and FLAC files, so cues land on the same transient in both apps.

## Performance

None on the audio thread. Export runs on a worker thread reading the collection database; the background "keep up to date" writes at most once a minute and only after a change.

## Impact

library or plugins (new `rekordbox/xml_export.rs` beside the reader), app (menus, settings tick, progress line), user guide (Moving your library to and from other apps).

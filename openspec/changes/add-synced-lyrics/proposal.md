# Proposal

Planned for: **v0.7 · Your library, everywhere** (moved from v1.x on 2026-11-06 after the lyrics and karaoke research; the karaoke screen follows as its own change)

The tasks still open moved to **v0.8 · From the booth to the studio** on 2026-11-12, so v0.7 ships with what is built.

## Why

VirtualDJ shows lyrics next to the waveforms and on its video output, and rekordbox has line- and word-synced lyrics on its video decks; Apple Music and Spotify made lyrics something listeners expect. Many DJs already have lyrics: `.lrc` files from tools such as LRCGET, and lyrics in tags written by MusicBee, foobar2000 or Mp3tag. DJ Sugar reads none of them. Lyrics also lead to a karaoke screen with the instrumental from the vocal stem, which is the next change.

## What Changes

- **Reading lyrics** (`djsugar_engine::lyrics`): an `.lrc` file beside the track (`Song.lrc` or `Song.mp3.lrc`), plain LRC and enhanced LRC (a time before each word, `<mm:ss.xx>`), with `[offset:]` and lines sung at several times; then the track's tags: ID3 `SYLT` (timed, by line or by word), ID3 `USLT`, Vorbis/FLAC `LYRICS`, MP4 `©lyr` (these often hold LRC text, which is read as timed). The finest timing found wins.
- **Lyrics tab** in the side panel: follows the track playing (or the selected one, or deck A or B), says where the lyrics are from and how they are timed, lights the line sung now and keeps it in view, and a click on a timed line plays the deck from there.
- **Player mode Lyrics view**: a Lyrics button on the Player bar opens the lyrics large in place of the browser, with sung words lit when they are timed by word (built once the Player redesign settles).
- **Finding lyrics online, when asked**: Find lyrics in the tab looks the track up on LRCLIB (lrclib.net, a free community database with no key), on at first and switched off in Settings › Collection › Health › Find lyrics online. It looks up the track by artist, title, album and length (LRCLIB matches within 2 s, so a radio edit doesn't get the album version's timing) and saves the pick as an `.lrc` beside the track (in DJ Sugar's own lyrics folder when the track's folder can't be written). Collection Health's Info area offers it for many tracks at once, one request at a time.
- **Fixing lyrics**: edit the text, tap to sync plain lyrics (a key per line while the track plays), nudge the timing, and save to the `.lrc` or the file's tags.

## Capabilities

### New Capabilities

- `lyrics`: reading, showing, finding and fixing a track's lyrics

### Modified Capabilities

None.

## Out of Scope

- Musixmatch, LyricFind, Genius and other licensed or closed lyric services: their full and synced lyrics need a paid commercial key, which an open-source app can't keep secret.
- Sending lyrics anywhere (LRCLIB's publish endpoint stays unused).
- Word timing from the audio (Whisper on the vocal stem) and transcribing tracks with no lyrics: a later change.
- The karaoke screen, and lyrics on the deck waveforms: later changes.

## Difficulty

**Medium: four PRs.** Parsing is small and well tested; the tab follows the Info tab's pattern. LRCLIB is a plain JSON GET with an identifying User-Agent.

## Performance

Lyrics are read on a thread of their own when the tab's track changes, never on the audio thread. With the tab closed nothing is read. Following the line sung is one binary search per frame on the UI thread.

## Impact

engine (new `lyrics` module on lofty), app side panel (Lyrics tab), Player bar, Settings › Library, Collection Health; user guide page Lyrics.

# Proposal

Planned for: **v0.5 · Stems, performance and switching over** (2026-11-02, from the ecosystem standards review)

## Why

DJs who stream (Twitch, YouTube, Mixcloud Live, Kick) want the current track on screen, and VJ and lighting software wants to know what is playing. VirtualDJ, Serato (through third-party tools) and Mixxx all get asked for this, and the usual answer is a text file that OBS reads. DJ Sugar already knows which deck the audience hears and has a small web server for the phone remote, so the feed costs little. Research: notes/ecosystem-standards-review.md in the project files.

## What Changes

- A **Now Playing** switch in Settings › Mixer (off at first).
- When on, DJ Sugar decides which track the audience hears (the loudest deck in the master mix, held for a few seconds so a quick cut doesn't count) and publishes it:
  - `now-playing.txt` with a template the DJ can change (default `{artist} - {title}`), and `now-playing.json` with title, artist, album, cover, BPM, key, deck, start time and the last ten tracks, written to a folder the DJ picks;
  - an **overlay page** at `http://127.0.0.1:<port>/now-playing` with a clear background, for an OBS Browser Source, in three styles (lower third, corner card, ticker);
  - the same JSON at `/now-playing.json` and as events on a WebSocket, for other tools.
- A copy button for the overlay address and a preview of the overlay in Settings.

## Capabilities

### New Capabilities

- `now-playing`: tell streaming, overlay and visuals software which track the audience hears

### Modified Capabilities

None.

## Out of Scope

- Sending the tracklist to Mixcloud, Twitch chat or scrobbling services (later, with add-live-broadcast).
- A public, internet-facing page. The overlay page answers this computer only unless the DJ allows the local network.
- Showing artwork from streaming services.

## Difficulty

**Extra small to small: 1 to 2 days, one PR.** The "audible deck" rule is the only real logic; the HTTP and WebSocket plumbing exists in `crates/remote` (it needs a read-only route set that skips pairing and is bound to 127.0.0.1 by default).

## Performance

Nothing runs on the audio thread. The app already reads deck levels for the meters; the audible-deck rule runs on the UI thread at 4 Hz using values it already has. Files are written on a worker thread, only when the track changes, by writing a temporary file and renaming it, so OBS never reads a half-written file.

## Impact

app (`now_playing.rs`, Settings › Mixer section), `crates/remote` (read-only `/now-playing*` routes and overlay page), user guide (new "Streaming overlays" section on the Remote page or its own page).

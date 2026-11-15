# Proposal

Planned for: **v0.7 · Your library, everywhere** (from a gap review of Player mode against Spotify and TIDAL, a user request 2026-11-09; issue #536)

## Why

Spotify and TIDAL never stop: when the queue ends they play on with tracks like the last one (Autoplay, Track Radio). In DJ Sugar, Player mode goes quiet when Up Next runs out. A DJ's library is better suited to this than a streaming catalogue, since every track has a key and a tempo, but a set that adds tracks on its own is not what a DJ wants on the decks. So it is a switch, off at first, and only in Player mode, like cruise control.

## What Changes

- **Keep going**, a switch in Up Next's header in Player mode, off at first. While it is on and Automix plays, Up Next never runs dry: when fewer than two tracks wait, the next pick goes on the list.
- Picks come from a **pool**: everything (the collection), the list the browser shows when it is switched on, or what Start Radio set. They mix with the track before them: the fewest steps on the Camelot wheel and the smallest tempo change (half and double time count), as Smooth flow orders the list. A little chance among the best few keeps two runs from being the same.
- Tracks played in the last 3 hours, on a deck, in Up Next or already picked this run are left out. The same artist twice in a row is avoided where something else fits.
- The **next 3 picks** show greyed at the end of Up Next, so the DJ sees what is coming and can drag in something else first.
- **Start Radio** in the track menu (a track, or its artist) and the list menu (a crate or playlist), in Player mode: switches Keep going and Automix on with that seed. A track goes next in Up Next (or plays, if nothing does) and picks follow it from everything; an artist's radio plays from their tracks and those sharing their genres; a list's radio plays from that list.
- Switching it on also switches Automix on. Leaving Player mode stops the top-up (tracks already added stay in Up Next).

## Capabilities

### New Capabilities

- `keep-going`: Keep going and Start Radio in Player mode

### Modified Capabilities

None.

## Out of Scope

- Streaming services as a pool (browse only, no playback).
- Learning from skips or likes.
- The DJ layouts.

## Difficulty

**Medium: one PR.** A pure picker (testable), a little state in the browser that tops up the Automix list each frame, a switch and pool picker in Up Next's header, greyed rows, and Start Radio menu lines.

## Performance

Picks are worked out only when the end of Up Next or the pool changes, not every frame; one pass over the pool (a few thousand tracks) scores them.

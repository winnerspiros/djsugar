# Proposal

Planned for: **v0.7 · Your library, everywhere** (from a gap review of Player mode against Spotify and TIDAL, a user request 2026-11-09; issue #536)

## Why

In Spotify and TIDAL a click on an artist or album opens its page: the art, Play and Shuffle, the tracks, the albums. In DJ Sugar's Player, the artist goes nowhere and Go to Artist only narrows the list with a filter chip. A collection is browsed by artist and album as much as by list, so Player needs the same pages, and a DJ also wants them played in an order that mixes.

## What Changes

- **Artist page** in the middle pane, over the list, in Player mode: round art, the name, how many tracks and albums and how long; **Play**, **Shuffle** and **Mix** (in the smoothest order by key and tempo); their tracks in the collection, most played first, any credit counting ("Bicep, Hammer" is on both pages); then their albums, newest first, as covers.
- **Album page**: the cover, the name, the artist (a link to their page), year, tracks and length; the same buttons; its tracks in disc and track order. A compilation shows whole when its tracks share a folder.
- Clicking a track plays it now (on an album, with the rest of the album after it). Play, Shuffle and Mix put the tracks in Up Next and mix into the first now, as the search box's Play now does.
- Opened from the bar's artist, the search box's artists, and Go to Artist / Go to Album in the track menu (Player only; the DJ layouts keep the filter chip). Pages stack: the back arrow goes to the page before, then the list; picking a list in the tree closes them.
- Later in this change: **Home**, what Player opens on (recently played, recently added, crates, and mixes made from the collection by genre and decade).

## Capabilities

### New Capabilities

- `player-pages`: artist and album pages and Home in Player mode

### Modified Capabilities

None.

## Out of Scope

- Streaming services' artists and albums (a browse-only row may follow).
- Artist photos and biographies (no source in the collection).

## Difficulty

**Medium: two PRs.** Pages first (one module building them from the collection, one Slint view), then Home.

## Performance

A page is built when opened, in one pass over the collection; covers come from the album art cache already used by the Albums source.

# Proposal

Planned for: **v0.7 · Your library, everywhere** (from a gap review of Player mode against Spotify and TIDAL, a user request 2026-11-09)

## Why

Every music player answers the keyboard's play/pause, next and previous keys, the buttons on headphones, and the system's own Now Playing (macOS Control Center and lock screen, the Windows media overlay, Linux desktop media widgets over MPRIS). DJ Sugar answers none of them, so Player mode can't be paused from headphones or seen from the menu bar. It was the largest gap in the review.

## What Changes

- DJ Sugar shows the track the Player bar shows in the system's Now Playing: title, artist, album, length, album art, playing or paused, and where in the track it is.
- Media keys, headphone buttons and the Now Playing controls act as the Player bar does: **Play/Pause** (starting also switches Automix on), **Next** mixes into the next track in Up Next, **Previous** goes back to the start of the track; moving the position seeks.
- By default only Player mode takes the keys, so a headphone button can't stop a deck in the middle of a set. Settings › Keyboard › **Media keys in every layout** lets them play and pause the master deck in the DJ layouts too; off at first.
- One library, souvlaki (MIT), covers all three systems: MPRemoteCommandCenter and MPNowPlayingInfoCenter on macOS, SystemMediaTransportControls on Windows, MPRIS over D-Bus (pure Rust zbus, no system library) on Linux.

## Capabilities

### New Capabilities

- `media-controls`: the system's media keys and Now Playing

### Modified Capabilities

None.

## Out of Scope

- Shuffle and repeat from the system controls.
- Volume from MPRIS.

## Difficulty

**Small: one PR.** One module that keeps the system controls in step with the Player bar on a timer and turns their events into the bar's actions.

## Performance

A timer four times a second compares a few fields and calls the system only when something changed. The cover is written as a small PNG once per track.

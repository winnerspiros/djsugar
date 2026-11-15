# Proposal

Planned for: **v0.7 · Your library, everywhere** (from a user request, 2026-11-09, after the lyrics and karaoke changes)

## Why

The Lyrics tab and the karaoke screen show the words of one track. A DJ mixing two vocal tracks wants to see, on each deck, where the singing is and what is sung next, so a vocal doesn't land on top of another. rekordbox shows lyrics in its waveforms for the same reason.

## What Changes

- **Lyrics on the waveforms**: a thin lane along the bottom of each deck's big scrolling waveform (2-Deck, 4-Deck and Prep) shows the track's timed lyrics, each line a strip from when it is sung to when it ends, scrolling with the audio. The line being sung is lit, and its words fill with lilac in step with it.
- Lyrics are read when a track loads on a deck, as the Lyrics tab reads them (`.lrc` beside the track, tags, DJ Sugar's own lyrics folder), on a thread of their own.
- Settings › Decks › **Lyrics on waveforms** turns the lane off; on at first. The lane makes way for an instrument deck's macro lanes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `lyrics`: lyrics on the deck waveforms

## Out of Scope

- Word timing from the audio: `add-whisper-word-timing`.
- A vocal-presence lane from the vocal stem.

## Difficulty

**Small: one PR.** A Slint lane over the existing waveform, positioned from the deck's play position and the waveform's zoom.

## Performance

A track's lyrics are read once when it loads, off the UI thread. The lane is a few dozen rectangles per deck, positioned each frame from two control values; nothing is drawn on the GPU waveform itself.

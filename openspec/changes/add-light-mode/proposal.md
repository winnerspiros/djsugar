# Proposal

Planned for: **v0.7 · Your library, everywhere** (from a user request, issue #646, 2026-11-12)

## Why

DJ Sugar is dark only. Dark panels are hard to read in daylight, at outdoor gigs and in bright rooms, and many people keep their computer in light mode. Serato added a Day Mode for summer gigs in 2019, VirtualDJ has a Daylight colour scheme and rekordbox 7 has a light theme. Of these, rekordbox keeps its waveforms on black, while Serato's put them on white where they read pale; VirtualDJ users add black boxes behind the waveforms in Daylight by hand.

## What Changes

- An Appearance choice in Settings › Layouts: Dark, Light or Match system. Dark stays the default. Match system follows the computer's light or dark setting as it changes.
- Light turns the panels, library, buttons, menus and Settings light. The large waveforms, the overviews and their colours stay on dark wells, as now, so every waveform colour scheme keeps working.
- Electric Lilac #B69CFF stays as a fill behind dark ink. Lilac used as text becomes a deeper #6A4BD4 on light backgrounds; the library selection becomes #7559CC with white text. CUE, deck colours, the key wheel and the collection stage colours get darker light versions so they read on white.
- The Player layout in Light has the calm look of music apps: no striped rows or lines, lilac only for what is playing and for state.
- The native title bar and the system's text fields follow the appearance.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `app-layout`: an Appearance setting with Dark, Light and Match system

## Out of Scope

- Colour schemes made by the DJ, and skin files.
- Light waveform wells (a "Paper" look with waveforms on white).
- The karaoke screen, which stays dark for the audience.

## Difficulty

**Medium: six PRs.** The `Theme` global already holds most colours, so the switch itself is small. Most of the work moves about 744 colour literals in 30 `.slint` files and 33 in Rust onto `Theme` tokens. Those PRs must leave Dark pixel-identical, which before/after screenshots check.

## Performance

Nothing per frame. Switching re-evaluates the colour bindings once.

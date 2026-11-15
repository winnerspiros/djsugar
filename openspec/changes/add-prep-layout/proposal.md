# Proposal

Planned for: **v0.6 · Buy it from the artist** (issue #292; follows improve-beatgrids, issue #134). Steps 1-3 were built in v0.5 behind a switch (`PREP_OFFERED` in `crates/app/src/main.rs`), hidden to leave time to test it.

The tasks still open moved to **v0.8 · From the booth to the studio** on 2026-11-12, so v0.7 ships with what is built.

## Why

Tracks wait in In Prep until their grid, cues and tags are right, but there is no place built for that work: grid tools sit in a jog menu, hot cues on pads sized for performing, and the stage move in the track menu. rekordbox (EXPORT mode), Serato, Traktor, Engine DJ, VirtualDJ and djay Pro all prepare tracks on one deck with a large waveform, grid tools on one side and cue pads on the other, browser below. None ties preparation to a stage, so DJ Sugar can make getting a track from In Prep to Play Ready one short, keyboard-driven pass of 15 to 60 seconds.

## What Changes

- A fourth layout, **Prep**, on Deck 1: no mixer, crossfader or FX bar. Other decks are not played while preparing.
- Header with the stage, the checklist count and **Play Ready, next track** (Enter): locks the grid, moves the track to Play Ready and loads the next In Prep track. The button lights up when every checklist line is ticked.
- Large zoomable waveform with bar numbers; dragging the waveform moves an unlocked grid. Overview with phrases, cues and tempo sections.
- Grid panel from the existing grid controls (Bar 1 here, Beat here, Tap, Click, move, tempo, ½ / ×2, sections, Back to detected, Lock) plus **Check 4 spots**: two bars at four points of the track with the click, showing how far the kick is from the grid.
- Eight named hot cue pads, Memory cue, and **Suggest from phrases**: the first beat or the end of the intro (whichever comes first), all elements in, the drop, and before the outro, shown dashed until kept.
- **Suggested Automix points**: the mix-in and mix-out points Automix would pick from the phrases (or imported points), shown dashed beside the suggested cues; once kept or moved, Automix uses them as the track's own points.
- Side panel Checklist tab: lines the app checks (grid, four hot cues, genre, rating, cover) and lines the DJ defines and ticks, with an optional personal score; edited in Settings › Library › Stages. Cues tab listing hot cues, memory cues and saved loops.
- Quick review keys and an average time per track.
- Later step: editing phrases (kind, edges, split, merge).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `app-layout`: Prep layout, quick review keys
- `beatgrids`: drag to move, Check 4 spots, lock on Play Ready
- `collection-stages`: prep checklist, Play Ready and next
- `performance-pads`: suggested cues from phrases, suggested Automix points, memory cues

## Out of Scope

- Preparing on a headphone-only deck while a set plays.
- Prepare lists for a gig (dropped 2026-11-12; the In Prep stage and Automix lists cover it).
- Writing cues or grids into file tags.

## Impact

app ui (new prep.slint, main.slint layout menu, library.slint side tabs and columns), browser stage moves, engine grid drag and check playback, phrases to cue suggestions, settings (checklist lines), user guide. Mockups in the project notes (prep-layout-review).

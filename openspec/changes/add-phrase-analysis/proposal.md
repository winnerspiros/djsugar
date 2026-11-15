# Proposal

Planned for: **v0.6 · Buy it from the artist** (moved from v0.7 2026-11-06, with add-phrase-aware-automix, which needs its edges)

The tasks still open moved to **v0.8 · From the booth to the studio** on 2026-11-12, so v0.7 ships with what is built.

## Why

Phrases (intro, build, drop, breakdown, outro) feed automatic cues, phrase-aware Automix and lighting later. rekordbox, VirtualDJ and Traktor 4 have them; djay does not. DJ Sugar already has what detection needs (beat grid, 3-band waveform, chroma), so it can be done in Rust inside the normal analysis pass, with no model to download. Research: `notes/phrase-analysis-review.md` in the project files.

## What Changes

- Detect phrases on the beat grid during analysis: whole bars, segmented by band energy and a chroma novelty curve, with a length prior that prefers 8- and 16-bar phrases; labelled Intro, Build, Drop, Chorus, Verse, Breakdown or Outro (sections that come back are matched, so a chorus returning or a second verse gets the same label), with an energy level 1-10.
- Keep phrases in the existing `markers` table (kind `phrase`, source `djsugar`). Imported phrases win and are kept.
- Show them as coloured bands on the overview, phrase lines on the scrolling waveform, and "Drop · 8 bars left" on the deck.
- Automatic cues: a CUE point at the first downbeat with sound and hot cues on phrase changes, named after the phrase, filling only empty pads unless asked.
- Search by phrase (`has:intro>=16`).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `track-analysis`: phrase sections and energy from analysis
- `waveforms`: phrase bands, lines and countdown
- `performance-pads`: automatic hot cues from phrases
- `library-search`: phrase search

## Out of Scope

- Phrase-aware Automix (its own change, `add-phrase-aware-automix`).
- Neural structure models (All-In-One, SongFormer); revisit after `improve-stem-separation`.
- Bundling Essentia or librosa (licence and packaging; see the review).
- Learning cue habits from the DJ's own cues.
- Lighting from phrases (much later).

## Impact

engine analysis (new phrase module), library `markers`, waveform shader, deck header, cue storage (`source = 'auto'`), settings › Analysis.

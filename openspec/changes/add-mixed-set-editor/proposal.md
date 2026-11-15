# Proposal

Planned for: **v0.8 · From the booth to the studio** (moved from v0.7 on 2026-11-12)

## Why

A recorded set is only a sound file today, with a cue sheet beside it. DJ Sugar already logs every control change, track load and jump next to each recording (`<name>.controls.jsonl`), so it can show the set as it was played: which tracks, where each transition happened, how long it lasted and what was moved. Mix editors such as DJ.Studio show mixes this way but cannot open a set played live. This first step is for reviewing a set and making the fixes DJs most often make after recording one (trim, level one track, loudness, cut out a mistake, tracklist with chapters), without the warping and automation lanes that make DAWs hard for beginners. Changing the mix itself, re-rendering through the engine and exports to Ableton follow in v0.8 (add-set-view, add-ableton-set-export).

## What Changes

- **Mixed Set Editor**: right-click a recording in Recordings (or a History set that has one) › Open in Mixed Set Editor. It opens in a **simple view**: the recording's waveform with a block per track and the transitions marked, plus the transitions list. A **Show decks** switch turns on the full timeline: a row per deck with track blocks and waveforms, transitions shaded with their length in bars and the style found (bass swap, filter, echo out, smooth, cut), loops as brackets with a repeat count, scratches and other hard moves hatched, and folded lanes for volume, bass, filter, crossfader and FX. An overview of the whole set scrolls it; zoom goes from the whole set to 8 bars.
- **Playback**: the recording plays with a playhead across the timeline; clicking anywhere jumps there.
- **Transitions list**: a side panel listing every transition (out → in, time, length, style); a click jumps to 4 bars before it.
- **Learn pair mixes**: from the transitions list, save transitions as pair mixes: "track A into track B" with its out point, in point, length in bars and style found. A pair mix is used only when Automix mixes that same pair again; Automix's order is the pair mix, then the tracks' own Automix points (set in Prep or imported, add-prep-layout), then phrases and rules. Learning never changes a track's own points.
- **Check my mix**: a list of things to listen to, each jumping to its place: clipping or the limiter working hard, silences, a big level jump between tracks, beats drifting during a transition, an effect left on past a phrase change, a jump mid-phrase.
- **Quick fixes** on the audio, never changing the recording (each fix is kept as a list of edits and Save writes a new file beside it): trim the start and end with fades; turn one track up or down (-6 to +6 dB, eased over a beat at each end); a loudness preset per platform (Mixcloud, SoundCloud, YouTube: -14 LUFS integrated, -1 dBTP, through the existing loudness meter and true-peak limiter); cut out a section, snapped to bars with a short crossfade.
- **Export with chapters**: a Mixcloud tracklist, YouTube chapters (first at 0:00, at least 3, each at least 10 s), a .cue sheet, and Export for Mixcloud with ID3 chapters as today, all following the fixes (a cut moves later times).
- Recordings without a control log open with the recording's waveform and the cue sheet's tracklist only.


## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `mix-recording`: open a recording as a timeline with its transitions
- `automix`: pair mixes learned from a recorded set

## Out of Scope

- Changing the mix itself (moving transitions, editing curves and loops), rendering through the engine, Keep or Bypass for scratches, frame-accurate capture (add-set-view, later in v0.8).
- Ableton export (add-ableton-set-export).
- Recordings made by other apps (no control log).
- Learning a track's own Automix points from a set: those are set in Prep (add-prep-layout).

## Impact

A control-log reader and timeline model in a new module (shared with add-set-view and add-ableton-set-export), a `pair_mixes` table in the library and an Automix plan lookup, the Mixed Set Editor screen and its menu entries, Check my mix detectors, an audio-only fix renderer (trim, fades, track gain, cut, loudness) reusing the loudness meter and true-peak limiter, chapter exports.

# Design

Builds on `add-groove-box`. The research on other apps is in that change's design.md and the project notes (`notes/pattern-player-review.md`). This file keeps what is new for 2.0.

## Research (2026-11-06)

Full notes: project notes `notes/groove-box-2-research.md`.

| App | What it does well | Taken |
|---|---|---|
| Audiomodern Playbeat 4 | 8 lanes with own length, timing and swing; one dice button for pattern and sounds, a dice per parameter, a lock per lane, a density slider; Remix varies a pattern and keeps its feel; 24 generated variations on keys for live switching | Dice with row locks and a Busy slider, Remix, variations as live pads |
| Synthstrom Deluge (community firmware, GPL-3.0, C/C++) | Per-note probability, probability groups (notes at one step summing to 100 % play exactly one), iterance *X of Y*, Fill / Not fill conditions, nudge at 1/384, repeats, per-row play direction (forward, reverse, ping-pong), song view with clip modes Infinite / Once / Fill | Step conditions (chance, X of Y, Fill, Not fill), probability groups, direction per row, Fill as a step condition rather than a separate pattern |
| Sugar Bytes DrumComputer | 8 synth engines (resonator, oscillator, resynth or sample layers), probability, rolls, ± step delay, humanize, auto and manual fills, intelligent randomiser for sounds | Rolls, Slop, randomise a sound within its engine's range |
| Wave Alchemy Triaz | 12 sample channels with 3 layers blended on an X/Y pad; per-track length and speed; per-step lanes for repeat, chance, start time, pan, pitch, FX sends; Slop; pattern switch keeps position; drag patterns out as MIDI | Pan and pitch per step (later), Slop, MIDI export; X/Y layer blend considered for kits with layered sounds |
| UVI Drum Designer | Kick, snare, clap and cymbal engines mixing samples and synthesis; a few front-page macros per sound with deep editors behind | One named macro knob per row on the face, deep parameters only in the row menu |
| mpump (browser groovebox, AGPL-3.0, TypeScript) | Synthesised drum kits level-matched to classic machines; 9 drum voices plus bass and synth rows with semitone, velocity and slide per step and scale lock; Euclid; one-tap randomise with 3 levels of undo; per-channel trance gate patterns; song mode with scenes and transitions; MIDI clock in, Link through a bridge; a beat shared as a URL | Undo for Dice and Remix, slide and scale lock on the melodic row, a 303-style filter for its synth, share a groove as a short text code |
| Patterning 3 | Each sound a ring with its own step count (polyrhythm at a glance), Euclidean mode, per-ring direction and rotation, parametric swing, a timeline to arrange patterns | Euclid per row, rotate a row, the timeline; the circle itself only as an optional view |

What none of them do, and DJ Sugar can: follow a DJ mix. The timeline lines up with the other decks' waveforms, FILL can fire on the leading deck's phrases, and the melodic row follows the leading deck's key.

### Deluge, mpump and AudioKit code

AudioKit (MIT, Swift for Apple platforms only) is a toolkit, not a groove box. Its Swift layers and sequencers don't carry over, but the C DSP under SoundpipeAudioKit and DunneAudioKit does: Moog ladder, Korg low-pass and TB-303 style filters, FM and phase distortion oscillators, metal bar and plucked string models, transient shaper. MIT code can go into DJ Sugar (GPL-2.0-or-later) with its notice kept. That is the first place to look when the synth voices and the melodic row need filters, checking each file's licence first, since some Soundpipe modules came from Csound under the LGPL.

mpump is AGPL-3.0 JavaScript for the browser: nothing to port into a Rust engine, ideas only.


The Deluge firmware is GPL-3.0; DJ Sugar is GPL-2.0-or-later, so its code can be used in DJ Sugar, but files taken from it become GPL-3.0 and so does every build that contains them. The useful parts (step conditions, probability groups, play direction) are small and well described, so they are written fresh from the behaviour rather than ported; its DSP (filters, reverbs, wavefolder) can be looked at again for the synth voices if a port is worth the licence change.

## Principles kept from 1.0

- One press still plays: a kit loads with a groove, PLAY starts on the next bar, no tempo or clock to set.
- The face stays small. New detail (accent, chance, nudge, rolls, per-row length) lives in hold and right-click menus, and the timeline does nothing until something is dragged onto it.
- A Groove Box is still a deck: its channel, FX, cue and meters are untouched.

## The waveform strip becomes the timeline

A track deck draws its scrolling waveform across the deck. A Groove Box has no file to analyse, so the strip is free. It shows:

```
 bar  1        2        3        4        5        6        7        8
     ┌──────── A ────────┬─ A ──┬─ B ──┬──────── B ────────┬─ FILL ─┐
 KCK |▮   ▮   ▮   ▮   ▮  |▮   ▮ |▮   ▮ |▮   ▮   ▮   ▮   ▮  |▮ ▮▮▮▮ |
 HAT |  ▮   ▮   ▮   ▮    |  ▮   |▮▮▮▮▮▮|▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮▮|▮▮▮▮▮▮ |
     └───────────────────┴──────┴──────┴───────────────────┴────────┘
                    ▲ playhead, same position and zoom as the waveforms
```

- Same zoom and same playhead line as the other decks' waveforms, so a bar on the groove sits under the same bar of the leader: the groove's bars are the leader's bars.
- Blocks are grooves; ticks are hits (one thin lane per row, accent as height). The leader's phrase markers are drawn faintly behind, so "B for 4 bars, then fill" can be lined up with the next drop by eye.
- Empty, it shows the selected groove repeating; dragging A to H onto it builds a chain, click a block to jump there on the next bar, right-click to repeat, remove or set a fill.
- The 4-Deck strips have no room for it; they show the current groove letter and bars left in the chain.

Alternatives considered for the strip: more rows (the grid area grows instead, see below) and the per-sound knobs (they fit in the right column). The timeline is the one thing the strip's width and its alignment with the other waveforms is uniquely good for.

## Drum sounds and patterns (decided 2026-11-06)

After a listening comparison in the mpump porting thread:

- **Sounds:** the built-in kits move to the Plaits drum models (analog and synthetic kick and snare, hi-hat) through the Rust crate `mi-plaits-dsp` (MIT, a port of Mutable Instruments Plaits). Plaits has no clap or cymbals, so those stay DJ Sugar's own synthesis. mpump's drum synth and sound machines are not used (its highs sound tinny).
- **Patterns:** mpump's 20 genres, about 400 named drum patterns (names kept as they are, artist nods included) and its A/B chain (alternate two grooves every bar) are kept and played on DJ Sugar's kits. They are AGPL-3.0 data, so the relicence ask to their author narrows to the pattern files.
- **Rows:** nine drum rows in mpump's order (kick, rim, snare, closed hat, open hat, cowbell, cymbal, clap, ride) so its patterns map one to one; the Steps pad page gets a shift page for the ninth. Kit names change only slightly ("808", "909", "Linn-style", never a full model name).
- **Now:** eight tracks with the SEQUENCER, SYNTH and MIXER tabs are built (task 3.0); going to nine rows comes with the patterns.

## Grid and rows

- 8 rows on screen in 2-Deck when the window is tall enough, else two 4-row views with a 1-4 / 5-8 switch (the same shifter idea as the pads).
- Each row: name, steps, M, S, and its length shown as the last lit step; dragging the row's end handle sets its length (1-64), so a 3-step hat runs against a 16-step kick.
- Step size per row (1/16, 1/8, 1/16T) and direction (forward, reverse, ping-pong) from the row menu, plus Rotate left / right.
- **Euclid** in the row menu: spread N hits evenly over the row's length (3 over 8 is the tresillo), then edit by hand.
- 64 steps = 4 pages of 16; the page follows the playhead unless pinned.

## Step detail

| Detail | Values | How |
|---|---|---|
| Accent | soft, medium, hard | Hold a step and turn LEVEL, or right-click; drawn as fill brightness |
| Condition | chance 100, 75, 50, 25 %; *X of Y* bars (1 of 2, 2 of 4, 4 of 4…); Fill; Not fill | Hold a step and turn DECAY, or right-click; drawn as a small corner mark |
| Nudge | −50 % to +50 % of a step | Hold and turn SWING, or right-click |
| Roll | 1, 2, 3, 4 hits | Right-click; drawn as split cell |

Steps on different rows at the same position with chances adding up to 100 % form a group: exactly one of them plays (the Deluge's probability groups), which makes "kick or tom, never both" simple.

Engine: a step is a small packed value (on, accent 2 bits, condition 6 bits, nudge as i8, roll 2 bits), so a 16-row × 64-step groove stays a fixed-size value sent whole on each edit, as in 1.0.

## Sound knobs

LEVEL, PITCH, DECAY plus one named knob per row from `pack.json` (`"knob": {"name": "TONE", "param": "tone"}`). For the synthesised kits the parameter goes into the voice (kick sweep depth, snare noise mix "SNAPPY", hat band-pass "COLOR"); for sample rows it is a per-row low-pass. Knobs act on the selected row, as in 1.0; a controller with 16 encoders (Twister) gets one knob per row and parameter through `[GrooveN],rowR_knobK`.

## Make a beat without drawing one

- **Dice** (one button next to the groove menu): a new groove in the kit's style. Kits carry rules per row (where kicks may fall, how busy hats are) so the result fits the kit rather than being noise. A lock per row keeps it; a **Busy** slider sets how many hits.
- **Remix**: changes a few hits of the current groove and keeps its feel; pressing it again goes further. Dice and Remix keep three levels of undo, and Back to the kit's groove undoes all.
- Dice and Remix never touch the playing groove mid-bar: the new one starts on the next bar, so it can be done live.
- **Slop**: a groove-wide random timing spread up to ±10 ms, off by default.

## Fills and variation

- Fill is a step condition, as on the Deluge: steps marked *Fill* play only while FILL is on and steps marked *Not fill* stop. A fill is therefore part of the groove, not a second pattern to keep in step.
- FILL is held for a manual fill, or set to Auto, where it turns on for the last bar before the next phrase of the leading deck (or every 8 bars when there are no phrases).
- Variation: a groove can hold A/B versions; the switch alternates them every bar or every 2 bars.
- Built-in grooves gain fills, so FILL works out of the box.

## Melodic row

- One optional row of kind `melodic`: per step a note (scale degree and octave), length and slide.
- Steps can slide into the next note (a 303-style glide); the built-in synth gets a diode-ladder style filter for that sound.
- Notes are scale degrees, played in the key of the leading deck (from key detection), following its key changes on the bar. A fixed key can be set instead.
- Sound: a small mono synth in the engine (saw/square, filter, envelope, two knobs on the face: CUTOFF and RESONANCE as the row's knobs) or a hosted instrument plugin from `add-instrument-plugins`, which gets the notes with sample offsets.
- Editing: click a step to toggle, drag up and down for the degree; no piano roll.

## Record from pads

- REC arms recording into the selected groove; pad hits on the sampler or the Steps page land on the nearest step (or with nudge kept, an option), with accent from pad velocity.
- One bar count-in when the groove is stopped. Undo removes the last pass.

## Sounds from the mix

- Drop a file, a sampler pad, or a region selected on a track deck's waveform onto a row: it becomes the row's sound, trimmed to its first transient and saved as an override of the kit.
- With stems loaded, the row menu offers "Kick from deck 1" (and snare, hat): the engine finds the cleanest hit in the drum stem (onset with least overlap, by energy in the row's band) and cuts it.

## Replace drums

From the DDJ-GRV6's Groove Circuit idea (task 5.6 of `add-groove-box`):

- On a track deck with stems, "Replace drums" opens a Groove Box over the deck instead of in place of it. The drum stem is muted (and faded back on release), the groove plays in the track's grid on the track's channel.
- Drum roll and stutter act on the groove; releasing returns to the track's own drums on the next beat.
- This needs a deck that has both a track and a groove; the engine mixes the groove into the deck's buffer after the stems and before EQ.

## Kits and items

`pack.json` grows: up to 16 `rows`, per row `knob` and `kind` (`drum` or `melodic`), grooves with `fill` and `variation`, and `chains`. Users' grooves and chains are saved in the kit's overrides and can be saved as items shown in the Instruments source, dragged onto a Groove Box, and exported with the kit. A groove can be copied as a short text code (steps, conditions and kit name) to paste into a message and back into any DJ Sugar, as mpump shares beats as links. A groove or chain can also be dragged out as a standard MIDI file (General MIDI drum notes per row), as Triaz and Drum Designer do.

## Controllers

New `[GrooveN]` controls: `rowR_accentS`, `rowR_length`, `rowR_knobK`, `fill`, `variation`, `rec`, `chain_position`, `page` (0-3). The Steps pad page gets accent by velocity, rows 1-8 on Shift, and a two-row mode on 16-pad controllers (two rows of 8, or one row of 16).

## Order of work

1. Timeline in the waveform strip and chains (uses the free space, no engine changes beyond chain playback).
2. Step detail, conditions and per-row length, direction and Euclid (engine step format, menus).
2b. Dice, Remix and Slop.
3. More rows, sound knobs, kit format.
4. Fill and Not fill conditions, Auto fill, variation.
5. Record from pads.
6. Melodic row (after `add-instrument-plugins` for the plugin route; the built-in synth can come first).
7. Sounds from the mix and replace drums (need stems on a deck alongside a groove).

## Open questions

- Is the timeline the right use for the waveform strip, or should the grid itself grow into it?
- Melodic row: key-following by default, or a fixed key unless chosen?
- Eight rows on screen, or keep four with a row switch for beginners and eight in an "Advanced" view?
- Should 2.0 wait for v2.0, or ship in pieces (timeline and step detail could land in v0.6 or v0.7)?

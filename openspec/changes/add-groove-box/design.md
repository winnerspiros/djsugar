# Design

The full research (other apps, UI sketches, engine, controllers) is in the project notes, `notes/pattern-player-review.md` (2026-11-01). This file keeps the decisions.

## Research: what others do

| App / device | Shape | Taken |
|---|---|---|
| Traktor Pro 4 Pattern Player | An FX-unit mode on a deck: one sound at a time, 16 steps, kit of 8 sounds and 8 patterns, VOL, PREV/NEXT sound, PITCH, DECAY, DUCK; starts on the first beat, follows the master clock | The knob set, DUCK, kits with ready patterns, start on the beat. Not the FX-unit placement (no fader or EQ of its own, one sound at a time) |
| rekordbox sequencer | Records sampler pad hits into 1-4 bar loops, saved per bank | Later (task 5.2) |
| Novation Circuit Tracks | 4 drum + 2 synth tracks, 32 steps, velocity, probability, micro-timing, packs | Rows per sound, velocity, chance and nudge per step (hidden in a menu) |
| Ableton Move | 32 pads, 16 step buttons, 4 tracks, Link | Step row separate from the pads, knobs act on the selected sound, Link |

## Decisions

### Beginners first (2026-11-02)

- **One press.** A kit always has a groove loaded; PLAY starts it on the next bar at the right tempo. No tempo, clock source or quantize setting to understand.
- **Names, not numbers.** Rows are named after their sound, grooves after their style (*Four to the floor*, *Off-beat hats*, *Breakbeat*, *Half time*), kits after their character (*Boom*, *Punch*, *Tin*, *Dub*).
- **Hear what you add.** While stopped, a step click plays its sound. While playing, the change is heard the next time the playhead gets there.
- **Few controls on the face.** Steps, M/S, three knobs, SWING, A-D, PLAY. Velocity, chance and nudge only in a step's right-click menu. DUCK off until chosen.
- **Hard to break.** Back to the kit's groove undoes any edit; kit files never change; a new Groove Box starts with its fader as a new deck has it.
- **Tooltips everywhere** and a short part in the first-run tour.

### A deck, not an FX unit

The Groove Box replaces a deck's source, so its channel is a normal mixer channel. That gives EQ, filter, fader, crossfader, FX, cue and meters with no new mixer code, gives controllers something they already know how to drive (`[ChannelN],play`, `volume`, EQs), and keeps the 2-Deck and 4-Deck layouts' shapes. Any deck can be one; in 2-Deck it's usually deck 2, in 4-Deck an outer deck.

### Layout

2-Deck (deck row 272 px): the jog, tempo column and loop column give way to:

```
 GROOVE ▾  Boom ▾                       124.0 BPM · follows deck 1   ■□□□
 ┌────┐ KICK  ■ · · · ■ · · · ■ · · · ■ · · ·   M S
 │PLAY│ CLAP  · · · · ■ · · · · · · · ■ · · ·   M S
 ├────┤ HAT   · · ■ · · · ■ · · · ■ · · · ■ ·   M S
 │CUE │ PERC  · · · ■ · · · · · ■ · · · · · ·   M S
 └────┘ A B C D  Four to the floor ▾   ◯LEVEL ◯PITCH ◯DECAY  ◯SWING  DUCK ▾
```

Cells about 22 px; on screens too narrow for 16 the row shows 8 with the 1/2 page shifter the pads use. 4-Deck strips show the 4 × 16 grid in small cells where the pads and overview are, with LEVEL / PITCH / DECAY only.

### Engine

- **Clock.** One per Groove Box deck, advanced once per buffer after Link's `begin` and sync. Source: leading deck → Link session (`beat_at_time(heard_at)`) → own tempo. It keeps its own continuous beat count and corrects its rate toward the leader's phase (as SYNC does), snapping only when more than half a beat off, and then at the next bar. A hot cue or loop on the leader therefore never stutters the groove.
- **Steps.** 16ths: step k at beat k/4, plus swing on odd steps and nudge. For a buffer covering beats `[b0, b1)`, each step inside it plays at frame `round((t − b0) / beats_per_frame)`. That is exact to the sample at any buffer size and through tempo changes.
- **Voices.** One voice per row plus one fading tail (the sampler's 96-frame fade on a choke). Fractional read position, `rate = 2^(pitch/12) × file_rate / engine_rate`, linear interpolation first (cubic later, shared with the FX engine), exponential decay envelope, level glides of 5 ms. Patterns are small fixed-size values sent whole over the command queue on each edit and swapped at the next step, so nothing allocates on the audio thread.
- **Duck.** A one-pole gain envelope on the target deck's channel gain, triggered at the kick row's frame offsets.
- **Visual.** The clock's position joins `VisualPositions`, so the playhead moves with the audio like the waveforms.

### Kits and sounds

`pack.json` gains:

```json
{
  "kind": "kit",
  "name": "Boom",
  "author": "DJ Sugar",
  "licence": "GPL-3.0-or-later",
  "samples": [ { "builtin": 101, "name": "Kick" }, … up to 16 ],
  "rows": [0, 2, 3, 4],
  "grooves": [
    { "name": "Four to the floor", "steps": ["x...x...x...x...", "....x.......x...", "..x...x...x...x.", "...x......x....."] }
  ]
}
```

`x` is a hit, `.` a rest; `X` hard and `o` soft come with velocity. Edits are overrides in `settings.json`, as pad edits are.

### Classic drum machine sounds and licences (a user request for CC sounds from classic groove boxes, 2026-11-02)

DJ Sugar's builds are GPL-3.0 and published on GitHub, so anything bundled must allow redistribution, commercial use included. What was found:

| Source | Machines | Licence | Verdict |
|---|---|---|---|
| Synthesised by DJ Sugar | 808, 909, 606 and CR-78 style voices: kick as a pitch-swept sine with a click, snare as tones plus filtered noise, clap as noise bursts, hats as six detuned square waves through a band-pass (the 808's own method), toms, rim, cowbell, shaker | Ours, GPL | **Built in.** The analogue circuits are well documented, the sound is the classic character, there are no files and no licence questions. Extends the synthesised Kick/Snare/Clap/Hat already in `crates/engine/src/sampler.rs` |
| Freesound, filtered to CC0 | e.g. "TR-909 Drum Hits" by GRD-music- (made on a Roland JD-Xi), a "Roland TR606 sampleset", a "Roland TR08" set | CC0 as stated by the uploaders | **Get more Packs**, as a "Classic Machines" kit in the curated packs repository, after checking every sound's licence page and keeping a credits file |
| Michael Fischer's TR-808 set (1994, machines.hyperreal.org; repackaged by fluid-music as "no licensing restrictions") | TR-808, 116 samples | Called "ABSOLUTELY FREE!" with no licence text | Only with written permission from the author |
| Autodafe XoX Drums | 606, 707, 808, 909 | Article says the source is public domain, sources not credited | No, origin unclear |
| SampleRadar (MusicRadar) 808 and drum machine packs | Many | Free to use in music, "don't re-distribute" | No |
| TidalCycles Dirt-Samples | 808, 909 and more | No licence stated | No |

Kits are named by style, never with Roland, Linn or model names; "808-style" may appear in a description. Users can still import any samples they own into their own kits.

## Open questions

- Should a Groove Box be allowed on every deck, or only deck 2 in 2-Deck and decks 3-4 in 4-Deck? Every deck is simplest to explain; Automix and sync skip it either way.
- Four rows, or a "Simple" view with one row at a time like Traktor's? Four rows are proposed; one-row view only if testers find four too busy.

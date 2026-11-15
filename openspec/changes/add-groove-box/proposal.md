# Proposal

Planned for: **v0.5 · Stems, performance and switching over** (moved from v1.x, 2026-11-02, after the pattern player review in the project notes, `notes/pattern-player-review.md`)

## Why

Traktor Pro 4 has a Pattern Player, rekordbox a sampler sequencer, and hardware like Novation Circuit Tracks and Ableton Move make it easy to lay a beat under a mix. DJ Sugar has a sampler with packs but nothing that plays a rhythm by itself. The goal is a simple groove box that can take the place of a deck, that a beginner can start in one press and stay in time with the mix without thinking about it.

## What Changes

- **Groove Box as a deck.** Any deck can become a Groove Box from its header menu (an empty deck; a loaded one asks to eject first). Its mixer channel stays a normal channel: EQ, filter, fader, crossfader side, deck FX, headphone cue and meters all work. The jog and waveform area shows the groove box instead.
- **Ready to play.** Every kit comes with named grooves (for example *Four to the floor*, *Off-beat hats*, *Breakbeat*, *Half time*), and the first one is loaded. Pressing PLAY starts it on the next bar of the playing deck, at its tempo. There is no tempo to set and no clock to choose.
- **Four rows of 16 steps**, one per sound, named after the sound (Kick, Snare, Hat, Clap…), with mute (M) and solo (S) per row. Click a step to turn it on or off; while stopped, a click also plays the sound so you hear what you add.
- **Three knobs** for the selected row (LEVEL, PITCH, DECAY) and SWING for the whole groove. More (step velocity, chance, nudge) only in a step's right-click menu.
- **Four grooves per deck (A-D)**, switched on the next bar; copy, clear and **Back to the kit's groove**.
- **DUCK**: the music on another deck dips on every kick, as Traktor's Pattern Player does. Off until chosen.
- **Kits** are sample packs with grooves (same `pack.json`, `.djsugarpack` export and import, Get more Packs). Built in: synthesised kits in the style of classic drum machines, made by DJ Sugar itself, so they need no sample files and no licences.
- **Always in time.** The groove follows the leading deck (the one SYNC follows), or Ableton Link when no deck plays, or keeps its own tempo when nothing does. Every step plays on the exact sample it falls on, whatever the audio buffer size.
- **Controllers.** PLAY, EQ, filter and fader work through the deck's usual controls. New `[GrooveN]` controls for steps, rows, grooves and knobs; a **Steps** pad page shows 8 steps of the selected row on any pad controller, with the page shifter for steps 9-16.

## Capabilities

### New Capabilities

- `groove-box`: a step-sequencer deck that plays kits in time with the mix.

### Modified Capabilities

None. The Steps pad page and kits-as-packs are part of `groove-box`.

## Out of Scope

- Melodic synths and piano rolls (Move's Drift and Wavetable, Circuit's synth tracks). A melodic row (pitch per step) can come later.
- Recording what you play on the sampler pads into a groove (rekordbox's sequencer). A later step; it shares the clock and the steps.
- More than 32 steps, song mode and pattern chains beyond A-D.
- Sending MIDI clock or notes out to other gear (Link covers sync with other apps and with Move).
- Bundling sample files of real Roland or Linn machines whose licence doesn't allow it (see design.md).

## Impact

engine (a deck source that is a groove instead of a track, a groove clock that follows the leader or Link, sample-accurate step triggers, voices with pitch and decay, ducking), app (Groove Box deck UI in 2-Deck and 4-Deck, kits in the pack store, settings), controllers (`[GrooveN]` controls, Steps pad page, FLX4 / Twister / Xone K2 mappings), user guide (new Groove Box page), supported-controls.txt.

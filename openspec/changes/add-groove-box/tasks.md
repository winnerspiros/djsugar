# Tasks

## 1. Engine: clock and voices

- [x] 1.1 Deck source is a track or a groove; a Groove Box deck renders into the deck's channel before EQ, so the channel chain is unchanged
- [x] 1.2 Groove clock: follows the leading deck, then Link, then its own tempo; phase-locks rather than copying the leader's position, moves on a leader's jump only at the next bar
- [ ] 1.3 Sample-accurate steps: every step's frame offset within the buffer, swing, nudge and chance; start on the next bar (done except nudge and chance)
- [ ] 1.4 Voices: one per row plus a fading tail (a new hit chokes the last), pitch by resampling, decay envelope, velocity and level with 5 ms glides; nothing allocated on the audio thread (done except velocity and glides)
- [x] 1.5 DUCK on the chosen deck's channel gain
- [x] 1.6 `[GrooveN]` controls; regenerate website/book/supported-controls.txt
- [ ] 1.7 Tests: the same hits on the same frames at buffer sizes 64, 333 and 2048; hits within one frame of the grid through a tempo ramp; a beat jump on the leader moves the groove at the next bar; no allocation in `process` (done except the allocation test)

## 2. Kits

- [ ] 2.1 `pack.json` kits: `"kind": "kit"`, rows, named grooves as step strings
- [x] 2.2 Four synthesised built-in kits in classic drum machine styles, each with at least four grooves (see design.md)
- [ ] 2.3 Kit edits as overrides in settings, Back to the kit's groove; import and export with `.djsugarpack`

## 3. 2-Deck Groove Box

- [x] 3.1 Groove Box / Track deck in the deck header (a button on a stopped deck), kept across restarts; eject first on a loaded deck
- [x] 3.6 Instruments source in the library rail replaces the header buttons: Groove Box kits, instrument plugins and the Mic and Aux inputs dragged onto a deck (`[ChannelN],live_input` plays an input through the deck's channel, one deck per input); a track or EJECT makes it a track deck again
- [ ] 3.2 Rows × 16 steps with names, M and S, playhead and shaded beats; click to toggle and preview; right-click step menu (done except the step menu)
- [ ] 3.3 LEVEL, PITCH, DECAY for the selected row, SWING; grooves A-D switched on the bar; kit menu with author and licence; DUCK menu (done except author and licence, and the DUCK menu: the `duck_deck` control works)
- [ ] 3.4 Tooltips on every control and a Groove Box part in the first-run tour (Help › Tour)
- [x] 3.5 Update the user guide: new website/book/src/guide/groove-box.md, linked from decks.md and layouts.md

## 4. 4-Deck and pads

- [ ] 4.1 Compact Groove Box in a 4-Deck strip
- [ ] 4.2 Steps pad page with the page shifter; Shift selects row and groove
- [ ] 4.3 DDJ-FLX4 Steps page, Midi Fighter Twister and Xone K2 example mappings; controller mapper function group "Groove Box"

## 5. Later

Planned in detail for v2.0 in [add-groove-box-2](../add-groove-box-2/proposal.md) (5.1 as per-row length up to 64 steps, 5.2 as record from pads, 5.4 as the melodic row, 5.5 as rows and sound knobs, 5.6 as Replace drums); 5.3 stays here.

- [ ] 5.1 32 steps (two pages of 16)
- [ ] 5.2 Record what you play on the sampler pads into the groove
- [ ] 5.3 Classic Machines kit pack in Get more Packs (CC0 recordings checked file by file, see design.md)
- [ ] 5.4 A melodic row
- [x] 5.5 Accent per step and more rows per kit, in the style of rhythm composers with one lane per instrument (soft and accented steps, nine rows: Kick, Snare, Hat, Open hat, Clap, Rim, Cowbell, Crash, Ride)
- [x] 5.7 Plaits drum models for the Analog and Synthetic kits; a groove library by genre read from text files in crates/engine/data/grooves, with a chain of two grooves that take turns a bar each
- [ ] 5.6 Replace drums: a Groove Box over a track deck with that track's drum stem muted, plus drum roll and stutter on the groove

## Workflow follow-up

- Archive the change once its last PR is merged, so the main specs take its requirements.

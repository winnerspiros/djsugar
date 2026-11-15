# Proposal

Planned for: **v2.0**, after `add-groove-box` (v0.5/v0.6) and `add-instrument-plugins`

## Why

The first Groove Box (`add-groove-box`) is a beginner-first drum machine on a deck: four rows of 16 steps, three knobs, grooves A to D. It plays in time with the mix, but it stays a small loop under a track. The aim for 2.0 is an instrument a DJ can perform a whole section with, somewhere between Novation Circuit Tracks, Ableton Move and a rhythm composer with one lane per instrument, without losing the one-press start of the first version.

A Groove Box deck has no audio file, so it never has a waveform. The strip that a track deck uses for its waveform is free, and it is the widest space on the deck.

## What Changes

- **Bar timeline in the waveform strip.** Grooves laid out as blocks along the bars (a chain), with a playhead that scrolls at the same zoom and on the same beat as the other decks' waveforms, so a groove change lines up with a drop in the track it plays against. Hits show as ticks per row, so the strip still reads like a waveform at a glance. Drag grooves onto it to build a chain; with no chain the selected groove simply repeats, as today.
- **Eight grooves (A to H)** and chains of up to 64 bars per Groove Box.
- **More rows.** Up to 8 rows on screen (16 sounds per kit), scrolling in 4-row views where space is short.
- **Per-step detail, still out of the way:** accent (soft, medium, hard), nudge, rolls (2, 3 or 4 hits in one step) and a condition: a chance, every *X of Y* bars, or only with FILL on or off. Set by holding a step or from its right-click menu.
- **Per-row length** (1 to 64 steps, 4 pages of 16), so rows can run against each other, per-row step size (16th, 8th, 16th triplet) and direction (forward, reverse, ping-pong).
- **Make a beat without drawing one.** A dice button that makes a new groove in the kit's style, with a lock per row and a Busy slider; Remix that varies the current groove while keeping its feel; and Euclid on a row (spread N hits over its length).
- **Slop:** a small random timing spread for a played-by-hand feel.
- **Knobs per sound.** Every row keeps LEVEL, PITCH, DECAY and gains a fourth knob that the kit names (TONE, SNAPPY, COLOR…), which for the synthesised kits drives the synth voice itself.
- **Fills and variation.** Steps marked *Fill* play only while FILL is on, and steps marked *Not fill* stop; FILL can turn itself on for the last bar before the leading deck's next phrase. A Variation switch alternates two versions of a groove.
- **Melodic row.** A bass or chord row that plays notes per step from a small built-in synth or a hosted instrument plugin, in the key of the leading deck, so it fits the track it plays under.
- **Record from pads.** What is played on the sampler pads or a controller's pads while recording is armed lands in the groove, quantised to the step.
- **Sounds from the mix.** Drop a track region, a sampler pad or a file on a row to use it as that row's sound; with stems, take the kick or snare from the drum stem of a loaded track.
- **Replace drums.** A Groove Box can sit over a track deck: it mutes the track's drum stem and plays its own groove in the track's grid, with roll and stutter on the groove.
- **Grooves as items.** Grooves and chains can be saved, named and dragged from the Instruments source like kits, and exported in `.djsugarpack`, or dragged out as a MIDI file.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `groove-box`: timeline and chains, more rows and grooves, step detail, per-row length, sound knobs, fills, melodic row, recording, sounds from the mix, replace drums.

## Out of Scope

- A full piano roll, polyphonic synth tracks and synth editing beyond the row's four knobs (instrument plugins cover that).
- Sending MIDI notes or MIDI clock out (Link already covers syncing other apps and gear; can come with the MIDI clock work).
- Bouncing a groove to an audio file (recording the mix already captures it).
- Real-machine sample sets whose licence doesn't allow redistribution (see `add-groove-box/design.md`).

## Impact

engine (more voices and rows, per-row length and step size, rolls, accents, chain playback, synth voice parameters, a melodic voice and a note route to hosted instruments, stem muting on a track deck under a groove), app (timeline in the waveform strip, step menus, row knobs, FILL and Variation, recording, drop targets, Instruments items), kits (`pack.json` rows, knobs, fills, variations), controllers (`[GrooveN]` additions, Steps page with accents and rows), user guide.

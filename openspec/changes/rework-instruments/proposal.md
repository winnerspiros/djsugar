# Proposal

Planned for: **v0.6**, after the Instruments source (`#285`) and plugin hosting (`add-audio-plugin-hosting`, `add-instrument-plugins`)

## Why

The Instruments source (v0.5) shows kits, plugins and inputs as tiles on one scrolling page. It reads well with a handful of items and stops working with many instruments and their presets. A deck playing an instrument still shows a track deck's pads, loop, PLAY and CUE, none of which do anything for it, and nothing to play or shape the sound with. Tweaks made in a plugin's window are lost on restart, because DJ Sugar doesn't save plugin state, so there is no way to keep a sound and find it again. Some gear comes in very low (a hardware synth on an interface input with no gain knob) and nothing on the deck brings it up or shows its level against clipping.

## What Changes

- **Instruments browser like Collection.** A tree on the left (Recently played, My presets, Groove Box, Plugins by maker, Inputs) and a list like tracks on the right. A folder lists instruments; an instrument lists its presets, its default sound first. Type and Character tag chips over the list. One **Groove Box** entry replaces the six kit tiles; the kit is picked on the deck. Rows drag onto decks or load on double-click, as tracks do.
- **DJ Sugar presets.** One file per preset under the library folder (`Instruments/Presets/<instrument>/`): the instrument, its full state, knob pages, trim, pan, scale settings, name, tags and art. Groove Box presets keep kit, patterns, swing and per-track sound settings. SAVE on the deck (Save, Save as new, Rename, Tags). A plugin's own presets are listed too where the format allows (CLAP preset discovery, `.vstpreset`, AU factory presets and `.aupreset`).
- **Plugin state save and restore**, the base for presets; it also keeps plugin tweaks across restarts.
- **Instrument deck.** In place of pads, loop, PLAY and CUE: artwork behind the header, preset name with ‹ › to step through the instrument's presets, a scope where the overview was, EDIT and SAVE, 8 knobs in pages (the plugin's own pages, else its first 8 parameters, else the user's picks), **TRIM** (−12 to +24 dB, set on first play) with a level meter, **PAN**, an always-on safety limiter (no readout), and an on-screen keyboard that lights the notes played. The channel's EQ, filter and fader stay in the mixer. Instrument decks are not on the crossfader by default (Thru); the 4-Deck bar's chooser still changes it.
- **Scale, chord and arpeggiator.** Scale mode (Off, Auto, or a picked root and scale; Auto follows the master deck's key): played notes move to the nearest scale note, and the keyboard lights playable notes and dims the rest. Scales offered: Major, Minor, Major pentatonic, Minor pentatonic; more behind a switch later. A simple chord mode (one key plays a chord built on the scale) and an arpeggiator synced to the mix's beat, modelled on what Arturia's KeyStep Pro and KeyLab mk3 offer.
- **MIDI from** on each instrument deck: a device and channel (Any by default), so a controller with several tracks on different channels (a KeyStep Pro on channels 1-4) can play a different instrument on each deck. It builds on the MIDI device roles' deck choice.
- **4-Deck strip** for instrument decks: art, name, preset ‹ ›, thin scope, 4 knobs (half a page), trim and meter, EDIT; no keyboard.
- **Inputs** (mic, aux) on a deck get the same header naming their interface input, with trim, pan, meter and the safety limiter; no knobs or keyboard.
- **Artwork** for each instrument and preset: the user's own image, then NKS artwork when installed, then a snapshot of the plugin's window, then the plugin bundle's icon, then one consistent generated fallback.

## Out of scope

- Clips and a clip launcher (assessed 2026-11-07: a phrase looper may come later as its own change).
- Macros that move several parameters at once, MPE, and controller following of knob pages.
- Groove Box sequencer changes (see `add-groove-box-2`).
- Scales beyond major, minor and the two pentatonics.

## Capabilities

### New Capabilities

- `instruments`: browsing instruments and presets, instrument and input decks (with trim, pan and the safety limiter), presets, scale/chord/arp, artwork.

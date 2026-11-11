# Denon LC6000 Prime for Mixxx

Custom Mixxx controller mapping for the Denon LC6000 Prime.

This repository provides:

- two Mixxx presets:
  - `Denon-LC6000-Prime.midi.xml`
  - `Denon-LC6000-Prime-Display.midi.xml`
- two script files:
  - `Denon-LC6000-Prime-scripts.js`
  - `Denon-LC6000-Prime-Display-scripts.js`

The PR preset exposes per-controller settings for deck assignment, deck color, jog sensitivity, and vinyl mode. The WD preset exposes deck assignment, deck color, and display brightness.

## Features

- Transport, cue, sync, master, key lock, slip
- Jog wheel with touch scratching
- 14-bit tempo fader
- Pitch bend buttons with range switching on `Shift`
- Browse encoder and track loading
- Hold `Select` and turn to zoom the waveform
- Hot Cue mode
- Roll mode
- Auto Loop mode
- Needle search strip
- Button, pad, and platter ring feedback
- Optional second WD preset for center-display ring, slip ring, and loop text

## Installation

1. Copy these files into your Mixxx controllers directory.

Common locations:

```text
~/.mixxx/controllers/
%LOCALAPPDATA%\Mixxx\controllers\
~/Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx/controllers/
```

Copy:

- `Denon-LC6000-Prime.midi.xml`
- `Denon-LC6000-Prime-scripts.js`
- `Denon-LC6000-Prime-Display.midi.xml`
- `Denon-LC6000-Prime-Display-scripts.js`

2. Start Mixxx.

3. Open `Preferences` -> `Controllers`.

4. For each connected LC6000 `PR` port, select the preset `Denon LC6000 Prime`.

5. Optional: for each connected LC6000 `WD` port, select the preset `Denon LC6000 Prime Display`.

6. Configure the controller settings for each unit if needed:

- `Deck assignment`: `Deck 1` to `Deck 4`
- `Deck color`: `Deck Default`, `Green`, `Blue`, `Orange`, `Purple`
- `Jog sensitivity, Vinyl-Mode`
- `Jog sensitivity, Non-Vinyl-Mode`
- `Start in vinyl mode`

7. If you use the display preset on `WD`, configure the same `Deck assignment` there as on the `PR` preset. Otherwise controls and display will point to different Mixxx decks.

## Multi-Controller Setup

Use one PR preset per physical LC6000. Add one WD preset per physical LC6000 if you also want center-display feedback.

Example for a two-deck setup:

- LC6000 #1 PR -> preset `Denon LC6000 Prime`, setting `Deck assignment = Deck 1`
- LC6000 #1 WD -> preset `Denon LC6000 Prime Display`, setting `Deck assignment = Deck 1`
- LC6000 #2 PR -> preset `Denon LC6000 Prime`, setting `Deck assignment = Deck 2`
- LC6000 #2 WD -> preset `Denon LC6000 Prime Display`, setting `Deck assignment = Deck 2`

Do not assign the same deck to two devices.

On the tested Linux setup the ports appear as:

- `Denon DJ LC6000 PRIME LC6000 PR`
- `Denon DJ LC6000 PRIME WD MIDI 1`

## Controls

### Transport

- `Play`: play/pause
- `Shift + Play`: stutter from cue
- `Cue`: standard cue behavior
- `Censor/Reverse`: reverse only while held
- `Shift + Censor/Reverse`: momentary censor
- `Sync`: beat sync
- `Shift + Sync`: disable sync
- `Master`: toggle sync leader
- `Key Lock`: toggle key lock
- `Shift + Key Lock`: reset key
- `Slip`: toggle slip mode
- `Pitch Bend - / +`: temporary pitch bend
- `Shift + Pitch Bend - / +`: change tempo range

### Browse and Loading

- `Back / Forward`: move library focus
- `Select turn`: move up/down in the library
- `Select press`: load selected track to the assigned deck
- `Shift + Select press`: load selected track to `PreviewDeck1` and play
- `Hold Select + turn`: waveform zoom
- `Shift + Select turn`: waveform zoom

### Track Navigation

- `Track Skip Prev / Next`: move in the library playlist
- `Shift + Track Skip Prev / Next`: continuous track search backward/forward
- `Beat Jump < / >`: beat jump backward/forward
- `Shift + Beat Jump < / >`: change beat jump size

### Loop Controls

- `Loop In`: set loop in point
- `Loop Out`: set loop out point
- `Auto Loop press`: enable loop or exit reloop
- `Auto Loop encoder`:
  - loop active: halve/double loop size
  - no loop active: change beatloop size
  - `Shift` while loop is active: move the loop
- `Parameter < / >`:
  - in loop modes: change loop size
  - in other modes: change beat jump size

### Performance Modes

- `Hot Cue`: selects Hot Cue pad mode
- `Loop`: selects Auto Loop mode
- `Roll`: selects Roll mode
- `Slicer`: selects Slicer mode
- Pressing an already selected mode button does nothing

### Pads

### Hot Cue Mode

- `Pad 1-8`: trigger Hot Cues 1-8
- `Shift + Pad 1-8`: clear Hot Cues 1-8

### Roll Mode

- `Pad 1`: `1/8`
- `Pad 2`: `1/6`
- `Pad 3`: `1/4`
- `Pad 4`: `1/3`
- `Pad 5`: `1/2`
- `Pad 6`: `2/3`
- `Pad 7`: `1`
- `Pad 8`: `2`

### Auto Loop Mode

- `Pad 1`: `1/8`
- `Pad 2`: `1/4`
- `Pad 3`: `1/2`
- `Pad 4`: `1`
- `Pad 5`: `2`
- `Pad 6`: `4`
- `Pad 7`: `8`
- `Pad 8`: `16`

### Manual Loop and Slicer Modes

- `Slicer` mode can be selected directly.
- Full native pad behavior for Manual Loop and Slicer is not implemented yet.

### Jog Wheel and Needle Strip

- `Platter touch`: enables scratching when vinyl mode is on
- `Jog wheel`: scratch or nudge depending on touch/vinyl state
- `Shift + Jog`: faster seek/nudge
- `Vinyl`: toggle vinyl mode
- `Needle touch + strip`: absolute track position seek

### LEDs

- Main transport and mode LEDs follow Mixxx deck state
- Pad LEDs reflect the current pad mode
- The outer platter ring:
  - off while stopped
  - deck color while playing
  - blinking near the end of the track

## WD Display Preset

The optional `Denon LC6000 Prime Display` preset is meant for the separate `WD` MIDI device. It currently drives:

- outer display platter position
- slip ring visibility
- loop size text
- display brightness and default colors

It does not yet upload artwork or logos.

## Notes

- Deck assignment is handled by the selected preset file, not by an on-device deck assignment mode.
- Mixxx still treats `PR` and `WD` as separate assigned devices. The display works by giving the WD port its own preset, not by discovering the second port from inside one script.
- The mapping intentionally uses `Censor/Reverse` as `Reverse` on normal press and `Censor` on `Shift`.

## Attribution

This mapping was created with AI assistance using OpenAI Codex.

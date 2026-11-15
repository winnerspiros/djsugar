# Tasks

## 1. Browser

- [x] 1.1 Instruments tree (All instruments, Groove Box, Plugins by maker, Inputs) and a track-style list with search; rows drag to a deck, double-click loads on a free deck
- [x] 1.2 One Groove Box entry; kit picked on the deck
- [x] 1.3 Type and Character tag chips over the list; tags on presets (SAVE › Tags)

## 2. Presets

- [x] 2.1 Plugin state save and restore (CLAP state, VST3 component and controller state, AU full state) in the plugin host; kept across restarts
- [x] 2.2 DJ Sugar preset files (`Instruments/Presets/<instrument>/`) for plugins and the Groove Box; SAVE menu on the deck (Save, Save as new, Rename)
- [x] 2.3 Plugin presets listed from the formats: CLAP preset discovery, `.vstpreset`, AU factory presets and `.aupreset`
- [x] 2.5 Recently played in the tree; column sort (My presets and presets under their instrument, default sound first, are in)
- [x] 2.4 ‹ › on the deck steps through the instrument's presets in the browser's order

## 3. Instrument deck

- [x] 3.1 Live channels: TRIM (−12 to +24 dB), PAN and an always-on true-peak safety limiter at −1 dBTP for every deck playing an instrument or input; Thru on the crossfader by default
- [x] 3.2 2-Deck instrument deck: header with the art box (artwork itself in 5), preset ‹ ›, scope, EDIT, SAVE, 8 knobs in pages, TRIM, PAN, meter, keyboard lighting played notes
- [x] 3.3 Knob pages from the plugin's first 32 parameters, eight to a page; the knobs follow the plugin's own changes (its window, a loaded preset)
- [x] 3.8 Knob pages from CLAP remote controls when the plugin has them; the user's picks (right-click Assign…), saved in the preset
- [x] 3.4 4-Deck instrument strip: art, preset ‹ ›, thin scope, 4 knobs, TRIM, meter, EDIT; no keyboard
- [x] 3.5 Input decks (mic, aux): header with the interface input, TRIM, PAN, meter
- [x] 3.7 TRIM set on first play from the input's level
- [x] 3.6 MIDI from: the instrument takes notes from a chosen keyboard and channel (Any by default), so one track of a sequencer, or one keyboard of several, plays it; note-offs always pass so nothing hangs
- [x] 3.9 Deck clean-up: real piano keys, MIDI from behind the keyboard button in the deck's top bar, the plugin's format as a chip there, a smaller scope, same-size knobs with white markers, icons on Scale, Chord and Arp, a small OPEN

MIDI from per deck (one controller's tracks playing different decks) waits until several instruments play at once; it moved to add-instrument-per-deck (v0.7).

## 4. Scale, chord and arpeggiator

- [x] 4.1 Scale mode: Off, Auto (master deck's key), or a picked root and scale (Major, Minor, Major pentatonic, Minor pentatonic); notes move to the nearest scale note; keyboard lights playable notes and dims the rest
- [x] 4.2 Chord mode: one key plays a chord built on the scale (Triad, Seventh, Power, or a chord learned by holding it)
- [x] 4.3 Arpeggiator synced to the mix's beat (Up, Down, Up-down, Random, Order; rate, gate, octaves, hold)

## 5. Artwork

- [x] 5.1 Own image dropped on the deck's cover; generated fallback in one consistent style
- [x] 5.2 NKS artwork when installed, plugin bundle icon (a snapshot of the plugin's window was dropped)

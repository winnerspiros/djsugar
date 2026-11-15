## ADDED Requirements

### Requirement: Instruments browser
The Instruments source SHALL show a tree (Recently played, My presets, Groove Box, Plugins grouped by maker, Inputs) beside a list like the track list. Selecting a folder SHALL list its instruments; selecting an instrument SHALL list its presets with its default sound first. The list SHALL offer search, column sort, and Type and Character tag chips. A row SHALL load onto a deck by dragging it there or by double-clicking it (focused deck). The Groove Box SHALL appear once; its kit SHALL be picked on the deck.

#### Scenario: Find a saved bass
- **WHEN** the user selects Plugins › Arturia › Acid V and the Bass chip
- **THEN** the list shows only Acid V's presets tagged Bass, and dragging one onto deck 2 plays Acid V with that sound through deck 2

### Requirement: Presets
DJ Sugar SHALL save an instrument's sound as a preset file holding the instrument, its full state, knob pages, trim, pan, scale settings, name, tags and art. A Groove Box preset SHALL hold its kit, patterns, swing and track sound settings. Loading a preset SHALL bring the instrument back exactly as saved. A plugin's own presets SHALL be listed where its format lets a host read them.

#### Scenario: Keep a tweaked sound
- **WHEN** the user changes Jun-6 V's sound in its window, presses SAVE › Save as new and names it "Ambient Ebow", then restarts DJ Sugar and loads "Ambient Ebow"
- **THEN** Jun-6 V sounds as it did when saved

#### Scenario: Tweaks survive a restart
- **WHEN** the user changes a plugin's sound in its window, closes the window and restarts DJ Sugar
- **THEN** the plugin loads with the changed sound

#### Scenario: Step through presets on the deck
- **WHEN** a deck plays Acid V with its 4th preset and › is pressed
- **THEN** the 5th preset in the browser's order loads on that deck

### Requirement: Instrument deck
A deck playing an instrument plugin SHALL replace pads, loop, PLAY and CUE with the instrument's artwork, preset name with ‹ ›, a scope, EDIT, SAVE, 8 knobs in pages, TRIM, PAN, a level meter and an on-screen keyboard that lights the notes being played. In the 4-Deck layout it SHALL show 4 knobs and no keyboard. Instrument and input decks SHALL default to Thru on the crossfader.

#### Scenario: Knob pages from the plugin
- **WHEN** a CLAP plugin that publishes remote-control pages is loaded on a deck
- **THEN** the deck's 8 knobs show the plugin's first page, named as the plugin names them

### Requirement: MIDI from
Each instrument deck SHALL take notes from a chosen MIDI device and channel, Any by default.

#### Scenario: One controller, two instruments
- **WHEN** deck 3 plays a bass taking MIDI from KeyStep Pro channel 1 and deck 4 plays a pad taking channel 2
- **THEN** the KeyStep Pro's track 1 plays only the bass and track 2 only the pad

### Requirement: Level and clip protection
Every deck playing an instrument or input SHALL have TRIM from −12 to +24 dB, set on first play so peaks sit near −12 dBFS, and an always-on true-peak limiter at −1 dBTP before the channel's EQ and effects.

#### Scenario: A quiet hardware synth
- **WHEN** a synth on Aux 2 peaks at −30 dBFS and is dragged onto deck 3 and played
- **THEN** deck 3's TRIM rises so its peaks sit near −12 dBFS, and turning TRIM to +24 dB never takes the channel over −1 dBTP

### Requirement: Scale, chord and arpeggiator
An instrument deck SHALL have a Scale mode: Off, Auto (the master deck's key) or a picked root and scale from Major, Minor, Major pentatonic and Minor pentatonic. With a scale on, each played note SHALL move to the nearest note of the scale and the keyboard SHALL light playable notes and dim the others. A chord mode SHALL play a chord built on the scale from one key, and an arpeggiator SHALL play held notes in time with the mix's beat.

#### Scenario: Auto follows the playing track
- **WHEN** Scale is Auto and the master deck plays a track in A minor, and the user plays G#
- **THEN** the instrument plays the nearest A minor note and the keyboard lights the A minor notes

### Requirement: Instrument artwork
Each instrument and preset SHALL show artwork from, in order: an image the user dropped on it, NKS artwork installed with the plugin, a snapshot of the plugin's window, the plugin bundle's icon, or a generated image in one consistent style.

#### Scenario: Nothing installed
- **WHEN** a plugin has no image of its own and its window was never opened
- **THEN** its deck and rows show the generated image for its kind

## ADDED Requirements

### Requirement: Several instruments at once
Each deck SHALL be able to play its own instrument plugin at the same time as instruments on other decks, each in its own helper process with its own notes, MIDI from, scale, chord, arpeggiator, knobs and preset. Loading an instrument on one deck SHALL NOT change the instrument on another deck.

#### Scenario: Bass and pad
- **WHEN** deck 3 plays a bass synth taking MIDI from KeyStep Pro channel 1 and a pad synth is dragged onto deck 4 taking channel 2
- **THEN** both play at once, channel 1 plays only the bass and channel 2 only the pad

### Requirement: Plugins side by side
Within each audio buffer DJ Sugar SHALL hand every plugin that can run independently its block before waiting for any of them, so plugins on different decks and the instruments do their work at the same time rather than one after another. A plugin that misses its time SHALL play dry (effects) or silent (instruments) for that buffer, as before.

#### Scenario: Two heavy effects
- **WHEN** deck 1 and deck 2 each run a plugin effect that needs 1.5 ms of a 5.3 ms buffer
- **THEN** the buffer waits about 1.5 ms for plugins, not 3 ms

### Requirement: Idle instruments sleep
An instrument that has been silent for one second, holds no notes, receives none and is on no deck SHALL get no audio blocks until a note arrives, one of its knobs moves or it is put on a deck; it SHALL play in the same buffer that wakes it.

#### Scenario: A note wakes it at once
- **WHEN** an instrument has slept for a minute and a key is pressed
- **THEN** the note sounds in the next buffer, with no extra delay

## ADDED Requirements

### Requirement: Devices that play instruments
A MIDI device SHALL play the Instrument input only when it runs no mapping and is not off, not one of the computer's own ports, not a keyboard's DAW or Mackie Control port and not the MIDI side of an HID controller running its HID mapping, or when the DJ picks **Play instruments** in its mapping menu. A device running a mapping SHALL never reach the instrument. Settings › Controllers SHALL say "Plays instruments" under each device that does, and the instrument deck SHALL name the keyboard that played it last.

#### Scenario: DJ controller pressed
- **WHEN** the Xone K2 runs its mapping and the DJ presses its buttons while an instrument is on a deck
- **THEN** only the K2's mapped controls move, and the instrument plays nothing

#### Scenario: DAW port left out
- **WHEN** a keyboard with a MIDI port and a DAW port is plugged in
- **THEN** its MIDI port plays the instrument and its DAW port does not, until the DJ picks Play instruments for it

#### Scenario: A keyboard plays for the first time
- **WHEN** a keyboard plays the instrument for the first time since DJ Sugar started
- **THEN** a notice names it, and the instrument deck says it is playing from it

### Requirement: The deck a device plays
Each device in Settings › Controllers SHALL have a Deck menu: **Auto**, **As mapped**, **Follow instrument** and **Deck 1** to **Deck 4**, kept by device. Following, the device's deck 1 SHALL be the deck holding the instrument plugin, else the first Groove Box, and its other decks the ones after it; when the instrument moves to another deck the device SHALL move with it, lights included. Auto SHALL follow for devices that play instruments and be as mapped for the rest. A keyboard pinned to a deck SHALL play the instrument only while the instrument is on that deck, and its note offs SHALL always reach it.

#### Scenario: Instrument dragged to another deck
- **WHEN** a pad controller is set to Follow instrument and the DJ drags the instrument from deck 2 to deck 4
- **THEN** its PLAY and pads act on deck 4

#### Scenario: Pinned keyboard
- **WHEN** a keyboard is set to Deck 3 and the instrument is on deck 1
- **THEN** its notes don't play the instrument until the instrument is on deck 3

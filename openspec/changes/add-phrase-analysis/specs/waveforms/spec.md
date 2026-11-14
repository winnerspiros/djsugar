## ADDED Requirements

### Requirement: Phrase bands
Each deck's overview SHALL show, from top to bottom and without overlapping, the waveform, a dot at every minute and a phrase bar with one colour per kind (intro and outro #245A78, build #9472D5, drop #CA4B68, chorus #48C5A1, break #E99B68) and no text. The large scrolling waveforms SHALL flag each phrase start with a line in the phrase's colour and a tab naming it under the bar numbers, and nothing between flags; where a waveform is too short for tabs (4-Deck), only the lines. A live deck's scrolling waveform SHALL show the leading deck's phrases, moved onto its beats, as flags and a thin strip along its top. Phrases in the waveforms button's drop-down SHALL show or hide the overviews' bars and the scrolling waveforms' flags (Off, Overview, Waveform or Both) for all decks, remembered between runs. The deck SHALL show the current phrase and the bars left in it.

#### Scenario: Bands
- **WHEN** a track with phrases loads
- **THEN** the overview shows its phrases as bands

#### Scenario: Hidden
- **WHEN** the DJ picks Phrases › Waveform
- **THEN** no overview shows phrase bars, also after a restart, and the waveform takes their room

#### Scenario: Flags
- **WHEN** a drop starts within the view of a deck's scrolling waveform
- **THEN** a rose line marks its first bar with a DROP tab under the bar numbers

#### Scenario: Live deck
- **WHEN** a live deck plays while a track with phrases leads the mix
- **THEN** its scrolling waveform shows the track's phrases on the shared beats, the coming ones ahead of the play line

#### Scenario: Flags hidden
- **WHEN** the DJ picks Phrases › Overview
- **THEN** no scrolling waveform shows flags, also after a restart, while the overviews keep their phrase bars

#### Scenario: Countdown
- **WHEN** a deck plays 8 bars before the end of a Drop
- **THEN** the deck shows "Drop · 8 bars left"

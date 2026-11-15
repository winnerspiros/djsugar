# automix Specification

## Purpose
Automix: DJ Sugar beatmatches and crossfades through a list for the DJ, in a chosen length and style, on decks 1 and 2.

## Requirements

### Requirement: Automix list
The Automix tab of the side panel SHALL hold the list to play. Tracks SHALL be added by dragging or "Add to Automix" (`[Library],AutoDjAddBottom`/`AutoDjAddTop`), reordered by dragging, and dragged out onto decks or lists.

#### Scenario: Reorder
- **WHEN** a row is dragged above another
- **THEN** it plays earlier

### Requirement: Start
Pressing AUTOMIX (`[AutoDJ],enabled`) SHALL start mixing through the list. If a deck is already playing, the louder one SHALL stay live and the first list track load on the other.

#### Scenario: Take over a playing deck
- **WHEN** deck 1 plays and AUTOMIX is pressed
- **THEN** deck 1 keeps playing and the first list track loads on deck 2

### Requirement: Lengths
Mix length SHALL be chosen from 0 s (gapless: the next track starts at the last frame, crossfader centred), 12 s, 15 s, 30 s, 16 bars and 32 bars. Bar lengths SHALL beatmatch within the tempo fader range.

#### Scenario: Gapless album
- **WHEN** the length is 0 s
- **THEN** the next track starts the moment the playing one ends

### Requirement: Styles
Mix style SHALL be Smooth, Bass swap (bass swapped halfway), Filter (outgoing track high-passed away) or Cut.

#### Scenario: Bass swap
- **WHEN** the style is Bass swap
- **THEN** halfway through the mix the incoming bass comes in as the outgoing bass goes

### Requirement: Reliable start and end
A mix SHALL start the chosen length before the outgoing track's audible end (silence below -60 dB ignored). A loop on the outgoing deck SHALL be released when a mix starts, and mix progress SHALL never go backwards.

#### Scenario: Loop on outgoing deck
- **WHEN** the outgoing deck is looping as the mix starts
- **THEN** the loop is released and the mix finishes

### Requirement: Taking over
The DJ SHALL be able to take over at any time; `fade_now` mixes at once and `skip_next` skips the next track.

#### Scenario: Skip
- **WHEN** skip_next is pressed
- **THEN** the next list track is skipped

### Requirement: Decks 1 and 2 only
Automix SHALL use decks 1 and 2 only, whatever the layout.

#### Scenario: Four-deck layout
- **WHEN** Automix runs in 4-Deck
- **THEN** decks 3 and 4 are untouched

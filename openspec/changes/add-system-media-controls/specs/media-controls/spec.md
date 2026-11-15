## ADDED Requirements

### Requirement: System Now Playing
While Player mode shows a track, DJ Sugar SHALL show it in the system's Now Playing (macOS Control Center, the Windows media overlay, MPRIS on Linux) with its title, artist, album, length, album art when it has any, whether it is playing, and the position.

#### Scenario: Track changes
- **WHEN** Automix mixes from "Midnight Drive" into "Low Tide Signals" in Player mode
- **THEN** the system's Now Playing shows "Low Tide Signals" with its artist and art

### Requirement: Media keys
In Player mode, the system's play/pause, next and previous keys and controls SHALL act as the Player bar's buttons: Play/Pause plays or pauses the track shown and starting switches Automix on, Next mixes into the next track in Up Next, Previous goes back to the start of the track, and setting a position moves through the track.

#### Scenario: Headphone pause
- **WHEN** a track plays in Player mode and the DJ presses the pause button on their headphones
- **THEN** the track pauses and the system's Now Playing shows it paused

### Requirement: DJ layouts keep their decks
In the DJ layouts, media keys SHALL do nothing and DJ Sugar SHALL leave the system's Now Playing, unless Settings › Keyboard › Media keys in every layout is on (off at first); then they act on the master deck.

#### Scenario: Live set
- **WHEN** the DJ plays in 2-Deck with the setting off and a headphone button sends pause
- **THEN** no deck stops

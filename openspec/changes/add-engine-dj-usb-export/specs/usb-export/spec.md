## ADDED Requirements

### Requirement: Engine DJ players
The Export to USB dialog SHALL also offer Engine DJ players (Denon DJ Prime and SC, Numark, Rane) and Pioneer DJ and Engine DJ players together. For Engine DJ the stick SHALL hold `Engine Library/Database2/m.db` (schema 3.0.2) listing each exported track, with its title, artist, album, genre, label, composer, remixer, comment, year, key, BPM, rating, length, bitrate, album art and performance data (beat grid, overview waveform, hot cues A to H with names and colours, saved loops, main cue), and the exported lists and folders in order. Its audio SHALL be the same files the Pioneer export writes under `Contents`.

#### Scenario: Export for Engine DJ
- **WHEN** the DJ exports a folder holding two playlists for Engine DJ players
- **THEN** an Engine OS player shows the folder with both playlists and their tracks in order, with their grids, cues and waveforms

#### Scenario: One stick for both brands
- **WHEN** the DJ exports for Pioneer DJ and Engine DJ players
- **THEN** the stick holds both libraries, and each track's audio is on it once

#### Scenario: A library Engine DJ wrote
- **WHEN** the stick already holds an Engine Library that DJ Sugar didn't write
- **THEN** the export stops before writing anything, says so, and the library is left as it was

#### Scenario: Exporting again
- **WHEN** the DJ exports to a stick DJ Sugar wrote an Engine Library to before
- **THEN** that library is replaced with the exported lists, and audio already on the stick isn't copied again

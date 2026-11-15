## ADDED Requirements

### Requirement: Export to USB
The tree's right-click menu SHALL offer **Export to USB…** on any playlist, folder, smart list, history session or stage. It opens a dialog marked experimental that shows what is exported and how many tracks, lists the mounted drives to pick from, and asks which players the stick is for: all players (the default), the Device Library players (CDJ-3000, NXS2 and older, XDJ-XZ, XDJ-RX3), or the OneLibrary players (CDJ-3000X, XDJ-AZ, OPUS-QUAD, OMNIS-DUO).

#### Scenario: Export a folder
- **WHEN** the DJ exports a folder holding two playlists and a subfolder
- **THEN** a player shows the folder with the two playlists and the subfolder, each with its tracks in order

#### Scenario: No drive
- **WHEN** no drive is mounted
- **THEN** the dialog says to plug in a stick formatted FAT32 or exFAT and press Refresh, and Export can't be pressed

### Requirement: What a player gets
For each exported track the stick SHALL hold its audio, its beat grid, hot cues A to H with names and colours, memory cues and loops, the waveforms players draw, its album art, and its title, artist, album, genre, label, key (in the notation DJ Sugar shows), BPM, rating, colour, comment, year and play count. Tracks never analysed SHALL be analysed first.

#### Scenario: Hot cues past H
- **WHEN** a track has hot cues on pads I or J
- **THEN** those cues are left out and the result says how many were

### Requirement: Progress and result
The export SHALL run in the background, showing which track it is on, and can be stopped. When it ends, the dialog SHALL say how many tracks and lists are on the stick, how many files were copied, and which tracks were left out and why.

#### Scenario: Stop
- **WHEN** the DJ presses Stop during an export
- **THEN** the stick keeps the library it had before

#### Scenario: Nothing can be exported
- **WHEN** none of the tracks can be read
- **THEN** the dialog says why and the stick is left unchanged

### Requirement: Exporting again
Each export SHALL replace the stick's library with the exported lists, and SHALL NOT copy audio already on the stick again.

#### Scenario: Second export
- **WHEN** the DJ exports a list again after adding one track
- **THEN** only the new track's audio is copied

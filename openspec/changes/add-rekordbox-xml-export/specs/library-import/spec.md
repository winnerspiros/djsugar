## ADDED Requirements

### Requirement: rekordbox XML export
The Collection node's menu SHALL offer **Export Collection as rekordbox XML…**, and every list and folder SHALL offer **Export as rekordbox XML…** for its tracks and tree. The file SHALL be a `DJ_PLAYLISTS` document rekordbox can import, with for each track: location as a `file://localhost/` URL, title, artist, album, genre, label, year, comment, rating, colour, date added, play count, length, average BPM, key, every beat grid marker (`TEMPO`), hot cues with their number and colour, memory cues, and saved loops with their end (`POSITION_MARK`). Playlists, crates, sets and folders SHALL keep their names, order and tree. Streamed placeholders SHALL be left out.

#### Scenario: Into rekordbox
- **WHEN** the DJ exports a playlist and imports the file in rekordbox 7
- **THEN** the playlist appears with each track's hot cues on the same beats, in the same colours, and its beat grid lined up

#### Scenario: Variable tempo
- **WHEN** a track with a three-marker grid (tempo changes twice) is exported
- **THEN** the file holds three `TEMPO` entries at those positions and tempos

#### Scenario: Round trip
- **WHEN** an exported file is imported by DJ Sugar's own rekordbox XML reader into an empty collection
- **THEN** every exported tag, cue, loop, grid marker and list comes back unchanged

### Requirement: Keep rekordbox.xml up to date
With **Keep rekordbox.xml up to date** ticked in Settings › Collection › General, DJ Sugar SHALL rewrite the whole-collection export at the chosen path in the background, at most once a minute and only after the collection changes.

#### Scenario: Library tool follows DJ Sugar
- **WHEN** the tick is on and the DJ adds a hot cue
- **THEN** the file at the chosen path holds that cue within about a minute

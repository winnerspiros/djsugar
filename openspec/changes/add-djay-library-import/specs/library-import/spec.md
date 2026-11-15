## ADDED Requirements

### Requirement: djay Pro import
Settings › Collection › Import SHALL offer Import from djay Pro, with the three-step wizard, reading djay's `MediaLibrary.db` read-only, kept up to date at start, and removable.

#### Scenario: djay Pro library
- **WHEN** a djay Pro library is imported
- **THEN** each local track keeps its tags, BPM, key, hot cues 1 to 8 with names and colors, saved loops, Automix start and end, play count, color and grid anchor

#### Scenario: djay playlists
- **WHEN** the Playlists group is ticked
- **THEN** djay's playlists appear as lists in djay's order, holding the tracks that are files

#### Scenario: djay is open
- **WHEN** djay Pro is running while DJ Sugar starts
- **THEN** DJ Sugar still reads the library without changing it

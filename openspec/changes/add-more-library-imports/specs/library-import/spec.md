## ADDED Requirements

### Requirement: More importers
Settings › Collection › Import SHALL offer Import from rekordbox and Import from Mixxx alongside VirtualDJ, each with the three-step wizard, read-only, kept up to date at start, and removable.

#### Scenario: rekordbox library
- **WHEN** a rekordbox 6 or 7 library is imported
- **THEN** tracks keep their hot and memory cues with colours, saved loops, beat grid, phrases, rating and play count, and rekordbox playlists and history appear as lists

#### Scenario: rekordbox XML
- **WHEN** the DJ picks a rekordbox XML export instead of the library
- **THEN** the same tracks, cues, grids and playlists come across

#### Scenario: Mixxx library
- **WHEN** a Mixxx `mixxxdb.sqlite` is imported
- **THEN** tracks keep their hot cues, saved loops, beat grid, rating and play count, and Mixxx crates and playlists appear as lists

### Requirement: Pick lists to import
The import wizard SHALL list the source's playlists and folders with a check box each, all ticked except smart lists, and only ticked lists SHALL come across.

#### Scenario: Leave a folder out
- **WHEN** the DJ unticks a VirtualDJ folder in the wizard
- **THEN** its lists don't appear in DJ Sugar, and its tracks come across only if another list or the collection holds them

### Requirement: Cross-source matching
When two imports point at different files of the same track (same artist, title and duration within 2 s), DJ Sugar SHALL ask which copy to keep, defaulting to the one already in the stages, and merge the rest onto it.

#### Scenario: Same song twice
- **WHEN** VirtualDJ and rekordbox hold different copies of a track
- **THEN** the library has one track with both apps' cues

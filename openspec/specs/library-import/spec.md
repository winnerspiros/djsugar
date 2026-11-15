# library-import Specification

## Purpose
Bringing in another DJ app's library, today VirtualDJ's, and keeping it up to date while the DJ still uses that app.

## Requirements

### Requirement: Import wizard
Settings › Collection › General › Import from VirtualDJ › Set up… SHALL walk through three steps: find `database.xml` (default `~/Library/Application Support/VirtualDJ`, `~/.vdj` on Linux) and show what it holds; choose which lists come (playlists, virtual folders, filter folders as smart lists, history); choose whose tags win when they differ.

Once an import is on it SHALL get its own tab in Settings › Collection (after General, Stages and Fields) holding its settings, Stop importing and Remove import; a DJ who has not set one up SHALL not see one. Stopping or removing it SHALL take the tab away.

#### Scenario: VirtualDJ tab
- **WHEN** a DJ who never imported from VirtualDJ opens Settings › Collection
- **THEN** there is no VirtualDJ tab, only Import from VirtualDJ under General

#### Scenario: Pick lists
- **WHEN** only playlists and history are ticked and Import is pressed
- **THEN** playlists and history come in and folders do not

### Requirement: What comes across
Each track SHALL keep VirtualDJ's tags, stars, colour, comment, play count, dates, BPM, grid, key, hot cues with names and colours, saved loops, gain, and remix and automix points; empty fields SHALL be filled from the file's tags. Lists SHALL go into a VirtualDJ folder and history into the History tab.

#### Scenario: Hot cue names
- **WHEN** a VirtualDJ track with named cues is imported
- **THEN** its hot cues have the same names and colours

### Requirement: Read only and kept up to date
DJ Sugar SHALL only read VirtualDJ's files. While the import is on, changes SHALL be brought in at each start; tracks are matched by file so nothing is added twice; grids and cues edited in DJ Sugar SHALL be kept; the VirtualDJ folder SHALL be remade each time.

#### Scenario: Edited grid kept
- **WHEN** a track's grid was edited in DJ Sugar and VirtualDJ's file changes
- **THEN** the DJ Sugar grid stays

### Requirement: Stop and remove
Stop SHALL keep what came in but stop updating. Remove… (pressed twice) SHALL take out the VirtualDJ folder and history, VirtualDJ's cues, grids and markers, and tracks only VirtualDJ had.

#### Scenario: Remove keeps own tracks
- **WHEN** an import is removed
- **THEN** tracks in the DJ's own lists or music folder stay

### Requirement: Imports land in stages
Imported tracks SHALL get their stage from their folder, and tracks removed from the library SHALL stay out.

#### Scenario: Removed stays out
- **WHEN** a track removed from the library is still in VirtualDJ's database
- **THEN** the next import doesn't bring it back

# music-library Specification

## Purpose
The library database: what it keeps about each track, how tracks come in from the music folder, and the DJ's playlists, folders, smart lists, history and linked tracks.

## Requirements

### Requirement: One library database
DJ Sugar SHALL keep its library in one SQLite file (`library.sqlite` in the settings folder): every track's tags, rating, colour, comment, cues, loops, grid and analysis, the DJ's lists and the play history. Once a track is in it, the library SHALL be the source of truth.

#### Scenario: Edits survive a rescan
- **WHEN** a track's rating is changed and the music folder is rescanned
- **THEN** the rating is kept

### Requirement: Field superset
A track SHALL be able to hold every field VirtualDJ, rekordbox and OneLibrary keep: title, artist, remix, album, year, genre, grouping, track and disc number, label, composer, remixer, ISRC, BPM, key, gain, rating, colour, comment, the DJ's tag fields, play count, first seen, last played, cues, loops and markers.

#### Scenario: Nothing dropped on import
- **WHEN** a VirtualDJ track with a label, a remixer and automix points is imported
- **THEN** each of those is stored

### Requirement: Music folder scan
DJ Sugar SHALL look through the music folders (any number, added and removed in Settings › Collection, `~/Music` until set, or `DJSUGAR_MUSIC`) at each start (unless Update library on start is off) and on Rescan, adding new files and re-reading tags only of files changed since. Bringing a track in SHALL only read its tags.

#### Scenario: New file found
- **WHEN** a file is added to the music folder and Rescan is pressed
- **THEN** it appears in the Collection with its tags and is not yet analysed

#### Scenario: No scan on start
- **WHEN** Update library on start is off and DJ Sugar starts
- **THEN** the library shows as saved and the music folders are only looked through on Rescan

### Requirement: Playlists and folders
The DJ SHALL be able to create, rename and delete playlists and folders, nest them, and add tracks by dragging or from the track menu.

#### Scenario: Drag into a playlist
- **WHEN** three selected tracks are dragged onto a playlist
- **THEN** they are appended to it

### Requirement: Smart lists
A search SHALL be savable as a smart list whose contents follow the search; its search SHALL be editable.

#### Scenario: Smart list updates
- **WHEN** a smart list is `bpm:120-126 rating:>=4` and a track is rated 4 at 124 BPM
- **THEN** that track appears in the smart list

### Requirement: Play history
A track playing 30 seconds on a deck SHALL count as played: its play count goes up, its last-played date is set, and it is added to this session's history (named by date and time) in the History tab. Empty sessions SHALL be dropped.

#### Scenario: History session
- **WHEN** the first track of the evening has played 30 seconds
- **THEN** a history session named with the date and time appears with that track

### Requirement: Linked tracks
The DJ SHALL be able to link tracks that go well together. The Linked tab SHALL show the selected track's links, and dropping a track on it SHALL link the two.

#### Scenario: Link by drop
- **WHEN** a track is dropped on the Linked tab while another is selected
- **THEN** each shows the other in its Linked tab

### Requirement: Sidelist
The Sidelist tab SHALL be a scratch list for tracks to come.

#### Scenario: Park a track
- **WHEN** a track is dragged to the Sidelist
- **THEN** it stays there until removed

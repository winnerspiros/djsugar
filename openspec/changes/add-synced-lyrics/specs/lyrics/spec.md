## Purpose

Showing the words of the track playing, from lyrics the DJ already has or finds, timed by line or by word when they can be.

## ADDED Requirements

### Requirement: Lyrics beside the track
DJ Sugar SHALL read lyrics from an `.lrc` file beside the track (`Song.lrc`, or `Song.mp3.lrc`) before the track's tags. In an `.lrc` file it SHALL read line times, word times (enhanced LRC), lines with several times, and `[offset:]` (a positive offset shows lyrics sooner).

#### Scenario: LRC beside the track
- **WHEN** `Song.mp3` has `Song.lrc` beside it with `[00:12.00]First line`
- **THEN** its lyrics show "First line" from 0:12

#### Scenario: Words timed
- **WHEN** a line reads `[00:10.00]<00:10.00>Hold <00:10.40>the <00:10.70>line`
- **THEN** the line is timed by word, "the" from 10.4 s

### Requirement: Lyrics in tags
When there is no `.lrc` file, DJ Sugar SHALL read the track's ID3 `SYLT` (timed in milliseconds), ID3 `USLT`, Vorbis `LYRICS` and MP4 `©lyr`, reading LRC text in them as timed, and SHALL show the most finely timed of what it finds.

#### Scenario: LRC text in a tag
- **WHEN** a FLAC file's `LYRICS` holds LRC text and there is no `.lrc` file
- **THEN** its lyrics show timed by line

### Requirement: Lyrics tab
The side panel SHALL have a Lyrics tab that follows the track playing (or, as picked, the selected track, deck A or deck B), says where its lyrics are from and how they are timed, and, while the track is on a deck, lights the line sung now and keeps it in view. Clicking a timed line SHALL play that deck from the line's start.

#### Scenario: Following the deck
- **WHEN** deck A plays a track with timed lyrics past 0:39 and its line at 0:39 is the last started
- **THEN** that line is lit in the Lyrics tab

#### Scenario: No lyrics
- **WHEN** the track has no `.lrc` file and no lyrics in its tags
- **THEN** the tab says it has no lyrics and how to add them

### Requirement: Online lookup only when asked
DJ Sugar SHALL fetch lyrics from the internet only when the DJ presses Find, SHALL let the DJ turn the lookup off (it is on at first), and SHALL never send lyrics anywhere.

#### Scenario: First launch
- **WHEN** a track with no lyrics is shown in the Lyrics tab on a new install
- **THEN** no request is made to any lyrics service until the DJ presses Find

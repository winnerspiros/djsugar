## Purpose

Turning a recorded mix into a Mixcloud show with a timed tracklist, as a file and as a direct upload.

## ADDED Requirements

### Requirement: Recordings show up at once
When a recording stops, its files SHALL be in the Mixed Sets source without waiting for a library scan.

#### Scenario: Stop recording
- **WHEN** the DJ stops a recording while Mixed Sets is open
- **THEN** the new recording is listed there straight away

### Requirement: Tracklist beside the recording
When a recording stops, DJ Sugar SHALL write `<name>.tracklist.txt` beside it, one `m:ss Artist - Title` line (`h:mm:ss` from an hour) per track, timed from when the track became audible.

#### Scenario: Two tracks
- **WHEN** a recording has one track audible from the start and the next from 4 minutes 5 seconds
- **THEN** the file reads `0:00 Artist - One` and `4:05 Artist - Two`

### Requirement: Export for Mixcloud
Export for Mixcloud SHALL make one 320 kbps MP3 from a mixed set, joining the parts of a split recording, with an ID3v2.4 tag holding the mix's name, a chapter per track of the recording's `.cue` sheet, and the tracklist as a comment. It SHALL not hold the whole recording in memory. Tracks starting in the same second as the one before SHALL be left out, as Mixcloud wants start times to go up.

#### Scenario: A split recording
- **WHEN** the DJ exports a recording made of two parts
- **THEN** one MP3 holds both, and the second part's tracks start after the first part's length

### Requirement: Upload checks first
Before uploading, DJ Sugar SHALL check what Mixcloud would turn down (no name, a description over 1000 characters, audio over 4 GB) and say so, since Mixcloud only answers after the whole file is sent.

#### Scenario: Too long a description
- **WHEN** the description is 1200 characters
- **THEN** DJ Sugar says it is too long before any audio is sent

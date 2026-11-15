# mix-recording Specification

## Purpose
Recording the master mix to a file, with a tracklist and a log of every control move that can later rebuild the set.

## Requirements

### Requirement: Record the master
REC in the top bar (`[Recording],toggle_recording`) SHALL record the master mix, inputs included, to WAV or FLAC at 16 or 24 bits as chosen in Settings › Audio › Recording, into the recordings folder. `[Recording],status` SHALL be 2 while recording and `duration` count the seconds.

#### Scenario: Record a FLAC
- **WHEN** the format is FLAC 24-bit and REC is pressed, then pressed again
- **THEN** a 24-bit FLAC of the mix is in the recordings folder

### Requirement: Crash safe
The file's header SHALL be rewritten every couple of seconds so a crash or power cut keeps everything until then. WAV recordings SHALL move to a new part before 4 GB.

#### Scenario: Power cut
- **WHEN** DJ Sugar is killed during a recording
- **THEN** the file plays up to a couple of seconds before the kill

### Requirement: Tracklist
When a recording stops, a `.cue` file SHALL list each track from the moment it became audible.

#### Scenario: Cue sheet
- **WHEN** three tracks were mixed in a recording
- **THEN** the cue sheet lists them with their start times

### Requirement: Control log
Beside the recording, `<name>.controls.jsonl` SHALL hold one line per control change, stamped with the time in the recording, and one line per track loaded on a deck.

#### Scenario: Fader move logged
- **WHEN** a channel fader moves during a recording
- **THEN** the log has a `[ChannelN],volume` line with its time

### Requirement: Show recordings
Settings SHALL have "Show recordings", opening the folder with the last recording selected.

#### Scenario: Find the file
- **WHEN** Show is pressed
- **THEN** the file manager opens with the last recording selected

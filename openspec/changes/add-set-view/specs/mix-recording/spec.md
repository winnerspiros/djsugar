## ADDED Requirements

### Requirement: Frame-accurate control log
Control changes in the control log SHALL be stamped with the audio frame at which they took effect. Plugin parameter changes and instrument notes SHALL be logged.

#### Scenario: Exact time
- **WHEN** a hot cue is pressed during a recording
- **THEN** its log line's time matches the frame where the jump is heard in the recording

### Requirement: Render an edited set
After the DJ moves, stretches or edits transitions or lanes in the Mixed Set Editor, DJ Sugar SHALL render the set through its engine to a new file beside the recording and SHALL NOT change the recording.

#### Scenario: Scratch kept or bypassed
- **WHEN** a set has a scratch on deck 2 and the DJ leaves it on Keep from recording
- **THEN** the render holds the recording's audio for that stretch
- **WHEN** the DJ switches it to Bypass
- **THEN** the render plays deck 2 straight through that stretch as if it had not been scratched

#### Scenario: Unedited render matches
- **WHEN** a recording with no edits is rendered
- **THEN** the render matches the recording within -60 dBFS, apart from third-party plugin effects

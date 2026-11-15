# collection-stages Specification

## Purpose
Stages take new music from where it lands to tracks ready to play: Inbox, In Prep (with prep crates) and Play Ready (stored as `triage` and `collection`), each with its folder, with queued file moves and undo.

## Requirements

### Requirement: Stage folders
Settings › Collection › Stages SHALL hold any number of Inbox folders, one In Prep folder and one Play Ready folder. Every track SHALL get the stage of the folder its file is in; tracks elsewhere have no stage.

#### Scenario: Stage from folder
- **WHEN** a file sits in the In Prep folder
- **THEN** it shows under In Prep and matches `stage:prep` (or `stage:triage`)

### Requirement: Prep crates
Subfolders of the In Prep folder SHALL be prep crates. Adding a track from a crate to Play Ready SHALL also add it to the list named after the crate without "STG" (`Yoga STG` joins Yoga).

#### Scenario: Crate to list
- **WHEN** a track in `Yoga STG` is sent to Play Ready
- **THEN** it joins the Yoga list

### Requirement: Moving between stages
The track menu and dropping on a stage SHALL move tracks: Send to In Prep (or a crate), Send to Play Ready, Back to In Prep, Back to Inbox; the menu SHALL show the stages in order with the track's own lit, and mark moves back and forward with arrows. Undo (⌘Z / Ctrl+Z, Edit › Undo on macOS) SHALL take back the last batch. There SHALL be no hidden stage: a track the DJ doesn't want goes to Quarantine, and tracks once hidden from the Inbox SHALL show in it again.

#### Scenario: Undo
- **WHEN** five tracks are sent to Play Ready and Undo is chosen
- **THEN** they are back in In Prep

### Requirement: Queued file moves
A stage change SHALL apply in DJ Sugar at once. Its file SHALL only move while "Move files" is on (off by default); otherwise the move waits and is made when it is turned on. A file SHALL never be moved over another.

#### Scenario: Moves wait
- **WHEN** Move files is off and a track goes to In Prep
- **THEN** it shows in In Prep but its file stays where it was until Move files is on

### Requirement: Rename into In Prep
Files moving from Inbox to In Prep SHALL be renamed `Artist - Title (Remix)` from their tags unless "Rename files sent to In Prep" is off.

#### Scenario: Clean name
- **WHEN** `track01_final.mp3` tagged Artist A, Title B, Remix C moves to In Prep
- **THEN** it is named `Artist A - Title B (C).mp3`

### Requirement: Background analysis in In Prep
Tracks entering In Prep SHALL be analysed in the background so tempo and key fill in.

#### Scenario: Prep dots fill in
- **WHEN** a track enters In Prep
- **THEN** its tempo and key dot fills once analysis finishes

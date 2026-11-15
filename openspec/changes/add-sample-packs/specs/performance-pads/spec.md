## MODIFIED Requirements

### Requirement: Sampler
There SHALL be 16 sampler slots (`[Sampler1]` to `[Sampler16]`). A deck's SAMPLER page SHALL show 8 of them, page 1 (slots 1-8) or page 2 (slots 9-16), with a 1/2 page button on the side of the pads that flips the page; each deck keeps its own page in `[ChannelN],sampler_page`, and a controller's sampler pads SHALL play the slots of that deck's page. Each pad SHALL show its sample's name and light while it plays; samplers SHALL join the main mix after the crossfader. Out of the box the slots SHALL hold the Default DJ Sample Pack: kick, snare, clap, hat, horn, siren, laser and crash.

#### Scenario: Air horn
- **WHEN** the Horn pad is pressed
- **THEN** the horn plays once in the master and the pad lights while it plays

#### Scenario: Page 2
- **WHEN** the left deck's 1/2 button is clicked and the DDJ-FLX4's left pad 1 is pressed in SAMPLER mode
- **THEN** slot 9 plays and the left deck's first pad on screen lights

## ADDED Requirements

### Requirement: Sample packs
A sample pack SHALL be a named set of up to 16 samples with an optional cover, author and licence. The built-in sounds SHALL be the Default DJ Sample Pack, which can't be deleted. A SAMPLE PACK menu under the pads, never between the page tabs and the pads, SHALL list the packs with their covers, switch the slots to the chosen pack, offer + to make a new empty pack or import a `.djsugarpack` file, and end with Get more Packs. The chosen pack SHALL be kept across restarts.

#### Scenario: Switch pack
- **WHEN** "Dancehall Drops" is chosen in the SAMPLE PACK menu
- **THEN** the pads show and play that pack's samples, and it is still chosen after a restart

#### Scenario: Default pack can't be removed
- **WHEN** the SAMPLE PACK menu is opened
- **THEN** the Default DJ Sample Pack has no Delete option

### Requirement: Choose a sample
A pad's "…" button and right-click menu SHALL offer Choose Sample…, Load file…, Rename…, Play mode, Reset to pack sample and Clear. Choose Sample SHALL open a popover above the pad with a pack menu and that pack's samples, each with a preview button and its length; the preview SHALL play in the headphones only. Picking a sample SHALL put it on the pad.

#### Scenario: Borrow a sample from another pack
- **WHEN** Choose Sample is opened on pad 3, the pack menu is set to Default DJ Sample Pack and Horn is picked
- **THEN** pad 3 plays the horn and is named "Horn"

#### Scenario: Preview in headphones
- **WHEN** the preview button next to "Siren" is pressed
- **THEN** the siren plays in the headphones and not in the master

### Requirement: Own samples
An audio file (WAV, AIFF, FLAC, MP3, M4A or OGG, up to 60 seconds) SHALL be loadable onto a pad by dropping it from the file manager or the track list, or with Load file…. A pad SHALL be renamable, and Reset to pack sample SHALL bring back the pack's own sample, name, level and mode. Loading SHALL NOT interrupt playback, and a file that can't be read SHALL leave the pad as it was and say why.

#### Scenario: Drop a file on a pad
- **WHEN** `pull-up.wav` is dropped on pad 6
- **THEN** pad 6 is named "pull-up" and plays that file, also after a restart

#### Scenario: Rename
- **WHEN** pad 6 is renamed "Rewind"
- **THEN** the pad shows "Rewind" on every deck's SAMPLER page

#### Scenario: Missing file
- **WHEN** the file on pad 6 has been deleted since it was loaded
- **THEN** pad 6 shows its name dimmed with a warning mark and pressing it plays nothing

### Requirement: Pad level and play mode
Each pad SHALL have a level from silent to +6 dB (0 dB by default), set with a small knob on the pad in edit mode, where pads can also be dragged onto each other to swap them, and a play mode: One shot (plays to the end; pressing again restarts), Hold (plays while held), On/Off (pressing again stops) or Loop (repeats until pressed again). Shift + pad SHALL stop a playing sample. Level, name, sample and mode SHALL be saved per pack in settings, without changing the pack's files.

#### Scenario: Turn a drop down
- **WHEN** pad 6's level knob is turned to -6 dB and DJ Sugar is restarted
- **THEN** pad 6 still plays at -6 dB

#### Scenario: Swap two pads
- **WHEN** edit mode is on and pad 1 is dragged onto pad 3
- **THEN** the two pads swap samples, names, levels and modes, and the swap is kept for that pack

#### Scenario: Hold mode
- **WHEN** a pad in Hold mode is pressed and released half way through its sample
- **THEN** the sample stops when the pad is released

### Requirement: Share packs
A pack SHALL be exportable as one `.djsugarpack` file holding its samples, names, levels, modes, cover and licence, and importable on another computer by + › Import or by dropping the file on the SAMPLE PACK menu.

#### Scenario: Move a pack
- **WHEN** "Dancehall Drops" is exported on one computer and the file is imported on another
- **THEN** the second computer has a "Dancehall Drops" pack with the same pads

### Requirement: Record a sample
Choose Sample SHALL offer Record New Sample from deck 1, the mic or deck 2 (and decks 3 and 4 in four-deck layouts). Recording SHALL start on press and stop on a second press, or after the deck's current loop when one is playing; the recording SHALL go on the pad and into a "My Recordings" pack.

#### Scenario: Capture a loop
- **WHEN** record from deck 1 is pressed on an empty pad while a 4-beat loop plays
- **THEN** the pad holds those 4 beats, in Loop mode

### Requirement: Synced loop samples
A pad in Loop mode whose sample has a known BPM SHALL play at the master deck's tempo and start on the next beat when quantize is on.

#### Scenario: Loop follows the mix
- **WHEN** a 120 BPM loop sample is started while the master deck plays at 126 BPM with quantize on
- **THEN** the loop plays at 126 BPM, starting on the next beat

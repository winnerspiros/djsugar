## ADDED Requirements

### Requirement: Groove Box deck
Any deck SHALL be able to become a Groove Box from its header menu, and back to a track deck the same way. Only an empty deck SHALL switch; on a loaded deck the menu SHALL offer to eject first. A Groove Box deck SHALL keep its mixer channel: EQ, filter, channel fader, crossfader side, deck FX, Color FX, headphone cue and meters SHALL work on it as on a track. It SHALL never lead SYNC or Ableton Link, and Automix SHALL skip it. Which decks are Groove Boxes, and their kit and grooves, SHALL be kept across restarts.

#### Scenario: Deck 2 becomes a Groove Box
- **WHEN** deck 2 is empty and Groove Box is chosen from its header menu
- **THEN** the jog and waveform area shows the Groove Box with the first kit and its first groove, and deck 2's EQ and fader still work

#### Scenario: Loaded deck
- **WHEN** Groove Box is chosen on a deck that holds a track
- **THEN** DJ Sugar asks to eject the track first and nothing changes until it is ejected

### Requirement: One press to play
Every kit SHALL come with at least four named grooves and the first SHALL be loaded when the kit is chosen. PLAY (`[ChannelN],play` on the Groove Box deck) SHALL start the groove on the next bar of the leading deck when one plays, and at once otherwise; pressing it again SHALL stop at once. A new Groove Box SHALL start with its channel fader as a new deck has it, so it is never louder than the music by surprise.

#### Scenario: Start in time
- **WHEN** deck 1 plays at 124 BPM and PLAY is pressed on the Groove Box during beat 3
- **THEN** the groove's first step plays exactly on deck 1's next downbeat, at 124 BPM

#### Scenario: Nothing playing
- **WHEN** no deck plays, Link is off and PLAY is pressed
- **THEN** the groove starts at once at the last tempo used (120 BPM the first time)

### Requirement: Steps and rows
The Groove Box SHALL show four rows of 16 steps, each row named after its sound, with the beats (steps 1, 5, 9, 13) shaded so the bar reads at a glance and a playhead on the step playing. Clicking a step SHALL turn it on or off; while the groove is stopped the click SHALL also play the row's sound. Each row SHALL have mute (M) and solo (S). Clicking a row's name SHALL select it and open the kit's sounds to swap it. A step's right-click menu SHALL offer velocity (soft, medium, hard), chance (100, 75, 50, 25 %) and nudge (early, on time, late).

#### Scenario: Add a clap
- **WHEN** the Clap row's steps 5 and 13 are clicked while the groove plays
- **THEN** a clap plays on beats 2 and 4 from the next time the playhead reaches them

#### Scenario: Mute the hats
- **WHEN** M is clicked on the Hat row
- **THEN** the hats stop at once and the other rows play on

### Requirement: Sound controls
LEVEL, PITCH (−12 to +12 semitones) and DECAY SHALL act on the selected row, and SWING (0 to 75 %) on the whole groove, delaying every second 16th. Changes SHALL glide without clicks.

#### Scenario: Shorter kick
- **WHEN** the Kick row is selected and DECAY is turned left
- **THEN** each kick dies away sooner and the other rows don't change

### Requirement: Grooves A to D
A Groove Box SHALL hold four grooves, A to D. Choosing another while it plays SHALL switch on the next bar, the chosen letter blinking until then. The groove menu SHALL offer the kit's named grooves, Copy to…, Clear, and Back to the kit's groove. Edits SHALL be kept per kit in settings; kit files SHALL never change.

#### Scenario: Switch on the bar
- **WHEN** B is clicked during the second beat of a bar
- **THEN** A plays to the end of the bar and B starts on the next downbeat

### Requirement: Duck
DUCK SHALL be off by default. With a deck chosen in its menu, that deck's channel SHALL dip on every step of the Kick row, by the amount chosen (default 6 dB), recovering over the step.

#### Scenario: Duck deck 1
- **WHEN** DUCK is set to deck 1 and the groove plays four kicks a bar
- **THEN** deck 1 dips briefly on each kick and its fader position doesn't move

### Requirement: Always in time
The groove's tempo and bar SHALL follow, in this order: the leading deck while it plays, Ableton Link's session when Link is on and no deck plays, or its own tempo. It SHALL follow the leader's tempo changes and stay in phase; a jump on the leading deck (a hot cue, a loop, a beat jump) SHALL move the groove only at the next bar, never mid-bar. Each step SHALL sound on the exact audio sample it falls on, whatever the audio buffer size.

#### Scenario: Tempo change
- **WHEN** the leading deck's tempo fader moves from 124 to 126 BPM while the groove plays
- **THEN** the groove plays at 126 BPM and stays on the leader's beat

#### Scenario: Link
- **WHEN** Link is on, no deck plays and Ableton Live plays at 100 BPM
- **THEN** the groove plays at 100 BPM on Live's bar

### Requirement: Kits
A kit SHALL be a sample pack with up to 16 sounds, four of them on the rows, and named grooves written as steps. Kits SHALL use the pack store, `.djsugarpack` export and import, and Get more Packs; a kit's sounds also play on the sampler pads. DJ Sugar SHALL ship at least four built-in kits synthesised by the app itself, in the style of classic drum machines and named by style (for example *Boom* and *Punch*), never by a manufacturer's model name. Kits from others SHALL carry their author and licence, shown in the kit menu.

#### Scenario: Import a kit
- **WHEN** a `.djsugarpack` file holding a kit is dropped on the Groove Box
- **THEN** the kit joins the kit menu with its grooves, author and licence

### Requirement: Controller control
The Groove Box SHALL be reachable through `[GrooveN]` controls (N is the deck): `play` (start and stop, as `[ChannelN],play`), `groove` (0-3), `groove_queued`, `row` (selected row, 0-3), `row1_mute`…`row4_mute`, `row1_solo`…`row4_solo`, `level`, `pitch`, `decay` (selected row), `swing`, `step1_1`…`step4_16` (on/off, readable for LEDs), `position` (playing step, 0-15) and `duck_deck`. The pads SHALL have a Steps page showing 8 steps of the selected row, steps 1-8 or 9-16 with the page shifter, lit when on, the playing step blinking; with Shift, pads 1-4 select the row and 5-8 the groove.

#### Scenario: DDJ-FLX4 steps
- **WHEN** the left pads are on the Steps page of a Groove Box on deck 1 and pad 5 is pressed
- **THEN** step 5 of the selected row turns on and the pad lights

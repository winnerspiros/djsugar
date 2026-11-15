## ADDED Requirements

### Requirement: Timeline in the waveform strip
A Groove Box deck SHALL use the strip a track deck uses for its waveform to show a bar timeline: grooves as blocks along the bars, the hits of each row as ticks, and a playhead at the same position and zoom as the other decks' waveforms, with the groove's bars on the leading deck's bars. The leading deck's phrase markers SHALL be drawn behind the blocks. With no chain, the timeline SHALL show the selected groove repeating.

#### Scenario: Line up with a drop
- **WHEN** deck 1 plays a track with a drop in 6 bars and the Groove Box on deck 2 plays groove A
- **THEN** deck 2's timeline shows the drop's phrase marker 6 bars ahead, under the same point as deck 1's waveform

### Requirement: Chains
A Groove Box SHALL hold eight grooves (A to H) and a chain of up to 64 bars built by dragging grooves onto the timeline. A chain SHALL play in order and loop; clicking a block SHALL move playback there on the next bar.

#### Scenario: Four bars of A, four of B
- **WHEN** A is dragged onto bars 1-4 and B onto bars 5-8 and PLAY is pressed
- **THEN** A plays for four bars, B for four, then A again

### Requirement: Step detail
Each step SHALL have an accent (soft, medium, hard), a nudge (−50 % to +50 % of a step), a roll (1 to 4 hits within the step) and a condition: a chance (100, 75, 50, 25 %), *X of Y* bars, Fill or Not fill. These SHALL be set from the step's right-click menu or by holding the step and turning a knob. Steps at the same position whose chances add up to 100 % SHALL play exactly one of them. The face SHALL show accent as brightness, rolls as a split cell and a condition as a corner mark, and nothing else.

#### Scenario: Every fourth bar
- **WHEN** step 15 of the Snare row has the condition 4 of 4
- **THEN** it plays only in the fourth bar of every four

#### Scenario: Roll at the end of the bar
- **WHEN** step 16 of the Snare row is set to a roll of 4
- **THEN** four evenly spaced snares play within that 16th, each on its exact sample

### Requirement: Rows and lengths
A kit SHALL hold up to 16 sounds and show up to 8 rows. Each row SHALL have its own length (1 to 64 steps) and step size (1/16, 1/8, 1/16 triplet). Each row SHALL also have a direction (forward, reverse, ping-pong) and offer Euclid (N hits spread evenly over its length) and Rotate. Rows of different lengths SHALL run independently and restart together when the groove restarts or changes.

#### Scenario: Three-step hats
- **WHEN** the Hat row's length is set to 3 while the Kick row is 16 steps
- **THEN** the hat pattern repeats every 3 steps against the 16-step kick

### Requirement: Sound knobs
Every row SHALL have LEVEL, PITCH, DECAY and a fourth knob named by the kit. For built-in synthesised sounds the fourth knob SHALL change the synth voice; for sample sounds it SHALL be a low-pass filter.

#### Scenario: Snappier snare
- **WHEN** the Snare row of a built-in kit is selected and SNAPPY is turned right
- **THEN** the snare's noise part is louder on the next hit

### Requirement: Generate grooves
A dice button SHALL make a new groove from the kit's rules for each row, leaving locked rows as they are, with a Busy control for how many hits. Remix SHALL change a few hits of the current groove. A new or remixed groove SHALL start on the next bar while playing. Back to the kit's groove SHALL undo both. A groove-wide Slop control SHALL spread hit timing randomly by up to ±10 ms, off by default.

#### Scenario: Keep the kick
- **WHEN** the Kick row is locked and the dice is pressed while playing
- **THEN** the other rows change on the next bar and the kick plays as before

### Requirement: Fills and variation
While FILL is on, steps marked Fill SHALL play and steps marked Not fill SHALL be silent. FILL SHALL be on while held, or, set to Auto, during the last bar before the leading deck's next phrase (every 8 bars when the leader has no phrases). The Variation switch SHALL alternate the groove and its variation every bar or every two bars. Every built-in groove SHALL have Fill steps.

#### Scenario: Auto fill into the drop
- **WHEN** FILL is set to Auto 3 bars before the leading deck's next phrase
- **THEN** the groove plays normally for 2 bars, with its Fill steps on the third, and normally again from the phrase

### Requirement: Melodic row
A kit MAY have one melodic row whose steps hold a scale degree, octave, length and slide. Notes SHALL be played in the key of the leading deck, following key changes on the bar, unless a fixed key is set. The row SHALL play through a built-in mono synth or a hosted instrument plugin.

#### Scenario: Follow the key
- **WHEN** the melodic row plays degrees 1, 1, 5, 4 and the leading deck is in A minor
- **THEN** the row plays A, A, E, D

### Requirement: Record from pads
With REC armed, hits on the sampler pads or the Steps page SHALL be written into the selected groove at the nearest step, with accent from velocity. When the groove is stopped, recording SHALL start after a one-bar count-in. Undo SHALL remove the last pass.

#### Scenario: Record a clap
- **WHEN** REC is on and the Clap pad is hit slightly late on beats 2 and 4
- **THEN** the Clap row's steps 5 and 13 turn on

### Requirement: Sounds from the mix
A file, a sampler pad or a region selected on a track deck's waveform dropped onto a row SHALL become that row's sound, trimmed to its first transient and kept as an override of the kit. With stems loaded on a track deck, a row's menu SHALL offer the kick, snare or hat cut from that deck's drum stem.

#### Scenario: Kick from the track
- **WHEN** deck 1 has stems and "Kick from deck 1" is chosen on the Kick row
- **THEN** the Kick row plays a kick cut from deck 1's drum stem

### Requirement: Replace drums
A track deck with stems SHALL offer Replace drums: a Groove Box over the deck that mutes the track's drum stem and plays its groove in the track's grid on the deck's channel, with roll and stutter. Turning it off SHALL bring the track's drums back on the next beat.

#### Scenario: Swap the drums
- **WHEN** Replace drums is turned on during a track's chorus
- **THEN** the track's drum stem fades out and the groove's drums play on the track's beats until it is turned off

### Requirement: MIDI file export
A groove or chain SHALL be draggable out of DJ Sugar as a standard MIDI file, each drum row on its General MIDI drum note and the melodic row on its notes, at the groove's tempo.

#### Scenario: Into a DAW
- **WHEN** groove A is dragged from the Groove Box onto a DAW track
- **THEN** the DAW gets one bar of notes with the kick on note 36 and accents as velocity

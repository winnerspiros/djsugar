## ADDED Requirements

### Requirement: FX performance budget
The effect rack SHALL NOT allocate, lock or wait on the audio thread. It SHALL hold delay memory only for effects that are sounding, lent from a pool allocated at start-up, and SHALL process active effects in blocks where the benchmark shows a gain. A benchmark SHALL report each effect's CPU cost per frame, a busy four-deck rack's cost, and the rack's memory.

#### Scenario: Four decks with effects
- **WHEN** all four decks each run three effects at 48 kHz
- **THEN** the benchmark reports the rack's cost per frame, and no allocation happens on the audio thread

#### Scenario: Memory
- **WHEN** DJ Sugar starts at 48 kHz
- **THEN** the rack holds under 40 MB of delay memory, instead of one two-second buffer per instance

#### Scenario: Pool empty
- **WHEN** an effect turns on while no delay buffer is free
- **THEN** it stays silent without glitching, and starts once a buffer comes back

### Requirement: Clean effects
Every effect SHALL give silence for silence (except Noise), stay finite and bounded at every parameter setting, and let feedback tails decay to true silence.

#### Scenario: Extreme settings
- **WHEN** an effect with every parameter at its minimum or maximum gets full-scale noise
- **THEN** its output stays finite and below +18 dBFS

### Requirement: Smooth parameters
Every continuous effect parameter SHALL glide per sample, so turning any knob, even by big MIDI steps, never clicks or zips.

#### Scenario: Reverb decay jump
- **WHEN** a sounding reverb's decay jumps from minimum to maximum
- **THEN** the output has no step larger than the music's own

### Requirement: Beat-locked timing
Gate, Flanger and Phaser SHALL take their phase from the routed deck's beat grid, and Roll SHALL start its slice on the beat division, unless the unit runs on a tapped tempo.

#### Scenario: Gate on the beat
- **WHEN** a quarter-beat Gate turns on part-way through a beat
- **THEN** it opens on the next quarter-beat of the deck's grid

### Requirement: Clean modulated delays
Echo, Flanger and Spiral SHALL read their delay lines with cubic or better interpolation, so sweeps keep their high frequencies and don't zipper.

#### Scenario: Swept delay read
- **WHEN** a delay line holding a 10 kHz tone is read at a sweeping fractional delay
- **THEN** the tone's level stays within 1 dB across the sweep

### Requirement: More effects
DJ Sugar SHALL add Drive (oversampled so it doesn't alias), Chorus, a Ladder filter, a Hall reverb, Rev Roll and Filter Echo, after the twelve existing effects so saved choices keep their meaning. Each SHALL be beat-synced where it has a time setting, and each SHALL have click-free parameters.

#### Scenario: Drive without aliasing
- **WHEN** Drive distorts a 5 kHz sine at full drive
- **THEN** audible aliased components (below 20 kHz) stay at least 60 dB below the fundamental

## MODIFIED Requirements

### Requirement: Effect list
DJ Sugar SHALL offer these effects: Echo, Reverb, Filter, Flanger, Bitcrusher, Phaser, Gate, Roll, Echo Out, Brake, Noise, Spiral, Drive, Chorus, Ladder, Hall, Rev Roll and Filter Echo, each with up to four parameters.

#### Scenario: Effects listed
- **WHEN** an effect slot's menu is opened
- **THEN** it lists the eighteen effects by name

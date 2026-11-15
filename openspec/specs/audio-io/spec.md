# audio-io Specification

## Purpose
Where DJ Sugar's sound goes and comes from: output devices for the master, headphone and booth mixes, buffer size and sample rate, and running without a sound device.

## Requirements

### Requirement: Output routes
Settings › Audio › Outputs SHALL send each of the master, headphone (cue) and booth mixes to a chosen device and channel pair. Mixes MAY go to different devices; devices other than the master's SHALL be kept in time by dropping or padding a little audio as their clocks drift.

#### Scenario: Headphones on a second device
- **WHEN** master goes to a controller's outputs 1-2 and headphones to the laptop's built-in output
- **THEN** both play, and the headphone delay stays short over a long set

### Requirement: Buffer size
Settings › Audio SHALL offer the buffer size, with "Device default" first, to trade latency against dropouts.

#### Scenario: Smaller buffer
- **WHEN** a smaller buffer size is chosen
- **THEN** controls respond with less delay from the next restart of the audio stream

### Requirement: Sample rate
Settings › Audio SHALL offer the sample rate, with "Device default" first. A new rate SHALL apply from the next start, if the device plays it, and the setting SHALL say what runs now.

#### Scenario: Rate takes effect next start
- **WHEN** 48 kHz is chosen while running at 44.1 kHz
- **THEN** the setting reads "Runs at 44.1 kHz now; 48 kHz from the next start"

### Requirement: Headless operation
Without a usable output device, the engine SHALL run on a timer so everything except the sound keeps working.

#### Scenario: CI machine
- **WHEN** DJ Sugar starts on a machine with no sound device
- **THEN** decks still load, play and move their playheads

### Requirement: Real-time safety
The audio thread SHALL NOT allocate, block or wait on disk or network; tracks, analysis and network work SHALL happen on other threads.

#### Scenario: Slow disk
- **WHEN** a track loads from a slow network drive while another deck plays
- **THEN** the playing deck has no dropouts

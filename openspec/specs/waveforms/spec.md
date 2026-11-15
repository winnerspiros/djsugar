# waveforms Specification

## Purpose
The waveforms drawn on the GPU: scrolling and overview waveforms per deck, their colours and markers, and showing or hiding the large ones.

## Requirements

### Requirement: GPU waveforms
Waveforms SHALL be drawn by GPU shaders, coloured by frequency band (low, mid, high), with every frame changing only a small uniform block.

#### Scenario: Smooth at 60 fps
- **WHEN** four decks play with the large waveforms shown
- **THEN** the waveforms scroll smoothly

### Requirement: Scrolling and overview
Each deck SHALL have a scrolling waveform around the playhead and an overview of the whole track with the played part, hot cues, loop and playhead marked, plus the time remaining.

#### Scenario: Hot cue markers
- **WHEN** a hot cue is set
- **THEN** it appears in its colour on both waveforms

### Requirement: Glide between pixels
The scrolling waveform SHALL glide between pixels rather than step, without shimmer.

#### Scenario: Slow playback
- **WHEN** a deck plays at -50 %
- **THEN** the waveform moves smoothly with no tug and pull

### Requirement: Show or hide large waveforms
The WAVEFORMS button in the top bar SHALL show or hide the large waveforms (setting `waveforms_visible`). In 4-Deck they SHALL stack in the order 3, 1, 2, 4.

#### Scenario: Hide waveforms
- **WHEN** WAVEFORMS is turned off
- **THEN** the large waveforms disappear and the library gets the space

### Requirement: Idle redraw
The window SHALL redraw only when something on it changes, so an idle DJ Sugar uses little CPU.

#### Scenario: Idle
- **WHEN** no deck plays and nothing is touched
- **THEN** DJ Sugar uses close to no CPU

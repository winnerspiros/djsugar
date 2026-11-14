## ADDED Requirements

### Requirement: Waveform color schemes
Settings › Decks › Waveform color SHALL pick how every deck's waveforms are colored, on every layout: RGB (one shape colored by its loudest band; the default), 3-Band (low, mid and high stacked, low widest), Blue (one blue shape, lighter where the highs are loud) or Stems (each stem stacked in its pad color when the track has stems, RGB otherwise). For 3-Band, Settings › Decks › Palette SHALL pick Blue-Orange-White or Lilac-Cyan-White for low, mid and high. Changing either SHALL redraw the decks at once without loading the tracks again.

#### Scenario: rekordbox colors
- **WHEN** the DJ picks 3-Band with Blue-Orange-White
- **THEN** each deck's waveforms show the bass in blue, the mids in orange and the highs in white, stacked

#### Scenario: Default
- **WHEN** a new user opens the app
- **THEN** the waveforms are RGB

### Requirement: Half overview
Settings › Decks › Overview SHALL draw each deck's overview Full (mirrored about its middle) or Half (growing from its bottom edge up). The big scrolling waveform SHALL stay whole either way.

#### Scenario: Half
- **WHEN** the DJ picks Half
- **THEN** each deck's overview grows from its bottom edge, its cue and hot cue flags at the top

### Requirement: Show vocals
Vocals › Bar in the waveforms button's drop-down SHALL mark the parts of the large waveforms where a track is sung with a cyan bar along their bottom edge, leaving the waveform colours unchanged. The vocal level along the track SHALL come from a vocal-detection pass (Spleeter's vocal network on its own) run in the background the first time a file is loaded on a deck or analysed, and kept beside its analysis so it runs once per file. When a track has high-quality stems, the level SHALL come from its vocal stem instead. The waveforms SHALL draw without the strip until it is found.

#### Scenario: Chorus
- **WHEN** Vocals › Bar is picked and a deck's track has a sung chorus at 1:00
- **THEN** that stretch of its large waveform has the cyan bar along the bottom

#### Scenario: First load
- **WHEN** a track that was never checked for vocals is loaded
- **THEN** its waveforms draw at once and the bar appears a few seconds later, and at once on every later load

### Requirement: Size follows gain
Settings › Decks › Size follows gain, off at first, SHALL scale the height of a deck's waveforms with its gain knob, between a quarter and twice the height.

#### Scenario: Gain up
- **WHEN** Size follows gain is on and the DJ turns deck A's gain up
- **THEN** deck A's waveforms grow taller

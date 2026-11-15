# track-analysis Specification

## Purpose
How DJ Sugar works out a track's waveform, tempo, beat grid, key and loudness, when it does so, and how results are kept.

## Requirements

### Requirement: What analysis finds
Analysis SHALL produce a 3-band waveform summary, the tempo (to 0.01 BPM), a beat grid, the key (unless the tags or an import have one) and the EBU R128 loudness and peak.

#### Scenario: New track analysed
- **WHEN** a track with no key in its tags is analysed
- **THEN** it gets a waveform, BPM, beat grid, key and loudness

### Requirement: When analysis runs
A track SHALL be analysed the first time it loads on a deck, playing meanwhile. Tracks in In Prep, and tracks Collection Health's Analyze asks for, SHALL be analysed in the background one at a time. Grids, keys and cues from an import SHALL be used without waiting.

#### Scenario: In Prep analysis
- **WHEN** a track is sent to In Prep
- **THEN** its tempo and key fill in without loading it on a deck

### Requirement: Tempo range
Tempo detection SHALL look within the BPM range set in Settings › Analysis (70-180 BPM unless changed).

#### Scenario: Drum and bass range
- **WHEN** the range is 118-235 BPM and a 174 BPM track is analysed
- **THEN** it reads 174, not 87

### Requirement: Dynamic grids from analysis
Where most tracked beats stray from a steady grid (live drums, old disco), analysis SHALL produce a grid of sections with their own tempos.

#### Scenario: Live drummer
- **WHEN** a live recording that speeds up is analysed
- **THEN** its grid has several sections and stays on the beats

### Requirement: Key detection choice
Settings › Analysis "Key detection" SHALL offer Standard (chroma template, default) and Neural (a KeyNet CNN run in Rust).

#### Scenario: Neural detector
- **WHEN** Neural is chosen and a track is analysed
- **THEN** its key comes from the neural detector and is marked as detected

### Requirement: Analysis cache
Results SHALL be cached on disk by file path, size and modification time, so an edited file is analysed again, and SHALL survive tag rewrites made by DJ Sugar. Settings › Collection "Clear" SHALL throw the cache away.

#### Scenario: Edited file re-analysed
- **WHEN** a file's audio is replaced outside DJ Sugar
- **THEN** it is analysed again the next time it loads

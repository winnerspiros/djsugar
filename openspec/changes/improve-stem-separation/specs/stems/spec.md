## RENAMED Requirements

- FROM: `### Requirement: Separate with Demucs`
- TO: `### Requirement: Separate any track`

## MODIFIED Requirements

### Requirement: Separate any track
Any track that is not an NI stem file SHALL get stems from DJ Sugar itself, without Python or Demucs installed. The models SHALL download on first use and SHALL be checked before use. Separation SHALL run off the audio thread, on the GPU when one is available (Metal on macOS, Vulkan on Linux) and on the CPU otherwise. When the models cannot be downloaded and Demucs is installed, SEPARATE STEMS SHALL still use Demucs; otherwise the STEMS page SHALL say why there are no stems and offer TRY AGAIN.

#### Scenario: Fresh Mac
- **WHEN** a track is loaded on a fresh install with the models downloaded
- **THEN** the deck gets stems without Demucs or Python

#### Scenario: No GPU
- **WHEN** no GPU adapter is available
- **THEN** separation runs on the CPU and the stems still arrive

#### Scenario: Separate a track
- **WHEN** SEPARATE STEMS is pressed on a loaded MP3 while *Stems on load* is off
- **THEN** the page shows progress, and when done the deck plays the stems

#### Scenario: Cached stems
- **WHEN** a separated track is loaded again
- **THEN** its stems are available at once

### Requirement: Stem controls
Each stem SHALL have `volume`, `mute` and `color` (`[ChannelN_StemM]`, M = 1 to 5). Muting or changing volume SHALL ramp without clicks. A deck playing an NI stem file plays the sum of its stems. A deck playing a separated track SHALL play the original track minus the part of each stem that is turned down, so with every stem at full level it plays the original exactly.

#### Scenario: Mute vocals
- **WHEN** the vocal stem's pad is pressed
- **THEN** the vocals are muted and the pad dims

#### Scenario: Untouched stems
- **WHEN** a separated track plays with every stem at full level
- **THEN** the deck's output is the original track, sample for sample

### Requirement: Stem pads
The STEMS pad page SHALL show VirtualDJ's eight pads: Vocal, Instru, Bass and Acapella on top; Kick, HiHat, Stems FX and Instrumental below. A stem's pad SHALL mute and unmute it and be lit while it plays. Acapella SHALL play the vocals alone and Instrumental everything but the vocals, each bringing every stem back when pressed again. Stem levels SHALL be set on the mixer's knobs in stems mode, not on the pads. Right-clicking a pad SHALL offer Prepare High Quality Stems. There SHALL be no button to start a separation.

#### Scenario: Acapella
- **WHEN** Acapella is pressed on a deck with stems
- **THEN** only the vocals play and the Acapella and Vocal pads are lit

#### Scenario: Back to the full track
- **WHEN** Acapella is pressed again
- **THEN** every stem plays

## ADDED Requirements

### Requirement: Instant stems on load
When *Stems on load* is on, loading a track SHALL start a fast separation from the cue point (or the playhead) outwards. The stems for the 12 seconds around the cue point SHALL be ready within 2 seconds on an Apple Silicon Mac, and the whole of a 6-minute track within 15 seconds. Until a stretch has stems, the deck plays the original there and stem controls have no effect on it.

#### Scenario: Load and cut the vocals
- **WHEN** a track without cached stems is loaded on an M1 and the vocal pad is pressed 2 seconds later
- **THEN** the vocals are muted from the cue point on

### Requirement: High-quality stems in the background
After the fast separation, DJ Sugar SHALL separate the track again with the high-quality model in the background, nearest the playhead first, and cache the result. A later load SHALL use the cached stems at once with no fast separation. The STEMS page SHALL show whether the deck has Instant or HQ stems.

#### Scenario: Cached HQ stems
- **WHEN** a track with cached high-quality stems is loaded
- **THEN** its stems are available at once and the STEMS page shows HQ

### Requirement: Seamless swap
High-quality stems SHALL replace the fast ones stretch by stretch with a crossfade of about 20 ms, without clicks, gaps or a change in level. A stretch whose stem controls are being moved SHALL swap only once they are at rest.

#### Scenario: Swap while a stem is muted
- **WHEN** a track plays with the drums muted while its high-quality stems arrive
- **THEN** the drums stay muted, the sound gets cleaner, and there is no click

### Requirement: Separate ahead of time
The DJ SHALL be able to separate a list, crate or set in the background, one track at a time, and see which tracks have high-quality stems ready. A track being played or loaded SHALL always come first.

#### Scenario: Prepare a set
- **WHEN** Separate stems is chosen for a 20-track list
- **THEN** tracks gain a stems mark as each finishes while the decks keep playing

### Requirement: Speed
A 6-minute track SHALL finish high-quality separation in under a minute on an Apple Silicon Mac, while the app keeps drawing smoothly and the audio does not drop out.

#### Scenario: M1 timing
- **WHEN** a 6-minute track is separated on an M1 while another deck plays
- **THEN** it finishes in under 60 seconds with no audio dropouts

### Requirement: Stems settings
Settings > Stems SHALL offer *Stems on load* (Instant + high quality, High quality only, Off), the background quality, the model download state, and a *Stems cache* limit (default 10 GB) with its current use. When the cache is over the limit, the least recently loaded tracks' stems SHALL be removed first, never those of tracks in a list prepared for stems.

#### Scenario: Cache limit
- **WHEN** the stems cache passes its limit
- **THEN** the stems of the track loaded longest ago are removed until it is under the limit

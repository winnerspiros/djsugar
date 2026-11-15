## Purpose

Telling streaming, overlay, lighting and visuals software which track the audience is hearing, as a text file, a JSON file, an overlay web page and a live event feed.

## ADDED Requirements

### Requirement: Off until switched on
The Now Playing switch in Settings › Mixer SHALL be off at first. While it is off, DJ Sugar SHALL write no now-playing files and serve no now-playing pages.

#### Scenario: First launch
- **WHEN** DJ Sugar starts for the first time
- **THEN** no now-playing file is written and `/now-playing` is not served

### Requirement: The track the audience hears
The now-playing track SHALL be the track on the deck that contributes most to the master mix (its level after the channel fader and crossfader), once it has done so for the hold time (default 10 seconds, 0 to 60). A deck heard only in the headphones SHALL never count. When no deck is heard for the hold time, the feed SHALL say nothing is playing.

#### Scenario: Long blend
- **WHEN** deck 2 is faded in over a 32-bar blend and passes deck 1's level halfway through
- **THEN** the now-playing track changes to deck 2's track 10 seconds after it became the louder one

#### Scenario: Cueing in the headphones
- **WHEN** the DJ previews a track on deck 3 with its fader down
- **THEN** the now-playing track does not change

### Requirement: Files for OBS and other tools
While on, DJ Sugar SHALL keep `now-playing.txt` (one line from a template with `{artist}`, `{title}`, `{album}`, `{bpm}`, `{key}`, `{label}`; default `{artist} - {title}`) and `now-playing.json` (the current track's fields, its deck and start time, and the last ten tracks) up to date in a folder the DJ picks. Each file SHALL be replaced whole, so a reader never sees half a file.

#### Scenario: OBS text source
- **WHEN** OBS shows a Text source reading `now-playing.txt` and the track changes
- **THEN** the text changes to the new artist and title within a second of the hold time passing

### Requirement: Overlay page
While on, DJ Sugar SHALL serve an overlay page at `http://127.0.0.1:<port>/now-playing` with a clear background, in the style chosen in Settings (lower third, corner card or ticker), showing cover, title and artist and animating between tracks. `/now-playing.json` SHALL return the JSON file's content and a WebSocket at `/now-playing/ws` SHALL send it again on every change. These SHALL answer only this computer unless the DJ ticks "Allow other devices on the network", and SHALL never accept commands.

#### Scenario: OBS browser source
- **WHEN** the overlay address copied from Settings is added to OBS as a Browser Source
- **THEN** the current track shows over the stream and updates by itself

#### Scenario: Another computer
- **WHEN** a computer on the same Wi-Fi opens the overlay address and "Allow other devices" is off
- **THEN** the request is refused

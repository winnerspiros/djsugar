## Purpose

Streaming the master mix live to internet radio servers and livestream platforms, with now-playing, reconnects and a clear on-air status.

## ADDED Requirements

### Requirement: Stations
Settings › Broadcast SHALL hold a list of stations. An Icecast or Shoutcast station SHALL have a name, server type (Icecast 2, Shoutcast 1), host, port, mount point (Icecast), user, password, TLS on or off, format (MP3 by default, or Opus) and bitrate. Each station SHALL have its own on/off switch and a Test button. Settings › Broadcast SHALL say that a public station needs music licences.

#### Scenario: Add an Icecast station
- **WHEN** the DJ adds an Icecast 2 station with host `radio.example.org`, port 8000, mount `/live`, user `source` and a password, and clicks Test
- **THEN** DJ Sugar connects, reports "Connected" and disconnects

### Requirement: Station details
A station MAY also have a stream name, description, genre and website, and "List in public directories", which SHALL be off by default.

#### Scenario: Private by default
- **WHEN** a new station is added
- **THEN** it is not listed in the Icecast or Shoutcast directories

### Requirement: Secrets kept safe
Server passwords and stream keys SHALL be kept in the system keychain, shown masked in Settings, and never written to settings files, exports or logs.

#### Scenario: Settings file
- **WHEN** a station with a password has been saved
- **THEN** the password is not in DJ Sugar's settings file

### Requirement: On air
ON AIR in the top bar (`[Shoutcast],enabled`) SHALL stream the master output, mic and aux included, exactly as it is recorded, to every station that is switched on, at the same time. While on air the pill SHALL show the time on air and a light per station (connecting, connected, failed), and its panel the listener count for Icecast stations that report one. `[Shoutcast],status` SHALL be 0 off, 1 connecting, 2 connected to all, 3 failed, 4 connected to some. The ON AIR pill SHALL only show while "Broadcasting" is on in Settings › Broadcast (off by default) and at least one station is set up and switched on, or while on air from a controller.

#### Scenario: A DJ who doesn't broadcast
- **WHEN** Broadcasting is off in Settings › Broadcast
- **THEN** the top bar shows no ON AIR pill

#### Scenario: Two stations
- **WHEN** an Opus and an MP3 station are switched on and ON AIR is pressed
- **THEN** both servers receive the mix, and the panel shows two red (on air) lights and the pill the time on air

### Requirement: No dropouts from the network
Streaming SHALL never cause an audio dropout, whatever the network does: encoding and sending happen away from the audio thread, and a station that falls behind is dropped and reconnected instead of holding anything up.

#### Scenario: Slow network
- **WHEN** the upload stalls for 20 seconds while on air
- **THEN** the mix in the headphones and on the master output keeps playing without a dropout, and the station's light turns amber

### Requirement: Reconnect
When a station's connection drops or falls more than about 10 seconds behind, DJ Sugar SHALL reconnect by itself, first at once and then every 5 seconds, for up to 10 minutes, then mark the station failed and say why (wrong password, mount in use, server unreachable).

#### Scenario: Wi-Fi blip
- **WHEN** the network drops for 5 seconds while on air
- **THEN** streaming resumes by itself and the time on air keeps counting

#### Scenario: Wrong password
- **WHEN** the server rejects the password
- **THEN** the station is marked failed with "The server turned down the password", without retrying

### Requirement: Now playing
While on air, each station SHALL receive the audible track's artist and title as now-playing, changing when the incoming track becomes the louder one in a blend. Settings › Broadcast SHALL offer the text as a template (`{artist} - {title}`, `{dj}`) or a fixed text. MP3 stations SHALL send titles through the server's metadata update; Opus stations SHALL carry the first title in the stream's headers, and later titles only when the DJ chooses (a chained Ogg stream, which some players don't follow). The same now-playing SHALL go into the recording's cue sheet when recording at the same time.

#### Scenario: Blend
- **WHEN** the DJ blends from track A into track B over 32 bars
- **THEN** listeners' players show B once B's fader is above A's, not when B loads

### Requirement: Go live to platforms
Settings › Broadcast SHALL offer platform presets (Twitch, YouTube, Mixcloud Live, Facebook, Kick and Custom RTMP) that need only a stream key. A platform station SHALL stream the master with a picture DJ Sugar draws (the audible track's cover, artist, title and progress, and the DJ's name and logo), updated when the track changes. Several platforms and radio stations SHALL be able to stream at once.

#### Scenario: Twitch without OBS
- **WHEN** the DJ pastes a Twitch stream key into the Twitch preset and presses ON AIR
- **THEN** the Twitch channel goes live with the mix and the now-playing picture

### Requirement: WebRTC streaming
Settings › Broadcast SHALL offer a WHIP station (a URL and an optional token) that streams the master over WebRTC with Opus audio, for services that accept WHIP ingest.

#### Scenario: Low delay
- **WHEN** the DJ streams to a WHIP service on a normal home connection
- **THEN** listeners hear the mix less than a second behind

### Requirement: Radio show tools
A Show panel SHALL offer a show timer with a countdown to the top of the hour, station IDs and jingles played from a sampler bank, and an ON AIR light that shows while the mic is open and on air. A now-playing feed (JSON and WebSocket, read-only, off until switched on) SHALL publish the audible track for stream overlays. A show's recording SHALL be saved with chapters from its tracklist.

#### Scenario: Top of the hour
- **WHEN** the show timer reaches one minute to the hour
- **THEN** the Show panel shows a countdown, and the station ID pad is highlighted

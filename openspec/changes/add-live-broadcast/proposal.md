# Proposal

Planned for: **v0.5 · Stems, performance and switching over** for steps 1 and 2 (Broadcast meter, Icecast radio), **v0.6** for steps 3 to 5 (Go live to video platforms, WebRTC, radio show tools). Moved from v1.x, 2026-11-02, after the radio and livestream research in the project notes (`notes/radio-livestream-review.md`).

The tasks still open moved to **v0.8 · From the booth to the studio** on 2026-11-12, so v0.7 ships with what is built.

## Why

Internet radio, podcasts and DJ livestreams are a big part of why people pick Mixxx: it streams straight to Icecast with a mic, talkover and Auto DJ. But it gets the parts a broadcaster cares about wrong. Its main output has no limiter. Its meters don't read in any unit and fill up at about −11 dBFS average, so any modern track pegs them. It has no loudness (LUFS) meter, so a stream can't be set to a target. And it can't reach Twitch, YouTube or Mixcloud Live without OBS in between.

DJ Sugar already has most of what's needed underneath: a look-ahead master limiter, EBU R128 loudness measurement (used for auto gain), mic and aux inputs with talkover ducking, Automix, and a recorder that taps the master off the audio thread and knows which track is audible. Licensed DJ streaming (Mixcloud Live, Twitch DJ Program) is growing, and streaming platforms all level loudness now, so a clean stream at a known loudness matters more than a loud one.

## What Changes

1. **Broadcast meter (step 1).** A loudness meter on the master: momentary, short-term and integrated LUFS, loudness range and true peak, with targets for Internet radio (−16), Podcast / talk (−18), Livestream (−14) and Broadcast EBU R128 (−23). It lives in the MASTER panel, and in the top bar while recording or on air. The master limiter catches true peaks (between samples), not only sample peaks, with a −1 dBTP ceiling while recording or broadcasting. The peak meters keep their dBFS scale and get a scale printed beside the master meter.
2. **Icecast and Shoutcast radio (step 2).** Settings › Broadcast holds stations (server, port, mount point, user, password, TLS, format and bitrate). ON AIR in the top bar streams the master, mic included, to every station that is switched on, with the audible track's artist and title as now-playing. Formats: Opus, MP3, and AAC where the system has an encoder. Automatic reconnect, and a status pill showing time on air, a connection light per station and listener counts when the server reports them. Uses Mixxx's `[Shoutcast]` controls so controller mappings with a broadcast button work.
3. **Go live to Twitch, YouTube, Mixcloud Live and others (step 3).** Platform presets that need only a stream key. Sends RTMP with the mix and a simple video picture DJ Sugar draws itself (a "now playing" card with cover, artist, title and the DJ's name and logo), so no OBS is needed. Several destinations at once.
4. **Low-latency streaming over WebRTC (step 4).** A WHIP output (the WebRTC ingest standard, RFC 9725) for services that accept it, with Opus audio and sub-second delay. Also lays the ground for remote guest mics and back-to-back sets between two places later.
5. **Radio show tools (step 5).** A show timer with a top-of-hour countdown, station IDs and jingles from the sampler, an ON AIR light on the mic, a now-playing feed (JSON and WebSocket, through the phone remote's server) for stream overlays, and the show's recording saved with chapters from its tracklist, ready to upload.

## Capabilities

### New Capabilities

- `broadcast`: stream the master to radio servers and livestream platforms, with now-playing, reconnects and status

### Modified Capabilities

- `mixer`: a loudness meter with stream targets, a scale on the master meter, and a true-peak master limiter

## Out of Scope

- Video mixing, camera switching, scenes or overlays beyond the now-playing card. A camera feed stays a job for OBS, which can take DJ Sugar's audio and now-playing feed.
- Running a server: DJ Sugar is the source, not an Icecast server or a station automation tool (AzuraCast and LibreTime do that and accept DJ Sugar as a live DJ).
- SRT output and Media over QUIC (watch them; revisit after step 4).
- Licences for public broadcasting (SoundExchange, PRS, GEMA and so on). The Broadcast settings say that the DJ needs them for a public station.
- Recording or streaming individual decks or stems separately.
- Bundling an AAC encoder that GPL builds can't ship (libfdk-aac). AAC comes from the operating system.

## Impact

- Engine: the master tap that feeds the recorder feeds the meter and the broadcast outputs too; the limiter's detector is oversampled.
- New crate `crates/broadcast`: Ogg/Opus and MP3 encoders, an Icecast source client, then FLV/RTMP and WHIP; one thread per output, never on the audio thread.
- App: Settings › Broadcast, ON AIR pill and panel, loudness meter in the MASTER panel; settings `broadcast_stations`, `loudness_target`.
- New dependencies, all GPL-compatible: `opus`/`audiopus` (BSD), `ogg` (BSD), `mp3lame-encoder` (LAME, LGPL), `rubato` (MIT) for resampling to 48 kHz; step 3 adds an RTMP client (`rml_rtmp`, MIT) and system H.264/AAC encoders; step 4 a WebRTC stack (`str0m`, MIT).
- Passwords and stream keys go in the system keychain, not in settings files.

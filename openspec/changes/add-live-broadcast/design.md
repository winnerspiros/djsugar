# Design

The research (how Mixxx is used for radio and livestreams, how accurate its meters are, loudness standards, streaming protocols, where the use case is heading) is in the project notes, `notes/radio-livestream-review.md` (2026-11-02). This file keeps the decisions and the scope.

## Research: what others do

| App | Radio (Icecast/Shoutcast) | Video platforms | Loudness meter | Limiter |
|---|---|---|---|---|
| Mixxx 2.5 | Icecast 1/2, Shoutcast 1; MP3, Ogg Vorbis, Opus (48 kHz only), AAC if installed; several at once | No (DJs use OBS) | No; meters are mean-absolute through a non-dB curve | No (open issue #6513) |
| VirtualDJ | Icecast, Shoutcast, own servers | Built in, with video | No | Yes |
| Traktor Pro 4 | Icecast only | No | No | Yes |
| djay Pro, Serato, rekordbox | No (or through partner apps) | Through partners or OBS | No | Yes |
| OBS | Icecast through an output plugin | RTMP, SRT, WHIP | Peak meter only (LUFS through plugins) | Through filters |

Nobody in DJ software shows the stream's loudness, so step 1 is new.

## Scope and size

Sizes are for one person working with Claude Code, as the PRs in this repo have gone so far: S = 1 PR, about a day; M = 1 to 2 PRs, a few days; L = 3 or more PRs, a week or more.

| Step | What | Size | Main risk |
|---|---|---|---|
| 1. Broadcast meter | Live loudness meter (M/S/I, LRA, true peak), targets, true-peak limiter, master meter scale | **S–M** | Keeping the oversampled limiter cheap; the meter must run off the audio thread |
| 2. Icecast radio | Stations in settings, Opus/MP3/AAC encoders, Icecast source client, metadata, reconnect, ON AIR pill and status | **M** | Opus metadata (see Open questions); Linux has no AAC encoder |
| 3. Go live (RTMP) | Platform presets, FLV/RTMP client, H.264 + AAC encoding, now-playing card renderer, several destinations | **L** | Encoders differ per OS; platforms change ingest rules; real-world tests need accounts on each |
| 4. WebRTC (WHIP) | WHIP client with Opus, optional video | **M–L** | Which platforms accept WHIP for real (Twitch support is reported, not confirmed); NAT and ICE edge cases |
| 5. Radio show tools | Show timer, jingles, mic ON AIR light, now-playing feed, chaptered recording | **M** | None big; mostly UI |

Total: about 2 to 3 weeks of PRs, of which steps 1 and 2 (the part that already beats Mixxx) are about a week.

## Decisions

### One master tap for recording, metering and broadcasting

The recorder already copies the master pair into a lock-free ring buffer on the audio thread and does everything else on its own threads. The meter and each broadcast output read from the same tap. The audio thread does no more work than today, and a slow network can never cause a dropout: each output has its own bounded queue (about 10 seconds), and an output that falls behind drops its connection and reconnects rather than blocking anything.

What is streamed is exactly what is recorded: the master after the limiter, mic and aux included.

### Loudness meter

- Built from the analysis meter (`engine/src/loudness.rs`), which already follows ITU-R BS.1770 exactly. A running version adds momentary (400 ms), short-term (3 s), integrated since reset, loudness range (EBU Tech 3342) and true peak (4× oversampled, BS.1770 Annex 2).
- Integrated loudness resets when ON AIR or REC starts, and from a Reset button.
- Checked against the EBU Tech 3341 and 3342 test signals (free to download) in tests, not only the one sine the analysis test uses.
- Targets are presets, not free numbers: Internet radio −16 LUFS, Podcast / talk −18, Livestream −14, Broadcast (EBU R128) −23. The meter is green within ±1 LU of the target, amber 1 to 3 LU off, red beyond. Defaults to Internet radio.
- Auto gain keeps its own target. The broadcast target does not change track levels; it tells the DJ where the mix sits. (Linking them is an open question.)

### True-peak limiter

- The limiter's detector is oversampled 4× so it sees peaks between samples. The gain is still applied at the normal rate, so the sound only changes when it would otherwise overshoot.
- Ceiling −1 dBTP while recording or on air, −0.5 dBFS otherwise as today. A setting allows −2 dBTP for low-bitrate streams.
- The peak meters stay sample peak in dBFS (honest and cheap) with a printed scale (−40, −20, −10, −6, −3, 0). Their fall rate slows from 60 dB/s to about 20 dB/s so they're readable, close to a DIN PPM.

### Encoders and licences

DJ Sugar builds are GPL-3.0, so every encoder must be GPL-compatible:

| Format | Source | Platforms |
|---|---|---|
| Opus (in Ogg for Icecast) | libopus, BSD | All. Needs 48 kHz: resampled by DJ Sugar's own polyphase resampler when the engine runs at 44.1 kHz |
| MP3 | LAME, LGPL | All |
| AAC (LC and HE-AAC) | The operating system: AudioToolbox on macOS, Media Foundation on Windows | macOS, Windows. Not Linux: libfdk-aac can't ship in a GPL app; FFmpeg's encoder may be used when the system has FFmpeg (later) |
| H.264 (step 3) | VideoToolbox (macOS), Media Foundation (Windows), OpenH264 (Linux, Cisco's binary fetched on first use as Firefox does, which covers the patent licence) | All |

Default for Icecast: MP3 128 kb/s, because it plays everywhere and Icecast shows its titles; Opus is offered for the best quality per bit (96 kb/s). (Step 2 changed this from Opus: Icecast 2.4 doesn't show titles for Ogg mounts, and players built on FFmpeg 6 stop at chained Ogg streams.) Default for RTMP: AAC 160 kb/s (what every platform accepts). On Linux RTMP sends Opus through Enhanced RTMP where the platform accepts it; see Open questions.

### Now-playing metadata

- The track counted as audible is the one the recorder's cue sheet already uses (fader up past the audible threshold), and the title changes when the incoming track becomes the louder one in a blend, not at load.
- The text is "Artist – Title" by default, with a template (`{artist} – {title}`, `{dj}`), and a fixed text option for privacy ("Live with DJ Example").
- MP3 and AAC: Icecast's metadata update. Opus/Ogg: see Open questions.
- The same now-playing goes to the step-5 feed and into the recording's cue sheet, so all three agree.

### Video for platforms that need it (step 3)

Twitch and YouTube reject audio-only streams. DJ Sugar sends a 1280×720 picture it draws itself with Slint's software renderer: cover art, artist, title, a progress bar and the DJ's name and logo, at a low frame rate (it changes only when the track does), so encoding costs next to nothing. This is not video mixing (out of scope by project decision); it is a picture for the audio.

### Credentials

Server passwords and stream keys are kept in the system keychain (macOS Keychain, Windows Credential Manager, Secret Service on Linux, through the `keyring` crate), never in settings files or logs, and are masked in Settings. Stations can be exported without their secrets.

### Controls

Mixxx's names where they exist, so mappings with a broadcast button work:

- `[Shoutcast],enabled` (ON AIR on or off) and `[Shoutcast],status` (0 off, 1 connecting, 2 connected, 3 failed, 4 some stations failed).
- DJ Sugar's own: `[Master],loudness_momentary`, `[Master],loudness_short_term`, `[Master],loudness_integrated`, `[Master],loudness_range`, `[Master],true_peak`, `[Master],loudness_reset`, `[Broadcast],on_air_seconds`.

## Open questions

1. **Opus metadata on Icecast.** Updating the title inside an Ogg stream starts a new chained stream, which makes some players drop or glitch (Mixxx turns it off by default for this reason). Options: chain anyway (works in browsers, VLC and most apps), or send MP3 alongside for directory listings. Proposed: chain, with a setting to turn it off, and test with the main players before step 2 merges.
   **Decided in step 2:** tested against Icecast 2.4.4 and FFmpeg 6.1. Icecast 2.4 refuses title updates for Ogg mounts ("Mountpoint will not accept URL updates") and doesn't read titles inside Opus streams for its status page; FFmpeg 6.1 fails at the first chained stream. So the first title always goes into the Opus stream's headers, later titles only with "Send each new title in the stream" (off by default), and new stations default to MP3, whose titles every player shows.
2. **Which platforms accept what in step 3.** Mixcloud Live may accept audio-only streams; Enhanced RTMP with Opus may or may not be accepted by Twitch and YouTube. To be tested with real accounts before step 3; the presets follow the results.
3. **Linux AAC.** Use the system's FFmpeg when present, or Opus-only on Linux? Proposed: Opus and MP3 only on Linux for step 2, and decide at step 3.
   **Step 2:** Opus and MP3 on every system; AAC (AudioToolbox, Media Foundation) moves to step 3, where RTMP needs it anyway.
4. **Link the broadcast target to auto gain?** A "match auto gain to the stream target" switch would set auto gain 1 to 2 LU under the target (transitions and the mic add loudness). Proposed: not in step 1; see how DJs use the meter first.
5. **Where step 5's now-playing feed lives.** In the phone remote's server (same port, its own read-only path) or its own server. Proposed: the remote's server, read-only, no pairing needed, off unless switched on.

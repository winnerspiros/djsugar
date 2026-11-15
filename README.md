# DJ Sugar

![DJ Sugar logo](https://raw.githubusercontent.com/winnerspiros/djsugar/main/res/images/icons/scalable/apps/mixxx.svg)

**Open-source DJ software for Android, iOS, Windows, macOS and Linux.**

DJ Sugar is a fork of [Mixxx](https://github.com/mixxxdj/mixxx) that adds streaming services,
on-device stem separation, a touch interface and mobile builds. It started from the Android port
[JaviLabarumDJ/mixxx-android](https://github.com/JaviLabarumDJ/mixxx-android) and merges upstream
Mixxx regularly, so every Mixxx feature and controller mapping is included.

## Downloads

Every push to `main` builds all platforms and publishes a pre-release on the
[Releases page](https://github.com/winnerspiros/djsugar/releases).

| Platform | Architecture | File |
| --- | --- | --- |
| Android | arm64 | `.apk` |
| iOS / iPadOS 14+ | arm64 | `.ipa` (unsigned) |
| Windows | x64, ARM64 | `.msi` |
| macOS 11+ | Intel, Apple Silicon | `.dmg` |
| Ubuntu / Debian | x86_64 | `.deb` |
| Any Linux | x86_64 | `.AppImage` |

<details>
<summary>Installing the iOS build</summary>

The `.ipa` is not signed, because there is no paid Apple developer account behind it. Install it
with [AltStore](https://altstore.io) or [Sideloadly](https://sideloadly.io): they sign it with your
own Apple ID. With a free Apple ID the app must be re-signed every 7 days; AltStore does that
automatically.

On iOS, YouTube search works but yt-dlp does not (iOS apps cannot run other programs), and there is
no HID/USB controller support yet.

</details>

## Features

### Streaming

- YouTube: search, load to a deck or save to the library, SponsorBlock skips non-music parts
- SoundCloud: search with genre, cover art and play counts
- Spotify (Android, Premium): search, Liked Songs and playlists, played as real files
- Subsonic servers and custom stream URLs

### Performance

- FX Station: drum pads and effect scenes on top of the decks
- Built-in sample library, your own recordings and a sample editor
- On-device stem separation (Spleeter or HTDemucs)
- Smart Auto DJ: chooses the next track by energy, key and phrase and mixes it in on the beat

### Hardware

- Pioneer DDJ-FLX4 over USB and Bluetooth MIDI, detected automatically
- USB audio interfaces and MIDI learn on Android, no root needed
- Rekordbox USB drives: read playlists and back them up

### Mobile

- Touch skins (Lite and Booth) with a compact library and a streaming hub
- Low-latency audio through Oboe, latency calibration with the microphone
- Fast library on large phone collections, Greek and Greeklish search

<details>
<summary>How it works</summary>

- **Language and UI**: C++20 with Qt 6.12. The mobile interface is QML; desktop keeps the Mixxx
  widget skins and the QML interface.
- **Streaming**: YouTube goes through yt-dlp (bundled on Android, self-updating) and downloads each
  file in 16 parallel ranges. Spotify uses [librespot](https://github.com/librespot-org/librespot)
  (Rust) and caches the Ogg Vorbis 320 kbit/s stream, so waveforms, beatgrids, loops and scratching
  work as with local files.
- **Stems**: Spleeter and HTDemucs models run through ONNX Runtime on the device; results are saved
  as stem files next to the track.
- **Audio on Android**: Oboe for the system audio path; USB audio interfaces are driven directly
  over USB from user space.
- **Library**: SQLite in WAL mode, with a case- and accent-insensitive collation for sorting and
  search.
- **Assets**: images ship as WebP, SVGs are optimized, bundled samples are Opus where that is
  smaller.
- **Release builds**: link-time optimization and removal of unused code on all platforms.

</details>

## Building

DJ Sugar builds with CMake and vcpkg, the same way as Mixxx. The general steps are in the
[Mixxx build instructions](https://github.com/mixxxdj/mixxx/blob/main/CONTRIBUTING.md).

<details>
<summary>Platform details</summary>

- **Dependencies**: prebuilt per platform in
  [winnerspiros/vcpkg](https://github.com/winnerspiros/vcpkg) releases (Qt 6.12, FFmpeg, ONNX
  Runtime and the rest). `tools/<platform>_buildenv.sh setup` downloads the right bundle and checks
  its SHA256.
- **Android**: cross-compiled for arm64 with the NDK, then packaged with Gradle. Spotify support
  needs Rust: `rustup target add aarch64-linux-android`.
- **iOS**: cross-compiled from macOS with `-DCMAKE_SYSTEM_NAME=iOS`, the `arm64-ios-release`
  triplet and a host Qt for the build tools (`QT_HOST_PATH`); packaged as an unsigned IPA.
- **CI**: [`.github/workflows/build.yml`](.github/workflows/build.yml) builds and tests every
  platform and is the reference for the exact CMake flags of each one.
  [`auto-release.yml`](.github/workflows/auto-release.yml) publishes the releases.

</details>

## Bug reports

[Open an issue](https://github.com/winnerspiros/djsugar/issues/new/choose) and attach `mixxx.log`.

<details>
<summary>Where to find mixxx.log</summary>

- Android: `Android/data/org.mixxx/files/Documents/Mixxx/`
- Linux: `~/.local/share/mixxx/`
- Windows: `%LOCALAPPDATA%\Mixxx\`
- macOS: `~/Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx/`

</details>

## Contributing

Fork, branch and open a pull request. Code style and guidelines are in
[CONTRIBUTING.md](CONTRIBUTING.md).

## License and credits

GPLv2, see [LICENSE](LICENSE).

- [Mixxx](https://github.com/mixxxdj/mixxx), copyright the Mixxx contributors
- [mixxx-android](https://github.com/JaviLabarumDJ/mixxx-android) by JaviLabarumDJ
- [yt-dlp](https://github.com/yt-dlp/yt-dlp), [SponsorBlock](https://sponsor.ajay.app),
  [librespot](https://github.com/librespot-org/librespot) and
  [ONNX Runtime](https://github.com/microsoft/onnxruntime)

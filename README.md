# 🍬 DJ Sugar

![DJ Sugar logo](https://raw.githubusercontent.com/winnerspiros/djsugar/main/res/images/icons/scalable/apps/mixxx.svg)

**Free, open-source DJ software for phones, tablets and desktops.**

DJ Sugar is a fork of [Mixxx](https://github.com/mixxxdj/mixxx), built on the Android port
[JaviLabarumDJ/mixxx-android](https://github.com/JaviLabarumDJ/mixxx-android). Everything Mixxx does
is still here. DJ Sugar adds streaming, a touch interface and mobile builds, and merges upstream Mixxx
regularly.

## 📥 Downloads

Every push to `main` publishes a pre-release on the
[Releases page](https://github.com/winnerspiros/djsugar/releases).

| Platform | File |
| --- | --- |
| 🤖 Android (ARM64) | `.apk` |
| 🍎 iOS / iPadOS (ARM64) | `.ipa`, unsigned: install with AltStore or Sideloadly |
| 🪟 Windows (x64, ARM64) | `.msi` |
| 🍏 macOS (Intel, Apple Silicon) | `.dmg` |
| 🐧 Ubuntu / Debian (x86_64) | `.deb` |
| 🐧 Any Linux (x86_64) | `.AppImage` |

## ✨ What DJ Sugar adds

### 🎵 Streaming

- **YouTube**: search in the app, load to deck 1 or 2, or save to the library. Fast parallel
  downloads, genre bubbles, BPM / key / played markers, SponsorBlock skips non-music parts.
  yt-dlp is bundled on Android and updates itself (not available on iOS).
- **SoundCloud**: search with genre, cover art and play counts. Go+ tracks are 30-second previews.
- **Spotify** (Android, Premium): log in through the Spotify app, then search, Liked Songs and
  playlists. Tracks load as real files through [librespot](https://github.com/librespot-org/librespot),
  so waveforms, beatgrids, loops and scratching work.
- **Subsonic** servers and custom stream URLs.

### 🎛️ Performance

- **FX Station**: drum pads and effect scenes on top of the decks.
- **Samples**: built-in sample library, plus your own recordings with a simple editor.
- **Stems**: split any track into stems on the device (Spleeter or HTDemucs, ONNX Runtime).
- **Smart Auto DJ**: picks the next track by energy, key and phrase, and lines the mix up on the
  beat.

### 🎚️ Controllers and hardware

- **Pioneer DDJ-FLX4** over USB and Bluetooth (BLE MIDI), detected automatically.
- All of Mixxx's controller mappings, plus MIDI learn on Android.
- USB audio interfaces on Android without root.
- Rekordbox USB drives: read playlists and back them up.

### 📱 Mobile

- Touch skins (Lite and Booth) with a compact library and a streaming hub.
- Low-latency audio through Oboe, latency calibration with the mic.
- Fast library on large phone collections, Greek and Greeklish search.

## 🛠️ Building

DJ Sugar uses the same CMake + vcpkg setup as Mixxx: see the
[Mixxx build instructions](https://github.com/mixxxdj/mixxx/blob/main/CONTRIBUTING.md).
Prebuilt dependencies for every platform come from
[winnerspiros/vcpkg](https://github.com/winnerspiros/vcpkg) releases; the `tools/*_buildenv.sh`
scripts download them. [`.github/workflows/build.yml`](.github/workflows/build.yml) is the reference
for the exact steps of each platform, including Android and iOS.

Spotify on Android needs Rust: `rustup target add aarch64-linux-android`.

## 🐞 Bug reports

[Open an issue](https://github.com/winnerspiros/djsugar/issues/new/choose) and attach `mixxx.log`:

- Android: `Android/data/org.mixxx/files/Documents/Mixxx/`
- Linux: `~/.local/share/mixxx/`
- Windows: `%LOCALAPPDATA%\Mixxx\`
- macOS: `~/Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx/`

## 🤝 Contributing

Fork, branch, open a pull request. Code style and guidelines: [CONTRIBUTING.md](CONTRIBUTING.md).

## 📜 License and credits

GPLv2: see [LICENSE](LICENSE).

- [Mixxx](https://github.com/mixxxdj/mixxx), copyright the Mixxx contributors
- [mixxx-android](https://github.com/JaviLabarumDJ/mixxx-android) by JaviLabarumDJ, the Android port
  DJ Sugar builds on
- [yt-dlp](https://github.com/yt-dlp/yt-dlp), [SponsorBlock](https://sponsor.ajay.app) and
  [librespot](https://github.com/librespot-org/librespot)

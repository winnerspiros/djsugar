# 🍬 DJ Sugar

![DJ Sugar logo](https://raw.githubusercontent.com/winnerspiros/djsugar/main/res/images/icons/scalable/apps/mixxx.svg)

**Free, open-source DJ software — on your phone and on your desktop.**

DJ Sugar is a fork of [Mixxx](https://github.com/mixxxdj/mixxx) 🎛️, built on the Android port by
[JaviLabarumDJ/mixxx-android](https://github.com/JaviLabarumDJ/mixxx-android) 📱. Everything Mixxx
does stays; DJ Sugar adds streaming sources and a touch UI made for phones. Upstream Mixxx is merged
in regularly.

---

## ✨ What DJ Sugar adds

### 📺 YouTube

- Search inside the app and load results straight to deck 1 or deck 2 (D1 / D2), or save them to the library
- Genre bubbles to browse, Greek genres included 🇬🇷
- Played, BPM, key and duplicate markers on results
- Fast downloads: yt-dlp finds the audio link, then the file comes in 16 parallel parts
- The links of the top results are fetched in the background, so a tap usually starts the download at once
- yt-dlp is bundled on Android and updates itself
- **SponsorBlock** ⏭️ skips sponsor segments, intros and other non-music parts

### ☁️ SoundCloud

- Search with genre, cover art and play counts
- D1 / D2 / save buttons, same as YouTube
- Go+ tracks are marked; they are 30-second previews and can't be loaded

### 🎚️ Controllers

- **Pioneer DDJ-FLX4** over USB and Bluetooth (BLE MIDI), picked up automatically when connected
- Jog wheels with scratch (vinyl mode on by default), browse, load, back and stems mode
  (SHIFT + SAMPLER: pads 1–4 mute, pads 5–8 solo)
- BROWSE and LOAD work on whichever screen is open: library, YouTube or SoundCloud
- All of Mixxx's controller mappings are included

### 📱 Android

- Touch UI with a compact library and a streaming hub
- Low-latency audio through Oboe
- USB controller permission handling
- Renderer choice in Settings → Interface

### 🤖 AI Bro

- Auto-DJ that finds similar songs on YouTube, downloads them and mixes them in

---

## 📥 Downloads

Get the latest build from [Releases](https://github.com/winnerspiros/djsugar/releases). Every push to
`main` publishes a new pre-release.

| Platform | File |
| --- | --- |
| 🤖 Android (ARM64) | `.apk` |
| 🪟 Windows (x64, ARM64) | `.msi` |
| 🍎 macOS (Intel, Apple Silicon) | `.dmg` |
| 🐧 Ubuntu / Debian (x86_64) | `.deb` |

## 🛠️ Building

DJ Sugar uses the same CMake + vcpkg setup as Mixxx: follow the
[Mixxx build instructions](https://github.com/mixxxdj/mixxx/blob/main/CONTRIBUTING.md). The Android
APK is built by the Android job in [`.github/workflows/build.yml`](.github/workflows/build.yml), the
reference for the exact steps.

## 🐞 Bug reports

[Open an issue](https://github.com/winnerspiros/djsugar/issues/new/choose) and attach `mixxx.log`:

- 🤖 Android: `Android/data/org.mixxx/files/Documents/Mixxx/`
- 🐧 Linux: `~/.local/share/mixxx/`
- 🪟 Windows: `%LOCALAPPDATA%\Mixxx\`
- 🍎 macOS: `~/Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx/`

## 🤝 Contributing

Fork, branch, open a pull request. Code style and guidelines: [CONTRIBUTING.md](CONTRIBUTING.md).

## 📜 License and credits

GPLv2 — see [LICENSE](LICENSE).

- [Mixxx](https://github.com/mixxxdj/mixxx), copyright the Mixxx contributors
- [mixxx-android](https://github.com/JaviLabarumDJ/mixxx-android) by JaviLabarumDJ, the Android port DJ Sugar builds on
- [yt-dlp](https://github.com/yt-dlp/yt-dlp) and [SponsorBlock](https://sponsor.ajay.app)

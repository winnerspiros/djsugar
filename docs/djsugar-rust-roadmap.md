# DJ Sugar Rust rewrite: roadmap and tracker

DJ Sugar is an independent project: we own this code going forward and are not affiliated with
openDJ or Mixxx. Base: openDJ (github.com/openwave-studio/opendj, GPL-2.0-or-later), imported with
full history; GPL requires keeping LICENSE and the inherited notices in NOTICE.
Reference implementation: DJ Sugar C++ on `main` (Mixxx fork). Every DJ Sugar feature is ported
here; Mixxx code is read for behaviour, not translated line by line.

Status: TODO, WIP, DONE. All work lands on `rust`.

## Overview

Timeframes are estimates for the work itself; P0 is now, P3 is later. "Input" is what needs the owner.

| Part | Done | Priority | Timeframe | Input |
| --- | --- | --- | --- | --- |
| 0: platforms | 15% (1.5/10) | P0 | 4-6 weeks | store accounts, signing keys |
| P: per-platform, per-device, per-driver tuning | 0% (0/8) | P1 | 12-20 weeks, ongoing | device testing |
| Q: code quality and performance | 0% (0/9) | P0 | ongoing | no |
| R: pure-Rust dependencies | 15% (2/13) | P1 | 8-12 weeks | no |
| C: controllers and DVS, every platform | 0% (0/8) | P1 | 10-16 weeks | hardware tests |
| 1: DJ Sugar features | 0% (0/19) | P1 | 8-12 weeks | UI/feature review |
| 2: Mixxx parity | 0% (0/6) | P1 | 6-10 weeks | no |
| X: library import/export/sync | 0% (0/9) | P1 | 10-14 weeks | no |
| T: mapping translator | 0% (0/3) | P2 | 4-6 weeks | no |
| M: mods | 0% (0/4) | P2 | 6-8 weeks | no |
| L: lighting and visuals (later) | 0% (0/4) | P3 | 12+ weeks | yes, scope |
| A: Automix (later, design discussion required before starting) | 0% (0/1) | P3 | tbd | yes, design first |

## Decisions

- Latest stable toolchain, edition and libraries; clean, safe, optimal code. Comments only where the code can't say it.
- Optimal results are a hard rule: per-platform and per-device optimization (CPU features via runtime dispatch, GPU paths, audio buffer tuning per device, mobile power/thermal profiles); optimization continues until nothing measurable is left.
- Minimal resources and dependencies: write our own where a dependency is heavy or broad; existing work may serve as a starting point.
- Rust everywhere possible: app, build tooling (xtask, not shell/Python), mods and their SDK. Non-Rust only where a platform forces it (minimal Android/iOS glue) or for compatibility (JS mappings).

- Base on openDJ instead of translating Mixxx: it already has the engine (cpal, symphonia,
  signalsmith-stretch), Mixxx controller mappings (midir, hidapi, rquickjs), library with Mixxx
  import (rusqlite), wgpu waveforms, Slint UI, stems, plugin hosting.
- UI: Slint (supports Android and iOS). Built binaries are GPL-3.0 because of Slint.
- Database: rusqlite (bundled SQLite). Turso is a later drop-in candidate once it supports custom
  collations and functions; stoolap is not SQLite-file compatible and is not used.
- vcpkg fork: only kept for C/C++ dependencies that have no Rust replacement (branch `rust`).
- No obligation to stay mergeable with openDJ; cherry-pick useful upstream fixes when worth it.

## Goal: optimal Rust, not a port

The end result is idiomatic, fast, lean Rust, not translated C++ or a copy of openDJ. Every part
that is ported or touched must meet these rules; inherited code is brought up to them over time.

- Real-time audio thread: no allocation, no locks, no syscalls, no logging, no panics. Lock-free
  communication only (rtrb/atomics). Checked with an allocation-failing test allocator
  (assert_no_alloc) on the engine callback.
- Performance is measured, not assumed: criterion benchmarks for DSP, decoding, analysis, library
  queries and waveform generation; regressions fail CI. Profile (perf/samply) before optimising.
- SIMD where it pays (std::simd/portable or wide/pulp); no per-sample virtual dispatch.
- unsafe only where unavoidable (FFI, proven hot spots), each block with a `// SAFETY:` comment;
  unsafe count tracked and driven down. Miri runs on unsafe-containing pure crates.
- Strong types over primitives (sample rates, frame positions, BPM, keys as newtypes); errors as
  enums (thiserror), no `unwrap` in non-test code outside proven-infallible spots.
- Workspace lints: clippy pedantic + nursery selected, `unsafe_op_in_unsafe_fn`, `missing_docs` for
  public APIs; `cargo deny` for licences/advisories; `cargo machete`/udeps for unused deps.
- Lean dependency tree: every crate justified; prefer std and small crates; feature-gate heavy
  ones (ML, plugin hosting) so mobile builds only carry what they use.
- Release profile: LTO (fat), codegen-units = 1, panic = abort in the app binary, strip; PGO
  for the engine once benchmarks exist. Track binary size and startup time per platform.
- Async only for I/O (streaming, downloads); never on the audio or UI thread.
- Tests: unit + property tests (proptest) for DSP and parsers, golden-file tests for analysis
  vs the C++ reference, fuzzing (cargo-fuzz) for decoders, tag and playlist parsers.

- One app on every platform: identical features, functions, layouts/skins and settings on
  Android, iOS, Windows, macOS and Linux. Platform code is limited to thin backends (audio API,
  USB/MIDI/HID access, file pickers, packaging) behind shared traits; no platform-only features
  unless the OS makes them impossible, and each such gap is listed in the parity table below.
  Layouts adapt to screen size and input (touch/mouse), not to the OS.

Platform gaps the OS forces (keep this list short and current):

| Gap | Platforms | Reason |
| --- | --- | --- |
| yt-dlp subprocess | iOS | iOS apps cannot spawn processes; needs an in-process extractor |

Baseline inherited from openDJ (2026-11-15): 173 `unsafe` occurrences, 1122 crates in Cargo.lock,
no tuned release profile, no workspace lint table. Part Q tracks closing these gaps.

## Part Q: code quality and performance

| # | Item | Status |
| --- | --- | --- |
| Q.1 | Release profile (LTO, codegen-units 1, panic abort, strip), size/startup tracking | TODO |
| Q.2 | Workspace lint table, clippy pedantic triage, `cargo deny` + machete in CI | TODO |
| Q.3 | Audio-callback no-alloc/no-lock guard (assert_no_alloc) + audit of engine thread | TODO |
| Q.4 | criterion benchmark suite + CI regression check | TODO |
| Q.5 | unsafe audit: justify, wrap or remove each block; Miri on pure crates | TODO |
| Q.6 | Dependency diet: remove/feature-gate, target well below the inherited 1122 crates | TODO |
| Q.7 | SIMD pass on hot DSP (EQ, filters, mixing, waveform summaries) after profiling | TODO |
| Q.8 | Fuzzing for decoders, tags, playlist and library import parsers | TODO |
| Q.9 | PGO build for release engine | TODO |

## Part 0: platforms

| # | Part | Status |
| --- | --- | --- |
| 0.0 | Rebrand: crate names, app id, binary name, env vars, data dirs, icons, .github workflows/templates, packaging (flatpak id), openspec | DONE |
| 0.1 | CI: Linux, Windows, macOS desktop builds on `rust` | TODO |
| 0.2 | Android arm64 build: all crates check in CI (cargo-ndk), winit + FemtoVG wgpu backend, `djsugar-android` cdylib with `android_main`; APK packaging next | WIP |
| 0.3 | Android audio: cpal AAudio backend, low-latency stream, Oboe parity | TODO |
| 0.4 | Android storage and permissions (music folders, SAF) | TODO |
| 0.5 | iOS arm64 build and unsigned IPA | TODO |
| 0.6 | iOS audio (CoreAudio via cpal), file access | TODO |
| 0.7 | Windows ARM64, AppImage, .deb, .dmg, .msi packaging | TODO |
| 0.8 | Store builds: cargo feature `store` drops YouTube/SoundCloud download and librespot (Play/F-Droid policy); full build ships on GitHub releases; About screen links source (GPL) | TODO |
| 0.9 | Mixcloud sign-in server: Rust token-swap service (holds client secret); app reads its URL from `DJSUGAR_MIXCLOUD_SIGN_IN` until hosted | TODO |

## Part P: per-platform, per-device, per-driver tuning

Every backend picks the fastest path the hardware offers, decided at runtime from a device profile database (manufacturer, model, SoC, GPU, driver version, OS version). Vendor SDKs are reimplemented in Rust where their interfaces allow.

| # | Part | Status |
| --- | --- | --- |
| P.1 | Device profile database (Rust, versioned data file): detection on each OS, overrides per manufacturer/model/driver, benchmark-on-first-run to fill gaps | TODO |
| P.2 | Graphics: wgpu backend choice per device (Vulkan, Metal, DX12, GLES fallback), Vulkan extensions and driver workarounds (Adreno, Mali, Xclipse, PowerVR, NVIDIA, AMD, Intel), present mode, frame pacing, swapchain image count, HDR/120 Hz | TODO |
| P.3 | Android audio: AAudio exclusive + low-latency performance mode, MMAP path, burst-size buffers, sustained-performance and CPU affinity hints, per-OEM quirks (Samsung, Pixel, Xiaomi, OnePlus...), USB audio class direct path | TODO |
| P.4 | Desktop audio: WASAPI exclusive/event, ASIO (Rust host), CoreAudio HAL with IO cycle tuning, ALSA direct/PipeWire native with quantum negotiation, JACK; per-interface buffer/latency profiles | TODO |
| P.5 | Vendor SDK features in Rust where possible: Samsung Professional Audio / low-latency extensions, Qualcomm/MediaTek performance hints (ADPF), Apple Audio Workgroups, Windows MMCSS / Game Mode | TODO |
| P.6 | CPU: runtime SIMD dispatch (SSE4.2/AVX2/AVX-512, NEON/SVE/SME), big.LITTLE-aware thread placement, real-time thread priority per OS | TODO |
| P.7 | Power/thermal profiles on mobile and laptops; automatic quality scaling without audio dropouts | TODO |
| P.8 | Per-device latency and glitch test suite in CI and on-device; results feed P.1 | TODO |

## Part X: library import, export and sync

All in the app, both directions, no external tool: tracks, playlists/crates, cues, loops, beatgrids, keys, ratings, play history, colours.

| # | Part | Status |
| --- | --- | --- |
| X.1 | Common library model and lossless conversion layer (Rust); dry-run preview and conflict report | TODO |
| X.2 | Rekordbox: XML and the encrypted master.db (6/7), device exports (PDB/ANLZ on USB) | TODO |
| X.3 | Serato: crates, ID3/MP4/FLAC GEOB markers, Serato Stems | TODO |
| X.4 | Traktor: collection.nml with cues, grids and playlists | TODO |
| X.5 | Engine DJ: database and USB exports | TODO |
| X.6 | VirtualDJ: database.xml and playlists; Mixxx (exists, extend to export); djay; iTunes/Music XML | TODO |
| X.7 | Path relinking for moved drives and other OSes; duplicate detection by audio fingerprint | TODO |
| X.8 | Two-way sync: keep a DJ Sugar library and another app's library in step, incremental | TODO |
| X.9 | Export to USB for club players (CDJ/XDJ via X.2 formats) | TODO |

## Part M: mods

One install mechanism for user add-ons: zip with `mod.toml` (id, version, kind, permissions), installed from file or URL, listed/toggled in Settings > Mods.

| # | Part | Status |
| --- | --- | --- |
| M.1 | Data mods: sample packs, skins (Slint themes/colors/assets), controller mappings, effect presets. No code. | TODO |
| M.2 | Source mods: written in Rust, compiled to WASM (wasm32-wasip2, wasmtime/wasmi), with a `djsugar-mod` SDK crate; implementing a `MusicSource` interface (search, browse, resolve-to-audio); host gives HTTP + cache only via declared permissions. YouTube, SoundCloud, Spotify become mods instead of built-ins. | TODO |
| M.3 | Controller mods in Rust (WASM) too; rquickjs kept only to run existing Mixxx/openDJ JS mappings, never for new code. Native plugins desktop-only (plugin-host crate). | TODO |
| M.4 | Store policy: Play forbids downloading native code; WASM/JS interpreted in a sandbox is the allowed path. Store build ships no source mods and no built-in mod catalog URL. | TODO |

## Part R: pure-Rust dependencies

Non-Rust code in the current tree (from `cargo tree`), and its replacement. OS-layer C libraries are audited in R.11; only true OS ABIs remain.

| # | C/C++ dependency | Replacement | Status |
| --- | --- | --- | --- |
| R.1 | rquickjs (QuickJS, C) | boa (pure-Rust JS engine) for legacy JS mappings | TODO |
| R.2 | openssl-sys (pulled in by libsqlite3-sys for SQLCipher, to read encrypted Rekordbox databases) | goes with R.3: pure-Rust SQLCipher-compatible page decryption (AES-256-CBC + HMAC via RustCrypto) | TODO |
| R.3 | libsqlite3-sys (diesel/rusqlite) | pure-Rust DB (turso/limbo or equivalent) with migration of existing library DBs; scheduled late | TODO |
| R.4 | signalsmith-stretch (C++) | port to Rust in-tree | TODO |
| R.5 | whisper-rs-sys (whisper.cpp) | Whisper on burn/candle (burn already a dependency) | TODO |
| R.6 | mp3lame-sys | pure-Rust MP3 encoder (port shine/LAME core) | TODO |
| R.7 | opusic-sys | pure-Rust Opus encoder (evaluate existing crates, else port) | TODO |
| R.8 | hidapi | Linux: own hidraw backend (DONE); macOS IOKit and Windows HID in Rust next | WIP |
| R.9 | yeslogic-fontconfig-sys | fontique/fontdb font discovery | TODO |
| R.10 | renderdoc-sys | debug-only feature, off by default | TODO |
| R.11 | alsa-sys (libasound), wayland-sys, other OS-layer C libs | audit: talk to kernel ALSA ioctls / PipeWire protocol directly; wayland-client pure-Rust backend (no libwayland); keep only true OS ABIs (CoreAudio, WASAPI, AAudio) | TODO |
| R.12 | rusty_link (Ableton Link, C++) | `djsugar-link`: pure-Rust implementation of the Link session protocol (discovery, clock measurement, session join, timeline sharing) | DONE (needs a live test against Ableton Live/Traktor) |
| R.13 | burn 0.20 (a second wgpu, v26, via cubecl; ndarray CPU backend with zstd-sys) | DONE: burn 0.22 with backend-erased tensors and a runtime device; one wgpu (30) shared with the UI; pure-Rust flex CPU backend (no zstd-sys); demucs-core now a workspace member, its 58 tests run in CI. Left: compare stems against the previous build on a real mix | WIP |

## Part T: mapping translator

Converts existing Mixxx/openDJ controller mappings (XML + JS) into DJ Sugar Rust mods.

| # | Part | Status |
| --- | --- | --- |
| T.1 | Parser: XML mapping + JS (oxc/swc, pure Rust) into a typed IR | TODO |
| T.2 | Codegen: IR to Rust source using the `djsugar-mod` SDK; builds to WASM with the normal toolchain (xtask). Used to convert the bundled mappings once. | TODO |
| T.3 | In-app import: phones and stores cannot run rustc, so the app compiles the IR directly to a sandboxed, interpretable form (no JS engine needed at runtime). Unsupported constructs fall back to boa and are reported. | TODO |

## Part C: controllers and DVS, every platform

Target: every controller that has a mapping anywhere works on Linux, Windows, macOS, Android and iOS, over USB and Bluetooth LE.

| # | Part | Status |
| --- | --- | --- |
| C.1 | Transport layer in Rust: USB MIDI (midir; Android `android.media.midi` via JNI, iOS CoreMIDI), USB HID (R.8), BLE MIDI (btleplug, pure Rust: BlueZ, WinRT, CoreBluetooth, Android) with pairing UI and latency/jitter compensation | TODO |
| C.2 | Hotplug and auto-detect on every OS (by USB VID/PID, BLE name/service), mapping picked automatically | TODO |
| C.3 | Coverage: every bundled mapping loads and runs in the test suite; per-controller status list generated from tests, not hand-written | TODO |
| C.4 | Controllers with built-in audio interfaces (multichannel I/O routing: main, booth, headphones per device) | TODO |
| C.5 | DVS timecode decoder in Rust: Serato CV02 (Control Vinyl/CD), Traktor MK1/MK2, Mixvibes, with pitch, direction, absolute position, needle drop, relative/absolute modes, per-deck calibration | TODO |
| C.6 | DVS interfaces: Rane SL1/SL2/SL3/SL4, Rane Seventy/Seventy-Two/Twelve, Pioneer DJM/Serato-certified mixers, Traktor Audio 6/10 and similar; class-compliant multichannel, phono/line switching | TODO |
| C.7 | Rane Twelve/One style motorized/MIDI platters, jog displays (HID screens on S4 MK3, DDJ-REV7 etc.) | TODO |
| C.8 | Controller-without-mapping fallback: MIDI learn wizard builds a Rust mapping mod | TODO |

## Part L: lighting and visuals (later)

| # | Part | Status |
| --- | --- | --- |
| L.1 | DMX output in Rust: USB interfaces (Enttec Open/Pro, DMXKing), Art-Net, sACN (E1.31) | TODO |
| L.2 | Fixture library (Open Fixture Library format import), patching, groups | TODO |
| L.3 | In-app setup: fixture placement in a 3D stage view (wgpu), live preview, beat/phrase-driven scenes and chases | TODO |
| L.4 | Visualizer support (scope to be decided) | TODO |

## Part A: Automix (later, design discussion required before starting)

| # | Part | Status |
| --- | --- | --- |
| A.1 | Automix redesign: scope and approach to be agreed before any work | TODO |

## Part 1: DJ Sugar features

| # | Feature (C++ reference on main) | Status |
| --- | --- | --- |
| 1.1 | Touch UI: Lite and Booth layouts, compact library, UI scale | TODO |
| 1.2 | Library: Greek and Greeklish search, accent-insensitive collation, fast large collections | TODO |
| 1.3 | Import existing DJ Sugar/Mixxx library (mixxxdb.sqlite, cues, grids, crates) | TODO (openDJ has Mixxx import, verify) |
| 1.4 | YouTube: search, load/save, 16-range parallel download, SponsorBlock cuts | TODO |
| 1.5 | yt-dlp: bundled and self-updating on Android; iOS limits | TODO |
| 1.6 | SoundCloud search (genre, cover, play counts) | TODO |
| 1.7 | Spotify via librespot (already Rust): login, liked, playlists, search, play as files | TODO |
| 1.8 | Subsonic servers and custom stream URLs (openDJ has Navidrome/Jellyfin/Plex) | TODO |
| 1.9 | Streaming hub and smart URL box | TODO |
| 1.10 | FX Station: drum pads, effect scenes | TODO |
| 1.11 | Sample library, recordings, sample editor | TODO |
| 1.12 | Stems: Spleeter/HTDemucs on device (openDJ has demucs via burn, compare) | TODO |
| 1.13 | Smart Auto DJ: energy, key, phrase scoring, beat-matched mix-in | TODO |
| 1.14 | DDJ-FLX4 over USB and Bluetooth MIDI, auto-detect | TODO |
| 1.15 | Android USB audio interfaces from user space, MIDI learn, no root | TODO |
| 1.16 | Rekordbox USB drives: read playlists, back up (openDJ has rbox crates) | TODO |
| 1.17 | Latency calibration with the microphone | TODO |
| 1.18 | Android controller hotplug | TODO |
| 1.19 | Rane SL3 DVS (see C.5, C.6) | TODO |

## Part 2: Mixxx parity

| # | Area | Status |
| --- | --- | --- |
| 2.1 | Effects: Mixxx built-in effect set, LV2 | TODO |
| 2.2 | Key detection and BPM accuracy vs Mixxx analyzers | TODO |
| 2.3 | Controller mapping coverage (MIDI + HID test suite from Mixxx) | TODO |
| 2.4 | Recording, broadcasting (openDJ has both) | TODO (verify) |
| 2.5 | Vinyl control (DVS timecode, see Part C) | TODO |
| 2.6 | Settings and config migration from Mixxx config | TODO |

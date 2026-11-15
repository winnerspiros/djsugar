# DJ Sugar Rust rewrite: roadmap and tracker

DJ Sugar is an independent project: we own this code going forward and are not affiliated with
openDJ or Mixxx. Base: openDJ (github.com/openwave-studio/opendj, GPL-2.0-or-later), imported with
full history; GPL requires keeping LICENSE and the inherited notices in NOTICE.
Reference implementation: DJ Sugar C++ on `main` (Mixxx fork). Every DJ Sugar feature is ported
here; Mixxx code is read for behaviour, not translated line by line.

Status: TODO, WIP, DONE. One part per pull request into `rust`.

## Decisions

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
| 0.2 | Android arm64 build (cargo-ndk, Slint android-activity backend, APK packaging) | TODO |
| 0.3 | Android audio: cpal AAudio backend, low-latency stream, Oboe parity | TODO |
| 0.4 | Android storage and permissions (music folders, SAF) | TODO |
| 0.5 | iOS arm64 build and unsigned IPA | TODO |
| 0.6 | iOS audio (CoreAudio via cpal), file access | TODO |
| 0.7 | Windows ARM64, AppImage, .deb, .dmg, .msi packaging | TODO |
| 0.8 | Store builds: cargo feature `store` drops YouTube/SoundCloud download and librespot (Play/F-Droid policy); full build ships on GitHub releases; About screen links source (GPL) | TODO |

## Part M: mods

One install mechanism for user add-ons: zip with `mod.toml` (id, version, kind, permissions), installed from file or URL, listed/toggled in Settings > Mods.

| # | Part | Status |
| --- | --- | --- |
| M.1 | Data mods: sample packs, skins (Slint themes/colors/assets), controller mappings, effect presets. No code. | TODO |
| M.2 | Source mods: written in Rust, compiled to WASM (wasm32-wasip2, wasmtime/wasmi), with a `djsugar-mod` SDK crate; implementing a `MusicSource` interface (search, browse, resolve-to-audio); host gives HTTP + cache only via declared permissions. YouTube, SoundCloud, Spotify become mods instead of built-ins. | TODO |
| M.3 | Controller mods in Rust (WASM) too; rquickjs kept only to run existing Mixxx/openDJ JS mappings, never for new code. Native plugins desktop-only (plugin-host crate). | TODO |
| M.4 | Store policy: Play forbids downloading native code; WASM/JS interpreted in a sandbox is the allowed path. Store build ships no source mods and no built-in mod catalog URL. | TODO |

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
| 1.19 | Rane SL3 DVS (backlog) | TODO |

## Part 2: Mixxx parity

| # | Area | Status |
| --- | --- | --- |
| 2.1 | Effects: Mixxx built-in effect set, LV2 | TODO |
| 2.2 | Key detection and BPM accuracy vs Mixxx analyzers | TODO |
| 2.3 | Controller mapping coverage (MIDI + HID test suite from Mixxx) | TODO |
| 2.4 | Recording, broadcasting (openDJ has both) | TODO (verify) |
| 2.5 | Vinyl control (DVS timecode) | TODO |
| 2.6 | Settings and config migration from Mixxx config | TODO |

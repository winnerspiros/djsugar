# Study: OpenDJ (djsugar-rust) and OpenFadeR

## 1. Architecture

### OpenDJ (djsugar-rust, ~/djsugar-rust)
**Workspace**: 26 crates under `crates/`, GPL-2.0-or-later (binaries GPL-3.0 due to Slint).

**Crate graph** (key crates):
- `djsugar-engine` — real-time audio: decks, mixer, effects, stem separation, time-stretch, key shift, looper, sampler, groove box, recorder, MIDI clock, Ableton Link, output routing, microphone/aux inputs, preview player.
- `djsugar-control` — lock-free named controls (`[Group],key`), atomic reads/writes, behaviour types (Linear, CenteredOnDefault, Toggle, Push, Indicator, Free). Used by audio thread and UI.
- `djsugar-waveform` — GPU waveform renderer (wgpu), draws into textures Slint displays; summary data uploaded once per track.
- `djsugar-stretch` — phase vocoder with peak phase-locking (realfft), real-time stretch/pitch-shift, no allocation after construction.
- `djsugar-separation` — Demucs (via `demucs-core`, `burn-flex`, `rten`) for 4-stem separation (vocals, drums, bass, other), runs on GPU (Metal/CUDA) or CPU.
- `djsugar-library` — SQLite (rusqlite) library: tracks, cues, beat grids, playlists, history.
- `djsugar-controllers` — MIDI/HID controller mappings (XML + JS), soft takeover, learn mode, 100+ bundled mappings.
- `djsugar-plugin-host` — out-of-process CLAP/VST3 host (`clack-host`, `vst3`), helper binary per slot, crash/hang isolation, restart-once-then-disable policy.
- `djsugar-script` — JavaScript (boa) for controller logic and UI scripting.
- `djsugar-android` — `cdylib` entry point, Slint `backend-android-activity-06`, JNI glue.
- `djsugar-export` — USB export (rekordbox, Engine DJ, Traktor, Serato, VirtualDJ, Mixxx, iTunes), cue/grid sync.
- `djsugar-waveform`, `djsugar-broadcast`, `djsugar-remote`, `djsugar-osc`, `djsugar-link`, `djsugar-mods`, `djsugar-phrase-bench`, `djsugar-stem-bench`, `djsugar-analysis-bench`, `djsugar-hid`, `djsugar-js`, `djsugar-timecode`.

**Audio engine** (`crates/engine/src/engine.rs`):
- Single real-time audio callback (via `cpal`) runs `Engine::process`.
- Wait-free queues (`rtrb` ring buffers) between audio thread and loader threads: `Command` (Load, Analysed, Regrid, LatencyMeasured) in; `Returned` (Done, Ejected, LatencyHeard, GridEdit, PrepareTimecode) out.
- Nothing allocates or locks on audio thread. Tracks arrive over queue, replaced/ejected tracks freed off-thread.
- 4 decks, 3-band isolator EQ + filter per channel, 2 effect units (Color FX + Beat FX), crossfader, master/headphones/booth/external routing, microphone with talkover/ducker, aux input with ducking, sampler (16 slots), groove box (drum slots), stem separation per deck (5 stems max), recorder (WAV/FLAC), preview player, timecode vinyl/CD (DVS).

**Threading**:
- Audio thread: `cpal` callback, real-time priority on Linux/Windows (`realtime` feature), Core Audio already real-time on macOS.
- Loader threads: `rayon` pool for decoding/analysis, background analysis at low priority (`rten` thread pool).
- Plugin host: separate helper processes per plugin slot, communicated via shared memory + IPC.
- UI thread: Slint event loop.

**Audio backends per platform**:
- Linux: ALSA (cpal), PulseAudio/PipeWire, JACK via cpal. `realtime` feature for real-time priority.
- macOS: Core Audio (cpal), real-time by default.
- Windows: WASAPI + ASIO (cpal `asio` feature, downloads Steinberg ASIO SDK at build, GPLv3).
- Android: Oboe (cpal `oboe-shared-stdcxx`), Slint `backend-android-activity-06` (Skia).
- iOS: not yet implemented (Slint has no iOS backend).

**Decoding**: Symphonia (`symphonia` with `all` features: mp3, aac, flac, wav, aiff, ogg, m4a). Whole-file decode to `Audio` (Arc<[f32]> interleaved stereo) or streaming `Decoder` that yields frames while UI plays. Growing file support for recordings (`crates/engine/src/growing.rs`).

**Time-stretch**: `djsugar-stretch` — custom phase vocoder (realfft), block 2048/4096, hop = interval, peak phase-locking for clean tones and tight transients. Input/output rates independent; ratio = stretch. Used for key-lock (tempo change without pitch change) and pitch-shift.

**Analysis** (`crates/engine/src/analysis.rs`, `beats.rs`, `beatthis.rs`, `key.rs`, `keynet.rs`):
- Onset envelope (200 Hz) → autocorrelation → tempo candidate → DP beat tracker (Ellis 2007) → dynamic grid (`gridfit.rs`) with tempo sections.
- Beat This! (MIT, `beat-this` on `rten` runtime) as neural fallback for difficult tracks.
- Key detection: multiple detectors (Krumhansl-Schmuckler, `keynet` spectrogram CNN via `rten`).
- 3-band waveform (low/mid/high) at 150 Hz (detail) and 1200 entries (overview).
- Loudness: EBU R128 (`loudness.rs`).
- Vocal regions (spectral flux + onset).
- Phrase detection from beat grid + 3-band waveform (`phrases.rs`).
- Cached on disk keyed by audio fingerprint (`cache.rs`).

---

### OpenFadeR (~/.hermes/cache/scratch/openfader)
**Workspace**: root + `sdk`, `ui`, `compatibility/*`, `mobile`. AGPL-3.0.

**Crate structure** (single crate `openfader` with many modules under `src/`):
- `audio/` — entire audio engine: `bus` (routing), `player` (one cpal stream, varispeed/key-lock, loop, EQ, filter, meter, latency-compensated clock), `stretch` (WSOLA key-lock), `decode` (symphonia → `Pcm` f32 or `Track16` quantised), `device` (output device caps), `fx` (Beat FX), `cfx` (Color FX), `merge` (Merge FX), `stems` (per-stem gain mix), `sampler`, `groove`, `rec`, `cue` (headphone cue), `mic`, `auxin`, `duck`, `metronome`, `preview`, `input`, `route`, `transport` (key shift, reverse, slip, brake), `track16` (16-bit track, half RAM), `stemfile`, `loudness`, `wavout`.
- `analysis/` — beat grid, 3-band waveforms, key (Camelot), vocal regions, phrases, fingerprint, cache, similar-tracks features.
- `library/` — pluggable sources: `own` (Collection, SQLite + SQLCipher), `folder`, `snapshot`, `playlist_file`, `relocate`, `import`, `translate`, `open_format` (JSON/CSV), plus built-in adapters for Rekordbox (device/db/xml), Engine DJ, Serato, Traktor, VirtualDJ, iTunes (`compatibility/*`).
- `plugin/` — extensible plugin system: Lua scripts (mlua) or child processes (JSON stdin/stdout), native libraries for effects, libraries, controllers, knob/pad modes, artwork. Registry loads on startup, hot-reload for actions/events/settings.
- `device/` — removable device management (USB/SD): scan, eject, backup, export, format (FAT32/exFAT MBR), sync, history import, link (cue sync), update.
- `app/` — Slint UI wiring: `deck`, `browser`, `mixer`, `fx`, `sampler`, `groove`, `automix`, `analyze`, `export`, `midi`, `keys`, `prefs`, `skin`, `split`, `lighting`, `video`, `plugins`, `about`, `diag`, `welcome`, `update`, `stems_model`, `render` (waveform composition).
- `compatiblity/` — format adapters as library plugins.

**Audio engine** (`src/audio/mod.rs`, `src/audio/player.rs`):
- Single `cpal` stream shared by all `Player`s (2 or 4 decks).
- Each `Player`: varispeed or key-locked (WSOLA stretch), loop, volume, cue gain, EQ (3-band isolator + filter), level meter, latency-compensated position clock.
- Master bus: headphones cue (second device), booth/external routing, crossfader, recorder.
- Effects: Beat FX unit per channel/master, Color FX knob (FILTER, SPACE, DUB ECHO, SWEEP, NOISE, CRUSH), Merge FX (one knob builds HPF+noise+echo), Mic FX, Send FX, Lever FX.
- Stems: ONNX Runtime (`ort`) + Demucs model (316 MB download), GPU (WebGPU/Dawn) or CPU separation, per-stem gain buttons/pads.
- Sampler: 16 slots, mixed into master bus.
- Groove: Beat Lab drum slots, capture from playing deck.
- Recorder: post-master WAV.
- Preview player: browser preview.

**Threading**:
- Audio callback: `cpal` stream, `Player::process` runs on audio thread (no allocation, lock-free `Ring` for frame queues between producer/consumer).
- Decode/analysis workers: `std::thread` per job (`DecodeJob` with cancellation), bulk analysis uses process-wide thread budget.
- Plugin processes: one per persistent plugin, JSON IPC.
- UI thread: Slint event loop.

**Audio backends per platform**:
- Linux: ALSA (cpal), ALSA mixer control (`alsa` crate), Wayland drag-and-drop (`wayland-client/backend`).
- macOS: Core Audio (cpal), `objc2` for native file dialogs.
- Windows: WASAPI + ASIO (cpal), `windows-sys` for disk formatting/eject, `fatfs` for FAT32 >32GB.
- Android: cpal `oboe-shared-stdcxx`, Slint `backend-android-activity-06`, separate `mobile` crate (`openfader-mobile`).
- iOS: not supported (no Slint backend).

**Decoding**: Symphonia 0.5 (mp3, aac, flac, wav, aiff, pcm). `decode_async` / `decode_track16_async` on worker thread, cancellation via `AtomicBool`. Video fallback via `ffmpeg` (`src/video.rs`). `Track16` quantises packet-by-packet to 16-bit (half RAM of f32).

**Time-stretch**: `src/audio/stretch.rs` — WSOLA (Waveform Similarity Overlap-Add) for key-lock. Simpler than phase vocoder, good for small stretch ratios (±16%), used when key-lock on.

**Analysis** (`src/analysis/mod.rs`, `detect.rs`, `key.rs`, `auto.rs`):
- Parallel detection: key, vocal regions, waveform+grid run in `thread::scope` for interactive loads; sequential for bulk.
- Beat grid: dynamic programming (same Ellis 2007), tempo sections when beats drift.
- 3-band waveform at 150 Hz (detail) and 1200 entries (overview). Color preview (6-byte RGBH per entry).
- Key: multiple detectors (configurable), Camelot output.
- Vocal: spectral flux + onset envelope.
- Phrases: from beat grid + 3-band waveform (`auto::phrases`).
- Cache: per-track analysis files keyed by fingerprint (`cache.rs`).
- Similar tracks: timbre/energy features + year/composer tags + background profile index.

---

## 2. UI Framework and Layout

### OpenDJ (djsugar-rust)
- **Framework**: Slint 1.18 with `unstable-wgpu-30`, `unstable-fontique-011`, `unstable-winit-030`, `renderer-femtovg-wgpu`.
- **UI definition**: `.slint` files under `crates/app/ui/` (30+ files: `main.slint`, `deck.slint`, `library.slint`, `player.slint`, `mixer.slint`, `controls.slint`, `theme.slint`, `preferences.slint`, `sampler.slint`, `lyrics.slint`, `karaoke.slint`, etc.).
- **Layout**: Hand-positioned in `main.slint` (2000+ lines). Global bar (mode selector: Export/Performance/Edit/Lighting/Video), waveforms stacked above deck row (2/4 deck horizontal/vertical, Browse), mixer column, sampler strip, browser, status bar. No layout managers — explicit `x`, `y`, `width`, `height` arithmetic.
- **Waveform rendering**: `djsugar-waveform` crate (wgpu) → GPU texture → Slint `Image`. Summary data (`GpuTrack`) uploaded once; per-frame uniform block only. Styles: RGB (band balance), Bands (stacked), Mono (played/unplayed), Blue (rekordbox-style), Stems (per-stem colour).
- **Menu bar**: Native macOS menu bar (`crates/app/src/mac_menu.rs`); Windows/Linux: ☰ button opens same File/Edit/View menus (`crates/app/src/app_menu.rs`, `appmenu.slint`). 40 actions (Settings, Quit, Undo, Find, Export*, Record Mix, Mini Player, Zoom, Waveform Height, Tour, Tutorials, User Guide, Report Problem, Check for Updates).
- **Touch**: `touch.slint`, `touch-actions.slint`, `deck-touch.slint` — touch workspace, gestures, large hit targets. Layout profile 1 = touch.
- **Theming**: `theme.slint`, `themes.slint`, `skin/` — Catppuccin Mocha base, skin files restyle `Theme` globals, per-deck waveform colours.

### OpenFadeR
- **Framework**: Slint 1.17/1.18 (pinned), `unstable-winit-030`, `raw-window-handle-06`. UI built as separate crate `openfader-ui` (workspace member).
- **UI definition**: `.slint` files under `ui/` (50+ files: `app.slint`, `deck.slint`, `browser.slint`, `mixer.slint`, `fx.slint`, `sampler.slint`, `groove.slint`, `deck-wave.slint`, `deck-wave-v.slint`, `deck-compact.slint`, `theme.slint`, `themes.slint`, `prefs.slint`, `plugins.slint`, `device.slint`, `exportmode.slint`, `edit.slint`, `lighting.slint`, `video.slint`, `stage/state.slint`, `stage/stage.slint`, `widgets.slint`, `touch.slint`, `split.slint`, etc.).
- **Layout**: `app.slint` (650 lines) hand-positioned. Global bar, mode selector (Export/Performance/Edit/Lighting/Video), waveforms (horizontal stack or vertical between decks), deck row, mixer column, sampler, browser, status bar. Touch layout profile 1, Stage workspace profile 2 (replaces desktop layout for 2-deck performance).
- **Waveform rendering**: CPU composition into `SharedPixelBuffer` (Slint's renderer, no second GPU context). `render/mod.rs` selects GPU image compositor (femtovg) or CPU fallback. `detail.rs`/`overview.rs`/`phrase.rs` compose layers: 3-band (low=blue, mid=amber, high=white), color preview (RGBH), mono. `Axis` maps time→pixel; `Wave` downsamples/upsamples entries to pixel columns; `Canvas` anti-aliases edges. Stem waveform replaces 3-band when stem muted.
- **Menu bar**: No native menu bar. Global bar with mode selector + panel toggles (FX, Mixer, Sampler, Browser). File/Edit/View actions in submenus of global bar or via keyboard shortcuts.
- **Touch**: `touch.slint`, `deck-touch.slint`, `touch-actions.slint` — touch workspace, safe-area insets, large controls. `Theme.layout-profile == 1` enables touch layout.
- **Theming**: `theme.slint` (tokens), `themes.slint` (theme cards), `skin/` — base theme + skin overrides, `Palette` struct in Rust for waveform colours.

---

## 3. Full Feature List

### OpenDJ (djsugar-rust)
**Core DJ features**:
- 4 decks with sync, key-lock, tempo fader (±100%), jog (touchstrip/controller), slip mode, quantize.
- 3-band isolator EQ + filter per channel (kill -∞ dB, boost +9 dB).
- Crossfader (cut-in adjustable), channel faders, master/headphones/booth/external routing.
- 2 effect units × 4 slots: Color FX (knob) + Beat FX (tap tempo, 14 effects).
- Hot cues (8 per deck), memory cue, loop (auto/beat/manual), beat jump, beat loop, slicer, freeze.
- Phrase view: auto-detected phrases (Intro/Build/Drop/Breakdown/Outro/Chorus/Verse) as flags on scrolling waveform + bar on overview.
- Lyrics: synced lyrics (LRCLIB, local LRC), karaoke window.
- Beat counter: position (bars.beats) or distance to next memory cue.
- Key sync: transpose to match key, Camelot wheel display.
- Stem separation (Demucs 4-stem): per-stem gain, mute, solo, isolator per stem, stem FX, instant doubles.
- Sampler: 16 slots, sample packs (.djsugarpack), sequencer (step lanes, macro lanes).
- Groove Box: drum slots (Analog/Synthetic kits via Mutable Instruments Plaits), capture, release FX over stems.
- Looper: 8 loops per deck, heard/quantized modes, overdub.
- Recorder: WAV/FLAC, post-master, split by track.
- Broadcast: Icecast/Shoutcast, MP3/Ogg/Opus, Now Playing (MPRIS/macOS/Windows), Mixcloud upload.
- Timecode vinyl/CD (DVS): control vinyl, CD, relative/absolute modes.
- Ableton Link: tempo/phase sync, MIDI clock out.
- MIDI clock in/out (leader/follower).
- OSC: remote control via tablet/phone.
- Controller mappings: 100+ bundled (Pioneer, Numark, Denon, Reloop, Hercules, etc.), XML + JS, learn mode, soft takeover, layer support.
- Keyboard shortcuts: fully remappable, context-aware.
- Library: SQLite, playlists, smart playlists, history, tags, rating, cover art (Last.fm, MusicBrainz, iTunes), folder watch.
- Analysis: beat grid (dynamic tempo sections), key, vocal regions, phrases, 3-band waveform, loudness, fingerprint cache.
- Export: USB (rekordbox, Engine DJ, Traktor, Serato, VirtualDJ, Mixxx, iTunes), M3U, Sync Manager, Mixcloud.
- Plugins: CLAP/VST3 (out-of-process), instrument plugins (Groove Box), effect plugins on FX slots.
- Scripting: JavaScript (boa) for controllers and UI.
- Remote: web remote (WebSocket), phone/tablet UI.
- Multi-language: i18n (gettext-style macros), 20+ languages.
- Tutorials + guided tour.
- Diagnostics: crash reports, log collection, Help > Report a Problem.
- Update check + one-click update.

**Platform-specific**:
- Android: native APK, touch layout, Oboe audio, Slint Skia backend.
- macOS: native menu bar, Core Audio real-time, MPRIS via souvlaki.
- Windows: ASIO support, WASAPI, native Open dialog.
- Linux: ALSA/JACK/PipeWire, MPRIS, Wayland drag-and-drop.

### OpenFadeR
**Core DJ features**:
- 2 or 4 decks with sync, key-lock (WSOLA), tempo fader, jog, slip, quantize.
- 3-band isolator EQ + filter per channel.
- Crossfader (enable/disable), channel faders, master/headphones/booth/external (6 external outputs).
- Effects panel: Color FX (5 modes), Beat FX (12 effects, tap tempo), Release FX, Merge FX, Send FX, Lever FX, Mic FX.
- Hot cues (8 + memory), loops (auto/beat/manual), beat jump, slicer, freeze.
- Phrase view: auto-detected phrases as flags + overview bar.
- Stem separation (Demucs via ONNX Runtime): 4 stems (vocal, instrumental, bass, drums), per-stem gain/mute/solo, stem isolator, stem FX, instant doubles. GPU (WebGPU/Dawn) or CPU.
- Sampler: 16 pads, sample packs, sequencer (patterns).
- Beat Lab: loop slots under decks, CAPTURE from playing deck, DRUM SWAP, DRUM RELEASE FX over stems.
- Grid editor: type BPM, HERE (section-only), UNDO/REDO, Analysis Lock.
- Recorder: post-master WAV.
- Automix: transition styles (recipes), phrase-aware, queue.
- Library: pluggable sources (Collection SQLite+SQLCipher, folder, playlist files, snapshot, remote). Built-in adapters: Rekordbox (device/db/xml), Engine DJ, Serato, Traktor, VirtualDJ, iTunes. Relocate moved files. Import/export (JSON/CSV, M3U, device formats).
- Device manager: USB/SD scan, eject, backup, format (FAT32/exFAT MBR), export (multiple formats), sync, history import, cue link, update.
- Controller: MIDI mappings (installed auto-load), soft takeover, get mappings from catalog.
- Keyboard shortcuts: remappable.
- Analysis: beat grid (dynamic), key (Camelot), vocal, phrases, 3-band waveform (detail/preview), color preview, fingerprint cache, similar tracks.
- Lighting: DMX fixtures following master deck (ArtNet/sACN).
- Video: full-window video mode, video deck view.
- Plugins: Lua scripts (in-process) or child processes (JSON), native libraries for effects, libraries, controllers, knob/pad modes, artwork. Hot-reload for actions/events/settings.
- Split screen: browser in separate window.
- Export mode: player panels over decks A/B, layouts 1 Player…Full Browser.
- Theming: themes + skins, per-deck waveform colours.
- Multi-language: i18n, system language detection.
- Welcome tour, Help > Report a Problem, crash offer, update check + one-click update (with rollback).
- Diagnostics: verbose logging, stick verifier (`openfader verify <mount>`).

**Platform-specific**:
- Raspberry Pi: aarch64 builds, CPU-only stems (Dawn no ARM WebGPU), `Dockerfile.pi`, `profile.pi` (fat LTO, strip).
- Android: separate `mobile` crate, touch layout, software renderer preview.
- Linux: ALSA mixer control, Wayland drag-and-drop, udisks2 for device formatting.
- macOS: native file dialogs (osascript), Core Audio.
- Windows: ASIO, native Open dialog, elevated disk formatter, FAT32 >32GB via `fatfs`.

---

## 4. What Is Genuinely Good

### OpenDJ (djsugar-rust)
- **File/Edit/View native menu bar on macOS** (`mac_menu.rs` + `app_menu.rs`): macOS gets real menu bar; Windows/Linux get identical items from ☰ button. Single source of truth (`MENUS` array, 40 actions). No window height wasted on menu strip.
- **Wait-free audio/loader communication** (`engine.rs`): `rtrb` ring buffers, `Command`/`Returned` enums, zero allocation on audio thread. Tracks play immediately on load; waveform/tempo fill in asynchronously.
- **Phase vocoder with peak phase-locking** (`stretch/lib.rs`): Clean tones, tight transients, independent input/output rates. Custom, no external deps beyond `realfft`.
- **Out-of-process plugin host** (`plugin-host/`): Helper binary per slot, crash/hang isolation, restart-once-then-disable policy. Audio thread never waits.
- **GPU waveform renderer** (`waveform/`): wgpu textures, summary data uploaded once, per-frame uniform only. Multiple styles (RGB, Bands, Mono, Blue, Stems). Pixels never leave GPU.
- **Dynamic beat grid with tempo sections** (`beats.rs`, `gridfit.rs`): Handles live drums, old disco. DP beat tracker + grid fitting. Editable (BeatHere, Shift, Drag, Tempo, AddSection, BarHere, Scale, Lock, Reset).
- **Stem separation integrated into engine** (`stems.rs`, `separation.rs`): Demucs via `burn`/`rten`, GPU (Metal/CUDA) or CPU, per-stem gain/mute/solo/FX/instant-doubles.
- **Groove Box with Plaits drum models** (`groove.rs`, `kits.rs`): Mutable Instruments Plaits (MIT), analog/synthetic kits, capture from deck, release FX over stems.
- **Broadcast + Now Playing + Mixcloud upload** (`broadcast.rs`, `now_playing.rs`, `mixcloud_upload.rs`): Icecast, MPRIS, system media controls, one-click Mixcloud.
- **Timecode vinyl/CD (DVS)** (`timecode/`): Relative/absolute, multiple formats, latency measurement.
- **Ableton Link + MIDI clock** (`link.rs`, `midi_clock.rs`): Tempo/phase sync across apps/hardware.
- **Controller mappings in XML + JS** (`controllers/`): 100+ bundled, learn mode, soft takeover, layers. JS for complex logic.
- **Touch layout profile** (`touch.slint`, `deck-touch.slint`): Separate layout profile, large hit targets, gestures.
- **Theming/skinning system** (`skin/`, `theme.slint`): Catppuccin base, skin files override `Theme` globals, per-deck waveform colours.

### OpenFadeR
- **Pluggable library sources** (`library/`): Unified `Library` trait, built-in adapters for all major DJ formats (Rekordbox 3 ways, Engine DJ, Serato, Traktor, VirtualDJ, iTunes). Relocation finds moved files.
- **Device manager for USB/SD** (`device/`): Scan, eject, backup, format (FAT32/exFAT MBR), multi-format export, sync, history import, cue link, update. OS-specific implementations (Linux `udisks2`, Windows `DeviceIoControl`, macOS `diskutil`).
- **Plugin system** (`plugin/`): Lua (mlua) or child processes (JSON IPC), native libraries for effects/libraries/controllers/knob-pad modes/artwork. Hot-reload for actions/events/settings. Declarative knob/pad modes in JSON.
- **WSOLA key-lock** (`audio/stretch.rs`): Simpler than phase vocoder, good for ±16%, low CPU.
- **Stem separation via ONNX Runtime** (`stems.rs`): Demucs model, GPU (WebGPU/Dawn) or CPU, per-stem gain buttons/pads/isolator/FX.
- **Beat Lab** (`groove.rs`, `groove.slint`): Loop slots, CAPTURE, DRUM SWAP, DRUM RELEASE FX over stems.
- **Grid editor with UNDO/REDO** (`edit.rs`, `edit.slint`): Type BPM, HERE (section-only), Analysis Lock.
- **Color preview waveform** (`analysis/colors.rs`, `render/detail.rs`): 6-byte RGBH per entry, 3-bit colour + 5-bit height.
- **Similar tracks** (`analysis/similar.rs`): Timbre/energy features + tags + background profile index.
- **Lighting (DMX/ArtNet/sACN)** (`lighting.rs`, `lighting.slint`): Fixtures following master deck.
- **Split screen browser** (`split.rs`, `split.slint`): Browser in separate window, status bar / View menu.
- **Export mode** (`exportmode.rs`, `exportmode.slint`): Player panels over decks A/B, 6 layouts.
- **Raspberry Pi optimisation** (`profile.pi`, `Dockerfile.pi`): Fat LTO, strip, CPU-only stems, prebuilt archives.
- **Rollback on failed update** (`update.rs`, `main.rs`): Keeps `.prev` copy, `--rollback` flag.
- **Low-memory malloc tuning** (`main.rs`): `mallopt` for glibc (274→203 MB RSS measured).
- **Stick verifier** (`device/verify.rs`): `openfader verify <mount>` headless, checks player compatibility.

---

## 5. What Is Weak or Incomplete

### OpenDJ (djsugar-rust)
- **iOS**: No backend (Slint lacks iOS). Android works but touch layout is desktop-adapted.
- **Video**: No video support (no video decoding, no video output).
- **Lighting/DMX**: Not implemented.
- **Library plugins**: No pluggable library sources; only SQLite collection + folder scan. No Rekordbox/Serato/Traktor import (roadmap has it).
- **Device manager**: No USB/SD management, no export to player formats (roadmap: `export` crate exists but minimal).
- **Plugin host**: CLAP/VST3 only; no AudioUnit (macOS), no LV2 (Linux). Helper binary per slot = process overhead.
- **Stem separation**: Demucs only (4-stem). No model choice, no CPU/GPU toggle in UI (hardcoded profiles).
- **Analysis cache**: Fingerprint-based, but no migration when analysis improves.
- **Documentation**: Roadmap in `docs/djsugar-rust-roadmap.md` but many crates lack internal docs.
- **Testing**: Few integration tests; `analysis-bench`, `stem-bench`, `phrase-bench` are benchmarks, not tests.
- **Windows ASIO**: Requires Steinberg SDK download at build (GPLv3), not redistributable easily.

### OpenFadeR
- **iOS**: No backend (Slint lacks iOS). Android is separate `mobile` crate with software renderer only.
- **AudioUnit / LV2**: Plugin system supports native libraries but no AudioUnit/LV2 scanning (CLAP/VST3 only via `libloading` in `effect.rs`).
- **Ableton Link**: Not implemented (no `link` crate).
- **MIDI clock**: Not implemented.
- **Timecode/DVS**: Not implemented.
- **Broadcast/Now Playing**: Not implemented.
- **OSC**: Not implemented.
- **Scripting**: Lua only (no JS). Controller logic in Lua or native plugin.
- **Groove Box**: Beat Lab only (loop slots, capture); no drum synthesis (Plaits).
- **Sampler sequencer**: Patterns only; no step lanes/macro lanes like djsugar.
- **Lyrics/karaoke**: Not implemented.
- **Multi-language**: i18n exists but fewer translations.
- **Tutorials/tour**: Welcome window only; no guided tour or tutorials.
- **Diagnostics**: Crash reports + log collection, but no structured health check like djsugar's `diagnostics.rs`.
- **Update/rollback**: Works but Windows `TerminateProcess` hack for thread cleanup on exit.
- **Waveform rendering**: CPU composition (SharedPixelBuffer) — scales poorly on high-DPI/large windows; GPU path via femtovg but not primary.
- **Android**: `mobile` crate is preview only (software renderer, no audio engine features).

---

## 6. Crate Dependencies: Worth Reusing vs Avoid

### Worth Reusing (Pure Rust, Well-Maintained, DJ-Relevant)

| Crate | Use Case | Why |
|-------|----------|-----|
| `symphonia` | Audio decoding | Pure Rust, supports mp3/aac/flac/wav/aiff/ogg/m4a. Both projects use it. |
| `cpal` | Audio I/O | Cross-platform (ALSA/CoreAudio/WASAPI/Oboe/ASIO). Standard. |
| `realfft` | FFT for stretch/analysis | Pure Rust, used by `djsugar-stretch` and `beat-this`. |
| `rubato` | Sample rate conversion | High-quality, pure Rust. Consider for resampling. |
| `rtrb` | Lock-free ring buffers | Used by djsugar for audio↔loader queues. Simple, correct. |
| `rayon` | Thread pool for analysis | Standard, work-stealing. Both use it (djsugar explicitly, openfader via `thread::scope`). |
| `parking_lot` | Fast mutex/RwLock | Used by openfader; faster than std. |
| `rusqlite` + `sqlcipher` | Library DB | Both use SQLite. SQLCipher for encrypted adapters (openfader). |
| `serde` + `serde_json` | Serialization | Universal. |
| `mlua` | Lua scripting | Openfader uses for plugins; pure Rust, Lua 5.4. |
| `ort` (ONNX Runtime) | ML inference (stems) | Openfader uses for Demucs. GPU (WebGPU/Dawn) + CPU. |
| `burn` + `rten` | ML inference (Demucs, Beat This!) | Djsugar uses for stem separation + beat tracking. Pure Rust runtimes. |
| `demucs-core` | Stem separation model | Djsugar's wrapped Demucs. MIT. |
| `beat-this` | Neural beat tracking | Djsugar uses as fallback. MIT, runs on `rten`. |
| `lofty` | Audio metadata/tags | Djsugar uses. Pure Rust, supports ID3, Vorbis, MP4, APE. |
| `hound` | WAV read/write | Djsugar uses for recorder. Pure Rust. |
| `flacenc` | FLAC encoding | Djsugar uses for recorder. Pure Rust. |
| `midir` | MIDI I/O | Both use. Cross-platform (ALSA/CoreMIDI/WinMM). |
| `mdns-sd` | Bonjour/MDNS | Djsugar for OSC discovery. Pure Rust. |
| `souvlaki` | Now Playing / Media controls | Djsugar uses (MPRIS/macOS/Windows). |
| `whisper-rs` | Speech-to-text (lyrics) | Djsugar uses for word timing. Wrapper over whisper.cpp. |
| `rten` + `rten-*` | ML runtime | Pure Rust, no C++ deps. Used by beat-this, burn. |
| `fontique` | Font discovery without fontconfig | Djsugar patches for Linux fonts. Pure Rust. |
| `femtovg` | 2D vector graphics (Slint renderer) | Djsugar patches pipeline cache fix. |
| `slint` | UI framework | Both use. Declarative, Rust-native, GPU/CPU backends. |
| `wgpu` | GPU compute/graphics | Djsugar waveform renderer. Pure Rust, Vulkan/Metal/DX12/WebGPU. |
| `bytemuck` | Pod/Zeroable for GPU buffers | Djsugar waveform. Standard. |
| `ab_glyph` | Font rasterisation | Djsugar waveform text. Pure Rust. |
| `skrifa` + `read-fonts` + `write-fonts` | Font parsing | Djsugar for theme fonts. Pure Rust. |
| `chrono` | Date/time | Both use. |
| `ureq` | HTTP client | Both use. Pure Rust, minimal. |
| `ring` | Crypto (sha256) | Both use for model/file verification. |
| `sha2` / `hmac-sha256` | Hashing | Openfader for update verification. |
| `tar` / `flate2` / `zip` | Archives | Both use for sample packs, updates. |
| `notify` | File system watch | Djsugar for library folder watch. |
| `directories-next` | XDG dirs | Cross-platform config/data dirs. |
| `clap` | CLI args | Openfader for `verify`, `--rollback`, `--format-disk`. |

### Avoid or Use with Caution

| Crate | Reason |
|-------|--------|
| `vst3` / `clack-host` / `clack-extensions` | Complex, Steinberg SDK (VST3) or CLAP spec bindings. Plugin host is a major subsystem — consider if you need it. |
| `libloading` | Dynamic library loading for plugins. Platform-specific, unsafe. Openfader uses for Beat FX native libs. |
| `winit` | Windowing (Slint uses internally). Don't use directly unless you need custom window handling. |
| `objc2` / `objc2-*` | macOS Objective-C bindings. Djsugar uses for native menu bar; openfader for file dialogs. Platform-specific, verbose. |
| `windows-sys` | Windows API bindings. Huge, version-pinned. Openfader uses extensively for device/eject/format. |
| `fatfs` | FAT32 formatter (Windows >32GB). Niche, only for device formatting. |
| `zbus` | D-Bus (udisks2). Linux-only, heavy. Openfader uses for device formatting. |
| `alsa` | ALSA mixer control. Linux-only. Openfader uses for hardware mixer. |
| `wayland-client` / `wayland-backend` | Wayland drag-and-drop. Linux-only, complex. |
| `jni` | Android JNI. Only for Android entry point. |
| `block2` | macOS blocks (plugin host). Platform-specific. |
| `demucs-core` (git dep) | Not on crates.io, pinned commit. Fork/maintenance risk. |
| `mi-plaits-dsp` (git dep) | Not on crates.io, Mutable Instruments Plaits. Fork/maintenance risk. |
| `whisper-rs` | Requires cmake + whisper.cpp build. Heavy native dep. |
| `symphonia-bundle-mp3` / `symphonia-bundle-flac` | Codec bundles (C deps via autotools/meson). Djsugar uses `symphonia/all` which pulls them. |
| `cpal` `asio` feature | Downloads Steinberg ASIO SDK at build. GPLv3, not redistributable. |
| `slint` `backend-android-activity-06` | Tied to specific Slint version, Skia backend. |
| `ort` (ONNX Runtime) | Downloads binaries at build (`download-binaries`). Large (100+ MB), version-pinned, Dawn/WebGPU only on some platforms. |
| `burn-flex` / `gemm-f32` / `matrixmultiply` / `rten-*` | ML stack. Heavy compile times, many deps. Only if you need stem separation/beat tracking. |

### Recommendations for djsugar-next
- **Audio engine**: Use `cpal` + `symphonia` + `rubato` (resampling) + custom phase vocoder (like `djsugar-stretch`) or WSOLA (like openfader) for key-lock. Avoid `rubato` for time-stretch (it's for resampling).
- **Analysis**: Port `djsugar-engine` beat grid (`beats.rs`, `gridfit.rs`) + key detection (`key.rs`, `keynet.rs`) + 3-band waveform. They are pure Rust, well-tested.
- **Stems**: If needed, use `ort` + Demucs ONNX model (openfader approach) — simpler than `burn`+`demucs-core`+`rten`. But adds 100+ MB binary download.
- **UI**: Slint (wgpu renderer). Use `djsugar-waveform` approach (GPU textures, summary data uploaded once) over openfader's CPU composition.
- **Library**: Pluggable `Library` trait (openfader) + SQLite collection (djsugar). Support Rekordbox/Serato/Traktor via compatibility adapters.
- **Devices**: Openfader's device manager is comprehensive (format, eject, backup, sync). Port if USB export is a goal.
- **Plugins**: Start without. If needed, CLAP out-of-process (djsugar) is safer than native libraries (openfader).
- **Controllers**: XML + JS mappings (djsugar) — proven, 100+ bundled. Learn mode + soft takeover.
- **Android**: Slint `backend-android-activity-06` + `cpal` Oboe. Works but touch layout needs work.
- **iOS**: Blocked on Slint iOS backend. No near-term path.
- **Profiling**: `profile.android` (thin LTO, opt-level=s for most, 3 for audio) + `profile.pi` (fat LTO, strip) are good references.

---

## 7. Key File Paths (for reference)

### OpenDJ (djsugar-rust)
- Workspace: `/home/ubuntu/djsugar-rust/Cargo.toml`
- Engine: `/home/ubuntu/djsugar-rust/crates/engine/src/` (engine.rs, analysis.rs, beats.rs, stretch.rs, loader.rs, plugins.rs, controls.rs, effects.rs, track.rs, decode.rs, dsp.rs, groove.rs, stems.rs, separation.rs, key.rs, keynet.rs, phrases.rs, cache.rs, beatthis.rs, loudness.rs, vocals.rs, word_timing.rs, looper.rs, sampler.rs, recorder.rs, output.rs, inputs.rs, midi_clock.rs, link.rs, arp.rs, gridfit.rs, scale.rs, patterns.rs, kits.rs, synth.rs, cover.rs, auto_trim.rs, background.rs, process.rs, rt_guard.rs, visual.rs, scan.rs, tags.rs, import.rs, round_trip.rs, paths.rs, meter.rs, lyrics.rs, growing.rs, preview.rs)
- Stretch: `/home/ubuntu/djsugar-rust/crates/stretch/src/lib.rs`
- Waveform: `/home/ubuntu/djsugar-rust/crates/waveform/src/` (lib.rs, waveform.wgsl)
- Control: `/home/ubuntu/djsugar-rust/crates/control/src/lib.rs`
- Controllers: `/home/ubuntu/djsugar-rust/crates/controllers/src/` (host.rs, manager.rs, catalog.rs, format.rs, mapping.rs, edit.rs, script.rs, learn.rs, pickup.rs, routing.rs, rows.rs, view.rs, hid.rs, quick.rs, probe.rs, support.rs, own.rs, export.rs, import/, layout.rs)
- Plugin host: `/home/ubuntu/djsugar-rust/crates/plugin-host/src/` (bin/djsugar-plugin-host.rs)
- App/UI: `/home/ubuntu/djsugar-rust/crates/app/src/` (lib.rs, main.rs, app_menu.rs, mac_menu.rs, browser.rs, deck.slint, main.slint, theme.slint, controls.slint, deck.slint, mixer.slint, player.slint, sampler.slint, lyrics.slint, karaoke.slint, preferences.slint, waveforms, touch, layouts, skin, diagnostics.rs, update.rs, tour.rs, tutorials.rs, shortcuts.rs, settings.rs, prefs/, browser/, waveforms/)
- Android: `/home/ubuntu/djsugar-rust/crates/android/src/`
- Library: `/home/ubuntu/djsugar-rust/crates/library/src/`
- Export: `/home/ubuntu/djsugar-rust/crates/export/src/`
- Broadcast: `/home/ubuntu/djsugar-rust/crates/broadcast/src/`
- Remote: `/home/ubuntu/djsugar-rust/crates/remote/src/`
- OSC: `/home/ubuntu/djsugar-rust/crates/osc/src/`
- Script: `/home/ubuntu/djsugar-rust/crates/script/src/`
- JS: `/home/ubuntu/djsugar-rust/crates/js/src/`
- Timecode: `/home/ubuntu/djsugar-rust/crates/timecode/src/`
- Separation: `/home/ubuntu/djsugar-rust/crates/separation/src/`
- Analysis bench: `/home/ubuntu/djsugar-rust/crates/analysis-bench/src/`

### OpenFadeR
- Root: `/home/ubuntu/.hermes/cache/scratch/openfader/Cargo.toml`
- Audio engine: `/home/ubuntu/.hermes/cache/scratch/openfader/src/audio/` (mod.rs, player.rs, bus.rs, stretch.rs, decode.rs, device.rs, fx.rs, cfx.rs, merge.rs, stems.rs, sampler.rs, groove.rs, rec.rs, cue.rs, mic.rs, auxin.rs, duck.rs, metronome.rs, preview.rs, input.rs, route.rs, transport.rs, track16.rs, stemfile.rs, loudness.rs, wavout.rs, handover.rs, dsp/, player/, fx/, groove/, sampler/, cfx/, merge/, device/, rec/, stemfile/)
- Analysis: `/home/ubuntu/.hermes/cache/scratch/openfader/src/analysis/` (mod.rs, detect.rs, key.rs, auto.rs, cache.rs, fingerprint.rs, colors.rs, options.rs, phrase_edit.rs, similar.rs, tag.rs, detect/, auto/, key/, similar/)
- Library: `/home/ubuntu/.hermes/cache/scratch/openfader/src/library/` (mod.rs, own.rs, folder.rs, snapshot.rs, playlist_file.rs, relocate.rs, import.rs, import_report.rs, open_format.rs, translate.rs, print.rs, connect.rs)
- Plugin: `/home/ubuntu/.hermes/cache/scratch/openfader/src/plugin/` (mod.rs, action.rs, builtin.rs, config.rs, controller.rs, device.rs, effect.rs, event.rs, install.rs, lua.rs, process.rs, source.rs, tests.rs, action/, controller/, device/, effect/, event/, library/, source/)
- Device: `/home/ubuntu/.hermes/cache/scratch/openfader/src/device/` (mod.rs, linux.rs, macos.rs, windows.rs, eject.rs, backup.rs, export.rs, format.rs, history.rs, link.rs, plan.rs, plug.rs, sync.rs, transfer.rs, undo.rs, update.rs, watch.rs, check.rs, eject/, format/)
- App/UI: `/home/ubuntu/.hermes/cache/scratch/openfader/src/app/` (mod.rs, main.rs, deck.rs, browser.rs, mixer.rs, fx.rs, sampler.rs, groove.rs, automix.rs, analyze.rs, export.rs, midi.rs, keys.rs, prefs.rs, skin.rs, split.rs, lighting.rs, video.rs, plugins.rs, about.rs, diag.rs, welcome.rs, update.rs, stems_model.rs, render/, deck/, browser/, mixer/, fx/, sampler/, groove/, analyze/, export/, midi/, keys/, prefs/, device/, lighting/, video/, plugins/, stage/)
- UI (Slint): `/home/ubuntu/.hermes/cache/scratch/openfader/ui/` (app.slint, deck.slint, deck-wave.slint, deck-wave-v.slint, deck-compact.slint, browser.slint, mixer.slint, fx.slint, sampler.slint, groove.slint, theme.slint, themes.slint, prefs.slint, plugins.slint, device.slint, exportmode.slint, edit.slint, lighting.slint, video.slint, split.slint, touch.slint, touch-actions.slint, widgets.slint, stage/state.slint, stage/stage.slint, deck-controls.slint, deck-header.slint, deck-panel.slint, deck-state.slint, pads.slint, gauges.slint, help.slint, about.slint, report.slint, welcome.slint, toast.slint, track-menu.slint, trackinfo.slint, artwork.slint, catalog.slint, midi.slint, mixer-touch.slint, mixpoint.slint, mixpoint-state.slint, dnd.slint, resources.slint, stems-model.slint, update.slint, video.slint, welcome.slint, icons.slint, dialog.slint, checklist.slint, crate_icon.slint, filters.slint, fourdeck.slint, info.slint, instruments.slint, karaoke.slint, layouts.slint, lite.slint, mini.slint, nowplaying.slint, remote.slint, sampler.slint, scroll.slint, sources.slint, sugar.slint, toppanels.slint, tour.slint, tourmark.slint, usb_export.slint, wave_height.slint, wave_look.slint, zoom.slint, translations/)
- Mobile: `/home/ubuntu/.hermes/cache/scratch/openfader/mobile/` (Cargo.toml, android/, src/, ui/)
- Compatibility: `/home/ubuntu/.hermes/cache/scratch/openfader/compatibility/` (rekordbox/, engine-dj/, itunes/, serato/, traktor/, virtualdj/)
- SDK: `/home/ubuntu/.hermes/cache/scratch/openfader/sdk/`
# Code Helper Projects: Feature Parity & Porting Guide

**Purpose**: Map what `djsugar-next` (Rust rewrite) can take from two helper projects — **OpenFadeR** (Rust SDK for DJ controllers, effects, stems, library import) and **OpenDJ** (`djsugar-rust`, full Rust DJ app) — to reach parity with **DJ Sugar Mixxx** (`~/djsugar`, C++/Qt Mixxx fork).

> **Rules**: All code must be original Rust (no copying, no other app names in code). Describe algorithms/approaches, not paste code. ASCII only. Do not edit `~/djsugar-next` code or commit.

---

## Legend

| Symbol | Meaning |
|--------|---------|
| ✅ | Implemented (file:function) |
| ⚠️ | Partial / scaffold only |
| ❌ | Not implemented |
| 🟢 | Best basis for `djsugar-next` |

---

## 1. Engine Core — Playback, Mixing, Threading

| Feature | DJ Sugar Mixxx | OpenFadeR | OpenDJ (`djsugar-rust`) | `djsugar-next` | Best Basis |
|---------|----------------|-----------|------------------------|----------------|------------|
| Multi-deck playback (4+) | `EngineMixer` + `EngineDeck` (4+ decks) | — | `crates/engine/src/mixer.rs:Mixer` + `deck.rs:Deck` (DECKS=4) | `crates/engine/src/mixer.rs:Mixer` + `deck.rs` (DECKS=4) | **OpenDJ** — complete mixer + deck architecture with wait-free command queue |
| Sample-accurate position | `FramePos` (u64 frame count) | — | `Deck.position: f64` (frames) | `Deck.position: f64` | **OpenDJ** |
| Variable rate playback | `RateControl::calculateSpeed()` | — | `Deck.rate` (relative) + `nudge` + `velocity` | `Deck.rate` + `nudge` | **OpenDJ** |
| Reverse playback | `RateControl` reverse + `m_reverse_old` | — | `Deck.reverse: bool` + negative step | `Deck.reverse` | **OpenDJ** |
| Brake / spin-down | Motor sim (33/45 RPM, power-cut/brake) | — | `Deck.brake` + `velocity` decay (simple linear) | `Deck.brake` | **DJ Sugar** for algorithm (configurable motor sim); **OpenDJ** for integration |
| Track end handling | `ReadAheadManager` wrap + `notifyWrapAround` | — | Stops at `pos > last` | Stops | **DJ Sugar** for repeat/loop-wrap logic |
| Vinyl control (timecode) | `VinylControlControl` + `PositionScratchController` | — | ❌ | ❌ | **DJ Sugar** — only project with complete DVS |
| Read-ahead / caching | `CachingReader` + `ReadAheadManager` (async decode) | — | Loads whole `Track` into `Box<[[f32;2]]>` — no streaming | Loads whole track | **DJ Sugar** for architecture; **OpenDJ** for symphonia decode base |
| Multi-channel output | Main/booth/headphones/sidechain buses | — | Single stereo `out` + `Station` for FX bus | Single stereo | **DJ Sugar** for bus architecture |
| Sample rate conversion | `EngineBufferScale` (linear, SoundTouch, RubberBand) | — | `sugar_stretch::Stretch` (SoundTouch-style phase vocoder) per-deck | `sugar_stretch::Stretch` | **OpenDJ** — `stretch` crate is the algorithm |
| Triple-buffered pipeline | `EngineBuffer` (input→process→output) | — | `Control` with `rtrb::Producer<Command>` + `triple_buffer::Output<Telemetry>` | `Control` with command queue | **OpenDJ** — lock-free audio thread design |

---

## 2. Decks — Cues, Loops, Sync, Quantize, Slip

| Feature | DJ Sugar Mixxx | OpenFadeR | OpenDJ | `djsugar-next` | Best Basis |
|---------|----------------|-----------|--------|----------------|------------|
| Main cue point | `CueControl::m_pCue` (per-deck) | — | `Deck.cue: f64` | `Deck.cue` | **OpenDJ** |
| Hot cues (8/deck) | `HotCueControl` (8, colored) | — | `Track.hotcues: [Hotcue; 8]` | `Track.hotcues` | **OpenDJ** |
| Hot cue colors | 8 colors, customizable | — | `Hotcue.color: u8` (index) | `Hotcue.color` | **OpenDJ** |
| Cue modes (CDJ/Serato/Traktor) | `cue_mode` control | — | ❌ | ❌ | **DJ Sugar** for mode logic |
| Beat jump (1..64 beats) | `beatjump_*`, `BEATJUMP_SIZES` | — | `BEATJUMP_SIZES` const + `Deck.seek_relative()` | `BEATJUMP_SIZES` | **OpenDJ** |
| Loop in/out | `LoopControl` (manual + auto) | — | `Deck.loop: Option<(f64,f64)>` + `loop_halve/double/move` | `Deck.loop` | **OpenDJ** |
| Saved loops (memory cues) | `CueType::Loop` in cue list | — | `Track.markers` with `MarkerKind::Loop` | `Track.markers` | **OpenDJ** |
| Quantize (snap to beat/bar/phrase) | `QuantizeControl` | — | `Grid.snap()` + `quantize` control | `Grid.snap()` | **OpenDJ** — `Grid.snap` is the core algorithm |
| Slip mode | `SlipModeState` (playhead continues) | — | `Deck.slip: bool` + `slip_position` | `Deck.slip` | **OpenDJ** |
| Sync (beat/phase/bar/phrase) | `BPMControl::sync` + `EngineDeck::sync()` | — | `Deck.sync` + `beat_sync()` using `Grid` | `Deck.sync` | **OpenDJ** — `beats.rs` has variable-tempo grid sync |
| Master tempo (leader deck) | `MASTER` button per deck | — | `Deck.master: bool` | `Deck.master` | **OpenDJ** |
| Key lock (master tempo) | `keylock` + `keylock_disable_on_sync` | — | `Deck.key_lock: bool` + `keyshift` | `Deck.key_lock` | **OpenDJ** — `keyshift.rs` integrates with `stretch` |
| Key shift (+/- semitones) | `key_shift` control | — | `Deck.key_shift: f32` (semitones) | `Deck.key_shift` | **OpenDJ** |
| Key sync (match other deck) | `key_sync` button | — | ❌ | ❌ | **DJ Sugar** for UI flow; **OpenDJ** `key.rs` for key math |
| Reverse play | `reverse` button | — | `Deck.reverse` | `Deck.reverse` | **OpenDJ** |
| Flux mode (Serato-style) | `flux_mode` control | — | ❌ | ❌ | **DJ Sugar** only |

---

## 3. BPM / Tempo Analysis & Beatgrid

| Feature | DJ Sugar Mixxx | OpenFadeR | OpenDJ | `djsugar-next` | Best Basis |
|---------|----------------|-----------|--------|----------------|------------|
| BPM detection | `AnalyzerBeats` (libbeat/autocorr) | — | `crates/analysis/src/lib.rs:grid()` — flux + autocorr | `crates/analysis/src/lib.rs:grid()` | **OpenDJ** — pure Rust flux+autocorr; `djsugar-next` already ports it |
| Beat grid (variable tempo) | `AnalyzerBeats` → `BeatGrid` (markers) | — | `crates/engine/src/beats.rs:BeatGrid` (markers with per-section BPM) | `crates/core/src/time.rs:Grid` (simple) | **OpenDJ** — `BeatGrid` handles tempo changes, nudges to whole beats |
| Grid editing (beat here, shift, tempo adjust, add/remove section, scale, bar here, undo, lock, reset) | `BeatGridControl` + `EngineDeck` edits | — | `crates/engine/src/beats.rs:GridEdit` enum + `BeatGrid::edited()` | ❌ | **OpenDJ** — complete edit enum + pure algorithm |
| Downbeat detection | `AnalyzerBeats` (energy/onset) | — | ❌ (only first beat) | ❌ | **DJ Sugar** for algorithm; **OpenDJ** `grid` could extend |
| Tap tempo | `bpm_tap` control | — | `TAP_GAP` + `TAPS` averaging in `effects.rs` | ❌ | **OpenDJ** — tap logic in `effects.rs` |
| BPM range / lock | `bpm_range`, `bpm_lock` | — | `bpm` control with range | ❌ | **OpenDJ** controls |

---

## 4. Key Detection & Harmonic Mixing

| Feature | DJ Sugar Mixxx | OpenFadeR | OpenDJ | `djsugar-next` | Best Basis |
|---------|----------------|-----------|--------|----------------|------------|
| Key detection | `AnalyzerKey` (Krumhansl-Schmuckler) | — | `crates/engine/src/key.rs:Key` (K-S profile) | `crates/core/src/key.rs:Key` | **OpenDJ** — `Key::detect()` with major/minor profiles |
| Key notation (Camelot, OpenKey, classic) | `Key` class with multiple notations | — | `Key::notation()` (Camelot, OpenKey, classic) | `Key::notation()` | **OpenDJ** |
| Key lock (pitch-invariant stretch) | `keylock` + elastique/rubberband | — | `keyshift.rs` + `stretch::Stretch` | `stretch::Stretch` | **OpenDJ** — `keyshift` applies semitone transpose to stretcher |
| Key shift (+/- 6 semitones, menu to +/-12) | `key_shift` control | — | `Deck.key_shift` + `Key::transpose()` | `Deck.key_shift` | **OpenDJ** |
| Key sync (match other deck) | `key_sync` button | — | ❌ | ❌ | **DJ Sugar** for flow; **OpenDJ** `Key::compatible()` for harmonic match |
| Harmonic mixing wheel | UI only | — | `keynet.rs` (compatibility graph) | ❌ | **OpenDJ** — `keynet.rs` builds compatibility edges |

---

## 5. Keylock / Time-Stretch / Pitch-Shift

| Feature | DJ Sugar Mixxx | OpenFadeR | OpenDJ | `djsugar-next` | Best Basis |
|---------|----------------|-----------|--------|----------------|------------|
| Algorithm | SoundTouch / elastique / RubberBand (selectable) | — | Phase vocoder with peak phase-locking (`stretch::Stretch`) | `stretch::Stretch` | **OpenDJ** — `stretch` crate is the algorithm |
| Phase vocoder block/hop | Configurable per backend | — | `block=2048`, `hop=512` (fixed in `Stretch::new`) | `block=2048`, `hop=512` | **OpenDJ** |
| Peak phase-locking | SoundTouch internal | — | `Stretch` tracks spectral peaks, locks bin phases to peak | `Stretch` | **OpenDJ** — core algorithm in `stretch/src/lib.rs` |
| Transient preservation | SoundTouch internal | — | Onset detection (`ONSET_RISE=0.25`) + separate transient handling | `Stretch` | **OpenDJ** |
| Split processing (work spread over interval) | SoundTouch internal | — | `Stretch.split` + `Job` stepping | `Stretch.split` | **OpenDJ** |
| Keylock integration | `RateControl` + `EngineBufferScale` | — | `keyshift.rs` applies `transpose` to `Stretch` per deck | `keyshift.rs` | **OpenDJ** |
| Pitch shift without stretch | `RateControl` + keylock off | — | `Stretch.transpose` (semitones) | `Stretch.transpose` | **OpenDJ** |

---

## 6. Effects — Built-In Rack, Slots, Parameters

| Feature | DJ Sugar Mixxx | OpenFadeR | OpenDJ | `djsugar-next` | Best Basis |
|---------|----------------|-----------|--------|----------------|------------|
| Effect rack architecture | `EffectRack1_EffectUnitN` (4 units × 3 slots) | `Slot` enum (Beat, Color, Mic, Send, Lever, Fader) | `NUM_EFFECT_UNITS=10` × `EFFECTS_PER_UNIT=3` + mic/quick/pad FX | ❌ | **OpenDJ** — maps to Mixxx control groups; **OpenFadeR** `Slot` for plugin ABI |
| Built-in effects list | 30+ (Echo, Reverb, Filter, Flanger, Bitcrusher, Phaser, Gate, Roll, Brake, Noise, Spiral, Drive, Chorus, Ladder, Hall, Rev Roll, Filter Echo, ...) | `dsp.rs`: Delay `Line`, `Lfo`, `Svf`, `Shifter`, `Reverb` (Freeverb) | `AVAILABLE_EFFECTS[18]` + plugin base | ❌ | **OpenDJ** for rack + control mapping; **OpenFadeR** `dsp.rs` for DSP primitives (Svf, Line, Lfo, Shifter, Reverb) |
| Effect parameters (knobs + buttons) | `parameterK`, `button_parameterK` (4+2 per slot) | `Ctx`: `level`, `knob`, `g`, `beats`, `period` | `MAX_PARAMETERS=4`, `MAX_BUTTON_PARAMETERS=2` + `parameter_names()` | ❌ | **OpenDJ** for control surface; **OpenFadeR** `Ctx` for audio-thread context |
| Per-slot effect instances (deck + main mix) | Each (unit,slot,target) has own instance | Plugin instance per slot | `Pool` of delay buffers lent to instances; fades on engage/release | ❌ | **OpenDJ** — buffer pooling + engage/release fades |
| Beat-synced effects (period from `beats`) | `EffectParameterSlot` + `beat_sync` | `Ctx.period` (tempo-slewed samples per beat) | `effects.rs` uses `Ctx.period` for Echo, Roll, etc. | ❌ | **OpenFadeR** `Ctx.period` design; **OpenDJ** for integration |
| Effect tails (release fade) | `release` parameter | `Ctx.g` (engage ramp, 5 ms) + effect `reset` | `RELEASE_BEATS=8.0` + `release` flag in instance | ❌ | **OpenFadeR** `Ctx.g` + `reset`; **OpenDJ** beat-based release |
| Color FX (per-channel post-EQ) | `Color FX` knob per channel | `Slot::Color` (knob = -1..1) | `COLOR_FX` enum + `quick_effect_group` | ❌ | **OpenFadeR** `Slot::Color` mapping; **OpenDJ** for rack |
| Microphone FX | `Mic FX` unit | `Slot::Mic` | `MIC_FX_UNIT` + `input_target()` | ❌ | **OpenFadeR** `Slot::Mic`; **OpenDJ** for routing |
| Send/Return FX | `EffectRack1_EffectUnitN` routed to send | `Slot::Send` | `Station` for FX bus | ❌ | **OpenFadeR** `Slot::Send`; **OpenDJ** `Station` |
| Plugin effects (VST/AU/LV2) | `EffectChain` + LV2 backend | `library` feature: track library plugins; `controller` plugins | `plugins` crate + `plugin-host` (CLAP/VST3 via `djsugar_plugin_host`) | ❌ | **OpenDJ** — `plugin-host` crate loads CLAP/VST3; **OpenFadeR** for simple native plugin ABI |
| Pad FX (performance pads) | `PadFXController` | — | `pad_fx_unit` + `GROOVE_*` pads | ❌ | **OpenDJ** |

---

## 7. EQ / Filter / Gain

| Feature | DJ Sugar Mixxx | OpenFadeR | OpenDJ | `djsugar-next` | Best Basis |
|---------|----------------|-----------|--------|----------------|------------|
| 3-band isolator EQ | `ChannelMixer` + `Biquad` (Linkwitz-Riley 24 dB/oct) | — | `dsp.rs:Isolator` (two LR4 crossovers @ 246/2484 Hz) | ❌ | **OpenDJ** — `Isolator` splits low/mid/high with phase alignment |
| EQ gains (Low/Mid/High) | `gainLow`, `gainMid`, `gainHi` per channel | — | `Channel.eq: [f32;3]` + `Channel.kill: [bool;3]` | ❌ | **OpenDJ** |
| Filter (LPF/HPF sweep) | `Filter` effect + `FilterControl` | `dsp.rs:Svf` (state-variable, ZDF) | `Channel.filter: f32` (morph LPF→HPF) using `Svf` | ❌ | **OpenDJ** — `Svf` is ZDF, sweepable per frame |
| Pre-fader gain | `EnginePreGain` | — | `Channel.gain: f32` | ❌ | **OpenDJ** |
| Channel fader | `ChannelMixer` fader + crossfader | `Slot::Fader` (knob = fader pos) | `Channel.volume: f32` + `Command::Crossfader` | ❌ | **OpenDJ** |
| Crossfader curves | Constant power, linear, cut (`EngineXFader`) | — | `XFADER_CUT_IN` + `crossfader` command | ❌ | **OpenDJ** + **DJ Sugar** for curve math |
| Auto gain (target LUFS) | `AutoGainControl` | — | `DEFAULT_AUTO_GAIN_TARGET` + `loudness.rs` (EBU R128) | ❌ | **OpenDJ** — `loudness.rs` uses `ebur128` crate |

---

## 8. Library — SQLite, Tags, Playlists, Analysis Storage

| Feature | DJ Sugar Mixxx | OpenFadeR | OpenDJ | `djsugar-next` | Best Basis |
|---------|----------------|-----------|--------|----------------|------------|
| SQLite database | `TrackCollection` + `LibraryTableModel` | `compatibility/*` adapters read other apps' DBs | `crates/library/src/lib.rs` — own `library.sqlite` (WAL) | ❌ | **OpenDJ** — complete schema with migrations (`VERSION=13`) |
| Track metadata (tags, rating, color, comment) | `Track` + `LibraryInfo` | Reads Rekordbox, Serato, Traktor, Engine DJ, iTunes, VDJ | `Track` + `LibraryInfo` + `Hotcue` + `Marker` + `BeatGrid` | ❌ | **OpenDJ** — mirrors Mixxx fields; **OpenFadeR** for import adapters |
| Hot cues + beatgrid stored | `TrackCollection` + `Analyzer` writes | Reads other apps' cues/grids | Stored in `library.sqlite` (cues, markers, grid) | ❌ | **OpenDJ** |
| Playlists + folders | `PlaylistTableModel` + tree | — | `playlists` table + `playlist_tracks` + `playlist_folders` | ❌ | **OpenDJ** |
| Smart lists (query-based) | `SearchQuery` + smart playlists | — | `playlists.kind=3` + `query` column | ❌ | **OpenDJ** |
| History (played tracks) | `HistoryListModel` | — | `history` table | ❌ | **OpenDJ** |
| Folder scan (watch + manual) | `TrackCollectionManager` + `Scanner` | — | `stages.rs` (scan stages) + `checklist.rs` | ❌ | **OpenDJ** — staged scan with progress |
| Duplicate detection | `CollectionHealth` | — | `dupes.rs` | ❌ | **OpenDJ** |
| Relocate missing files | `RelocatedTrack` | — | `relocate.rs` + `moves.rs` | ❌ | **OpenDJ** |
| Import other libraries | `LibraryFeature` (iTunes, Serato, Traktor, Rekordbox, Engine DJ, Mixxx, ...) | `compatibility/*` adapters (device, DB, XML) | `crates/plugins/src/*.rs` (importers: `rekordbox.rs`, `serato.rs`, `traktor.rs`, `mixxx.rs`, `engine.rs`, `itunes.rs`, ...) | ❌ | **OpenFadeR** for low-level format parsing (XML, SQLite with SQLCipher); **OpenDJ** for importer logic + UI flow |
| Cover art cache | `CoverArtCache` | — | `cover.rs` | ❌ | **OpenDJ** |

---

## 9. Waveforms — GPU Rendering, Overview, Scrolling, Stems

| Feature | DJ Sugar Mixxx | OpenFadeR | OpenDJ | `djsugar-next` | Best Basis |
|---------|----------------|-----------|--------|----------------|------------|
| GPU waveform rendering | `WaveformWidget` (OpenGL shaders) | — | `crates/waveform/src/lib.rs` — wgpu, fragment shader per frame | ❌ | **OpenDJ** — `WaveformView` + `GpuTrack` texture upload once |
| Summary texture (peaks/RMS at multiple resolutions) | `OverviewCache` | — | `GpuTrack` holds band peaks + RMS at full + coarse levels | ❌ | **OpenDJ** |
| Scrolling waveform (zoomed, centered on playhead) | `WaveformWidget` scrolling mode | — | `Mode::Scrolling` + `Frame.play_seconds` + `visible_seconds` | ❌ | **OpenDJ** |
| Overview waveform (full track, played darkened) | `WaveformWidget` overview | — | `Mode::Overview` | ❌ | **OpenDJ** |
| Stem waveforms (per-stem color, loudest at back) | `StemWaveform` (4 stems) | — | `Style::Stems` + `Extras` (stem levels from `GpuTrack`) | ❌ | **OpenDJ** — `MAX_STEMS=5`, `Style::Stems` |
| Vocal strip (separate vocal waveform) | `VocalWaveform` | — | `Style::Stems` includes vocal | ❌ | **OpenDJ** |
| Cue/loop/hotcue markers on waveform | `WaveformWidget` markers | — | `Frame.cue`, `loop_range`, `hotcues`, `grid` sections | ❌ | **OpenDJ** |
| Phrase markers (structure flags) | `PhraseWaveform` | — | `Frame.phrases` (overview bar, scrolling flags) | ❌ | **OpenDJ** |
| Slip mode ghost playhead | `WaveformWidget` slip | — | `Frame.slip` | ❌ | **OpenDJ** |
| Minute/bar/beat grid lines | `WaveformWidget` grid | — | `Frame.grid` (sections with BPM, bar lines every 4 beats) | ❌ | **OpenDJ** |

---

## 10. Controllers — MIDI/HID, Mappings, Scripting

| Feature | DJ Sugar Mixxx | OpenFadeR | OpenDJ | `djsugar-next` | Best Basis |
|---------|----------------|-----------|--------|----------------|------------|
| MIDI input/output | `ControllerManager` + `MidiInput`/`MidiOutput` | `controller` plugin ABI (host owns ports, plugin drives) | `crates/controllers` — runs Mixxx `.midi.xml` + JS unchanged | ❌ | **OpenDJ** — full Mixxx mapping compatibility; **OpenFadeR** for native plugin ABI design |
| HID controllers (Traktor Kontrol) | `HIDController` + `.hid.xml` | — | `hid.rs` + `controller.sendOutputReport` in JS | ❌ | **OpenDJ** |
| Mapping file format | Mixxx `.midi.xml` + `.js` | — | `mapping.rs` reads XML as Mixxx does | ❌ | **OpenDJ** |
| JavaScript engine (controller scripts) | QuickJS (`ScriptingEngine`) | — | `script.rs` — QuickJS with `engine` + `midi` API | ❌ | **OpenDJ** |
| Controller probing (auto-detect) | `ControllerEnumerator` + `MappingMatcher` | `ControllerPlugin::probe(port) -> score` | `probe.rs` + `manager.rs` | ❌ | **OpenDJ** for Mixxx mapping match; **OpenFadeR** for plugin probe ABI |
| Soft takeover | `SoftTakeover` (pickup) | `Host::takeover(key, on)` | `pickup.rs` + `softtakeover` in `controller.rs` | ❌ | **OpenDJ** + **OpenFadeR** `Host::takeover` |
| MIDI learn (click control, move hardware) | `ControllerLearning` | — | `learn.rs` | ❌ | **OpenDJ** |
| LED feedback / output mapping | `ControllerOutputMapping` | `Host::midi_out` + `watch` | `mapping.rs` output bindings + `controller.sendOutputReport` | ❌ | **OpenDJ** |
| Multiple controllers / layers | `Layer` system (A/B/C/D per deck) | — | `host.rs:LAYER_GROUP`, `LAYER_KEYS` | ❌ | **OpenDJ** |

---

## 11. Streaming — YouTube, SoundCloud, Beatport, etc.

| Feature | DJ Sugar Mixxx | OpenFadeR | OpenDJ | `djsugar-next` | Best Basis |
|---------|----------------|-----------|--------|----------------|------------|
| YouTube search + stream | `YouTubeSource` + `youtube-dl`/`yt-dlp` | — | `crates/plugins/src/youtube.rs` + `crates/net/src/youtube.rs` | `crates/net/src/youtube.rs` | **OpenDJ** — `net` crate has `Hit` + `Error`; `plugins/youtube.rs` for UI integration |
| SoundCloud search + stream | `SoundCloudSource` | — | `crates/plugins/src/audius.rs` (Audius), no SoundCloud | ❌ | **DJ Sugar** for SoundCloud; **OpenDJ** for framework |
| Beatport / Beatsource / TIDAL / Qobuz | Various `*Source` classes | — | `plugins`: `bandcamp.rs`, `audius.rs`, `subsonic.rs`, `jellyfin.rs`, `plex.rs` | ❌ | **OpenDJ** plugin framework; add new sources |
| Local network (Subsonic, Jellyfin, Plex) | `SubsonicSource` etc. | — | `plugins`: `subsonic.rs`, `jellyfin.rs`, `plex.rs` | ❌ | **OpenDJ** |
| Streaming cache (temp files) | `Cache` + `CachedTrack` | — | `cache.rs` in engine | ❌ | **OpenDJ** |

---

## 12. Broadcast — Icecast / Shoutcast

| Feature | DJ Sugar Mixxx | OpenFadeR | OpenDJ | `djsugar-next` | Best Basis |
|---------|----------------|-----------|--------|----------------|------------|
| Icecast/Shoutcast streaming | `BroadcastManager` + `Encoder` | — | `crates/broadcast/src/lib.rs` — ring buffer tap + per-station encode thread | ❌ | **OpenDJ** — complete: Opus/MP3, reconnect, metadata (Opus chained streams) |
| Format (Opus in Ogg, MP3) | `Encoder` (libopus, libmp3lame) | — | `encode.rs` + `resample.rs` | ❌ | **OpenDJ** |
| Multiple stations | `BroadcastManager` station list | — | `Station` struct + per-station thread | ❌ | **OpenDJ** |
| Metadata (title/artist) | `Encoder` metadata | — | `opus_titles` (chained Ogg) + Icecast mount metadata | ❌ | **OpenDJ** |

---

## 13. Stem Separation — Spleeter / HTDemucs

| Feature | DJ Sugar Mixxx | OpenFadeR | OpenDJ | `djsugar-next` | Best Basis |
|---------|----------------|-----------|--------|----------------|------------|
| Spleeter (fast, 2/4/5 stem) | `SpleeterSeparator` (ONNX Runtime) | `stems` feature + `ort` (ONNX Runtime) | `crates/separation/src/spleeter.rs` (burn) | ❌ | **OpenDJ** — pure Rust on `burn` (GPU: Metal/Vulkan/WGPU); **OpenFadeR** for ONNX Runtime integration reference |
| HTDemucs (high quality, 4/6 stem) | `HTDemucsSeparator` (ONNX Runtime) | `stems` feature + `ort` | `crates/separation/src/htdemucs.rs` (burn + `demucs-core`) | ❌ | **OpenDJ** — `demucs-core` + `burn`; **OpenFadeR** for model download/sha256 |
| GPU acceleration (Metal, Vulkan, CUDA) | ONNX Runtime providers | `ort/webgpu` (Dawn) | `burn` wgpu backend (Metal on macOS, Vulkan on Linux) | ❌ | **OpenDJ** — `burn` abstraction; **OpenFadeR** for Dawn/WebGPU specifics |
| Background separation (non-blocking) | `StemsWorker` thread | — | `separation` crate called from engine `separation.rs` | ❌ | **OpenDJ** |
| Stem file caching (`.stem.mp4`) | `StemFileWriter` | — | `separation` writes companion files | ❌ | **OpenDJ** |

---

## 14. OSC / Ableton Link / Remote / MIDI Clock

| Feature | DJ Sugar Mixxx | OpenFadeR | OpenDJ | `djsugar-next` | Best Basis |
|---------|----------------|-----------|--------|----------------|------------|
| OSC (Open Sound Control) | `OscManager` | — | `crates/osc/src/lib.rs` — every control by address, feedback, subscribe | ❌ | **OpenDJ** — complete: `/channel1/play`, feedback rate 30 Hz |
| Ableton Link | `LinkManager` (abletonlink) | — | `crates/link/src/lib.rs` — `AblLink` + `Timeline` + multicast | ❌ | **OpenDJ** — lock-free timeline, audio-thread capture/commit |
| Phone remote (web UI) | `Remote` (web server + WebSocket) | — | `crates/remote/src/lib.rs` — QR pairing, WebSocket JSON API | ❌ | **OpenDJ** — `Server` + `pairing` + `page.html` |
| MIDI Clock output | `MidiClockSender` | — | `crates/engine/src/midi_clock.rs` — `MidiClock` from engine tempo | ❌ | **OpenDJ** |
| MIDI Clock input (sync to external) | `MidiClockReceiver` | — | ❌ | ❌ | **DJ Sugar** |

---

## 15. Recording / Sampler / DVS / Other

| Feature | DJ Sugar Mixxx | OpenFadeR | OpenDJ | `djsugar-next` | Best Basis |
|---------|----------------|-----------|--------|----------------|------------|
| Recording (WAV, AIFF, FLAC, MP3, Ogg) | `Recorder` + `Encoder` | — | `crates/engine/src/recorder.rs` — `Recorder` + `RecordFormat` | ❌ | **OpenDJ** |
| Sampler decks (16 slots × 4 banks) | `SamplerController` | — | `crates/engine/src/sampler.rs` — `SamplerInput` + `NUM_SAMPLERS` | ❌ | **OpenDJ** |
| DVS / Timecode vinyl (Serato, Traktor, MixVibes) | `VinylControl` + `PositionScratchController` | — | ❌ | ❌ | **DJ Sugar** only |
| Pitch bend / nudge (temp + perm) | `RateControl` temp/perm | — | `Command::Nudge` + `Command::Rate` | `Command::Nudge` | **OpenDJ** |
| Beatloop / rolling loop | `LoopControl` beatloop | — | `BEATLOOP_SIZES` + loop commands | ❌ | **OpenDJ** |
| Microphone input (talkover, ducking) | `EngineTalkoverDucking` + `EngineSidechainCompressor` | `Slot::Mic` | `inputs.rs` + `MICROPHONE` + `Station` for mic FX | ❌ | **DJ Sugar** for sidechain/ducking; **OpenFadeR** `Slot::Mic` for FX routing |
| Lyrics (LRC, synced) | `Lyrics` + `AnalyzerLyrics` | — | `crates/engine/src/lyrics.rs` + `crates/app/src/lyrics.rs` | ❌ | **OpenDJ** |
| Groove / performance pads | `GroovePadController` | — | `crates/engine/src/groove*.rs` (dice, fills, pads) | ❌ | **OpenDJ** |

---

## Summary: Best Basis per Category

| Category | Best Helper Project | Why |
|----------|---------------------|-----|
| **Engine core / mixer / threading** | **OpenDJ** | Complete lock-free command queue, triple-buffer telemetry, per-deck `Stretch` integration |
| **Deck logic (cue, loop, sync, quantize, slip)** | **OpenDJ** | Pure Rust `BeatGrid` with variable tempo, `GridEdit` enum, `keyshift` + `stretch` |
| **BPM/Beatgrid analysis** | **OpenDJ** | `analysis::grid()` flux+autocorr; `beats.rs` variable-tempo grid with whole-beat snapping |
| **Key detection / harmonic mixing** | **OpenDJ** | `key.rs` Krumhansl-Schmuckler + `keynet.rs` compatibility graph |
| **Time-stretch / keylock** | **OpenDJ** (`stretch` crate) | Peak-phase-locked phase vocoder with transient preservation, split processing |
| **Effects rack + built-ins** | **OpenDJ** (rack) + **OpenFadeR** (DSP primitives) | OpenDJ: rack architecture, parameter mapping, buffer pooling, engage/release fades. OpenFadeR: `Svf` (ZDF), `Line` (interp delay), `Lfo`, `Shifter`, `Reverb` (Freeverb) |
| **EQ / Filter** | **OpenDJ** | `Isolator` (LR4 crossover, phase-aligned), `Svf` sweepable filter |
| **Library (SQLite, tags, playlists, import)** | **OpenDJ** (schema + logic) + **OpenFadeR** (format parsers) | OpenDJ: complete schema v13, smart lists, scan stages, cover art. OpenFadeR: Rekordbox/Serato/Traktor/Engine DJ/iTunes/VDJ adapters with SQLCipher |
| **Waveforms (GPU)** | **OpenDJ** | `wgpu` fragment shader, `GpuTrack` summary texture, `Style::Stems`, phrase/grid/cue overlays |
| **Controllers (MIDI/HID + JS)** | **OpenDJ** | Runs Mixxx `.midi.xml` + `.js` unchanged; QuickJS engine; HID support; soft takeover |
| **Streaming (YouTube, etc.)** | **OpenDJ** | `net` crate `Hit/Error`, `plugins` framework for store sources |
| **Broadcast** | **OpenDJ** | Ring buffer tap, per-station encode thread, Opus chained streams, reconnect |
| **Stem separation** | **OpenDJ** (`separation` + `burn`) | Pure Rust Spleeter/HTDemucs on GPU (Metal/Vulkan), background worker, companion files |
| **OSC / Link / Remote** | **OpenDJ** | Complete OSC address mapping, lock-free Ableton Link, QR-paired phone remote |
| **DVS / Timecode** | **DJ Sugar Mixxx** | Only project with complete vinyl control (Serato/Traktor/MixVibes, relative/absolute) |

---

## Top 15 Highest-Value Things to Port (Ranked)

1. **`stretch` crate (phase vocoder with peak phase-locking)** — Core keylock algorithm; already in `djsugar-next/crates/stretch` but verify integration with `keyshift`.
2. **`analysis::grid()` (flux + autocorr BPM/beatgrid)** — Pure Rust, fast, accurate; `djsugar-next/crates/analysis` has it; ensure `BeatGrid` variable-tempo from OpenDJ `beats.rs` is ported.
3. **`dsp::Svf` (ZDF state-variable filter)** — Sweepable LPF/HPF/BP for channel filter + EQ crossover; basis for `Isolator`.
4. **`dsp::Isolator` (3-band LR4 crossover, phase-aligned)** — The DJ EQ sound; copy algorithm (two LR4 crossovers + all-pass for low band).
5. **`engine::BeatGrid` + `GridEdit`** — Variable-tempo grid with whole-beat snapping, all edit operations (beat here, shift, tempo, add/remove section, scale, bar, undo, lock, reset).
6. **`engine::keyshift` + `Key` detection** — Krumhansl-Schmuckler key detection, Camelot/OpenKey notation, transpose, `keynet` compatibility.
6. **`engine::effects` rack architecture** — 10 units × 3 slots, `Pool` of delay buffers, per-target instances, engage/release fades, beat-synced `period`, parameter/buttons mapping, plugin base.
7. **`waveform` GPU rendering (`GpuTrack` + fragment shader)** — Summary texture upload once, per-frame uniform block, `Style::Stems`, phrase/grid/cue overlays.
8. **`library` SQLite schema + migrations + scan stages** — Complete v13 schema, WAL, smart lists, cover art, import pipeline.
9. **`controllers` Mixxx mapping compatibility** — `.midi.xml` + `.js` parsing, QuickJS `engine`/`midi` API, HID, soft takeover, layer system.
10. **`separation` Spleeter/HTDemucs on `burn`** — GPU stem separation (Metal/Vulkan), background worker, companion file cache.
11. **`broadcast` Icecast/Shoutcast with Opus chained streams** — Ring buffer tap, per-station thread, reconnect, metadata.
12. **`link` Ableton Link (lock-free timeline)** — Audio-thread capture/commit, multicast peer discovery, clock sync.
13. **`osc` full control surface via OSC** — Every control addressable, feedback, subscribe, 30 Hz rate limit.
14. **`remote` phone web remote (QR + WebSocket)** — Pairing, device tokens, JSON RPC + pushed state.
15. **OpenFadeR `compatibility/*` format parsers** — Rekordbox (device/DB/XML), Serato, Traktor, Engine DJ, iTunes, VDJ — for library import; use SQLCipher for encrypted DBs.

---

## Notes for `djsugar-next` Implementation

- **No copying**: Re-implement algorithms in your own modules. Use the helper projects as *reference implementations* for math/approach.
- **DSP primitives first**: Port `Svf`, `Line`, `Lfo`, `Shifter`, `Reverb` from OpenFadeR `dsp.rs` → your `dsp` crate. These are building blocks for effects, EQ, filter.
- **BeatGrid before effects**: Variable-tempo grid (`beats.rs`) is needed for beat-synced effects (Echo, Roll, Brake).
- **Stretch before keylock**: `stretch::Stretch` must work standalone; then `keyshift` applies semitone transpose per deck.
- **Library schema early**: Define `library.sqlite` schema (port from OpenDJ v13) so scan/import/analysis have a target.
- **Controller mapping compatibility**: This is a force multiplier — running Mixxx mappings unmodified gives instant hardware support.
- **GPU waveforms**: `wgpu` + fragment shader is the modern path; `GpuTrack` summary texture design avoids per-frame CPU→GPU uploads.
- **Plugin host**: OpenDJ `plugin-host` (CLAP/VST3) is more relevant than OpenFadeR's simple native ABI for effects.
- **DVS**: Only DJ Sugar has this. If needed, port `VinylControl` + `PositionScratchController` algorithms (timecode decode → position/speed).
# DJ Sugar Mixxx Fork — Feature & UI/UX Study

**Source:** `/home/ubuntu/djsugar` (C++/Qt, Mixxx-based, branch `main`)  
**Author:** winnerspiros (DJ Sugar additions)  
**Date:** November 2026

---

## 1. Complete Feature Inventory (Grouped)

### 1.1 Decks / Transport
| Feature | Description |
|---------|-------------|
| 4 Decks | Four independent deck channels (`[Channel1]`–`[Channel4]`), togglable 2/4 deck mode via toolbar |
| Play/Pause/Cue | Standard transport with cue point return, double-tap deck focus toggle |
| Pitch Fader | Rate control with sync, key lock, tempo range ±100%; right sidebar rate controls (Fine/Coarse, ±8/16/50/100%) |
| Keylock | SoundTouch (default) + RubberBand (Faster/Finer/R3 Short Window) engines; per-deck selection |
| Beatjump | Forward/backward jump by 1/4, 1/2, 1, 2, 4, 8, 16, 32 beats; configurable via skin |
| Slip Mode | Continues playback position while scratching/looping; restores on release |
| Vinyl Control | Timecode vinyl (Serato, Traktor, MixVibes); xwax processor; steady-pitch mode; signal quality widget |
| Quantize | Snap seeks, loops, hotcues to beatgrid; configurable quantization values |
| Passthrough | Hardware thru mode for external mixer integration |

### 1.2 Sync / Beatgrid / Key
| Feature | Description |
|---------|-------------|
| Sync | Multi-deck sync with leader/follower; tempo + phase alignment; `EngineSync` + `SyncControl` |
| Beatgrid | Editable beat markers (grid, anchors, BPM); `analyzerbeats.cpp`, `beats.cpp`; double-click to set anchor |
| BPM Detection | Multi-algorithm analysis (autocorrelation, spectral flux); `analyzerbeats.cpp`, `bpm.cpp` |
| Key Detection | Key + camelot notation; `analyzerkey.cpp`, `keyutils.cpp`; harmonic mixing wheel |
| Key Lock | Pitch-independent tempo change (see Keylock above) |
| Phrase Detection | 4/8/16/32-beat phrase boundaries; used by smart AutoDJ |

### 1.3 Hotcues / Loops / Beatjump / Rolls
| Feature | Description |
|---------|-------------|
| Hotcues | 8 hotcues per deck (A–H); color-coded; editable labels; jump/play/stutter modes |
| Intro/Outro Cues | Special markers for mix-in/mix-out zones; shown on waveform overview |
| Loops | Auto-loop (1/2/4/8/16/32 beats); manual loop in/out; loop move/resize; quantized |
| Loop Roll | Beat-synced roll (1/16–1 beat) via pad mode; `beatrepeateffect.cpp` |
| Beatjump | Dedicated beatjump controls (see 1.1) |
| Cue Types | Hotcue, Loop, Intro, Outro, Load, Grid; `cue.cpp`, `cueinfo.cpp` |

### 1.4 Samplers
| Feature | Description |
|---------|-------------|
| 64 Sampler Slots | Configurable banks (16×4); `num_samplers` control; `SamplersRack.qml` |
| Sample Player | One-shot, loop, reverse; velocity-sensitive pads; pitch control |
| Sample Recorder | Record from deck/main mix; trim/normalize; `samplerecorder.cpp` |
| Sample Editor | Trim, fade, normalize, reverse, pitch; `sampleeditor.cpp` |
| Bundled Samples | 24 built-in samples (kick, snare, clap, hats, risers, impacts, vinyl stop, airhorn, etc.) in `res/samples/djsugar/` |
| FX Station X-Pad | 4 banks × 4 instruments (kick/snare/clap/hat) with rolls 1/1–1/16, pitch, overdub loop, mutes; `fxstationinstruments.cpp` |

### 1.5 Effects — Complete List (Built-in Backend)
**Mixing EQs**  
- Bessel 4th-order LV Mix EQ  
- Bessel 8th-order LV Mix EQ  
- Linkwitz-Riley 8th-order EQ  
- Three-Band Biquad EQ  
- Biquad Full-Kill EQ  

**Compensation EQs**  
- Graphic EQ (31 bands)  
- Parametric EQ (4 bands)  
- Loudness Contour (Fletcher-Munson)  

**Filter / Fading Effects**  
- Filter (LP/HP/BP/Notch)  
- Moog Ladder 4-Pole Filter  
- Bitcrusher  
- White Noise  
- Balance (L/R)  
- Auto Gain Control  
- Gain  

**Fancy Effects**  
- Flanger  
- Echo  
- DJ Echo (tempo-synced ping-pong)  
- Pro Compressor  
- Pro Flanger  
- Vintage Delay (tape-style)  
- Auto Pan  
- Pro Reverb (algorithmic)  
- Pro Filter (analog-modelled)  
- Pro Noise  
- Beat Repeat (loop roll)  
- Tape Stop  
- Back Spin (hand-throw + coast, needle model)  
- Reverb (convolution, disabled on Mac App Store)  
- Phaser (8-stage all-pass)  
- Metronome Click  
- Key Comparison (harmonic mixing aid)  
- Spiral (up/down sweep)  
- Tremolo  
- Pitch Shift (WSOLA)  
- Distortion  
- Glitch  
- Compressor (basic)  

**FX Station — Scene Effects** (`fxstationscenes.h`)  
- Flanger (beat-swept, stereo quarter-cycle offset)  
- Phaser (8-stage, log sweep 120 Hz–5 kHz, opposite channels)  
- Sweep Filter (LP left of center, HP right, 24 dB/oct, resonant)  
- Roll (WSOLA-recorded slice repeat with decay + 2 ms crossfades)  
- Gate / Trans (raised-cosine chop, width + softness)  
- Pitch Shifter (WSOLA, ~20 ms latency, phase-matched jumps)  
- Drive (2× oversampled tanh saturation, antiderivative AA, even-harmonic bias, tone LP, level-matched)  

**FX Station — Sections**  
- Scene (17 effects on big dial + 2 sub-params)  
- Isolate (3 bands × 4 modes: Isolator, Cut/Add, Trans/Roll, Gate/Drive)  
- X-Pad (4 instruments × 4 banks, rolls, pitch, overdub loop, mutes)  
- Release (Vinyl Brake, Echo, Back Spin — kills other sections)

### 1.6 EQ / Filters / Mixer
| Feature | Description |
|---------|-------------|
| Channel EQ | 3-band (Low/Mid/High) + Full Kill; per-deck; Bessel/Linkwitz-Riley/Biquad options |
| Master EQ | Graphic / Parametric / Loudness Contour on main mix |
| Crossfader | Adjustable curve (transform/constant power); calibration; cut-in points |
| Crossfader Modes | Scratch, Constant Power, Linear, Custom |
| Headphone Cue | Split cue (L/R), mix knob, dedicated headphone gain/delay |
| Booth Output | Independent gain/delay/EQ |
| Talkover Ducking | Automatic music duck on mic input; `EngineTalkoverDucking` |
| Sidechain Compressor | Main mix sidechain from external source; `EngineSideChain` |
| Main/Booth/Headphone Delays | Latency compensation per output; `EngineDelay` |
| VuMeters | Peak + RMS; compact mode; `EngineVuMeter` |

### 1.7 Library / Crates / Playlists / History / AutoDJ / Search
| Feature | Description |
|---------|-------------|
| Track Collection | SQLite-backed; `TrackCollection`, `TrackCollectionManager`; import from iTunes, Traktor, Serato, Rekordbox, Banshee, Rhythmbox, Engine DJ |
| Crates | Hierarchical crates (folders); `Crates` model; drag-drop reorganization |
| Playlists | Standard + smart playlists; `PlaylistFeature`, `BasePlaylistFeature`; export M3U/PLS/CSV |
| History | Session history (played tracks, timestamps); `SetLogFeature`; auto-save per session |
| AutoDJ | Queue with crossfade, fade-in/out, tempo transition; `AutoDJFeature`, `AutoDJProcessor` |
| Smart AutoDJ | Beatmatch, Bridge Finder, Energy Store, Genre Scan, Mix Scorer, Phrase Align, Vocal Map; `smart/` |
| Search Syntax | `artist:`, `title:`, `album:`, `genre:`, `composer:`, `grouping:`, `comment:`, `location:`, `directory:`, `crate:`, `type:`, `year:`, `key:`, `bpm:`, `duration:`, `added:`, `played:`, `rating:`, `bitrate:`, `track:`; prefixes `-` (NOT), `~` (fuzzy); quoted phrases; OR operator (`|` or `OR`); `SearchQueryParser` |
| Cover Art | Embedded + external; cache; WebP support (`qtimageformats[webp]`); `coverart.cpp`, `coverartcache.cpp` |
| Greeklish Search | Transliterated Greek search; `library.cpp` |

### 1.8 Analysis (BPM / Key / Waveform / ReplayGain)
| Feature | Description |
|---------|-------------|
| BPM Analysis | Multi-threaded; `AnalyzerThread`, `AnalyzerBeats`; background + priority queue |
| Key Analysis | Key + camelot; `AnalyzerKey`; harmonic mixing wheel |
| Waveform Overview | Multi-resolution (overview + zoomed); `AnalyzerWaveform`, `WaveformOverviewRenderer` |
| Waveform Zoom | Synchronized zoom across decks (optional); mouse wheel zoom; animated smooth zoom |
| ReplayGain | EBU R128 loudness; `AnalyzerEbur128`, `AnalyzerGain`; per-track + album gain |
| Energy Analysis | Per-beat energy curve; `AnalyzerEnergy`, `EnergyCalculator`; used by smart AutoDJ |
| Silence Detection | Auto-cue on silence; `AnalyzerSilence` |
| Android Kick Grid | Mobile-optimized beatgrid; `AndroidKickGrid` |

### 1.9 Controllers / MIDI / HID
| Feature | Description |
|---------|-------------|
| MIDI | PortMidi + BLE MIDI; `MidiController`, `PortMidiController`, `BleMidiController`; learning mode; soft takeover |
| HID | Raw HID + report descriptor parser; `HidController`, `HidEnumerator`; 230 KB usage table (`hidusagetablesdata.h`) |
| Bulk Controllers | Generic bulk endpoint support; `BulkController` |
| Keyboard | Full keyboard mapping; `KeyboardController` |
| Controller Mapping | XML + JS scripting; `ControllerMappingInfo`, `ControlPickerMenu`; learning UI (`DlgControllerLearning`) |
| Scripting | JavaScript controller scripts; `controllers/scripting/` |
| Screen Preview | Controller screen rendering (Traktor S4 MK3, etc.); `ControllerScreenPreview` |

### 1.10 Recording / Broadcasting
| Feature | Description |
|---------|-------------|
| Recording | WAV/FLAC/MP3/OGG/OPUS; split by track; `RecordingManager`; threaded encoder |
| Broadcasting | Icecast/Shoutcast; MP3/OGG/OPUS; metadata push; `BroadcastManager`; separate thread |
| Encoders | LAME, FLAC, Vorbis, Opus, FAAD (AAC); `encoder/` |

### 1.11 Vinyl Control
| Feature | Description |
|---------|-------------|
| Timecode Formats | Serato (CV02), Traktor (Scratch), MixVibes (DVS) |
| Processor | xwax-based (`vinylcontrolxwax.cpp`); steady-pitch mode; signal quality widget |
| Calibration | Auto-calibration wizard; noise threshold; RPM detection |
| CDJ Mode | HID/USB control for CDJ-2000/3000, XDJ series |

### 1.12 Preferences / Settings
| Category | Key Options |
|----------|-------------|
| Audio | Sample rate (44.1/48/96 kHz), buffer size (16–4096), latency compensation, API (PortAudio/JACK/CoreAudio/WASAPI/ALSA/Oboe) |
| Library | Auto-analyze, watch folders, iTunes/Traktor/Serato import, cover art fetch |
| Interface | Skin (LateNight, LateNightQML, Deere, Shade, Tango), language, waveforms, deck layout |
| Controllers | MIDI/HID mapping, learning, soft takeover, LED feedback |
| Recording | Format, bitrate, split mode, directory |
| Broadcasting | Server, mount, format, bitrate, metadata |
| Vinyl Control | Format, calibration, lead-in time, needle drop |
| Effects | Effect rack layout, 4-unit mode, FX Station enable |

---

## 2. DJ Sugar Fork Additions (with Commit Refs / Files)

| Feature | Commit(s) | Key Files |
|---------|-----------|-----------|
| **FX Station** (remix effector: Scene/Isolate/X-Pad/Release) | `93aafee913` (feat/effects-overhaul), `f9d5cc4792` (7 new Scene effects), `f4d63c6bd5` (swap Scene/Release, Back Spin global), `63a7eba1e6` (WSOLA Pitch) | `src/engine/fxstation/*.cpp/h`, `src/effects/backends/builtin/backspineffect.cpp`, `src/effects/backends/builtin/beatrepeateffect.cpp`, `src/effects/backends/builtin/flangereffect.cpp`, `phasereffect.cpp`, `filtereffect.cpp`, `pitchshifteffect.cpp`, `glitcheffect.cpp`, `pronoiseeffect.cpp`, `proreverbeffect.cpp`, `proflangereffect.cpp`, `profiltereffect.cpp`, `procompressoreffect.cpp` |
| **Streaming Services** (YouTube, SoundCloud, Spotify via YouTube backend) | `aa349495b1` (streaming rows hold menu), `f8a5e7e716` (drag streaming tracks), `240cb87150` (Greeklish search, playlist folders), `8950446b20` (modern library sidebar), `4d7ec002f1` (track menu card) | `src/sources/youtube/soundsourceyoutube.cpp`, `src/library/youtube/youtubeservice.cpp`, `youtubefeature.cpp`, `youtubeaccount.cpp`, `sponsorblockcontroller.cpp`, `youtubetrackmodel.cpp`, `src/library/youtube/soundcloudaccount.h` |
| **Bundled Samples** (24 samples in `res/samples/djsugar/`) | `28f3acfa16` (samples: hub intro, League kill calls, Fatality, Teams call), `ecf94a6a87` (normalize to ~-14 LUFS) | `res/samples/djsugar/*.wav/*.ogg`, `src/library/samples/bundledsamples.cpp` |
| **WebP Cover Art** | `c144fe7f88` (qtimageformats[webp] plugin) | `src/library/coverartutils.cpp` |
| **Lyrics Support** | Implied by `youubefeature.cpp` metadata fetching | `src/library/youtube/youtubefeature.cpp` |
| **LateNightQML Skin** (complete QML rewrite) | Multiple merges from upstream (`a86fa6eb7e`, `dc4bf99322` mixxxdj#17270) | `res/skins/LateNightQML/` |
| **Back Spin Effect** (hand throw + coast, needle model) | `f4d63c6bd5`, `161f8d96ee` (format) | `src/effects/backends/builtin/backspineffect.cpp`, `src/engine/fxstation/fxstationscenes.h` (BackSpin Release) |
| **Improved Library Scanner** (songs show within seconds, cancellation) | `3016e6dab6`, `024e5a849a` (cancellation checks), `170d9e7995` (skip phone/app folders) | `src/library/scanner/`, `src/library/androidscanorder.cpp` |
| **Modern Track Menu** (Artist-Title card, hold→menu for decks/download/playlist) | `a389bbe6eb`, `aa349495b1`, `4d7ec002f1` | `src/qml/` (TrackMenu), `src/library/youtube/youtubeservice.cpp` |
| **Playlist Folders + Multi-create** | `240cb87150`, `04200834af` (remove playlist folders), `cf33997461` (track menu: check playlists, add/remove many) | `src/library/trackset/playlistfeature.cpp` |
| **Android Optimizations** (no debug logs, no full rescan, SponsorBlock off) | `4090a4fb4d`, `170d9e7995` | `src/library/androidscanorder.cpp`, `src/controllers/android.cpp` |
| **FX Station X-Pad** (app samples bank 1, edit icon, shared picker) | `5f2b82adf4`, `13e382ffc0` | `src/engine/fxstation/fxstationinstruments.cpp`, `res/skins/LateNightQML/Samplers/` |
| **Greeklish Search** | `240cb87150` | `src/library/library.cpp` |
| **SponsorBlock Integration** (YouTube) | `9bf34759ab` (resume dropped chunks), `src/library/youtube/sponsorblockcontroller.cpp` | `src/library/youtube/sponsorblockcontroller.cpp` |
| **iOS Build Support** | `67c442abf6`, `377f8c3e68` | `tools/ios_buildenv.sh`, `.github/workflows/build.yml` |
| **WebP Plugin + Cover Art Extension** | `c144fe7f88` | `src/library/coverartutils.cpp` |

---

## 3. UI/UX Notes — LateNight & LateNightQML Skins

### 3.1 Layout Regions (LateNightQML `MainWindow.qml`)
```
┌─────────────────────────────────────────────────────────────┐
│ Toolbar (deck size, 4-deck toggle, effects, sampler, mic,   │
│ library maximize, mixer, waveforms, vinyl, key controls)    │
├───────────────────┬───────────────────┬─────────────────────┤
│ Deck 1 (left)     │   Mixer (center)  │ Deck 2 (right)      │
│ [Full/Compact/    │  [Channel EQs,    │ [Full/Compact/      │
│  Mini state]      │   crossfader,     │  Mini state]        │
│                   │   cue/mix,        │                     │
│ Deck 3 (opt)      │   master/booth]   │ Deck 4 (opt)        │
├───────────────────┴───────────────────┴─────────────────────┤
│ Effects Rack (collapsible, spring animation)                │
├─────────────────────────────────────────────────────────────┤
│ Samplers Rack (64 slots, collapsible)                       │
├─────────────────────────────────────────────────────────────┤
│ Mic/Aux Rack (collapsible)                                  │
├─────────────────────────────────────────────────────────────┤
│ Library (maximizable to full height; animated transition)   │
└─────────────────────────────────────────────────────────────┘
```

**Key Layout Behaviors:**
- **Deck States**: Mini (waveform only) → Compact (transport + mini waveform) → Full (all controls); animated via `deckHeightAnimation` (250 ms OutCubic) or big-library transition (80–100 ms)
- **Big Library Mode**: Library expands to full height; decks shrink to Mini or hide; smooth `AnchorAnimation` + `SpringAnimation` (damping 0.2, spring 2)
- **Compact VuMeters**: Optional center slot between decks; animated width spring
- **Effects/Samplers/Mic**: Collapsible drawers below decks; `SpringAnimation` on height (500 ms, damping 0.2)

### 3.2 Per-Deck Display (Full Deck — `FullDeck.qml`)
| Row | Content |
|-----|---------|
| **Top** | FX Assignment buttons (4 units), vinyl control placeholder, Key Controls (camelot wheel + shift) |
| **Middle** | **Left**: Big Spinny/Cover Art (rotating vinyl or cover) — toggleable Large/Small<br>**Right**: Title/Artist/Time rows (`TitleTimeRows`) + Overview Row (waveform overview + small spinny) |
| **Bottom** | Transport/Loop/Beatjump/Hotcues (`TransportLoopBeatjumpPlaceholders`) — 8 hotcues, intro/outro, loop in/out/move, beatjump 1/2/4/8/16/32 |
| **Right Sidebar** | Rate Controls (Fine/Coarse, tempo range buttons, sync, keylock, master tempo) |

**Compact Deck**: Collapses middle row; keeps transport + mini overview.  
**Mini Deck**: Waveform only + play/cue.

### 3.3 Waveform Types (`LateNightWaveformDisplay.qml`, `waveformrenderer*.cpp`)
| Type | Description | Renderer |
|------|-------------|----------|
| **Simple** | Mono envelope | `WaveformRendererSimpleSignal` |
| **Filtered** | 3-band split (Low/Mid/High) colored | `WaveformRendererFilteredSignal` |
| **HSV** | Frequency → hue, amplitude → value | `WaveformRendererHSV` |
| **RGB** | Low=Red, Mid=Green, High=Blue | `WaveformRendererRGB` |
| **Stacked** | Overview + zoomed stacked vertically | `WaveformStack.qml` + `WaveformOverviewRenderer` |
| **Stem** | Per-stem waveforms (vocals/drums/bass/other) | `StemControls.qml` + stem renderers |

**Interactions:**
- Left-click drag → scratch (`scratch_position` control)
- Right-click drag → pitch bend (`wheel` control)
- Mouse wheel → zoom (1–10×, animated `SmoothedAnimation` 500 ms)
- Double-click → toggle stem split view
- Beat markers, cue markers, loop range, intro/outro markers rendered as overlays (`WaveformMark`, `WaveformMarkRange`)

### 3.4 Color Scheme (`LateNightTheme.qml`)
| Role | Classic | Pale Moon |
|------|---------|-----------|
| Deck Panel | `#1a1a1a` | `#2d2d2d` |
| Waveform BG (Primary) | `#0d0d0d` | `#1a1a1a` |
| Waveform BG (Secondary) | `#121212` | `#1f1f1f` |
| Primary Signal | `#00d4aa` (teal) | `#00d4aa` |
| Secondary Signal | `#ff6b6b` (coral) | `#ff6b6b` |
| Cue Marker | `#ffeb3b` (yellow) | `#ffeb3b` |
| Loop Marker | `#00d4aa` | `#00d4aa` |
| Intro/Outro | `#ff9800` (orange) | `#ff9800` |
| Play Position | `#ffffff` | `#ffffff` |
| Beat Axes | `#333333` | `#444444` |
| End-of-Track Warning | `#ff4444` | `#ff4444` |
| Passthrough Label | `#888888` | `#aaaaaa` |

**Typography**: Open Sans; dynamic sizing; `deckPanelBorderDark` `#000000` / `#111111`.

### 3.5 Interactions That Make It Great
1. **Animated Layout Transitions** — Every region change (deck size, library maximize, effects toggle) uses spring/OutCubic animations; feels responsive, not jarring.
2. **Focusable Decks** — Double-click title row → deck gains focus (highlighted border); keyboard shortcuts target focused deck.
3. **Unified Waveform Stack** — Overview + zoomed waveform in single component; synchronized zoom; stem split on double-click.
4. **FX Assignment at Top** — Always visible; 4-unit mode toggle; no menu diving.
5. **Search Syntax in Library** — Advanced filters (`bpm:120..130 key:8A genre:house`) work instantly; fuzzy `~` and negation `-` supported.
6. **Sample Pads with Overdub** — X-Pad records 1-bar overdub loop; mutes per instrument; 4 banks.
7. **Smart AutoDJ** — Phrase-aware, energy-matched, key-compatible transitions; bridge finder for genre shifts.

### 3.6 Weak Spots / Pain Points
| Issue | Location | Impact |
|-------|----------|--------|
| **No Waveform Color Customization** | `LateNightTheme.qml` hardcodes colors; no user palette editor | Power users can't adapt to color blindness or preference |
| **FX Station UI Only in QML** | `res/skins/LateNightQML/Effects/`; classic skin lacks FX Station rack | Classic LateNight users miss the flagship feature |
| **Stem Separation Not Real-Time** | `soundsourcestem.cpp` — offline only; no live stem EQ/mute on deck | Stem controls show placeholders but no live processing |
| **Library Scan Blocks UI on Large Collections** | `AnalyzerThread` runs in background but DB writes cause micro-stutters | 10k+ tracks → occasional frame drops |
| **Controller Mapping UI Complexity** | `DlgControllerLearning` + JS scripting steep learning curve | New users struggle to map unsupported hardware |
| **No Cloud Sync for Cues/Loops** | Local SQLite only; export/import manual | Multi-device DJs lose metadata |
| **Android Library Scan Aggressive** | Full rescan on start (mitigated in fork but still heavy) | Slow startup on mobile |

---

## 4. Engine Architecture Lessons (Worth Reimplementing)

### 4.1 Audio Thread Design (`EngineMixer`, `EngineBuffer`, `EngineWorkerScheduler`)
- **Single Audio Thread** — All DSP (mixing, effects, EQ, sync, FX Station) runs on one real-time thread (`EngineWorkerScheduler` + `EngineWorker`); no locks, no allocations.
- **Lock-Free Control Proxies** — `ControlProxy` / `ControlObject` use atomic value exchanges; UI thread writes, audio thread reads; no mutexes on hot path.
- **Fixed Block Size** — `kMaxEngineSamples` (default 4096 frames @ 48 kHz ≈ 85 ms); all buffers pre-allocated; `SampleBuffer` ring buffers for delay/recording.
- **Worker Scheduler** — `EngineWorkerScheduler` divides work into parallelizable units (per-channel buffer processing, effects, analysis) but keeps serial mixdown; uses `QThreadPool` with high priority.
- **Latency Compensation** — Per-output `EngineDelay` (main/headphones/booth) + `AudioLatencyCalibrator` measures round-trip; reported via `audio_latency_usage` / `audio_latency_overload` controls.

### 4.2 Buffer Management (`EngineBuffer`, `CachingReader`, `ReadAheadManager`)
- **ReadAheadManager** — Prefetches decoded frames into `ReadaheadFrameBuffer`; decouples disk I/O from audio thread; configurable lookahead (default 2 s).
- **CachingReader** — Caches decoded PCM for recently played tracks; LRU eviction; enables instant cue/jump.
- **EngineBufferScale** — Time-stretch/pitch-shift (SoundTouch / RubberBand) applied per-buffer; `EngineBufferScaleLinear` for keylock off; keylock engines selectable at runtime.
- **Slip Mode** — `SlipModeState` tracks virtual play position while looping/scratching; restores on exit without seeks.

### 4.3 Sync Architecture (`EngineSync`, `SyncControl`, `InternalClock`)
- **InternalClock** — Global tempo master (tappable, MIDI clock syncable); drives FX Station + AutoDJ.
- **SyncControl per Deck** — Phase-locked loop (PLL) aligns deck beatgrid to leader; tempo + phase correction; quantized/loose modes.
- **Beat Distance Control** — `beat_distance` control reports phase offset to leader; used for sync LED feedback.

### 4.4 FX Station — Allocation-Free DSP (`fxstationscenes.h`)
- **No Heap Allocations on Audio Thread** — All delay lines, buffers, state pre-allocated in `allocate(maxSampleRate)`; `process()` uses only stack + member arrays.
- **Hermite Interpolation** — 4-point Hermite for fractional delay reads (`HermiteLine`); flat to 0.4× sample rate; no zipper noise.
- **WSOLA Pitch Shift** — Overlap-add with phase-matched jumps (`WsolaPitch`); ~20 ms latency; searches ±6 ms for best waveform correlation.
- **2× Oversampling for Drive** — Kaiser-windowed half-band FIR (31 taps, ~90 dB stopband); antiderivative anti-aliasing on tanh; even-harmonic bias.
- **Beat-Synced LFOs** — All Scene/Isolate/Release modulation derived from `FxStation` clock (beat phase); zero drift.

### 4.5 Waveform Rendering (`src/waveform/renderers/`, `VisualsManager`, `VSyncThread`)
- **Scene Graph Integration** — `QSGNode` subclasses (`WaveformWidgetRenderer`) render directly on Qt Quick scene graph thread; no CPU→GPU copy.
- **Multi-Resolution Overviews** — `AnalyzerWaveform` generates downsampled peaks (1/256, 1/64, 1/16, 1/4); `WaveformOverviewRenderer` picks level by zoom.
- **VSync Thread** — Dedicated `VSyncThread` drives 60 fps waveform updates; decoupled from audio thread; `VisualPlayPosition` interpolates playhead.
- **Stem Waveforms** — Separate renderers per stem; `WaveformStack.qml` composites; double-click toggles split view.

### 4.6 Library / Database (`TrackCollection`, `BaseSqlTableModel`, `ColumnCache`)
- **Async SQL** — `BaseSqlTableModel` runs queries on background thread; emits `dataChanged` on UI thread via queued connections.
- **Column Cache** — `ColumnCache` memoizes expensive metadata (key, BPM, duration) per track ID; invalidated on re-analysis.
- **Proxy Models** — `ProxyTrackModel` filters/sorts without copying; used for crate/playlist/search views.
- **External Library Import** — `BaseExternalLibraryFeature` + adapters (iTunes, Traktor, Serato, Rekordbox, Engine DJ) map foreign DBs to Mixxx schema.

### 4.7 Streaming Architecture (`AudioSource`, `SoundSourceProxy`, `YouTubeService`)
- **AudioSource Abstraction** — `AudioSource` + `SoundSourceProxy` wrap any decoder (FFmpeg, FLAC, MP3, MediaFoundation, CoreAudio, yt-dlp).
- **YouTube Streaming** — `YouTubeService` uses yt-dlp (resume support); `SponsorBlockController` skips segments; `YouTubeAudioCutter` trims silence.
- **Seekable Proxy** — `SoundSourceProxy` presents streaming source as seekable; caches chunks in `ReadaheadFrameBuffer`.

---

## 5. Key Files Reference Map

| Area | Files |
|------|-------|
| **Engine Core** | `src/engine/enginebuffer.cpp/h`, `src/engine/enginemixer.cpp/h`, `src/engine/engineworkerscheduler.cpp/h` |
| **FX Station** | `src/engine/fxstation/fxstation.cpp/h`, `fxstationdsp.h`, `fxstationinstruments.cpp/h`, `fxstationscenes.h` |
| **Effects (Built-in)** | `src/effects/backends/builtin/*.cpp/h`, `src/effects/backends/builtinbackend.cpp` |
| **Decks/Channels** | `src/engine/channels/enginedeck.cpp/h`, `enginechannel.cpp/h` |
| **Sync** | `src/engine/sync/enginesync.cpp/h`, `synccontrol.cpp/h`, `internalclock.cpp/h` |
| **Waveforms** | `src/waveform/waveform.cpp/h`, `waveformwidgetrenderer.cpp/h`, `renderers/*.cpp/h` |
| **Library** | `src/library/library.cpp/h`, `trackcollection.cpp/h`, `trackcollectionmanager.cpp/h`, `searchqueryparser.cpp/h` |
| **Analysis** | `src/analyzer/analyzerthread.cpp/h`, `analyzerbeats.cpp/h`, `analyzerkey.cpp/h`, `analyzerebur128.cpp/h`, `analyzerenergy.cpp/h` |
| **LateNightQML Skin** | `res/skins/LateNightQML/MainWindow.qml`, `Deck/*.qml`, `Waveforms/*.qml`, `LateNightTheme/*.qml` |
| **LateNight (Classic)** | `res/skins/LateNight/skin.xml`, `decks/*.xml`, `waveform.xml`, `style*.qss` |
| **Controllers** | `src/controllers/*.cpp/h`, `midi/`, `hid/`, `bulk/`, `scripting/` |
| **Recording/Broadcast** | `src/recording/recordingmanager.cpp/h`, `src/broadcast/broadcastmanager.cpp/h` |
| **Vinyl Control** | `src/vinylcontrol/*.cpp/h` |
| **Samples** | `src/library/samples/*.cpp/h`, `res/samples/djsugar/*.wav/*.ogg` |
| **Streaming** | `src/sources/youtube/*.cpp/h`, `src/library/youtube/*.cpp/h` |
| **AutoDJ** | `src/library/autodj/*.cpp/h`, `smart/*.cpp/h` |

---

## 6. Summary for djsugar-next (Rust) Reimplementation

**Must-have architecture:**
1. **Single real-time audio thread** with lock-free control plane (atomics, no mutexes on hot path)
2. **Pre-allocated buffer pools** (engine block size, delay lines, read-ahead, waveform peaks)
3. **FX Station as first-class engine module** — allocation-free, beat-synced, 4-section remix effector
4. **Waveform rendering on GPU/scene graph** — multi-resolution peaks, stem-ready, VSync-driven
5. **Library as async SQL + column cache** — proxy models for filtering, external importers
6. **Streaming via unified AudioSource trait** — yt-dlp + SponsorBlock + seekable proxy
7. **LateNightQML layout patterns** — animated deck states, collapsible drawers, maximizable library, focusable decks
8. **Search syntax parity** — field filters, negation, fuzzy, OR, quotes, ranges

**DJ Sugar differentiators to carry forward:**
- FX Station (Scene/Isolate/X-Pad/Release) with 7 custom Scene effects
- 64-slot sampler with X-Pad overdub loop
- Bundled sample pack (24 sounds, normalized)
- YouTube/SoundCloud streaming + SponsorBlock
- WebP cover art
- Greeklish search
- Smart AutoDJ (phrase/energy/key/bridge)
- Back Spin effect (needle physics)
- Modern LateNightQML skin with spring animations

**Avoid / Improve:**
- Classic/LateNight skin split → single QML skin with theme variants
- Stem separation → real-time (rvl/spleeter integration)
- Cloud sync for cues/loops/playlists
- Controller mapping → visual node-based editor
- Library scan → incremental, non-blocking, progress UI
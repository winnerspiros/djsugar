# DJ Sugar (Mixxx) vs djsugar-next (Rust) — Feature Parity: EFFECTS & UI FUNCTIONS

**Legend:** `DONE` = feature exists with same behaviour, `PARTIAL` = exists but behavioural difference noted, `MISSING` = not implemented.

---

## 1. EFFECT UNITS & BUILT-IN EFFECT ALGORITHMS

| Effect (DJ Sugar) | Rust (station.rs) | Status | Behavioural Difference |
|---|---|---|---|
| **EQs (Mixing)** | | | |
| Bessel4LVMixEQEffect | — | MISSING | No LV2-style 4-band Bessel EQ |
| Bessel8LVMixEQEffect | — | MISSING | No 8-band Bessel EQ |
| LinkwitzRiley8EQEffect | — | MISSING | No 8-band Linkwitz-Riley EQ |
| ThreeBandBiquadEQEffect | — | MISSING | No 3-band biquad EQ |
| BiquadFullKillEQEffect | — | MISSING | No full-kill biquad EQ |
| **EQs (Compensations)** | | | |
| GraphicEQEffect | — | MISSING | No graphic EQ (31-band) |
| ParametricEQEffect | — | MISSING | No parametric EQ |
| LoudnessContourEffect | — | MISSING | No loudness contour (Fletcher-Munson) |
| **Fading / Utility** | | | |
| FilterEffect | Scene::Filter | PARTIAL | Rust has single stereo filter; DJ Sugar has LPF/HPF/BPF per-parameter |
| MoogLadder4FilterEffect | — | MISSING | No Moog 4-pole ladder filter |
| BitCrusherEffect | Scene::CrushEcho (partial) | PARTIAL | Rust bit-crush only inside CrushEcho scene |
| WhiteNoiseEffect | Scene::Noise | PARTIAL | Rust noise only inside Noise scene |
| BalanceEffect | — | MISSING | No stereo balance |
| GainEffect | — | MISSING | No simple gain effect |
| AutoGainControlEffect | — | MISSING | No auto-gain |
| **Fancy / Creative** | | | |
| FlangerEffect | Scene::Flanger | PARTIAL | Rust flanger is a scene; DJ Sugar is per-slot effect |
| EchoEffect | Scene::Echo / HpfEcho / LpfEcho | PARTIAL | Rust splits echo into 3 scene variants |
| DJEchoEffect | — | MISSING | No dedicated DJ echo (ping-pong) |
| ProCompressorEffect | — | MISSING | No pro compressor |
| ProFlangerEffect | — | MISSING | No pro flanger |
| VintageDelayEffect | — | MISSING | No vintage delay |
| AutoPanEffect | — | MISSING | No auto-pan |
| ProReverbEffect | Scene::ReverbUp / ReverbDown | PARTIAL | Rust reverb only as up/down scenes |
| ProFilterEffect | — | MISSING | No pro filter |
| ProNoiseEffect | — | MISSING | No pro noise |
| BeatRepeatEffect | Scene::Roll | PARTIAL | Rust roll is a scene; DJ Sugar beat-repeat is per-slot |
| TapeStopEffect | Release::Brake | PARTIAL | Rust brake is a release effect, not per-slot |
| BackSpinEffect | Release::BackSpin | PARTIAL | Rust backspin is a release effect |
| ReverbEffect | — | MISSING | (Mac App Store excluded in DJ Sugar) |
| PhaserEffect | Scene::Phaser | PARTIAL | Rust phaser is a scene |
| MetronomeEffect | — | MISSING | No metronome |
| KeyComparisonEffect | — | MISSING | No key comparison |
| SpiralEffect | Scene::SpiralUp / SpiralDown | PARTIAL | Rust splits into up/down scenes |
| TremoloEffect | — | MISSING | No tremolo |
| PitchShiftEffect | Scene::Pitch | PARTIAL | Rust pitch is a scene |
| DistortionEffect | Scene::Drive | PARTIAL | Rust drive is a scene |
| GlitchEffect | Scene::Gater | PARTIAL | Rust gater is a scene |
| CompressorEffect | — | MISSING | No basic compressor |

**Summary:** DJ Sugar has **38 built-in effects** (including Mac-excluded Reverb).  
djsugar-next exposes **16 FX Station scenes** + 3 release types. No per-deck effect rack, no EQ effects, no utility effects.

---

## 2. QUICK EFFECTS (per-deck super-knob)

| DJ Sugar (QuickEffectRack) | djsugar-next | Status | Difference |
|---|---|---|---|
| Filter (LPF↔HPF) on `super1` | `Station.iso` (3-band isolator) + `Scene::Filter` | PARTIAL | DJ Sugar: single filter knob per deck. Rust: 3-band isolator on master + filter scene |
| Quick effect selectable per deck (via prefs) | — | MISSING | No per-deck quick effect selection |
| Routing: effect unit → deck (group_[ChannelN]_enable) | — | MISSING | No per-deck FX unit routing |

---

## 3. FX STATION (Master Remix Effector)

| Feature | DJ Sugar (FxStation.qml + C++ engine) | djsugar-next (station.rs) | Status | Difference |
|---|---|---|---|---|
| **Scenes** | 17: BPF ECHO, ECHO, NOISE, SPIRAL UP, REVERB UP, HPF ECHO, LPF ECHO, CRUSH ECHO, SPIRAL DOWN, REVERB DOWN, FLANGER, PHASER, FILTER, ROLL, GATER, PITCH, DRIVE | 16: Echo, HpfEcho, LpfEcho, CrushEcho, SpiralUp, SpiralDown, Noise, ReverbUp, ReverbDown, Flanger, Phaser, Filter, Roll, Gater, Pitch, Drive | PARTIAL | DJ Sugar has BPF ECHO as separate; Rust merges into Echo/HpfEcho/LpfEcho |
| **Release Effects** | 3: VINYL BRAKE, ECHO, BACK SPIN | 3: Brake, Echo, BackSpin | DONE | Exact match |
| **Isolator (3-band)** | Yes (Low/Mid/High kills) | Yes (`iso: [f32;3]`) | DONE | |
| **X/Y Pad** | Yes (scene param 1 & 2) | Yes (`dial`, `p1`, `p2`) | DONE | |
| **Tempo Sync** | Yes (beat-synced delay times) | Yes (`bpm`, `beat_frames()`) | DONE | |
| **Button Remapping** | User can swap scene/release buttons | — | MISSING | No button remapping UI |
| **Compact/Strip/Full Layout** | 3 layouts (phone tabs, strip, full) | — | MISSING | Rust draws station only in View::Fx |
| **Scene Params per Scene** | Each scene has custom param mapping | Fixed: `p1`=time/sweep, `p2`=depth/feedback | PARTIAL | Less flexible param mapping |

---

## 4. X/Y PAD (Touch Pad)

| Feature | DJ Sugar (XYPad.qml) | djsugar-next | Status | Difference |
|---|---|---|---|---|
| Filter X-axis (LPF↔HPF) | Yes (`currentX`) | `Scene::Filter` uses `dial` | PARTIAL | Rust filter scene uses single dial |
| Wet/Dry Y-axis | Yes (`currentY`) | `Station.mix` | PARTIAL | |
| Hold/Snap Mode | Yes (`holdMode`, `savedState`) | — | MISSING | No hold/snap restore |
| Deck Routing Save/Restore | Yes (saves routing, mix, filter) | — | MISSING | |
| Multi-deck Target | Yes (switchable targetDeck) | — | MISSING | Station is master-only |
| Closable Overlay | Yes | — | MISSING | Rust station is a view, not overlay |

---

## 5. BEAT FX (Controller-driven)

| Feature | DJ Sugar | djsugar-next | Status |
|---|---|---|---|
| Beat FX Catalog (EffectsManager.beatFxCatalog) | Yes (maps to effect rack unit 1) | — | MISSING |
| Pad FX Catalog (EffectsManager.padFxCatalog) | Yes | — | MISSING |
| BEAT FX SELECT (controller pads trigger scenes) | Yes (BoothBeatFx.qml) | — | MISSING |
| Focused Effect Slot (`focused_effect`) | Yes | — | MISSING |

---

## 6. MENUS (MainMenuBar + ApplicationMenuActions)

| Menu / Action | DJ Sugar | djsugar-next | Status |
|---|---|---|---|
| **File** | Load Deck 1-4, Quit | — | MISSING |
| **Library** | Rescan, Export, Search Current/All, New Playlist/Crate | — | MISSING |
| **View** | Show Microphone, Vinyl Control, Preview Deck, Cover Art, Keywheel, Maximize Library, Auto DJ, Full Screen | View::Dj/Wave/Lib/Fx/Stream/Ctl via toolbar | PARTIAL | Rust has view switcher, no menu bar |
| **Options** | Vinyl Control enable 1-4, Keyboard Shortcuts, Live Broadcasting, Preferences, Record Mix | — | MISSING |
| **Help** | About, Community Support, Keyboard Shortcuts, Settings Dir, Translate, User Manual | — | MISSING |
| **Developer** (debug mode) | Reload Skin, Tools, Debugger, Stats | — | MISSING |

---

## 7. SETTINGS / PREFERENCES PAGES

| Page | DJ Sugar (DlgPref*.h + Settings/*.qml) | djsugar-next | Status | Key Options Missing in Rust |
|---|---|---|---|---|
| **Sound / Audio** | DlgPrefSound: API, sample rate, buffer, latency, channels, calibrate, keylock threading | — | MISSING | All audio hardware config |
| **Vinyl Control** | DlgPrefVinyl: type, speed, pitch estimator, lead-in per deck | — | MISSING | All vinyl control |
| **Beats / Analysis** | DlgPrefBeats: analyzer plugin, fixed tempo, fast analysis, re-analyze, stem strategy | — | MISSING | All beat detection config |
| **Key Detection** | DlgPrefKey: analyzer, notation (Traditional/OpenKey/Lancelot/Custom), re-analyze, stem strategy | — | MISSING | All key detection config |
| **Broadcast** | DlgPrefBroadcast: Icecast profiles, reconnect, metadata, multiple connections | — | MISSING | All live broadcasting |
| **Mixer** | DlgPrefMixer: EQ type per deck, QuickEffect per deck, crossfader curve/mode/reverse/calibration, main EQ, gain/auto-reset | — | MISSING | Crossfader config, per-deck EQ/QuickEffect selection |
| **Effects** | DlgPrefEffects: visible/hidden effects, chain presets import/export/rename/delete | — | MISSING | Effect visibility, chain presets |
| **Record** | DlgPrefRecord: format (WAV/MP3/OGG/FLAC/OPUS), quality, split, cue file, metadata | record.rs: 32-bit float WAV only | PARTIAL | Format options, split, cue, metadata |
| **ReplayGain** | DlgPrefReplayGain: enabled, analyzer, boost, default boost, re-analyze | — | MISSING | All ReplayGain |
| **Library** | DlgPrefLibrary: music folders, row height, font, sync metadata, BPM precision, date format, Serato export, double-click action, cover art quality | library.rs: scan only | PARTIAL | Folders, UI, metadata sync, Serato |
| **Interface** | DlgPrefInterface: tooltips, skin, color scheme, scale factor, screensaver, multi-sampling | — | MISSING | Skinning, tooltips, screensaver |
| **Shortcuts** | (Not found as separate dialog — likely in Interface) | — | MISSING | Keyboard shortcut editor |
| **Colors** | DlgPrefColors: hotcue/track/key palettes, replace cue color | — | MISSING | Color palettes |
| **Waveform** | DlgPrefWaveform: type (Simple/Filtered/HSV/RGB/Stacked), frame rate, acceleration, options (split stereo, high detail), zoom, visual gains, beat grid alpha, play marker, until-mark, hot cue labels, stem opacity/display | sugar.rs: scrolling overview + detail, 3-band color | PARTIAL | Type selection, frame rate, zoom sync, visual gains, cue labels, stem display |
| **Auto DJ** | DlgPrefAutoDJ: requeue ignored, random queue | state.rs: Auto struct (next, fade) | PARTIAL | Queue behaviour options |
| **Stems** | Settings/Stems.qml: model download (spleeter/htdemucs), benchmark, memory warning | — | MISSING | Stem separation entirely |
| **Controllers** | Settings/Controller.qml: USB rescan, mapping editor, device settings | — | MISSING | MIDI mapping, controller management |
| **Pads & FX** | Settings/Pads.qml: sampler banks A/B, pad FX 1/2, beat FX assignments, reset buttons | state.rs: pads + banks + FX_PADS | PARTIAL | Pad FX / Beat FX assignment UI |
| **Analyzer** | Settings/Analyzer.qml: placeholder | — | MISSING | |
| **Interface (QML)** | Settings/Interface.qml: deck layout, waveforms, library, performance pads, skin, language | — | MISSING | |

---

## 8. CONTROLLERS / MIDI MAPPING

| Feature | DJ Sugar | djsugar-next | Status |
|---|---|---|---|
| ControllerManager (device detection, rescan) | Yes | — | MISSING |
| Mapping Editor (Controller.qml) | Yes | — | MISSING |
| MIDI Learn / Mapping Import/Export | Yes | — | MISSING |
| Device-specific Settings Panels | Yes | — | MISSING |
| USB Hotplug Rescan | Yes | — | MISSING |
| HID / MIDI / Serial Support | Yes | — | MISSING |

---

## 9. LYRICS

| Feature | DJ Sugar | djsugar-next (lyrics.rs) | Status | Difference |
|---|---|---|---|---|
| LRC Parsing (`[mm:ss.xx]`) | Yes (via C++) | Yes (`Lyrics::parse`) | DONE | |
| Offset Tag (`[offset:ms]`) | Yes | Yes | DONE | |
| Multi-timestamp Lines | Yes | Yes | DONE | |
| Current Line Highlight | Yes (C++) | `Lyrics::current(seconds)` | DONE | |
| Display in UI | Waveform/Deck overlays | View::Wave + View::Ctl (lyrics panel) | PARTIAL | Different UI integration |

---

## 10. SAMPLES / SAMPLER PADS

| Feature | DJ Sugar | djsugar-next | Status | Difference |
|---|---|---|---|---|
| Sampler Deck (16 slots × 2 banks) | Yes (Sampler.qml, PlayerManager) | `state.rs`: `samples`, `pads[2][32]`, `bank[2]` | DONE | |
| Bundled Samples | `PlayerManager.bundledSamples()` | `SAMPLES` const array (compile-time) | PARTIAL | Rust samples are embedded, not user-loadable |
| Sample Load/Play | Drag-drop, right-click load | Click pad → loads on demand | PARTIAL | No drag-drop, no user sample management |
| Sampler Gain / Pregain | Yes | — | MISSING | |
| Sampler Loop / Sync / Reverse | Yes | — | MISSING | |
| Pad FX Mode (effects on pads) | Settings/Pads.qml | — | MISSING | |
| Bank Switching (SHIFT) | Yes | `bank[deck]` | DONE | |
| Pad Grid UI (2×4) | Settings/Pads.qml PadGrid | `sugar.rs` samplers() row | PARTIAL | Rust shows single row, no grid config UI |

---

## 11. AUTO DJ (Note Only)

| Feature | DJ Sugar | djsugar-next | Status |
|---|---|---|---|
| Auto DJ Queue | Yes (library view) | `state.rs`: `Auto { next, fade }` | PARTIAL |
| Crossfade Time / Curve | Yes | `fade: Option<(deck, from, to)>` | PARTIAL |
| Repeat / Random / Queue Sources | Yes | — | MISSING |
| Trigger on Deck End | Yes | — | MISSING |
| Settings Page | DlgPrefAutoDJ | — | MISSING |

---

## 12. SPLIT VIEW

| Feature | DJ Sugar | djsugar-next | Status |
|---|---|---|---|
| Horizontal Split (waveforms top, decks bottom) | Yes (Deck.qml model) | View::Dj / View::Wave / View::Ctl | PARTIAL |
| Vertical Split (2 decks side-by-side) | Yes (Cardinality East/West) | `classic.rs` stacked/portrait vs landscape | PARTIAL |
| Dynamic Reflow | Yes | `VIEW_FADE` 150ms transition | DONE |
| Collapsible Sections | Yes | `show_fx`, `show_samplers` toggles | DONE |

---

## 13. 4-DECK MODE

| Feature | DJ Sugar | djsugar-next | Status |
|---|---|---|---|
| 4 Deck Players | Yes (`numberOfDecks` up to 4) | `DECKS = 2` hardcoded in engine | MISSING |
| Deck Switching (1-2 / 3-4 layers) | Yes | — | MISSING |
| 4-Deck Skin Layouts | Yes (Booth, LateNight, etc.) | — | MISSING |
| Menu: Load Deck 3/4 | Yes (enabled when ≥3 decks) | — | MISSING |

---

## 14. WAVEFORM TYPES & OPTIONS

| Waveform Type / Option | DJ Sugar (DlgPrefWaveform + WaveformDisplay.qml) | djsugar-next (sugar.rs scrolling) | Status |
|---|---|---|---|
| **Types** | Simple, Filtered (3-band), HSV, RGB, Stacked | Single custom: 3-band colored (low/mid/high) | PARTIAL |
| **Frame Rate** | Configurable (30/60/120) | Fixed (implicit via render loop) | MISSING |
| **GPU Acceleration** | Yes (OpenGL shaders) | CPU raster (sugar_ui) | MISSING |
| **Visual Gains** | All/Low/Mid/High separate | Fixed scaling | MISSING |
| **Zoom** | Default zoom, sync across decks | Implicit via detail frames | PARTIAL |
| **Beat Grid Alpha** | Configurable | Fixed | MISSING |
| **Play Marker Position** | Configurable | Center line fixed | MISSING |
| **End-of-Track Warning** | Configurable time | `WaveformRendererEndOfTrack` (hardcoded color) | MISSING |
| **Preroll Indicator** | Yes | Yes (Preroll renderer) | DONE |
| **Loop/Intro/Outro Ranges** | Yes (MarkRange renderers) | Yes (green loop range in scrolling) | DONE |
| **Hot Cue Markers** | Letters, label placement/style | Colored vertical lines + number | PARTIAL |
| **Until-Mark (Time to Cue)** | Show beats/time, align, text size | — | MISSING |
| **Stem Display** | Opacity, reorder, outline, mode (stacked/split) | — | MISSING |
| **Overview Type** | Separate overview waveform type | Single overview strip (scrolling) | PARTIAL |
| **Overview Stereo/Mono** | Configurable | Mono summary | PARTIAL |
| **Overview Minute Markers** | Yes | — | MISSING |
| **Overview Scaling** | Configurable | Fixed | MISSING |

---

## 15. ADDITIONAL UI FUNCTIONS

| Feature | DJ Sugar | djsugar-next | Status |
|---|---|---|---|
| **Microphone Section** | Dedicated UI, talkover, gain | — | MISSING |
| **Preview Deck** | Separate preview player | — | MISSING |
| **Cover Art** | Library + deck display | — | MISSING |
| **Keywheel Dialog** | F12 / Menu | — | MISSING |
| **Vinyl Control UI** | Dedicated section | — | MISSING |
| **Maximize Library** | Space / Menu | View::Lib | PARTIAL |
| **Full Screen** | F11 / Menu | — | MISSING |
| **Keyboard Shortcuts Editor** | Dialog | — | MISSING |
| **Skinning System** | QML skins (LateNight, Booth, Shade, Deere, Tango) | Hardcoded colors in sugar.rs | MISSING |
| **Color Schemes** | Per-skin schemes | — | MISSING |
| **Language / Locale** | Preferences | — | MISSING |
| **Track Context Menu** | Right-click deck/library | `track_menu` in state.rs | PARTIAL |
| **Cue Menu** | Hotcue color picker, clear | `cue_menu` in sugar.rs | DONE |
| **Grid Menu** | Beatgrid edit (half, here, earlier, later, halve, double, match, undo, lock) | `grid_menu` in sugar.rs | DONE |
| **Library Search** | Toolbar + shortcuts | Toolbar search + `hub` (YouTube) | PARTIAL |
| **Streaming / Online Sources** | SoundCloud, YouTube, Spotify, Subsonic, Custom URL | YouTube only (`hub.rs`) | PARTIAL |
| **Recording** | Multi-format, split, cue, metadata | 32-bit float WAV only | PARTIAL |
| **History / Playlists** | Full library | `playlists`, `history` in state.rs | PARTIAL |
| **Crate / Playlist Management** | Create, rename, delete, drag-drop | Load from store only | MISSING |

---

## RANKED TOP 20 GAPS (by impact on DJ workflow)

1. **No per-deck effect rack** — DJ Sugar has 2×3-slot effect units per deck; Rust has only master FX Station.
2. **No 4-deck support** — Engine hardcoded to 2 decks.
3. **No MIDI/controller mapping system** — Cannot use hardware controllers.
4. **Missing 22/38 built-in effects** — No EQ effects, no utility effects, no pro effects.
5. **No audio hardware configuration** — Sample rate, buffer, latency, device routing.
6. **No vinyl control** — Timecode vinyl/CDJ support entirely missing.
7. **No broadcast/streaming** — Icecast live streaming absent.
8. **No stem separation** — Neural stem splitting (spleeter/htdemucs) missing.
9. **No ReplayGain** — Loudness normalization missing.
10. **No skinning / theming system** — Hardcoded colors only.
11. **No waveform type selection** — Single fixed 3-band waveform.
12. **No GPU waveform rendering** — CPU-only rasterization.
13. **No microphone / preview deck / cover art** — Core DJ UI sections missing.
14. **No keyboard shortcut editor** — Fixed keymap only.
15. **No user sample management** — Samples compile-time only, no drag-drop load.
16. **No pad FX / beat FX assignment UI** — Controller pad modes missing.
17. **No Auto DJ queue management** — Only basic crossfade struct.
18. **No library folder management / metadata sync** — Scan-only library.
19. **No cue/loop/intro/outro range visual config** — Fixed colors/alphas.
20. **No online sources beyond YouTube** — SoundCloud, Spotify, Subsonic missing.

---

*Generated by code-level comparison of `/home/ubuntu/djsugar` (Mixxx DJ Sugar fork) vs `/home/ubuntu/djsugar-next` (Rust rewrite) on 2026-11-23.*
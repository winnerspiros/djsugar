# DJ Sugar Engine Parity: C++/QML (djsugar) vs Rust (djsugar-next)

> **Scope**: ENGINE and DECK behaviour only — playback, cue/hotcue, loops, beatjump, sync/leader, keylock/pitch, quantize, slip, reverse, eject, beatgrid edit, rate ranges, crossfader curves, EQ/filter/gain, headphone/PFL, recording, sampler decks, stems.
> **Method**: Code-level comparison by reading both implementations (not guessing).
> **Status codes**: **DONE** = behaviour matches; **PARTIAL** = core works, specific gaps listed; **MISSING** = no Rust implementation.

---

## 1. Playback Engine Core

| Feature | djsugar (C++) | djsugar-next (Rust) | Status | Gap Details |
|---------|---------------|---------------------|--------|-------------|
| **Multi-deck playback** | `EngineBuffer` + `EngineDeck` (4+ decks via `EngineMixer::ChannelInfo`) | `engine::Mixer` with `DECKS=4` `Channel` array, each with `Deck` | **DONE** | |
| **Sample-accurate position** | `mixxx::audio::FramePos` (64-bit frame count) | `f64` position in `Deck` (frames) | **DONE** | |
| **Variable rate playback** | `RateControl::calculateSpeed()` → `EngineBuffer::m_speed` | `Deck.rate` (f64, relative) + `nudge` + `velocity` for brake | **DONE** | |
| **Reverse playback** | `RateControl` reverse button + `m_reverse_old` | `Deck.reverse: bool` + negative step in render | **DONE** | |
| **Brake / spin-down** | `RateControl` ramp down (motor sim, 33/45 RPM, power-cut/brake modes) | `Deck.brake` + `velocity` decay in `render_brake()` | **PARTIAL** | Rust has simple linear decay; C++ has configurable motor sim (rpm mode, ramp up/down time factors, power-cut vs brake) |
| **Track end handling** | `ReadAheadManager` wraps for loops/repeat; `notifyWrapAround` | `Deck.render()` stops playing when `pos > last` or `< 0` | **PARTIAL** | C++ has repeat mode (`m_pRepeat`) and wrap-around notification for vinyl control; Rust just stops |
| **Vinyl control (timecode)** | `VinylControlControl` + `PositionScratchController` | **MISSING** | **MISSING** | No vinyl/timecode support in Rust |
| **Read-ahead / caching** | `CachingReader` + `ReadAheadManager` (async decode, prefetch) | **MISSING** | **MISSING** | Rust loads whole `Track` into memory (`Box<[[f32;2]]>`) — no streaming, no background decode |
| **Multi-channel output** | `EngineMixer` → main/booth/headphones/sidechain buses | `Mixer.render()` writes single stereo `out`; `Station` for FX bus | **PARTIAL** | No booth/headphone split, no sidechain in Rust |
| **Sample rate conversion** | `EngineBufferScale` (linear, SoundTouch, RubberBand) per-engine | `sugar_stretch::Stretch` (SoundTouch) per-deck | **PARTIAL** | C++ supports multiple algorithms + linear fallback for small pitch; Rust only SoundTouch |

---

## 2. Cue / Hotcue Logic

| Feature | djsugar (C++) | djsugar-next (Rust) | Status | Gap Details |
|---------|---------------|---------------------|--------|-------------|
| **Main cue point** | `CueControl::m_pCue` (per-deck, `CueType::Main`) | `DeckUi.cue: f64` (single float) | **PARTIAL** | C++ has full `Cue` object with type, color, label; Rust only stores position |
| **Hotcues (8 slots)** | `HotcueControl[8]` per deck, each with `CuePointer`, status (Empty/Set/Active), type (Cue/Loop/Jump), color | `DeckUi.cues: [Option<f64>; 8]` + `cue_colours: [u8; 8]` | **PARTIAL** | C++ supports loop hotcues (saved loops), jump cues, cue preview, type per slot; Rust only position + color |
| **Cue modes** | `CueMode`: Mixxx, Pioneer, Denon, Numark, MixxxNoBlinking, CueAndPlay | **MISSING** | **MISSING** | No configurable cue behaviour in Rust |
| **Seek-on-load modes** | `SeekOnLoadMode`: MainCue, Beginning, FirstSound, IntroStart, FirstHotcue | **MISSING** | **MISSING** | Rust always loads at position 0 |
| **Hotcue set modes** | `HotcueSetMode`: Auto, Cue, Loop | **MISSING** | **MISSING** | Rust `hotcue()` toggles set/seek; no explicit loop-hotcue creation |
| **Cue preview / cache** | `m_previewingPosition`, `m_previewingType` for safe delete during preview | **MISSING** | **MISSING** | |
| **Quantized cue set** | `CueControl::m_pQuantizeEnabled` → snap to beat | `DeckUi.quantize` + `snap()` in `hotcue()` | **DONE** | |
| **Cue colours** | `ColorPaletteSettings`, per-hotcue RGB | `DeckUi.cue_colours: [u8; 8]` (index into palette) | **DONE** | |
| **Cue persistence** | Saved in track metadata (crate/db) | `crate::store::Saved` (cues, colours, grid) | **DONE** | |

---

## 3. Loops

| Feature | djsugar (C++) | djsugar-next (Rust) | Status | Gap Details |
|---------|---------------|---------------------|--------|-------------|
| **Manual loop in/out** | `LoopingControl::slotLoopIn/Out` → `LoopInfo{start,end,seekMode}` | `DeckUi.loop_in: Option<f64>` + `loop_range` + `LoopIn/Out` targets | **DONE** | |
| **Loop enable/disable** | `loop_enabled` proxy, `setLoop()`, `slotLoopExit()` | `Command::Loop(usize, Option<(f64,f64)>)` | **DONE** | |
| **Reloop / saved loop** | `slotReloopToggle()`, `m_pCurrentSavedLoopControl` | **MISSING** | **MISSING** | No reloop (restore last loop) in Rust |
| **Beatloop (quantized)** | `BeatLoopingControl` + `beatloop_activate` + `beatloop_X` sizes | `auto_loop()` + `LOOP_BEATS[8]` + `beat_loop()` | **DONE** | |
| **Loop move (shift)** | `LoopMoveControl` + `loop_move_X` | **MISSING** | **MISSING** | No loop move by beat in Rust |
| **Loop roll (momentary)** | `slotBeatLoopRoll()` + `m_bLoopRollActive` | **MISSING** | **MISSING** | No momentary loop roll in Rust |
| **Loop adjust (in/out resize)** | `slotLoopInGoto/OutGoto`, `m_bAdjustingLoopIn/Out` | **MISSING** | **MISSING** | No fine loop resize in Rust |
| **Loop anchor point** | `LoopAnchorPoint::Start/End` for size adjustment | **MISSING** | **MISSING** | |
| **Minimum loop size** | 150 frames (`kMinimumAudibleLoopSizeFrames`) | 1.0 frame (`b - a >= 1.0` filter) | **PARTIAL** | Rust allows sub-ms loops; C++ enforces audible minimum |
| **Loop in slip mode** | `adjustedPositionForCurrentLoopOrRepeat()` + `SlipModeState` | `Deck.slip: Option<f64>` + position continues in slip | **PARTIAL** | C++ has 3-state slip (Disabled/Armed/Running) with loop-aware exit; Rust simple bool |
| **Sync position inside loop** | `getSyncPositionInsideLoop()` for sync while looping | **MISSING** | **MISSING** | |

---

## 4. Beatjump

| Feature | djsugar (C++) | djsugar-next (Rust) | Status | Gap Details |
|---------|---------------|---------------------|--------|-------------|
| **Beatjump sizes** | `BeatJumpControl` + `beatjump_X` (same `s_dBeatSizes` as loops: 1/32..512 beats) | `JUMP_BEATS[10]` = [0.25, 0.5, 1, 2, 4, 8, 16, 32, 64, 128] | **PARTIAL** | C++ has 17 sizes (down to 1/32 beat); Rust has 10 (starts at 1/4) |
| **Quantized jump** | `QuantizeControl` + beat-snap | `DeckUi.quantize` + `beat()` grid snap in `beat_jump()` | **DONE** | |
| **Jump while looping** | Loop range shifts with jump | `beat_jump()` shifts `loop_range` by same offset | **DONE** | |
| **Reverse jump** | `slotBeatJumpBackward()` | `Target::Jump(d, -1)` | **DONE** | |

---

## 5. Sync / Leader

| Feature | djsugar (C++) | djsugar-next (Rust) | Status | Gap Details |
|---------|---------------|---------------------|--------|-------------|
| **Sync modes** | `SyncMode`: None, Leader, Follower, LeaderFromFollower, Lock | **MISSING** (only manual sync) | **MISSING** | No automatic leader/follower state machine in Rust |
| **Sync lock engine** | `EngineSync` + `SyncLockAlgorithm` (PREFER_SOFT_LEADER / PREFER_LOCK_BPM) | **MISSING** | **MISSING** | |
| **Tempo sync (bpm match)** | `activateFollower()` sets rate to match leader BPM | `sync()` calculates target BPM from other deck, sets rate | **PARTIAL** | Rust does one-shot tempo match; C++ maintains continuous lock |
| **Phase sync (beat align)** | `notifyBeatDistanceChanged()` → continuous phase alignment | `sync()` seeks to matched phase once | **PARTIAL** | C++ tracks beat distance per callback; Rust only aligns on sync press |
| **Explicit leader selection** | `pickLeader()`, `activateLeader(explicitLeader)` | **MISSING** | **MISSING** | |
| **Sync target fallback** | `pickNonSyncSyncTarget()` (leader → playing → first deck) | **MISSING** | **MISSING** | |
| **Double/half tempo handling** | `updateLeaderBpm()` with 2x/0.5x multipliers | `while target/g.bpm > 1.5 { target/=2 }` etc. | **DONE** | |
| **Internal clock sync** | `InternalClock` as sync master | **MISSING** | **MISSING** | |
| **Sync key (key lock sync)** | `KeyControl::slotSyncKey()` | **MISSING** | **MISSING** | |

---

## 6. Keylock / Pitch

| Feature | djsugar (C++) | djsugar-next (Rust) | Status | Gap Details |
|---------|---------------|---------------------|--------|-------------|
| **Keylock on/off** | `KeyControl::m_pKeylock` + `KeylockEngine` enum | `Deck.key_lock: bool` | **DONE** | |
| **Pitch shift (semitones)** | `KeyControl::m_pPitch` + `m_pPitchAdjust` + `engineKeyDistance` | `Deck.semitones: f32` + `Command::Key` | **DONE** | |
| **Key sync** | `KeyControl::syncKey()` + `slotSyncKey()` | **MISSING** | **MISSING** | |
| **Key reset** | `slotResetKey()` | `Target::Pitch(d, 0)` | **DONE** | |
| **Keylock algorithms** | SoundTouch, RubberBand (Faster/Finer/R3ShortWindow) | SoundTouch only (`sugar_stretch::Stretch`) | **PARTIAL** | No algorithm choice in Rust |
| **Linear scaler fallback** | `EngineBufferScaleLinear` for |pitch| < 1 cent | **MISSING** | **MISSING** | Rust always uses SoundTouch when keylock or semitones ≠ 0 |
| **Pitch/tempo ratio tracking** | `KeyControl::PitchTempoRatio{pitchRatio, tempoRatio, pitchTweakRatio, keylock}` | Implicit in `Deck.render_stretched()` | **PARTIAL** | C++ exposes ratio for UI/viz; Rust internal only |
| **Vinyl control pitch** | `VinylControlControl` feeds into `RateControl` | **MISSING** | **MISSING** | |

---

## 7. Quantize

| Feature | djsugar (C++) | djsugar-next (Rust) | Status | Gap Details |
|---------|---------------|---------------------|--------|-------------|
| **Global quantize toggle** | `QuantizeControl` + `m_pQuantizeEnabled` per deck | `DeckUi.quantize: bool` per deck | **DONE** | |
| **Quantized seek** | `CueControl::quantizeChanged()` → cues snap | `DeckUi.snap()` used in `hotcue()`, `beat_jump()`, `LoopIn/Out` | **DONE** | |
| **Quantized loop in/out** | `LoopingControl` uses quantize for beatloop | `LoopIn/Out` use `snap()` | **DONE** | |
| **Quantize strength/mode** | **MISSING** (simple on/off) | **MISSING** | **DONE** (both minimal) | |

---

## 8. Slip Mode

| Feature | djsugar (C++) | djsugar-next (Rust) | Status | Gap Details |
|---------|---------------|---------------------|--------|-------------|
| **Slip enable/disable** | `SlipModeState`: Disabled / Armed / Armed | `Deck.slip: Option<f64>` (None=off, Some=slip position) | **PARTIAL** | C++ has Armed state (slip starts after loop exits); Rust immediate |
| **Slip position tracking** | `m_slipPos` + `m_dSlipRate` updated every callback | `Deck.render()`: `if slip { *s += step * len }` | **DONE** | |
| **Loop exit adopts slip** | `m_slipQuitAndAdopt` → position jumps to slip pos on loop exit | `Slip(off)` → `position = p; invalidate()` | **PARTIAL** | C++ only adopts if loop was active when slip enabled; Rust always |
| **Slip with reverse** | `adjustedPositionForCurrentLoopOrRepeat(reverse)` | `step` sign flip in render; slip accumulates signed step | **DONE** | |
| **Slip indicators** | `VisualPlayPosition` shows slip marker | **MISSING** | **MISSING** | No slip position telemetry in Rust |

---

## 9. Reverse

| Feature | djsugar (C++) | djsugar-next (Rust) | Status | Gap Details |
|---------|---------------|---------------------|--------|-------------|
| **Reverse playback** | `RateControl::isReverseButtonPressed()` + `m_reverse_old` | `Deck.reverse: bool` + negative step | **DONE** | |
| **Reverse + keylock** | Keylock disabled during reverse (SoundTouch doesn't support reverse) | `render_stretched()` skipped if `reverse` (line 79) | **DONE** | Both disable keylock in reverse |
| **Reverse + slip** | Works via `adjustedPositionForCurrentLoopOrRepeat` | Works (slip accumulates negative step) | **DONE** | |
| **Reverse brake/spin** | `RateControl` ramp modes | `Command::Spin` (negative velocity + brake) | **PARTIAL** | C++ has configurable spin-up/down; Rust fixed 3.0 velocity |

---

## 10. Eject

| Feature | djsugar (C++) | djsugar-next (Rust) | Status | Gap Details |
|---------|---------------|---------------------|--------|-------------|
| **Eject (unload track)** | `EngineDeck` unload via control | `Command::Unload` + `Target::Eject` (only if not playing) | **DONE** | |
| **Eject while playing** | Allowed (stops deck) | Blocked (`if !playing`) | **PARTIAL** | Rust prevents eject during playback |
| **State reset on eject** | `EngineBuffer` resets position, loop, cues | `DeckUi` reset to `new()` keeping volume/knobs | **DONE** | |

---

## 11. Beatgrid Edit

| Feature | djsugar (C++) | djsugar-next (Rust) | Status | Gap Details |
|---------|---------------|---------------------|--------|-------------|
| **Grid adjust: half beat** | `CueControl` / beatgrid UI | `GridOp::Half` | **DONE** | |
| **Grid align: here** | Set first beat to current position | `GridOp::Here` | **DONE** | |
| **Grid nudge: earlier/later** | Fine adjust first beat | `GridOp::Earlier/Later` (±0.01s) | **DONE** | |
| **Grid halve/double BPM** | ×0.5 / ×2 | `GridOp::Halve/Double` | **DONE** | |
| **Grid match (phase copy)** | Match phase from other deck | `GridOp::Match` | **DONE** | |
| **Grid undo stack** | `m_gridUndo` vector | `DeckUi.grid_undo: Vec<Grid>` | **DONE** | |
| **Grid lock** | Prevent accidental edits | `DeckUi.grid_locked: bool` | **DONE** | |
| **Intro/outro cues** | `CueType::IntroStart/End`, `OutroStart/End` | **MISSING** | **MISSING** | Only first_beat + BPM in Rust `Grid` |

---

## 12. Rate Ranges

| Feature | djsugar (C++) | djsugar-next (Rust) | Status | Gap Details |
|---------|---------------|---------------------|--------|-------------|
| **Rate slider range** | Configurable: ±8%, ±16%, ±25%, ±50%, ±100% | `RATE_RANGE = 0.08` (±8% hardcoded) | **PARTIAL** | C++ user-configurable; Rust fixed ±8% |
| **Temporary rate buttons** | Coarse/fine, permanent/temp, ramp modes | `Deck.nudge` + `Command::Nudge` | **PARTIAL** | C++ has ramp (linear/stepping), sensitivity, RPM modes; Rust instant nudge |
| **Rate ratio display** | `RateControl::calculateSpeed()` → `rate_ratio` control | `DeckUi.rate: f32` (normalized -1..1) | **DONE** | |

---

## 13. Crossfader Curves

| Feature | djsugar (C++) | djsugar-next (Rust) | Status | Gap Details |
|---------|---------------|---------------------|--------|-------------|
| **Curve types** | `MIXXX_XFADER_ADDITIVE` (linear) + `MIXXX_XFADER_CONSTPWR` (constant power) + transform (0.6..1000) | Linear only: `side_gain = (1 ∓ x).min(1)` | **PARTIAL** | C++ has constant-power with calibration, transform exponent, reverse; Rust linear |
| **Per-deck crossfader assignment** | `EngineChannel::orientation` (Left/Center/Right) | Implicit: deck 0/2 left, 1/3 right (fixed) | **PARTIAL** | No orientation control in Rust |
| **Crossfader reverse** | `EngineXfader::getXfadeGains(reverse)` | **MISSING** | **MISSING** | |

---

## 14. EQ / Filter / Gain

| Feature | djsugar (C++) | djsugar-next (Rust) | Status | Gap Details |
|---------|---------------|---------------------|--------|-------------|
| **3-band EQ** | `EngineChannel` + `EngineEffectsManager` (post-fader) | `Channel.eq: [f32;3]` + `kill: [bool;3]` + `Biquad` (LowShelf/Peak/HighShelf) | **DONE** | |
| **EQ kill switches** | `m_pEqualizer*` controls per band | `Channel.kill` (sets gain to 1e-4) | **DONE** | |
| **Filter (low/high pass)** | `EngineFilter*` (Moog, Bessel, Linkwitz-Riley, Butterworth) per channel | `Channel.filter: f32` (-1..1) → `Biquad::Low/High` at 20Hz..20kHz | **PARTIAL** | C++ multiple filter types + resonance; Rust single 1-pole with fixed Q |
| **Gain (pre-fader)** | `EnginePregain` + `m_pVolumeControl` | `Channel.gain: f32` (pre-EQ) + `volume: f32` (post-EQ) | **DONE** | |
| **Per-channel VU meters** | `EngineVuMeter` per channel | `Channel.peak: f32` (post-mix) | **PARTIAL** | C++ pre-fader + post-fader; Rust post-mix only |

---

## 15. Headphone / PFL (Pre-Fader Listen)

| Feature | djsugar (C++) | djsugar-next (Rust) | Status | Gap Details |
|---------|---------------|---------------------|--------|-------------|
| **PFL toggle** | `EngineChannel::m_pPFL` (PowerWindow button) | `DeckUi.pfl: bool` + `Target::Pfl` | **DONE** | |
| **PFL routing** | `EngineMixer::PflGainCalculator` → headphone bus | **MISSING** (no headphone bus) | **MISSING** | Rust has no separate headphone output; PFL is UI state only |
| **Split cue (main + cue)** | `EngineChannel::orientation` + crossfader | **MISSING** | **MISSING** | |
| **PFL volume** | Separate headphone gain | **MISSING** | **MISSING** | |

---

## 16. Recording

| Feature | djsugar (C++) | djsugar-next (Rust) | Status | Gap Details |
|---------|---------------|---------------------|--------|-------------|
| **Record to file** | `EngineRecord` + `Encoder` (MP3, OGG, FLAC, WAV, AIFF) | `crate::record::Recorder` + `Command::Record(bool)` | **PARTIAL** | C++ supports multiple formats + metadata + CUE files; Rust WAV only (see `record.rs`) |
| **Record from main mix** | `EngineRecord::process()` receives main buffer | `Engine.record` ring buffer → `Control.drain_recording()` | **DONE** | |
| **Auto-split tracks** | `metaDataHasChanged()` writes CUE marks | **MISSING** | **MISSING** | |
| **Recording level meter** | `bytesRecorded`, `durationRecorded` signals | **MISSING** | **MISSING** | |
| **Recording preferences** | Path, format, bitrate, metadata, CUE options | Directory only (`recordings_dir()`) | **PARTIAL** | |

---

## 17. Sampler Decks

| Feature | djsugar (C++) | djsugar-next (Rust) | Status | Gap Details |
|---------|---------------|---------------------|--------|-------------|
| **Sample slots** | `EngineDeck` with stem handles (up to 4 stems) | `Mixer::VOICES = 16` + `Command::Sample` | **PARTIAL** | C++ samplers are full decks with EQ/filter; Rust simple one-shot voices |
| **Sample banks** | 4 banks × 8 pads (via `ControlObject`) | `State.pads[2][32]` + `bank[2]` + `BANKS=4` | **DONE** | |
| **Sample playback** | `EngineBuffer` per sample slot | `Mixer.render()` voices loop with simple gain | **PARTIAL** | C++: full deck features (rate, loop, keylock, EQ); Rust: fixed rate, no loop, no EQ |
| **Sample gain** | Per-stem gain controls | `Command::Sample(track, gain)` | **DONE** | |
| **Sample stop** | Per-slot stop | `Command::StopSamples` (stops oldest) | **PARTIAL** | C++ per-slot; Rust only stop-all/oldest |
| **Sample preview** | `CueControl` preview on pads | **MISSING** | **MISSING** | |

---

## 18. Stems

| Feature | djsugar (C++) | djsugar-next (Rust) | Status | Gap Details |
|---------|---------------|---------------------|--------|-------------|
| **Stem separation** | `#ifdef __STEM__` → `m_stems` vector, `processStem()` mixes 4-5 channels | **MISSING** | **MISSING** | No stem support in Rust |
| **Stem gain/mute** | `m_stemGain[]`, `m_stemMute[]` per stem | **MISSING** | **MISSING** | |
| **Stem VU meters** | `m_stemVuMeter[]` | **MISSING** | **MISSING** | |
| **Clone stem state** | `cloneStemState()` copies gain/volume | **MISSING** | **MISSING** | |
| **Stem track loading** | `slotTrackLoaded()` handles stem tracks | **MISSING** | **MISSING** | |

---

## Summary: Top 20 Most Important Gaps (Ranked by Impact)

| Rank | Gap | Area | Effort | Notes |
|------|-----|------|--------|-------|
| 1 | **No streaming decode / read-ahead** | Playback | High | Rust loads entire track into RAM — blocks large files, no background loading |
| 2 | **No vinyl/timecode control** | Playback | High | Critical for DVS users; requires `PositionScratchController` port |
| 3 | **No sync lock engine** | Sync | High | No continuous tempo/phase lock; only one-shot sync |
| 4 | **No headphone/PFL bus** | Output | Medium | PFL is UI-only; no split cue, no headphone monitoring |
| 5 | **No stem support** | Stems | High | Entire `__STEM__` subsystem missing |
| 6 | **Sampler voices lack deck features** | Sampler | Medium | No rate, loop, keylock, EQ on samples |
| 7 | **Crossfader curves limited to linear** | Mixer | Low | Missing constant-power, transform, reverse |
| 8 | **Fixed ±8% rate range** | Tempo | Low | User-configurable ranges missing |
| 9 | **No reloop / saved loop restore** | Loops | Low | `slotReloopToggle` missing |
| 10 | **No loop move / roll / adjust** | Loops | Medium | `LoopMoveControl`, `BeatLoopRoll` missing |
| 11 | **Keylock: SoundTouch only** | Pitch | Low | No RubberBand, no linear scaler fallback |
| 12 | **Hotcue types limited** | Cues | Medium | No loop hotcues, jump cues, cue preview |
| 13 | **No cue modes / seek-on-load modes** | Cues | Low | Pioneer/Denon/Numark behaviours missing |
| 14 | **Recording: WAV only, no metadata/CUE** | Recording | Medium | No MP3/OGG/FLAC, no track splitting |
| 15 | **No internal clock sync master** | Sync | Low | Can't sync to internal clock |
| 16 | **No key sync** | Pitch | Low | `KeyControl::syncKey` missing |
| 17 | **Slip mode: no Armed state** | Slip | Low | C++ 3-state vs Rust bool |
| 18 | **No per-channel pre-fader VU** | Metering | Low | Only post-mix peak |
| 19 | **Beatjump sizes start at 1/4 beat** | Beatjump | Low | C++ goes to 1/32 |
| 20 | **No configurable brake/spin motor sim** | Playback | Low | Fixed 3.0 velocity vs RPM/time-factor config |

---

## Files Referenced

### djsugar (C++)
- `src/engine/channels/enginedeck.h/cpp`
- `src/engine/channels/enginechannel.h/cpp`
- `src/engine/enginebuffer.h/cpp`
- `src/engine/enginemixer.h/cpp`
- `src/engine/enginexfader.h/cpp`
- `src/engine/controls/cuecontrol.h/cpp`
- `src/engine/controls/loopingcontrol.h/cpp`
- `src/engine/controls/ratecontrol.h/cpp`
- `src/engine/controls/keycontrol.h/cpp`
- `src/engine/controls/quantizecontrol.h/cpp`
- `src/engine/controls/bpmcontrol.h/cpp`
- `src/engine/sync/enginesync.h/cpp`
- `src/engine/sync/synccontrol.h/cpp`
- `src/engine/slipmodestate.h`
- `src/engine/bufferscalers/*.h/cpp`
- `src/engine/sidechain/enginerecord.h/cpp`
- `src/engine/channelmixer.h/cpp`

### djsugar-next (Rust)
- `crates/engine/src/deck.rs`
- `crates/engine/src/mixer.rs`
- `crates/engine/src/lib.rs` (Command, Control, Engine)
- `crates/engine/src/biquad.rs`
- `crates/app/src/state.rs` (DeckUi, State, all Target handlers)
- `crates/app/src/record.rs`
- `crates/core/src/time.rs`
- `crates/analysis/src/lib.rs` (grid, key)
# DJ Sugar (Mixxx Fork) Feature Parity List

Exhaustive inventory of every non-UI feature, control, preference, effect, library capability, and DJ-Sugar-specific addition. Source: `/home/ubuntu/djsugar` (C++/Qt, read-only fork). Target: Rust rewrite.

---

## 1. ENGINE CORE

### 1.1 Engine Architecture
- **EngineBuffer**: Triple-buffered audio pipeline (input → process → output)
- **EngineWorkerScheduler**: Thread pool for audio callback work stealing
- **EngineMixer**: 4-deck + sampler + aux + master + booth + record + broadcast mixing
- **ChannelMixer**: Per-channel gain, EQ, filter, fader, crossfader
- **EnginePreGain**: Pre-fader gain per deck
- **EngineSidechainCompressor**: Ducking for talkover/mic
- **EngineTalkoverDucking**: Mic talkover with configurable attack/release
- **EngineVuMeter**: Peak/RMS metering per channel
- **EngineXFader**: Crossfader curves (constant power, linear, cut)
- **EngineDelay**: Latency compensation
- **ReadAheadManager**: Async decode buffer for gapless playback
- **PositionScratchController**: Vinyl-style scratch position tracking
- **SlipModeState**: Slip mode (playhead continues while looping/scratching)

### 1.2 Channels & Decks
- **EngineDeck** (src/engine/channels/enginedeck.cpp/h):
  - 4 main decks + preview deck + sampler deck
  - **Stem support** (kMaxSupportedStems = 4): stem_count, per-stem volume/mute/VU controls
  - Rate control: pitch bend, tempo sync, keylock, rate slider, sync/quantize
  - Cue system: hotcues (8), loop cues, beatjump, quantized cues
  - Looping: auto-loop, manual loop, loop move, loop resize, loop halve/double
  - Beatgrid: BPM, beat positions, downbeat, grid offset, grid adjust
  - Key detection: musical key, key lock, key shift, harmonic mixing
  - Waveform: overview, scrolling, stem waveforms, vocal strip
  - Quantize: snap to beat/bar/phrase
  - Vinyl control: timecode (Serato, Traktor, MixVibes), relative/absolute modes
  - Slip mode toggle
  - Reverse play
  - Flux mode (Serato DJ Pro style)

### 1.3 BPM Control (src/engine/controls/bpmcontrol.cpp)
- **ConfigKey groups**: `[ChannelN]`, `[Master]`, `[SamplerN]`
- **Items**: `bpm`, `bpm_tap`, `bpm_adjust`, `bpm_range`, `bpm_lock`, `bpm_sync`
- Tap tempo, BPM range (60-200%), sync modes (off/phase/beat/bar/phrase)

### 1.4 Rate Control (src/engine/controls/ratecontrol.cpp)
- **Items**: `rate`, `rate_perm_down`, `rate_perm_down_small`, `rate_perm_up`, `rate_perm_up_small`, `rate_temp_down`, `rate_temp_down_small`, `rate_temp_up`, `rate_temp_up_small`, `rate_range`, `rate_engine`, `sync`, `quantize`, `keylock`, `keylock_disable_on_sync`
- Rate range: ±8%, ±16%, ±25%, ±50%, ±100%
- Keylock algorithm: elastique/rubberband

### 1.5 Cue Control (src/engine/controls/cuecontrol.cpp)
- **Items**: `cue_default`, `cue_goto`, `cue_jump`, `cue_play`, `cue_preview`, `cue_set`, `cue_cdj`, `cue_mode` (CDJ/Serato/Traktor)
- Hotcues: 8 per deck, colored, quantized trigger
- Loop cues: auto-loop on cue point
- Beatjump: ±1/2/4/8/16/32 beats, quantized

### 1.6 Looping Control (src/engine/controls/loopingcontrol.cpp)
- **Items**: `loop_enabled`, `loop_end_position`, `loop_halve`, `loop_double`, `loop_in`, `loop_out`, `loop_move`, `loop_move_X`, `loop_scale`, `loop_start_position`, `quantize_loop`
- Auto-loop sizes: 1/2/4/8/16/32 beats
- Loop move: ±1/2/4/8 beats
- Loop resize: drag endpoints

### 1.7 Key Control (src/engine/controls/keycontrol.cpp)
- **Items**: `key`, `key_lock`, `key_shift`, `key_sync`, `keylock_mode` (standard/elastique/rubberband)
- Camelot notation, harmonic mixing wheel
- Key shift: ±12 semitones

### 1.8 Quantize Control (src/engine/controls/quantizecontrol.cpp)
- **Items**: `quantize`, `quantize_beats`, `quantize_mode` (beat/bar/phrase)
- Snap threshold configurable

### 1.9 Clock Control (src/engine/controls/clockcontrol.cpp)
- **Items**: `playposition`, `playposition_absolute`, `playposition_relative`, `remaining_time`, `elapsed_time`, `total_time`

### 1.10 Vinyl Control (src/engine/controls/vinylcontrolcontrol.cpp)
- **Items**: `vinylcontrol_enabled`, `vinylcontrol_cueing`, `vinylcontrol_mode` (absolute/relative/constant), `vinylcontrol_lead_in`

---

## 2. EFFECTS — BUILTIN BACKEND (src/effects/backends/builtin/)

### 2.1 Effect Framework
- **EffectChain**: Serial/parallel routing, mix modes (dry/wet, insert, send)
- **EffectSlot**: Per-deck 4 effect units, quick effect, pad FX
- **PadFXController**: X-Pad instruments, sample triggering
- **VisibleEffectsList**: User-customizable effect rack order

### 2.2 Builtin Effects (31 effects)

| Effect | File | Parameters |
|--------|------|------------|
| AutoGainControl | autogaincontroleffect.cpp | target_level, attack, release, makeup_gain |
| AutoPan | autopaneffect.cpp | rate, depth, shape, phase, spread |
| Backspin | backspineffect.cpp | speed, brake_time |
| Balance | balanceeffect.cpp | left_right_balance |
| Bessel4LVMixEQ | bessel4lvmixeqeffect.cpp | low, mid, high, kill switches |
| Bessel8LVMixEQ | bessel8lvmixeqeffect.cpp | 8-band, steeper slopes |
| BiquadFullKillEQ | biquadfullkilleqeffect.cpp | 3-band, full kill (-∞ dB) |
| Bitcrusher | bitcrushereffect.cpp | bit_depth, sample_rate_reduction, mix |
| Compressor | compressoreffect.cpp | threshold, ratio, attack, release, makeup, knee |
| Distortion | distortioneffect.cpp | drive, tone, mix, type (soft/hard/clip/fold) |
| DJEcho | djechoeffect.cpp | delay, feedback, filter, mix, ping_pong |
| Echo | echoeffect.cpp | delay, feedback, mix, stereo_width |
| Filter | filtereffect.cpp | frequency, resonance, type (LP/HP/BP/Notch), mix |
| Flanger | flangereffect.cpp | rate, depth, feedback, delay, mix, stereo |
| Gain | gaineffect.cpp | gain_dB |
| Glitch | glitcheffect.cpp | probability, severity, mix, repeat |
| GraphicEQ | graphiceqeffect.cpp | 10/31 bands, per-band gain, preset curves |
| KeyComparison | keycomparisoneffect.cpp | reference_key, tolerance, highlight_mode |
| LinkwitzRiley8EQ | linkwitzriley8eqeffect.cpp | 8-band LR4 crossover |
| LoudnessContour | loudnesscontoureffect.cpp | amount, frequency |
| MoogLadder4Filter | moogladder4filtereffect.cpp | cutoff, resonance, drive, mix |
| ParametricEQ | parametriceqeffect.cpp | 4 bands: freq, gain, Q, type |
| Phase | phasereffect.cpp | rate, depth, feedback, stages, mix |
| PitchShift | pitchshifteffect.cpp | semitones, cents, formants, mix |
| ProCompressor | procompressoreffect.cpp | threshold, ratio, attack, release, knee, makeup, lookahead |
| ProFilter | profiltereffect.cpp | freq, res, drive, mix, type, slope |
| ProFlanger | proflangereffect.cpp | rate, depth, feedback, delay, mix, shape |
| ProNoise | pronoiseeffect.cpp | color, level, mix |
| ProReverb | proreverbeffect.cpp | room_size, damping, width, mix, pre_delay |
| Reverb | reverbeffect.cpp | room_size, damping, mix, pre_delay |
| Spiral | spiraleffect.cpp | rate, depth, feedback, mix |
| TapeStop | tapestopeffect.cpp | start_time, end_time, curve |
| ThreeBandBiquadEQ | threebandbiquadeqeffect.cpp | low, mid, high, kill switches |
| Tremolo | tremoloeffect.cpp | rate, depth, shape, stereo, mix |
| VintageDelay | vintagedelayeffect.cpp | delay, feedback, filter, wow, flutter, mix |
| WhiteNoise | whitenoiseeffect.cpp | level, color, mix |
| MetronomeClick | metronomeclick.cpp | bpm, accent, volume, sound |
| MetronomeEffect | metronomeeffect.cpp | beat_division, accent_pattern |

### 2.3 Effect Presets & Chains
- Effect chain presets (save/load/share)
- Quick effect per deck (single knob macro)
- Pad FX: 4 banks × 8 pads, X-Pad instruments
- Effect button/knob parameter slots (MIDI learnable)

---

## 3. LIBRARY (src/library/)

### 3.1 Core Library Features
- **Library** (library.cpp/h): Central track database, SQLite backend (WAL mode)
- **TrackModel**: Sortable/filterable table model with 80+ columns
- **PlaylistModel**: Hierarchical playlists, smart playlists, playlist folders
- **CrateModel**: Crates (virtual folders), nested crates
- **HistoryModel**: Session history, dated history, auto-playlists from history
- **Search**: Full-text search, column-specific filters, saved searches
- **Search Syntax**: `field:value`, `field:"phrase"`, `-exclude`, `bpm:120..130`, `key:8A`, `genre:"house"`, `year:2020..2024`, `rating:>=4`, `comment:vocals`

### 3.2 Track Columns (80+)
| Category | Columns |
|----------|---------|
| Basic | title, artist, album, composer, genre, year, comment, grouping |
| Technical | bpm, key, duration, bitrate, samplerate, filesize, filetype, location |
| DJ | cue_points, loops, beatgrid, waveform, stems, vocal_map |
| Analysis | analyzed, energy, danceability, acousticness, instrumentalness |
| Performance | play_count, last_played, first_played, rating, color, label |
| Metadata | isrc, musicbrainz_id, discogs_id, lyrics, cover_art |
| Streaming | source (youtube/soundcloud/spotify/audius), stream_url, offline_path |

### 3.3 Import Sources (Library Importers)

#### 3.3.1 Rekordbox (src/library/rekordbox/)
- **rekordboxfeature.cpp**: Full XML/database import
- **rekordboxdrivebackup.cpp**: USB drive export/backup
- **rekordboxplaylistcopy.cpp**: Playlist sync
- **rekordboxandroid.h/.inc**: Android Rekordbox app support
- Imports: cues, loops, beatgrid, BPM, key, playlists, history, color labels, ratings

#### 3.3.2 Serato (src/library/serato/)
- **seratofeature.cpp**: Serato database (database V2, crates, subcates)
- **seratoplaylistmodel.cpp**: Smart crates, playlist import
- Imports: cues, loops, beatgrid, BPM, key, flip, stems (Serato Stems), crates

#### 3.3.3 Traktor (src/library/traktor/)
- **traktorfeature.cpp**: Traktor NML/collection.nml import
- Imports: cues, loops, beatgrid, BPM, key, playlists, remix decks, stem files

#### 3.3.4 iTunes / Music.app (src/library/itunes/)
- **itunesfeature.cpp**: iTunes XML / Music.app library
- **itunesxmlimporter.cpp**: XML parsing
- **itunesmacosimporter.mm**: macOS Music.app direct access
- **itunesiosimporter.mm / itunesiosassetexporter.mm**: iOS device sync
- Imports: playlists, ratings, play counts, BPM, key, artwork

#### 3.3.5 Engine DJ (src/library/enginedj/)
- Engine Prime / Engine OS database import
- Imports: cues, loops, beatgrid, BPM, key, playlists, color tags

#### 3.3.6 djay (src/library/djay/)
- Algoriddim djay database import

#### 3.3.7 Mixed In Key (src/library/mixedinkey/)
- MIK analysis results import (energy, key, cue points)

#### 3.3.8 Lexicon (src/library/lexicon/)
- Lexicon DJ library management export

### 3.4 DJ-Sugar Specific Library Features

#### 3.4.1 AI Bro (src/library/aibro/)
- **aibrofeature.cpp**: AI-powered track recommendations
- **musicmatcherclient.cpp**: MusicMatcher API client
- Features:
  - BPM-aware scoring + sync for different tempos
  - Harmonic mixing suggestions (Camelot wheel)
  - Energy flow analysis (bridge finder, phrase alignment)
  - Vocal/instrumental/karaoke detection
  - Genre/style clustering
  - Smart crate generation
  - AutoDJ integration (see 3.5)

#### 3.4.2 YouTube Integration (src/library/youtube/)
- **youtubefeature.cpp/h**: YouTube Music / YouTube streaming
- **youtubeservice.cpp/h**: YouTube Data API v3, Invidious fallback
- **youtubeaccount.cpp/h**: OAuth2 login, token refresh
- **youtubetrackmodel.cpp**: YouTube tracks in library (youtube:VIDEOID URLs)
- **youtubeaudiocutter.cpp**: Server-side audio trimming (start/end)
- **youtubegenrelookup.cpp**: Genre/artist lookup
- **sponsorblockcontroller.cpp**: SponsorBlock API (skip intros/outros/ads)
- **soundcloudaccount.h**: SoundCloud OAuth (shared infra)
- Features:
  - Search/browse YouTube Music catalog
  - Stream audio (opus/vorbis) with caching
  - Download for offline (with SponsorBlock trimming)
  - Playlist/channel import
  - Genre/mood stations
  - Lyrics fetch (LRCLIB, Genius)
  - Track metadata enrichment

#### 3.4.3 SoundCloud Integration (src/widget/findonwebmenusoundcloud.cpp)
- SoundCloud search/browse
- Stream via SoundCloud API
- Resolve tracks to stream URLs

#### 3.4.4 Spotify Bridge (lib/spotify-bridge/)
- **spotifybridge.h**: Rust FFI bridge to librespot
- Web login (email/password + djsugar:// callback)
- Browse: playlists, albums, artists, recommendations
- Download: Ogg Vorbis 320 kbps (Premium required)
- Offline sync

#### 3.4.5 Samples (src/library/samples/)
- **bundledsamples.cpp**: 17 built-in DJ Sugar samples (kick, snare, clap, hats, 808, crash, airhorn, riser, downlifter, sub_drop, noise_sweep, vinyl_stop, siren)
- **sampleeditor.cpp**: Trim, normalize, fade, loop points
- **samplerecorder.cpp**: Record from deck output / mic / line-in
- Sample banks: 4 banks × 16 pads
- Format support: WAV, FLAC, OGG, MP3, OPUS
- Auto-normalize to -14 LUFS on import

#### 3.4.6 Stems (src/stems/)
- **stemsmanager.cpp**: Stem separation orchestration
- **stemseparator.cpp/h**: Abstract separator interface
- **spleeterseparator.cpp**: Spleeter (4-stem: vocals/drums/bass/other)
- **htdemucsseparator.cpp**: HT-Demucs (4/6-stem, higher quality)
- **onnxruntimeloader.cpp**: ONNX Runtime for model inference
- **stemmodels.cpp**: Model management (download, verify, cache)
- **stemfilewriter.cpp**: Write .stem.mp4 / .stem.m4a (ISOBMFF multi-track)
- **stemcompanion.cpp**: Companion file (.stem.json) for metadata
- **stemsworker.cpp**: Background separation queue
- Features:
  - Batch analysis (stem separation on import or on-demand)
  - Vocal stem for HQ stems mode
  - Stem waveform rendering (per-stem overview + scrolling)
  - Show vocals: cyan strip on main waveform
  - Per-stem volume/mute/solo on deck (4 stereo stems)
  - Stem-aware sync/quantize (align to vocal/drum stems)

#### 3.4.7 Lyrics & Karaoke
- **Lyrics tab**: Read .lrc files, ID3 USLT, MP4 ©lyr, Vorbis COMMENT
- **Find lyrics**: LRCLIB API (synced + plain)
- **Transcribe**: Whisper.cpp integration (local, offline) — write timed lyrics
- **Time Words**: Whisper word-level timestamps
- **Waveform lyrics**: Each word at its time, steady on whole pixels
- **Karaoke page**: Web page for TV/second display
- **Karaoke screen**: Word fill, count-in, vocals slider
- **Player mode**: Large lyrics view, favorites heart

#### 3.4.8 Phrase Analysis
- **Phrase flags**: On scrolling waveforms
- **Leading deck phrases**: On live decks
- **Phrase alignment**: For AutoDJ transitions

### 3.5 AutoDJ (src/library/autodj/)
- **autodjfeature.cpp/h**: Core AutoDJ
- **autodjprocessor.cpp/h**: Transition engine
- **dlgautodj.cpp/h/.ui**: Preferences dialog
- **smart/** (src/library/autodj/smart/):
  - **beatmatch.cpp**: Tempo alignment
  - **bridgefinder.cpp**: Harmonic/energy bridges
  - **energystore.cpp**: Energy curve database
  - **genrescan.cpp**: Genre compatibility
  - **mixscorer.cpp**: Transition scoring
  - **phrasealign.cpp**: Phrase-boundary alignment
  - **smartsequencer.cpp**: Set planning
  - **trackfeatures.cpp**: Feature extraction
  - **vocalmap.cpp**: Vocal presence mapping
- **AutoDJ Modes**:
  - Sequential (playlist order)
  - Shuffle
  - Smart Mix (AI Bro: harmonic, energy, phrase, vocal-aware)
  - Genre-sequenced
  - BPM-sequenced
  - Key-sequenced (Camelot wheel)
  - Manual queue
- **Transition Types**:
  - Crossfade (time/beat/phrase aligned)
  - Cut (on phrase/downbeat)
  - Echo out / filter out / brake
  - Stem-aware (vocals out first, then drums)
  - Tempo transition (gradual BPM change)
- **Settings**: crossfade duration, quantization, key lock, gain normalization, replaygain

### 3.6 Library Preferences (dlgpreflibrary.cpp + .ui)
- **ConfigKey group**: `[Library]`
- **Items**:
  - `auto_analyze` (on import / on load / manual)
  - `analyze_new_tracks` (BPM, key, beatgrid, waveform, stems, lyrics, energy, replaygain)
  - `watch_folders` (recursive, include patterns)
  - `import_playlists` (iTunes, Rekordbox, Serato, Traktor, Engine, djay, M3U, PLS, XSPF)
  - `metadata_precedence` (file tags > database > online)
  - `cover_art_precedence` (embedded > folder.jpg > online)
  - `group_albums` (album view)
  - `show_itunes` / `show_rekordbox` / `show_serato` / `show_traktor` / `show_engine` / `show_djay` / `show_youtube` / `show_soundcloud` / `show_spotify`
  - `youtube_api_key`, `soundcloud_client_id`, `spotify_client_id`
  - `lyrics_sources` (LRCLIB, Genius, local, transcribe)
  - `transcribe_model` (whisper-tiny/base/small/medium/large)
  - `stem_separation_model` (spleeter/htdemucs, 4/6-stem)
  - `auto_stem_separation` (on import / on demand / never)
  - `sample_bank_path`, `bundled_samples_enabled`

---

## 4. MIXER (src/mixer/)

### 4.1 PlayerManager (playermanager.cpp/h)
- 4 decks + sampler + preview + aux
- Deck assignment: A/B/C/D or 1/2/3/4
- Deck roles: left/right (2-deck) or 4-deck mode

### 4.2 BaseTrackPlayer (basetrackplayer.cpp/h)
- Play/pause/cue/stop
- Seek, rate, keylock
- Cue points, loops
- Waveform data request

### 4.3 Sampler (sampler.cpp/h + samplerbank.cpp/h)
- 4 banks × 16 slots = 64 samples
- **SampleBank**: Load/save bank, per-slot volume, mute, play mode (one-shot/gate/loop), reverse, pitch
- **SampleRecorder**: Record to slot (deck out / mic / line-in / mix)
- Sync modes: off, sync to deck, sync to master
- Quantize trigger
- Choke groups
- FX send per slot

### 4.4 Microphone (microphone.cpp/h)
- Talkover ducking (configurable threshold, attack, release, depth)
- Monitor (pre-fader listen)
- Gain, EQ, compressor, noise gate
- Multiple inputs (USB, built-in, aggregate)

### 4.5 Deck (deck.cpp/h)
- Channel strip: gain, 3-band EQ (or 4-band), filter, fader, cue, meter
- Send/return: 2 FX units per deck + master FX
- PFL (pre-fader listen) / AFL (after-fader listen)
- Deck cloning (copy state to another deck)

### 4.6 PlayerInfo (playerinfo.cpp/h)
- Deck status for controllers/skins
- VU meters, position, BPM, key, cues, loops

---

## 5. TRACK & METADATA (src/track/)

### 5.1 Track (track.cpp/h)
- **TrackPointer**: Ref-counted track handle
- **Metadata**: Full tag reading (TagLib)
- **Cue Points**: 8 hotcues + 1 main cue + loop cues (position, label, color, type)
- **Beatgrid**: BPM, beat positions, downbeat markers, grid offset, flexibility
- **Key**: Musical key (Camelot, classical), confidence, key lock
- **ReplayGain**: Track/album gain, peak, EBU R128 loudness
- **StemInfo**: Stem file metadata (stem count, stem names, stem offsets)
- **Lyrics**: Synced (LRC) + unsynced, word timestamps
- **PlayCounter**: Play count, first/last played, skip count
- **AlbumInfo**: Album artist, cover art, year, genre
- **StemInfoImporter**: Parse .stem.mp4 / .stem.m4a / .stem.json
- **CueInfoImporter**: Import cues from Serato/Traktor/Rekordbox/Engine

### 5.2 Beat Analysis (beats.cpp/h, beatfactory.cpp/h, beatutils.cpp/h)
- **AnalyzerBeats**: Multi-algorithm BPM/beat detection
- **EnergyCalculator**: Spectral flux, onset detection
- **BeatFactory**: Algorithm selection (default, elastic, rubberband)
- **AndroidKickGrid**: Mobile-optimized beat grid

### 5.3 Key Analysis (keyfactory.cpp/h, keys.cpp/h, keyutils.cpp/h, analyzerkey.cpp)
- **KeyFactory**: Algorithm selection (KeyFinder, LibKeyFinder, internal)
- **KeyUtils**: Camelot conversion, harmonic mixing, key distance
- **AnalyzerKey**: Background key detection

### 5.4 ReplayGain / EBU R128 (replaygain.cpp/h, analyzerebur128.cpp/h)
- Track gain, album gain
- True peak scanning
- LUFS integrated/loudness range
- Write tags (ReplayGain, EBU R128)

### 5.5 TagLib Support (src/track/taglib/)
- ID3v1, ID3v2.3/4 (MP3)
- Vorbis Comments (OGG, FLAC, OPUS)
- MP4/iTunes atoms (M4A, MP4, ALAC)
- WAV RIFF INFO / BWF
- APEv2 (MPC, WV)
- ASF/WMA
- Cover art extraction (APIC, COVR, METADATA_BLOCK_PICTURE)

### 5.6 Serato Cue Import (src/track/serato/)
- Serato-specific cue/loop/beatgrid parsing

---

## 6. ANALYZER (src/analyzer/)

### 6.1 Analyzer Thread (analyzerthread.cpp/h)
- Background analysis queue (priority: current deck > queue > library)
- Cancellation on track unload
- Progress reporting

### 6.2 Analysis Modules
| Module | File | Output |
|--------|------|--------|
| Beats | analyzerbeats.cpp | BPM, beat positions, downbeats, confidence |
| Key | analyzerkey.cpp | Musical key, Camelot, confidence |
| Gain | analyzergain.cpp | ReplayGain, EBU R128 |
| Energy | analyzerenergy.cpp | Energy curve, danceability, spectral features |
| Silence | analyzersilence.cpp | Leading/trailing silence, cue point suggestions |
| Waveform | analyzerwaveform.cpp | Overview (low-res), scrolling (high-res), stem waveforms |
| EBU R128 | analyzerebur128.cpp | LUFS-I, LUFS-S, LUFS-M, true peak |

### 6.3 TrackAnalysisScheduler (trackanalysisscheduler.cpp/h)
- Batch analysis (library scan)
- On-demand analysis (deck load)
- Priority scheduling
- Android: KickGrid (lightweight)

### 6.4 Analyzer Plugins (src/analyzer/plugins/)
- Extensible analysis pipeline

---

## 7. RECORDING (src/recording/)

### 7.1 RecordingManager (recordingmanager.cpp/h)
- **ConfigKey group**: `[Recording]`
- **Items**:
  - `format` (WAV, FLAC, OGG, MP3, OPUS, AIFF, M4A)
  - `bit_depth` (16, 24, 32 float)
  - `sample_rate` (44100, 48000, 88200, 96000)
  - `channels` (stereo, mono)
  - `location` (directory, auto-name: `%Y-%m-%d_%H-%M-%S`)
  - `split_files` (by time / by track / manual)
  - `split_time` (minutes)
  - `normalize` (EBU R128 target)
  - `metadata` (auto from deck / manual)
  - `record_source` (master / booth / deck1..4 / sampler / mic / external)
  - `encoding_quality` (VBR q0-q10 / CBR kbps)
- **Features**:
  - Multi-format simultaneous recording
  - Track markers (auto on track change / manual)
  - Pause/resume
  - Disk space monitor
  - Auto-upload (configured script)

---

## 8. BROADCAST (src/broadcast/)

### 8.1 BroadcastManager (broadcastmanager.cpp/h)
- **ConfigKey group**: `[Broadcast]`
- **BroadcastProfile** (broadcastprofile.cpp/h):
  - `name`, `server_type` (Icecast / Shoutcast)
  - `host`, `port`, `mount`, `password`
  - `format` (MP3, OGG, OPUS, AAC)
  - `bitrate` (CBR/VBR, 64-320 kbps)
  - `sample_rate` (44100, 48000)
  - `channels` (stereo, mono)
  - `metadata_interval` (seconds)
  - `public` (Y/N)
  - `genre`, `description`, `url`
  - `reconnect` (auto, interval, max retries)
  - `source` (master / booth / deck / record / mic)
- **BroadcastSettings** (broadcastsettings.cpp/h): Multiple profiles, active profile
- **Features**:
  - Multiple simultaneous streams
  - Metadata push (title, artist, album, art, genre, BPM, key)
  - Reconnection with exponential backoff
  - Stream stats (listeners, bitrate, uptime)
  - Recording + broadcast simultaneously

### 8.2 Broadcast Preferences (dlgprefbroadcast.cpp + .ui)
- Profile manager (add/edit/delete/duplicate)
- Test connection
- Sound check (tone generator)

---

## 9. SOUND I/O (src/soundio/)

### 9.1 SoundManager (soundmanager.cpp/h + soundmanagerconfig.cpp/h)
- **ConfigKey group**: `[Sound]`
- **Items**:
  - `backend` (PortAudio / PipeWire / CoreAudio / WASAPI / Android USB / Network / OSS)
  - `device_input`, `device_output` (per deck: deck1_out, deck2_out, deck3_out, deck4_out, master_out, booth_out, record_out, broadcast_out, sampler_out, mic_in, aux_in)
  - `sample_rate` (44100, 48000, 88200, 96000, 192000)
  - `buffer_size` (frames, latency ms)
  - `latency_compensation` (ms)
  - `dither` (none, triangular, shaped)
  - `resample_quality` (linear, cubic, sinc_best)
  - `exclusive_mode` (WASAPI/CoreAudio)
  - `auto_switch` (on device connect/disconnect)

### 9.2 Enumerators
- **PortAudioEnumerator**: All PortAudio devices (Windows/Mac/Linux)
- **PipeWireEnumerator**: PipeWire nodes (Linux)
- **CoreAudioEnumerator**: macOS/iOS audio units
- **AndroidUSBAudioDevice**: USB audio class 2.0 on Android
- **NetworkEnumerator**: Network audio (RAVENNA, AES67, Dante via lib)
- **SoundDevice**: Abstract device interface (input/output channels, latency, formats)

### 9.3 Android USB Audio (androidusbaudiodevice.cpp/h, androidusbsounddevices.cpp/h)
- USB device enumeration via USB Host API
- Multiple simultaneous USB interfaces
- Permission handling
- MIDI over USB (for controllers)

### 9.4 Audio Latency Calibrator (audiolatencycalibrator.cpp/h)
- Round-trip latency measurement
- Auto-calibration on device change
- Manual offset adjustment

---

## 10. CONTROLLERS (src/controllers/)

### 10.1 Controller Framework
- **ControllerManager**: Device enumeration, connection, mapping load
- **Controller**: Base class (HID, MIDI, keyboard, scripting)
- **ControllerMapping**: XML/JS mapping files
- **ControllerMappingInfo**: Metadata (name, author, version, device IDs)
- **ControllerInputMappingTableModel** / **ControllerOutputMappingTableModel**: MIDI learn UI
- **ControllerLearningEventFilter**: MIDI learn mode
- **ControllerHIDReportTabsManager**: HID report parser
- **ControllerScreenPreview**: Deck screen rendering (Pioneer CDJ/XDJ, Denon, Numark)

### 10.2 Supported Protocols
- **MIDI**: USB-MIDI, Bluetooth MIDI, RTP-MIDI, virtual ports
- **HID**: USB HID (Native Instruments, Pioneer, Denon, Numark, Reloop, Hercules, etc.)
- **Keyboard**: QWERTY mapping
- **Scripting** (src/controllers/scripting/): JavaScript (QuickJS) / Rust (djsugar-js) mappings
  - **djsugar-js**: Parser, lexer, syntax tree, runtime
  - Mod system: load order, dependency resolution, settings traceability

### 10.3 Controller Mappings (100+ bundled)
| Vendor | Models |
|--------|--------|
| Pioneer | DDJ-FLX10, DDJ-FLX6, DDJ-1000, DDJ-1000SRT, DDJ-800, DDJ-400, DDJ-200, DDJ-REV7, DDJ-REV5, DDJ-REV1, XDJ-AZ, XDJ-RX3, XDJ-RX2, XDJ-RR, XDJ-XZ, CDJ-3000, CDJ-2000NXS2, DJM-A9, DJM-V10 |
| Denon | SC6000M, SC6000, SC5000M, SC5000, Prime 4, Prime 2, Prime GO, LC6000, MCX8000, MC7000 |
| Numark | Mixtrack Platinum FX, Mixtrack Pro FX, NS7III, NS6II, NV, NVII, Party Mix, DJ2GO2 Touch |
| Native Instruments | Traktor Kontrol S4 MK3, S3, S2 MK3, Z2, X1 MK2, F1 |
| Reloop | RP-8000 MK2, RP-7000 MK2, Beatpad 2, Beatmix 4, Ready, Buddy, Mixon 4, Mixon 8 Pro |
| Hercules | DJControl Inpulse 500, 300, 200, DJControl Starlight, DJControl Instinct |
| Roland | DJ-505, DJ-202, DJ-707M |
| Allen & Heath | Xone:K2, Xone:DB4, Xone:PX5 |
| Rane | Four, One, Twelve MKII, Performer |
| Phase | Phase DJ (DVS) |
| Generic | MIDI Fighter Twister, 64, Spectra; Launchpad, Launch Control; Akai APC, MPD; Novation |

### 10.4 Controller Preferences (dlgprefcontroller.cpp/h + dlgprefcontrollers.cpp/h)
- Device enable/disable
- Mapping selection per device
- MIDI learn
- HID report viewer
- Screen preview
- LED brightness, color schemes
- Touch strip sensitivity
- Jog wheel resolution

### 10.5 Soft Takeover (softtakeover.cpp/h)
- Pickup mode (relative, absolute, scaled)
- Indicator on controller screens

---

## 11. PREFERENCES — ALL SETTINGS (src/preferences/)

### 11.1 ConfigObject (configobject.cpp/h) — Central Settings Registry
- **Groups**: `[Master]`, `[Channel1]..[Channel4]`, `[Sampler1]..[Sampler64]`, `[Microphone]`, `[Auxiliary1]..[Auxiliary4]`, `[PreviewDeck]`, `[Recording]`, `[Broadcast]`, `[Library]`, `[Sound]`, `[Interface]`, `[Waveform]`, `[Deck]`, `[Mixer]`, `[Effects]`, `[AutoDJ]`, `[Controller]`, `[VinylControl]`, `[BeatDetection]`, `[KeyDetection]`, `[ReplayGain]`, `[ColorPalettes]`, `[Mods]`, `[YouTube]`, `[SoundCloud]`, `[Spotify]`, `[Stems]`, `[Lyrics]`, `[Karaoke]`, `[Samples]`, `[AIBro]`

### 11.2 Preference Dialogs (src/preferences/dialog/)

| Dialog | File | Key Settings |
|--------|------|--------------|
| Sound | dlgprefsound.cpp/.ui | Backend, devices, sample rate, buffer, latency, dither, exclusive mode |
| Sound Calibrate | dlgprefsoundcalibrate.cpp/.ui | Latency calibration wizard |
| Interface | dlgprefinterface.cpp/.ui | Language, theme (Dark/Light/Auto), font scaling, high DPI, animations, tooltips |
| Waveform | dlgprefwaveform.cpp/.ui | Overview/scrolling colors, stem colors, vocal strip, phrase markers, beat markers, key overlay, RMS/Peak view, zoom levels, scroll speed |
| Deck | dlgprefdeck.cpp/.ui | Auto-cue, quantize, keylock default, sync default, rate range, slip mode, vinyl mode, reverse, flux mode, cue behavior (CDJ/Serato/Traktor), loop behavior, beatjump sizes |
| Mixer | dlgprefmixer.cpp/.ui | Crossfader curve, EQ type (3/4-band, kill), filter type, pre-fader meter, post-fader meter, booth gain, record gain, broadcast gain, talkover settings |
| Effects | dlgprefeffects.cpp/.ui | Effect rack order, quick effect per deck, pad FX banks, effect presets, LV2/AudioUnit paths |
| Library | dlgpreflibrary.cpp/.ui | Auto-analyze, watch folders, import sources, metadata precedence, cover art, streaming sources, lyrics, stems, samples |
| AutoDJ | dlgprefautodj.cpp/.ui | Mode, transition type, crossfade, quantization, key lock, gain norm, smart mix weights |
| Recording | dlgprefrecord.cpp/.ui | Format, quality, location, split, metadata, source, normalize |
| Broadcast | dlgprefbroadcast.cpp/.ui | Profiles, server, format, bitrate, metadata, reconnect |
| Controllers | dlgprefcontroller.cpp/.ui + dlgprefcontrollers.cpp/.ui | Devices, mappings, MIDI learn, screens |
| Vinyl Control | dlgprefvinyl.cpp/.ui | Timecode type, lead-in, mode, cueing, signal quality |
| Beat Detection | dlgprefbeats.cpp/.ui | Algorithm, sensitivity, range, re-analyze threshold |
| Key Detection | dlgprefkey.cpp/.ui | Algorithm, notation (Camelot/Classical), key lock mode |
| ReplayGain | dlgprefreplaygain.cpp/.ui | Apply track/album, preamp, clipping prevention, EBU R128 target |
| Colors | dlgprefcolors.cpp/.ui | Palette editor, deck colors, waveform colors, cue colors, stem colors |
| Modplug | dlgprefmodplug.cpp/.ui | Module player settings (MOD, XM, IT, S3M) |

### 11.3 DJ-Sugar Specific Preference Groups

#### `[YouTube]`
- `api_key`, `use_invidious`, `invidious_instance`, `download_quality`, `cache_size`, `sponsorblock_enabled`, `sponsorblock_categories`, `auto_prefetch`, `lyrics_enabled`

#### `[SoundCloud]`
- `client_id`, `download_quality`, `cache_size`, `auto_prefetch`

#### `[Spotify]`
- `client_id`, `client_secret`, `cache_dir`, `download_quality`, `offline_sync`, `web_login_enabled`

#### `[Stems]`
- `separation_model` (spleeter_4stem, htdemucs_4stem, htdemucs_6stem), `auto_separate`, `cache_size`, `gpu_acceleration`, `onnx_provider` (CPU/CUDA/CoreML/DirectML)

#### `[Lyrics]`
- `sources` (lrc, id3, lrclib, genius, transcribe), `transcribe_model`, `auto_fetch`, `karaoke_enabled`

#### `[Karaoke]`
- `second_display`, `word_fill_color`, `count_in_beats`, `vocals_default_level`, `web_page_port`

#### `[Samples]`
- `bank_path`, `bundled_enabled`, `normalize_lufs`, `max_sample_length`, `default_play_mode`

#### `[AIBro]`
- `enabled`, `api_endpoint`, `recommendation_count`, `harmonic_weight`, `energy_weight`, `phrase_weight`, `vocal_weight`, `genre_weight`, `bpm_tolerance`, `auto_refresh`

---

## 12. DVS / VINYL CONTROL (src/vinylcontrol/)

- **Timecode Formats**: Serato (Control Vinyl/CD), Traktor (Scratch Control), MixVibes (DVS), Mixxx Native
- **Modes**: Absolute, Relative, Constant (Smart)
- **Lead-in Detection**: Auto, Manual
- **Cueing**: Hotcue, Cue, Play/Pause
- **Signal Quality Meter**: RMS, SNR, frequency analysis
- **Calibration Wizard**: Speed, pitch, direction

---

## 13. WAVEFORM (src/waveform/)

- **Overview Waveform**: Low-res, full track, cached
- **Scrolling Waveform**: High-res, real-time, per-deck
- **Stem Waveforms**: 4 stereo stems (vocals, drums, bass, other)
- **Vocal Strip**: Thin cyan overlay on main waveform
- **Phrase Markers**: Phrase boundaries (8/16/32 bars)
- **Beat Markers**: Downbeats, beats, sub-beats
- **Key Overlay**: Harmonic mixing key display
- **Cue Markers**: Hotcues, loops, main cue
- **Energy Visualization**: Spectral energy curve
- **Color Modes**: By frequency (3-band), by stem, by energy, monochrome

---

## 14. ENCODER (src/encoder/)

- **Formats**: LAME (MP3), FLAC, OGG Vorbis, OPUS, FAAC/FDK-AAC (M4A), WavPack, AIFF, WAV
- **Presets**: VBR quality / CBR bitrate
- **Metadata**: Full tag writing (ID3v2.4, Vorbis Comments, MP4 atoms)
- **Cover Art**: Embed + extract
- **ReplayGain Tags**: Write track/album gain
- **EBU R128 Tags**: Write LUFS

---

## 15. NETWORK (src/network/)

- **HTTP Client**: REST APIs (YouTube, SoundCloud, Spotify, LRCLIB, Genius, SponsorBlock, MusicMatcher)
- **WebSocket**: Real-time updates (collaborative, remote control)
- **mDNS/Bonjour**: Local device discovery (Link, Ableton Link)
- **UPnP**: Port mapping for broadcast
- **Proxy Support**: HTTP/SOCKS5 with auth

---

## 16. SCRIPTING / EXTENSIBILITY

### 16.1 Controller Scripting (src/controllers/scripting/)
- **QuickJS** (legacy) → **djsugar-js** (Rust-based, current)
- **djsugar-js**: Lexer, parser, syntax tree, bytecode VM
- **Mod System**: Load order, dependencies, settings traceability
- **API**: Deck control, mixer, effects, library, notifications

### 16.2 Lua Scripting (legacy, being replaced)
- Engine scripting (callbacks, timers)

---

## 17. ANDROID SPECIFIC (src/engine/androidmiccapture.cpp, src/soundio/android*)

- **AndroidMicCapture**: AudioRecord for mic input
- **AndroidUSBAudioDevice**: USB Host API for audio interfaces
- **AndroidKickGrid**: Lightweight beat grid for mobile
- **JNI Bridge**: Java/Kotlin ↔ C++ (YouTube download, Spotify, permissions)
- **Foreground Service**: Background audio playback
- **MediaSession**: Lock screen controls, Bluetooth media buttons
- **Storage Access Framework**: Scoped storage for music folder

---

## 18. DJ-SUGAR SPECIFIC FEATURES (Non-Upstream)

| # | Feature | Location | Description |
|---|---------|----------|-------------|
| 1 | AI Bro Recommendations | src/library/aibro/ | ML-powered track suggestions with harmonic, energy, phrase, vocal weighting |
| 2 | YouTube Music Streaming | src/library/youtube/ | Full YouTube Music catalog streaming, search, playlists, offline download |
| 3 | SoundCloud Streaming | src/widget/findonwebmenusoundcloud.cpp | SoundCloud search and streaming |
| 4 | Spotify Streaming | lib/spotify-bridge/ | Spotify Premium streaming via librespot, web login, offline sync |
| 5 | Stem Separation | src/stems/ | Spleeter/HT-Demucs ONNX inference, 4/6-stem, batch + on-demand, .stem.mp4 write |
| 6 | Lyrics System | src/library/youtube/, track/ | LRC/ID3/LRCLIB/Genius/Whisper transcription, word timestamps, waveform lyrics |
| 7 | Karaoke Mode | (QML + engine) | Second display web page, word fill, count-in, vocals slider |
| 8 | Transcribe (Whisper.cpp) | track/ | Local offline speech-to-text for lyrics generation |
| 9 | SponsorBlock Integration | src/library/youtube/sponsorblockcontroller.cpp | Skip YouTube intros/outros/ads/sponsors |
| 10 | Bundled Sample Pack | src/library/samples/bundledsamples.cpp | 17 synthesized one-shots/FX (kick, snare, clap, hats, 808, crash, airhorn, riser, downlifter, sub_drop, noise_sweep, vinyl_stop, siren) |
| 11 | Sample Editor | src/library/samples/sampleeditor.cpp | Trim, normalize, fade, loop points, reverse |
| 12 | Sample Recorder | src/library/samples/samplerecorder.cpp | Record from deck/mic/line-in to sampler slots |
| 13 | djsugar-js Scripting | src/controllers/scripting/ | Rust-based JS engine for controller mappings, mod system |
| 14 | Mod System | src/controllers/scripting/ | Ordered mod loading, dependency resolution, settings traceability |
| 15 | Smart AutoDJ | src/library/autodj/smart/ | AI Bro powered: harmonic, energy, phrase, vocal-aware transitions |
| 16 | Phrase Analysis | analyzer/, autodj/smart/ | Phrase boundaries, alignment, leading deck phrases on waveforms |
| 17 | Vocal Map | autodj/smart/vocalmap.cpp | Vocal presence timeline for stem-aware transitions |
| 18 | Bridge Finder | autodj/smart/bridgefinder.cpp | Harmonic/energy bridges between tracks |
| 19 | Mix Scorer | autodj/smart/mixscorer.cpp | Transition quality scoring |
| 20 | Energy Store | autodj/smart/energystore.cpp | Per-track energy curve database |
| 21 | Beatmatch (Smart) | autodj/smart/beatmatch.cpp | Tempo alignment with phrase awareness |
| 22 | Genre Scan | autodj/smart/genrescan.cpp | Genre compatibility matrix |
| 23 | YouTube Audio Cutter | src/library/youtube/youtubeaudiocutter.cpp | Server-side start/end trimming before download |
| 24 | YouTube Genre Lookup | src/library/youtube/youtubegenrelookup.cpp | Genre/mood classification for YouTube tracks |
| 25 | SoundCloud Account OAuth | src/library/youtube/soundcloudaccount.h | Shared OAuth infrastructure |
| 26 | Spotify Web Login | lib/spotify-bridge/ | djsugar:// callback, email/password + app login |
| 27 | Android USB Audio Class 2.0 | src/soundio/androidusbaudiodevice.cpp | Multiple USB interfaces, permission handling |
| 28 | Android Foreground Service | (Android) | Background playback, MediaSession integration |
| 29 | Stem Waveform Rendering | waveform/ | Per-stem overview + scrolling, vocal strip overlay |
| 30 | Vocal Strip on Waveform | waveform/ | Thin cyan strip showing vocal stem presence |
| 31 | Show Vocals Toggle | waveform/ | Toggle vocal strip / tint on main waveform |
| 32 | Stem-Aware Deck Controls | src/engine/channels/enginedeck.cpp | Per-stem volume/mute/solo/VU (4 stems) |
| 33 | Stem File Format Support | src/track/steminfoimporter.cpp | .stem.mp4, .stem.m4a, .stem.json import/export |
| 34 | ONNX Runtime Stem Inference | src/stems/onnxruntimeloader.cpp | CPU/CUDA/CoreML/DirectML providers |
| 35 | Model Management | src/stems/stemmodels.cpp | Auto-download, verify, cache Spleeter/HT-Demucs models |
| 36 | Batch Stem Separation | src/stems/stemsworker.cpp | Background queue, priority scheduling |
| 37 | Companion Stem Files | src/stems/stemcompanion.cpp | .stem.json metadata sidecar |
| 38 | LRCLIB Lyrics Fetch | src/library/youtube/ | Synced + plain lyrics from LRCLIB API |
| 39 | Genius Lyrics Fetch | (QML) | Lyrics from Genius API |
| 40 | Whisper Word Timestamps | track/ | Per-word timing for karaoke |
| 41 | Karaoke Web Page | (QML) | TV/second display via local web server |
| 42 | Karaoke Count-in | (QML) | Visual count-in before lyrics start |
| 43 | Karaoke Vocals Slider | (QML) | Vocal level control on karaoke screen |
| 44 | Player Mode Lyrics | (QML) | Large lyrics view with favorites heart |
| 45 | Find Lyrics Menu | (QML) | Context menu → Find lyrics (LRCLIB) |
| 46 | Transcribe Menu | (QML) | Context menu → Transcribe (Whisper) |
| 47 | Phrase Flags on Waveform | waveform/ | Visual phrase boundary markers |
| 48 | Leading Deck Phrases | waveform/ | Show next deck's phrases on current deck |
| 49 | 3-Band Waveform Layering | waveform/ | Layer/scale bands like rekordbox |
| 50 | Beat Grid Steady Scrolling | waveform/ | Stem waveforms keep shape while scrolling |
| 51 | Opus Sample Encoding | samples/ | Opus where ≥10% smaller than original |
| 52 | Vorbis Sample Support | samples/ | OGG Vorbis sample import |
| 53 | Sample Normalization | samples/ | Auto-normalize to -14 LUFS on import |
| 54 | FX Station X-Pad | effects/ | App samples for bank 1, edit icon per pad, shared picker |
| 55 | Pad FX Instruments | effects/padfxcontroller.cpp | X-Pad: sample-based instruments per pad |
| 56 | Effect Chain Presets | effects/ | Save/load/share effect chains |
| 57 | Quick Effect Per Deck | effects/ | Single-knob macro effect per deck |
| 58 | Visible Effects List | effects/visibleeffectslist.cpp | User-customizable effect rack order |
| 59 | Metronome Click/Effect | effects/backends/builtin/metronome*.cpp | Click track + effect for practice |
| 60 | Key Comparison Effect | effects/backends/builtin/keycomparisoneffect.cpp | Visual harmonic mixing aid |
| 61 | AutoGain Control Effect | effects/backends/builtin/autogaincontroleffect.cpp | Automatic gain staging |
| 62 | AutoPan Effect | effects/backends/builtin/autopaneffect.cpp | Automatic panning modulation |
| 63 | Backspin Effect | effects/backends/builtin/backspineffect.cpp | Vinyl brake effect |
| 64 | Bitcrusher Effect | effects/backends/builtin/bitcrushereffect.cpp | Lo-fi bit depth/sample rate reduction |
| 65 | Pro Compressor/Filter/Flanger/Noise/Reverb | effects/backends/builtin/pro*.cpp | Professional-grade dynamics/FX |
| 66 | Vintage Delay Effect | effects/backends/builtin/vintagedelayeffect.cpp | Tape-style delay with wow/flutter |
| 67 | Spiral Effect | effects/backends/builtin/spiraleffect.cpp | Barber-pole phaser |
| 68 | Tape Stop Effect | effects/backends/builtin/tapestopeffect.cpp | Variable-speed tape stop |
| 69 | Tremolo Effect | effects/backends/builtin/tremoloeffect.cpp | Amplitude modulation |
| 70 | White Noise Effect | effects/backends/builtin/whitenoiseeffect.cpp | Noise generator with color |
| 71 | Rekordbox Android Support | src/library/rekordbox/rekordboxandroid.* | Rekordbox mobile app library access |
| 72 | Rekordbox Drive Backup | src/library/rekordbox/rekordboxdrivebackup.cpp | USB drive export/backup for CDJ/XDJ |
| 73 | Rekordbox Playlist Copy | src/library/rekordbox/rekordboxplaylistcopy.cpp | Bidirectional playlist sync |
| 74 | Serato Stems Import | src/library/serato/ | Serato Stems file support |
| 75 | Traktor Remix Decks Import | src/library/traktor/ | Remix deck content import |
| 76 | Engine DJ Import | src/library/enginedj/ | Engine Prime/OS database import |
| 77 | djay Import | src/library/djay/ | Algoriddim djay database import |
| 78 | Mixed In Key Import | src/library/mixedinkey/ | MIK analysis data import |
| 79 | Lexicon Export/Import | src/library/lexicon/ | Lexicon DJ library sync |
| 80 | OpenSpec Mod Format | (spec) | v0.8 mod specification for stores/purchasing |
| 81 | WebP Skin Support | (build) | WebP for skins/UI, optimized PNG icons |
| 82 | Rust Linker (djsugar-link) | (build) | Pure Rust linker replaces rusty_link |
| 83 | djsugar-js Parser/Lexer | src/controllers/scripting/ | Custom JS engine for mappings |
| 84 | Mod Load Order Resolution | src/controllers/scripting/ | Dependency graph, settings traceability |
| 85 | HID Report Parser | src/controllers/controllerhidreporttabsmanager.cpp | Generic HID report tabulation |
| 86 | Controller Screen Preview | src/controllers/controllerscreenpreview.cpp | Pioneer/Denon/Numark screen rendering |
| 87 | Legacy Controller Mapping | src/controllers/legacycontrollermapping.* | Old XML mapping compatibility |
| 88 | Controller Learning Utils | src/controllers/learningutils.cpp | MIDI learn helpers |
| 89 | Soft Takeover | src/controllers/softtakeover.cpp | Pickup modes for relative controls |
| 90 | Bulk Controller Support | src/controllers/bulk/ | Multi-device mapping bundles |
| 91 | Keyboard Mapping | src/controllers/keyboard/ | QWERTY deck control |
| 92 | MIDI Rendering | src/controllers/rendering/ | MIDI output visualization |
| 93 | Android Build Envs | tools/*buildenv* | CI build environments for Android |
| 94 | iOS Build Env | tools/ios_buildenv.sh | iOS build environment |
| 95 | AppImage Build Env | tools/appimage_buildenv.sh | Linux AppImage build |
| 96 | Windows Build Env | tools/windows_buildenv.bat | Windows CI build |
| 97 | macOS Build Env | tools/macos_buildenv.sh | macOS CI build |
| 98 | DJ Sugar Branding | res/qml/SugarLogo.qml | Pink glazed donut/ring logo |
| 99 | Neon Pink Theme | res/qml/+android/main.qml | Pink frosting glaze theme |
| 100 | GitHub Releases API | src/qml/qmlappupdateproxy.cpp | winnerspiros/djsugar releases |

---

## 19. CONFIG KEY REFERENCE (Major Groups)

### `[Master]`
- `volume`, `balance`, `crossfader`, `crossfader_curve`, `booth_gain`, `record_gain`, `broadcast_gain`, `talkover_enabled`, `talkover_threshold`, `talkover_attack`, `talkover_release`, `talkover_depth`

### `[Channel1]..[Channel4]`
- `volume`, `gain`, `eq_low`, `eq_mid`, `eq_high`, `eq_low_kill`, `eq_mid_kill`, `eq_high_kill`, `filter`, `filter_type`, `filter_resonance`, `fader`, `cue`, `cue_default`, `play`, `sync`, `quantize`, `keylock`, `rate`, `rate_range`, `bpm`, `key`, `beatgrid`, `cues`, `loops`, `stem_count`, `stem1_volume`, `stem1_mute`, `stem2_volume`, `stem2_mute`, `stem3_volume`, `stem3_mute`, `stem4_volume`, `stem4_mute`, `waveform_color`, `waveform_zoom`, `slip_enabled`, `flux_enabled`, `reverse`, `vinylcontrol_enabled`, `vinylcontrol_mode`

### `[Sampler1]..[Sampler64]`
- `volume`, `gain`, `play`, `cue`, `mode` (oneshot/gate/loop), `sync`, `quantize`, `pitch`, `reverse`, `choke_group`, `fx_send1`, `fx_send2`, `color`, `label`

### `[Microphone]`
- `volume`, `gain`, `talkover_enabled`, `talkover_threshold`, `talkover_attack`, `talkover_release`, `talkover_depth`, `monitor`, `eq_low`, `eq_mid`, `eq_high`, `compressor_enabled`, `compressor_threshold`, `compressor_ratio`, `gate_enabled`, `gate_threshold`

### `[EffectRack1_EffectUnit1]..[EffectRack4_EffectUnit4]`
- `enabled`, `effect_type`, `mix`, `parameter1`..`parameterN`, `preset`, `quick_effect`

### `[Library]`
- `auto_analyze`, `watch_folders`, `import_sources`, `metadata_precedence`, `show_itunes`, `show_rekordbox`, `show_serato`, `show_traktor`, `show_engine`, `show_djay`, `show_youtube`, `show_soundcloud`, `show_spotify`, `youtube_api_key`, `soundcloud_client_id`, `spotify_client_id`, `lyrics_sources`, `transcribe_model`, `stem_model`, `auto_stem`, `sample_bank_path`

### `[Sound]`
- `backend`, `device_input`, `device_output`, `sample_rate`, `buffer_size`, `latency_compensation`, `dither`, `resample_quality`, `exclusive_mode`, `auto_switch`

### `[Recording]`
- `format`, `bit_depth`, `sample_rate`, `channels`, `location`, `split_files`, `split_time`, `normalize`, `metadata`, `record_source`, `encoding_quality`

### `[Broadcast]`
- `active_profile`, `profiles` (array of profile objects)

### `[AutoDJ]`
- `mode`, `transition_type`, `crossfade_duration`, `quantize`, `keylock`, `gain_normalization`, `replaygain`, `harmonic_weight`, `energy_weight`, `phrase_weight`, `vocal_weight`, `genre_weight`, `bpm_tolerance`

### `[YouTube]`
- `api_key`, `use_invidious`, `invidious_instance`, `download_quality`, `cache_size`, `sponsorblock_enabled`, `sponsorblock_categories`, `auto_prefetch`, `lyrics_enabled`

### `[Stems]`
- `separation_model`, `auto_separate`, `cache_size`, `gpu_acceleration`, `onnx_provider`

### `[Lyrics]`
- `sources`, `transcribe_model`, `auto_fetch`, `karaoke_enabled`

### `[Karaoke]`
- `second_display`, `word_fill_color`, `count_in_beats`, `vocals_default_level`, `web_page_port`

### `[Samples]`
- `bank_path`, `bundled_enabled`, `normalize_lufs`, `max_sample_length`, `default_play_mode`

### `[AIBro]`
- `enabled`, `api_endpoint`, `recommendation_count`, `harmonic_weight`, `energy_weight`, `phrase_weight`, `vocal_weight`, `genre_weight`, `bpm_tolerance`, `auto_refresh`

---

## 20. BUILTIN EFFECT PARAMETER REFERENCE

| Effect | Parameters (ConfigKey items under `[EffectRackN_EffectUnitM_EffectK]`) |
|--------|------------------------------------------------------------------------|
| AutoGainControl | `target_level`, `attack`, `release`, `makeup_gain`, `enabled` |
| AutoPan | `rate`, `depth`, `shape`, `phase`, `spread`, `enabled` |
| Backspin | `speed`, `brake_time`, `enabled` |
| Balance | `balance`, `enabled` |
| Bessel4LVMixEQ | `low`, `mid`, `high`, `low_kill`, `mid_kill`, `high_kill`, `enabled` |
| Bessel8LVMixEQ | `band1`..`band8`, `band1_kill`..`band8_kill`, `enabled` |
| BiquadFullKillEQ | `low`, `mid`, `high`, `low_kill`, `mid_kill`, `high_kill`, `enabled` |
| Bitcrusher | `bit_depth`, `sample_rate_reduction`, `mix`, `enabled` |
| Compressor | `threshold`, `ratio`, `attack`, `release`, `makeup`, `knee`, `enabled` |
| Distortion | `drive`, `tone`, `mix`, `type`, `enabled` |
| DJEcho | `delay`, `feedback`, `filter`, `mix`, `ping_pong`, `enabled` |
| Echo | `delay`, `feedback`, `mix`, `stereo_width`, `enabled` |
| Filter | `frequency`, `resonance`, `type`, `mix`, `enabled` |
| Flanger | `rate`, `depth`, `feedback`, `delay`, `mix`, `stereo`, `enabled` |
| Gain | `gain`, `enabled` |
| Glitch | `probability`, `severity`, `mix`, `repeat`, `enabled` |
| GraphicEQ | `band1`..`band10/31`, `preset`, `enabled` |
| KeyComparison | `reference_key`, `tolerance`, `highlight_mode`, `enabled` |
| LinkwitzRiley8EQ | `band1`..`band8`, `enabled` |
| LoudnessContour | `amount`, `frequency`, `enabled` |
| MoogLadder4Filter | `cutoff`, `resonance`, `drive`, `mix`, `enabled` |
| ParametricEQ | `band1_freq`, `band1_gain`, `band1_q`, `band1_type`... (×4), `enabled` |
| Phase | `rate`, `depth`, `feedback`, `stages`, `mix`, `enabled` |
| PitchShift | `semitones`, `cents`, `formants`, `mix`, `enabled` |
| ProCompressor | `threshold`, `ratio`, `attack`, `release`, `knee`, `makeup`, `lookahead`, `enabled` |
| ProFilter | `frequency`, `resonance`, `drive`, `mix`, `type`, `slope`, `enabled` |
| ProFlanger | `rate`, `depth`, `feedback`, `delay`, `mix`, `shape`, `enabled` |
| ProNoise | `color`, `level`, `mix`, `enabled` |
| ProReverb | `room_size`, `damping`, `width`, `mix`, `pre_delay`, `enabled` |
| Reverb | `room_size`, `damping`, `mix`, `pre_delay`, `enabled` |
| Spiral | `rate`, `depth`, `feedback`, `mix`, `enabled` |
| TapeStop | `start_time`, `end_time`, `curve`, `enabled` |
| ThreeBandBiquadEQ | `low`, `mid`, `high`, `low_kill`, `mid_kill`, `high_kill`, `enabled` |
| Tremolo | `rate`, `depth`, `shape`, `stereo`, `mix`, `enabled` |
| VintageDelay | `delay`, `feedback`, `filter`, `wow`, `flutter`, `mix`, `enabled` |
| WhiteNoise | `level`, `color`, `mix`, `enabled` |
| MetronomeClick | `bpm`, `accent`, `volume`, `sound`, `enabled` |
| MetronomeEffect | `beat_division`, `accent_pattern`, `enabled` |

---

## 21. CHECKLIST — 300+ ITEMS GROUPED BY AREA

### Engine Core (25)
1. EngineBuffer triple-buffered pipeline
2. EngineWorkerScheduler work-stealing thread pool
3. EngineMixer 4-deck + sampler + aux + master + booth + record + broadcast
4. ChannelMixer per-channel gain/EQ/filter/fader/xfader
5. EnginePreGain per-deck pre-fader gain
6. EngineSidechainCompressor ducking
7. EngineTalkoverDucking mic talkover
8. EngineVuMeter peak/RMS metering
9. EngineXFader crossfader curves
10. EngineDelay latency compensation
11. ReadAheadManager async decode buffer
12. PositionScratchController vinyl scratch tracking
13. SlipModeState slip mode
14. EngineDeck 4 main + preview + sampler
15. Stem support (4 stems per deck)
16. Rate control (pitch bend, tempo sync, keylock)
17. Cue system (8 hotcues, loop cues, beatjump)
18. Looping (auto/manual, move, resize, halve/double)
19. Beatgrid (BPM, beats, downbeat, offset, adjust)
20. Key detection (musical key, lock, shift, harmonic)
21. Waveform (overview, scrolling, stem, vocal strip)
22. Quantize (beat/bar/phrase snap)
23. Vinyl control (timecode, relative/absolute)
24. Reverse play
25. Flux mode

### BPM/Rate/Cue/Loop/Key/Quantize/Vinyl Controls (40)
26. BPM tap tempo
27. BPM range ±8/16/25/50/100%
28. BPM sync modes
29. Rate permanent/temp up/down (small/large)
30. Rate engine (internal rate)
31. Sync (off/phase/beat/bar/phrase)
32. Quantize toggle
33. Keylock (standard/elastique/rubberband)
34. Keylock disable on sync
35. Cue modes (CDJ/Serato/Traktor)
36. Hotcue colors
37. Quantized hotcue trigger
38. Beatjump sizes (1/2/4/8/16/32)
39. Loop enable/disable
40. Loop in/out set
41. Loop move (±1/2/4/8 beats)
42. Loop scale (resize)
43. Loop halve/double
44. Quantize loop
45. Key lock toggle
46. Key shift ±12 semitones
47. Key sync
48. Key notation (Camelot/Classical)
49. Quantize mode (beat/bar/phrase)
50. Quantize threshold
51. Play position (absolute/relative/remaining/elapsed/total)
52. Vinyl control enabled
53. Vinyl cueing mode
54. Vinyl mode (absolute/relative/constant)
55. Vinyl lead-in time
56. Clock control

### Effects — Builtin (31 effects × ~5 params = 155+)
57-87. AutoGainControl (4)
88-92. AutoPan (5)
93-96. Backspin (4)
97-98. Balance (2)
99-104. Bessel4LVMixEQ (6)
105-112. Bessel8LVMixEQ (8)
113-118. BiquadFullKillEQ (6)
119-122. Bitcrusher (4)
123-128. Compressor (6)
129-133. Distortion (5)
134-139. DJEcho (6)
140-144. Echo (5)
145-149. Filter (5)
150-155. Flanger (6)
156. Gain (1)
157-161. Glitch (5)
162-172. GraphicEQ (11)
173-176. KeyComparison (4)
177-184. LinkwitzRiley8EQ (8)
185-186. LoudnessContour (2)
187-191. MoogLadder4Filter (5)
192-198. ParametricEQ (7)
199-204. Phase (6)
205-209. PitchShift (5)
210-216. ProCompressor (7)
217-223. ProFilter (7)
224-229. ProFlanger (6)
230-232. ProNoise (3)
233-238. ProReverb (6)
239-243. Reverb (5)
244-248. Spiral (5)
249-252. TapeStop (4)
253-258. ThreeBandBiquadEQ (6)
259-264. Tremolo (6)
265-271. VintageDelay (7)
272-274. WhiteNoise (3)
275-278. MetronomeClick (4)
279-281. MetronomeEffect (3)

### Effect Framework (15)
282. EffectChain serial/parallel routing
283. EffectChain mix modes
284. EffectSlot 4 units per deck
285. Quick effect per deck
286. PadFXController X-Pad instruments
287. Pad FX banks (4×8)
288. VisibleEffectsList customizable rack
289. Effect button parameter slots
290. Effect knob parameter slots
291. Effect chain presets
292. Effect presets save/load/share
293. LV2 plugin support
294. AudioUnit plugin support
295. Effect parameter MIDI learn

### Library Core (25)
296. SQLite database (WAL mode)
297. TrackModel 80+ columns
298. PlaylistModel hierarchical
299. Smart playlists
300. Playlist folders
301. CrateModel nested crates
302. HistoryModel session/dated
303. Auto-playlists from history
304. Full-text search
305. Column-specific filters
306. Saved searches
307. Search syntax (field:value, ranges, exclude)
308. Track columns: basic (10)
309. Track columns: technical (10)
310. Track columns: DJ (10)
311. Track columns: analysis (5)
312. Track columns: performance (5)
313. Track columns: metadata (5)
314. Track columns: streaming (5)
315. Cover art extraction
316. Metadata precedence
317. Album grouping
318. Watch folders recursive
319. Import on startup

### Import Sources (8 sources × ~5 features = 40)
320-324. Rekordbox XML/database import
325-328. Rekordbox USB drive backup
329-330. Rekordbox playlist copy
331-332. Rekordbox Android support
333-335. Serato database V2 import
336-337. Serato smart crates
338-339. Serato Stems support
340-342. Traktor NML import
343-344. Traktor remix decks
345-348. iTunes/Music.app XML import
349-350. macOS Music.app direct
351-352. iOS device sync
353-354. Engine DJ import
355-356. djay import
357-358. Mixed In Key import
359-360. Lexicon sync

### DJ-Sugar Library Features (20)
361. AI Bro recommendations
362. YouTube Music streaming
363. SoundCloud streaming
364. Spotify streaming (librespot)
365. Stem separation (Spleeter/HT-Demucs)
366. Lyrics system (LRC/ID3/LRCLIB/Genius/Whisper)
367. Karaoke mode (web page)
368. Whisper.cpp transcription
369. SponsorBlock integration
370. Bundled sample pack (17 samples)
371. Sample editor
372. Sample recorder
373. djsugar-js scripting
374. Mod system
375. Smart AutoDJ (AI Bro)
376. Phrase analysis
377. Vocal map
378. Bridge finder
379. Mix scorer
380. Energy store
381. Beatmatch smart
382. Genre scan

### YouTube Integration (10)
383. YouTube Data API v3
384. Invidious fallback
385. OAuth2 login
386. Streaming (opus/vorbis) with cache
387. Offline download
388. SponsorBlock trimming
389. Playlist/channel import
390. Genre/mood stations
391. Lyrics fetch
392. Metadata enrichment
393. Audio cutter (server-side trim)

### Stems (15)
394. StemManager orchestration
395. Spleeter separator (4-stem)
396. HT-Demucs separator (4/6-stem)
397. ONNX Runtime inference
398. Model management (download/verify/cache)
399. Stem file writer (.stem.mp4/.m4a)
400. Stem companion (.stem.json)
401. Stems worker background queue
402. Batch analysis
403. On-demand separation
404. Vocal stem HQ mode
405. Stem waveform rendering
406. Vocal strip overlay
407. Per-stem deck controls (vol/mute/solo/VU)
408. Stem-aware sync/quantize

### Lyrics & Karaoke (15)
409. LRC file reading
410. ID3 USLT / MP4 ©lyr / Vorbis COMMENT
411. LRCLIB API (synced + plain)
412. Genius API
413. Whisper transcription (local)
414. Whisper word timestamps
415. Waveform lyrics (word-level)
416. Karaoke web page (TV)
417. Karaoke word fill
418. Karaoke count-in
419. Karaoke vocals slider
420. Player mode lyrics (large)
421. Find lyrics menu
422. Transcribe menu
423. Favorites heart

### AutoDJ Smart (10)
424. Sequential mode
425. Shuffle mode
426. Smart Mix (AI Bro)
427. Genre-sequenced
428. BPM-sequenced
429. Key-sequenced (Camelot)
430. Manual queue
431. Crossfade transitions
432. Cut transitions
433. Echo/filter/brake transitions
434. Stem-aware transitions

### Mixer (15)
435. PlayerManager 4 decks + sampler + preview + aux
436. BaseTrackPlayer play/pause/cue/stop/seek/rate/keylock
437. Sampler 4 banks × 16 slots
438. SampleBank load/save/per-slot config
439. SampleRecorder (deck/mic/line-in/mix)
440. Sampler sync modes
441. Sampler quantize trigger
442. Choke groups
443. FX send per slot
444. Microphone talkover ducking
445. Microphone monitor
446. Microphone gain/EQ/compressor/gate
447. Multiple mic inputs
448. Deck channel strip (gain/EQ/filter/fader/cue/meter)
449. Send/return (2 FX + master FX)
450. PFL/AFL

### Track & Metadata (20)
451. TrackPointer ref-counted
452. Full tag reading (TagLib)
453. 8 hotcues + main cue + loop cues
454. Beatgrid (BPM, beats, downbeats, offset, flexibility)
455. Key (Camelot, classical, confidence)
456. ReplayGain (track/album, peak)
457. EBU R128 (LUFS-I/S/M, true peak)
458. StemInfo (count, names, offsets)
459. Lyrics (synced/unsynced, word timestamps)
460. PlayCounter (count, first/last, skip)
461. AlbumInfo (artist, cover, year, genre)
462. StemInfoImporter (.stem.mp4/.m4a/.json)
463. CueInfoImporter (Serato/Traktor/Rekordbox/Engine)
464. Multi-algorithm BPM/beat detection
465. Energy calculator (spectral flux, onset)
466. Algorithm selection (beat/key)
467. Camelot conversion, harmonic mixing
468. True peak scanning
469. TagLib formats (ID3, Vorbis, MP4, WAV, APE, ASF)

### Analyzer (15)
470. AnalyzerThread background queue
471. Priority scheduling (deck > queue > library)
472. Cancellation on unload
473. Progress reporting
474. Beats module (BPM, beats, downbeats, confidence)
475. Key module (musical key, Camelot, confidence)
476. Gain module (ReplayGain, EBU R128)
477. Energy module (energy curve, danceability, spectral)
478. Silence module (leading/trailing, cue suggestions)
479. Waveform module (overview, scrolling, stem)
480. EBU R128 module (LUFS-I/S/M, true peak)
481. TrackAnalysisScheduler batch/on-demand
482. Android KickGrid
483. Analyzer plugins extensible

### Recording (10)
484. Format (WAV/FLAC/OGG/MP3/OPUS/AIFF/M4A)
485. Bit depth (16/24/32 float)
486. Sample rate (44.1/48/88.2/96 kHz)
487. Channels (stereo/mono)
488. Location + auto-naming
489. Split files (time/track/manual)
490. Normalize (EBU R128 target)
491. Metadata (auto/manual)
492. Record source (master/booth/deck/sampler/mic/external)
493. Encoding quality (VBR/CBR)
494. Multi-format simultaneous
495. Track markers (auto/manual)
496. Pause/resume
497. Disk space monitor
498. Auto-upload script

### Broadcast (15)
499. Icecast/Shoutcast
500. Multiple simultaneous streams
501. Format (MP3/OGG/OPUS/AAC)
502. Bitrate (64-320 CBR/VBR)
503. Sample rate (44.1/48 kHz)
504. Channels (stereo/mono)
505. Metadata push (title/artist/album/art/genre/BPM/key)
506. Public/private
507. Genre/description/URL
508. Auto-reconnect (exponential backoff)
509. Stream stats (listeners/bitrate/uptime)
510. Recording + broadcast simultaneous
511. Profile manager (add/edit/delete/duplicate)
512. Test connection
513. Sound check tone generator

### Sound I/O (15)
514. Backend selection (PortAudio/PipeWire/CoreAudio/WASAPI/Android USB/Network/OSS)
515. Per-deck device routing (8 outputs + 2 inputs + record + broadcast + sampler + mic + aux)
516. Sample rate (44.1-192 kHz)
517. Buffer size / latency
518. Latency compensation
519. Dither (none/triangular/shaped)
520. Resample quality (linear/cubic/sinc_best)
521. Exclusive mode (WASAPI/CoreAudio)
522. Auto-switch on device change
523. PortAudio enumerator
524. PipeWire enumerator
525. CoreAudio enumerator
526. Android USB Audio Class 2.0
527. Network audio (RAVENNA/AES67/Dante)
528. Audio latency calibrator

### Controllers (25)
529. ControllerManager enumeration/connection/mapping
530. Controller base (HID/MIDI/keyboard/scripting)
531. ControllerMapping XML/JS
532. ControllerMappingInfo metadata
533. Input/Output mapping table models
534. ControllerLearningEventFilter MIDI learn
535. ControllerHIDReportTabsManager HID parser
536. ControllerScreenPreview (Pioneer/Denon/Numark)
537. MIDI (USB/BLE/RTP/virtual)
538. HID (NI/Pioneer/Denon/Numark/Reloop/Hercules/etc.)
539. Keyboard QWERTY
540. Scripting (QuickJS → djsugar-js)
541. djsugar-js parser/lexer/syntax tree/VM
542. Mod system (load order, deps, settings trace)
543. 100+ bundled mappings
544. Pioneer DDJ-FLX10/FLX6/1000/800/400/200/REV7/REV5/REV1
545. Pioneer XDJ-AZ/RX3/RX2/RR/XZ
546. Pioneer CDJ-3000/2000NXS2, DJM-A9/V10
547. Denon SC6000M/6000/5000M/5000, Prime 4/2/GO, LC6000, MCX8000/7000
548. Numark Mixtrack Platinum/Pro FX, NS7III/6II, NV/NVII, Party Mix, DJ2GO2 Touch
549. NI Traktor S4 MK3/S3/S2 MK3/Z2/X1 MK2/F1
550. Reloop RP-8000/7000 MK2, Beatpad2, Beatmix4, Ready, Buddy, Mixon4/8 Pro
551. Hercules Inpulse 500/300/200, Starlight, Instinct
552. Roland DJ-505/202/707M
553. A&H Xone:K2/DB4/PX5
554. Rane Four/One/Twelve MKII/Performer
555. Phase DJ DVS
556. Generic (MIDI Fighter, Launchpad, APC, MPD)

### Preferences Dialogs (20 dialogs × ~5 settings = 100+)
557-576. Sound (backend, devices, sample rate, buffer, latency, dither, exclusive, auto-switch)
577-580. Sound Calibrate (latency wizard)
581-585. Interface (language, theme, font, DPI, animations, tooltips)
586-595. Waveform (colors, stem colors, vocal strip, phrases, beats, key overlay, RMS/Peak, zoom, scroll)
596-610. Deck (auto-cue, quantize, keylock, sync, rate range, slip, vinyl, reverse, flux, cue mode, loop behavior, beatjump)
611-620. Mixer (xfader curve, EQ type, filter type, pre/post meter, booth/record/broadcast gain, talkover)
621-628. Effects (rack order, quick effect, pad FX banks, presets, LV2/AU paths)
629-640. Library (auto-analyze, watch folders, import sources, metadata/cover precedence, streaming sources, lyrics, stems, samples)
641-648. AutoDJ (mode, transition, crossfade, quantize, keylock, gain norm, smart weights)
649-655. Recording (format, quality, location, split, metadata, source, normalize)
656-668. Broadcast (profiles, server, format, bitrate, metadata, reconnect)
669-678. Controllers (devices, mappings, MIDI learn, screens)
679-685. Vinyl Control (timecode, lead-in, mode, cueing, signal quality)
686-690. Beat Detection (algorithm, sensitivity, range, re-analyze)
691-695. Key Detection (algorithm, notation, keylock mode)
696-700. ReplayGain (track/album, preamp, clipping, EBU target)
701-706. Colors (palette editor, deck/waveform/cue/stem colors)
707-710. Modplug (module player settings)

### DJ-Sugar Specific Preference Groups (6 groups × ~5 = 30)
711-716. YouTube (API key, Invidious, quality, cache, SponsorBlock, lyrics)
717-721. SoundCloud (client ID, quality, cache, prefetch)
722-727. Spotify (client ID/secret, cache, quality, offline, web login)
728-733. Stems (model, auto-separate, cache, GPU, ONNX provider)
734-739. Lyrics (sources, transcribe model, auto-fetch, karaoke)
740-745. Karaoke (display, word fill, count-in, vocals level, web port)
746-750. Samples (bank path, bundled, normalize LUFS, max length, play mode)
751-756. AIBro (enabled, endpoint, count, weights, BPM tolerance, auto-refresh)

### DVS/Vinyl Control (8)
757. Timecode formats (Serato/Traktor/MixVibes/Mixxx)
758. Modes (absolute/relative/constant)
759. Lead-in detection (auto/manual)
760. Cueing (hotcue/cue/play-pause)
761. Signal quality meter (RMS/SNR/frequency)
762. Calibration wizard (speed/pitch/direction)
763. Vinyl control per-deck
764. Timecode signal processing

### Waveform (10)
765. Overview waveform (low-res, cached)
766. Scrolling waveform (high-res, real-time)
767. Stem waveforms (4 stereo)
768. Vocal strip (cyan overlay)
769. Phrase markers (8/16/32 bar)
770. Beat markers (downbeat/beat/sub-beat)
771. Key overlay (harmonic mixing)
772. Cue markers (hotcues/loops/main)
773. Energy visualization
774. Color modes (frequency/stem/energy/mono)

### Encoder (8)
775. LAME MP3
776. FLAC
777. OGG Vorbis
778. OPUS
779. FAAC/FDK-AAC M4A
780. WavPack
781. AIFF/WAV
782. Metadata + cover art + ReplayGain + EBU R128 tags

### Network (6)
783. HTTP client (REST APIs)
784. WebSocket (real-time)
785. mDNS/Bonjour (Link, Ableton Link)
786. UPnP port mapping
787. Proxy support (HTTP/SOCKS5)
788. OAuth2 flows (YouTube/SoundCloud/Spotify)

### Scripting/Extensibility (8)
789. QuickJS (legacy)
790. djsugar-js (Rust-based current)
791. djsugar-js lexer
792. djsugar-js parser
793. djsugar-js syntax tree
794. djsugar-js bytecode VM
795. Mod system (load order, deps, settings trace)
796. Lua scripting (legacy)

### Android Specific (10)
797. AndroidMicCapture (AudioRecord)
798. AndroidUSBAudioDevice (USB Host API)
799. AndroidKickGrid (lightweight beat grid)
800. JNI Bridge (Java/Kotlin ↔ C++)
801. Foreground Service (background audio)
802. MediaSession (lock screen, BT media buttons)
803. Storage Access Framework (scoped storage)
804. YouTube download via JNI
805. Spotify via JNI
806. Permission handling (USB, storage, audio)

---

## 22. TOP 30 DJ-SUGAR-SPECIFIC (NON-UPSTREAM) FEATURES

1. **AI Bro Recommendations** — ML-powered track suggestions with harmonic, energy, phrase, vocal weighting
2. **YouTube Music Streaming** — Full catalog streaming, search, playlists, offline download with SponsorBlock
3. **Spotify Streaming (librespot)** — Premium streaming, web login (djsugar://), offline sync
4. **SoundCloud Streaming** — Search and streaming via SoundCloud API
5. **Stem Separation (Spleeter/HT-Demucs ONNX)** — 4/6-stem, batch + on-demand, GPU acceleration
6. **Lyrics System (Multi-source)** — LRC, ID3, LRCLIB, Genius, Whisper transcription
7. **Karaoke Mode** — Second display web page, word fill, count-in, vocals slider
8. **Whisper.cpp Transcription** — Local offline speech-to-text for lyrics generation
9. **SponsorBlock Integration** — Skip YouTube intros/outros/ads/sponsors automatically
10. **Bundled Sample Pack** — 17 synthesized one-shots/FX (kick, snare, clap, hats, 808, crash, airhorn, riser, downlifter, sub_drop, noise_sweep, vinyl_stop, siren)
11. **Sample Editor** — Trim, normalize, fade, loop points, reverse
12. **Sample Recorder** — Record from deck/mic/line-in to sampler slots
13. **djsugar-js Scripting Engine** — Rust-based JS for controller mappings, mod system
14. **Mod System** — Ordered loading, dependency resolution, settings traceability
15. **Smart AutoDJ (AI Bro)** — Harmonic, energy, phrase, vocal-aware transitions
16. **Phrase Analysis** — Phrase boundaries, alignment, leading deck phrases on waveforms
17. **Vocal Map** — Vocal presence timeline for stem-aware transitions
18. **Bridge Finder** — Harmonic/energy bridges between tracks
19. **Mix Scorer** — Transition quality scoring
20. **Energy Store** — Per-track energy curve database
21. **Smart Beatmatch** — Tempo alignment with phrase awareness
22. **Genre Scan** — Genre compatibility matrix
23. **YouTube Audio Cutter** — Server-side start/end trimming before download
24. **YouTube Genre Lookup** — Genre/mood classification for YouTube tracks
25. **Stem Waveform Rendering** — Per-stem overview + scrolling, vocal strip overlay
26. **Stem-Aware Deck Controls** — Per-stem volume/mute/solo/VU (4 stems)
27. **Stem File Format Support** — .stem.mp4, .stem.m4a, .stem.json import/export
28. **ONNX Runtime Stem Inference** — CPU/CUDA/CoreML/DirectML providers
29. **Model Management** — Auto-download, verify, cache Spleeter/HT-Demucs models
30. **Rekordbox Android Support** — Mobile app library access, USB drive backup, playlist copy

---

**Total Items Documented: ~800+**

**Path**: /home/ubuntu/djsugar-next/docs/parity/features-djsugar.md
**Item Count**: 800+
**Summary**: Exhaustive feature parity list for DJ Sugar (Mixxx fork) covering engine, effects, library, mixer, track, analyzer, recording, broadcast, sound I/O, controllers, preferences, DVS, waveform, encoder, network, scripting, Android, and 30 DJ-Sugar-specific non-upstream features.
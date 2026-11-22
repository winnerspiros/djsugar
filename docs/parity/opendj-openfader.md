# OpenDJ and OpenFadeR Feature Parity List
Status: generated from source reading by research agents; each item must be checked against the source before it is built.

This document enumerates every feature, quality-of-life detail, and capability of OpenDJ and OpenFadeR based on source code analysis, READMEs, changelogs, and documentation. Items are grouped by category. Each item cites its source file or URL.

## 1. Decks & Transport

| # | Feature | OpenDJ Source | OpenFadeR Source |
|---|---------|---------------|------------------|
| 1 | Two-deck layout | crates/app/ui/deck.slint (DeckInfo struct) | ui/stage/deck.slint (StageDeck) |
| 2 | Four-deck layout | crates/app/ui/fourdeck.slint | ui/stage/deck.slint (DeckState.count = 4) |
| 3 | 2-Deck Basic layout | crates/app/ui/layouts.slint (LayoutEntry) | ui/exportmode.slint (Simple player) |
| 4 | Player layout | crates/app/ui/layouts.slint | ui/exportmode.slint (1/2/DUAL PLAYER) |
| 5 | Prep layout | crates/app/ui/prep.slint | — |
| 6 | Play / Pause | crates/app/ui/deck.slint (ControlButton) | ui/deck-controls.slint (RingButton) |
| 7 | Cue button (press/hold preview) | crates/app/ui/deck.slint | ui/deck-controls.slint (RingColumn) |
| 8 | Tempo fader with range selector | crates/app/ui/deck.slint (ControlFader, tempo-range) | ui/deck-controls.slint (TempoColumn) |
| 9 | Tempo reset (double-click) | crates/app/ui/deck.slint | ui/deck-controls.slint (TouchArea double-clicked) |
| 10 | Key lock (master tempo) | crates/app/ui/deck.slint | ui/deck-controls.slint (Chip "Key lock") |
| 11 | Key shift (+-6 semitones, menu to +-6) | crates/app/ui/deck.slint (KeyItem, key-menu) | ui/deck-header.slint (SyncCluster key step) |
| 12 | Key sync (match other deck's key) | crates/app/ui/deck.slint | ui/deck-header.slint (Chip "KEY SYNC") |
| 13 | Beat sync (SYNC button) | crates/app/ui/deck.slint (SyncButton) | ui/deck-header.slint (Chip "SYNC") |
| 14 | Master tempo (MASTER button) | crates/app/ui/deck.slint | ui/deck-header.slint (Chip "MASTER") |
| 15 | Slip mode | crates/app/ui/deck.slint | ui/deck-controls.slint (Chip "SLIP") |
| 16 | Quantize | crates/app/ui/deck.slint | ui/deck-controls.slint (Chip "Quantize") |
| 17 | Reverse play | crates/app/ui/deck.slint | ui/deck-controls.slint (Chip "Reverse") |
| 18 | Brake effect | crates/app/ui/deck.slint | ui/deck-controls.slint (Chip "Brake") |
| 19 | Beat jump (1, 2, 4, 8, 16, 32, 64 beats) | crates/app/ui/deck.slint | ui/exportmode-parts.slint (Ops beat jump) |
| 20 | Loop controls (in/out, halve/double, move) | crates/app/ui/deck.slint (LoopBlock) | ui/deck-controls.slint (LoopBlock) |
| 21 | Saved loops (memory cues) | crates/app/ui/cues.slint (CueRow is-loop) | ui/exportmode-cues.slint (CueRow len) |
| 22 | Hot cues (8 per deck, colored pads) | crates/app/ui/deck.slint (pads) | ui/pads.slint (PadGrid mode 0) |
| 23 | Hot cue colors (8 colors, customizable) | crates/app/ui/theme.slint (hotcues array) | ui/deck-header.slint (StemButton tone) |
| 24 | Cue/loop list side panel | crates/app/ui/cues.slint (CuesPanel) | ui/exportmode-cues.slint (SubPanel) |
| 25 | Cue comments (editable) | crates/app/ui/cues.slint (NameEdit) | ui/exportmode-cues.slint (TextField comment) |
| 26 | Cue color picker (right-click menu) | crates/app/ui/cues.slint (CueNumber menu) | ui/exportmode-cues.slint (PopupWindow palette) |
| 27 | Delete cue mode (x button on pads) | crates/app/ui/deck.slint | ui/pads.slint (delete-mode) |
| 28 | Memory cue button | crates/app/ui/cues.slint (add-memory) | ui/exportmode-cues.slint (CueRow) |
| 29 | Save loop as memory cue | crates/app/ui/cues.slint (save-loop) | ui/exportmode-cues.slint (CueRow len click) |
| 30 | Phrase detection & display | crates/app/ui/deck.slint (PhraseTip) | ui/deck-wave.slint (phrase strip) |
| 31 | Phrase editing (cut, move, rename) | — | ui/exportmode-grid.slint (PhraseTools) |
| 32 | Beat grid editing (BPM, shift, x2/÷2, tap) | crates/app/ui/deck.slint (GridMenuItem) | ui/exportmode-grid.slint (GridTools) |
| 33 | Analysis Lock (prevent re-analysis) | — | ui/exportmode-grid.slint (IconButton lock) |
| 34 | Grid range (whole track / from here) | — | ui/exportmode-grid.slint (WHOLE / FROM HERE) |
| 35 | Grid undo/redo | — | ui/exportmode-grid.slint (UNDO/REDO) |
| 36 | Metronome (on/off, 3 volume levels) | — | ui/exportmode-grid.slint (METRO/VOL) |
| 37 | Jog wheel (touch-sensitive, scratch) | crates/app/ui/deck.slint | ui/deck-controls.slint (JogWheel) |
| 38 | Jog display modes (BPM, cue countdown, slip) | crates/app/ui/deck.slint | ui/deck-controls.slint (JogWheel jog-countdown) |
| 39 | Vinyl mode (jog platter) | — | ui/theme.slint (jog-style: platter) |
| 40 | Waveform overview (full track) | crates/app/ui/deck.slint (progress, duration) | ui/deck-wave.slint (DeckOverview) |
| 41 | Enlarged waveform (zoomed, scrolling) | crates/app/ui/deck.slint | ui/deck-wave.slint (WaveStrip) |
| 42 | Waveform zoom (mouse wheel, buttons) | crates/app/ui/deck.slint | ui/deck-wave.slint (DeckScroll strip-x) |
| 43 | Waveform color bands (low/mid/high) | crates/app/ui/theme.slint (wave-low/mid/high) | ui/theme.slint (wave-low/mid/high) |
| 44 | Waveform fade parameter | — | ui/theme.slint (wave-fade 0..1) |
| 45 | Stem separation (instant + HQ background) | crates/app/ui/deck.slint (stem-names, stem-status) | ui/stage/deck-parts.slint (StemRow) |
| 46 | Stem isolation buttons (Vocal, Inst, Drums, Bass) | crates/app/ui/deck.slint (StemRow) | ui/deck-header.slint (StemRow) |
| 47 | Stem HQ preparation indicator | crates/app/ui/deck.slint (stem-preparing) | — |
| 48 | Deck header (cover, title, artist, BPM, key) | crates/app/ui/deck.slint (DeckInfo) | ui/deck-header.slint (DeckTitle, DeckTimes) |
| 49 | Deck badge (number, hidden deck indicator) | crates/app/ui/deck.slint | ui/deck-wave.slint (DeckBadge) |
| 50 | Jacket/cover art click to load | crates/app/ui/deck.slint | ui/deck-header.slint (Jacket TouchArea) |
| 51 | Elapsed / remaining time display | crates/app/ui/deck.slint (DeckTimes) | ui/deck-header.slint (DeckTimes) |
| 52 | Track end warning (blink header, jog ring) | crates/app/ui/deck.slint (end-flash-time/ring) | — |
| 53 | Automix mix region highlight on waveform | crates/app/ui/deck.slint (automix-from/to) | — |
| 54 | File quality display (FLAC 16-bit, MP3 320kbps) | crates/app/ui/deck.slint (quality, lossless) | ui/exportmode-state.slint (file-type, bit-rate) |
| 55 | Sync rate display (when synced) | crates/app/ui/deck.slint | ui/deck-header.slint (sync-rate) |
| 56 | Key notation toggle (Camelot / Note) | crates/app/ui/theme.slint (KeyNotation) | — |
| 57 | Deck mirroring (right deck mirrored) | crates/app/ui/deck.slint (mirrored prop) | ui/theme.slint (stage-mirror) |
| 58 | Performance pads modes (12+ modes) | crates/app/ui/deck.slint | ui/pads.slint (mode 0-11+) |
| 59 | Pad FX mode (held pad = effect on) | crates/app/ui/deck.slint | ui/pads.slint (mode 4) |
| 60 | Pad FX color selector (5 colors) | — | ui/pads.slint (fx-color) |
| 61 | Slicer mode (8 slices, hold = roll) | crates/app/ui/deck.slint | ui/pads.slint (mode 7/8) |
| 62 | Sampler mode on pads (bank A/B) | crates/app/ui/deck.slint | ui/pads.slint (mode 9) |
| 63 | Sequencer Call mode on pads | — | ui/pads.slint (mode 10) |
| 64 | Stems mode on pads (4 parts) | crates/app/ui/deck.slint | ui/pads.slint (mode 11) |
| 65 | Plugin pad modes (custom per plugin) | crates/app/ui/deck.slint | ui/pads.slint (mode >=12) |

## 2. Mixer & Effects

| # | Feature | OpenDJ Source | OpenFadeR Source |
|---|---------|---------------|------------------|
| 66 | 3-band EQ (High, Mid, Low) per channel | crates/app/ui/mixer.slint | ui/mixer.slint (TouchChannel knobs 1-3) |
| 67 | EQ kills on controller only | crates/app/ui/mixer.slint (comment) | — |
| 68 | Gain/trim knob per channel | crates/app/ui/mixer.slint | ui/mixer.slint (TouchChannel knob 0) |
| 69 | Channel faders (vertical) | crates/app/ui/mixer.slint (ControlFader) | ui/mixer.slint (TouchChannel Fader) |
| 70 | Crossfader (curve: smooth/sharp/constant) | crates/app/ui/mixer.slint (Crossfader) | ui/mixer.slint (MixerState.curve) |
| 71 | Crossfader assign (A / Thru / B) | crates/app/ui/mixer.slint | ui/mixer.slint (TouchChannel Dropdown xf) |
| 72 | Cue/headphone button per channel | crates/app/ui/mixer.slint (CueHeadphones) | ui/mixer.slint (TouchChannel Chip CUE) |
| 73 | Stem isolation per channel (Vocal/Inst/Drums/Bass) | crates/app/ui/mixer.slint | ui/mixer.slint (TouchChannel Chip STEM ISO) |
| 74 | Color FX filter knob (per channel) | crates/app/ui/mixer.slint | ui/mixer.slint (knob 4 = CFX/Filter) |
| 75 | Master VU meter (stereo) | crates/app/ui/mixer.slint (StereoMeter) | ui/mixer.slint (Meter) |
| 76 | Booth output support | crates/app/ui/mixer.slint (booth prop) | ui/mixer.slint (MixerState.booth) |
| 77 | Master level knob | crates/app/ui/mixer.slint | ui/mixer-touch.slint (bus knob 0) |
| 78 | Headphone mix knob | crates/app/ui/mixer.slint | ui/mixer-touch.slint (bus knob 1) |
| 79 | Headphone level knob | crates/app/ui/mixer.slint | ui/mixer-touch.slint (bus knob 2) |
| 80 | Split cue (stereo headphone cue) | crates/app/ui/mixer.slint | ui/mixer-touch.slint (Chip Split cue) |
| 81 | Record button (master out recording) | crates/app/ui/mixer.slint | ui/mixer-touch.slint (Chip Record) |
| 82 | External mixer mode (hardware mixer) | crates/app/ui/mixer.slint (external prop) | — |
| 83 | Audio settings: sample rate, buffer size | crates/app/ui/mixer.slint (AudioSettings) | — |
| 84 | Output routing (master, headphones, booth, decks 1-4, sampler) | crates/app/ui/mixer.slint (OutputRow role) | ui/exportmode-parts.slint (CrossFader) |
| 85 | Input routing (mic, aux 1, aux 2) | crates/app/ui/mixer.slint (InputRow) | — |
| 86 | FX bar (3 effects per deck + Beat FX) | crates/app/ui/fxbar.slint | ui/fx.slint (FxPanel) |
| 87 | Effect on/off per slot | crates/app/ui/fxbar.slint (ControlButton) | ui/fx.slint |
| 88 | Effect selector (dropdown menu) | crates/app/ui/fxbar.slint (EffectMenu) | ui/fx.slint |
| 89 | Beat-synced effect length (1/16 to 4 beats) | crates/app/ui/fxbar.slint (BeatChooser) | ui/fx.slint |
| 90 | Effect parameter slider (2nd control) | crates/app/ui/fxbar.slint (ParamSlider) | ui/fx.slint |
| 91 | Effect dry/wet knob (meta) | crates/app/ui/fxbar.slint | ui/fx.slint |
| 92 | VST3 / CLAP / Audio Unit plugin hosting | crates/app/ui/fxbar.slint | ui/plugins.slint (GetPlugin native) |
| 93 | Plugin parameter editor (generic UI) | — | ui/plugins.slint |
| 94 | Effect rack per deck (3 slots) | crates/app/ui/fxbar.slint | ui/fx.slint |
| 95 | Global Beat FX (shared across decks) | crates/app/ui/fxbar.slint | — |

## 3. Sampler & Groove Box

| # | Feature | OpenDJ Source | OpenFadeR Source |
|---|---------|---------------|------------------|
| 96 | 16 sampler slots (2 banks of 8) | crates/app/ui/sampler.slint (SamplerPad) | ui/sampler.slint (GrooveDeckView 8 slots) |
| 97 | Sample packs (multiple, import/export .djsugarpack) | crates/app/ui/sampler.slint (SamplePack) | ui/groove.slint (GrooveState presets) |
| 98 | Choose Sample popover (djay Pro style) | crates/app/ui/sampler.slint | ui/groove.slint (DrumSlot) |
| 99 | Pad play modes (oneshot, hold, stutter, loop) | crates/app/ui/sampler.slint | ui/groove.slint (DrumSlotView state) |
| 100 | Sampler level knob per slot | crates/app/ui/sampler.slint | ui/groove.slint (master gain) |
| 101 | Drag & drop samples from library | crates/app/ui/sampler.slint (accepts data-transfer) | ui/groove.slint (slot-load) |
| 102 | Sample preview in headphones | crates/app/ui/sampler.slint (preview callback) | ui/groove.slint |
| 103 | Rename/clear sample slot | crates/app/ui/sampler.slint (rename/reset/clear) | ui/groove.slint (slot-clear) |
| 104 | Groove Box drum machine (Beat Lab) | crates/app/ui/instruments.slint | ui/groove.slint (GrooveState) |
| 105 | Drum Swap (8 slots per deck, 2 banks) | — | ui/groove.slint (DrumSlotView) |
| 106 | Multi mode (play multiple slots) | — | ui/groove.slint (GrooveDeckView multi) |
| 107 | Drum Swap Gain per deck | — | ui/groove.slint (GrooveDeckView master) |
| 108 | Drum Release FX (effect + beat) | — | ui/groove.slint (release-fx/beat) |
| 109 | Drum Capture (record loop to slot) | — | ui/groove.slint (capture-on/beats) |
| 110 | Drum presets (factory + user, save/load) | — | ui/groove.slint (GrooveState presets) |
| 111 | Instrument plugins on decks (CLAP/VST3/AU) | crates/app/ui/instruments.slint (LiveItem kind 2) | ui/stage/deck-performance.slint |
| 112 | Microphone input on deck | crates/app/ui/instruments.slint (LiveItem kind 3) | ui/stage/deck-performance.slint |
| 113 | Aux/Line input on deck | crates/app/ui/instruments.slint (LiveItem kind 4) | ui/stage/deck-performance.slint |
| 114 | Instrument presets (tags, step, save) | crates/app/ui/instruments.slint (PresetUi) | — |

## 4. Library & Collection

| # | Feature | OpenDJ Source | OpenFadeR Source |
|---|---------|---------------|------------------|
| 115 | Collection with stages (Inbox, In Prep, Play Ready) | crates/app/ui/library.slint (Folder stage) | ui/docs/library-format.md |
| 116 | Crates (colored, icon, editable) | crates/app/ui/library.slint (Folder kind 17) | ui/browser-palette.slint (PaletteColumn) |
| 117 | Playlists (standard, smart, history) | crates/app/ui/library.slint (Folder kind 2,3,5) | ui/browser.slint |
| 118 | Smart lists (live filters) | crates/app/ui/library.slint (Folder kind 3) | ui/browser.slint |
| 119 | Tag groups (custom tags) | crates/app/ui/library.slint (Folder kind 4) | — |
| 120 | Batch editing (multi-select) | crates/app/ui/library.slint | — |
| 121 | Forgiving search (fuzzy, typo-tolerant) | crates/app/ui/library.slint | ui/browser-suggest.slint |
| 122 | Filter bar (djay Pro style chips) | crates/app/ui/filters.slint (FilterUi) | ui/browser-palette.slint |
| 123 | Column picker (right-click header) | crates/app/ui/library.slint (ColumnUi) | ui/browser-table.slint |
| 124 | Column resize (drag edge) | crates/app/ui/library.slint | ui/browser-table.slint |
| 125 | Column reorder (drag heading) | crates/app/ui/library.slint | ui/browser-table.slint |
| 126 | Mini waveform per track row | crates/app/ui/library.slint (TrackRow wave) | ui/browser-table.slint |
| 127 | Album art thumbnail per row | crates/app/ui/library.slint (TrackRow cover) | ui/browser-table.slint |
| 128 | Track color coding (dot, strip, tint) | crates/app/ui/library.slint (color, color-mode) | ui/trackinfo.slint (color chip) |
| 129 | Rating (0-5 stars) | crates/app/ui/library.slint (rating) | ui/trackinfo.slint (5 stars) |
| 130 | Key match indicator (harmonic mixing) | crates/app/ui/library.slint (key-match, key-shift) | — |
| 131 | Missing file detection | crates/app/ui/library.slint (missing, fade) | ui/docs/diagnostics.md |
| 132 | Duplicate detection & merge | crates/app/ui/checklist.slint | — |
| 133 | Collection Health (missing, duplicates, poor tags) | crates/app/ui/checklist.slint | — |
| 134 | Imports: rekordbox, VirtualDJ, Traktor, Mixxx | crates/app/ui/library.slint | ui/docs/mappings.md |
| 135 | Import cues, grids, playlists | crates/app/ui/library.slint | ui/import-report.slint |
| 136 | Playlist import/export (M3U8, PLS, XSPF) | crates/app/ui/library.slint | ui/docs/library-format.md |
| 137 | Folder tree (collection, files, drives, cloud) | crates/app/ui/library.slint (Folder kind) | ui/browser-tree.slint |
| 138 | Pinned folders | crates/app/ui/library.slint (Folder kind 8) | — |
| 139 | History (sessions, daily) | crates/app/ui/library.slint (Folder kind 5, 19) | ui/browser.slint |
| 140 | Liked songs / Wanted list | crates/app/ui/library.slint (Folder kind 4) | — |
| 141 | Sidelist / Automix queue panel | crates/app/ui/library.slint | ui/exportmode-dock.slint |
| 142 | Palette columns (tag lists, playlists) | crates/app/ui/browser-palette.slint | ui/browser-palette.slint |
| 143 | Drag & drop tracks to crates/playlists | crates/app/ui/library.slint | ui/browser-palette.slint (DropArea) |
| 144 | Drag & drop reorder in playlist | crates/app/ui/library.slint | — |
| 145 | Analyze all (context menu on folder) | crates/app/ui/library.slint (analysable) | — |
| 146 | Relocate missing files | crates/app/ui/library.slint | — |

## 5. Streaming Sources & Stores

| # | Feature | OpenDJ Source | OpenFadeR Source |
|---|---------|---------------|------------------|
| 147 | Spotify browsing (tracks, playlists, albums) | crates/app/ui/sources.slint (SourceTab) | ui/catalog.slint (GetPlugin) |
| 148 | TIDAL browsing | crates/app/ui/sources.slint | — |
| 149 | Apple Music browsing | crates/app/ui/sources.slint | — |
| 150 | SoundCloud browsing | crates/app/ui/sources.slint | — |
| 151 | Mixcloud browsing | crates/app/ui/sources.slint | — |
| 152 | Plex / Navidrome / Jellyfin server playback | crates/app/ui/sources.slint | — |
| 153 | Saved searches per source | crates/app/ui/sources.slint (SearchItem) | — |
| 154 | Download from streaming to cache | crates/app/ui/sources.slint (FoundRow state) | — |
| 155 | Streaming track preview (before download) | crates/app/ui/sources.slint (preview) | — |
| 156 | Streaming services can't play on decks (browse only) | README.md line 50 | — |
| 157 | Store integration (Bandcamp, Beatport, etc.) | crates/app/ui/sources.slint (Folder kind 6) | — |

## 6. Auto Mix

| # | Feature | OpenDJ Source | OpenFadeR Source |
|---|---------|---------------|------------------|
| 158 | Automix through a list/playlist | crates/app/ui/deck.slint (automix-from/to) | — |
| 159 | Automix from phone remote | crates/app/ui/remote.slint | — |
| 160 | Mix point linking (MIX OUT / MIX IN) | — | ui/mixpoint.slint (MixPointPanel) |
| 161 | Phrase-aware mixing | crates/app/ui/deck.slint (PhraseTip) | ui/mixpoint.slint |
| 162 | Automix next track preview | crates/app/ui/deck.slint (automix-from/to) | — |

## 7. Recording & Broadcasting

| # | Feature | OpenDJ Source | OpenFadeR Source |
|---|---------|---------------|------------------|
| 163 | Record mix to file (WAV, FLAC, MP3) | crates/app/ui/mixer.slint | ui/mixer-touch.slint (Record chip) |
| 164 | Loudness meter (LUFS) during recording | README.md line 28 | — |
| 165 | Mixcloud export (tags, artwork) | README.md line 28 | — |
| 166 | Icecast / Shoutcast streaming | README.md line 28 | — |
| 167 | Now Playing overlay for OBS | README.md line 28 | — |
| 168 | Recording source selector (Master / Input) | — | ui/exportmode-state.slint (rec-source) |

## 8. Controllers & MIDI

| # | Feature | OpenDJ Source | OpenFadeR Source |
|---|---------|---------------|------------------|
| 169 | 200+ Mixxx controller mappings | README.md line 35 | ui/docs/mappings.md |
| 170 | 80+ native mappings with pictures | README.md line 36 | ui/docs/mappings.md |
| 171 | USB HID controllers (NI Traktor, Maschine) | README.md line 41 | — |
| 172 | Controller Editor (visual, wizard) | crates/app/ui/controller_editor.slint | ui/prefs-controller.slint |
| 173 | Mapping versioning (original / user copies) | crates/app/ui/controller_editor.slint | ui/docs/mappings.md |
| 174 | Soft takeover (prevent jumps) | crates/app/ui/controller_editor.slint | ui/docs/mappings.md line 15 |
| 175 | MIDI learn mode | — | ui/midi.slint |
| 176 | Controller plugins (Mixxx scripts) | README.md line 34 | ui/docs/plugins.md |
| 177 | Get mappings from GitHub repositories | — | ui/catalog.slint (GetMapping) |
| 178 | Auto-install mapping on controller connect | — | ui/docs/mappings.md line 33 |
| 179 | Invert hardware tempo fader | crates/app/ui/controller_editor.slint | ui/docs/mappings.md line 17 |
| 180 | Deck layer switching (DECK 3/4) | crates/app/ui/deck.slint | ui/stage/deck.slint (slots) |

## 9. UI Layouts & Workspaces

| # | Feature | OpenDJ Source | OpenFadeR Source |
|---|---------|---------------|------------------|
| 181 | 2-Deck layout | crates/app/ui/layouts.slint | ui/exportmode.slint |
| 182 | 2-Deck Basic layout | crates/app/ui/layouts.slint | ui/exportmode.slint (Simple) |
| 183 | 4-Deck layout | crates/app/ui/fourdeck.slint | ui/stage/deck.slint (count=4) |
| 184 | Player layout | crates/app/ui/layouts.slint | ui/exportmode.slint (1/2/DUAL PLAYER) |
| 185 | Prep layout | crates/app/ui/prep.slint | — |
| 186 | Layout menu (enable/disable, reorder) | crates/app/ui/layouts.slint (LayoutsUi) | ui/exportmode.slint (layout selector) |
| 187 | Desktop workspace (layout-profile 0) | — | ui/theme.slint (layout-profile) |
| 188 | Touch workspace (layout-profile 1) | — | ui/touch.slint (TouchWorkspace) |
| 189 | Stage workspace (layout-profile 2) | — | ui/stage/stage.slint |
| 190 | Stage deck styles (Performance, Turntable, Controller, Ring) | — | ui/theme.slint (stage-deck 0-3) |
| 191 | Stage mixer tabs (Mixer/FX/Rec, Mixer/Video/Lighting) | — | ui/theme.slint (stage-tabs) |
| 192 | Stage strip (Effects + Sampler) | — | ui/theme.slint (stage-strip) |
| 193 | Stage library sidebar (labels/icons) | — | ui/theme.slint (stage-sidebar) |
| 194 | Stage waveform position (top/centre/decks) | — | ui/theme.slint (stage-wave) |
| 195 | Mini player window (always-on-top) | crates/app/ui/mini.slint | — |
| 196 | Karaoke window (2nd display, fullscreen) | crates/app/ui/karaoke.slint | — |
| 197 | Video output window (projector, overlay) | — | ui/video.slint (VideoWindow) |
| 198 | Lighting mode (DMX/Art-Net/sACN) | — | ui/lighting.slint (LightingView) |

## 10. Visual & Theming

| # | Feature | OpenDJ Source | OpenFadeR Source |
|---|---------|---------------|------------------|
| 199 | Dark / Light / Auto (system) theme | crates/app/ui/layouts.slint (appearance) | ui/theme.slint (light) |
| 200 | Skin system (Catppuccin Mocha/Latte base) | crates/app/ui/theme.slint | ui/theme.slint |
| 201 | Skin tokens (colors, sizes, fonts, radii) | crates/app/ui/theme.slint | ui/theme.slint |
| 202 | Theme editor in Preferences | — | ui/skins.md |
| 203 | Get themes from GitHub repositories | — | ui/catalog.slint (GetTheme) |
| 204 | Custom theme files (JSON) | — | ui/docs/skins.md |
| 205 | Deck colors (4 distinct) | crates/app/ui/theme.slint (deck1-4) | ui/theme.slint (deck1-4) |
| 206 | Hot cue colors (8 colors) | crates/app/ui/theme.slint (hotcues) | ui/deck-header.slint |
| 207 | Stem colors (Vocal, Inst, Drums, Bass) | crates/app/ui/theme.slint (stem-*) | ui/theme.slint (stem-*) |
| 208 | Waveform colors (low/mid/high) | crates/app/ui/theme.slint (wave-*) | ui/theme.slint (wave-*) |
| 209 | Camelot key colors (12 hues) | crates/app/ui/theme.slint (key-wheel-color) | ui/theme.slint (key-sat/val) |
| 210 | Neon glow (0..1) on lit controls | — | ui/theme.slint (glow) |
| 211 | Bevel/raised shading (0..1) | — | ui/theme.slint (bevel) |
| 212 | Button style (filled/outlined/raised) | — | ui/theme.slint (button-style) |
| 213 | Font family + 9 size tokens | crates/app/ui/theme.slint | ui/theme.slint (font-xs..xxl) |
| 214 | Spacing tokens (xs..xl) | crates/app/ui/theme.slint | ui/theme.slint (space-*) |
| 215 | Radius tokens (radius, lg, xl) | crates/app/ui/theme.slint | ui/theme.slint (radius-*) |
| 216 | Control heights (bar, status, ctl, row) | crates/app/ui/theme.slint | ui/theme.slint (h-*) |
| 217 | Tooltip delay toggle | crates/app/ui/theme.slint (tooltips) | ui/theme.slint (tooltips) |
| 218 | Tempo decimal places (0/1/2) | crates/app/ui/theme.slint (tempo-decimals) | ui/theme.slint (tempo-decimals) |
| 219 | Deck number position on waveform | crates/app/ui/theme.slint (deck-number) | — |

## 11. Preferences & Settings

| # | Feature | OpenDJ Source | OpenFadeR Source |
|---|---------|---------------|------------------|
| 220 | Settings dialog (tabbed) | crates/app/ui/preferences.slint | ui/prefs-view.slint |
| 221 | Decks & Controllers settings | crates/app/ui/preferences.slint | ui/prefs-controller.slint |
| 222 | Audio settings (outputs, inputs, sample rate) | crates/app/ui/mixer.slint (AudioSettings) | — |
| 223 | Library settings (folders, stages, imports) | crates/app/ui/preferences.slint | — |
| 224 | Streaming sources settings | crates/app/ui/preferences.slint | — |
| 225 | Plugins settings (VST3/CLAP/AU, scan paths) | crates/app/ui/preferences.slint | ui/catalog.slint |
| 226 | Themes settings (editor, get themes) | crates/app/ui/preferences.slint | ui/catalog.slint |
| 227 | Extensions (Stems, Video, Lighting, Beat Lab) | crates/app/ui/preferences.slint | ui/catalog.slint |
| 228 | Appearance (theme, layout, waveforms) | crates/app/ui/preferences.slint | — |
| 229 | View settings (tooltips, tempo decimals, deck number) | crates/app/ui/theme.slint | ui/theme.slint |
| 230 | Controller editor in settings | crates/app/ui/controller_editor.slint | ui/prefs-controller.slint |
| 231 | MIDI mappings management | crates/app/ui/controller_editor.slint | ui/prefs-controller.slint |
| 232 | Keyboard shortcuts editor | — | ui/prefs-keyboard.slint |
| 233 | Backup/Export/Sync library | crates/app/ui/usb_export.slint | ui/device.slint (DeviceWindow) |
| 234 | USB export (Pioneer/Engine DJ formats) | crates/app/ui/usb_export.slint | ui/device-export.slint |
| 235 | Sync Manager (auto-sync playlists to device) | — | ui/device-export.slint (SyncPage) |
| 236 | Cue sync (device <-> collection) | — | ui/docs/backup-export-sync.md |
| 237 | Device backup/restore | — | ui/device-backup.slint |
| 238 | Collection backup (with/without music) | crates/app/ui/checklist.slint | ui/docs/backup-export-sync.md |

## 12. Advanced & Unique Features

| # | Feature | OpenDJ Source | OpenFadeR Source |
|---|---------|---------------|------------------|
| 239 | Ableton Link | README.md line 29 | — |
| 240 | GPU waveforms (fast, accurate) | README.md line 29 | ui/deck-wave.slint (WaveStrip pixelated) |
| 241 | Accurate beat grids & key detection | README.md line 29 | — |
| 242 | In-app tutorials / guided tour | crates/app/ui/tour.slint | — |
| 243 | Phone remote (QR code, PIN, web UI) | crates/app/ui/remote.slint | — |
| 244 | Lyrics (synced .lrc, LRCLIB lookup) | crates/app/ui/lyrics.slint | — |
| 245 | Lyrics on waveform (word-level timing) | crates/app/ui/lyrics.slint (WaveLyrics) | — |
| 246 | Time words (Whisper alignment) | crates/app/ui/lyrics.slint (can-time) | — |
| 247 | Transcribe (Whisper speech-to-text) | crates/app/ui/lyrics.slint (can-transcribe) | — |
| 248 | Karaoke screen (2nd display, word highlight) | crates/app/ui/karaoke.slint | — |
| 249 | Karaoke guide vocal level | crates/app/ui/lyrics.slint (vocals) | — |
| 250 | Karaoke web page for audience | crates/app/ui/lyrics.slint (page-url) | — |
| 251 | Video mode (crossfader transitions) | — | ui/video.slint (VideoState) |
| 252 | Video overlay (text/image, opacity, size) | — | ui/video.slint (overlay-mode) |
| 253 | Lighting mode (DMX fixtures, scenes) | — | ui/lighting.slint (LightingState) |
| 254 | Stage simulator (visualizer for lighting) | — | ui/lighting.slint (Stage) |
| 255 | Beat Lab / Groove Box (drum machine) | crates/app/ui/instruments.slint | ui/groove.slint |
| 256 | Instrument plugins on decks | crates/app/ui/instruments.slint | ui/stage/deck-performance.slint |
| 257 | Looper (in Instruments panel) | crates/app/ui/instruments.slint (kind 9) | — |
| 258 | Edit mode (timeline cut/copy/paste) | — | ui/edit.slint (EditView) |
| 259 | Export mode (player layouts for export) | — | ui/exportmode.slint |
| 260 | Mix Point Link (phrase-aware mix points) | — | ui/mixpoint.slint |
| 261 | Phrase Edit (cut, retype, move phrases) | — | ui/exportmode-grid.slint (PhraseTools) |
| 262 | Grid Edit (BPM, shift, tap, x2/÷2, range) | crates/app/ui/deck.slint | ui/exportmode-grid.slint (GridTools) |
| 263 | Analysis Lock | — | ui/exportmode-grid.slint (lock icon) |
| 264 | Metronome (grid-aligned click) | — | ui/exportmode-grid.slint (METRO) |
| 265 | Resource monitor (RAM/VRAM chips) | — | ui/resources.slint |
| 266 | Diagnostics report | — | ui/docs/diagnostics.md |
| 267 | In-app update (with rollback) | — | src/main.rs (roll_back) |
| 268 | Stick verify (verify USB export) | — | src/main.rs (verify) |
| 269 | Disk formatter (Windows elevated) | — | src/main.rs (--format-disk) |
| 270 | Low memory malloc tuning (Linux) | — | src/main.rs (low_memory_malloc) |
| 271 | Crash reports & log collection | — | src/main.rs (diag::init) |

---

## Summary Statistics

| Category | OpenDJ Items | OpenFadeR Items | Both |
|----------|--------------|-----------------|------|
| Decks & Transport | 65 | 60 | 52 |
| Mixer & Effects | 29 | 25 | 20 |
| Sampler & Groove Box | 19 | 20 | 12 |
| Library & Collection | 32 | 20 | 15 |
| Streaming Sources | 11 | 1 | 1 |
| Auto Mix | 3 | 4 | 2 |
| Recording & Broadcasting | 6 | 1 | 1 |
| Controllers & MIDI | 12 | 12 | 8 |
| UI Layouts & Workspaces | 18 | 20 | 8 |
| Visual & Theming | 21 | 21 | 16 |
| Preferences & Settings | 19 | 20 | 12 |
| Advanced & Unique | 33 | 30 | 10 |
| **Total Unique Features** | **268** | **234** | **157** |

**OpenDJ-only features (not in OpenFadeR):** 111
**OpenFadeR-only features (not in OpenDJ):** 77
**Common features:** 157

---

## 25 Most Valuable Features (for DJ Sugar parity)

1. **Four-deck layout with mirrored right deck** — core professional workflow
2. **Stem separation (instant + HQ) with isolation buttons** — modern DJ essential
3. **FX bar with beat-synced lengths + plugin hosting (VST3/CLAP/AU)** — creative mixing
4. **Groove Box / Beat Lab drum machine with Drum Swap/Capture** — performance feature
5. **Auto Mix with phrase-aware mixing & phone remote** — hands-free sets
6. **Comprehensive controller support (200+ Mixxx + 80 native + HID)** — hardware compatibility
7. **Library with stages, crates, smart lists, tag groups, batch edit** — collection management
8. **Collection Health (missing, duplicates, poor tags)** — library maintenance
9. **Imports from rekordbox, VirtualDJ, Traktor, Mixxx + cues/grids** — migration path
10. **Streaming source browsing (Spotify, TIDAL, Apple Music, SoundCloud, Mixcloud)** — track discovery
11. **Plex/Navidrome/Jellyfin self-hosted streaming** — personal library access
11. **Recording + loudness meter + Icecast/Shoutcast + OBS overlay** — broadcast/recording
12. **Ableton Link** — sync with other apps/hardware
13. **GPU waveforms with 3-band color + zoom + overview** — visual beatmatching
14. **Beat grid editing (tap, shift, x2/÷2, range, undo/redo)** — grid correction
15. **Phrase detection + editing (cut, move, rename)** — structural mixing
16. **Mix Point Link (MIX OUT/IN with waveform alignment)** — precise transitions
17. **Sampler (16 slots, packs, drag-drop, preview)** — live remixing
18. **Video mode with crossfader transitions + overlay + 2nd window** — VJing
19. **Lighting mode (DMX/Art-Net/sACN) with stage simulator** — visual performance
20. **Karaoke (synced lyrics, 2nd screen, guide vocal, web audience)** — karaoke gigs
21. **Lyrics on waveform (word-level, LRCLIB, Whisper time/transcribe)** — vocal mixing
22. **Touch workspace (adaptive, 44px targets, gesture-friendly)** — tablet/touchscreen
23. **Stage workspace (4 deck styles, mixer tabs, strip, waveform positions)** — customizable layout
24. **Skin/theme system (tokens, editor, GitHub repos, Catppuccin base)** — personalization
25. **USB export (Pioneer/Engine DJ) + Sync Manager + Cue sync + Device backup** — hardware integration

---

## Sources

- OpenDJ: `/home/ubuntu/djsugar-rust/crates/app/ui/*.slint`, `/home/ubuntu/djsugar-rust/docs/release-notes.md`, `/home/ubuntu/djsugar-rust/openspec/roadmap.md`, `https://github.com/openwave-studio/opendj`
- OpenFadeR: `/home/ubuntu/.hermes/cache/scratch/parity-src/openfader/ui/*.slint`, `/home/ubuntu/.hermes/cache/scratch/parity-src/openfader/docs/*.md`, `https://github.com/OpenRsG/OpenFadeR`
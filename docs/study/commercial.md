# Commercial DJ App UI/UX & Feature Study

**Target**: Build the best DJ app from scratch in Rust for Android (heavy Samsung DeX use, S23 Ultra), iOS/iPadOS, Linux, Windows, macOS — fully resizable at any aspect ratio, OLED-friendly (true black).

**Sources**: Official manuals, release notes, feature pages, and support docs for Serato DJ Pro, rekordbox, VirtualDJ, Traktor Pro, djay Pro (incl. iPad/iPhone/Android/DeX), Engine DJ. All URLs cited inline.

---

## 1. Serato DJ Pro

### 1.1 Screen Layout per Mode
| Mode | Layout |
|------|--------|
| **Vertical** | Virtual decks left/right; vertical waveforms center [^serato-display] |
| **Horizontal** | Virtual decks left/right; horizontal waveforms center [^serato-display] |
| **Extended** | Minimized deck info; horizontal waveforms full-width [^serato-display] |
| **Stack** | Decks stacked left; waveforms right; extra tab for simplified deck / Cue Points / Loops (requires primary hardware) [^serato-display] |
| **Library** | Maximized browser; minimized deck info (Spacebar shortcut with hardware) [^serato-display] |
| **2/4 Deck Toggle** | Available in all modes; active decks highlighted with blue border [^serato-display] |

### 1.2 Deck Anatomy
- **Waveform Display**: Color-coded — red=bass (low), green=mid, blue=treble (high) [^serato-waveform]
- **Stripe/Overview**: Full-track waveform with cue points, loops, playhead
- **Transport**: Play, Cue, Sync, tempo fader, key lock, pitch bend
- **Deck Header**: Track title, remaining time, BPM, cover art (in larger layouts)

### 1.3 Waveform Styles
- **RGB 3-band**: Fixed red/green/blue mapping to frequency bands [^serato-waveform]
- **Orientation**: Vertical (default) or horizontal per display mode
- **Beat Grid Markers**: White lines on waveform; downbeats brighter
- **Overview/Stripe**: Full-track miniature waveform below or beside main waveform
- **Zoom**: +/- keys; Shift+click in Extended mode toggles small/large

### 1.4 Pad Modes
- **Performance Pads** (hardware-mapped): Hot Cue, Roll, Slicer, Sampler, Loop, Pitch Play
- **Software pads**: Mirror hardware; limited customization without controller

### 1.5 FX UI
- **FX Panel**: 2 FX units (multi-effects + single effects)
- **Mixer FX**: 1 per channel (filter, noise, gate, etc.)
- **Assignment**: Drag-and-drop FX to deck slots

### 1.6 Library / Browse UX
- **Crates/Playlists**: Left sidebar tree
- **Track List**: Column browser (BPM, Key, Genre, etc.)
- **Smart Crates**: Auto-populate by rules
- **Search**: Top-bar; filters by field
- **History**: Auto-logged sessions

### 1.7 Stems
- **Stems Separation**: Neural/Algoriddim-style AI separation (Serato Stems) [^serato-stems-prep]
- **Deck Control**: Load stem file → auto-switches to Stem Deck; per-stem volume, mute, filter, FX

### 1.8 Lyrics / Karaoke
- **Not natively supported** (no lyrics overlay or karaoke mode in current versions)

### 1.9 Streaming Integration
- **Supported**: TIDAL, SoundCloud Go+, Beatport/Beatsource Streaming, Beatsource
- **Workflow**: Login in browser panel; stream tracks directly to decks; offline cache limited

### 1.10 Touch Gestures
- **Limited**: Waveform scrub (click-drag), pinch-zoom on touchscreens (Windows)
- **No dedicated phone/tablet UI** — desktop-first

### 1.11 Signature Features
- **DVS**: Industry-standard timecode vinyl control
- **Serato Play**: Controller-free practice mode (2 decks, keyboard/mouse)
- **Hardware Lock-in**: Best experience with Serato-licensed controllers
- **Stability**: Rock-solid audio engine, low latency

---

## 2. rekordbox (AlphaTheta / Pioneer DJ)

### 2.1 Screen Layout per Mode
| Mode | Layout |
|------|--------|
| **Export Mode** | Library-focused; 5 layout presets (2/4 deck, vertical/horizontal waveforms) [^rekordbox-style] |
| **Performance Mode** | Same 5 layouts; adds decks, mixer, FX, sampler, performance pads |
| **Mobile (iOS/Android)** | Condensed 2-deck; vertical waveforms; touch-optimized browser [^rekordbox-mobile] |
| **CloudDirectPlay** | Access cloud library directly from CDJ/XDJ hardware [^rekordbox-overview] |

### 2.2 Deck Anatomy
- **Waveform**: RGB 3-band (blue=low, green=mid, white=high) or multi-color modes
- **Overview**: Full-track stripe with cue/loop markers
- **Deck Header**: Cover art, title, artist, BPM, key, remaining time
- **Performance Pads**: 8 pads × 4 banks (Hot Cue, Pad FX1/2, Beat Loop, Slicer, etc.)
- **Mixer Section**: 3-band EQ, filter, fader, cue/meter

### 2.3 Waveform Styles
- **Color Modes**: Standard (blue/green/white), 3-band RGB, multi-color
- **Orientation**: Vertical (default) or horizontal; per-deck or stacked parallel
- **Beat Grid**: White lines; downbeats brighter; phrase markers (16-bar) [^rekordbox-overview]
- **Vocal Position Detection**: AI-detected vocal regions shown on overview waveform [^rekordbox-overview]
- **Zoom**: Continuous zoom slider; waveform scales with pitch

### 2.4 Pad Modes
| Page | Function |
|------|----------|
| Hot Cue | 8 cues + 4 banks (32 total) |
| Pad FX 1/2 | Momentary/latching FX per pad |
| Beat Loop | Auto-loops at 1/4, 1/2, 1, 2, 4, 8, 16, 32 beats |
| Slicer | 8 slices across beat grid |
| Sampler | Trigger sampler slots |
| Keyboard | Pitch-play slices chromatically |
| Key Shift | Semitone transpose per pad |
| Beat Jump | Jump ±1, 2, 4, 8, 16, 32 beats |

### 2.5 FX UI
- **Mixer FX** (per channel): 9 types (Filter, Noise, Gate, etc.) — 1 knob control
- **Software FX Units**: 2 units × 3 slots (43 effects total) — X/Y pad + parameters
- **RMX Effects** (Performance plan): Scene-based performance effects (Roll, Echo, Vinyl Brake, etc.)

### 2.6 Library / Browse UX
- **Collection Filter**: Multi-column filter (date, genre, artist, album, BPM, key, label) [^rekordbox-overview]
- **Sub Browser**: Dual collection panes side-by-side [^rekordbox-overview]
- **Column View**: Finder-style hierarchical playlist/folder navigation [^rekordbox-overview]
- **Media Browser**: Icon-grid view for streaming services (artist/album artwork focus) [^rekordbox-overview]
- **Intelligent Playlist**: Auto-populate by genre/BPM/key/artist/comment rules [^rekordbox-overview]
- **Collaborative Playlists**: Share with metadata (cues, grids, order) [^rekordbox-overview]
- **Cloud Library Sync**: Dropbox/Google Drive (5 TB on Professional plan) [^rekordbox-overview]

### 2.7 Stems
- **Stems Separation**: AI-powered (Neural Mix style) — Vocal, Drums, Bass, Other [^rekordbox-overview]
- **Deck Control**: Per-stem volume, mute, EQ kill, FX send
- **STEM Playlist**: Auto-generated in sidebar

### 2.8 Lyrics / Karaoke
- **Not natively supported** (no lyrics display or karaoke mode)

### 2.9 Streaming Integration
- **Services**: Beatport, Beatsource, TIDAL, SoundCloud, (Apple Music via CloudDirectPlay)
- **Streaming Radar**: Discover similar tracks from streaming catalog [^rekordbox-overview]
- **Offline Cache**: Limited; requires subscription per service

### 2.10 Touch Gestures
- **Mobile App**: Pinch-zoom waveform, swipe deck load, drag-drop playlist, tap-hold track info
- **Desktop**: Mouse-centric; touch support basic

### 2.11 Signature Features
- **Hardware Integration**: Native CDJ/XDJ HID/Pro DJ Link — waveforms, cues, loops sync to hardware screens
- **Dual Player (Export Mode)**: Linked 2-deck preview with EQ Kill, Key Sync, Beat Jump [^rekordbox-overview]
- **Cloud Analysis**: 26% faster analysis via cloud [^rekordbox-overview]
- **Intelligent Cue Creation**: Learns your cue patterns, applies to new tracks [^rekordbox-overview]
- **Mix Point Link**: Pre-set transition points for hands-free mixing [^rekordbox-overview]

---

## 3. VirtualDJ

### 3.1 Screen Layout per Mode
| Layout | Description |
|--------|-------------|
| **Starter** | Minimal: 2 decks, rhythm wave, basic mixer |
| **Essentials** | 2/4 decks, rhythm wave, pads (button style), browser sideview |
| **Pro** | 2/4 decks, rhythm wave, pads (dropdown), full mixer, FX, sampler, sideview tabs |
| **Performance** | Pro + video mixer, karaoke, broadcast, advanced pads |
| **Custom Skins** | Fully skinnable; community skins for any layout [^vdj-waveform] |

### 3.2 Deck Anatomy
- **Rhythm Wave** (top): Multi-deck parallel waveforms with CBG (Computerized Beat Grid) [^vdj-rhythmwave]
- **Deck Waveforms** (per deck): Vertical; 5 color modes (Monochrome, Neutral, Ultra-Blue, Infrared, Per deck) [^vdj-waveform]
- **Scratch Waveforms** (scratch mixer): Vertical, zoomable, pinpoint scratch zones [^vdj-waveform]
- **Track Info**: Title, artist, BPM, key, cover art, time elapsed/remaining
- **Pads**: 8/16 pads per deck; page-based (see 3.4)
- **Mixer**: 3-band EQ, filter, gain, fader, cue, meter, crossfader assign

### 3.3 Waveform Styles
| Style | Description |
|-------|-------------|
| **Beats** | White peaks on dark; CBG blocks below |
| **Colors** | 3-band RGB (red=bass, green=mid, blue=treble) |
| **Shapes** (Default) | Frequency-shaped waveform; greys out muted stems when using Stem Isolation [^vdj-waveform] |
| **Show Gridlines** | Toggle CBG lines on/off |
| **Phrase Indicator** | 16-beat phrase counter (Pro layout option) [^vdj-rhythmwave] |
| **Beat Keeper** | 4-beat bar position indicator [^vdj-rhythmwave] |
| **Lyrics Overlay** | Synced lyrics on waveform (Stems 2.0) [^vdj-rhythmwave] |

- **Orientation**: Rhythm wave = horizontal; deck/scratch waveforms = vertical
- **Stacked Parallel**: Rhythm wave shows all decks stacked; deck waveforms side-by-side in 4-deck
- **Zoom**: Continuous slider per waveform

### 3.4 Pad Modes (Pages)
| Page | Function |
|------|----------|
| **Hot Cues** | 8 slots × unlimited banks; Smart Cue, Quantize, Lock, Play/Stutter modes |
| **STEMS** | 5 stems (Vocal, Drums, Bass, Piano, Other) — mute/solo, per-stem FX [^vdj-stems] |
| **Sampler** | 16 slots × banks; sync, trigger, slice |
| **KeyCue** | Hot cue + key shift combined |
| **Saved Loops** | Stored loops per track |
| **Manual Loop** | Live loop in/out adjust via pads/jog; Smart Loop, Loop Back, Quantize |
| **Remix Points** | 8 remix sections per track (POI editor) |
| **DMX** | OS2L lighting control |
| **Scratch Bank** | Saved scratch samples |
| **Custom** | User-scripted buttons (VDJScript) |

- **16 Pad Mode**: GUI-only 16-pad grid (View → Pads 16) [^vdj-pads]
- **Pad Editor**: Full customization of pages, parameters, colors, scripts

### 3.5 FX UI
- **FX Rack**: 2–4 units × 3 slots; 80+ native effects
- **FX Panels**: Per-deck XY pad + parameter sliders
- **Mixer FX**: Quick filter/noise/gate per channel
- **Video FX**: Separate video effect rack (Visuals, Transitions, Titles)
- **Touch FX**: Swipe gestures on touchscreens (Pro/Performance layouts)

### 3.6 Library / Browse UX
- **SideView Tabs**: Sidelist, Remixes, Sampler, Automix, **Karaoke**, Clone [^vdj-karaoke]
- **Folder List**: Tree + favorites
- **File List**: Columns (BPM, Key, Length, Genre, Rank, etc.)
- **Smart Folders**: Auto-playlists by query
- **Online Music**: Beatport, Beatsource, SoundCloud, Deezer, TIDAL, etc. [^vdj-browser]
- **CloudDrive**: Dropbox, Google Drive, OneDrive integration
- **GeniusDJ / Ask The DJ / Charts**: AI-powered recommendations
- **OneLibrary**: Sync cues/grids across devices
- **Stems Browser**: Filter by stem availability
- **AI Prompt**: Natural-language search ("uplifting vocal house 126 bpm")

### 3.7 Stems (Stems 2.0)
- **Real-time Separation**: 5 stems (Vocal, Drums, Bass, Piano, Other) — CPU/GPU accelerated [^vdj-stems]
- **PAD Control**: Mute/solo per stem; Stems FX Pad for per-stem effects
- **EQ Modes**: EZRemix (3-knob vocal/instrument/beat), Stems (5-knob full control) [^vdj-stems]
- **Karaoke**: Mute Vocal stem + Karaoke VideoFX = instant karaoke [^vdj-karaoke]

### 3.8 Lyrics / Karaoke
- **Lyrics Editor**: Full LRC/synced lyrics editing [^vdj-browser]
- **Lyrics Overlay**: On rhythm wave (Stems 2.0) [^vdj-rhythmwave]
- **Karaoke VideoFX**: Displays lyrics on video output with guide vocal [^vdj-karaoke]
- **Clone View**: Secondary monitor for singer lyrics

### 3.9 Streaming Integration
- **Services**: Beatport, Beatsource, SoundCloud, Deezer, TIDAL, Qobuz, etc.
- **Alternate Catalogs**: Multiple accounts per service
- **CloudDrive**: Stream from cloud storage
- **Content Subscriptions**: In-app subscription management

### 3.10 Touch Gestures
- **Touch FX**: Swipe any direction for build-ups/drops (Performance layout)
- **Waveform**: Pinch-zoom, drag-scrub, tap-to-cue
- **Pads**: Multi-touch pad triggering
- **Browser**: Drag-drop, swipe navigation
- **VirtualDJ Remote**: Phone/tablet as wireless controller (dedicated apps) [^vdj-remote]

### 3.11 Signature Features
- **Stems 2.0 + Lyrics + Karaoke**: Unique all-in-one package
- **VDJScript**: Deep scripting for custom mappings/skins
- **Skins**: Complete UI replacement (hundreds of community skins)
- **Video Mixing**: Full video deck, transitions, visuals, karaoke
- **Broadcast/ShoutOut**: Built-in streaming to Icecast/Shoutcast
- **Event Scheduler**: Automated playlist/timer events
- **Sandbox Mode**: Safe experimentation without affecting live decks

---

## 4. Traktor Pro (Native Instruments)

### 4.1 Screen Layout per Mode (Traktor Layouts)
| Layout | Purpose |
|--------|---------|
| **Essential** | Basic mixing: waveforms, stripes, transport; no internal mixer (for external controller) [^traktor-setup] |
| **Extended** | Hotcue + effect panels; mixer with panorama, Mixer FX, FX Units [^traktor-setup] |
| **Browser** | Maximized browser; minimized decks; no mixer [^traktor-setup] |
| **Mixer** | Full mixer view; decks minimized |
| **Parallel** | **Stacked waveforms above decks** full-width; parallel beatgrids for visual sync; works 2/4 deck [^traktor-setup] |
| **Preparation** | Single deck (A) full-width + maximized browser; precise cue/grid editing [^traktor-setup] |
| **Custom Layouts** | User-saved combinations of panels |

### 4.2 Deck Anatomy (Deck Layouts — 7 levels)
| Layout | Elements |
|--------|----------|
| **Micro** | Stripe, Play, header (title, time, BPM) |
| **Small** | + Transport, Key Lock, pitch bend, tempo fader, phase meter, SYNC/MASTER, header+cover |
| **Essential** | + Waveform, Stripe |
| **Full** | + Full header (all track info) |
| **Advanced** | + Advanced Panel (Hotcues, Loops, Flux, Macro) |
| **Parallel Slim** | Stacked waveforms A/B above deck; stripe, transport, minimal header |
| **Parallel Full** | Stacked waveforms A/B + Advanced Panel + full header |

- **Switch**: Double-click deck header cycles layouts [^traktor-setup]
- **Deck Types**: Track Deck, **Stem Deck**, Remix Deck, Live Input [^traktor-overview]

### 4.3 Waveform Styles
- **Color Modes (4)**: Ultraviolet (default), Infrared, X-Ray, Spectrum [^traktor-pro4]
- **Waveform Display**: Zoomed view at playhead; brighter=high freq, darker=low freq [^traktor-overview]
- **Stripe**: Full-track overview with cues, loops, beatgrid
- **Beat Grid Markers**: Thin lines (beats), bright + top marker (downbeats), strongest (phrase/16-bar) [^traktor-overview]
- **Parallel Waveforms**: Stacked A/B (or A/B/C/D) above decks; full window width [^traktor-setup]
- **Multi-Track Waveform (Stem Deck)**: 4–5 stem waveforms in distinct colors, shared beatgrid, per-stem volume/FX [^traktor-overview]

### 4.4 Pad Modes
- **Performance Pads** (hardware: X1, S4, S8, F1): 8 pads × 4 pages
- **Pages**: Hotcue, Loop, Slicer, Sampler, Beat Jump, Key Shift, Pattern Player, Fader FX
- **Remix Decks**: 4×4 slot grid (64 cells) — one-shot/loop samples, per-cell FX, quantization
- **Pattern Player**: Step sequencer (16 steps × 4 tracks) per deck [^traktor-pro4]

### 4.5 FX UI
- **FX Units**: 4 units × 2 modes
  - **Group Mode**: 3 FX chained per unit (X/Y macro control)
  - **Single Mode**: 1 FX per unit, full parameter control
- **Mixer FX**: 9 per channel (Filter, Delay, Reverb, etc.) — 1-knob macro
- **Assignment**: Any FX Unit → any Deck/Channel/Bus
- **Pattern Player Units**: 4 units × 17 kits (Pro 4) [^traktor-pro4]

### 4.6 Library / Browse UX
- **Track Collection**: Flat list + playlists (folders)
- **Smart Playlists**: Rule-based auto-playlists
- **Playlist Favorites**: Pinned playlists
- **History Playlists**: Auto-session logs
- **Preview Deck**: Dedicated preview player (headphone cue without loading to deck)
- **Beatport/Beatsource Streaming**: Integrated preview + streaming [^traktor-pro4]
- **Flexible Beatgrids**: Multiple tempo anchors per track [^traktor-pro4]
- **Key/Phrase Analysis**: Alphanumeric + Classic key; phrase markers

### 4.7 Stems
- **Stem Deck**: Native .stem.mp4 playback; auto-switch on load [^traktor-overview]
- **Per-Stem Control**: Volume, filter, FX amount, mute/solo [^traktor-pro4]
- **Stem Separation**: Not built-in (requires pre-separated files); third-party tools

### 4.8 Lyrics / Karaoke
- **Not natively supported**

### 4.9 Streaming Integration
- **Beatport Streaming**: Full catalog, preview, streaming to decks [^traktor-pro4]
- **Beatsource Streaming**: Open-format focused
- **Beatport Previews**: 30-sec preview in browser

### 4.10 Touch Gestures
- **Limited**: Waveform scrub, pinch-zoom (Windows touch)
- **No dedicated tablet/phone UI**

### 4.11 Signature Features
- **Parallel Layout**: Stacked waveforms for visual beatmatching without sync
- **Flexible Beatgrids**: Handles variable-tempo tracks (live, classical, etc.)
- **Remix Decks**: Unique 64-cell performance sampler/looper
- **Pattern Player**: Step sequencing integrated into decks
- **Stem Deck**: Native multi-track stem playback with per-stem FX
- **HID/CDJ Support**: Native Pioneer CDJ control (no timecode needed)
- **Ozone Maximizer**: Master bus limiting (iZotope)

---

## 5. djay Pro (Algoriddim) — incl. iPad, iPhone, Android, Samsung DeX

### 5.1 Screen Layout per Mode
| Platform / View | Layouts |
|-----------------|---------|
| **Desktop (Mac/Win)** | 2 Deck, 4 Deck, Sampler, Expanded Library — horizontal/vertical waveforms [^djay-windows] |
| **iPad** | Starter, Automix, Classic (turntables), Pro (jog wheels + waveforms), Looper, One Deck, **4 Decks**, Video [^djay-ios-decks] |
| **iPhone** | Starter, Automix, Classic, Pro, Looper, One Deck — adaptive portrait/landscape [^djay-ios-decks] |
| **Android** | Same view modes as iOS; Beatport/Beatsource streaming added [^djay-releasenotes] |
| **Samsung DeX** | Desktop-class layouts on external monitor; multi-window, keyboard/mouse/touch; 4-deck + library side-by-side |

### 5.2 Deck Anatomy
- **Classic**: Virtual turntables (vinyl look), crossfader, basic mixer
- **Pro**: Jog wheels, scrolling waveforms (horizontal/vertical), detailed track info, performance tools [^djay-ios-decks]
- **4 Decks**: 4 jog wheels + waveforms; crossfader assignments (L/R/Thru) [^djay-windows]
- **Looper**: Grid-based looper (48 slots) + sampler [^djay-ios-decks]
- **One Deck**: Full-screen waveform, library, cue/loop/grid editing [^djay-ios-decks]
- **Video**: Video decks + audio-reactive visual loops [^djay-ios-decks]
- **Track Info**: Cover art, title, artist, BPM, key, time, waveform overview stripe

### 5.3 Waveform Styles
- **4-Band Color Coding**: Red=bass, Yellow=low-mid, Green=high-mid, Blue=treble [^djay-ipad-manual]
- **Orientation**: Horizontal (parallel, scroll R→L) or Vertical (parallel, scroll top→bottom) — switchable live [^djay-ios-decks]
- **Layouts**: 2-deck / 4-deck / single-deck; variable zoom [^djay-windows]
- **Overview Stripe**: Full-track with cue points (colored) + red playhead [^djay-ipad-manual]
- **Neural Mix Waveforms**: Per-stem waveforms in deck (Vocal, Drums, Harmonic) [^djay-windows]

### 5.4 Pad Modes
- **Performance Pads** (Pro/4 Deck/Looper): 8 pads × pages
- **Pages**: Hot Cue, Loop, Sampler, Slicer, Beat Jump, Key Shift, Neural Mix (stems)
- **Looper Grid**: 48 slots (6×8) — quantized, time-stretched, beat-synced [^djay-ios-decks]
- **XY Pad**: Crossfader Fusion / Instant FX control

### 5.5 FX UI
- **Deck FX**: Up to 3 chainable FX per deck; XY Pad + Instant FX [^djay-windows]
- **Mixer FX**: Filter, Gate, Noise, etc. per channel
- **Crossfader Fusion™**: Morph tracks by deconstructing in real-time with intelligent filter/FX [^djay-ios]
- **30+ Sugar Bytes FX** (IAP): Advanced effects rack

### 5.6 Library / Browse UX
- **Unified Media Library**: Local + Spotify + Apple Music + TIDAL + SoundCloud + Beatport + Beatsource [^djay-windows]
- **Playlist Tree + Track List**: Standard two-pane browser
- **Smart Playlists**: Auto-rules
- **Automix Queue**: Drag-drop reorder
- **Column View** (Mac): Finder-style hierarchy
- **Search**: Unified across all sources; filters
- **iCloud Sync**: Cues, grids, metadata, FX purchases sync across iOS/Mac [^djay-ipad-press]

### 5.7 Stems (Neural Mix™)
- **Real-time AI Separation**: Vocals, Drums, Harmonic (3 stems) — on-device (Core ML / Neural Engine) [^djay-windows]
- **Deck Control**: Isolate/mute/solo per stem; crossfader morphs stems
- **EQ Integration**: Neural Mix EQ mode (3-knob stem balance)
- **Pad Page**: Neural Mix pads for live stem triggering

### 5.8 Lyrics / Karaoke
- **Not natively supported** (no lyrics overlay or karaoke mode)

### 5.9 Streaming Integration
| Service | Platforms |
|---------|-----------|
| Spotify | All (Premium required) |
| Apple Music | All (100M+ catalog) [^djay-windows] |
| TIDAL | All |
| SoundCloud | All (Go+ for full catalog) |
| Beatport / Beatsource | Android, Mac, Win, iOS [^djay-releasenotes] |
- **Offline Cache**: Limited; streaming-only tracks marked
- **Unified Search**: Single query across all sources

### 5.10 Touch Gestures
- **Multi-touch Scratching/Seeking**: Two fingers on waveform/jog → scratch, seek, hold [^djay-releasenotes]
- **Multi-touch FX/Pad**: Two fingers on pad → pressure/position control [^djay-releasenotes]
- **Pinch-zoom Waveform**: Standard
- **Swipe Load**: Swipe track from browser to deck
- **Split View / Slide Over** (iPad): djay + another app side-by-side [^djay-ipad-press]
- **Smart Keyboard / Trackpad**: 70+ shortcuts; trackpad gestures mapped [^djay-ipad-press]
- **DeX**: Mouse/keyboard + touch on external monitor; resizable windows

### 5.11 Signature Features
- **Neural Mix™**: Real-time 3-stem AI separation on mobile/desktop (no cloud)
- **Crossfader Fusion™**: Intelligent stem-aware crossfading
- **Fluid Beatgrid™**: Advanced beat-matching across tempo/genre changes
- **Automix AI**: Smart transitions with phrase/key awareness
- **Video Mixing + Visual Loops**: Audio-reactive visuals, Live Photos
- **DVS (AI-powered)**: First AI timecode system [^djay-windows]
- **Cross-platform Sync**: iCloud cues/grids/metadata across iOS/Mac/Android/Win
- **DeX Ready**: Desktop-class on Samsung S23 Ultra + monitor

---

## 6. Engine DJ (Denon DJ / Numark / Rane — Engine OS)

### 6.1 Screen Layout per Mode
| Context | Layouts |
|---------|---------|
| **Desktop (Engine DJ Software)** | Library + 2-deck performance view; Layout Manager (3 saved views) [^engine-4-release] |
| **Standalone Hardware (Prime 4/4+, Prime 2, Prime Go, SC Live, SC5000/6000)** | **5 View Presets (View 1–5)** — one-tap recall; survives reboot [^engine-x] |
| **4-Deck Consoles** | Independent 2/4 deck + 2/4 waveform selection [^engine-4-release] |
| **Linked Players (SC5000/6000)** | Roles: Auto / 1+3 / 2+4 / 1+2 / 3+4 / 1 / 2 — shadow decks mirror linked player [^engine-x] |
| **Phone/Tablet (Engine Remote)** | Remote control + library browse |

### 6.2 Deck Anatomy
- **Waveform**: Main scrolling waveform (zoomable pinch) + overview stripe
- **Dual Layer**: Two tracks on one deck (layer A/B) — both waveforms visible [^engine-os]
- **Shadow Decks**: Show linked player's waveform, cues, playhead, sync status [^engine-x]
- **Track Info**: Title, artist, BPM, key, cover art, time
- **Performance Pads**: 8 pads (4×2 grid) — off by default for waveform space [^engine-x]
- **Beat Phase / Beat Keeper**: 4-bar grid above waveform; left=slow, right=fast [^engine-4-blog]
- **Key Controls**: Semitone shift, Key Sync, Reset [^engine-4-release]

### 6.3 Waveform Styles
- **3 Modes**: Denon (blue/green/white), **RGB**, **3-Band** (low/mid/high) [^engine-x]
- **Orientation**: Vertical (default) or horizontal — per saved view [^engine-x]
- **Stacked Parallel**: 4 waveforms toggle (all decks on screen) [^engine-x]
- **Shadow Waveforms**: Cross-unit — streaming tracks show real waveform/beatgrid/cues via stm.db sync [^engine-x]
- **Beat Grid**: Bar ticks every 16 bars; downbeats bright; phrase markers [^engine-x]
- **Cue Countdown**: Time-to-cue display option [^engine-x]

### 6.4 Pad Modes
- **Pages**: Hot Cue, Hold FX, Beat Jump, Sampler, Keyboard, Key Shift, Beat Loop, Slicer [^opendj-spec]
- **Stems Pad Page**: Vocal, Melody, Bass, Drums — per-stem mute/solo/volume [^engine-x]
- **Pads Off by Default**: Maximizes waveform real estate [^engine-x]
- **Pad LED Brightness**: Configurable [^engine-x]

### 6.5 FX UI
- **Touch FX**: Swipe any direction → build-ups, drops, transitions [^engine-os]
- **Filter Echo / Filter Dub Echo**: Extra amount slider for tweakability [^engine-4-release]
- **FX Buttons**: Flash when engaged; Reset FX = defaults [^engine-4-release]
- **Channel FX**: 4 assignable hardware FX slots (Mixstream, Prime Go) [^engine-x]
- **Filter/Noise Sweep**: Audible in CUE/PFL [^engine-4-release]

### 6.6 Library / Browse UX
- **Collection Pane**: Drives, Playlists, Smartlists, Streaming, Cloud [^engine-manual]
- **Expanded Track List**: More rows + full waveform overviews with cues/loops [^engine-4-release]
- **Collapsible Playlist Tree**: Auto-collapses on track list focus; manual expand button [^engine-4-release]
- **Search Bar**: Same position in Library & Performance views; searches collection or current playlist [^engine-4-release]
- **Smartlists**: Rule-based auto-playlists across all drives [^engine-os]
- **On-device Playlist Management**: Drag-drop create/edit/reorder [^engine-os]
- **Library Import**: Serato, Traktor, rekordbox, iTunes/Apple Music — cues/grids preserved [^engine-manual]
- **Sync Manager**: Export to drive / Sync to Engine cloud [^engine-manual]

### 6.7 Stems
- **4-Part Stems**: Vocal, Melody, Bass, Drums [^engine-x]
- **On-Device Separation**: Swipe left on track → split to Stems playlist (basic quality) [^engine-x]
- **High Quality**: LAN offload to computer running `htdemucs` (Prime 4 G2, System One keep official) [^engine-x]
- **Control**: Performance Pads or Channel EQ knobs (`Stems EQ Mode` or `Shift+Cue`) [^engine-x]

### 6.8 Lyrics / Karaoke
- **Not natively supported**

### 6.9 Streaming Integration
- **7 Services**: TIDAL, NetEase Cloud Music, Spotify, Deezer, SoundCloud, Beatport, Apple Music [^engine-x]
- **Auth**: QR code phone login; Spotify Connect one-tap [^engine-x]
- **Quality**: FLAC / 320 kbps selection per service [^engine-x]
- **OmniSource Search**: Cross-source search (local + streaming + LAN) [^engine-x]
- **Local Caching**: Per-service cache settings [^engine-x]
- **Shadow Deck Streaming**: Cross-unit waveform/cue sync for streaming tracks [^engine-x]

### 6.10 Touch Gestures
- **Pinch-zoom Waveform**: Standard
- **Swipe Load**: Swipe track to deck
- **Drag-drop Playlist**: Reorder tracks
- **Tap-hold**: Track info popup
- **Touch FX**: Swipe for FX (build-up, drop, transition) [^engine-os]
- **Deck Swap**: Swap deck/waveform positions (SC5000/6000) [^engine-4-release]

### 6.11 Signature Features
- **Engine OS**: Embedded OS on hardware — no laptop needed
- **Layout Manager (5 Views)**: One-tap layout recall, persists reboot
- **Shadow Decks / Cross-unit Waveforms**: Unique multi-player visual sync
- **On-device Stems Separation**: Swipe-to-split, LAN offload for quality
- **7 Streaming Services + QR Auth**: Broadest standalone streaming
- **Engine Lighting**: DMX/Nanoleaf/Hue auto-sync + touchscreen control [^engine-os]
- **Ableton Link Wireless**: First standalone hardware with wireless Link [^engine-os]
- **Bluetooth Audio I/O**: Stream from phone / route to BT speaker [^engine-x]
- **Trainspotting Mode**: Hide track title/artist (blind test) [^engine-4-release]

---

## 7. Best-of Synthesis: Concrete Recommendations for Our App

### 7.1 Layout Presets per Form Factor

| Form Factor | Primary Layout | Alternative Layouts | Key Adaptations |
|-------------|----------------|---------------------|-----------------|
| **Phone Portrait** (S23 Ultra 19.3:9) | **Starter/Compact 2-Deck** — vertical waveforms stacked, large touch targets, collapsible browser drawer | Classic (turntables), Pro (jog+waveform), One Deck (prep) | Hide mixer EQ; tap deck header → expanded deck; swipe browser overlay; bottom sheet for pads/FX |
| **Phone Landscape** | **Pro 2-Deck** — horizontal parallel waveforms, jog wheels, mixer strip, pads side panel | 4-Deck (condensed), Looper, Video | Split-view: waveforms top 60%, mixer/pads bottom 40%; swipe pads/FX panels |
| **Tablet Portrait** (iPad 4:3) | **Pro 2-Deck** — vertical waveforms, full mixer, pads right sidebar, browser left drawer | Classic, Looper, One Deck | Sidebar browser (rekordbox Column View style); pads always visible |
| **Tablet Landscape** | **4-Deck Pro** — 4 horizontal waveforms, crossfader assignments, pads bottom grid | 2-Deck Expanded, Video, Looper | Equal deck sizing; pad grid 4×4; browser floating panel |
| **DeX / Desktop** (16:9, 21:9, 32:9, 4:3, any) | **Modular Layout Engine** — 5 saved Views (Engine-style) + free panel docking | Parallel (Traktor), Stack (Serato), Browser (Traktor), Preparation | **Fully resizable panels**: drag borders, snap to grid, save/restore 5 presets; per-view 2/4 deck + 2/4 waveform independent |

**Layout Engine Requirements**:
- **Panel System**: Waveform panel, Deck panel (×4), Mixer panel, Browser panel, Pads panel, FX panel, Sampler panel, Library panel — each independently show/hide/resize/dock
- **View Presets**: 5 slots (View 1–5) + "Save Current View" (Engine) + keyboard shortcuts (Traktor)
- **Orientation Logic**: Horizontal waveforms for landscape/wide; vertical for portrait/tall; user override always available
- **Aspect Ratio Agnostic**: Constraint solver (CSS Grid / Flex / custom) — no fixed coordinates

### 7.2 Deck Anatomy — Unified Component

| Element | Placement | Behavior |
|---------|-----------|----------|
| **Waveform Display** | Center (per deck) | Zoomable (pinch/wheel), scrub (drag), tap-to-cue, color mode toggle |
| **Overview Stripe** | Below waveform (or right in vertical) | Full-track; cue/loop/beatgrid markers; tap-to-seek |
| **Jog Wheel / Turntable** | Left of waveform (Pro/Classic) | Touch: scratch, seek, nudge; mouse: wheel scroll = pitch bend |
| **Transport** | Below jog | Play, Cue, Sync, Shift, Keylock, Tempo fader, Pitch bend ± |
| **Deck Header** | Top | Cover art, title, artist, BPM, Key, Time elapsed/remaining, Stem indicator |
| **Performance Pads** | Right sidebar (landscape) / bottom sheet (portrait) / floating | 8 pads × pages; page selector dropdown; 16-pad mode toggle |
| **Mixer Strip** | Right of deck (2-deck) / bottom (4-deck) | 3-band EQ + Filter, Gain, Fader, Cue, Meter, Crossfader assign |
| **FX Send** | Mixer strip top | Per-deck FX unit assignment, macro knob |

### 7.3 Waveform Styles — OLED-First Palette

| Mode | Colors (True Black Base #000000) | Use Case |
|------|----------------------------------|----------|
| **3-Band RGB** | Low: `#FF3333` (red), Mid: `#FFCC00` (amber), High: `#00FFFF` (cyan) | Standard mixing; color-blind safe |
| **4-Band (djay)** | Bass: `#FF3333`, Low-Mid: `#FFCC00`, High-Mid: `#33FF33`, Treble: `#0099FF` | Fine frequency discrimination |
| **Ultraviolet (Traktor)** | Single hue gradient: `#4B0082` → `#8A2BE2` → `#FFFFFF` | Minimal distraction; OLED power saving |
| **Infrared** | `#FF0000` → `#FF8800` → `#FFFF00` | High contrast |
| **Spectrum** | Full rainbow mapping (FFT bins) | Visual analysis |
| **Monochrome / Neutral** | `#FFFFFF` / `#CCCCCC` on `#000000` | Maximum battery life; accessibility |

**Beat Grid Markers**:
- Beats: 1px `#FFFFFF40` (25% opacity)
- Downbeats: 2px `#FFFFFF` + top triangle marker
- Phrases (16-bar): 3px `#FFFF00` + bar number label (Engine-style)

**Overview Stripe**: Same color mode; 2px height; cues as colored ticks (Hot Cue colors)

**Stem Waveforms** (when Stem Deck active):
- Vocal: `#FF66CC` (magenta)
- Drums: `#FF3333` (red)
- Bass: `#00FFFF` (cyan)
- Melody/Harmonic: `#FFCC00` (amber)
- Other: `#888888` (gray)

**OLED Optimization**:
- True black (`#000000`) background everywhere
- No large solid bright areas (avoid burn-in)
- UI chrome: `#111111` panels, `#222222` borders
- Accent color: Single user-picked hue (default `#00FFFF` cyan) for focus, playhead, active cues
- Dim inactive decks: 60% opacity

### 7.4 Pad Modes — Unified Page System

| Page | Pads (8) | Shift+Pad | Parameters |
|------|----------|-----------|------------|
| **Hot Cue** | 1–8 cues | Delete cue | Quantize, Smart Cue, Lock, Color, Bank (4) |
| **Loop** | 1/4, 1/2, 1, 2, 4, 8, 16, 32 beats | Move loop ± | Smart Loop, Quantize, Loop Back |
| **Slicer** | 8 slices across beat grid | Edit slice | Grid snap, quantize |
| **Sampler** | 8 slots × banks | Stop slot | Velocity, sync, choke groups |
| **Beat Jump** | ±1, 2, 4, 8, 16, 32 beats | ±1/2 beat | Quantize |
| **Key Shift** | -6..+6 semitones | Reset | Key Sync, Match |
| **Stems** | Vocal, Drums, Bass, Melody, Other (5) | Per-stem FX | Mute/Solo, Volume, Filter |
| **Pattern Player** | 16 steps × 4 tracks | Page tracks | Kit select, swing, length |
| **Custom** | User VDJScript / Rust closures | — | — |

**UI**: Page selector dropdown (Pro) / button bar (Essentials) / 16-pad toggle (View menu). Pad colors = cue colors / stem colors / sampler slot colors.

### 7.5 FX UI — Dual-Layer Design

**Layer 1: Mixer FX (Per Channel, 1-Knob Macro)**
- Filter (HP/LP/BP), Noise, Gate, Crush, Width, Duck — single knob + type selector
- Always visible on mixer strip

**Layer 2: FX Units (Global, 4 Units × 2 Modes)**
- **Group Mode**: 3 FX chained → X/Y macro pad (Traktor) / XY Pad (djay) / Touch swipe (Engine)
- **Single Mode**: 1 FX per unit → full parameter sliders
- **Assignment Matrix**: Any Unit → Deck A/B/C/D / Master / Sampler / Mic
- **Touch FX**: Swipe gestures on waveform/FX panel (Engine) → mapped to X/Y of active unit

**FX Rack Panel**: Collapsible; shows all 4 units with mini XY pads; expand for parameters.

### 7.6 Library / Browse UX — Best-of Features

| Feature | Source | Implementation |
|---------|--------|----------------|
| **Unified Media Browser** | djay, VirtualDJ | Single tree: Local, Streaming (all services), Cloud, LAN, Smartlists |
| **Column View** | rekordbox | Hierarchical playlist/folder nav (Finder-style) — desktop/tablet |
| **Collection Filter** | rekordbox | Multi-column filter bar (Genre, BPM, Key, Artist, Label, Date, Stem avail) |
| **Sub Browser / Dual Pane** | rekordbox | Two collection panes side-by-side (compare playlists) |
| **Expanded Track List** | Engine 4 | Toggle: compact rows ↔️ expanded rows with mini waveform + cues |
| **Search Bar** | Engine 4 | Fixed position (both Library & Performance views); scope: Collection / Playlist / Streaming |
| **Smartlists / Intelligent Playlists** | rekordbox, Engine | Rule-based auto-playlists; AI "similar to selected" (Collection Radar) |
| **Cloud Sync** | rekordbox, djay | Cues, grids, metadata, playlists → cloud (user choice: iCloud, Dropbox, Google, custom) |
| **Library Import** | Engine | Serato, Traktor, rekordbox, iTunes — cues/grids preserved |
| **Stems Playlist** | rekordbox, Engine | Auto-generated "Stems" playlist in sidebar |
| **Vocal Position Markers** | rekordbox | AI vocal detection → markers on overview waveform |
| **Offline Cache Mgmt** | Engine | Per-service cache size, auto-evict LRU |

### 7.7 Stems — Neural Mix + Engine Hybrid

- **Separation**: On-device (Core ML / ONNX Runtime / candle) — 4 stems (Vocals, Drums, Bass, Melody) + Other
- **Quality Tiers**: Fast (mobile) / Balanced / High (desktop GPU) / LAN offload (Engine-style)
- **Deck Integration**: Load → auto Stem Deck; per-stem volume, mute, solo, filter, FX send
- **Crossfader Fusion**: Stem-aware morph (djay) — assign stems to crossfader curve
- **Pad Page**: 5-pad stem control + per-stem FX
- **EQ Modes**: 3-knob (Vocal/Drums/Harmonic) + 5-knob full
- **Stems Playlist**: Auto-populated in browser

### 7.8 Lyrics / Karaoke — VirtualDJ Parity

- **Lyrics Editor**: LRC + custom format; sync to beatgrid
- **Lyrics Overlay**: On waveform (toggle) + full-screen karaoke view
- **Karaoke VideoFX**: Lyrics on video output (HDMI/DeX external display) with guide vocal (vocal stem muted)
- **Clone View**: Second window for singer (DeX / desktop multi-monitor)
- **Instant Karaoke**: One tap → mute Vocal stem + enable Karaoke VideoFX

### 7.9 Streaming Integration — Unified + OmniSource

| Service | Auth | Quality | Offline | Notes |
|---------|------|---------|---------|-------|
| Spotify | OAuth | 320 kbps | Cache | Premium required |
| Apple Music | MusicKit | 256 AAC / Lossless | Cache | 100M+ catalog |
| TIDAL | OAuth | FLAC / HiRes | Cache | |
| SoundCloud | OAuth | 256 kbps | Cache | Go+ for full |
| Beatport / Beatsource | OAuth | FLAC / 320 | Cache | DJ-focused |
| Deezer / Qobuz | OAuth | FLAC / HiRes | Cache | |
| NetEase | QR | 320 kbps | Cache | Engine-style QR login |

- **OmniSource Search**: Single query → merged results (local + all streaming + LAN) with source badges
- **Cross-source Playlists**: Mix local + streaming tracks; missing tracks grayed out
- **QR Login Flow**: Engine-style — scan QR on phone → token stored securely

### 7.10 Touch Gestures — Comprehensive Map

| Gesture | Target | Action |
|---------|--------|--------|
| **Tap** | Waveform | Set temporary cue / seek |
| **Double-tap** | Waveform | Set hot cue (next slot) |
| **Long press** | Waveform | Loop in/out start |
| **Drag** | Waveform | Scrub / scratch |
| **Pinch** | Waveform | Zoom |
| **Swipe L→R** | Track row (browser) | Load to Deck 1 / next free |
| **Swipe R→L** | Track row | Stems split / more actions |
| **Two-finger rotate** | Jog / Waveform | Scratch / seek (djay) |
| **Two-finger drag** | Pad / XY Pad | Pressure + position (djay) |
| **Swipe any dir** | FX Panel / Waveform | Touch FX build-up/drop (Engine) |
| **Three-finger tap** | Anywhere | Toggle browser / pads / FX |
| **Edge swipe (DeX)** | Screen edge | Show/hide panels (browser, pads, FX) |

**Mouse/Keyboard**: Full parity — every touch action has keyboard shortcut (Traktor/djay 70+) and mouse equivalent.

### 7.11 Signature Features to Adopt / Innovate

| Feature | Origin | Our Twist |
|---------|--------|-----------|
| **Modular Layout Engine** | Engine (Views) + Traktor (Layouts) | 5 saved views + free docking + aspect-ratio agnostic |
| **Parallel Waveforms** | Traktor Parallel | Always-available toggle; works 2/4 deck; phrase-aligned |
| **Shadow Decks** | Engine | Cross-device (phone + DeX + desktop) via local LAN sync |
| **Neural Mix Stems** | djay | 4-stem ONNX; quality tiers; LAN offload |
| **Crossfader Fusion** | djay | Stem-aware + user-defined curves |
| **Fluid Beatgrid** | djay | Variable-tempo support (Traktor flexible beatgrids) |
| **OmniSource Search** | Engine | Unified local + 7 streaming + LAN |
| **Lyrics + Karaoke** | VirtualDJ | LRC editor + overlay + Clone View + instant karaoke |
| **Touch FX Swipe** | Engine | Customizable swipe→FX mapping |
| **Ableton Link + Wireless** | Engine | Native Link + Bluetooth MIDI |
| **Engine Lighting** | Engine | DMX/Art-Net/sACN + Hue/Nanoleaf + touchscreen designer |
| **DVS (AI)** | djay | Timecode + AI-assisted lead-in detection |
| **VDJScript Equivalent** | VirtualDJ | Rust plugin API + WASM user scripts |
| **Skins/Themes** | VirtualDJ | CSS-in-Rust theming + OLED palette generator |
| **Sandbox Mode** | VirtualDJ | Isolated experiment session |

---

## 8. OLED Dark Palette Guidance (True Black Base)

### 8.1 Color Tokens

| Token | Hex | Usage |
|-------|-----|-------|
| `bg.true` | `#000000` | App background, panel backgrounds |
| `bg.panel` | `#0A0A0A` | Elevated panels (browser, FX rack) |
| `bg.raised` | `#141414` | Modals, dropdowns, tooltips |
| `border.subtle` | `#222222` | Panel dividers, deck outlines |
| `border.focus` | `#00FFFF` | Active deck, focused input, playhead |
| `fg.primary` | `#E0E0E0` | Primary text |
| `fg.secondary` | `#888888` | Secondary text, timestamps |
| `fg.muted` | `#555555` | Disabled, placeholder |
| `accent.cyan` | `#00FFFF` | Default accent (playhead, active cues, sync) |
| `accent.magenta` | `#FF33CC` | Vocal stem, hot cues (user-assignable) |
| `accent.amber` | `#FFCC00` | Melody/Bass stem, downbeat markers |
| `accent.red` | `#FF3333` | Drums stem, bass freq, warning |
| `accent.green` | `#33FF33` | High-mid freq, play state |
| `accent.blue` | `#0099FF` | Treble freq, cue points |
| `wave.low` | `#FF3333` | 3-band low |
| `wave.mid` | `#FFCC00` | 3-band mid |
| `wave.high` | `#00FFFF` | 3-band high |
| `wave.vocal` | `#FF66CC` | Stem vocal |
| `wave.drums` | `#FF3333` | Stem drums |
| `wave.bass` | `#00FFFF` | Stem bass |
| `wave.melody` | `#FFCC00` | Stem melody |
| `wave.other` | `#888888` | Stem other |
| `grid.beat` | `#FFFFFF40` | Beat grid line (25% opacity) |
| `grid.downbeat` | `#FFFFFF` | Downbeat line + marker |
| `grid.phrase` | `#FFFF00` | Phrase (16-bar) line + label |

### 8.2 Contrast Rules
- **Text on `bg.true`**: `fg.primary` (15.3:1) — AAA
- **Text on `bg.panel`**: `fg.primary` (12.6:1) — AAA
- **Accent on `bg.true`**: `accent.cyan` (10.2:1) — AAA
- **Grid lines**: Opacity-based (not color) — preserves true black pixels
- **No pure white (`#FFFFFF`) large areas** — use `#E0E0E0` max for text
- **Glow effects**: CSS `filter: drop-shadow(0 0 4px accent)` — no box-shadow (bleeds on OLED)

### 8.3 Power-Saving Modes
| Mode | Changes |
|------|---------|
| **Standard** | Full palette above |
| **Dim** | `fg.primary` → `#AAAAAA`; accents 60% opacity; waveforms 50% brightness |
| **Ultra** | Monochrome waveform (`#888888`); hide overview stripe; panels `#000000`; only playhead + active cues lit |
| **Burn-in Protection** | Slow UI element drift (1px/hr); periodic full-screen black frame (1 frame/10 min) |

### 8.4 Accessibility
- **High Contrast Toggle**: `fg.primary` → `#FFFFFF`, borders → `#444444`
- **Color Blind Safe**: 3-band palette uses luminance-separated hues (red/amber/cyan)
- **Reduced Motion**: Disable waveform scroll animation; instant seek

---

## 9. Sources

[^serato-display]: Serato DJ Pro Display Modes — https://support.serato.com/hc/en-us/articles/225122768-Display-Modes
[^serato-waveform]: Serato Main Waveform Display — https://support.serato.com/hc/en-us/articles/224969307-Main-Waveform-Display
[^serato-stems-prep]: Preparing tracks for use with Stems — https://support.serato.com/hc/en-us/articles/5700969287183-Preparing-tracks-for-use-with-Stems
[^rekordbox-style]: rekordbox Style — https://rekordbox.com/en/feature/style/
[^rekordbox-overview]: rekordbox 7 Overview — https://rekordbox.com/en/feature/overview/
[^rekordbox-mobile]: rekordbox Mobile — https://rekordbox.com/en/feature/mobile/
[^vdj-rhythmwave]: VirtualDJ Rhythm Wave Manual — https://virtualdj.com/manuals/virtualdj/interface/topsection/rhythmwave.html
[^vdj-waveform]: VirtualDJ Waveform Display Manual — https://virtualdj.com/manuals/virtualdj/interface/topsection/appcontrols/layout/waveform.html
[^vdj-pads]: VirtualDJ Pads Manual — https://virtualdj.com/manuals/virtualdj/interface/decks/decksadvanced/pads.html
[^vdj-stems]: VirtualDJ Stems 2.0 Manual — https://virtualdj.com/manuals/virtualdj/interface/stems2.html
[^vdj-karaoke]: VirtualDJ Karaoke Manual — https://virtualdj.com/manuals/virtualdj/interface/browser/sideview/karaoke.html
[^vdj-browser]: VirtualDJ Browser Manual — https://virtualdj.com/manuals/virtualdj/interface/browser.html
[^vdj-remote]: VirtualDJ Remote — https://virtualdj.com/manuals/virtualdj/vdjremote.html
[^traktor-setup]: Traktor Setting Up Traktor (Layouts) — https://docs.native-instruments.com/online-guides/traktor-pro-manual/en/setting-up-traktor
[^traktor-overview]: Traktor Overview — https://docs.native-instruments.com/online-guides/traktor-pro-manual/en/traktor-overview
[^traktor-pro4]: Traktor Pro 4 Features — https://www.native-instruments.com/products/traktor-pro
[^djay-windows]: djay Pro Windows — https://www.algoriddim.com/djay-pro-windows
[^djay-ios-decks]: djay iOS Deck Views — https://help.algoriddim.com/user-manual/djay-ios/getting-started/decks
[^djay-ipad-manual]: djay iOS iPad Manual (PDF) — http://download.algoriddim.com/manual/djay%20iOS%20iPad%20Manual%20v1.pdf
[^djay-releasenotes]: djay Pro Release Notes — https://www.algoriddim.com/djay-pro-mac/releasenotes
[^djay-ipad-press]: Algoriddim Unveils djay Pro for iPad — https://www.algoriddim.com/press_releases/265-algoriddim-unveils-djay-pro-for-ipad
[^djay-ios]: djay iOS — https://www.algoriddim.com/djay-ios
[^engine-os]: Engine DJ OS — https://enginedj.com/software/enginedj-os
[^engine-x]: EngineX OS Deep Dive — https://github.com/Cd1s/EngineX-OS
[^engine-manual]: Engine DJ User Guide v2.1.0 — https://cdn.inmusicbrands.com/engine/Engine%20DJ%20-%20User%20Guide%20-%20v2.1.0.pdf
[^engine-4-release]: Engine DJ 4.0 Release Notes — https://inmusicsupport.freshdesk.com/en/support/solutions/articles/69000855869-engine-dj-4-0-release-notes
[^engine-4-blog]: Engine DJ 4.0 Blog — https://wearecrossfader.co.uk/blog/engine-4-0/
[^opendj-spec]: OpenDJDecks SPEC.md — https://github.com/nurkert/OpenDJDecks/blob/main/docs/SPEC.md

---

## 10. Next Steps for Implementation

1. **Layout Engine**: Build constraint-based panel system (Rust + GPU UI framework) with 5 view presets + free docking
2. **Waveform Renderer**: GPU instanced rendering; 4 color modes + stem waveforms; true black background
3. **Deck Component**: Unified deck widget with 7 layout densities (Micro → Parallel Full)
4. **Pad System**: Page-based, 8/16 pads, shift actions, custom Rust/WASM scripts
5. **FX Rack**: 4 units × 2 modes + Mixer FX; Touch FX swipe mapper
6. **Library Core**: Unified source abstraction (Local, Streaming×7, Cloud, LAN); Column View + Expanded List
7. **Stems Pipeline**: ONNX Runtime / candle inference; 4-stem; quality tiers; LAN offload protocol
8. **Lyrics/Karaoke**: LRC parser; overlay renderer; Clone View (multi-window)
9. **Streaming SDK**: OAuth/QR flows; OmniSource search; offline cache manager
10. **Gesture/Input Map**: Unified touch/mouse/keyboard/MIDI/HID action bus
11. **OLED Theme System**: Token-based theming; power modes; burn-in protection
12. **Cross-device Sync**: Local LAN discovery (mDNS); shadow deck state sync; cloud backup
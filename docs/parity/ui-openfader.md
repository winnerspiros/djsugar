# OpenFadeR UI Specification (Screen-by-Screen)

Based on source code at `/home/ubuntu/.hermes/cache/scratch/parity-src/openfader/ui/*.slint` (Slint UI files) and supporting Rust logic. Every screen, region, panel, control, popup, menu, gesture, color/size token is documented.

---

## 1. Global UI Structure

### 1.1 Main Window (`src/main.rs` → `app::run` → `ui/app.slint`)
- **Window**: `MainWindow` — full-screen or resizable, no title bar on Windows
- **Background**: `Theme.bg`
- **Root**: `VerticalLayout`
- **Global bar** (top, `Theme.h-bar` = 36px): `GlobalBar` with mode selector, transport, status chips
- **Workspace**: one of three layout profiles (Desktop / Touch / Stage) selected by `Theme.layout-profile`
- **No persistent sidebars** — library, decks, mixer, panels are workspace-dependent

### 1.2 Global Bar (`GlobalBar` in `ui/app.slint`)
- **Left**: Mode selector (SegmentedChips/Dropdown): LIBRARY, STAGE, EXPORT, VIDEO, LIGHTING
- **Center**: Global transport (Automix, Sync, Quantize), Master tempo, Clock
- **Right**: 
  - DEVICES chip (lit when busy, opens DeviceWindow)
  - ResourceChip (RAM/VRAM)
  - Settings button
  - Controller status (name + link)
  - Audio device status

### 1.3 Layout Profiles (`theme.slint` → `layout-profile`)
| Profile | Value | Description |
|---------|-------|-------------|
| Desktop | 0 | Standard desktop UI, mouse/keyboard, compact controls |
| Touch | 1 | Touch-friendly, 44px targets, enlarged controls, bottom bar |
| Stage | 2 | Performance workspace, 4 deck styles, mixer tabs, strip, sidebar options |

---

## 2. Workspace: LIBRARY Mode (Desktop Profile)

### 2.1 Layout (`ui/browser.slint` → `Browser` component)
```
┌─────────────────────────────────────────────────────────────────────┐
│ GLOBAL BAR                                                          │
├──────────────┬──────────────────────────────────────┬──────────────┤
│ BROWSER TREE │         TRACK LIST (center)          │  SIDE PANEL  │
│ (left)       │  [Filter Bar]                        │  (right)     │
│              │  [Track Table]                       │  [Tabs]      │
└──────────────┴──────────────────────────────────────┴──────────────┘
```

### 2.2 Browser Tree (`ui/browser-tree.slint`)
- **Component**: `BrowserTree` — `ListView` of `Folder` nodes
- **Folder kinds** (icon + behavior):
  - Collection root, List folder, Playlist, Smart list, History session
  - Imported folders (read-only, adoptable), Bandcamp store
  - Files view: Folder, Drive, Pinned folder, Cloud drive
- **Indicators**: separator lines, stage badge (Inbox/Triage/Collection), track count, color dot, icon (crate/star/heart/flame/bolt/moon/sun/leaf)
- **Interactions**:
  - Click → select, show tracks in center
  - Double-click playlist → rename (if editable)
  - Right-click → context menu (New playlist/crate, Rename, Delete, Import, Analyze All, Make lists mine)
  - Drag tracks onto folder → add to crate/playlist
  - Chevron expand/collapse

### 2.3 Filter Bar (`ui/filters.slint` — not present; OpenFadeR uses palette columns instead)
- **No global filter bar** — filtering done via Palette Columns (tag lists)

### 2.4 Track Table (`ui/browser-table.slint`)
- **Component**: `BrowserTable` — virtualized `ListView` of `TrackRow`
- **Columns** (fixed, not user-reorderable):
  - #, Title, Artist, Album, Genre, Length, BPM, Key, Rating, Color, Waveform
- **TrackRow** struct: title, artist, album, genre, length, bpm, key, rating, deck loaded, available, missing, cover, color, action (store), key-match, key-shift, mini waveform, analysing, fade, streamed, set-start
- **Row styling**:
  - Missing: red strikethrough
  - Duplicate (fade): dimmed
  - Streamed: badge
  - Key match: green highlight
  - Mini waveform: white, colored by list palette
- **Interactions**:
  - Click → select
  - Double-click → load to active deck
  - Right-click → track menu (`TrackMenu`)
  - Drag → deck, palette column, device, export
  - Sort: click header (single column)

### 2.5 Track Menu (`ui/track-menu.slint`)
- **PopupWindow** anchored to row
- **Items**: Play, Load to Deck 1/2/3/4, Add to Automix, Add to Sidelist, Edit Cues, Edit Grid, Find Artwork, Show in Folder, Delete, Properties, Copy metadata

### 2.6 Palette Columns (`ui/browser-palette.slint`) — right sidebar replacement
- **Component**: `PaletteColumn` (slot 0 = TAG LIST, slots 1-3 = user playlists)
- **Header**: name (click → select in tree, double-click → rename, chevron → pick playlist)
- **DropArea**: accepts tracks from table / other palettes, playlists from tree
- **Rows**: simple text list, click → select in table, drag → reorder
- **Context menu** (right-click row): Remove, Move up/down, Play, Load to deck

### 2.7 Side Panel Tabs (`ui/browser.slint` imports)
| Tab | Component | Source |
|-----|-----------|--------|
| Automix | `AutomixPanel` | `ui/automix.slint` (not in UI, Rust-owned) |
| Sidelist | `SidelistPanel` | `ui/exportmode-dock.slint` (Sidelist) |
| Cues | `CuesPanel` | `ui/exportmode-cues.slint` |
| Lyrics | `LyricsPanel` | `ui/lyrics.slint` (not present; uses `trackinfo.slint`) |
| Info | `TrackInfoPanel` | `ui/trackinfo.slint` |
| Instruments | `InstrumentsPanel` | `ui/instruments.slint` (not present; uses `groove.slint`) |
| Checklist | — | Not in OpenFadeR |

---

## 3. Workspace: STAGE Mode (Stage Profile)

### 3.1 Layout (`ui/stage/stage.slint`)
```
┌─────────────────────────────────────────────────────────────────────┐
│ GLOBAL BAR (mode=STAGE)                                             │
├─────────────────────────────────────────────────────────────────────┤
│ STAGE DECK 1 (left)                    STAGE DECK 2 (right)        │
│ [Performance/Turntable/Controller/Ring]                            │
├─────────────────────────────────────────────────────────────────────┤
│ STAGE MIXER (tabs: Mixer/FX/Rec  |  Mixer/Video/Lighting)          │
├─────────────────────────────────────────────────────────────────────┤
│ STAGE STRIP (Effects+Sampler  |  Sampler only)                      │
├─────────────────────────────────────────────────────────────────────┤
│ STAGE LIBRARY (sidebar: Labels  |  Icons only)                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 3.2 Stage Deck (`ui/stage/deck.slint` → `StageDeck`)
- **4 Styles** (`Theme.stage-deck`):
  0. **Performance** (`ui/stage/deck-performance.slint`): Waveform top, pads middle, jog bottom, transport
  1. **Turntable** (`ui/stage/deck-turntable.slint`): Large jog center, waveform ring, pads around
  2. **Controller** (`ui/stage/deck-controller.slint`): Mixer-style, faders, knobs, buttons
  3. **Ring** (`ui/stage/deck-ring.slint`): Circular waveform, jog ring, minimal
- **Mirroring**: `mirrored` prop + `Theme.stage-mirror` (0=off, 1=on)
- **DropTarget**: `DeckDropTarget` for track loading

### 3.3 Stage Mixer (`ui/stage/mixer.slint`)
- **Tabs** (`Theme.stage-tabs`):
  0. Mixer / FX / Rec
  1. Mixer / Video / Lighting
- **Mixer tab**: Channel strips (gain, EQ, filter, fader, cue, stem iso), master section (VU, crossfader, master level, record)
- **FX tab**: Effect rack per deck (3 slots), Beat FX center
- **Rec tab**: Record controls, source selector, level meter
- **Video tab**: Video output controls (transition, overlay, window)
- **Lighting tab**: DMX fixture controls, scene selector

### 3.4 Stage Strip (`Theme.stage-strip`)
- 0: Effects + Sampler (side by side)
- 1: Sampler only

### 3.5 Stage Library Sidebar (`Theme.stage-sidebar`)
- 0: Labels (playlist names)
- 1: Icons only (compact)

### 3.6 Stage Waveform Position (`Theme.stage-wave`)
- 0: Top (above decks)
- 1: Centre (between decks and mixer)
- 2: Decks (inside each deck)

---

## 4. Workspace: EXPORT Mode

### 4.1 Layout (`ui/exportmode.slint`)
```
┌─────────────────────────────────────────────────────────────────────┐
│ GLOBAL BAR (mode=EXPORT)                                            │
├─────────────────────────────────────────────────────────────────────┤
│ PLAYER LAYOUT SELECTOR: [1 PLAYER] [2 PLAYER] [DUAL PLAYER] [SIMPLE]│
├─────────────────────────────────────────────────────────────────────┤
│ SUB-PANEL TABS: [GRID] [CUES] [DOCK]                                │
└─────────────────────────────────────────────────────────────────────┘
```

### 4.2 Player Layouts (`ui/exportmode-parts.slint`)
- **1 PLAYER**: Single large deck (left), library right
- **2 PLAYER**: Two decks side by side, library below
- **DUAL PLAYER**: Two decks stacked vertically, library right
- **SIMPLE**: Minimal deck, no library (for quick export preview)

### 4.3 Grid Sub-Panel (`ui/exportmode-grid.slint`)
- **Phrase Tools** (`PhraseTools`):
  - Phrase list with type/character tags
  - Cut / Retype / Move phrase buttons
  - Phrase editor: start/end time, type dropdown, character dropdown
- **Grid Tools** (`GridTools`):
  - BPM display + edit, ×2 / ÷2 buttons
  - Shift grid: ← / → (nudge), Tap button
  - Range: WHOLE / FROM HERE selector
  - Analysis Lock: lock icon (prevents re-analysis)
  - Undo / Redo buttons
  - Metronome: on/off, volume (3 levels)

### 4.4 Cues Sub-Panel (`ui/exportmode-cues.slint`)
- **SubPanel** with cue list (`CueRow`: time, len, name, comment, color)
- **Color palette popup**: 8 colors + custom
- **Play modes**: Oneshot, Hold, Stutter, Loop (per cue)
- **Comment field**: inline edit

### 4.5 Dock Sub-Panel (`ui/exportmode-dock.slint`)
- **Palette columns** (same as Library mode): TAG LIST + 3 user playlists
- **Drag-drop** tracks to palettes, reorder within

---

## 5. Workspace: VIDEO Mode

### 5.1 Layout (`ui/video.slint` → `VideoState` global + `VideoView`)
```
┌─────────────────────────────────────────────────────────────────────┐
│ GLOBAL BAR (mode=VIDEO)                                             │
├─────────────────────────────────────────────────────────────────────┤
│ MASTER OUTPUT (16:9, letterboxed)                                   │
│   [Deck 1 thumb + weight]  [Deck 2 thumb + weight]  [Deck 3] [Deck 4]│
├─────────────────────────────────────────────────────────────────────┤
│ VIDEO LIBRARY (list of video tracks)                                │
├─────────────────────────────────────────────────────────────────────┤
│ CONTROLS: Transition [FADE|CUT], Follow Fader, Overlay [TEXT|IMAGE] │
│           Opacity, Size, Image picker, Text input                   │
│           [OUTPUT WINDOW] [FULL SCREEN]                             │
└─────────────────────────────────────────────────────────────────────┘
```

### 5.2 Video Output Window (`VideoWindow`)
- **Window**: 960×540 preferred, separate monitor/projector
- **Content**: `VideoOutput` — master frame + overlay (text/image)
- **Double-click** → toggle fullscreen
- **Overlay**: mode 0=Text (centered, sized), 1=Image (contain, centered)
- **Opacity/Size** sliders

---

## 6. Workspace: LIGHTING Mode

### 6.1 Layout (`ui/lighting.slint` → `LightingState` global + `LightingView`)
```
┌─────────────────────────────────────────────────────────────────────┐
│ GLOBAL BAR (mode=LIGHTING)                                          │
├─────────────────────────────────────────────────────────────────────┤
│ STAGE SIMULATOR (3 rows: Moving Heads back, Strobes mid, Pars front)│
├─────────────────────────────────────────────────────────────────────┤
│ FIXTURE PATCH LIST (name, profile, universe@address, color preview) │
├─────────────────────────────────────────────────────────────────────┤
│ CONTROLS: Protocol [OFF|ART-NET|SACN|BOTH], IP, Palette, Mode,     │
│           Strobe, Master dimmer, Profile stepper, Add/Remove fixture│
└─────────────────────────────────────────────────────────────────────┘
```

### 6.2 Stage Simulator (`Stage` component)
- **3 rows**: Moving heads (back, 38px), Strobes (middle, 30px×18px), Pars (front, 46px)
- **Fixtures**: positioned evenly per row, colored by live DMX values
- **Glow**: drop-shadow with fixture color
- **Conflict**: red border if DMX address overlap
- **Label**: name below fixture (dim)

### 6.3 Patch List (`PatchRow`)
- **Per fixture**: color swatch, name, universe spinbox, address spinbox, remove button
- **Conflict** highlighted in red
- **Add fixture** → picks `profile` from built-in + user profiles (`fixtures-dir`)

---

## 7. Deck Components (Shared)

### 7.1 Deck Header (`ui/deck-header.slint`)
- **Jacket** (`Jacket`): cover art, click → load, shows stem isolation badges
- **Title/Artist**: `DeckTitle` — track title, artist, album (if known)
- **Times** (`DeckTimes`): elapsed / remaining, BPM, key (Camelot/Note), sync rate
- **Sync Cluster** (`SyncCluster`): MASTER, SYNC, KEY SYNC chips
- **Stem Row** (`StemRow`): 4 chips (Vocal, Inst, Drums, Bass) — isolation toggles
- **Transport** (`DeckTransport`): prev/play/next, cue, jog countdown display

### 7.2 Deck Waveform (`ui/deck-wave.slint`)
- **Overview** (`DeckOverview`): full track, minute dots, progress, cues, loops, automix region
- **WaveStrip** (`WaveStrip`): enlarged scrolling waveform, pixelated rendering
  - `strip-x` scroll offset, `pixel-perfect` alignment
  - Stem layers: 4 colored waveforms (toggleable)
  - Lyrics words: `WaveLyrics` global (word timestamps)
- **Phrase Strip** (`PhraseStrip`): phrase markers below waveform
- **Scroll**: drag waveform, scrollbar, keyboard, touch swipe

### 7.3 Deck Controls (`ui/deck-controls.slint`)
- **TempoColumn**: tempo fader (vertical), range chip (±6/10/16/25/50/100%), key lock, sync, master tempo
- **LoopBlock**: In/Out, size, halve/double, move ←/→, reloop
- **RingColumn**: jog wheel (`JogWheel` touch-sensitive), play/pause center, cue button
- **RingButton**: circular buttons for transport, cue, hot cues
- **PadGrid** (`ui/pads.slint`): 8 pads, modes 0-11+ (Hot Cue, Pad FX, Slicer, Sampler, Sequencer, Stems, Plugin)

### 7.4 Deck Panel (`ui/deck-panel.slint`) — Touch Profile
- **Enlarged**: all controls ≥44px (`Theme.touch-target`)
- **Bottom bar**: transport, tempo, loop, pads, jog
- **Gesture-friendly**: swipe waveform, pinch zoom, long-press menus

---

## 8. Mixer Components

### 8.1 Desktop Mixer (`ui/mixer.slint`)
- **TouchChannel** (per deck): gain knob, 3 EQ knobs (High/Mid/Low), filter/CFX knob, fader, cue chip, stem iso chip
- **BusChannel** (master/headphones/booth): level knob, split cue, record chip
- **CrossFader**: curve selector, assign, crossfader slider
- **Meter**: stereo VU, peak hold, clip LED

### 8.2 Touch Mixer (`ui/mixer-touch.slint`)
- **Horizontal faders** (TouchChannel): gain, EQ, filter, fader all horizontal
- **BusKnob** (master/headphones/booth): large knobs, value display
- **Transport row**: prev/play/next, sync, quantize, automix, record
- **Bottom bar** integration

---

## 9. Sampler & Groove Box

### 9.1 Sampler (`ui/sampler.slint` → `GrooveDeckView` + `GrooveState`)
- **8 slots per deck** (2 banks A/B), `GrooveDeckView` per deck
- **DrumSlotView**: pad, state (empty/loaded/playing), play mode, level
- **Modes**: Oneshot, Hold, Stutter, Loop
- **Master gain** per deck
- **Multi mode**: play multiple slots simultaneously
- **Capture**: record loop to slot (`capture-on`, `capture-beats`)
- **Release FX**: effect + beat on release (`release-fx`, `release-beat`)
- **Presets**: factory + user, save/load/step (`GrooveState.presets`)

### 9.2 Groove Box (`ui/groove.slint`) — separate panel
- **Drum Swap**: 8 slots per deck, 2 banks, drag from library
- **Sequencer**: step sequencer per slot
- **Kits**: preset drum kits

---

## 10. Popups & Dialogs

### 10.1 Device Window (`ui/device.slint` → `DeviceWindow`)
- **DialogFrame**: 900×780 (desktop), full-screen (touch)
- **Tabs** (SegmentedChips/Dropdown): DEVICES, EXPORT, SYNC MANAGER, BACKUPS
- **Progress bar** at bottom when busy

### 10.2 DEVICES Tab (`ui/device-devices.slint` → `DevicesPage`)
- **Device list**: `DeviceItem` (name, icon, formats, status, capacity)
- **Actions**: Format (opens `DeviceFormat`), Backup, Restore, Eject
- **Format options** per device: Pioneer Device Library, OneLibrary, Engine DJ
- **Device preview** (`DevicePreview`): shows what write will do (add/update/blocked tracks)

### 10.3 EXPORT Tab (`ui/device-export.slint` → `ExportPage`)
- **Device/folder picker**: dropdown devices + folder text field
- **Format chips**: one per library format device supports (dot = already on device)
- **Playlist picker**: `PickList` (checklist of playlists, none = everything)
- **EXPORT button**: starts write, shows `DevicePreview` first

### 10.4 SYNC MANAGER Tab (`ui/device-export.slint` → `SyncPage`)
- **Toggle**: "Synchronize playlists with [device]"
- **Format chips** (same as Export)
- **Playlist checklist**: `PickList` (playlists to keep in sync)
- **SYNC NOW** button

### 10.5 BACKUPS Tab (`ui/device-backup.slint` → `BackupPage`)
- **Backup folder** text field
- **Backup list**: table (Date, Computer, Device, FS) with selection
- **Buttons**: BACKUP NOW, RESTORE TO DEVICE, DELETE

### 10.6 Device Preview (`ui/device-preview.slint` → `DevicePreview`)
- **DialogFrame**: 640px wide
- **Sections**: "X new tracks to copy (Y MB)", "Z tracks to refresh", info lines, blocked tracks (red)
- **Each section**: SHOW/HIDE LIST toggle → virtualized `TrackList`
- **START / CANCEL** buttons

### 10.7 Device Sync Ask (`ui/device-preview.slint` → `DeviceSyncAsk`)
- **DialogFrame**: 460px
- **Text**: "KEY is plugged in: sync 3 playlists (12 new tracks, 480 MB)?"
- **SYNC / NOT NOW** buttons

### 10.8 Device Review (`ui/device-review.slint` → `ReviewOverlay`)
- **Full-screen overlay** during device write
- **Progress**: phase + %, cancel button
- **Review rows**: compare collection vs device (ratings, colors, comments, history)

### 10.9 Controller Editor — not a dialog; integrated in Preferences
- **Preferences → Decks & Controllers** → `PrefsController` → "Get mappings" chip → `GetMappings` (`ui/catalog.slint`)

### 10.10 Artwork Picker (`ui/artwork.slint` → `ArtworkPicker`)
- **DialogFrame**: 584×480
- **Grid**: 4 columns, `ArtCandidate` (thumb, source, size)
- **Click** → choose, downloads and saves
- **Status**: "Asking MusicBrainz…", "No artwork found"

### 10.11 Preferences (`ui/prefs-view.slint` + `ui/prefs-controller.slint` + `ui/prefs-keyboard.slint`)
- **Root**: `PrefRows` from `prefs-state.slint` (Rust-owned)
- **Controller page**: connected controller name, "Get mappings" → `GetMappings`, then `PrefRows`
- **Keyboard page**: shortcut editor

### 10.12 Catalog / Extensions (`ui/catalog.slint`)
- **Repositories list**: `RepoBar` (name, note, remove, add, refresh)
- **Get Plugins**: `GetPlugin` list (name, version, installed, description, repo, native, action)
- **Get Themes**: `GetTheme` list (name, author, description, repo, action, active, colors)
- **Get Mappings**: `GetMapping` list (controller, port, repo, action, search, install)
- **Auto-offer**: "Mapping for X available" when controller connects without mapping

### 10.13 Import Report (`ui/import-report.slint`)
- **Dialog**: shows imported playlists, cues, grids, tracks, errors

### 10.14 Update Dialog (`ui/update.slint`)
- **Dialog**: version, changelog, RESTART NOW / LATER

### 10.15 Toast Notifications (`ui/toast.slint`)
- **Global**: `ToastState` (text, kind: info/warning/error, timeout)
- **Overlay**: `ToastOverlay` draws at bottom center, auto-dismiss

---

## 11. Waveform & Visual Details

### 11.1 Waveform Rendering (`ui/deck-wave.slint` → `WaveStrip`)
- **Pixel-perfect**: `pixelated` rendering, no anti-aliasing
- **Stem layers**: 4 textures (vocal/inst/drums/bass) blended
- **Grid lines**: beat (white), phrase (lilac), downbeat (bright)
- **Cue markers**: colored triangles (hotcue colors)
- **Loop brace**: lilac bracket
- **Playhead**: white line + time
- **Phrase strip**: below waveform, phrase markers with type/character

### 11.2 Deck Wave (`ui/deck-wave-v.slint`) — Vertical variant
- **Vertical waveform** for touch/profile layouts
- Same features, rotated

---

## 12. Theme & Styling Tokens (`ui/theme.slint`)

### 12.1 Colors (Catppuccin Mocha/Latte base)
- **Base**: `bg` `#1e1e2e`, `panel` `#252530`, `panel-raised` `#2a2a37`, `bg-deep` `#181825`
- **Text**: `text` `#cdd6f4`, `text-dim` `#a6adc8`, `text-faint` `#7f849c`, `text-bright` `#ffffff`
- **Accent**: `accent` `#cba6f7` (mauve), `play` `#a6e3a1` (green), `automix` `#a6e3a1`, `danger` `#f38ba8`
- **Stem colors**: `stem-vocal` `#f38ba8`, `stem-inst` `#a6e3a1`, `stem-drums` `#f9e2af`, `stem-bass` `#89b4fa`
- **Waveform**: `wave-low` `#f38ba8`, `wave-mid` `#f9e2af`, `wave-high` `#a6e3a1`, `wave-fade` 0.5
- **Deck colors**: `deck1` `#89b4fa`, `deck2` `#f38ba8`, `deck3` `#a6e3a1`, `deck4` `#f9e2af`
- **Key colors**: `key-sat` 0.7, `key-val` 0.9, `key-far` `#6c7086`
- **Neutral grays**: `line` `#313244`, `line-strong` `#45475a`, `select` `#cba6f733`, `hover-row` `#313244`
- **Scrim**: `scrim` `#00000080`

### 12.2 Sizing Tokens
- `h-bar` 36px, `h-status` 24px, `h-ctl` 28px, `h-row` 28px, `h-header` 32px
- `touch-target` 44px (touch profile)
- `radius` 6px, `radius-lg` 10px, `radius-xl` 16px
- `font-xs` 10px, `font-sm` 12px, `font-md` 13px, `font-lg` 15px, `font-xl` 18px, `font-xxl` 24px
- `space-xs` 4px, `space-sm` 8px, `space-md` 12px, `space-lg` 16px, `space-xl` 24px
- `icon-size` 16px, `icon-size-lg` 24px

### 12.3 Layout Tokens
- `layout-profile`: 0=Desktop, 1=Touch, 2=Stage
- `stage-deck`: 0=Performance, 1=Turntable, 2=Controller, 3=Ring
- `stage-mirror`: 0/1
- `stage-tabs`: 0=Mixer/FX/Rec, 1=Mixer/Video/Lighting
- `stage-strip`: 0=Effects+Sampler, 1=Sampler only
- `stage-sidebar`: 0=Labels, 1=Icons only
- `stage-wave`: 0=Top, 1=Centre, 2=Decks
- `button-style`: 0=Filled, 1=Outlined, 2=Raised
- `bevel`: 0.0 (flat) to 1.0 (raised)
- `glow`: 0.0 to 1.0 (neon on active)
- `tempo-decimals`: 0/1/2

### 12.4 Light/Dark Variants
- `light` boolean (0=Dark/Mocha, 1=Light/Latte)
- All colors have both variants; tokens reference `root.light ? light-val : dark-val`

---

## 13. Gestures & Interactions

| Gesture | Target | Action |
|---------|--------|--------|
| Click | Track row / Device / Playlist | Select |
| Double-click | Track row / Playlist header | Load / Rename |
| Drag track | Deck / Palette / Device / Export | Load / Assign / Export |
| Right-click | Track row / Cue / Pad | Context menu (`TrackMenu`, `CueMenu`) |
| Mouse wheel | Waveform / Knob / Fader | Zoom / Adjust |
| Pinch (touch) | Waveform | Zoom |
| Drag | Waveform / Scrollbar / Fader | Scroll / Adjust |
| Long press | Pad / Sync / Device | Delete mode / Menu / Eject |
| Swipe (touch) | Waveform / Palette | Scroll / Switch tab |
| Escape | Dialog / Fullscreen / Overlay | Close / Exit |
| F / Double-click | Video window | Toggle fullscreen |

---

## 14. Keyboard Shortcuts (from `src/app/shortcuts.rs`)

| Key | Action |
|-----|--------|
| Space | Play/Pause focused deck |
| 1-8 | Trigger hot cue 1-8 |
| Shift+1-8 | Set hot cue 1-8 |
| Q/A/Z | Tempo -/+ (Deck 1/2/3) |
| W/S/X | Tempo -/+ large (Deck 1/2/3) |
| Left/Right | Seek ±5s |
| Shift+Left/Right | Seek ±30s |
| Up/Down | Browse library |
| Enter | Load selected track |
| Delete | Remove from playlist |
| Ctrl+F | Focus search |
| Ctrl+R | Record |
| M | Toggle master tempo |
| Y | Toggle sync |
| F | Toggle fullscreen (video) |
| Tab | Next mode (Library/Stage/Export/Video/Lighting) |
| Escape | Close dialog / Cancel |

---

## 15. Numbered UI Checklist (120+ items)

1. Main window with global bar (mode selector, transport, status chips)
2. Global bar: mode selector (Library/Stage/Export/Video/Lighting)
3. Global bar: DEVICES chip (lit when busy, opens DeviceWindow)
4. Global bar: ResourceChip (RAM/VRAM with tooltips)
5. Global bar: Settings, Controller status, Audio device status
6. Layout profiles: Desktop (0), Touch (1), Stage (2) via `layout-profile`
7. LIBRARY mode: BrowserTree left, TrackTable center, PaletteColumns right
8. BrowserTree: Folder kinds (collection, crate, playlist, smart, history, files)
9. BrowserTree: indicators (separator, stage badge, count, color, icon)
10. BrowserTree: click select, double-click rename, right-click menu, drag-drop
11. TrackTable: virtualized ListView, fixed columns, mini waveform, cover thumb
12. TrackTable: row states (missing=red strikethrough, duplicate=dim, streamed=badge)
13. TrackTable: key match highlight, sort by header click
14. TrackMenu: Play, Load to Deck 1-4, Automix, Sidelist, Edit Cues/Grid, Artwork, Delete
15. PaletteColumns: TAG LIST + 3 user playlists, header click/rename/pick, drop area
16. PaletteColumns: row click select, drag reorder, context menu
17. STAGE mode: StageDeck 1/2, StageMixer, StageStrip, StageLibrary
18. StageDeck: 4 styles (Performance, Turntable, Controller, Ring)
19. StageDeck: mirroring via `stage-mirror`
20. StageDeck: drop target for track loading
21. StageMixer: tabs (Mixer/FX/Rec | Mixer/Video/Lighting)
22. StageMixer Mixer tab: channel strips, master section, crossfader
23. StageMixer FX tab: effect rack per deck (3 slots), Beat FX center
24. StageMixer Rec tab: record controls, source, level
25. StageMixer Video tab: transition, overlay, output window
26. StageMixer Lighting tab: DMX fixtures, scenes
27. StageStrip: Effects+Sampler or Sampler only
28. StageLibrary sidebar: Labels or Icons only
29. StageWaveform position: Top, Centre, or Decks
30. EXPORT mode: Player layout selector (1/2/DUAL/SIMPLE PLAYER)
31. EXPORT sub-panels: GRID, CUES, DOCK tabs
32. Grid panel: PhraseTools (cut/retype/move phrases, type/character tags)
33. Grid panel: GridTools (BPM edit, ×2/÷2, shift nudge, tap, range, lock, undo/redo, metro)
34. Cues panel: cue list with time/len/name/comment/color, play modes, color palette
35. Dock panel: palette columns (TAG LIST + 3 playlists), drag-drop
36. VIDEO mode: master output 16:9, deck thumbs with weight, video library list
37. VIDEO controls: transition FADE/CUT, follow fader, overlay TEXT/IMAGE, opacity/size
38. VIDEO: output window (960×540), fullscreen toggle, overlay rendering
39. LIGHTING mode: stage simulator (3 rows: moving heads, strobes, pars)
40. LIGHTING: fixture patch list (name, profile, universe@address, color, conflict)
41. LIGHTING controls: protocol OFF/ART-NET/SACN/BOTH, IP, palette, mode, strobe, master
42. LIGHTING: profile stepper, add/remove fixture, user profiles from `fixtures-dir`
43. DeckHeader: Jacket (cover + stem badges), DeckTitle, DeckTimes, SyncCluster, StemRow, DeckTransport
44. DeckWaveform: Overview (full track, minute dots, cues, loops, automix), WaveStrip (enlarged, pixelated)
45. DeckWaveform: stem layers (4 colors toggleable), lyrics words (WaveLyrics)
46. DeckWaveform: phrase strip, scroll via drag/scrollbar/keys/touch
47. DeckControls: TempoColumn (fader, range, key lock, sync, master)
48. DeckControls: LoopBlock (In/Out, size, halve/double, move, reloop)
49. DeckControls: RingColumn (JogWheel touch, play/pause, cue, countdown)
50. DeckControls: RingButton (circular transport/cue/hotcue)
51. PadGrid: 8 pads, modes 0-11+ (Hot Cue, Pad FX, Slicer, Sampler, Sequencer, Stems, Plugin)
52. Desktop Mixer: TouchChannel (gain, 3EQ, filter/CFX, fader, cue, stem iso)
53. Desktop Mixer: BusChannel (master/headphones/booth level, split cue, record)
54. Desktop Mixer: CrossFader (curve, assign, slider), Meter (stereo VU, peak, clip)
55. Touch Mixer: horizontal faders/knobs, bus knobs, transport row, bottom bar
56. Sampler: 8 slots/deck, 2 banks, DrumSlotView (pad, state, mode, level)
57. Sampler: master gain, multi mode, capture (on/beats), release FX (effect/beat)
58. Sampler: presets (factory/user), save/load/step
59. Groove Box: Drum Swap 8 slots/deck, sequencer, kits
60. DeviceWindow: DialogFrame 900×780, tabs (DEVICES, EXPORT, SYNC, BACKUPS)
61. DEVICES tab: device list (name, icon, formats, status, capacity), Format/Backup/Restore/Eject
62. DEVICES tab: DeviceFormat options (Pioneer Device Library, OneLibrary, Engine DJ)
63. DEVICES tab: DevicePreview before write (add/update/blocked sections with lists)
64. EXPORT tab: device/folder picker, format chips, playlist PickList, EXPORT button
65. SYNC tab: toggle sync, format chips, playlist PickList, SYNC NOW
66. BACKUPS tab: backup folder, backup list table (date, computer, device, fs), BACKUP/RESTORE/DELETE
67. DevicePreview: DialogFrame 640px, sections with SHOW/HIDE lists, START/CANCEL
68. DeviceSyncAsk: DialogFrame 460px, "device plugged in: sync N playlists?", SYNC/NOT NOW
69. DeviceReview: full-screen overlay during write, progress + cancel, review rows (compare)
70. ArtworkPicker: DialogFrame 584×480, 4-col grid of candidates (thumb, source, size), click to choose
71. Preferences: PrefRows (Rust-owned), Controller page (Get mappings → Catalog), Keyboard page
72. Catalog: RepoBar (repos list, add/remove/refresh), GetPlugins, GetThemes, GetMappings
73. GetPlugins: name, version, installed, description, repo, native, action (INSTALL/UPDATE/INSTALLED)
74. GetThemes: name, author, description, repo, action, active, colors preview
75. GetMappings: controller, port, repo, action, search, install, auto-offer on connect
76. ImportReport: dialog with imported playlists/cues/grids/tracks/errors
77. UpdateDialog: version, changelog, RESTART NOW/LATER
78. Toast: ToastState global, ToastOverlay bottom-center, auto-dismiss, kind (info/warn/error)
79. Waveform rendering: pixel-perfect, stem layers blended, grid lines, cue markers, loop brace
80. DeckWave vertical variant for touch/profile layouts
81. Theme tokens: Catppuccin Mocha/Latte base, 20+ semantic colors
82. Theme tokens: stem/wave/deck/key colors, neutral grays, status colors
83. Theme tokens: sizing (heights, radii, fonts, spacing), touch-target 44px
84. Layout tokens: layout-profile (3), stage-deck (4), stage-mirror, stage-tabs (2), stage-strip (2), stage-sidebar (2), stage-wave (3)
85. UI tokens: button-style (3), bevel (0-1), glow (0-1), tempo-decimals (0-2), light/dark
86. Gestures: click, double-click, drag-drop, right-click, wheel, pinch, long-press, swipe
87. Keyboard shortcuts: 20+ keys for transport, tempo, cues, seek, library, modes
88. DialogFrame: scrim, centered card, title bar with close, body flickable, whole-pixel positioning
89. FormRow: label + controls (stacked in touch), hint text
90. Chip: active/enabled states, icon fallback, tooltip, fit-width, pressed/released callbacks
91. Tooltip: TipState global, Tooltip component (hover delay), TipOverlay (clamped to window)
92. TouchArea: mouse-cursor, clicked/pressed/released/double-clicked
93. DropArea/DragState: can-drop, dropped, drag-preview, absolute position tracking
94. PopupWindow: close-policy (click-outside/esc), positioning
95. Flickable: content-width/height, vertical/horizontal scroll, touch-friendly
96. ListView: virtualized, delegate components, item spacing
97. GridLayout/HorizontalLayout/VerticalLayout: spacing, alignment, stretch
98. Icon/Glyph: from `icons.slint`, used in Chip, IconButton, Icon
99. Gauges: Fader (horizontal/vertical), Knob (circular), Meter (segmented)
100. RingButton: circular button with icon, press animation, active state
101. JogWheel: touch-sensitive, vinyl/CDJ modes, countdown display, ring LED
102. SyncCluster: MASTER/SYNC/KEY SYNC chips with states
103. StemRow: 4 isolation chips (Vocal/Inst/Drums/Bass) with colors
104. DeckBadge: deck number (1-4), hidden deck indicator
105. DeckDropTarget: visual feedback when dragging track over deck
106. TrackRow: all metadata + mini waveform + cover + color + key match + analysing state
107. Folder node: kind-based icon, editable/adoptable/locked/bundle flags
108. CueRow: pad/loop, time, detail, name, color, comment, play mode
109. PaletteColumn header: click select tree, double-click rename, chevron pick playlist
110. PaletteColumn rows: click select table, drag reorder, context menu
111. DeviceItem: icon per player type (Pioneer/Engine/Denon), formats chips, capacity bar
112. DevicePreview sections: add/update/blocked with virtualized TrackList
113. ReviewOverlay: full-screen, progress, cancel, compare rows (rating/color/comment/history)
114. ArtworkCandidate: thumb, source, size; grid 4-col, hover highlight, click choose
115. Catalog RepoBar: repo name, note (count/failed), remove, add field, REFRESH
116. GetPlugin: native flag (downloads native lib), action states
117. GetTheme: colors preview, active highlight, apply action
118. GetMapping: search filter, install, auto-offer banner
119. Stage simulator: 3 rows, live DMX colors, glow shadows, conflict borders
118. Stage fixture patch: universe/address spinners, remove, conflict highlight
119. VideoOutput: 16:9 letterbox, overlay text/image, opacity/size
120. VideoWindow: separate window, double-click fullscreen, title/icon
121. TouchWorkspace: enlarged controls, bottom bar, gesture-friendly
122. DeckTouch: enlarged jog, pads, tempo, loop, transport
123. MixerTouch: horizontal faders, bus knobs, transport row
124. ExportMode player layouts: 1/2/DUAL/SIMPLE with GRID/CUES/DOCK
125. GridTools: Analysis Lock, Undo/Redo, Metronome (3 volumes)
126. PhraseTools: type/character dropdowns, cut/retype/move
127. Cues SubPanel: color palette popup (8 colors + custom), play mode dropdown
128. Dock palette: same as Library, integrated in Export mode
129. Low memory malloc (Linux): mallopt tuning in main.rs
130. Crash diagnostics: diag::init/finish, log file, crash reports
131. In-app update with rollback (--rollback)
132. Stick verify command (verify USB export)
133. Windows disk formatter (elevated --format-disk)
134. Resource monitor chips (RAM/VRAM) in global bar
135. Ableton Link (not in UI, engine-level)
136. Right-to-left language support (Slint built-in)
137. Accessibility: roles, labels, keyboard nav
138. High DPI scaling (Slint auto)
139. Adaptive layout (desktop/touch/stage) via single `layout-profile` token
140. Touch targets ≥44px enforced in touch profile

---

## Sources

- `/home/ubuntu/.hermes/cache/scratch/parity-src/openfader/ui/*.slint` (50+ Slint files)
- `/home/ubuntu/.hermes/cache/scratch/parity-src/openfader/ui/theme.slint` (400+ lines of tokens)
- `/home/ubuntu/.hermes/cache/scratch/parity-src/openfader/docs/*.md` (library-format, diagnostics, mappings, skins, backup-export-sync, android, raspberry-pi, releases, plugins, code-structure)
- `/home/ubuntu/.hermes/cache/scratch/parity-src/openfader/src/app/*.rs` (UI state globals)
- `https://github.com/OpenRsG/OpenFadeR`
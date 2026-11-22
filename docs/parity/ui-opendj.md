# OpenDJ UI Specification (Screen-by-Screen)
Status: generated from source reading by research agents; each item must be checked against the source before it is built.

Based on source code at `/home/ubuntu/djsugar-rust/crates/app/ui/*.slint` (Slint UI files) and supporting Rust logic. Every screen, region, panel, control, popup, menu, gesture, color/size token is documented.

---

## 1. Global UI Structure

### 1.1 Main Window (`main.slint`)
- **Window**: `MainWindow` — full-screen or resizable, no title bar on Windows (`windows_subsystem = "windows"`)
- **Background**: `Theme.bg` (dark: `#101012`, light: mapped)
- **Root layout**: `VerticalLayout` with spacing 0
- **Top bar** (height `Theme.h-bar` = 36px): `TopBar` component
- **Content area**: `HorizontalLayout` split into:
  - Left sidebar (library browser): `Library` component, width `Theme.sidebar-width` (260px), collapsible
  - Center workspace: `Workspace` component (deck layouts)
  - Right sidebar (side panel): `SidePanel` component, width `Theme.sidebar-width` (260px), collapsible
- **Bottom status bar** (height `Theme.h-status` = 24px): `StatusBar` component

### 1.2 Top Bar (`toppanels.slint` to `TopBar`)
- **Left group**: Layout menu (LayoutsUi), Library toggle, Sidebar toggle
- **Center group**: Global transport (Automix, Sync, Quantize, Master tempo), Clock/BPM display
- **Right group**: FX bar toggle, Video/Lighting/Extensions buttons (when enabled), REMOTE button, Mini player button, Settings, Controller status, Audio device status, Resource chips (RAM/VRAM)

### 1.3 Status Bar (`main.slint`)
- **Left**: Drop zone indicator, Sync status, Recording status
- **Center**: Master VU meter (stereo), Crossfader position
- **Right**: CPU/RAM usage, Time, Notifications

---

## 2. Workspace Layouts (`layouts.slint`, `fourdeck.slint`, `player.slint`, `prep.slint`, `lite.slint`)

### 2.1 Layout Selector (`LayoutsUi` global)
- **Menu button** (pencil icon): opens chooser popup
- **Chooser popup**: list of `LayoutEntry` (layout id, name, detail, on/off, locked)
  - Drag to reorder (`moved` callback)
  - Toggle on/off (`toggled`)
  - Reset to defaults (`reset`)
- **Current layout** (`current`): 0=2-Deck, 1=Player, 2=4-Deck, 3=Prep, 4=2-Deck Basic
- **Appearance picker** (0=Dark, 1=Light, 2=Auto): `appearance-picked`

### 2.2 2-Deck Layout (`deck.slint` + `mixer.slint` center)
```
┌─────────────────────────────────────────────────────────────┐
│ Deck A (left)                    Deck B (right, mirrored)  │
├─────────────────────────────────────────────────────────────┤
│                    MIXER (center)                           │
├─────────────────────────────────────────────────────────────┤
│                    FX BAR (when shown)                      │
└─────────────────────────────────────────────────────────────┘
```
- **Deck component** (`Deck`): header, tempo column, pads, loop controls, jog, mixer strip
- **Mixer**: `Mixer` component (3-band EQ, gain, faders, crossfader, master meter)
- **FX Bar**: `FxBar` component (3 effects/deck + Beat FX)

### 2.3 4-Deck Layout (`fourdeck.slint`)
- Two rows of decks (1-2 top, 3-4 bottom) with shared mixer/FX
- Deck 3/4 headers show "3" / "4" badge
- Deck layer switching via controller DECK 3/4 button

### 2.4 Player Layout (`player.slint`)
```
┌─────────────────────────────────────────────────────────────┐
│ NOW PLAYING (large art, title, artist, album, quality)     │
├─────────────────────────────────────────────────────────────┤
│ LYRICS / DETAILS tabbed panel                               │
├─────────────────────────────────────────────────────────────┤
│ UP NEXT (sidelist)              │  DECK (single, large)    │
└─────────────────────────────────────────────────────────────┘
```
- **NowPlayingView**: large cover (gradient tint), title/artist/album, BPM/key/quality, Details list, Lyrics tab (PlayerLyrics)
- **Up Next**: `SidelistPanel` (queue, drag reorder, auto-play)
- **Deck**: enlarged single deck with all controls

### 2.5 Prep Layout (`prep.slint`)
```
┌─────────────────────────────────────────────────────────────┐
│ LIBRARY (full width, no decks)                             │
├─────────────────────────────────────────────────────────────┤
│ DECK 1 (waveform, cues, grid, key)  DECK 2 (same)          │
└─────────────────────────────────────────────────────────────┘
```
- Library occupies top 60%, two prep decks bottom
- No mixer, no FX bar, no crossfader
- Deck headers show cover, title, BPM, key, cues panel on right

### 2.6 2-Deck Basic Layout
- Simplified: no pads, no loop controls, no jog, minimal mixer
- For beginners / small screens

### 2.7 Lite Mode (`lite.slint`)
- Minimal window: single deck, no library, no side panel
- For background play / secondary monitor

---

## 3. Deck Component (`deck.slint`)

### 3.1 Deck Header (`DeckInfo` struct to rendered in `Deck`)
| Element | Description |
|---------|-------------|
| Cover art | 80x80px, rounded 8px, click to load track, drag from library |
| Title | Track title, ellipsis, font-size 15px, weight 600 |
| Artist | Track artist, dim color, font-size 12px |
| BPM | "126 BPM", click for grid menu, font-mono 13px |
| Key | "8A" or "Am", click for key shift menu, colored by Camelot wheel |
| Tempo fader value | "+3.45%" (or range), click to reset |
| Key shift badge | "-1" when shifted, click for menu |
| Grid source badge | "Detected" / "Edited" / "From VirtualDJ" |
| Grid unsure warning | Yellow warn when grid confidence low |
| Stem status | "Stems: Vocal, Drums, Bass" + "Preparing HQ..." spinner |
| Progress bar | Thin line under header, 0..1, shows automix region (lilac) |
| Track end warning | Remaining time blinks red, jog ring flashes deck color |

### 3.2 Tempo Column (outer edge)
- **Tempo fader** (`ControlFader`): vertical, 100px travel, tick marks every 10%
  - Range selector: +-6% / +-10% / +-16% / +-25% / +-50% / +-100% (dropdown)
  - Double-click to reset to 0%
  - Hardware pickup: ghost line shows hardware position
- **Tempo display**: large BPM value, decimal places per `Theme.tempo-decimals` (0/1/2)
- **Master tempo (key lock)**: toggle button, lit when on
- **Sync button** (`SyncButton`): lit when synced, long-press for sync menu
- **Key lock sync**: when both decks have key lock, sync matches key too

### 3.3 Pads (8 hot cues)
- **Pad grid**: 2x4 or 4x2 depending on deck mirroring
- **Pad states**: empty (+), hot cue (colored), loop (colored + loop icon), playing (pulse)
- **Right-click pad** to context menu:
  - Choose Sample (sampler mode)
  - Load file
  - Rename
  - Play mode (Oneshot, Hold, Stutter, Loop)
  - Level
  - Sample pack submenu
- **Pad modes** (12+): Hot Cue, Pad FX, Slicer, Sampler, Sequencer Call, Stems, Plugin...
- **Delete mode**: x button on each pad, click to clear

### 3.4 Loop Controls
- **Loop In / Out buttons**: set loop start/end at playhead
- **Loop size display**: "4 bars" / "1/2 beat" etc.
- **Halve / Double buttons**: ÷2 / x2 loop length
- **Move loop <-/->**: shift loop by its length
- **Saved loops**: appear in Cues panel as memory cues (pad = -1)

### 3.5 Jog Wheel
- **Outer ring**: touch-sensitive, scratch when vinyl mode
- **Inner display**: BPM / cue countdown / slip indicator / time
- **Play/Pause button**: large center, `ControlButton`
- **Cue button**: under jog, `ControlButton`, hold to preview
- **Jog modes**: `jog-style` token (0=CDJ, 1=Vinyl, 2=Auto)
- **Countdown**: shows beats to next cue when paused

### 3.6 Mixer Strip (inner edge, beside jog)
- **Gain knob** (`ControlKnob`): -24..+12 dB
- **3-band EQ** (High/Mid/Low): `ControlKnob` each, center detent at 0
  - Kill switches: on controller only (not on screen)
- **Filter/Color FX knob**: dual-mode (filter <-> Color FX), click toggles
- **Channel fader** (`ControlFader`): 60mm travel, tick marks, dB scale
- **Cue button** (`CueHeadphones`): headphone icon, lit when cued
- **Stem isolation buttons**: 4 small chips (Vocal, Inst, Drums, Bass)

---

## 4. Mixer (`mixer.slint`)

### 4.1 Center Mixer Layout
```
[EQ High] [EQ Mid] [EQ Low]   [GAIN]   [CH FADER]   [MASTER VU]   [CH FADER]   [GAIN]   [EQ Low] [EQ Mid] [EQ High]
                                                                              [CROSSFADER]
```
- **Left/Right mirrored**: EQ columns outside, gain/faders inside
- **Master meter** (`StereoMeter`): stereo, 30 segment, peak hold, clip LED
- **Booth meter**: shown when booth output configured (small, right of master)
- **Crossfader** (`Crossfader`): 45mm, curve selector (Smooth/Sharp/Constant), assign A/Thru/B
- **Master level knob**: below crossfader
- **Record button**: red when recording, shows duration

### 4.2 Audio Settings Dialog (`AudioSettings` global)
- **Output routes table** (`OutputRow`):
  - Role: Master / Headphones / Booth / Deck 1-4 / Sampler
  - Device dropdown (system default + connected)
  - Channel pair dropdown (1/2, 3/4, etc.)
  - Status (OK / Missing / Busy)
- **Input routes table** (`InputRow`):
  - Microphone, Auxiliary, Auxiliary 2
  - Device + channel selection
  - Add/remove auxiliary inputs
- **Sample rate / Buffer size** menus (per output device)
- **External mixer mode**: fixed rows per deck, no crossfader

---

## 5. FX Bar (`fxbar.slint`)

### 5.1 Layout (full width under decks)
```
[Deck 1 FX 1] [Deck 1 FX 2] [Deck 1 FX 3]   [BEAT FX]   [Deck 2 FX 1] [Deck 2 FX 2] [Deck 2 FX 3]
```
- **2-Deck Basic**: each effect = 2-row cell (name stepped by ‹ › arrows)
- **4-Deck**: each deck's 3 effects stacked in 1-line rows

### 5.2 Per-Effect Controls
- **Effect name** (`EffectMenu`): dropdown with all available effects (built-in + plugins)
- **On/Off button** (`ControlButton`): lit when active
- **Beat length chooser** (`BeatChooser`): ‹ 1/4 › (1/16 .. 4 beats, powers of 2)
- **Parameter slider** (`ParamSlider`): thin track, 5 ticks, round thumb, hardware pickup ghost
- **Dry/Wet knob** (`ControlKnob`): main parameter (`meta`), controller-mapped

### 5.3 Beat FX (center, mixer-width)
- Same controls but global (not per-deck)
- Effect types: Echo, Reverb, Flanger, Delay, etc.

---

## 6. Library Browser (`library.slint`)

### 6.1 Folder Tree (left sidebar)
- **TreeView** (`ListView`): `Folder` struct items
- **Folder kinds** (icon + behavior):
  - 0=Collection root, 1=List folder, 2=Playlist, 3=Smart list, 4=Liked/Wanted
  - 5=History session, 6=Bandcamp, 17=Crate, 18=Heading, 19=History day
  - Files view: 1=Folder, 7=Drive, 8=Pinned, 9=Cloud
- **Indicators**: stage badge (Inbox/In Prep/Play Ready), track count, color dot, icon
- **Interactions**:
  - Click to select, show tracks
  - Right-click to context menu (New playlist, New crate, Rename, Delete, Import, Analyze All)
  - Drag tracks onto folder to add to crate/playlist
  - Drag folder to reorder (crates/playlists only)
  - Expand/collapse chevron

### 6.2 Track List (center)
- **Columns** (`ColumnUi` global): user-configurable, right-click header to pick columns
  - Default: #, Title, Artist, Album, Genre, Length, BPM, Key, Rating, Color, Waveform
- **Column resize**: drag right edge of header
- **Column reorder**: drag header
- **Sort**: click header (multi-column with Shift)
- **TrackRow** struct: all metadata + mini waveform + cover thumbnail
- **Row colors**: `color-mode` token (0=dot, 1=edge strip, 2=row tint)
- **Key match highlight**: green tint when harmonically compatible
- **Missing file**: red strikethrough, `missing` flag
- **Duplicate**: faded (`fade` flag)
- **Context menu**: Play, Load to Deck 1/2, Add to Automix, Add to Sidelist, Edit cues, Edit grid, Find artwork, Show in folder, Delete, Properties

### 6.3 Filter Bar (`filters.slint` to `FilterUi` global)
- **Chip row** above track list: one `FilterChip` per active filter
  - Chip shows field + value ("Genre: House", "BPM: 120-130")
  - Click chip to popup with `FilterChoice` list (multi-select, counts)
  - Pin chip (stays when clearing others)
- **"+" button** to "Add Filter" menu (core fields, tag groups, more)
- **Finder**: type in search box to `SuggestionList` shows matching values across all fields
- **Tempo/Key follow**: chip can "Follow playing track" (auto-match)
- **Match all/any** toggle for multi-value filters

### 6.4 Side Panel (right sidebar) — tabs
| Tab | Component | Content |
|-----|-----------|---------|
| Automix | `AutomixPanel` | Queue, reorder, settings (crossfade, phrase align) |
| Sidelist | `SidelistPanel` | Queue, drag reorder, save as playlist |
| Cues | `CuesPanel` | Hot cues, memory cues, loops of selected/playing track |
| Lyrics | `LyricsPanel` | Synced/plain lyrics, LRCLIB find, karaoke launch |
| Info | `InfoPanel` | Full track metadata, file quality, analysis, comments |
| Instruments | `InstrumentsPanel` | Groove Box, plugins, mic, aux — drag to deck |
| Checklist | `ChecklistPanel` | Collection health (missing, duplicates, tags) |

---

## 7. Side Panel Components

### 7.1 Cues Panel (`cues.slint`)
- **Deck selector**: Deck 1 / Deck 2 / Prep (radio)
- **Cue list** (`CueList` global): rows = `CueRow` (pad 0-7 or -1=memory, loop flag, time, detail, name, color)
- **Per-row controls**:
  - Pad number (click to menu: move to pad 1-8, make memory, delete)
  - Name (inline edit, `NameEdit`)
  - Color dot (click to 8-color palette + custom)
  - Go button (jump to cue)
- **Buttons**: Add memory cue, Save loop (when looping)

### 7.2 Lyrics Panel (`lyrics.slint`)
- **Subject selector**: Playing deck / Selected track / Deck A / Deck B
- **Lyrics display**: `ListView` of `LyricLine` (time + text)
  - Current line highlighted (lilac), auto-scroll
  - Click line to seek (if `can-seek`)
- **Source badge**: "Timed by word · Song.lrc" / "Plain · Tags"
- **Find lyrics**: button to searches LRCLIB, shows `LyricMatch` list (synced/plain/instrumental, fits duration)
- **Karaoke button** to opens `KaraokeWindow`
- **Vocals slider**: guide vocal level (stem separation)
- **Time words / Transcribe**: Whisper alignment for untimed lyrics

### 7.3 Instruments Panel (`instruments.slint`)
- **Tree** (`InstrumentNode`): Groove Box, Plugins (by maker), Microphone, Aux Inputs, Looper
- **Presets** (`PresetUi`): per-deck preset list, tags, step/save/load
- **Mixer strip picker** (`PickerRow`): aux inputs with meters, click to assign to deck

### 7.4 Info Panel (`info.slint`)
- **TrackInfo** struct: all metadata tags, file info, analysis data
- **Editable fields**: title, artist, album, genre, comment, rating, color
- **Artwork**: current cover, "Find Artwork" button to `ArtworkPicker`
- **Analysis**: BPM, key, grid sections, grid source, duration

### 7.5 Checklist Panel (`checklist.slint`)
- **Sections**: Missing files, Duplicates, Poor tags, Unanalyzed
- **Each item**: count, "Fix" button (batch action), "Show" to filters library

---

## 8. Popups & Dialogs

### 8.1 Settings Dialog (`preferences.slint` to `PrefRows` from `prefs-state.slint`)
- **Tabs** (left rail): Decks & Controllers, Audio, Library, Streaming, Plugins, Themes, Extensions, Appearance, View, Advanced
- **Each tab**: `PrefRow` items (switch, slider, dropdown, button, file picker)
- **Controller Editor** button to opens `ControllerEditor` (full-screen overlay)

### 8.2 Controller Editor (`controller_editor.slint`)
- **Left**: Controller picture (SVG) with `EditorPart` hotspots (x,y,w,h, kind, mapped, lit, flash)
- **Right**: Controls list (`EditorRow`): grouped by function (Deck, Pads, Mixer, FX, Transport...)
  - Level 0=group, 1=category, 2=control
  - Expand/collapse groups
  - Search filter
  - Click row to edit panel below (function, target, control, layer, MIDI, script, detail)
- **Mapping versioning**: Original / User copy 1 / User copy 2 / Duplicate / Rename / Delete
- **Options tab**: `EditorSetting` (2/4 decks, deck order, takeover mode)
- **Map mode** (`MapMark`): press hardware control to highlights row, shows MIDI message

### 8.3 Choose Sample Popover (`sampler.slint`)
- **Pack selector**: horizontal scroll of `PackCover` (cover art or note initial)
- **Sample list**: `SampleChoice` rows (name, length), click to assign to slot
- **Preview**: headphone button per row
- **Pack actions**: New pack, Rename, Delete, Show in folder, Export (.djsugarpack), Import

### 8.4 Artwork Picker (`sources.slint` / artwork logic)
- **Grid** of `ArtCandidate` thumbnails (source + size)
- **Click** to choose, downloads and saves
- **Status**: "Asking MusicBrainz...", "No artwork found"

### 8.5 USB Export Dialog (`usb_export.slint`)
- **What**: playlist/folder name + track count
- **Drive picker**: dropdown of mounted drives
- **Format chips**: Pioneer Device Library / OneLibrary / Engine DJ / All
- **Progress bar** + cancel
- **Result**: success/fail with details

### 8.6 Mini Player Window (`mini.slint`)
- **Window**: no frame, always-on-top, square (min 200x200, pref 300x300)
- **Drag**: click art to move window
- **Resize**: corner grip to resize (stays square)
- **Art**: fills window, gradient tint from cover colors
- **On hover**: art dims, shows x (expand), title, artist, progress bar, transport (prev/play/next)
- **Progress**: track progress + automix next mix region (lilac)
- **Callbacks**: play-pause, previous, next, seek, expand, start-move, start-resize

### 8.7 Karaoke Window (`karaoke.slint`)
- **Window**: 1280x720 pref, min 480x270, black background (`#08080a`)
- **Title/artist** top small
- **State 0**: "DJ Sugar Karaoke" + "Lyrics show here"
- **State 1**: "No lyrics for this track" (lilac)
- **State 2**: "These lyrics aren't timed" (dim)
- **State 3 (lyrics)**: 
  - Before line (dim)
  - Current line: sung part (lilac fill by `part` 0..1), word being sung, rest
  - Next line, After line
  - Count-in dots (1-4 beats before line)
- **Keys**: Esc to leave fullscreen, F / double-click to toggle fullscreen

### 8.8 Guided Tour (`tour.slint` + `tourmark.slint`)
- **Setup dialog** (`TourSetupDialog`): left rail parts (Audio, Controller, Library), right content (title, body, `PrefRows`)
- **Parts**: dots with checkmarks when passed
- **Buttons**: Back, Skip (optional), Next
- **Lesson tips** (`TourMark`): anchored to UI elements, arrow pointing, text, "Got it" button

---

## 9. Remote Panel (`remote.slint`)

### 9.1 Remote Button (top bar)
- **Icon**: phone outline
- **States**: off (gray), on no phone (blue), on connected (green)
- **Click** to opens `RemotePanel` (popup)

### 9.2 Remote Panel (`RemotePanel`)
- **Header**: "Phone remote", Switch (on/off)
- **Error banner** (red) if failed to start
- **QR code**: large, scans to pair phone
- **URL**: displayed below QR (manual entry)
- **PIN**: 6-digit code (alternative to QR)
- **Other URLs**: other network interfaces
- **Paired devices list**: `DeviceRow` (name, connected dot, last seen, Remove button)
- **Code refresh**: countdown timer (seconds until QR renews)

---

## 10. Waveform & Visual Details

### 10.1 Waveform Look (`wave_look.slint` to `WaveLook` global)
- **Color bands**: `wave-low`, `wave-mid`, `wave-high` (theme tokens)
- **Fade parameter**: `wave-fade` 0..1 (blends bands)
- **Grid lines**: beat (white), phrase (lilac), downbeat (bright)
- **Cue markers**: colored triangles (hotcue colors), memory cues (white M)
- **Loop brace**: lilac bracket over loop region
- **Automix region**: lilac highlight on overview
- **Playhead**: white line with time label
- **Zoom**: mouse wheel / pinch / zoom buttons (+-)
- **Scroll**: drag waveform / scrollbar / keyboard

### 10.2 Deck Waveform (`deck.slint` overview + enlarged)
- **Overview**: full track, 200px wide, progress fill, minute dots
- **Enlarged**: scrolling, 30s window, centered on playhead
- **Stem waveforms**: 4 colored layers (Vocal/Inst/Drums/Bass) toggleable
- **Lyrics on waveform**: `WaveLyrics` global, words at word timestamps

---

## 11. Theme & Styling Tokens (`theme.slint`)

### 11.1 Colors (named by Dark value, Light auto-inverted via `n-*` tokens)
- **Primary**: `accent` = `#5b3fd1` (lilac), `lilac-text` = `#e8dfff`
- **Deck colors**: `deck1` `#00b8d4`, `deck2` `#ff6b5e`, `deck3` `#4cd07d`, `deck4` `#ffd60a`
- **Hotcue colors** (8): `#0064c8`, `#04bc00`, `#dcb800`, `#c45800`, `#c0002c`, `#c42874`, `#7838c4`, `#10bcbc`
- **Stem colors**: `stem-vocal` `#ff6b5e`, `stem-inst` `#4cd07d`, `stem-drums` `#ffd60a`, `stem-bass` `#00b8d4`
- **Waveform bands**: `wave-low` `#ff6b5e`, `wave-mid` `#ffd60a`, `wave-high` `#4cd07d`
- **Camelot key colors** (12): teal->green->lime->yellow->orange->red->pink->purple->blue->cyan
- **Neutral grays** (`n-*`): 50+ tokens from `#000000` to `#ffffff` with alpha variants
- **Status colors**: `automix` `#30d158`, `danger` `#ff453a`, `well` `#0d0d0e`

### 11.2 Sizing Tokens
- `h-bar` 36px, `h-status` 24px, `h-ctl` 28px, `h-row` 28px, `h-header` 32px
- `radius` 6px, `radius-lg` 12px, `radius-xl` 16px
- `font-xs` 10px, `font-sm` 12px, `font-md` 13px, `font-lg` 15px, `font-xl` 17px, `font-xxl` 24px
- `space-xs` 4px, `space-sm` 8px, `space-md` 12px, `space-lg` 16px, `space-xl` 24px
- `tooltip-delay` 800ms, `t-fast` 120ms, `t-normal` 200ms

### 11.3 Layout Tokens (Desktop vs Touch)
- `layout-profile`: 0=Desktop, 1=Touch (affects control heights, spacing, popup sizes)
- `stage-deck`: 0=Performance, 1=Turntable, 2=Controller, 3=Ring
- `stage-mirror`: 0/1 (mirror right deck)
- `stage-wave`: 0=Top, 1=Centre, 2=Decks
- `stage-strip`: 0=Effects+Sampler, 1=Sampler only
- `stage-sidebar`: 0=Labels, 1=Icons only

---

## 12. Gestures & Interactions

| Gesture | Target | Action |
|---------|--------|--------|
| Click | Track row | Select |
| Double-click | Track row | Load to active deck |
| Drag track | Deck header / Pad / Sampler slot | Load / Assign |
| Drag track | Crate/Playlist in tree | Add to list |
| Right-click | Track row / Pad / Cue / Column header | Context menu |
| Mouse wheel | Waveform / Tempo fader / EQ knob | Zoom / Adjust |
| Pinch (touch) | Waveform | Zoom |
| Drag | Waveform / Scrollbar | Scroll |
| Long press | Sync / Pad / Jog | Secondary action (menu / delete mode) |
| Double-click | Tempo fader / Jog display | Reset / Toggle mode |
| Shift+Click | Column header | Multi-column sort |
| Ctrl/Cmd+Click | Track rows | Multi-select |
| Escape | Any popup / Fullscreen | Close / Exit |
| F / Double-click | Karaoke / Video window | Toggle fullscreen |

---

## 13. Keyboard Shortcuts (from `controls.rs` / `shortcuts.rs`)

| Key | Action |
|-----|--------|
| Space | Play/Pause focused deck |
| Shift+Space | Play/Pause other deck |
| 1-8 | Trigger hot cue 1-8 (Deck A) |
| Shift+1-8 | Set hot cue 1-8 (Deck A) |
| Q/W | Deck A tempo -/+ (small) |
| A/S | Deck A tempo -/+ (large) |
| Z/X | Deck B tempo -/+ (small) |
| / | Deck B tempo -/+ (large) |
| Left/Right | Seek +-5s (focused deck) |
| Shift+Left/Right | Seek +-30s |
| Up/Down | Browse library |
| Enter | Load selected track to deck |
| Delete | Remove track from playlist |
| Ctrl+F | Focus search |
| Ctrl+R | Record |
| M | Toggle master tempo (key lock) |
| Y | Toggle sync |
| F | Toggle fullscreen (karaoke/video) |
| Esc | Close dialog / Exit fullscreen |

---

## 14. Numbered UI Checklist (120+ items)

1. Main window with top bar, content area, status bar
2. Top bar: layout menu, library toggle, global transport, clock
3. Top bar: FX toggle, Video/Lighting/Extensions, Remote, Mini, Settings
4. Status bar: drop zone, sync, recording, master VU, crossfader, CPU/RAM
5. Layout selector menu with chooser (enable/disable, reorder, reset)
6. Appearance picker (Dark/Light/Auto) in layout menu
7. 2-Deck layout: Deck A, Deck B (mirrored), center mixer, FX bar
8. 4-Deck layout: 2 rows of decks, shared mixer/FX
9. Player layout: Now Playing large, Lyrics/Details tabs, Up Next, single deck
10. Prep layout: Library top, two prep decks bottom
11. 2-Deck Basic layout: simplified controls
12. Lite mode: minimal single-deck window
13. Deck header: cover, title, artist, BPM, key, tempo, grid source, stem status
14. Deck header: progress bar with automix region highlight
15. Deck header: track end warning (blink remaining, jog ring flash)
16. Tempo column: fader with range selector, double-click reset
17. Tempo column: Master tempo (key lock) toggle
18. Tempo column: Sync button with long-press menu
19. Pads: 8 hot cues, colored, right-click menu (load, rename, play mode, level, pack)
20. Pad modes: 12+ (Hot Cue, Pad FX, Slicer, Sampler, Sequencer, Stems, Plugin)
21. Delete mode: x on pads to clear
22. Loop controls: In/Out, size display, halve/double, move
23. Saved loops appear in Cues panel as memory cues
24. Jog wheel: touch-sensitive, vinyl/CDJ modes, countdown display
25. Jog wheel: Play/Pause center, Cue button under
25. Mixer strip: Gain, 3-band EQ, Filter/Color FX, Channel fader, Cue, Stem isolation
26. Center mixer: EQ outside, gain/faders inside, master VU between, crossfader below
27. Crossfader: curve selector (Smooth/Sharp/Constant), assign A/Thru/B
28. Master level, Record button with duration
29. Booth meter when configured
30. Audio settings dialog: output routes, input routes, sample rate/buffer
31. External mixer mode: fixed rows per deck
32. FX bar: 3 effects/deck + Beat FX center, 2-Deck Basic / 4-Deck variants
33. Per-effect: name menu, On/Off, beat length chooser, param slider, dry/wet knob
34. Library: folder tree with kinds (collection, crates, playlists, smart, history, files)
35. Tree: stage badges, track counts, colors, icons, drag-drop reorder
36. Track list: configurable columns, resize, reorder, multi-sort
37. Track rows: mini waveform, cover thumb, color coding, key match, missing/duplicate flags
38. Filter bar: chips per filter, popup choices, pinned, finder suggestions
39. Filter bar: tempo/key follow playing track, match all/any toggle
40. Side panel tabs: Automix, Sidelist, Cues, Lyrics, Info, Instruments, Checklist
41. Cues panel: deck selector, cue list with pad number, name edit, color picker, go, add memory/save loop
42. Lyrics panel: subject selector, synced scroll, click seek, LRCLIB find, karaoke launch, vocals slider, time/transcribe
43. Instruments panel: tree (Groove Box, plugins, mic, aux, looper), presets with tags, mixer strip picker
44. Info panel: full metadata, editable fields, artwork picker, analysis details
45. Checklist panel: missing, duplicates, poor tags, unanalyzed with batch fix
46. Settings dialog: 10 tabs, PrefRows from Rust, Controller Editor button
47. Controller Editor: SVG picture with hotspots, controls list grouped, edit panel, versioning, options, Map mode
48. Choose Sample popover: pack scroll, sample list with preview, pack actions (new/rename/delete/export/import)
49. Artwork picker: grid of candidates with source/size, click to download
50. USB Export dialog: what, drive picker, format chips, progress, result
51. Mini player window: frameless, always-on-top, square, drag art, resize grip
52. Mini player: hover reveals close, title, artist, progress (with automix), transport
53. Karaoke window: 2nd display, fullscreen, word-level highlight, count-in dots, Esc/F keys
54. Guided tour: setup dialog (parts rail, PrefRows), lesson tips anchored to UI
55. Remote panel: QR code, URL, PIN, paired devices with remove, code refresh timer
56. Waveform: 3-band color, fade param, grid lines (beat/phrase/downbeat), cue markers, loop brace, automix highlight
57. Waveform: stem layers toggle, lyrics words on waveform
58. Waveform zoom: wheel/pinch/buttons, scroll: drag/scrollbar/keys
59. Theme tokens: 50+ neutral grays, deck/hotcue/stem/wave/key colors, status colors
60. Theme tokens: sizing (heights, radii, fonts, spacing), layout-profile (desktop/touch)
61. Layout tokens: stage-deck (4 styles), stage-mirror, stage-wave, stage-strip, stage-sidebar
62. Gestures: click, double-click, drag-drop, right-click, wheel, pinch, long-press, keyboard modifiers
63. Keyboard shortcuts: 25+ mapped keys for transport, tempo, cues, seek, library, record
64. Tooltip system: TipState global, Tooltip component, TipOverlay, delay toggle
65. Popup system: PopupWindow, PopupBackground, close-on-click-outside
66. Dialog frame: scrim, centered card, title bar with close, body flickable
67. FormRow: label + controls (stacked in touch), hint text
68. Chip button: active/enabled states, icon fallback, tooltip, fit-width
69. ControlKnob: value arc, hardware pickup ghost, mouse drag, touch
70. ControlFader: tick marks, dB scale, hardware pickup, mouse/touch drag
71. ControlButton: press animation, active/enabled states, map mark for MIDI learn
72. SyncButton: lit when synced, long-press menu
73. VuMeter/StereoMeter: segment bars, peak hold, clip LED, smooth animation
74. Crossfader: curve visualization, assign indicators
75. KeyNotation global: Camelot/Note toggle, click deck key to switch
76. TourMark: anchored tips with arrow, "Got it" dismiss
77. MapMark: shows MIDI learn hint on controls
78. EdgeLine/EdgeVLine: full-width dividers covering padding
79. CloseIcon: drawn x (font fallback)
80. Arc component: 3-segment arcs for jog/circular progress
81. GridMenuItem: hover highlight, click callback for grid menu
82. BeatChooser: halve/double arrows, beat display (1/4, 1/2, 1, 2, 4)
83. ParamSlider: tick marks, thumb, hardware ghost, drag
84. PackCover: image or note initial on lilac
85. MenuItem/MenuSeparator: checked, enabled, danger states
86. CueNumber: pad number or M, colored background, click to menu
87. NameEdit: inline TextInput, commit on enter/blur
88. SmallButton: lit/primary states, hover/press
89. Tab component: underline indicator, hover/active states
90. PhoneIcon: drawn SVG path for remote
91. PartRow (tour): dot with checkmark, current/passed states
92. StageDeck: 4 styles (Performance, Turntable, Controller, Ring), mirrored, drop target
93. StageGeom: natural height, jog/pads/waveform sizing
94. StagePerformance: waveform top, pads middle, jog bottom, transport
95. StageTurntable: large jog center, waveform ring, pads around
96. StageController: mixer-style, faders, knobs, buttons
97. StageRing: circular waveform, jog ring, minimal
98. StageMixer: tabs (Mixer/FX/Rec, Mixer/Video/Lighting), strip (Effects+Sampler)
99. StageLibrary: sidebar labels/icons, collapsible
100. VideoWindow: 16:9 output, overlay (text/image), fullscreen toggle
101. LightingView: fixture stage (3 rows), patch list, protocol (Art-Net/sACN), scenes
102. ExportMode: Player layouts (1/2/DUAL PLAYER, Simple), Grid/Cues/Dock sub-panels
103. ExportMode Grid: phrase tools (cut, retype, move), grid tools (BPM, shift, tap, x2/÷2, range, lock, undo/redo, metro)
104. ExportMode Cues: cue list with comments, color palette, play modes
105. ExportMode Dock: palette columns (tag lists), drag-drop, rename
106. TouchWorkspace: enlarged controls (44px), gesture-friendly, bottom bar
107. DeckTouch: enlarged jog, pads, tempo, loop, transport
108. MixerTouch: horizontal faders, bus knobs, transport
109. Low memory malloc (Linux): mallopt tuning for RSS reduction
110. Crash diagnostics: diag::init/finish, log file, crash reports
111. In-app update with rollback (--rollback flag)
112. Stick verify command (verify USB export integrity)
113. Windows disk formatter (elevated --format-disk)
114. Resource chips: RAM/VRAM in top bar (ResourceState)
115. Waveform pixelated rendering (WaveStrip, pixel-perfect)
116. Stem separation UI: instant stems on load, HQ preparing spinner, isolation buttons
117. Harmonic mixing: key match highlight in library, key shift menu with compatibility
118. Collection Health: missing files, duplicates, poor tags, batch fix
119. Import wizard: rekordbox/VirtualDJ/Traktor/Mixxx with cues/grids/lists
120. Streaming source browsing: Spotify/TIDAL/Apple Music/SoundCloud/Mixcloud + Plex/Navidrome/Jellyfin
121. Saved searches per streaming source
122. Download from streaming to cache with preview
123. Automix: queue, phrase-aware, phone remote control
124. Recording: file formats, loudness meter, Icecast/Shoutcast, OBS overlay
125. 200+ Mixxx mappings + 80 native with pictures + HID controllers
126. Soft takeover, MIDI learn, mapping versioning, auto-install on connect
127. Skin system: Catppuccin base, token editor, GitHub repos, custom JSON
114. Ableton Link sync
115. GPU waveforms (fast, accurate)
116. In-app tutorials (guided tour)
117. Phone remote (web UI, QR/PIN pairing)
118. Lyrics: .lrc, LRCLIB, Whisper time/transcribe, karaoke
119. Karaoke: 2nd screen, guide vocal, audience web page
120. Video: crossfader transitions, overlay, separate window
121. Lighting: DMX/Art-Net/sACN, fixture patch, stage simulator
122. Beat Lab: Groove Box, Drum Swap/Capture, presets
123. Instrument plugins on decks (CLAP/VST3/AU)
124. Edit mode: timeline cut/copy/paste
125. Mix Point Link: phrase-aware mix points
126. Phrase Edit: cut, retype, move phrases
127. Grid Edit: BPM, shift, tap, x2/÷2, range, lock, undo/redo, metronome
128. USB Export: Pioneer/Engine DJ formats, Sync Manager, Cue sync, Device backup
129. Collection backup: with/without music, restore
130. Diagnostics report generation
131. Touch targets >=44px in touch profile
132. Adaptive layout (desktop/touch/stage profiles)
133. Right-to-left language support (Slint built-in)
134. Accessibility: roles, labels, keyboard navigation
135. High DPI scaling (Slint auto)

---

## Sources

- `/home/ubuntu/djsugar-rust/crates/app/ui/*.slint` (40+ Slint files)
- `/home/ubuntu/djsugar-rust/crates/app/ui/theme.slint` (331 lines of tokens)
- `/home/ubuntu/djsugar-rust/docs/release-notes.md`
- `/home/ubuntu/djsugar-rust/openspec/roadmap.md`
- `https://github.com/openwave-studio/opendj`
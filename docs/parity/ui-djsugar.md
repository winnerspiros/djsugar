# DJ Sugar Android QML UI Parity Specification

This document exhaustively specifies the DJ Sugar Android QML interface for reproduction in Rust. It covers the Android variant first, then notes desktop differences. All file references are relative to `/home/ubuntu/djsugar/res/qml/`.

---

## 1. Overall Screen Structure

### 1.1 Application Entry Points

**main.qml** (lines 1-269)
- Root: `ApplicationWindow` with id `root`
- Design resolution: 1792×1008 (landscape), 1008×1792 (portrait)
- Mobile detection: `isMobile = Qt.platform.os === "android" || Qt.platform.os === "ios"`
- On mobile: fullscreen, size = `Screen.width` × `Screen.height`
- On desktop: windowed, centered on screen, min 680×300
- Property `skinItem` points to loaded skin's MainWindow
- Function `applySkinTheme()` preserves/restores Theme singleton values when switching skins
- Timer `clampTimer` (400ms, 3 passes) clamps window to screen on desktop

**OrientationToggleButton.qml** (71 lines)
- Horizontal `Fader` (slider) with range 0–2, step 1, snap on release
- Binds to Mixxx control via `ControlProxy` (group/key from required properties)
- Track height 2px, handle 3px wide, color from required `color` property
- Used for portrait/landscape/auto orientation switching

### 1.2 Main Window Layout (MainWindow.qml, 749 lines)

Root `Item` with required `ApplicationWindow applicationWindow`.

**Top Toolbar** (lines 112-238, height 36px)
- Background: `Theme.toolbarBackgroundColor` (#2e2e2e desktop, #0a0b10 Android)
- `RowLayout` filling width
- Buttons (left to right):
  1. **4 Decks** (`show4DecksButton`): checkable, visible when height > 515, toggles 4-deck mode
  2. **Library** (`maximizeLibraryButton`): checkable, bound to `[Skin]show_maximized_library`
  3. **Effects** (`showEffectsButton`): checkable
  4. **XY** (`showXyPadButton`): checkable, toggles `xyPadOverlay.visible`
  5. **Aux** (`showAuxButton`): checkable
  6. **Sampler** (`showSamplersButton`): checkable
  7. Spacer (`Item` with `Layout.fillWidth: true`)
  8. **Edit** (`editDeckButton`): checkable, enables deck edit mode
  9. **Develop** (`showDevToolsButton`): checkable, opens `DeveloperToolsWindow` (640×480)
  10. **Preferences** (`showPreferencesButton`): icon gear.svg 16×16, click opens `settingsPopup`, press-and-hold opens native `Mixxx.PreferencesDialog`

**Main Content Area** (SplitView, vertical, lines 239-705)
- Splitter handle: 3px default, 6px on hover/press, color `Theme.libraryPanelSplitterHandle` / `HandleActive`
- **Upper pane (waveforms)**: `Item id: waveforms`
  - Visible when `!root.maximizeLibrary`
  - 2-deck mode: two `WaveformDisplay` (Channel1 top, Channel2 bottom), each 50% height
  - 4-deck mode: four `WaveformDisplay` (Channel3 top, Channel1, Channel2, Channel4 bottom), each 25% height
  - Side fade gradients (125px wide) on left/right edges using `Theme.darkGray`
- **Lower pane (decks + mixer + library)**: `Item` with `SplitView.fillHeight: library.active`
  - **Decks area** (left of mixer, right of mixer):
    - 2-deck: Deck1 left, Deck2 right, each full mixer height
    - 4-deck: Deck1/Deck3 stacked left, Deck2/Deck4 stacked right, each half mixer height
    - When `maximizeLibrary`: decks collapse to 80px height, anchored to horizontal center (Deck1/3 left half, Deck2/4 right half)
    - Focus state: clicking a deck sets `root.focusedDeck`; on narrow screens (<1400px) focused deck expands, mixer shifts
  - **Mixer** (center): `Mixer` component, width 190px, shows 2 or 4 `MixerColumn` + crossfader
  - **Library** (bottom): `Loader` with `Skin.Library`, active when `maximizeLibrary || (height - mixer.height >= 400)`
    - Anchors: top = mixer.bottom (or deck3/4.bottom in 4-deck maximize), bottom = parent.bottom
    - Hidden when `maximizeLibrary && height - mixer.height < 400`

**Floating Overlays**
- `Settings` popup (`settingsPopup`): modal, centered, max 1400×840, Gaussian blur overlay on desktop
- `XYPad` overlay (`xyPadOverlay`): z=90, centered, max 680×380, closes on `closeRequested`

---

## 2. Theme / Colours (Theme/Theme.qml + Theme/+android/Theme.qml)

### 2.1 Desktop Theme (Sugar Classic)

| Property | Hex | Role |
|---|---|---|
| accentColor | #3a60be | Primary brand blue |
| backgroundColor | #1e1e20 | Window background |
| blue | #01dcfc | Cyan accent |
| buttonActiveColor | #D9D9D9 | White |
| buttonDisableColor | #747474 | Light gray |
| buttonFontPixelSize | 10 | Button text size |
| buttonNormalColor | #696969 | Mid gray |
| crossfaderBarColor | #01dcfc | Cyan |
| crossfaderOrientationColor | #747474 | Light gray |
| darkGray | #0f0f0f | Near black |
| darkGray2 | #2e2e2e | Toolbar/surface |
| darkGray3 | #3F3F3F | Elevated surface |
| darkGray4 | #202020 | Deep surface |
| deckActiveColor | #D9D9D9 | White |
| deckBackgroundColor | #0f0f0f | Deck background |
| deckBeatjumpBackgroundColor | #626262 | Mid gray 3 |
| deckBeatjumpLabelColor | #3F3F3F | Dark gray 3 |
| deckEmptyCoverArt | #3F3F3F | Dark gray 3 |
| deckInfoBarBackgroundColor | #0e0e0e | Near black |
| deckLineColor | #2e2e2e | Dark gray 2 |
| deckLoopBackgroundColor | #626262 | Mid gray 3 |
| deckLoopLabelColor | #3F3F3F | Dark gray 3 |
| deckTextColor | #b0b0b0 | Light gray 2 |
| effectColor | #fca001 | Yellow |
| effectUnitColor | #ea2a4e | Red |
| embeddedBackgroundColor | #a0000000 | Semi-transparent black |
| eqFxColor | #ea2a4e | Red |
| eqHighColor | #D9D9D9 | White |
| eqLowColor | #D9D9D9 | White |
| eqMidColor | #D9D9D9 | White |
| fontFamily | "Open Sans" | UI font |
| gainKnobColor | #01dcfc | Cyan |
| green | #85c85b | Green |
| knobBackgroundColor | #262626 | Knob track |
| libraryPanelSplitterBackground | #1e1e1e | Splitter track |
| libraryPanelSplitterHandle | #5f5f5f | Splitter handle |
| libraryPanelSplitterHandleActive | #7a7a7a | Splitter handle active |
| lightGray | #747474 | Disabled/muted |
| lightGray2 | #b0b0b0 | Secondary text |
| lightGray3 | #939393 | Tertiary text |
| midGray | #696969 | Button normal |
| midGray2 | #676767 | |
| midGray3 | #626262 | |
| panelSplitterBackground | #1e1e20 | |
| panelSplitterHandle | #696969 | |
| panelSplitterHandleActive | #b0b0b0 | |
| pflActiveButtonColor | #01dcfc | Cyan |
| red | #ea2a4e | Red |
| samplerColor | #01dcfc | Cyan |
| sunkenBackgroundColor | #0C0C0C | List backgrounds |
| textColor | #b0b0b0 | Primary text |
| textFontPixelSize | 14 | Body text size |
| toolbarActiveColor | #D9D9D9 | White |
| toolbarBackgroundColor | #2e2e2e | Toolbar bg |
| volumeSliderBarColor | #01dcfc | Cyan |
| warningColor | #7D3B3B | Dark red |
| waveformBeatColor | #747474 | Light gray |
| waveformCursorColor | #D9D9D9 | White |
| waveformMarkerDefault | #ff7a01 | Orange |
| waveformMarkerIntroOutroColor | #2c5c9a | Blue |
| waveformMarkerLabel | rgba(255,255,255,0.8) | White 80% |
| waveformMarkerLoopColor | #00b400 | Green |
| waveformMarkerLoopColorDisabled | #FFFFFF | White |
| waveformPostrollColor | #747474 | Light gray |
| waveformPrerollColor | #747474 | Light gray |
| white | #D9D9D9 | White |
| yellow | #fca001 | Yellow |

### 2.2 Android Theme (Theme/+android/Theme.qml — Sugar Night OLED)

| Property | Hex | Role |
|---|---|---|
| accentColor | #ff4fa8 | Hot pink — brand |
| activeColor | #7c5cff | Electric violet — pressed |
| backgroundColor | #000000 | True OLED black |
| blue | #ff4fa8 | Brand pink (replaces cyan) |
| buttonActiveColor | #ffffff | White |
| buttonDisableColor | #444444 | Dark gray |
| buttonFontPixelSize | 10 | |
| buttonNormalColor | #888888 | Medium gray |
| crossfaderBarColor | #ff4fa8 | Hot pink |
| crossfaderOrientationColor | #666666 | |
| darkGray | #000000 | Black |
| darkGray2 | #0a0b10 | Indigo ink |
| darkGray3 | #14161c | Indigo surface |
| darkGray4 | #0d0e13 | Indigo deep |
| deckActiveColor | #ffffff | |
| deckBackgroundColor | #000000 | |
| deckBeatjumpBackgroundColor | #14161c | |
| deckBeatjumpLabelColor | #666666 | |
| deckEmptyCoverArt | #14161c | |
| deckInfoBarBackgroundColor | #000000 | |
| deckLineColor | #0a0b10 | |
| deckLoopBackgroundColor | #14161c | |
| deckLoopLabelColor | #666666 | |
| deckTextColor | #e0e0e0 | |
| effectColor | #ff4fa8 | Hot pink |
| effectUnitColor | #ff4fa8 | Hot pink |
| embeddedBackgroundColor | #E0000000 | |
| eqFxColor | #ff4fa8 | |
| eqHighColor | #ffffff | |
| eqLowColor | #ffffff | |
| eqMidColor | #ffffff | |
| fontFamily | "Open Sans" | |
| gainKnobColor | #ff4fa8 | |
| green | #00e676 | Bright green |
| knobBackgroundColor | #0d0e13 | |
| libraryPanelSplitterBackground | #000000 | |
| libraryPanelSplitterHandle | #333333 | |
| libraryPanelSplitterHandleActive | #ff4fa8 | Hot pink |
| lightGray | #888888 | |
| lightGray2 | #bbbbbb | |
| lightGray3 | #999999 | |
| midGray | #555555 | |
| midGray2 | #444444 | |
| midGray3 | #333333 | |
| panelSplitterBackground | #000000 | |
| panelSplitterHandle | #555555 | |
| panelSplitterHandleActive | #bbbbbb | |
| pflActiveButtonColor | #ff4fa8 | Hot pink |
| red | #ff6b6b | Warm coral |
| samplerColor | #ff4fa8 | Hot pink |
| sunkenBackgroundColor | #000000 | |
| textColor | #e8e8e8 | |
| textFontPixelSize | 14 | |
| toolbarActiveColor | #ffffff | |
| toolbarBackgroundColor | #0a0b10 | Indigo ink |
| volumeSliderBarColor | #ff4fa8 | Hot pink |
| warningColor | #ff6b6b | Warm coral |
| waveformBeatColor | #222222 | Very dark |
| waveformCursorColor | #ffffff | |
| waveformMarkerDefault | #ff4fa8 | Hot pink |
| waveformMarkerIntroOutroColor | #ff99ca | Light pink |
| waveformMarkerLabel | #ffffff | |
| waveformMarkerLoopColor | #00e676 | Bright green |
| waveformMarkerLoopColorDisabled | #ffffff | |
| waveformPostrollColor | #1a1c22 | |
| waveformPrerollColor | #1a1c22 | |
| white | #eeeeee | |
| yellow | #ffd54f | Amber |

**Android-only library colours** (lines 93-120):
- libraryScrollHandle: #333333
- libraryScrollHandleHovered: #7c5cff
- libraryScrollHandlePressed: #ff4fa8
- libraryScrollTrack: #000000
- libraryActionBackground: #0a0b10
- libraryActionDisabledBackground: #0d0e13
- libraryActionText: #eeeeee
- libraryActionDisabledText: #666666
- libraryDeck1Background: #1f6fd1 (cyan-blue)
- libraryDeck1Pressed: #4aa3f5
- libraryDeck2Background: #e8650f (orange)
- libraryDeck2Pressed: #ff9a52
- libraryConfirmBackground: #ff4fa8 (accent)
- librarySuccessColor: #00e676
- librarySourceBarBackground: #000000
- librarySourceBarBorder: #0a0b10
- librarySourceTabBackground: #0d0e13
- librarySourceTabText: #888888
- libraryLocalTabColor: #ff4fa8
- libraryLocalTabBackground: #1a1c22
- libraryStreamingTabColor: #ff4fa8
- libraryStreamingTabText: #ff99ca
- libraryStreamingTabBackground: #1a1c22
- libraryKeyMatchColor: #00e676
- libraryKeyMatchBackground: #00e67633 (20% alpha)
- libraryKeyHarmonicColor: #7c5cff
- libraryKeyHarmonicText: #d0c8ff

---

## 3. Deck Layout (Deck.qml, 924 lines)

Deck is a dynamic, editable layout driven by `itemModel` (array of component descriptors). Default model (lines 63-119):

```
Column (id: column)
  Row (id: row)
    Column (left deck controls)
      Info (track title, artist, BPM, key, cover art)
      Row
        Column (transport)
          Play button
          Cue button
        Column (center)
          Toolbar (4 Decks, Library, Effects, XY, Aux, Sampler, Edit, Develop, Preferences)
          Row
            HotcueAndStem (minWidth 520)
            Beatjump (minWidth 650)
            Loop
    Column (right side)
      WaveformOverview (full width)
      WaveformDisplay (full width, main waveform)
```

### 3.1 Deck Components (Deck/ directory)

Each component type is a QML file under `Deck/` loaded via `DelegateChooser` (Qt.labs.qmlmodels / QtQml.Models).

**Info** (`Deck/Info.qml` - not read but referenced)
- Shows track title, artist, BPM, key, cover art

**Play** (`Deck/Play.qml`)
- Large play/pause button, binds to `[ChannelN],play`

**Cue** (`Deck/Cue.qml`)
- Cue button, binds to `[ChannelN],cue_default`

**Toolbar** (`Deck/Toolbar.qml`)
- Row of toggle buttons matching main toolbar (4 Decks, Library, Effects, XY, Aux, Sampler)

**HotcueAndStem** (`Deck/HotcueAndStem.qml`)
- 8 hotcue buttons (HotcueButton.qml) + stem separation button
- Hotcue buttons: numbered 1-8, color from hotcue color, long-press opens HotcuePopup
- Stem button: triggers stem separation for current track

**Beatjump** (`Deck/Beatjump.qml`)
- Forward/backward beat jump buttons (1, 2, 4, 8, 16, 32 beats)
- Binds to `[ChannelN],beatjump_X_forward/backward`

**Loop** (`Deck/Loop.qml`)
- Loop in/out, reloop, loop halve/double, beatloop sizes (1/2, 1, 2, 4, 8, 16, 32 beats)
- Binds to `[ChannelN],loop_in/out/reloop_toggle/beatloop_X_toggle/loop_halve/loop_double`

**WaveformOverview** (`WaveformOverview.qml` / `Mixxx/Controls/WaveformOverview.qml`)
- Full-width overview waveform with markers (hotcues, loops, intro/outro, cues)
- Toggles with passthrough control (shows cover art when passthrough active)

**WaveformDisplay** (`WaveformDisplay.qml` / `Mixxx/Controls/WaveformDisplay.qml`)
- Main zoomed waveform, supports multiple renderers:
  - Simple, Filtered (3-band), Stacked, HSV, RGB
  - Markers: end-of-track warning, preroll, loop range (green), intro/outro ranges (blue)
  - Zoom control via `Mixxx.Config.waveformDefaultZoom`
  - Smoothed zoom animation (500ms)

### 3.2 Deck Properties & States

- `group`: required string (e.g. "[Channel1]")
- `editMode`: bool (from toolbar Edit button)
- `minimized`: bool (when library maximized)
- `height`: 80px minimized, full mixer height otherwise
- SpringAnimation on height (damping 0.2, duration 500ms, spring 2)
- `toggleFocus` signal emitted on deck click
- Focus states (MainWindow lines 453-508): on narrow screens (<1400px), focused deck expands, mixer shifts to side

### 3.3 Deck Controls Binding Summary

| Control | Mixxx Key | Group |
|---|---|---|
| Play | play | [ChannelN] |
| Cue | cue_default | [ChannelN] |
| Sync | sync_enabled | [ChannelN] |
| Quantize | quantize | [ChannelN] |
| Slip | slip_enabled | [ChannelN] |
| Keylock | keylock | [ChannelN] |
| Rate/tempo | rate | [ChannelN] |
| Rate temp up/down | rate_temp_up/down | [ChannelN] |
| Beatjump | beatjump_X_forward/backward | [ChannelN] |
| Loop in/out | loop_in/out | [ChannelN] |
| Reloop | reloop_toggle | [ChannelN] |
| Beatloop | beatloop_X_toggle | [ChannelN] |
| Loop halve/double | loop_halve/double | [ChannelN] |
| Hotcue X | hotcue_X_activate | [ChannelN] |
| Hotcue X color | hotcue_X_color | [ChannelN] |
| Volume fader | volume | [ChannelN] |
| Gain/trim | pregain | [ChannelN] |
| EQ High/Mid/Low | parameter3/2/1 | [EqualizerRack1_[ChannelN]_Effect1] |
| Filter | super1 | [QuickEffectRack1_[ChannelN]] |
| PFL | pfl | [ChannelN] |
| Crossfader | crossfader | [Master] |

---

## 4. Mixer (Mixer.qml, 302 lines)

- Width: 190px fixed
- Height: EQ section (42px × deck count) + channel strips + crossfader
- **EQ Section** (top): `EqColumn` per deck (42px wide)
  - 3 EQ knobs (High/Mid/Low) + Quick FX knob + effect selector ComboBox
  - Colors: High=white, Mid=white, Low=white, FX=red (desktop) / hot pink (Android)
- **Channel Strips** (middle): `MixerColumn` per deck
  - Gain knob (40×40, cyan/blue) at top
  - VU meters (left/right, 4px wide) flanking volume fader
  - Volume fader (full height, bar color = volumeSliderBarColor)
  - PFL button at bottom (toggleable, active color = pflActiveButtonColor)
- **Crossfader** (bottom): horizontal fader, full width
  - Background: `Theme.imgCrossfaderBackground`
  - Handle: `Theme.imgCrossfaderHandle`
  - Orientation toggle: `OrientationToggleButton` (3-position: left/center/right)
- 4-deck mode: EQ columns and channel strips rearrange in 2×2 grid via states

---

## 5. Library (Library/ + Library/+android/)

### 5.1 Library.qml (not fully read, but Loader in MainWindow loads it)
- Fills available space below mixer
- Contains sidebar (SourceTree) + track list (TrackList) + search/filter

### 5.2 SourceTree (Library/SourceTree.qml + Library/+android/SourceTree.qml)

**Desktop** (Library/SourceTree.qml, 353 lines):
- `Mixxx.LibrarySourceTree` with default columns:
  1. Cover art (100px, hidden <750px)
  2. Title (fillSpan 3)
  3. Artist (fillSpan 2)
  4. Album (fillSpan 1, hidden <690px)
  5. Year (80px, hidden <750px)
  6. BPM (60px)
  7. Key (70px)
  8. File Type (70px, hidden <900px)
  9. Bitrate (70px, hidden <1200px)
- `playlistColumns()` adds position column after cover for playlists
- Sections: All Tracks, Playlists, Crates, Recordings, History, Streaming, Rekordbox, Engine DJ, Computer (drives)

**Android** (Library/+android/SourceTree.qml, 152 lines):
- Same columns but cover art height 30px (vs commented out waveform preview)
- Uses `LibraryComponent.Cell` delegate

### 5.3 TrackList (Library/TrackList.qml + Library/+android/TrackList.qml)

**Desktop** (Library/TrackList.qml, 470 lines):
- `HorizontalHeaderView` with clickable column headers for sorting
- `TableView` with `ItemSelectionModel`
- Columns: same as SourceTree defaultColumns
- Sorting: click header to toggle Asc/Desc
- Selection: single row, keyboard navigation (MoveVertical, SelectNext/PrevTrack)
- Drag: `LibraryComponent.TrackDrag` for touch drag-to-deck/playlist

**Android** (Library/+android/TrackList.qml, 953 lines):
- `HorizontalHeaderView` with sorting
- `TableView` with `SelectionModel`
- Row delegate: `LibraryComponent.Cell` (alternating row colors: sunkenBackgroundColor / backgroundColor)
- Highlighted border (2px accentColor) when `highlighted` property true
- `moveSelectionVertical(offset)` batches rapid knob turns
- `applyDefaultSort()`: playlist order if position column exists, else date added newest first
- Columns: Cover (38px), Title (fillSpan 3), Artist, Album, Year, BPM, Key, File Type, Bitrate
- Auto-hide columns at narrow widths (autoHideWidth property)

### 5.4 Cell (Library/Cell.qml + Library/+android/Cell.qml)

**Desktop**: Simple rectangle with right border (darkGray2)
**Android** (Library/+android/Cell.qml, 23 lines):
```qml
Rectangle {
    color: selected ? Theme.accentColor : (row % 2 == 0 ? Theme.sunkenBackgroundColor : Theme.backgroundColor)
    Rectangle { id: border; color: Theme.darkGray2; width: 1; anchors { bottom: parent.bottom; right: parent.right; top: parent.top } }
}
```

### 5.5 Track (Library/Track.qml, 153 lines)
- `DragHandler` + `TapHandler` (left/right click, long press)
- Context menu (`TrackMenu.qml`) on right-click/long-press
- Menu items:
  - Load to → Deck 1-4, Sampler, Preview Deck
  - Add to playlists
  - Add to crates
  - Analyze (local tracks)
  - Download (streaming tracks)
  - Playlists browser (checkable, shows count)

### 5.6 TrackDrag (Library/TrackDrag.qml, 205 lines)
- Touch drag: hold → move → ghost pill follows finger
- Ghost shows track label, drops on deck/sampler/playlist
- Streaming tracks: download first, then load
- `held` signal opens TrackMenu if no movement >12px

### 5.7 TrackMenu (Library/TrackMenu.qml, 157 lines)
- Popup centered, 360px max width, modal with dim overlay
- Header: track title
- Row: Deck 1 (cyan-blue) / Deck 2 (orange) load buttons
- Analyze/Download button
- Playlists section: `PlaylistBrowser` with search, checkable rows
- Tap row to add/remove, menu stays open

### 5.8 PlaylistBrowser (Library/PlaylistBrowser.qml, 213 lines)
- ColumnLayout with search field (visible when >6 playlists)
- ListView in sunkenBackgroundColor rounded rectangle
- Delegate: 48px rows, accent bar on left when checked
- Locked playlists at 45% opacity
- Search filters client-side

### 5.9 Library Control (Library/Control.qml + Library/+android/Control.qml)

**Desktop** (Library/Control.qml, 161 lines):
- `FocusedWidgetControl` (Sidebar / LibraryView)
- ControlProxy bindings for hardware controllers:
  - `[Library]GoToItem` → load selected to next deck
  - `[Playlist]LoadSelectedIntoFirstStopped` → load selected
  - `[Playlist]SelectTrackKnob` / `SelectPrevTrack` / `SelectNextTrack` → move vertical
  - `[Library]MoveVertical/Up/Down` → move vertical
  - Instantiates `ControlLoadSelectedTrackHandler` per deck

**Android** (Library/+android/Control.qml, 233 lines):
- Same FocusedWidgetControl
- Additional signals: `moveSidebar`, `activateSidebarItem`, `collapseSidebarItem`
- `[Library]GoToItem`: if sidebar focused → activate, else → load to next deck
- Hardware browse knob: `MoveVertical` on track list, `MoveSidebar` on sidebar
- `[Library]MoveUp/Down` also handled

### 5.10 Streaming Hub (Library/Streaming/StreamingHub.qml, 272 lines)

- Tab bar (height 26px) with 5 platforms:
  1. **Navidrome·URL·Radio** (cyan #00e5ff, icon ic_library_navidrome.svg)
  2. **Audius** (purple #ea80fc, triangle icon)
  3. **YouTube** (red #ff0000, drawn play triangle)
  4. **SoundCloud** (orange #ff8800, cloud icon)
  5. **Spotify** (green #1db954, icon)
- Each tab: `Skin.Button` full height, width varies (196/96/96/96/96)
- Content: `Loader` per platform (WebView, OnlineStreamView, YouTubeView, SoundCloudView, SpotifyView)
- `StreamingControl` handles controller input:
  - `MoveTab` (browse press) → cycle platforms
  - `MoveResult` (browse turn) → navigate results
  - `LoadResult` (load press) → load highlighted track
  - Disables library Control while hub visible

### 5.11 Search / Suggestions
- Library search: `TextInput` in Settings sidebar (searchSetting, line 56-93)
- Filters `sectionProperties` model in real-time
- PlaylistBrowser search: `TextField` filters playlists client-side
- Streaming hub: each platform handles its own search

---

## 6. Popups, Drawers, Menus, Overlays

### 6.1 HotcuePopup (HotcuePopup.qml, 100 lines)
- Popup, modal, dim=false, close on escape/outside
- Background: BorderImage with `Theme.imgPopupBackground` (button.svg), borders 20/10
- Grid 4 columns, spacing 2
- Repeater over `Mixxx.Config.hotcueColorPalette` (color rectangles 24×24)
- Click color → `hotcue.setColor()`, close
- Bottom row: Clear button (left, calls `hotcue.clear=1/0`), Close button (right, "X")

### 6.2 BeatgridPopup (not found as separate file — likely inline in Deck)
- BeatCounter (BeatCounter.qml, 41 lines): Row of 4 rectangles (14×7px, spacing 3)
  - Beat 1 (downbeat): border #ff1744, fill #ff1744
  - Beats 2-4: border #00b0ff, fill #00b0ff when active
  - Inactive: #a0101418
  - Binds to `[ChannelN],beat_in_bar` (1-4, 0=no beatgrid)

### 6.3 QuickLoadDrawer (QuickLoadDrawer.qml, 206 lines)
- Popup, modal, dim=true, close on escape/outside
- Background: #12151c, border 2px deck color (Deck1=#00b0ff, Deck2=#ff6d00), radius 8
- Header (50px): deck pill (80×32, deck bg color), title "Load track into DECK N", switch deck button, source tabs (LOCAL MUSIC / STREAMING)
- Content: `Library` component (local) or `StreamingHub` (streaming)
- Target deck: `[Channel1]` or `[Channel2]` (switchable)

### 6.4 SamplePicker (SamplePicker.qml, 650 lines)
- Item (not Popup), used by Settings and FX Station X-Pad
- `samples` array from `Mixxx.PlayerManager.bundledSamples()`
- Pill buttons for each sample (40px high, rounded, accent border)
- Play icon on pill, preview via PreviewDeck or main mix
- Import/record: `adopt(path)` adds to list and selects
- Trim popup for editing samples
- Delete confirmation

### 6.5 FxStation (FxStation.qml, 1296 lines)
- Rectangle, color #0b0e13, fills parent
- Modes: `strip` (height<560 && width>=900), `compact` (height<560 || width<760), else full
- Tabs (compact): Scene / Isolate / X-Pad / Release (tab property 0-3)
- **Scene** (17 effects): BPF ECHO, ECHO, NOISE, SPIRAL UP, REVERB UP, HPF ECHO, LPF ECHO, CRUSH ECHO, SPIRAL DOWN, REVERB DOWN, FLANGER, PHASER, FILTER, ROLL, GATER, PITCH, DRIVE
  - 4×4 grid of FxButtons, editable (edit icon swaps effect)
  - Tempo sync knob, amount knob
- **Isolate**: Low/Mid/High kill knobs + mode selector
- **X-Pad**: XYPad for sample triggering (SamplePicker integration)
- **Release**: 3 effects (VINYL BRAKE, ECHO, BACK SPIN), editable
- Header: title, tempo display, close button
- Effect assignment: buttons route FX Station to decks

### 6.6 XYPad (XYPad.qml, 563 lines)
- Rectangle, #e60a0d14 (dark semi-transparent), radius 12, border accentColor 40% alpha
- Touch surface: drag maps X→filter (LPF←→HPF, 0.5 neutral), Y→mix (dry→wet)
- Target deck selector (Deck 1/2 pills)
- Modes: Snap (return to center), Hold (latch), Momentary
- State save/restore: saves filter, mix, routing, deck on touch start
- Routes EffectRack1_EffectUnit1 to target deck, enables slot 1 if none active
- Close button (closable property)

### 6.7 StemsOverlay (StemsOverlay.qml, 139 lines)
- Item providing two UI elements:
  1. **StemsQueuePopup**: Popup centered, max 640px wide, modal
     - Header: "Stems queue" + Close button
     - `StemsQueue` component (see below)
  2. **StemsToast**: Bottom-center toast (60px from bottom), z=95
     - Green (#1e3a2a) for success, red (#4a1e1e) for error
     - Auto-dismiss timer (3s), click to dismiss
     - Text from `stemsToast.text` + `isError`

### 6.8 StemsQueue (StemsQueue.qml, 204 lines)
- ColumnLayout, spacing 8
- Status row: text (pending count, hold state), Pause/Resume button (green/red), Cancel All (red), Clear Done (when finished)
- Job rows: Repeater over `Mixxx.Stems.jobs`
  - Running: blue border #42a5f5, bg #1e2a3a, progress bar
  - Done: green border, shows stems icons (vocals, drums, bass, other)
  - Failed: red border, error text
  - Buttons: Load to Deck 1/2, Remove

### 6.9 MicrophoneDuckingPanel (MicrophoneDuckingPanel.qml, 61 lines)
- Column, spacing 2
- Mode button: cycles Off → Auto → Manual (3 states), text shows mode, highlight when Auto/Manual
  - Binds to `[Master],talkoverDucking`
- Strength knob: `ControlKnob` (35×35), arcStart=Maximum, color=effectUnitColor
  - Binds to `[Master],duckStrength`

### 6.10 Settings (Settings.qml, 311 lines)
- Popup, modal, centered, max 1400×840, radius 8, bg darkGray2
- Left sidebar (280px): search field + category ListView
  - Categories from `sectionProperties` (populated by Settings sections)
  - Search filters categories
- Right pane: active category's content (Loader)
- Categories (from Settings sections):
  - Interface, Decks, Library, Waveforms, Effects, Samplers, Recording, Broadcasting, MIDI, Controllers, Vinyl Control, Sound Hardware, Library Analysis, Library Metadata, Library Streaming, Privacy, About
- Each section: `activated()`/`deactivated()` lifecycle

### 6.11 MainMenuBar (MainMenuBar.qml, 187 lines) — Desktop only
- `MenuBar` with native macOS support
- **File**: Load Deck 1-4, Quit
- **Library**: Rescan, Export, Search Current View, Search Tracks, Create Playlist, Create Crate
- **View**: Show Microphone, Vinyl Control, Preview Deck, Cover Art, Keywheel, Maximize Library, Auto DJ, Fullscreen
- **Options**: Vinyl Control (Enable 1-4), Preferences, Record Mix
- **Developer** (if developerMode): Reload Skin, Developer Tools, Debugger, Base/Experiment Stats
- **Help**: User Manual, Keyboard Shortcuts, Translate, Community Support, Settings Directory, About

### 6.12 ApplicationMenuActions (ApplicationMenuActions.qml, 491 lines)
- Defines all `Action` objects for MainMenuBar
- Shortcuts configurable via `Mixxx.Application.menuShortcut()`
- Commands delegate to `ApplicationMenuCommands`

### 6.13 ActionPopup (ActionPopup.qml, 100 lines)
- Generic popup with arrow pointing Left/Top/Right/Bottom
- Shape-based arrow (20×20), rounded rectangle bg, drop shadow
- Content: ColumnLayout with 2px spacing
- Used for context menus, dropdowns

---

## 7. Gestures & Touch Interactions

### 7.1 Touch Spinny (Spinny.qml, 215 lines)
- `Mixxx.SpinnyPosition` provides vinyl position
- Two indicators: main (opaque) + ghost (slip mode, semi-transparent)
- Rotation: `360 * (rps/2π) * positionSeconds` where `rps = rpm/60 * 2π`
- Touch handling: `PointerHandler` for scratch
  - `DragHandler` on indicator: maps horizontal drag → `scratch_position`
  - `scratch_enable` control toggled with drag active
  - Cursor shape: OpenHandCursor / ClosedHandCursor / BlankCursor

### 7.2 Fader Touch (ControlFader.qml, 50 lines)
- Extends `Skin.Fader` with `MouseArea` covering full track
- `onPressed` / `onPositionChanged`: maps touch position → parameter value
- Vertical: `1 - (y - handleHeight/2) / travelY`
- Horizontal: `(x - handleWidth/2) / travelX`
- Double-tap: `control.reset()`

### 7.3 Knob Touch (Knob.qml, 67 lines)
- `MixxxControls.Knob` with SVG background (imgKnob) and shadow (imgKnobShadow)
- Foreground: rectangle indicator (width 2, height 20% of knob)
- Arc: 116° offset, radius 45%, width 2px, color from required property
- HiDPI: `sourceSize` set to `devicePixelRatio`×size
- Double-tap: `TapHandler` → `control.reset()`

### 7.4 Hotcue Long Press (HotcueButton.qml, 85 lines)
- `TapHandler` with `gesturePolicy: DragThreshold`, `onLongPressed`:
  - Gets position, calls `hotcueBehavior.pressSecondary(x,y)` then `releaseSecondary()`
- `MouseArea` for right-click: same secondary press/release
- Opens `HotcuePopup` at touch/mouse position

### 7.5 Track Drag (TrackDrag.qml)
- Hold threshold: 12px movement before drag activates
- Ghost follows finger (offset -24px Y)
- Disables list flickable during drag
- Drop delivers to deck/sampler/playlist via dynamic `Connections` waiter

### 7.6 XYPad Touch (XYPad.qml)
- Full-surface `MouseArea` / touch
- Drag updates `currentX` (filter) and `currentY` (mix) in real-time
- Snap mode: on release, animates back to center (0.5, 0)
- Hold mode: latches position
- Momentary: only active while touching

### 7.7 Library Browse Knob (Library/+android/Control.qml)
- `[Playlist]SelectTrackKnob` → `moveVertical(value)`
- `[Library]MoveVertical/Up/Down` → `moveVertical(±1)`
- Sidebar focus: `[Library]GoToItem` activates sidebar item

### 7.8 Effect Knobs (EffectSlot.qml +android)
- Parameter knobs in expanded view: horizontal ListView, non-interactive (knobs handle own touch)
- Meta knob (always visible): `ControlMiniKnob` 40px, arcStart=Minimum

### 7.9 Sampler Pads (Sampler.qml)
- Play button (40×40) + label + BPM
- Gain knob (40×40, samplerColor) top-right
- Right-click on play → cue_default
- Drag from sampler row

---

## 8. Waveform Display Options

### 8.1 WaveformDisplay (WaveformDisplay.qml / Mixxx/Controls/WaveformDisplay.qml)
- `MixxxControls.WaveformDisplay` with multiple renderers:
  - **Simple**: single-color waveform, axes #a1a1a1a1
  - **Filtered** (3-band): low=#2154D7 (blue), mid=#97632D (brown), high=#D5C2A2 (tan), gains from Config
  - **Stacked**: same colors, stacked=true
  - **HSV/RGB**: multi-color spectral
- Active renderer controlled by `Mixxx.Config.waveformType`
- Markers (WaveformRendererMarkRange):
  - Loop: green #00b400, opacity 0.7, disabled #FFFFFF 0.6
  - Intro: blue #2c5c9a, opacity 0.6, duration text after, white
  - Outro: blue #2c5c9a, opacity 0.6, duration text before, white
- Preroll/End-of-track: #ff8872 (coral)
- Cursor: white line
- Beat markers: lightGray (desktop) / #222222 (Android)
- Zoom: `Mixxx.Config.waveformDefaultZoom`, smoothed 500ms animation
- Frame rate: `Mixxx.Config.waveformFrameRate`

### 8.2 WaveformOverview (WaveformOverview.qml / Mixxx/Controls/WaveformOverview.qml)
- `MixxxControls.WaveformOverview` with markers layer
- Markers (WaveformOverviewMarkerLayer):
  - Hotcues: colored lines (hotcue_X_color), width 1px
  - Cue point: red line, "CUE" label
  - Loop: green range, "LOOP" label
  - Intro/Outro: blue ranges, "IN"/"OUT" labels
  - Visibility: `[Skin]show_intro_outro_cues`
- Passthrough mode: switches to cover art display (crossfade 150ms)

### 8.3 WaveDeckBar (WaveDeckBar.qml, 463 lines) — Android minimized deck view
- Two deck cards side by side (Deck 1 cyan, Deck 2 orange)
- Each card (96px high):
  - Header: deck pill, track title, BPM, key color indicator
  - Transport: play/pause (large), cue, sync, quantize, slip, keylock
  - Tempo: BPM display, rate slider (±8%/±16%/±50%), nudge ◀/▶ buttons
  - Time: elapsed/remaining toggle, progress bar
  - Waveform: mini WaveformDisplay (full card width)
  - Hotcues: 8 small buttons
- Nudge buttons: triangular indicators, press→rate_temp_up/down=1

---

## 9. Effects (Effects/ + Effects/+android/)

### 9.1 EffectUnit (EffectUnit.qml desktop 157 lines, +android 187 lines)
- **Desktop**: 3 slots side-by-side (width/3 each), expanded stacks vertically (160px vs 60px)
- **Android** (+android/EffectUnit.qml): `stacked` when expanded OR width<480
  - Expanded height 160px, stacked 132px, normal 116px
- Each slot: `EffectSlot` (see below)
- Right control panel (150px wide):
  - Expand toggle (▲/▼), binds to `[EffectRack1_EffectUnitN],show_parameters`
  - Preset button ("FX N") → opens `EffectChainPresetPopup`
  - PFL toggle: `[EffectRack1_EffectUnitN],group_[Headphone]_enable`
  - Mix mode (D/W): `[EffectRack1_EffectUnitN],mix_mode`
  - Mix knob: `ControlKnob` 40px, binds to `[EffectRack1_EffectUnitN],mix`
  - Assign buttons: `EffectAssignmentButtons` (FX1-FX4 or FX1-FX2 per channel)

### 9.2 EffectSlot (EffectSlot.qml desktop, +android/EffectSlot.qml 193 lines)
- **Selector area** (left, max 300px, 45% when expanded):
  - Focus button (F): `[EffectRack1_EffectUnitN],focused_effect` (show/hide via show_focus)
  - Enable button (ON): `[EffectRack1_EffectUnitN_EffectM],enabled`
  - Effect selector: `EffectSelector` (ComboBox) → `[EffectRack1_EffectUnitN_EffectM],effectId`
  - Meta knob: `ControlMiniKnob` 40px, `[EffectRack1_EffectUnitN_EffectM],meta`
- **Parameters** (right, visible when expanded):
  - Horizontal ListView, non-flickable (knobs handle touch)
  - Up to 8 parameters per type (knob/button)
  - Each: `ControlKnob` (55px) or `ControlButton` with label

### 9.3 EffectSelector (EffectSelector.qml / +android/EffectSelector.qml)
- ComboBox, model = `Mixxx.EffectsManager.visibleEffectsModel`
- Syncs `currentIndex` to `slot.effectId`
- `popupMaxItem: 8` (Android), desktop uses default

### 9.4 EffectFocusButton (EffectFocusButton.qml / +android/EffectFocusButton.qml)
- Button "F", highlight when focused
- Left tap: set focus to this effect (or clear if already focused)
- Right tap: clear focus
- **Android**: left tap never clears (controllers need persistent focus)

### 9.5 EffectPresetMenuItem / Separator / Menu
- `EffectPresetMenuItem`: MenuItem with checkmark, hover colors
- `EffectPresetMenuSeparator`: double line (top #000000, bottom #333333)
- `EffectSlotPresetMenu`: Menu for one slot
  - Presets from `slot.parametersModel` (checkable, toggle visibility)
  - Save snapshot, separator
- `EffectChainPresetPopup`: Menu for unit
  - Standard chain presets from `Mixxx.EffectsManager.standardChainPresetModel`
  - Update/Rename/Delete preset actions

### 9.6 EffectAssignmentButtons (EffectAssignmentButtons.qml, 37 lines)
- Row of 2 or 4 buttons (showFourUnits from `[Skin]show_4effectunits`)
- Each: `ControlButton` toggleable, binds to `[EffectRack1_EffectUnitM],group_[ChannelN]_enable`
- Text: "FX1"/"FX2" or "FX1"/"2"/"3"/"4"

---

## 10. Sampler (Sampler.qml, 313 lines + SamplerRow.qml)

- `SamplerRow`: RowLayout with 8 `Sampler` (repeater, samplerOffset 0 or 8)
- Each `Sampler` (group `[SamplerN]`):
  - Minimized height 50px, full 170px
  - Summary bar (50px): embedded bg, gain knob (40×40, samplerColor), play button (40×40), label, BPM
  - Play button: highlight when playing/latched, right-click → cue_default
  - Expanded: waveform overview, hotcues, beatjump, loop, volume fader, FX assign
  - Drag source: `DragHandler` with mime "mixxx/player"
  - Color: track color darkened 2×, fallback backgroundColor

---

## 11. Desktop Differences Summary

| Aspect | Android | Desktop |
|---|---|---|
| Theme | Sugar Night (OLED black, hot pink accent) | Sugar Classic (dark blue-gray, cyan accent) |
| Window | Fullscreen, no titlebar | Windowed, native menu bar (macOS) or loaded MainMenuBar |
| Toolbar | Same buttons, no native menu | MainMenuBar loaded when not fullscreen |
| Library | Touch-optimized: larger rows (48px), swipe gestures, TrackDrag | Mouse/keyboard: smaller rows, click headers to sort, context menu on right-click |
| Deck layout | Fixed 2-deck cards (WaveDeckBar) when minimized | Resizable, 2/4 deck, focus-based expansion |
| Effects | Stacked slots on narrow, tabs for FX Station | Side-by-side slots, expand toggle, all visible |
| Sampler | Not in main view (commented out in MainWindow) | Available via toolbar button |
| XYPad | Floating overlay, full touch | Floating overlay, mouse drag |
| Stems | Queue popup + toast | Same |
| Microphone | Panel in mixer area | Same |
| Settings | Popup, searchable sidebar | Same |
| Waveform | Passthrough shows cover art | Same |
| Orientation | OrientationToggleButton (3-position) | Not needed |
| Controller learning | LearnController (Android-specific) | Not present |
| App update | Banner at top | Same |
| Developer tools | Button in toolbar | Menu item + button |

---

## 12. Numbered Checklist of Distinct UI Features (150+ items)

### Application & Window (1-8)
1. ApplicationWindow with mobile/desktop detection
2. Design resolution 1792×1008 (landscape) / 1008×1792 (portrait)
3. Fullscreen on mobile, centered windowed on desktop
4. Window clamping to screen (desktop, 3-pass timer)
5. Skin switching via `applySkinTheme()` preserving Theme defaults
6. OrientationToggleButton (3-position horizontal slider)
7. Theme singleton with 80+ color/size/font properties
8. Android-specific Theme override (Sugar Night OLED palette)

### Toolbar (9-20)
9. Toolbar height 36px, background `toolbarBackgroundColor`
10. 4 Decks toggle button (checkable, visible height>515)
11. Library maximize toggle (bound to `[Skin]show_maximized_library`)
12. Effects toggle button
13. XY Pad toggle button (opens floating overlay)
14. Aux toggle button
15. Sampler toggle button
16. Edit Deck toggle button (enables deck edit mode)
17. Developer Tools toggle (opens 640×480 window)
18. Preferences button (gear icon, click=Settings popup, press-hold=native dialog)
19. Toolbar button style: `Skin.Button` with activeColor=white, checkable
20. Spacer item pushing Edit/Develop/Preferences right

### Main Layout (21-35)
21. Vertical SplitView with custom handle (3px→6px on hover, accent color on Android)
22. Upper pane: Waveforms area (hidden when library maximized)
23. 2-deck waveform split: Channel1 top 50%, Channel2 bottom 50%
24. 4-deck waveform split: Channel3/1/2/4 each 25%
25. Side fade gradients (125px) on waveform edges
26. Lower pane: Decks + Mixer + Library
27. Deck focus system (click toggles, narrow screen expands focused deck)
28. Deck minimized state (80px) when library maximized
29. SpringAnimation on deck height changes (damping 0.2, 500ms, spring 2)
30. Mixer centered, fixed 190px width
31. Library Loader (activates when space≥400px or maximized)
32. Library anchor transitions (under mixer, or under deck3/4 in 4-deck maximize)
33. FadeBehavior on library visibility
34. Settings popup (modal, centered, max 1400×840, Gaussian blur overlay)
35. XYPad overlay (floating, z=90, max 680×380, centered)

### Deck Components (36-65)
36. Deck layout driven by `itemModel` (editable via Edit mode)
37. Info panel: track title, artist, BPM, key, cover art
38. Play button (large, binds `[ChannelN],play`)
39. Cue button (binds `[ChannelN],cue_default`)
40. Toolbar with transport toggles
41. HotcueAndStem: 8 hotcue buttons + stem separation button
42. HotcueButton: numbered 1-8, color from hotcue, long-press→HotcuePopup
43. HotcuePopup: 4-column color grid + Clear + Close
44. Beatjump: 6 button pairs (1/2/4/8/16/32 beats forward/back)
45. Loop: in/out, reloop, halve/double, beatloop 1/2/1/2/4/8/16/32
46. WaveformOverview: full-width, markers, passthrough cover art toggle
47. WaveformDisplay: main zoom, multiple renderers (Simple/Filtered/Stacked/HSV/RGB)
48. Waveform renderers: gains from Config, colors per band
49. Loop range marker (green, opacity 0.7)
50. Intro/Outro range markers (blue, opacity 0.6, duration text)
51. End-of-track / preroll warning (coral #ff8872)
52. Beat markers (lightGray desktop / #222222 Android)
53. Cursor line (white)
54. Zoom control (smoothed 500ms, synchronized option)
55. Deck drag-and-drop (mime "mixxx/player" + track URL)
56. Deck toggleFocus signal
57. Deck editMode property
58. Deck minimized property
59. Deck height binding to mixer/show4decks/maximizeLibrary
60. Deck focus expansion on narrow screens (<1400px)
61. Mixer shift animation on focus change (AnchorAnimation 200ms)
62. Deck 3/4 Loaders (active when show4decks)
63. Deck component types: column, row, info, waveformOverview, play, cue, toolbar, hotcueAndStem, beatjump, loop
64. DelegateChooser for component rendering (Qt.labs.qmlmodels / QtQml.Models)
65. Serialize/deserialize model for edit mode persistence

### Mixer (66-80)
66. Mixer width 190px, height = EQ + strips + crossfader
67. EQ Column per deck (42px wide): High/Mid/Low knobs + QuickFX knob + effect selector
68. EQ knob colors: High/Mid/Low=white, FX=red (desktop) / hot pink (Android)
69. MixerColumn per deck: gain knob (top), VU meters (L/R), volume fader, PFL button
69. Gain knob: 40×40, cyan/blue, `[ChannelN],pregain`
70. VU meters: 4px wide, left/right keys
71. Volume fader: full height, bar color `volumeSliderBarColor`, custom handle image
72. PFL button: toggleable, active color `pflActiveButtonColor`
73. Crossfader: horizontal, full width, custom background/handle images
74. Crossfader orientation toggle (3-position slider)
75. 4-deck mixer rearrangement (2×2 grid via states)
76. Mixer height SpringAnimation (same as decks)
77. Mixer visibility tied to `!maximizeLibrary`
78. Mixer fade transition on show/hide
79. Mixer columns anchored to EQ columns above
80. Crossfader at bottom of mixer column

### Library (81-115)
81. Library fills bottom pane, Loader-activated
82. SourceTree (sidebar): sections All Tracks, Playlists, Crates, Recordings, History, Streaming, Rekordbox, Engine DJ, Computer
83. SourceTree columns: Cover, Title, Artist, Album, Year, BPM, Key, File Type, Bitrate
84. Playlist columns add Position after Cover
85. Auto-hide columns at narrow widths
86. TrackList: TableView with HorizontalHeaderView
87. Column header click → sort toggle (Asc/Desc)
88. Row selection: single, keyboard navigation
89. TrackList sort default: playlist order or date added newest
90. Track delegate: alternating row colors, highlighted border
91. Cell: Android uses sunkenBackgroundColor/backgroundColor alternation
92. TrackDrag: touch hold→drag, ghost pill, drop on deck/sampler/playlist
93. TrackDrag streaming: downloads first, then loads
94. TrackMenu (context): Load to Deck 1-4, Sampler, Preview; Analyze/Download; Playlist browser
95. PlaylistBrowser: searchable, checkable rows, accent left bar when checked
96. Library Control: FocusedWidgetControl (Sidebar/LibraryView)
97. Hardware browse knob → MoveVertical on track list
98. Hardware browse press → GoToItem (activate sidebar or load track)
99. Hardware SelectTrackKnob/Prev/Next → move vertical
100. LoadSelectedTrackHandler per deck (instantiated from num_decks)
101. StreamingHub: 5 platform tabs (Navidrome/URL/Radio, Audius, YouTube, SoundCloud, Spotify)
102. StreamingHub tab bar: colored buttons with icons, 26px height
103. StreamingHub content: Loader per platform (WebView, YouTubeView, etc.)
104. StreamingControl: routes controller browse/load to active platform
105. StreamingControl: MoveTab (browse press) cycles platforms
106. StreamingControl: MoveResult (browse turn) navigates results
107. StreamingControl: LoadResult (load press) loads highlighted track
108. StreamingControl disables library Control while active
109. Library search: TextInput in Settings sidebar filters categories
110. PlaylistBrowser search: TextField filters playlists client-side
111. SourceTree activation: click row → activate, toggle expand if has children
112. SourceTree drive access: USB permission request for Rekordbox/Engine DJ/Computer
113. SourceTree playlistColumns() adds position column
114. TrackList autoHideWidth on columns
115. TrackList moveSelectionVertical batches rapid knob turns

### Popups & Overlays (116-140)
116. HotcuePopup: color grid (4 cols) + Clear + Close
117. BeatCounter: 4 rectangles showing beat-in-bar (downbeat distinct)
118. QuickLoadDrawer: modal, deck switcher, source tabs (Local/Streaming), embedded Library/StreamingHub
119. SamplePicker: pill buttons, preview, import/record/trim/delete
120. FxStation: Scene/Isolate/X-Pad/Release sections, tabbed on compact, strip mode
121. FxStation Scene: 17 effects in 4×4 grid, editable assignments, tempo sync + amount knobs
122. FxStation Isolate: Low/Mid/High kill knobs + mode selector
123. FxStation X-Pad: XYPad for sample triggering with SamplePicker
124. FxStation Release: 3 effects, editable
125. FxStation header: tempo display, close button
126. XYPad: touch surface X=filter(LPF↔HPF), Y=mix(Dry↔Wet), Snap/Hold/Momentary modes
127. XYPad state save/restore (filter, mix, routing, deck)
128. XYPad routes EffectRack1_EffectUnit1 to target deck
129. StemsOverlay: StemsQueuePopup + StemsToast
130. StemsQueuePopup: status row (Pause/Resume, Cancel All, Clear Done) + job rows
131. StemsQueue job rows: progress bar (running), stem icons (done), error (failed), Load Deck 1/2, Remove
132. StemsToast: bottom-center, auto-dismiss 3s, green success / red error
133. MicrophoneDuckingPanel: mode button (Off/Auto/Manual) + strength knob
134. Settings popup: searchable sidebar + category content panes
135. Settings categories: Interface, Decks, Library, Waveforms, Effects, Samplers, Recording, Broadcasting, MIDI, Controllers, Vinyl Control, Sound Hardware, Library Analysis, Library Metadata, Library Streaming, Privacy, About
136. MainMenuBar (desktop): File, Library, View, Options, Developer, Help menus
137. ApplicationMenuActions: all Action objects with configurable shortcuts
138. ActionPopup: generic arrow popup (Left/Top/Right/Bottom) with shadow
139. AppUpdateBanner: top bar when update available, Skip/Download/Install/Close
140. AboutDialog: native MessageDialog with version/platform

### Effects (141-155)
141. EffectUnit: 3 slots, expand toggle (desktop side-by-side, Android stacked when narrow)
142. EffectSlot: focus button (F), enable (ON), selector ComboBox, meta knob
143. EffectSlot expanded: horizontal parameter list (knobs/buttons, up to 8 per type)
144. EffectSelector: ComboBox synced to slot.effectId, visibleEffectsModel
145. EffectFocusButton: "F" button, highlight when focused, left tap sets focus, right tap clears (Android: left never clears)
146. EffectPresetMenuItem: checkable menu item with hover styling
147. EffectPresetMenuSeparator: double-line separator
148. EffectSlotPresetMenu: parameter visibility toggles + Save snapshot
149. EffectChainPresetPopup: chain presets + Update/Rename/Delete
150. EffectAssignmentButtons: 2 or 4 toggle buttons routing FX units to channels
151. EffectUnit control panel: expand, preset, PFL, D/W, mix knob, assignments
152. EffectUnit PFL toggle: `[EffectRack1_EffectUnitN],group_[Headphone]_enable`
153. EffectUnit mix mode: `[EffectRack1_EffectUnitN],mix_mode`
154. EffectUnit mix knob: `ControlKnob` binds `[EffectRack1_EffectUnitN],mix`
155. EffectUnit preset button opens EffectChainPresetPopup

### Sampler & Misc (156-165)
156. SamplerRow: 2 rows × 8 samplers (offset 0 and 8)
157. Sampler: minimized 50px (gain, play, label, BPM), expanded 170px (waveform, hotcues, beatjump, loop, volume, FX)
158. Sampler play button: highlight when playing/latched, right-click→cue_default
159. Sampler gain knob: 40×40, samplerColor
160. Sampler drag source (mime "mixxx/player")
161. Sampler color from track color darkened 2×
162. PreviewDeck: placeholder (42 lines, "PreviewDeck placeholder")
163. EmbeddedBackground: rounded rect, border deckBackgroundColor, color embeddedBackgroundColor
164. SectionBackground: BorderImage with Theme.imgSectionBackground (borders 10px)
165. SugarLogo: procedural donut record + "DJ SUGAR" wordmark, scalable, no assets

### Controls & Primitives (166-185)
166. Button (Skin.Button): checkable, highlight, activeColor, normalColor, icon support
167. ControlButton: wraps ControlProxyButtonBehavior, binds group/key, toggleable
168. ControlProxyButtonBehavior: press/hold/latch/power-window logic, primary/secondary actions
169. ControlKnob: Knob + ControlProxy, double-tap reset
170. ControlFader: Fader + ControlProxy, touch-area tap-to-seek + drag, double-tap reset
171. Knob (Skin.Knob): SVG background (imgKnob), shadow (imgKnobShadow), arc indicator, HiDPI sourceSize
172. Fader (Skin.Fader): background/handle images, margins, drop shadow on handle
173. Slider (Skin.Slider): horizontal, markers, text input suffix, accent handle with glow
174. ComboBox (Skin.ComboBox): styled popup, max 6-8 items, footer items, InnerShadow/DropShadow
175. InputField: TextInput with drop/inner shadows, dark bg, white/red text
176. ArrowText: Row with optional ◀/▶ triangles (drawn, not glyphs)
177. FormButton: uppercase label, category colors (Action blue, Danger red), drop shadow
178. ActionButton: same as FormButton, Category enum
179. EditIcon: canvas-drawn pencil (14×14), click signal
180. ControlMiniKnob: smaller Knob variant (used for meta, gain, etc.)
181. HotcueButtonBehavior: press primary=activate, secondary=open popup, long-press→secondary
182. IntroOutroButton: cue button with behavior, right-click secondary
183. BeatCounter: 4 cells, beat_in_bar control
184. MobileRecordButton: REC pill, blinking red dot when recording, confirmation popup on stop
185. OrientationToggleButton: 3-position horizontal fader (0/1/2)

### Waveform & Markers (186-195)
186. WaveformDisplay (Mixxx.Controls): multi-renderer, zoom, frame rate
187. WaveformOverview (Mixxx.Controls): overview with marker layer
188. WaveformOverviewHotcueMarker: Shape line, hotcue color, position from track_samples
189. WaveformOverviewMarker: Shape line, cue/loop/intro/outro, labels
190. WaveformOverviewMarkerLayer: Repeater for hotcues + cue/loop/intro/outro markers
191. WaveformOverview passthrough: crossfade to cover art (150ms)
192. KeyColorIndicator: vertical split strip, colors from key palette, visual_key_distance
193. DurationFormatter: singleton, modes Traditional/Seconds/Kilo/Hecto
194. PlayerDropArea: (referenced, not found)
195. Spinny: vinyl indicator + ghost (slip), rotation from rpm, scratch drag handler

---

## 13. File Inventory (Key Files)

| File | Lines | Purpose |
|---|---|---|
| main.qml | 269 | Application entry, window setup, skin loading |
| MainWindow.qml | 749 | Main layout: toolbar, split view, decks, mixer, library, overlays |
| OrientationToggleButton.qml | 71 | 3-position orientation slider |
| Theme/Theme.qml | 88 | Desktop color/size/font singleton |
| Theme/+android/Theme.qml | 122 | Android OLED theme + library colors |
| Deck.qml | 924 | Dynamic deck layout, component model |
| Deck/ (directory) | — | Play, Cue, Toolbar, HotcueAndStem, Beatjump, Loop, Info |
| Mixer.qml | 302 | Mixer with EQ, channel strips, crossfader |
| MixerColumn.qml | 81 | Single channel strip |
| EqColumn.qml | 63 | EQ knobs + QuickFX + selector |
| WaveformDisplay.qml | 360 | Main waveform, multi-renderer |
| WaveformOverview.qml | 145 | Overview waveform + passthrough |
| WaveDeckBar.qml | 463 | Android minimized deck cards |
| Library.qml | — | Library root (loaded via Loader) |
| Library/SourceTree.qml | 353 | Sidebar source tree (desktop) |
| Library/+android/SourceTree.qml | 152 | Sidebar source tree (Android) |
| Library/TrackList.qml | 470 | Track table (desktop) |
| Library/+android/TrackList.qml | 953 | Track table (Android) |
| Library/Cell.qml | 23 | Row background (Android) |
| Library/Track.qml | 153 | Track drag/tap/context menu |
| Library/TrackDrag.qml | 205 | Touch drag ghost + drop |
| Library/TrackMenu.qml | 157 | Track context menu |
| Library/PlaylistBrowser.qml | 213 | Playlist picker with search |
| Library/Control.qml | 161 | Library controller bindings (desktop) |
| Library/+android/Control.qml | 233 | Library controller bindings (Android) |
| Library/ControlLoadSelectedTrackHandler.qml | 40 | Per-deck load handler |
| Library/Streaming/StreamingHub.qml | 272 | 5-platform streaming browser |
| Library/Streaming/StreamingControl.qml | 120 | Controller routing for streaming |
| HotcuePopup.qml | 100 | Hotcue color picker + clear |
| BeatCounter.qml | 41 | 4-cell beat indicator |
| QuickLoadDrawer.qml | 206 | Modal load drawer with deck switch |
| SamplePicker.qml | 650 | Sample browser with preview/import |
| FxStation.qml | 1296 | FX Station (Scene/Isolate/X-Pad/Release) |
| XYPad.qml | 563 | Touch XY pad for filter+mix |
| StemsOverlay.qml | 139 | Stems queue popup + toast |
| StemsQueue.qml | 204 | Stems job queue UI |
| MicrophoneDuckingPanel.qml | 61 | Ducking mode + strength |
| Settings.qml | 311 | Settings popup with categories |
| MainMenuBar.qml | 187 | Desktop native menu bar |
| ApplicationMenuActions.qml | 491 | All menu actions with shortcuts |
| ActionPopup.qml | 100 | Generic arrow popup |
| Effects/EffectUnit.qml | 157 | Effect unit (desktop) |
| Effects/+android/EffectUnit.qml | 187 | Effect unit (Android, stacked) |
| Effects/EffectSlot.qml | — | Effect slot (desktop) |
| Effects/+android/EffectSlot.qml | 193 | Effect slot (Android) |
| Effects/EffectSelector.qml | 35 | Effect picker ComboBox |
| Effects/EffectFocusButton.qml | 40 | Focus button (desktop) |
| Effects/+android/EffectFocusButton.qml | 42 | Focus button (Android) |
| Effects/EffectPresetMenuItem.qml | 54 | Preset menu item |
| Effects/EffectPresetMenuSeparator.qml | 28 | Preset menu separator |
| Effects/EffectSlotPresetMenu.qml | 92 | Slot preset menu |
| Effects/EffectChainPresetPopup.qml | 175 | Chain preset menu |
| Effects/EffectAssignmentButtons.qml | 37 | FX unit → channel routing |
| Sampler.qml | 313 | Single sampler slot |
| SamplerRow.qml | 25 | 8-sampler row (×2) |
| ControlButton.qml | 30 | Button + ControlProxy binding |
| ControlKnob.qml | 21 | Knob + ControlProxy |
| ControlFader.qml | 50 | Fader + touch area + ControlProxy |
| HotcueButton.qml | 85 | Hotcue button + long-press popup |
| IntroOutroButton.qml | 47 | Intro/Outro cue button |
| Button.qml | — | Base Button (Skin.Button) |
| FormButton.qml | 162 | Styled form button |
| ActionButton.qml | 51 | Action button with categories |
| ComboBox.qml | 232 | Styled ComboBox |
| InputField.qml | 48 | Styled TextInput |
| ArrowText.qml | 38 | Text with triangle arrows |
| Slider.qml | 170 | Slider with markers + text input |
| Knob.qml | 67 | SVG knob with arc |
| Fader.qml | 60 | Image-based fader |
| SectionBackground.qml | 15 | BorderImage section bg |
| EmbeddedBackground.qml | 11 | Rounded embedded bg |
| EditIcon.qml | 51 | Canvas pencil icon |
| MobileRecordButton.qml | 148 | REC button with confirmation |
| LearnController.qml | 305 | Android controller learning UI |
| AboutDialog.qml | 9 | Native about dialog |
| DeveloperToolsWindow.qml | 257 | Control inspector table |
| AppUpdateBanner.qml | 94 | Update notification bar |
| Mixxx/Controls/WaveformDisplay.qml | 7 | Thin wrapper |
| Mixxx/Controls/WaveformOverview.qml | — | Overview wrapper |
| Mixxx/Controls/WaveformOverviewHotcueMarker.qml | 77 | Hotcue marker line |
| Mixxx/Controls/WaveformOverviewMarker.qml | 51 | Generic marker line |
| Mixxx/Controls/WaveformOverviewMarkerLayer.qml | 336 | Marker layer repeater |
| Mixxx/Controls/Spinny.qml | 215 | Vinyl spinny + ghost |
| Mixxx/Controls/KeyColorIndicator.qml | 60 | Key color strip |
| Mixxx/Controls/Slider.qml | 189 | Base Slider control |
| Mixxx/Controls/Fader.qml | 46 | Base Fader control |
| Mixxx/DurationFormatter.qml | 61 | Time formatting singleton |

---

## 14. Mixxx Control Keys Reference (Per Component)

### Deck Controls ([ChannelN])
| UI Element | Control Key |
|---|---|
| Play button | `play` |
| Cue button | `cue_default` |
| Sync button | `sync_enabled` |
| Quantize button | `quantize` |
| Slip button | `slip_enabled` |
| Keylock button | `keylock` |
| Rate fader | `rate` |
| Rate temp up | `rate_temp_up` |
| Rate temp down | `rate_temp_down` |
| Beatjump 1/2/4/8/16/32 fwd | `beatjump_X_forward` |
| Beatjump 1/2/4/8/16/32 back | `beatjump_X_backward` |
| Loop in | `loop_in` |
| Loop out | `loop_out` |
| Reloop | `reloop_toggle` |
| Beatloop 1/2/1/2/4/8/16/32 | `beatloop_X_toggle` |
| Loop halve | `loop_halve` |
| Loop double | `loop_double` |
| Hotcue 1-8 activate | `hotcue_X_activate` |
| Hotcue 1-8 color | `hotcue_X_color` |
| Hotcue 1-8 status | `hotcue_X_status` |
| Hotcue 1-8 position | `hotcue_X_position` |
| Volume fader | `volume` |
| Gain/trim knob | `pregain` |
| EQ High | `parameter3` (group `[EqualizerRack1_[ChannelN]_Effect1]`) |
| EQ Mid | `parameter2` (group `[EqualizerRack1_[ChannelN]_Effect1]`) |
| EQ Low | `parameter1` (group `[EqualizerRack1_[ChannelN]_Effect1]`) |
| Filter knob | `super1` (group `[QuickEffectRack1_[ChannelN]]`) |
| PFL button | `pfl` |
| BPM display | `bpm` |
| Key display | `visual_key` |
| Play position | `playposition` |
| Duration | `duration` |
| Track loaded | `track_loaded` |
| Track samples | `track_samples` |
| Cue point | `cue_point` |
| Loop start | `loop_start_position` |
| Loop end | `loop_end_position` |
| Loop enabled | `loop_enabled` |
| Intro start | `intro_start_position` |
| Intro end | `intro_end_position` |
| Outro start | `outro_start_position` |
| Outro end | `outro_end_position` |

### Mixer ([Master])
| UI Element | Control Key |
|---|---|
| Crossfader | `crossfader` |
| Crossfader orientation | (OrientationToggleButton, 3-position) |
| Headphone mix | `headMix` |
| Headphone gain | `headGain` |
| Main gain | `gain` |
| Microphone ducking mode | `talkoverDucking` (0=Off,1=Auto,2=Manual) |
| Microphone duck strength | `duckStrength` |

### Effects ([EffectRack1_EffectUnitN], [EffectRack1_EffectUnitN_EffectM])
| UI Element | Control Key |
|---|---|
| Show parameters | `show_parameters` |
| Effect M enable | `enabled` (slot group) |
| Effect M selector | `effectId` (slot group) |
| Effect M meta knob | `meta` (slot group) |
| Effect M parameter K | `parameterK` (slot group) |
| Unit mix mode | `mix_mode` |
| Unit mix knob | `mix` |
| Unit PFL | `group_[Headphone]_enable` |
| Unit preset | `loaded_chain_preset` |
| Focused effect | `focused_effect` |
| Show focus | `show_focus` |
| Channel assign | `group_[ChannelN]_enable` |

### Library ([Library], [Playlist])
| UI Element | Control Key |
|---|---|
| Browse vertical | `MoveVertical` |
| Browse up | `MoveUp` |
| Browse down | `MoveDown` |
| GoToItem (activate/load) | `GoToItem` |
| Load selected to deck | `LoadSelectedTrack` / `LoadSelectedTrackAndPlay` |
| Load to first stopped | `LoadSelectedIntoFirstStopped` |
| Select track knob | `SelectTrackKnob` |
| Select prev/next | `SelectPrevTrack` / `SelectNextTrack` |

### Sampler ([SamplerN])
| UI Element | Control Key |
|---|---|
| Play | `cue_gotoandplay` |
| Gain | `pregain` |
| Volume | `volume` |
| Cue (right-click) | `cue_default` |

### Recording ([Recording])
| UI Element | Control Key |
|---|---|
| Status | `status` |
| Toggle recording | `toggle_recording` |

### Application ([App])
| UI Element | Control Key |
|---|---|
| Num decks | `num_decks` |
| Num samplers | `num_samplers` |
| Num preview decks | `num_preview_decks` |

### Waveform ([Waveform])
| UI Element | Control Key |
|---|---|
| Overview type | `WaveformOverviewType` |

### Skin ([Skin])
| UI Element | Control Key |
|---|---|
| Show maximized library | `show_maximized_library` |
| Show 4 effect units | `show_4effectunits` |
| Show intro/outro cues | `show_intro_outro_cues` |

---

## 15. Android-Specific Behaviours

1. **Theme**: Sugar Night (true black #000000, hot pink #ff4fa8 accent, electric violet #7c5cff active)
2. **Library rows**: 48px height, alternating sunkenBackgroundColor/backgroundColor, accent border on highlight
3. **TrackDrag**: Touch hold (12px threshold) → ghost pill → drop on deck/sampler/playlist
4. **Hotcue long-press**: TapHandler with DragThreshold → opens HotcuePopup
5. **Fader touch**: Full-track MouseArea maps touch position to value
6. **EffectUnit stacked**: When expanded OR width<480, slots stack vertically
7. **EffectFocusButton**: Left tap never clears focus (controller compatibility)
8. **StreamingHub**: 5 tabs, hardware browse/load routed to active platform
9. **WaveDeckBar**: Minimized deck cards (96px) with full transport + mini waveform
10. **OrientationToggleButton**: 3-position slider for portrait/landscape/auto
11. **LearnController**: Full controller mapping UI (transport, tempo, hotcues, loops, mixer, library, FX Station, main)
12. **AppUpdateBanner**: Top bar with Skip/Download/Install/Close
13. **No native menu bar**: MainMenuBar only loaded on desktop when not fullscreen
14. **Settings search**: TextInput in sidebar filters categories in real-time
15. **PlaylistBrowser search**: Client-side filter when >6 playlists
16. **StemsQueuePopup**: Modal with job list, progress, stem icons, load/cancel buttons
17. **StemsToast**: Auto-dismiss bottom toast for separation complete/error
18. **SamplePicker**: Pill buttons, preview via PreviewDeck or main mix, trim/import/record
19. **XYPad**: Floating overlay, Snap/Hold/Momentary, state save/restore
20. **MobileRecordButton**: Blinking REC, confirmation popup before stop

---

## 16. Desktop-Specific Behaviours

1. **Theme**: Sugar Classic (dark blue-gray #1e1e20, cyan #01dcfc accent)
2. **Native menu bar**: MainMenuBar loaded on macOS or when not fullscreen
3. **Window management**: Centered, clamped to screen, min 680×300
4. **Library rows**: Smaller, clickable column headers for sorting
5. **Context menus**: Right-click on tracks, headers
6. **EffectUnit**: Side-by-side slots, expand toggles vertical stack
7. **EffectFocusButton**: Left tap toggles focus, right tap clears
8. **Sampler row**: Available via toolbar (commented in MainWindow but present)
9. **Developer Tools**: Menu item + toolbar button
10. **Gaussian blur**: Settings overlay uses blur when hardware acceleration available
11. **Keyboard shortcuts**: Configurable via ApplicationMenuActions
12. **Multi-monitor**: Window clamping to screen with titlebar reachable

---

## 17. Component Hierarchy Summary

```
ApplicationWindow (main.qml)
├── MainWindow (skin root)
│   ├── Toolbar (36px)
│   │   ├── 4 Decks, Library, Effects, XY, Aux, Sampler, Edit, Develop, Preferences
│   ├── SplitView (vertical)
│   │   ├── Waveforms pane
│   │   │   ├── Deck3 WaveformDisplay (4-deck)
│   │   │   ├── Deck1 WaveformDisplay
│   │   │   ├── Deck2 WaveformDisplay
│   │   │   └── Deck4 WaveformDisplay (4-deck)
│   │   └── Decks+Mixer+Library pane
│   │       ├── Deck1 (left)
│   │       ├── Mixer (center, 190px)
│   │       │   ├── EQ Columns (×2 or ×4)
│   │       │   ├── MixerColumns (×2 or ×4)
│   │       │   └── Crossfader
│   │       ├── Deck2 (right)
│   │       ├── Deck3 (under Deck1, 4-deck)
│   │       ├── Deck4 (under Deck2, 4-deck)
│   │       └── Library (bottom, Loader)
│   ├── Settings Popup
│   └── XYPad Overlay
├── OrientationToggleButton
└── Theme Singleton
```

---

*End of specification. Total checklist items: 195+*
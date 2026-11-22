# DJ Sugar Mixxx Fork - Android & Desktop UI Specification
Status: generated from source reading by research agents; each item must be checked against the source before it is built.

Source: /home/ubuntu/djsugar/res/qml (94 files, ~12.7k lines)
Android overrides: res/qml/+android, res/qml/Deck/+android, res/qml/Library/+android, res/qml/Effects/+android, res/qml/Settings/+android, res/qml/Theme/+android

---

## 1. Overall Screen Structure

### 1.1 Application Entry (main.qml)
- ApplicationWindow with design size 1792x1008 (desktop), fullscreen on mobile
- `isMobile` property: Qt.platform.os === "android" || Qt.platform.os === "ios"
- Background: Theme.backgroundColor (#1e1e20)
- MenuBar loaded from MainMenuBar.qml on macOS or when not fullscreen
- Skin switching support via Settings to Interface
- Window centering and clamping on desktop screens

### 1.2 Main Window (MainWindow.qml) - Desktop
Root Column with anchors.fill: parent
- **Toolbar** (height: 36, color: Theme.toolbarBackgroundColor #2e2e2e)
  - RowLayout with buttons: 4 Decks, Library, Effects, XY, Aux, Sampler, Edit, Develop, Preferences (gear icon)
  - Preferences button: press-and-hold opens native PreferencesDialog
- **SplitView** (vertical, fills remaining height)
  - **Top section (Waveforms)**: Item with SplitView.fillHeight: !library.active
    - 2-deck mode: deck1waveform (top half), deck2waveform (bottom half) — each height = parent.height / 2
    - 4-deck mode: deck3waveform (top 1/4), deck1waveform (2/4), deck2waveform (3/4), deck4waveform (bottom 1/4)
    - Side gradient fades on left/right edges (125px wide)
  - **SplitHandle**: 3 dots, color Theme.libraryPanelSplitterHandle (#5f5f5f), active #7a7a7a
  - **Bottom section (Mixer + Decks + Library)**: SplitView.fillHeight: library.active
    - **Deck1** (left of mixer): group [Channel1], height = mixer.height (or mixer.height/2 in 4-deck), minimized when maximizeLibrary
    - **Mixer** (center): groups [Channel1]-[Channel4], show4decks binding, visible when !maximizeLibrary
    - **Deck2** (right of mixer): group [Channel2], same height logic
    - **Deck3** (below deck1, 4-deck only): group [Channel3]
    - **Deck4** (below deck2, 4-deck only): group [Channel4]
    - **Library** (Loader): active when maximizeLibrary || height - mixer.height >= 400, anchors.bottom to parent.bottom, top to mixer.bottom

### 1.3 Main Window (+android/MainWindow.qml) - Android
Root Item with viewMode property (1=dj, 3=wave, 2=lib, 0=ctl)
- **Top Bar** (RowLayout, height: 40): view mode buttons (DJ, Wave, Lib, Ctl) — hidden items configurable via Settings
- **Content Stack** (Loader per view mode):
  - **View 1 (DJ)**: Deck cards (WaveDeckBar) + Mini mixer + Transport controls
  - **View 2 (CTL)**: Controller learning UI (LearnController)
  - **View 3 (WAVE)**: Full-screen waveform display
  - **View 4 (LIB)**: Full-screen library
- **Bottom Bar** (RowLayout, height: 48): Play/Pause, Sync, Cue, Load, FX, Sampler, Settings
- **Popups**: QuickLoadDrawer, BeatgridPopup, XYPad overlay, FX Station, Settings popup

### 1.4 Orientation Toggle (OrientationToggleButton.qml)
- Horizontal Fader (slider) binding to [ChannelN],orientation or [AuxiliaryN],orientation
- Range 0-2 (Left, Center, Right), stepSize 1, snapOnRelease
- Visual: 2px line background, 3px wide handle with 150ms animation
- Color: Theme.crossfaderOrientationColor (#747474)
- Size: implicitWidth 56, implicitHeight 26

---

## 2. Deck Component (Deck.qml)

### 2.1 Deck Structure (Desktop)
Item with dynamic layout model (itemModel) defining regions:
- **Column: Info** (top)
- **Row: Main Controls** (middle)
  - Column: Play + Cue buttons
  - Column: Toolbar + Row [HotcueAndStem (minWidth 520), BeatJump (minWidth 650), Loop]
- **Column: Tempo** (right side)
  - Pitch down/up buttons, Key display with keylock toggle, Rate fader
- **Column: FX Assign** (far right)
  - 4 effect unit assignment buttons + master mix toggle

### 2.2 Deck Structure (Android: Deck/+android/InfoBar.qml)
Rectangle with border (deckColor accent, deckBgColor background)
- **Deck Badge**: "DECK 1" / "DECK 2" / "DECK 3" / "DECK 4" pill (accentDim bg, accent border)
- **Cover Art Thumbnail**: 36x36 (minimized: 22x22)
- **Track Info Column**: Title (bold), Artist (secondary)
- **Progress Bar**: Elapsed / Remaining time toggle, progress fill with deckColor
- **BPM + Key Display**: BPM large, Key with color from pitchKey.colorsMap (24 colors)
- **Transport Row**: Reverse, Beatgrid, Eject, Play (green triangle), Cue (orange), Sync
- **Pad Switcher** (narrow decks): Pads / Loops / Stems tabs
- **Mini Waveform Overview** (WaveDeckBar): 2 deck cards side by side

### 2.3 Deck Controls & Mixxx Bindings

| Control | Mixxx Key | Group | Type | Size/Position | States | Colors |
|---------|-----------|-------|------|---------------|--------|--------|
| PlayButton | cue_gotoandplay | [ChannelN] | ControlButton | 40x40 (desktop), 48x48 (Android) | highlight when playing | active: samplerColor (#3a60be) |
| CueButton | cue_default | [ChannelN] | ControlButton | 40x40 | right-click sets cue | active: samplerColor |
| SyncButton | sync_enabled | [ChannelN] | ControlButton | 40x40 | toggleable | active: deckActiveColor |
| PitchDown | pitch_down | [ChannelN] | ControlButton | 20x22 | triangle left | fill #626262 |
| PitchUp | pitch_up | [ChannelN] | ControlButton | 20x22 | triangle right | fill #626262 |
| Keylock | keylock | [ChannelN] | ControlButton | fillWidth x 22 | toggleable | key colors from colorsMap |
| Rate Fader | rate | [ChannelN] | Fader | vertical, full height | live, snapOnRelease | bar: bpmSliderBarColor (#01dcfc) |
| Gain Knob | pregain | [ChannelN] | ControlKnob | 40x40 (desktop), 35x35 (Android) | arcStart Minimum | color: gainKnobColor (#3a60be) |
| EQ High | parameter3 | [EqualizerRack1_[ChannelN]_Effect1] | ControlKnob | 42x42 | arcStart Minimum | color: eqHighColor (white) |
| EQ Mid | parameter2 | [EqualizerRack1_[ChannelN]_Effect1] | ControlKnob | 42x42 | arcStart Minimum | color: eqMidColor (white) |
| EQ Low | parameter1 | [EqualizerRack1_[ChannelN]_Effect1] | ControlKnob | 42x42 | arcStart Minimum | color: eqLowColor (white) |
| Filter Knob | super1 | [QuickEffectRack1_[ChannelN]] | ControlKnob | 48x48 | arcStart Minimum | color: gainKnobColor |
| Filter Toggle | super1_enabled | [QuickEffectRack1_[ChannelN]] | ControlButton | 26x26 | toggleable | active: eqFxColor |
| PFL Button | pfl | [ChannelN] | ControlButton | 26x26 | toggleable | active: pflActiveButtonColor (#3a60be) |
| Volume Fader | volume | [ChannelN] | Fader | width 42, height = deck height | vertical | bar: volumeSliderBarColor (#3a60be) |
| Crossfader | crossfader | [Master] | Fader | horizontal, width 190 | | bar: crossfaderBarColor |
| Orientation Toggle | orientation | [ChannelN]/[AuxiliaryN] | OrientationToggleButton | 56x26 | 3 positions | color: crossfaderOrientationColor |

### 2.4 Spinny (Vinyl Emulation)
- **Desktop (Spinny.qml)**: 140px diameter circle, 45 deg rotated, center tick mark, tempo display inside (BPM + pitch %)
- **Android (Deck/+android/Spinny.qml)**: Adaptive size (80-140px), vinyl groove rings (2), accentColor border, center tempo box (44% height, 62% width)
- **TouchSpinny.qml**: Touch handler with scratch_enable + scratch_position controls, ghost indicator for slip mode
- Binds: track_samples, track_samplerate, vinylcontrol_speed_type, track_loaded, slip_enabled, scratch_position_enable, scratch_position

### 2.5 Waveform Overview (Deck/WaveformOverview.qml)
- Rectangle 50px height, radius 5, color deckBackgroundColor
- Contains Skin.WaveformOverview (Mixxx.WaveformOverview) filling parent
- Shows: cue markers (red), intro/outro (blue), loops (green), play position (white)
- Interactive: click/drag to seek (playposition control)

### 2.6 InfoBar (Deck/InfoBar.qml) - Desktop
Rectangle with horizontal gradient (track color darker to deckInfoBarBackgroundColor #0e0e0e)
- Cover art circle (left, 4px radius)
- DelegateChooser cells: title, year, time, artist, rating
- YouTube download progress bar (bottom 3px, gradient #e63380 to #ff99ca to #00e5ff)
- Minimized mode: height reduces, cover art hidden

### 2.7 Toolbar (Deck/Toolbar.qml)
Row with: Reverse (triangle left), Beatgrid button (text "Beatgrid", visible width>165), Eject (triangle up + line)
- Button colors: trackLoaded ? buttonActiveColor (white) : buttonDisableColor (lightGray)

### 2.8 BeatJump (Deck/BeatJump.qml)
- Label "Beatjump" (deckBeatjumpLabelColor #626262)
- BeatSizeSpinBoxBehavior: beatjump_size control, halve/double keys
- Backward/Forward buttons with custom Shape double-triangle icons
- Background: deckBeatjumpBackgroundColor (#696969)

### 2.9 Loop (Deck/Loop.qml)
- Label "Loop" (deckLoopLabelColor #626262)
- BeatSizeSpinBoxBehavior: beatloop_size (1/32 to 512 beats)
- In / Out / Recall-Exit buttons (toggleable when loop enabled)
- Loop halve/double buttons
- Background: deckLoopBackgroundColor (#626262)

### 2.10 TempoColumn (Deck/TempoColumn.qml)
- Pitch Down/Up buttons (Shape triangles)
- Key display: keylock toggle button with key text (1d, 8d, 3d... 3m) and color from colorsMap (24 colors)
- Rate fader (vertical Fader) with rate control

### 2.11 HotcueAndStem (Deck/HotcueAndStem.qml) - Desktop
Tabbed interface (Hotcue / Stems)
- **Hotcue tab**: 4x2 GridLayout (8 hotcues), each Skin.Hotcue with color, label, activate on press
- **Stem tab**: 4 stem buttons (Drums, Bass, Vocals, Other) with mute/solo, stem_name + mute_control + solo_control
- States: inactive (no track), hotcue (default or no stems), stem (when stems exist and selected)
- Colors: background #0E0E0E, hotcue border white when set, darkGray3/darkGray4 when empty

### 2.12 HotcueAndStem (Deck/+android/HotcueAndStem.qml) - Android
- 3-mode pad switcher: Hotcue (0), Loop (1), Stems (2) — visible when deck width < 520
- **Hotcue**: 4x2 grid, adaptive margins/spacing (narrow: 4px/3px, wide: 16px/6px)
- **Loop**: 4 loop buttons with beatloop_size spinbox
- **Stems**: 4 stem buttons with solo logic (soloStem function)
- Background #0E0E0E, tab buttons at right (Hotcue/Stem icons)

### 2.13 BeatgridPopup (Deck/BeatgridPopup.qml)
Popup anchored center, width min(440, parent-20), modal, dim=false
- Header: "Beatgrid – Deck N" + BPM display + close X
- Grid buttons (4 columns): Align, 1/2x, 2x, Shift Left, Shift Right, Reset
- Each binds to [ChannelN],beats_* controls
- Enabled only when track loaded and not bpmlock
- Clear button (not in this popup — in HotcuePopup)

### 2.14 FXAssign (Deck/FXAssign.qml)
Row of 4 effect unit assignment buttons + Main mix toggle
- Binds: [EffectRack1_EffectUnitN],group_[ChannelN]_enable
- OrientationToggleButton for crossfader orientation (Aux units)

---

## 3. Mixer Component (Mixer.qml)

### 3.1 Structure
Column with EqColumn + MixerColumn per deck (2 or 4 decks)
- **EqColumn** (width 42): 3-band EQ knobs (High/Mid/Low) + filter knob
- **MixerColumn** (width 42): Volume fader + Gain knob + PFL button
- 2-deck state: decks 1/2 centered
- 4-deck state: decks 1/3 left, decks 2/4 right
- Animated transitions (AnchorAnimation 200ms, SpringAnimation 500ms on height)

### 3.2 Mixer Controls

| Control | Mixxx Key | Group | Type | Size |
|---------|-----------|-------|------|------|
| EQ High | parameter3 | [EqualizerRack1_[ChannelN]_Effect1] | ControlKnob | 42x42 |
| EQ Mid | parameter2 | [EqualizerRack1_[ChannelN]_Effect1] | ControlKnob | 42x42 |
| EQ Low | parameter1 | [EqualizerRack1_[ChannelN]_Effect1] | ControlKnob | 42x42 |
| Filter | super1 | [QuickEffectRack1_[ChannelN]] | ControlKnob | 48x48 |
| Gain | pregain | [ChannelN] | ControlKnob | 42x42 |
| Volume | volume | [ChannelN] | Fader | 42x(full height) |
| PFL | pfl | [ChannelN] | ControlButton | 26x26, toggleable |

---

## 4. Library Component (Library.qml)

### 4.1 Desktop Structure
Item with:
- **SearchField** (TextField, top): placeholder "Search...", debounce 300ms, Enter/Down focuses track list, Escape clears
- **SuggestionsStrip** (Skin.SuggestionsStrip): "Up Next" cards, tap to load, long-press to queue in AutoDJ
- **SplitView** (horizontal):
  - **Left**: SourceTree (LibraryComponent.SourceTree) — sidebar with playlists, crates, drives
  - **Right**: TrackList (Browser.qml to TrackList.qml) — table view of tracks

### 4.2 Android Library (Library/+android/)
- **Browser.qml**: SourceTree with storage access handling for USB drives (Rekordbox, Engine DJ, computer)
- **SourceTree.qml**: Default columns = Cover Art (46px) + Title (fillSpan 3) + Artist + BPM + Key + Duration + Year + Rating
- **TrackList.qml**: HorizontalHeaderView with sortable columns, TouchSpinny not used here
- **Track.qml**: Item with TapHandler (left/right click), DragHandler disabled (TrackDrag handles drag), capabilities bitmask
- **Control.qml**: Focus management (sidebar vs library view), hardware knob mappings (MoveVertical, SelectTrackKnob, GoToItem, LoadSelectedIntoFirstStopped)

### 4.3 Library Columns (SourceTree.qml defaultColumns)
1. Cover Art (columnIdx: Album, preferredWidth 100, autoHideWidth 750) — Image delegate
2. Title (columnIdx: Title, fillSpan 3, label "Title") — DefaultDelegate
3. Artist (columnIdx: Artist, preferredWidth 100)
4. BPM (columnIdx: BPM, preferredWidth 50)
5. Key (columnIdx: Key, preferredWidth 50)
6. Duration (columnIdx: Duration, preferredWidth 60)
7. Year (columnIdx: Year, preferredWidth 50)
8. Rating (columnIdx: Rating, preferredWidth 50)
9. Position (for playlists, inserted after cover, preferredWidth 46)

### 4.4 Search & Suggestions
- SearchField: onTextChanged to searchDebounce (300ms) to applySearch() to sidebar.tracklist.search(text)
- SuggestionsStrip: pinned top, shows "Up Next" recommendations
- Focus shortcut: Ctrl+F / menu item to focusSearch()

---

## 5. Popups, Drawers & Menus

### 5.1 HotcuePopup (HotcuePopup.qml)
Popup, modal, closeOnEscape|CloseOnPressOutside
- 4xN Grid of color squares (from Mixxx.Config.hotcueColorPalette)
- Each: 24x24, click to hotcue.setColor(color) + close
- Clear button: "Clear", onPressed cueDefault=1, onReleased cueDefault=0
- Close X button top-right

### 5.2 BeatgridPopup (Deck/BeatgridPopup.qml) — detailed in 2.13

### 5.3 QuickLoadDrawer (QuickLoadDrawer.qml)
Popup, modal, dim=true, background #12151c with deckColor border (deck1: #00b0ff, deck2: #ff6d00)
- **Header** (50px): Deck target pill (DECK 1/2), switch deck button, source tabs (LOCAL MUSIC / STREAMING)
- **Content** (TabView):
  - Tab 0: Local Library (Library component with search)
  - Tab 1: Streaming Hub (StreamingHub with SoundCloud, YouTube, Spotify, Subsonic, Custom URL)
- Deck target: property targetDeck, defaults to [Channel1]

### 5.4 SamplePicker (SamplePicker.qml)
Item (not Popup) — used by Settings > Pads & FX and FX Station X-Pad
- `samples` array from Mixxx.PlayerManager.bundledSamples()
- `previewGroup` for preview deck, otherwise uses main mix with trim
- PillButton components for each sample (accent color, play icon, filled state)
- Trim popup for importing/recording new samples
- Signals: picked(path), adopt(path), preview()

### 5.5 FxStation (FxStation.qml)
Rectangle #0b0e13, signal closeRequested
- **Layout modes**:
  - Strip: height<560 && width>=900 && height>=220 (single row, replaces library)
  - Compact: !strip && (height<560 || width<760) (tabs, one section at a time)
  - Full: otherwise (two rows, all sections visible)
- **Sections** (each a RowLayout of FxButtons):
  - Scene (17 effects): BPF ECHO, ECHO, NOISE, SPIRAL UP, REVERB UP, HPF ECHO, LPF ECHO, CRUSH ECHO, SPIRAL DOWN, REVERB DOWN, FLANGER, PHASER, FILTER, ROLL, GATER, PITCH, DRIVE
  - Isolate (3 bands): Low, Mid, High — each with dial
  - X-Pad: XYPad component (embedded)
  - Release (3 effects): VINYL BRAKE, ECHO, BACK SPIN
- **FxButton**: Rectangle 4px radius, border 1px #323a48 (active: 2px accentColor), text centered, edit icon on hold
- Controls: [FxStation],scene_on, scene_type, scene_dial, scene_param1, scene_param2, isolate_mode, isolate_low/mid/high, xpad_x, xpad_y, release_on, release_type

### 5.6 XYPad (XYPad.qml)
Rectangle #e60a0d14 (semi-transparent dark), radius 12, border accentColor 40% alpha
- **Touch surface**: drag updates currentX (0=LPF, 0.5=neutral, 1=HPF) and currentY (0=dry, 1=wet)
- Binds: [QuickEffectRack1_[ChannelN]],super1 (filter) + [EffectRack1_EffectUnit1],mix (wet/dry) + routing group_[ChannelN]_enable
- **Modes**: SNAP (returns to saved state on release), HOLD (latches)
- Deck switcher: Deck 1 / Deck 2 pills
- Hold button (holdColor #ab47bc)
- Close button (closable property)
- State save/restore on first touch

### 5.7 StemsOverlay (StemsOverlay.qml)
Item with:
- **StemsQueuePopup**: modal, centered, width min(640, parent-20), shows StemsQueue component
- **StemsToast**: bottom toast (60px margin), green (#1e3a2a) success / red (#4a1e1e) error, auto-dismiss timer
- Opens via openQueue() called from skin indicator

### 5.8 MicrophoneDuckingPanel (MicrophoneDuckingPanel.qml)
Column, spacing 2:
- **Mode Button**: Skin.Button, text cycles Off to Auto to Manual, activeColor pflActiveButtonColor (#3a60be)
- **Strength Knob**: ControlKnob, arcStart Maximum, color effectUnitColor (red #ea2a4e), binds [Master],duckStrength
- ControlProxy: duckingControl on [Master],talkoverDucking (0=Off, 1=Auto, 2=Manual), nextMode() cycles

### 5.9 Settings Popup (Settings.qml)
Popup, modal, centered, width min(1400, parent), height min(840, parent), radius 8
- **Background**: Theme.darkGray2 (#2e2e2e), radius 8
- **Left Sidebar** (280px): ListView of categories (searchable)
  - Categories: Interface, Controllers, Audio, Library, Waveforms, Pads & FX, Stems, Recording, Broadcast, AutoDJ, Skins, About
- **Right Content**: StackLoader showing active category page
- **Search bar** (top of sidebar): filters categories
- **Save/Reset/Load** logic per category (dirty tracking)

### 5.10 Settings Pages

#### Interface (Settings/Interface.qml)
- Skin selector (grid of skin previews)
- Top bar items visibility toggles (MobileApp.hiddenTopBarItems)
- Waveform overview type (RGB/Filtered/HSV)
- Fullscreen startup toggle
- Language selector
- High DPI scaling

#### Controllers (Settings/Controller.qml)
- USB status bar (connected count, Rescan USB button)
- Controller cards grid (ControllerCard.qml)
- Mapping editor: learning mode (LearnController), device selection, MIDI mapping table
- Edit/duplicate/delete mappings

#### Audio (Settings/AudioEntity.qml, AudioRouter.qml, SoundHardware.qml)
- Audio device selection (input/output/sample rate/buffer)
- Audio routing graph (AudioRouter: visual node editor)
- Latency compensation
- Master/Headphone gain

#### Library (Settings/Library.qml)
- Library folders management
- Auto-analyze on import
- Cover art handling
- Playlist/crate import/export

#### Waveforms (Settings/Waveform.qml)
- Waveform type: Simple / Filtered / HSV / RGB / Stacked
- Overview type: Filtered / HSV / RGB (index 0/1/2)
- Overview stereo: on/off
- Overview minute markers: on/off
- Overview scaling: normalize / global gain
- Waveform enabled: on/off
- Frame rate: spinbox (default 60)
- Zoom synchronization: on/off
- Split stereo signal: on/off (RGB only)
- High detail: on/off
- Default zoom: spinbox (default 3)
- End of track warning: spinbox seconds (default 30)
- Beat grid alpha: slider 0-100 (default 90)
- Play marker position: slider 0-100% (default 50)
- Visual gains: All/Low/Mid/High sliders (default 2/1/1/1)
- Until mark: show beats/time, align (left/center/right), text size
- Stem opacity: 0.75, outline opacity: 0.15
- Stem reorder on change: on/off
- Stem display mode: stacked / overlap
- Caching enabled: on/off
- Generate with analysis: on/off

#### Pads & FX (Settings/Pads.qml)
- Sampler Bank A (pads 1-8) + Bank B (9-16) — PadGrid with SamplePicker
- Pad FX 1 (pads 1-8) + Pad FX 2 (9-16) — PadGrid with EffectSelector
- Reset buttons for each
- Beat FX Select configuration

#### Stems (Settings/Stems.qml)
- Model selector: Spleeter (fast, ~250MB) / HTDemucs (best, 2-2.5GB)
- Download buttons with progress
- Measure speed benchmark
- Memory warning for HTDemucs
- Queue management (pause/resume/cancel/clear)

#### Recording, Broadcast, AutoDJ, Skins, About
- Standard Mixxx settings panels

### 5.11 MainMenuBar (MainMenuBar.qml + ApplicationMenuActions.qml)
**File Menu**:
- Load Track to Deck 1 (Ctrl+O) to commands.loadTrackToDeck(1)
- Load Track to Deck 2 (Ctrl+Shift+O) to loadTrackToDeck(2)
- Load Track to Deck 3 (enabled if 4 decks) to loadTrackToDeck(3)
- Load Track to Deck 4 (enabled if 4 decks) to loadTrackToDeck(4)
- Separator
- Exit (Ctrl+Q) to Qt.quit()

**Library Menu**:
- Rescan Library (Ctrl+Shift+L) to Mixxx.Library.rescanLibrary()
- Export Library to Mixxx.Library.exportLibrary()
- Separator
- Search Current View to librarySearchCurrentView
- Search Tracks to librarySearchTracks
- Separator
- Create Playlist to libraryCreatePlaylist
- Create Crate to libraryCreateCrate

**View Menu**:
- Show Microphone to viewShowMicrophone
- Show Vinyl Control to viewShowVinylControl
- Show Preview Deck to viewShowPreviewDeck
- Show Cover Art to viewShowCoverArt
- Show Keywheel to viewShowKeywheel
- Maximize Library to viewMaximizeLibrary
- Separator
- Show AutoDJ to viewShowAutoDJ
- Full Screen to viewFullScreen

**Options Menu**:
- Vinyl Control submenu (enabled if vinylControlAvailable):
  - Enable Vinyl 1-4 (each with enabled binding)
- Enable Keyboard Shortcuts to optionsEnableKeyboardShortcuts
- Enable Live Broadcasting to optionsEnableLiveBroadcasting
- Separator
- Preferences (Ctrl+P / Ctrl+,) to optionsPreferences

**Help Menu**:
- User Manual (external link) to helpUserManual
- Keyboard Shortcuts to helpKeyboardShortcuts
- Community Support to helpCommunitySupport
- Translate to helpTranslate
- Settings Directory to helpSettingsDirectory
- Separator
- About to helpAbout

**Developer Menu** (when developerMode):
- Show Developer Tools to developerTools
- Reload Skin to developerReloadSkin
- Base Stats to developerBaseStats
- Experiment Stats to developerExperimentStats
- Debugger to developerDebugger

---

## 6. Gestures

### 6.1 Touch Spinny (TouchSpinny.qml)
- **Scratch**: PointerHandler on indicator, drag to scratch_position_enable=1, scratch_position = mapped angle
- **Vinyl speed**: RPM from vinylcontrol_speed_type (default 33.33), calculates rps
- **Ghost indicator**: Shows slip position when slip_enabled, opacity animated
- **Release**: scratch_position_enable=0

### 6.2 XYPad Touch
- Drag on surface to updates currentX (filter LPF<->HPF) and currentY (dry<->wet)
- Binds to [QuickEffectRack1_[ChannelN]],super1 and [EffectRack1_EffectUnit1],mix
- Snap/Hold modes via buttons

### 6.3 Library Swipe/Scroll
- Flickable in TrackList, SourceTree
- Hardware knob: [Library],MoveVertical / [Playlist],SelectTrackKnob
- TapHandler on Track (Android): left click selects, right click opens context menu

### 6.4 Long Press
- Preferences button (MainWindow toolbar): press-and-hold to Mixxx.PreferencesDialog.show()
- Pad buttons (HotcueAndStem): press-and-hold to opens HotcuePopup (color picker)
- Effect buttons (FxStation): press-and-hold to shows edit icon to opens EffectSlotPresetMenu
- Library tracks: long-press to queue in AutoDJ (SuggestionsStrip)

### 6.5 Waveform Seek
- MouseArea on WaveformOverview: click/drag to playposition = x/width
- Touch on WaveformDisplay: same

---

## 7. Waveform Display Options

### 7.1 WaveformDisplay (WaveformDisplay.qml)
MixxxControls.WaveformDisplay filling parent, group binding
- **Renderers** (selected by Mixxx.Config.waveformType):
  - Simple: axesColor #a1a1a1a1
  - Filtered: lowColor #2154D7, midColor #97632D, highColor #D5C2A2, gains from Config
  - HSV: hue/saturation/value mapping
  - RGB: three color channels
  - Stacked: Filtered with stacked=true
- **Markers** (WaveformRendererMarkRange):
  - Loop: color #00b400, disabled #FFFFFF, opacity 0.7, controls loop_enabled/start/end
  - Intro: color #2c5c9a, durationTextColor white, location after, controls intro_start/end
  - Outro: color #2c5c9a, durationTextColor white, location before, controls outro_start/end
- **End of Track**: WaveformRendererEndOfTrack color #ff8872, warningTime from Config
- **Preroll**: WaveformRendererPreroll color #ff8872
- **Zoom**: Binding to zoomControl (waveformDefaultZoom), SmoothedAnimation 500ms
- **Frame Rate**: Mixxx.Config.waveformFrameRate

### 7.2 WaveformOverview (Mixxx/Controls/WaveformOverview.qml)
- Minute markers: Mixxx.Config.waveformOverviewMinuteMarkers
- Normalized: Mixxx.Config.waveformOverviewNormalized
- Renderer: Filtered (0) / HSV (1) / RGB (2) per waveformOverviewType
- Stereo: Mixxx.Config.waveformOverviewStereo
- Analyzer status text: "Ready to play, analyzing..." / "Finalizing..." / "Loading track..."
- Interactive seek: MouseArea click/drag to playposition

### 7.3 Deck WaveformOverview (Deck/WaveformOverview.qml)
- 50px height, radius 5, deckBackgroundColor
- Embedded Skin.WaveformOverview

---

## 8. Theme Colors (Theme/Theme.qml + Theme/+android/Theme.qml)

| Property | Desktop Hex | Android Hex (if different) |
|----------|-------------|---------------------------|
| accentColor | #3a60be | #00e5ff |
| backgroundColor | #1e1e20 | #0f1319 |
| blue | #01dcfc | #00e5ff |
| buttonActiveColor | white | #ffffff |
| buttonDisableColor | lightGray (#747474) | #626262 |
| buttonNormalColor | midGray (#696969) | — |
| crossfaderBarColor | blue (#01dcfc) | — |
| crossfaderOrientationColor | lightGray (#747474) | — |
| darkGray | #0f0f0f | — |
| darkGray2 | #2e2e2e | #14171f |
| darkGray3 | #3F3F3F | #2a3648 |
| darkGray4 | #202020 | — |
| deckActiveColor | white | #ffffff |
| deckBackgroundColor | darkGray (#0f0f0f) | #181a20 |
| deckBeatjumpBackgroundColor | midGray3 (#626262) | — |
| deckBeatjumpLabelColor | darkGray3 (#3F3F3F) | — |
| deckEmptyCoverArt | darkGray3 (#3F3F3F) | — |
| deckInfoBarBackgroundColor | #0e0e0e | — |
| deckLineColor | darkGray2 (#2e2e2e) | — |
| deckLoopBackgroundColor | midGray3 (#626262) | — |
| deckLoopLabelColor | darkGray3 (#3F3F3F) | — |
| deckTextColor | lightGray2 (#b0b0b0) | — |
| effectColor | yellow (#fca001) | — |
| effectUnitColor | red (#ea2a4e) | #ff4081 |
| embeddedBackgroundColor | #a0000000 | — |
| eqFxColor | red (#ea2a4e) | — |
| eqHighColor | white | — |
| eqLowColor | white | — |
| eqMidColor | white | — |
| fontFamily | "Open Sans" | "Open Sans" |
| gainKnobColor | blue (#3a60be) | — |
| green | #85c85b | — |
| knobBackgroundColor | #262626 | — |
| libraryPanelSplitterBackground | #1e1e1e | — |
| libraryPanelSplitterHandle | #5f5f5f | — |
| libraryPanelSplitterHandleActive | #7a7a7a | — |
| lightGray | #747474 | — |
| lightGray2 | #b0b0b0 | — |
| lightGray3 | #939393 | — |
| midGray | #696969 | — |
| midGray2 | #676767 | — |
| midGray3 | #626262 | — |
| panelSplitterBackground | backgroundColor | — |
| panelSplitterHandle | midGray | — |
| panelSplitterHandleActive | lightGray2 | — |
| pflActiveButtonColor | blue (#3a60be) | — |
| red | #ea2a4e | — |
| samplerColor | blue (#3a60be) | #00e676 |
| sunkenBackgroundColor | #0C0C0C | — |
| textColor | lightGray2 (#b0b0b0) | — |
| textFontPixelSize | 14 | — |
| toolbarActiveColor | white | — |
| toolbarBackgroundColor | darkGray2 (#2e2e2e) | #1c202a |
| volumeSliderBarColor | blue (#3a60be) | — |
| warningColor | #7D3B3B | — |
| waveformBeatColor | lightGray (#747474) | — |
| waveformCursorColor | white | — |
| waveformMarkerDefault | #ff7a01 | — |
| waveformMarkerIntroOutroColor | #2c5c9a | — |
| waveformMarkerLabel | rgba(255,255,255,0.8) | — |
| waveformMarkerLoopColor | #00b400 | — |
| waveformMarkerLoopColorDisabled | #FFFFFF | — |
| waveformPostrollColor | midGray | — |
| waveformPrerollColor | midGray | — |
| white | #D9D9D9 | #ffffff |
| yellow | #fca001 | — |

**Android-specific additions (Theme/+android/Theme.qml)**:
- Additional colors for deck accents: deck1 #00b0ff, deck2 #ff6d00, deck3 #00e676, deck4 #e040fb
- Surface colors: #080b10, #1a222d, #0f131a, #181b22
- Button colors: #303e4f borders, #90a4ae text

---

## 9. Android-Specific Differences Summary

| Aspect | Desktop | Android |
|--------|---------|---------|
| Window | 1792x1008, windowed | Fullscreen, Screen.width/height |
| Layout | Single SplitView with all regions | View modes (DJ/Wave/Lib/Ctl) via top bar |
| Deck | Full component with all controls | Compact InfoBar + Spinny + Pad switcher |
| Mixer | Central column with EQ+Volume | Mini mixer in DJ view, full in Wave view |
| Library | Sidebar + TrackList in SplitView | Full-screen in Lib view, drawer in DJ view |
| Waveform | 2-4 stacked over mixer | Full-screen in Wave view, mini in Deck cards |
| FX Station | Separate window/panel | Integrated in DJ view (strip) or full-screen |
| Settings | Popup centered | Full-screen or large popup |
| Menus | Native MenuBar (macOS) or in-window | No MenuBar, all via Settings popup |
| Touch | Mouse + limited touch | Full touch: spinny scratch, XYPad, swipe |
| Colors | Dark theme (#1e1e20 bg) | Darker (#0f1319 bg), neon accents |
| Orientation | OrientationToggleButton in Aux | Same, plus deck orientation in mixer |

---

## 10. Numbered Checklist of Distinct UI Features (150+ Items)

1. ApplicationWindow with mobile/desktop detection
2. Design resolution 1792x1008 (desktop)
3. Fullscreen on Android/iOS
4. Theme singleton with 88 color/size properties
5. Android Theme override with neon accents
6. Skin switching via Settings to Interface
7. MainWindow toolbar (36px) with 9 toggle buttons
8. 4 Decks toggle (show/hide decks 3&4)
9. Library toggle (maximize library panel)
10. Effects toggle (show effect rack)
11. XY Pad toggle (floating overlay)
12. Aux toggle (show auxiliary units)
13. Sampler toggle (show sampler row)
14. Edit mode toggle (deck editing)
15. Developer tools toggle
16. Preferences button (click=Settings popup, hold=native dialog)
17. Vertical SplitView (waveforms / mixer+decks+library)
18. Horizontal SplitView handle (3 dots, animated)
19. Waveform stack: 2-deck (50/50) or 4-deck (25% each)
20. Deck1 waveform ([Channel1])
21. Deck2 waveform ([Channel2])
22. Deck3 waveform ([Channel3], 4-deck only)
23. Deck3 waveform ([Channel4], 4-deck only)
24. Side gradient fades on waveform edges
25. Deck1 component ([Channel1], left of mixer)
26. Deck2 component ([Channel2], right of mixer)
27. Deck3 component ([Channel3], below deck1, 4-deck)
28. Deck4 component ([Channel4], below deck2, 4-deck)
29. Mixer component (center, 2 or 4 channels)
30. Mixer EQ columns (High/Mid/Low + Filter per deck)
31. Mixer volume faders (vertical, per deck)
32. Mixer gain knobs (per deck)
33. Mixer PFL buttons (per deck, toggleable)
34. Library panel (Loader, conditional activation)
35. Library SearchField (debounced 300ms)
36. SuggestionsStrip (Up Next cards)
37. Library SplitView (SourceTree | TrackList)
38. SourceTree sidebar (playlists, crates, drives)
39. TrackList table (sortable columns)
40. Track columns: Cover, Title, Artist, BPM, Key, Duration, Year, Rating, Position
41. OrientationToggleButton (3-position slider for crossfader/orientation)
42. Deck PlayButton (cue_gotoandplay, green on Android)
43. Deck CueButton (cue_default, orange on Android)
44. Deck SyncButton (sync_enabled)
45. Deck PitchDown/PitchUp buttons
46. Deck Keylock toggle (24-color key display)
47. Deck Rate fader (vertical)
48. Deck Gain knob (pregain)
49. Deck EQ High/Mid/Low knobs
50. Deck Filter knob (super1)
51. Deck Filter toggle button
52. Deck PFL button
53. Deck Volume fader
54. Deck Crossfader (horizontal, Master)
55. Desktop Spinny (140px, vinyl + tempo display)
56. Android Spinny (adaptive 80-140px, groove rings, tempo box)
57. TouchSpinny (scratch handler, ghost slip indicator)
58. WaveformOverview (50px, interactive seek)
59. InfoBar (gradient, cover art, track metadata cells)
60. YouTube download progress bar (bottom gradient)
61. Toolbar (Reverse, Beatgrid, Eject)
62. BeatJump panel (size spinbox, backward/forward)
63. Loop panel (size spinbox, In/Out/Recall, halve/double)
64. TempoColumn (pitch buttons, key display, rate fader)
65. HotcueAndStem desktop (8 hotcues 4x2 grid + 4 stem buttons)
66. HotcueAndStem Android (3-mode pad switcher: Hotcue/Loop/Stem)
67. Hotcue color popup (4-column grid + Clear)
68. BeatgridPopup (align, 1/2x, 2x, shift L/R, reset)
69. FXAssign row (4 effect units + Main mix + OrientationToggle)
70. Android MainWindow view modes (DJ/Wave/Lib/Ctl)
71. Android top bar (4 view buttons, configurable hidden items)
72. Android DJ view (WaveDeckBar + mini mixer + transport)
73. Android Wave view (full-screen waveform)
74. Android Lib view (full-screen library)
75. Android Ctl view (LearnController)
76. Android bottom bar (Play, Sync, Cue, Load, FX, Sampler, Settings)
77. WaveDeckBar (2 deck cards with transport, waveform, metadata)
78. QuickLoadDrawer (deck target, Local/Streaming tabs)
79. StreamingHub (SoundCloud, YouTube, Spotify, Subsonic, Custom URL)
80. SamplePicker (sampler browser + preview + trim/import)
81. FxStation (Scene/Isolate/X-Pad/Release sections)
82. FxStation layout modes (strip/compact/full)
83. Scene buttons (17 effects, editable mapping)
84. Isolate dials (Low/Mid/High)
85. X-Pad (embedded XYPad)
86. Release buttons (3 effects)
87. XYPad standalone (filter X, wet/dry Y, Snap/Hold, deck switcher)
88. StemsOverlay (queue popup + toast notifications)
89. StemsQueue (job list with progress, pause/resume/cancel/clear)
90. MicrophoneDuckingPanel (Off/Auto/Manual + strength knob)
91. Settings popup (sidebar categories + content stack)
92. Settings: Interface (skin, top bar items, waveform, language)
93. Settings: Controllers (USB status, cards, mapping editor, learning)
94. Settings: Audio (device, routing graph, latency)
95. Settings: Library (folders, analyze, cover art)
96. Settings: Waveforms (15+ options: type, overview, gains, stems, caching)
97. Settings: Pads & FX (Sampler banks A/B, Pad FX 1/2, Beat FX)
98. Settings: Stems (Spleeter/HTDemucs download, measure, queue)
99. Settings: Recording, Broadcast, AutoDJ, Skins, About
100. MainMenuBar (File/Library/View/Options/Help/Developer)
101. File: Load Deck 1-4, Exit
102. Library: Rescan, Export, Search, Create Playlist/Crate
103. View: Microphone, Vinyl, Preview, Cover Art, Keywheel, Maximize Lib, AutoDJ, Fullscreen
104. Options: Vinyl 1-4, Keyboard Shortcuts, Live Broadcast, Preferences
105. Help: Manual, Shortcuts, Community, Translate, Settings Dir, About
106. Developer: Tools, Reload Skin, Stats, Debugger
107. TouchSpinny scratch gesture (pointer drag to scratch_position)
108. XYPad drag gesture (filter + wet/dry)
109. Library flickable scroll (touch + hardware knob)
110. Waveform seek (click/drag on overview)
111. Long press: Preferences to native dialog
112. Long press: Hotcue to color picker popup
113. Long press: Effect button to preset menu
114. Long press: Suggestion card to AutoDJ queue
115. Hardware knob: Library MoveVertical
116. Hardware knob: Playlist SelectTrackKnob
117. Hardware button: Library GoToItem (load selected)
118. Hardware button: Playlist LoadSelectedIntoFirstStopped
119. Android storage access (Rekordbox/Engine DJ/Computer drives)
120. Android USB permission requests
121. Deck minimized state (80px height, compact info)
122. Deck focus toggle (expands focused deck on narrow screens)
123. SpringAnimation on deck height changes (500ms, damping 0.2)
124. AnchorAnimation on mixer position (200ms)
125. FadeBehavior on visibility changes (waveforms, mixer, library)
126. Waveform renderer switching (Simple/Filtered/HSV/RGB/Stacked)
127. Waveform markers: Cue (red), Intro/Outro (blue), Loop (green), Play (white)
128. Waveform end-of-track warning (configurable time)
129. Waveform preroll highlight
130. Waveform analyzer status text
131. Waveform minute markers toggle
132. Waveform normalized/global gain toggle
133. Waveform stereo/split stereo toggle
134. Waveform zoom synchronization toggle
135. Waveform beat grid alpha slider
136. Waveform play marker position slider
137. Waveform visual gains (All/Low/Mid/High)
138. Waveform until-mark options (beats/time/align/text size)
139. Waveform stem opacity/outline/reorder/display mode
140. Waveform caching toggle
141. Waveform generation with analysis toggle
142. Sampler row (commented out in MainWindow)
143. Effect row (commented out in MainWindow)
144. EffectUnit component (3 slots + controls: expand, preset, PFL, D/W, mix knob)
145. EffectSlot (selector, enable, meta knob, parameter list when expanded)
146. EffectChainPresetPopup (standard presets + update/rename/save)
147. EffectSlotPresetMenu (parameter visibility + save snapshot)
148. LearnController (7 sections, 50+ mappable functions per deck)
149. Controller learning flow (start/cancel, learned feedback)
150. ApplicationMenuActions (all menu items with shortcuts)
151. MobileApp.hiddenTopBarItems (persisted hidden top bar items)
152. Deck card states (inactive/hotcue/stem via stem_count)
153. Hotcue activator (press=activate, hold=color popup)
154. Stem solo logic (mutual exclusion)
155. Pad switcher tabs (Hotcue/Loop/Stem) on narrow Android decks
156. BeatSizeSpinBoxBehavior (halve/double keys for beatjump/beatloop)
157. ControlKnob with arcStart Minimum/Maximum variants
158. ControlFader with live/snap modes
159. ControlButton with toggleable/checkable states
160. EmbeddedBackground (semi-transparent overlay)
161. SectionBackground (section.svg image)
162. SectionText (styled labels)
163. Triangle shape component (for play/cue/reverse icons)
164. EditIcon (pencil, shows on editable items)
165. FadeBehavior (opacity animation on visible change)
166. ColorPaletteComboBox (for hotcue/stem colors)
167. RatioChoice (for aspect ratios)
168. SpinBox (numeric input with arrows)
169. FormButton (styled button for settings)
170. InfoBubble (tooltips)
171. ControllerCard (controller device representation)
172. ControllerMappingLink (mapping info link)
173. ControllerScreens (controller screen config)
174. AudioEntity (audio device node)
175. AudioEntityEdge (routing connection)
176. AudioRouter (visual routing graph editor)
177. AutoDJ settings panel
178. Broadcast settings panel
179. Recording settings panel
180. StatsPerformance panel
181. DefaultColorSelector (color picker)
182. MixerEffect (effect rack item)
183. Stems.js (stem name localization: Drums/Bass/Vocals/Other)
184. DurationFormatter (time formatting: Traditional/Seconds/Kilo/Hecto)
185. WaveformOverviewHotcueMarker (hotcue markers on overview)
186. WaveformOverviewMarker (generic position marker)
187. WaveformOverviewMarkerLayer (cue/intro/loop labels)
188. KeyColorIndicator (camelot wheel color)
189. PlayerDropArea (drag-drop track loading)
170. ControlLoadSelectedTrackHandler (library to deck loading)
171. TrackDrag (drag from library with track URL mime)
172. TrackMenu (context menu for tracks)
173. LibraryCell (track list cell with waveform preview, disabled)
174. LibraryComponent.Track (base track component)
175. LibraryComponent.Control (focus management)
176. LibraryComponent.SourceTree (sidebar model)
177. LibraryComponent.Browser (track list view)
178. LibraryComponent.ControlLoadSelectedTrackHandler
179. Streaming views: OnlineStreamView, SoundCloudView, YouTubeView, SpotifyView, SubsonicView, CustomUrlView, WebView
180. StreamingControl (unified streaming UI)
181. StreamingHub (tabbed streaming services)
182. AboutDialog (version, credits, license)
183. AppUpdateBanner / AppUpdateInstallPopup (auto-update UI)
184. ActionButton / ActionPopup (contextual popups with arrow)
185. InputField (styled text input)
186. ArrowText (text with arrow indicator)
187. SamplerRow (sampler bank UI, commented)
188. EffectRow (effect rack UI, commented)
189. AuxiliaryUnit (aux send with orientation + main mix + FX assign)
190. ControlMiniKnob (small knob for gain/meta)
191. VuMeter (vertical/horizontal level meter)
192. EmbeddedText (styled text in embedded bg)
193. PillButton (rounded button with accent)
194. Card (settings card container)
195. ModelRow (stem model download row)
196. PadGrid (2x4 pad configuration grid)
197. EffectSelector (effect type dropdown)
198. EffectFocusButton (focus indicator)
199. EffectPresetMenuItem / Separator (menu items)
200. Settings Category base component
201. ControllerManager integration (knownDevices, rescan)
202. MobileLibrary integration (playlists, USB access)
203. Mixxx.Stems integration (jobs, models, queue, toast)
204. Mixxx.EffectsManager integration (units, slots, presets, catalogs)
205. Mixxx.PlayerManager integration (players, bundled samples, fx station)
206. Mixxx.Config integration (waveform, library, app settings)
207. Mixxx.Application integration (menu shortcuts, developer mode)
208. Mixxx.Core ready state handling
209. Mixxx.ControllerLearning integration (mapping creation)
210. Mixxx.MobileApp integration (Android top bar, hidden items)
211. Mixxx.YouTube integration (deck download progress)
212. Mixxx.Library integration (rescan, export, search)
213. Mixxx.TrackListColumn / SQLColumns (library schema)
214. Mixxx.WaveformDisplay / WaveformOverview types and options
215. Mixxx.WaveformRenderer variants (Simple/Filtered/HSV/RGB/Stacked/MarkRange/EndOfTrack/Preroll)
216. Mixxx.ControlProxy pattern (group/key binding with updateInterval)
217. Mixxx.SpinnyPosition (vinyl position data)
218. Mixxx.EffectUnitProxy / EffectSlotProxy (effect introspection)
219. Mixxx.EffectSlotParametersModel (parameter list for UI)
220. Mixxx.FxStation controls (scene/isolate/xpad/release)
221. Mixxx.DurationFormatter (time display modes)
222. Mixxx.TrackListColumn delegate system (DefaultDelegate)
223. Mixxx.TrackListColumn fillSpan/autoHideWidth responsive columns
224. QtQuick.Controls 2.12/2.15 (Popup, Menu, Button, Slider, etc.)
225. QtQuick.Layouts (RowLayout, ColumnLayout, GridLayout, SplitView)
226. QtQuick.Shapes (custom button icons: triangles, eject, etc.)
227. Qt5Compat.GraphicalEffects (DropShadow, OpacityMask)
228. Qt.labs.qmlmodels / QtQml.Models (DelegateChooser, Instantiator)

---

Total items: 228 distinct UI features catalogued
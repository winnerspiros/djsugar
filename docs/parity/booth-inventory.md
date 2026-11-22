# Booth Skin Inventory (DJ Sugar Mixxx Fork)

Reference: `/home/ubuntu/djsugar/res/qml/Skins/Booth/*.qml`, shared `/home/ubuntu/djsugar/res/qml/` (Theme, Mixxx/Controls, Library, Skin/), Android overrides in `+android/`, `src/main.cpp` for scale. Rust app: `/home/ubuntu/djsugar-next/crates/app/src/{booth.rs,state.rs,sugar.rs,lib.rs}`.

---

## 1. TOP BAR (MainWindow.qml:512-610)

**Geometry**
- Height: `compact ? 26 : 34` (compact = height < 520)
- Left margin: 4, top margin: 3
- Spacing: `width < 760 ? 3 : 6`
- Two rows on narrow screens (`width < 760`)

**Tabs (Tab component:269-304)**
- `WAVE` (view 1): active when `viewMode === 1`
- `DJ` (view 3): active when `djView`
- `BROWSE` (view 2): active when `browsing && loadTarget.empty`
- `LOAD 1`: active when `browsing && loadTarget == "[Channel1]"`
- `LOAD 2`: active when `browsing && loadTarget == "[Channel2]"`
- Right row: `SPLIT` (headSplit), `REC` (MobileRecordButton), `FX ST` (fxStationLoader), `XY` (view 4), `BEAT FX` (compact only), `STEMS` (stemsOverlay), `UTILITY` (settingsPopup)
- Tab width: `width < 760 ? max(44, textW+12) : max(compact?64:96, textW+24)`
- Tab font: bold, `width<760?10:compact?11:13` px
- Active color: `accentColor (#ff4fa8)`, inactive: `lightGray3 (#999999)`
- Pressed bg: `darkGray3 (#14161c)`, border: `active?accent:midGray3 (#333333)`, width `active?2:1`
- TapHandler only (no double-tap on Android)

**SugarLogo** (521-525): size `compact?24:30`, wordmark when `width>=1000`

**MobileRecordButton** (571-578): height = parent, font `width<760?10:compact?11:13`, idle border `midGray3`, idle bg `backgroundColor`, radius 0

---

## 2. VIEW MODES (MainWindow.qml:25-30, 178-195)

| Mode | Value | Condition |
|------|-------|-----------|
| WAVE | 1 | Waveforms + deck panels |
| BROWSE | 2 | Library browser (BoothBrowse) |
| DJ | 3 | Touch controls (BoothDjView) |
| XY | 4 | XY pad (Skin.XYPad) |

- `compact = height < 520`
- `controllerConnected`: any enabled Mixxx.ControllerManager device
- `preferredView()`: controller? [1,3,2] : [3,1,2], first shown
- `beatFxShown`: phone overlay (compact)
- `beatFxPanelShown`: tablet panel (right of waveforms)
- `fxStationLoader`: FX Station overlay/panel

---

## 3. WAVE VIEW (viewMode=1)

### 3.1 Deck Panels (deckPanels:613-642)
- Visible when `!fxStationLoader.covering`
- Anchored above `djView` (or bottom), margins `panelInset + gap(3)`
- Height: `viewMode!==1 ? compact?58:84 : round(height*(compact?0.36:0.3))`
- Two `BoothDeckPanel`, each half width minus gap

### 3.2 DeckInfo Column (deckInfos:681-702)
- Visible `!(djView && compact)`
- Width: `compact?86:124`
- Two `DeckInfo` (DeckInfo component:306-478), each half height

### 3.3 Waveforms (712-725)
- Two `Skin.WaveformDisplay` (group Channel1/2), `isTouchMode:true`, `showHud:false`
- Each half height, full width between deckInfos and beatFx

### 3.4 BeatFx Panel (beatFx:727-738)
- Visible `!compact && itemShown("fx") && beatFxPanelShown`
- Width: 180, anchored right with `middleInset + gap`
- Compact prop: `djView`

### 3.5 Phone BeatFx Overlay (787-796)
- Visible `compact && beatFxShown && !browsing && !xyView && itemShown("fx")`
- Width: 190, z:10, compact:true

---

## 4. BOOTHDECKPANEL (BoothDeckPanel.qml)

**Geometry**
- `unit = max(9, min(height/11, width/30))`
- Border: 1px, `playing?lightGray2:midGray3` (#626262)
- Bg: `backgroundColor (#000000)`
- Title bar height: `round(unit*2.3)`
- Info area height: `round(unit*4.4)`, hidden when `slim:true`
- WaveformOverview: bottom margin 3, left 4, right 4, height `max(36, round(height*0.28))`

**Title Bar (144-260)**
- YouTube download progress bar (Behavior on width, 140ms OutCubic)
- Deck label: "DECK" + `playing?((N)):N`, color `playing?warningColor:#ff6b6b : buttonActiveColor(#ffffff)`
- Track title: `loaded? "♪ "+title : ""`, color `buttonActiveColor`, elide right
- Key label: `loaded? keyText : ""`, color `buttonActiveColor`, font `unit*1.3`
- Download gradient bar (Behavior 140ms OutCubic)

**Info Area (262-472)**
- Chips row (MASTER TEMPO, SYNC): height `unit*0.95`, label color `lightGray3`, onColor `warningColor`/`libraryDeck1Background`, border `on?onColor:lightGray3`
- Time display: Caption "REMAIN"/"TIME", digits font `unit*3.4`, ms font `unit*1.7`, scale to fit (`fit = min(1, width/implicitWidth)`), tap toggles `showElapsed`
- Tempo: Caption "TEMPO", range box (color `libraryDeck2Background`, text `rateRange*100%`), pitch value font `unit*1.45`, color `loaded?#d8dadd:midGray3`
- BPM box: border `leader?warningColor:lightGray2`, "BPM" caption, "MASTER" badge when leader, whole font `unit*2.6`, decimal `unit*1.5`, tap opens BeatgridPopup

**WaveformOverview (476-487)**
- `Skin.WaveformOverview` (overviewTypeRgb=2), cue markers, play position (updateInterval:33)

**Animations**
- YouTube progress bars: `Behavior on width { NumberAnimation { duration: 140; easing.type: Easing.OutCubic } }` (lines 173, 255)

---

## 5. DECKINFO (MainWindow.qml:306-478)

**Geometry**
- `rowHeight = max(18, min(34, (height-headerHeight)/4))`
- Header: `compact?18:24`, bg `midGray3`, text "DECK N", font `compact?10:12`, bottom line 2px deck color
- 2 rows (key, barsLeft): height `rowHeight`, bg `darkGray4`, border `midGray3`, value font `rowHeight*0.55`, unit font `rowHeight*0.36`
- 2 toggle rows (QUANTIZE, SLIP): height `rowHeight`, border `on?warningColor:midGray3`, bg `on?#3a1a08:darkGray4`, label font `rowHeight*0.42`, tap toggles

**Live Data**
- `barsLeft`: beats to end, format `BB.B` (bars.beat)
- `positionControl.updateInterval: 100`
- `bpmControl`, `quantizeControl`, `slipControl`

---

## 6. DJ VIEW (BoothDjView.qml)

**Geometry**
- `gap = 3`
- Top row: Deck1 (fill), Mixer (prefWidth `min(w*0.24, h*0.9)`), Deck2 mirrored (fill)
- Bottom bar: height `compact?30:40`, border `midGray3`, bg `darkGray4`
- Bottom row: Headphones knob (size `h-4`), label "MIXING", crossfader horizontal (width `min(w*0.3, 320)`, centerMark), Master knob, label "MASTER"
- All knobs `BoothKnob`, bipolar for headMix, unipolar for headGain/master

**BoothPerformanceDeck (BoothPerformanceDeck.qml)**
- `unit = max(9, min(h/14, w/34))`, bg `darkGray4`, border `midGray3`
- Pad modes: hotcue, beatjump, beatloop, stem, sampler (tap cycles)
- Pads 2x4 grid, each Pad component (169-326):
  - Hotcue: letter A-H, time when set, color from `hotcue_N_color`, long press opens HotcuePopup
  - Beatjump: left half back, right half forward, labels FINE,1/8,1/4,1/2,1,2,4,8, triangles
  - Beatloop: sizes 1/4,1/2,1,2,4,8,16,32 beats, lit when active size
  - Stem: 4 stems x2 rows (MUTE/FX), colors blue/accent/yellow/activeColor
  - Sampler: SamplerPad (SamplerPad.qml), 8 per deck side
- Loop column (520-655):
  - LOOP label, size box (tap toggles, border `looping?#ff6a13:midGray3`, bg `looping?warningColor:darkGray4`), EXIT/BEATS sublabel
  - 1/2 and x2 buttons, IN/OUT buttons (armed state)
  - GRID button (opens BeatgridPopup), SLIP button
- Transport (686-772):
  - SYNC/MASTER button (hold for leader), Q button, MT button
  - CUE circle (border `cue_indicator?#ff9a1f:#8a5a20`, radius 50%)
  - PLAY circle (border `play?green:#2a6a2a`, shows pause bars or play triangle)
- Jog (776-881):
  - Platter size `min(w,h)`, border `midGray2`, width `max(2, size*0.04)`, inner ring
  - `DeckComponent.TouchSpinny` with indicator (deck color bar at top)
  - Center: BPM whole `size*0.17`, decimal `size*0.09`, rate% `size*0.075`
  - Q/MT buttons bottom corner
- Tempo fader (884-892): `BoothFader`, inverted, centerMark, color `deckColor`, width `unit*2.6`

**Animations/Live**
- TouchSpinny: continuous rotation from `spinnyPosition.playPosition`, `roundsPerSecond = rpm/60*2π/π`
- Ghost indicator (slip): opacity 0/1, rotation from `slipPosition`
- Indicator fade: `Transition { PropertyAnimation { property: opacity; duration: 150 } }` (Spinny.qml:148-159)
- Scratch: PointHandler, angular delta -> `scratch_position`
- Waveform scroll: `playposition` updateInterval 33ms drives overview marker

---

## 7. MIXER (BoothMixer.qml)

**Geometry**
- `unit = max(9, min(h/14, w/11))`
- Two Channel strips (Channel1, Channel2) with divider
- Each channel: TRIM, HIGH, MID, LOW, FILTER knobs (BoothKnob), CUE button, LevelMeter, Fader
- Knobs: Repeater over model, `Layout.fillHeight/Width`, FILTER opacity `quickEffectEnabled?1:0.4`
- CUE button: height `unit*1.8`, border `pfl?warningColor:midGray2`, bg `pfl?#3a1a08:darkGray4`, tap toggles
- LevelMeter: width `unit*0.45`, updateInterval 33ms, peak `warningColor`, >0.8 `yellow`, else `green`, height `vu*h`
- Fader: `BoothFader`, color `deckColor`, vertical

**Live**
- VU meters: `vu_meter` and `peak_indicator` updateInterval 33ms
- Peak flashes `warningColor` for one frame

---

## 8. BROWSE VIEW (BoothBrowse.qml)

**Geometry**
- Header: height `compact?30:38`, bg `darkGray3`
  - Back button: width `h*1.3`, enabled when not streaming and (showTracks?levelIndex!=null:levelHasAll)
  - Path text: font `compact?12:14`, elide middle
  - Load target badge: bg `deck1/2 color`, text "LOAD TO DECK N"
  - Search field: TextField, bg `backgroundColor`, border `activeFocus?lightGray2:midGray2`, placeholder "SEARCH"
  - Streaming tab: border `streaming?accentColor:midGray3`, toggle
- Section column (sources): width `compact?64:78`, gradient rail bg
  - Button height: `min(compact?52:66, h/count)`
  - Active: bg `activeColor (#7c5cff)`, accent bar left 3px `accentColor`
  - Cursor (controller): border `lightGray2` 2px
  - Icon: SVG `ic_library_<name>.svg`, size `h*0.42`, ColorOverlay white/lightGray2
  - Label: font `compact?9:10`, sectionLabel (TRACK, PLAYLIST, CRATE, HISTORY, FOLDER, REKORDBOX, ENGINE DJ, REC)
- Level list (entries): visible `showLevel`, width to trackList, bg `darkGray4`
  - Header "ALL TRACKS" when `levelHasAll`: height `compact?40:46`, active `activeColor`
  - Rows: height `compact?40:46`, alt bg `#0c0d0f`/`#16181b`, cursor `activeColor`, label font `compact?15:17`, arrow `›` for children/drives
- TrackList: `LibraryComponent.TrackList`, columns from BoothSourceTree (cover 40px, TITLE fill 3, ARTIST fill 2, BPM 66, KEY 58, ADDED 96), visible `!showLevel`
- Info panel (right): visible `!compact && w>900`, width 210
  - Cover art square, title 15px bold, artist 13px dim, BPM/KEY/ADDED rows 22px

**Controller Integration** (LibraryComponent.Control:271-317)
- `onActivateSidebarItem`, `onCollapseSidebarItem`, `onFocusWidgetChanged`, `onLoadSelectedTrack`, `onMoveSidebar`, `onMoveVertical`
- Sidebar focus switches between sidebar and track list

**Animations**
- Section column: no explicit animations, but `color` changes on active/cursor/pressed are instant
- ListView: `boundsBehavior: Flickable.StopAtBounds`, ScrollBar

---

## 9. BOOTHBEATFX (BoothBeatFx.qml)

**Geometry**
- Header: height `compact?20:26`, bg `midGray3`, text "BEAT FX" font `compact?11:13` color `lightGray2`
- Focused slot row (131-199): height `compact?30:40`
  - Prev/Next arrows: 24px, triangles 11x8, pressed bg `darkGray3`
  - Slot name: font `compact?14:17`, HorizontalFit, color `buttonActiveColor`
- Three slot boxes (203-239): height `compact?20:24`, border `focused?#d8dadd:midGray3`, bg `enabled?#123a5c:bg`, number bold when focused
- Mix slider (246-253): label "LEVEL/DEPTH N%", Slider 0-1, height `compact?22:32`
- Meta slider (257-264): label "EFFECT AMOUNT N%", Slider 0-1
- CH SELECT (269-288): three ToggleBox (1,2,MASTER), height `compact?24:30`, border `on?accent:midGray3`, bg `on?accent:bg`, text color `on?buttonActiveColor:lightGray2`
- ON/OFF button (295-313): height `compact?34:44`, border `enabled?#ff6a13:midGray3`, bg `enabled?warningColor:bg`, text "ON/OFF" bold `compact?13:15`

**Live**
- `focusedControl` (focused_effect), `mixControl`, `metaControl`, `enabledControl`, `nextEffectControl`, `prevEffectControl`
- Sliders update controls on `onMoved`

---

## 10. FX STATION (FxStation.qml)

**Geometry**
- `strip: h<560 && w>=900 && h>=220` (single row, no header)
- `compact: !strip && (h<560 || w<760)` (tabs)
- `tab` 0-3 (Scene, Isolate, X-Pad, Release)
- Bg `#0b0e13`
- FxButton component (51-108): radius 4, border `active?activeColor:#323a48` width `active?2:1`, bg `pressed?#2a3240 : active?darker(activeColor,2.6):#161b24`, opacity `enabled?1:0.4`, text fit `max(9,min(14,h*0.32))`
- Section component (111-158): title 12px bold letter-spacing 1, actions row right

**Sections**
- Scene (297+): 17 effects, param1/param2 labels per type, FX SOURCE buttons (INPUT, X-PAD), Scene buttons grid, param knobs (BoothKnob)
- Isolate: HI/MID/LO kill buttons, ISO MODE knob (0-2)
- X-Pad: 4x4 grid, instrument selector, division, overdub, clear, bank, loop steps, reload
- Release: 3 types, button grid
- BPM/tempo row: tap, nudge, auto, quantize, BPM display, beat indicator

**Live**
- `beatControl.updateInterval: 33` drives beat indicator
- `bpmControl.updateInterval: 250`
- `sourceControl.updateInterval: 250`
- Beat flash: Section title color changes on beat

---

## 11. SETTINGS / UTILITY (Settings.qml, +android/Settings.qml)

**Android Settings (+android/Settings.qml:10-525)**
- Popup, modal, padding 8, radius 8, border `#383c48`
- Header bar 40px: Back button, title centered, Close X (44x56, hit area +12 margin)
- Category list left (280px): search field, ListView categories
- Content right: ScrollView with Settings sections
- Categories: Interface, Audio, Library, Streaming, Controllers, Advanced, About
- Each setting: label + control (toggle, slider, combobox, button)

**Desktop Settings (Settings.qml:10-311)**
- Similar but wider, no back button in header
- Background opacity animation: `Behavior on opacity { NumberAnimation { duration: 250 } }` (StemsOverlay.qml:97-101 for toast)

**Animations**
- Stems toast: `Behavior on opacity { NumberAnimation { duration: 250 } }`
- Settings background opacity: `opacity: parent.radius < 0 ? max(0.1, 1 + parent.radius/8) : 1` (animated via radius)

---

## 12. STEMS OVERLAY (StemsOverlay.qml)

**Stems Queue Popup**
- Modal, centered, width `min(640, w-20)`, max height `h-20`
- Bg `#14171f`, border `#2a3648`, radius 10
- Header: "Stems queue" + CLOSE button
- `Skin.StemsQueue` component

**Stems Toast**
- Bottom center, margin 60, z:95
- Width `min(560, w-40)`, radius 8
- Success: bg `#1e3a2a`, border `#66bb6a`; Error: bg `#4a1e1e`, border `#ef5350`
- `Behavior on opacity { NumberAnimation { duration: 250 } }`
- Auto-dismiss Timer (stemsToastTimer)

---

## 13. XY PAD (XYPad.qml)

**Geometry**
- Radius 12, border `accentColor 40%`, bg `#e60a0d14`
- Header row: title, spacer, Deck1/Deck2 selectors (28px, radius 4), Hold toggle, SNAP/GRID buttons, Close
- Surface: `surfaceColor #080b10`, border `surfaceBorderColor`, radius 8
- Control circle: radius `controlRadius(4)`, follows touch, color `accentColor`
- Deck routing buttons (bottom): 1, 2, MASTER toggles
- Hold mode: locks position, color `holdColor`

**Live**
- `filterProxy` (super1), `fxMixProxy` (mix) updated on drag
- `currentX`, `currentY` clamped 0-1
- Hold mode saves/restores state on deck change

---

## 14. POPUPS & MENUS

**BeatgridPopup** (DeckComponent.BeatgridPopup): used in BoothDeckPanel (BPM tap) and BoothPerformanceDeck (GRID button)

**HotcuePopup** (Skin.HotcuePopup): long-press on hotcue pad, shows color picker, delete

**StorageAccessPopup** (BoothBrowse.qml:856-917): modal, permission request for USB/folders

**Track Menu** (Library/TrackMenu.qml): right-click/long-press on track

**Context Menus** (Library/Control.qml): Load to Deck/Sampler, Add to Playlist/Crate

**File/Edit/View Menus** (MainMenuBar.qml): native macOS or Qt menu bar

---

## 15. COLORS (Theme/+android/Theme.qml)

| Role | Hex | Notes |
|------|-----|-------|
| accentColor | #ff4fa8 | Hot pink, brand |
| activeColor | #7c5cff | Electric violet, pressed/active |
| backgroundColor | #000000 | True black |
| darkGray | #000000 | |
| darkGray2 | #0a0b10 | Indigo ink |
| darkGray3 | #14161c | Indigo surface |
| darkGray4 | #0d0e13 | Indigo deep |
| textColor | #e8e8e8 | |
| lightGray | #888888 | |
| lightGray2 | #bbbbbb | |
| lightGray3 | #999999 | Captions, inactive |
| midGray | #555555 | |
| midGray2 | #444444 | |
| midGray3 | #333333 | Borders |
| warningColor | #ff6b6b | Warm coral |
| green | #00e676 | |
| yellow | #ffd54f | Amber |
| blue | #ff4fa8 | Alias to accent |
| libraryDeck1Background | #1f6fd1 | Deck 1 cyan-blue |
| libraryDeck1Pressed | #4aa3f5 | |
| libraryDeck2Background | #e8650f | Deck 2 orange |
| libraryDeck2Pressed | #ff9a52 | |
| libraryScrollHandle | #333333 | |
| libraryScrollHandleHovered | #7c5cff | |
| libraryScrollHandlePressed | #ff4fa8 | |
| libraryActionBackground | #0a0b10 | |
| libraryActionDisabledBackground | #0d0e13 | |
| libraryActionText | #eeeeee | |
| libraryActionDisabledText | #666666 | |
| libraryConfirmBackground | #ff4fa8 | |
| libraryKeyMatchColor | #00e676 | |
| libraryKeyMatchBackground | #00e67633 | |

---

## 16. FONTS & SIZING

- Base font: `Theme.fontFamily` = "Open Sans" (QML), Inter.ttf (Rust)
- Tab font: bold, `width<760?10:compact?11:13`
- DeckPanel unit: `max(9, min(h/11, w/30))`
  - Caption: `unit*0.95`
  - Time digits: `unit*3.4` / `unit*1.7`
  - Tempo: `unit*1.45` / `unit*0.95`
  - BPM: `unit*2.6` / `unit*1.5`
- BoothKnob: label `max(8, min(13, w*0.22))`, ring `max(12, min(w, h-labelH))`, line `max(2, w/11)`, pointer `max(1.5, w/18)`
- BoothPerformanceDeck unit: `max(9, min(h/14, w/34))`
  - Pad label: `unit*1.05`
  - Loop size: `unit*1.8` / `unit*0.8`
  - Transport buttons: `unit*2.2` / `unit*4.2`
  - Jog size: `min(w,h)`, indicator `size*0.12`, BPM `size*0.17`
- Mixer unit: `max(9, min(h/14, w/11))`
- Fader handle: `max(14, along*0.6)`, cap `0.55*handle`
- BeatFx: labels `compact?9:10` / `compact?11:13` / `compact?13:15`
- Settings: category 15px bold, setting labels 13px

---

## 17. ANIMATIONS & LIVE ELEMENTS (EXHAUSTIVE)

### 17.1 Explicit Animations (Behavior/Animation)

| File:Line | Target | Property | Animation |
|-----------|--------|----------|-----------|
| BoothDeckPanel:173 | YouTube progress bar | width | NumberAnimation 140ms OutCubic |
| BoothDeckPanel:255 | Download gradient bar | width | NumberAnimation 140ms OutCubic |
| Spinny:148-159 | Indicator container | opacity | PropertyAnimation 150ms (fade in/out) |
| StemsOverlay:97-101 | Toast | opacity | NumberAnimation 250ms |
| YouTubeView:579-580 | Indicator | x | NumberAnimation |
| YouTubeView:828-829 | Panel | width | NumberAnimation 140ms OutCubic |
| YouTubeView:1048-1049 | Panel | width | NumberAnimation 140ms OutCubic |
| SubsonicView:195-196 | Layout.prefHeight | height | NumberAnimation 200ms |
| TrackList:309 | Row | opacity | NumberAnimation 100ms Linear |
| TrackDrag:186-187 | Drag item | scale | NumberAnimation |
| +android/TrackList:362-363 | Column | width | NumberAnimation 140ms OutCubic |
| +android/TrackList:488-489 | Column | width | NumberAnimation 140ms OutCubic |
| +android/TrackList:551-552 | Column | width | NumberAnimation 140ms OutCubic |
| MainWindow:106 | Column content | x,y | NumberAnimation 150ms (move transition) |
| MainWindow:404-405 | Splitter | height | SpringAnimation |
| MainWindow:443-444 | Splitter | height | SpringAnimation |
| MainWindow:527-528 | Panel | height | SpringAnimation |
| MainWindow:565-566 | Panel | height | SpringAnimation |
| MainWindow:608-609 | Panel | height | SpringAnimation |
| Deck:707-708 | Item | x | SpringAnimation |
| Deck:722-723 | Item | y | SpringAnimation |
| Deck:793-794 | Item | x | SpringAnimation |
| Deck:808-809 | Item | y | SpringAnimation |
| Deck:447/463/852 | Overlay | visible | FadeBehavior |

### 17.2 Live/Continuous Elements

| Element | Update Rate | Source |
|---------|-------------|--------|
| WaveformDisplay play position | 33ms | `playposition` ControlProxy |
| WaveformOverview play position | 33ms | `playposition` ControlProxy |
| WaveformOverview analyzer progress | - | `analyzerProgress` |
| Spinny (platter) | Every frame | `spinnyPosition.playPosition` * rpm |
| Ghost spinny (slip) | Every frame | `spinnyPosition.slipPosition` |
| VU Meters (BoothMixer) | 33ms | `vu_meter`, `peak_indicator` |
| BeatFx beat indicator | 33ms | `beatControl` (FxStation) |
| BPM display (DeckPanel, PerfDeck) | 100ms | `bpmControl` |
| Time display (DeckPanel) | 100ms | `positionControl` |
| Tempo fader position | Continuous | `rate` ControlProxy |
| Crossfader position | Continuous | `crossfader` ControlProxy |
| Knob values (EQ, trim, filter) | Continuous | ControlProxy parameters |
| FX Station BPM | 250ms | `bpmControl` |
| FX Station source levels | 250ms | `sourceControl` |
| FX Station beat flash | 33ms | `beatControl` |
| Library search | On text change | `TextField.onTextChanged` |
| Stems progress | - | `Mixxx.Stems.progress` |
| YouTube download progress | - | `Mixxx.YouTube.deckDownloadProgress` |

### 17.3 Pressed/Active States (Visual Feedback)

| Component | Pressed State | Active State |
|-----------|---------------|--------------|
| Tab | bg `darkGray3` | border `accentColor` 2px, text `accentColor` |
| BoothKnob | - | Ring draws from center/start, pointer at value |
| BoothFader handle | border `buttonActiveColor` | - |
| Pad (PerfDeck) | border `buttonActiveColor`, bg `midGray3` | border `litColor` 2px if playing, 1px if lit |
| SamplerPad | border `buttonActiveColor`, bg `midGray3` | border `warningColor` 2px, bg `alpha(warning,0.3)` |
| SmallButton (PerfDeck) | bg `midGray3` | bg `fade(colour,0x2e)`, border `colour`, text `colour` |
| FxButton | bg `#2a3240` | border `activeColor` 2px, bg `darker(activeColor,2.6)` |
| ToggleBox (BeatFx) | - | border `onColor`, bg `onColor`, text `buttonActiveColor` |
| CUE button (PerfDeck) | bg `#3a2408` | border `#ff9a1f`, text `#ff9a1f` |
| PLAY button (PerfDeck) | bg `#0c2a0c` | border `green`, shows pause bars |
| LevelMeter peak | - | Flash `warningColor` for 1 frame |

---

## 18. LAYOUT FORMULAS (SUMMARY)

**MainWindow**
- Top bar: `h = compact?26:34`
- Deck panels: `h = view!=1 ? compact?58:84 : round(H*(compact?0.36:0.3))`
- DJ view: `h = round(H*(compact?0.5:0.46))`
- Middle (waveforms): fills between topBar and deckPanels
- BeatFx panel: `w = 180`, phone overlay `w = 190`
- FX Station: docked `h = middle.h`, covering `h = H - topMargin(compact?32:40)`

**BoothDeckPanel**
- `unit = max(9, min(h/11, w/30))`
- Title bar: `h = unit*2.3`
- Info: `h = unit*4.4`
- Overview: `h = max(36, round(h*0.28))`

**BoothPerformanceDeck**
- `unit = max(9, min(h/14, w/34))`
- Tempo fader: `w = unit*2.6`
- Transport buttons: `h = unit*2.2`, big `unit*4.2`
- Jog: `size = min(w,h)`
- Pads grid: 2x4, cell `pw=(w-3)/2`, `ph=(h-modeH-9)/4`

**BoothMixer**
- `unit = max(9, min(h/14, w/11))`
- Mixer width: `min(w*0.24, h*0.9)`
- Level meter: `w = unit*0.45`

**BoothBrowse**
- Header: `h = compact?30:38`
- Section col: `w = compact?64:78`, btn `h = min(compact?52:66, H/count)`
- Level/Track rows: `h = compact?40:46`
- Info panel: `w = 210` (tablet only)

**BoothBeatFx**
- Header: `h = compact?20:26`
- Focused slot: `h = compact?30:40`
- Slot boxes: `h = compact?20:24`
- Sliders: `h = compact?22:32`
- CH SELECT: `h = compact?24:30`
- ON/OFF: `h = compact?34:44`

---

## 19. CONTROLLER INTEGRATION (MainWindow.qml:841-918)

- `[Skin]mobile_library_toggle` -> `toggleLibraryView()`
- `[Skin]mobile_fx_toggle` -> `toggleBeatFx()`
- `[Skin]mobile_loader_deck1/2` -> `openQuickLoad("[ChannelN]")`
- `[ChannelN]LoadSelectedTrack` -> opens browser if not visible

---

## 20. RUST APP CURRENT STATE (djsugar-next)

### Views (state.rs:96-104)
```rust
enum View { Ctl, Dj, Lib, Wave, Fx, Stream }
```
- No BROWSE separate from LIB
- No XY view
- No SPLIT, REC, STEMS, BEAT FX tabs in toolbar

### Toolbar (sugar.rs:147-243)
- Single row (or triple on tall+narrow)
- Tabs: DJ, WAVE, LIB, FX, STREAM, CTL (6 tabs vs 10)
- Right items: LOAD 1, LOAD 2, DECK FX, SMPL, AUTO, REC (timer)
- Search field at right
- No beatFxShown, no beatFxPanelShown, no fxStationLoader

### DJ View (booth.rs:112-133)
- Stacks on portrait (`w < h*1.6`)
- Landscape: two decks + mixer (width `min(w*0.24, h*0.9)`)
- Bottom bar: AUTO DJ, crossfader, REC
- No headphones knobs, no MASTER knob, no MIXING/LEVEL labels

### Decks (booth.rs:155-188)
- 5 columns: pads, loops, transport, jog, tempo
- Width formulas differ from QML
- No stem/sampler pad modes (only Cues, Loops, Samples, Fx)
- No GRID, SLIP, Q, MT buttons in transport
- No BPM/rate display in jog center
- No quantize/master tempo buttons at jog bottom

### Pads (booth.rs:190-255)
- 2x4 grid, 4 modes (Cues, Loops, Samples, Fx)
- Hotcue: shows letter + time, color from cue_colour
- Loops: shows beat label, lit when active
- Samples: shows sample name, blue accent
- Fx: shows FX_PADS labels (1/4,1/2,3/4,1,2,OUT,BRK,REV), yellow
- Mode button cycles Cues->Loops->Samples->Fx (no stem, no sampler)

### Loops (booth.rs:257-308)
- LOOP label, size box, 1/2 x2, IN OUT, GRID, SLIP
- Size scales with `k = available/(unit*13)`
- No BEATS/EXIT sublabel change

### Transport (booth.rs:310-352)
- SYNC button (no MASTER hold)
- CUE circle (orange border)
- PLAY circle (green, shows pause bars or triangle)
- No Q, MT, SLIP, GRID buttons

### Jog (booth.rs:354-)
- Platter with border, inner ring
- TouchSpinny indicator (from engine)
- Center: BPM whole/decimal, rate%
- No Q/MT buttons at bottom

### Waveforms (sugar.rs:279-354)
- Two scrolling waveforms, beat lines, loop region, cue markers
- Center line glow (deck color)
- No overview waveform (BoothDeckPanel has overview)

### Library (sugar.rs: library function)
- Sidebar + track list, search
- No section column with gradient rail
- No info panel on right
- No controller browsing integration

### FX Station (sugar.rs: station function)
- Basic scene/iso/xpad/release tabs
- No beat indicator, no BPM auto/tap/nudge
- No param knobs per effect type

### Settings
- No Settings popup implementation found

### Animations (Rust)
- `State.tick()` called every frame (lib.rs:59)
- No explicit animation system (no Behavior, NumberAnimation, SpringAnimation)
- No fade transitions, no spring layouts
- VU meters: none
- Spinny: driven by engine `SpinnyPosition` but no ghost/slip indicator
- Waveform scroll: immediate, no smoothing
- Pressed states: instant color change, no animated press

### Colors (sugar.rs:7-28)
- Subset of QML theme, missing: libraryDeck1/2 pressed, libraryScrollHandle*, libraryAction*, libraryConfirmBackground, libraryKeyMatch*, warningColor differs (#7d3b3b vs #ff6b6b), midGray values differ

---

## 21. GAPS CHECKLIST (Ordered by Visual Impact)

### CRITICAL (Core Feel)

1. **No spring animations anywhere** — QML uses `SpringAnimation` on splitter heights, panel heights, item x/y (MainWindow:404,443,527,565,608; Deck:707,722,793,808). Rust has zero spring physics. Reference: MainWindow.qml:404-405, Deck.qml:707-709. Rust: sugar.rs has no animation system.

2. **No fade transitions** — QML uses `FadeBehavior on visible` (Deck:447,463,852; MainWindow:286,304,340,515). Rust: popups/overlays appear instantly. Reference: Deck.qml:447. Rust: no fade in booth.rs or sugar.rs.

3. **No spinning platters (TouchSpinny)** — QML Spinny.qml:124-136 rotates continuously from `spinnyPosition.playPosition`, ghost indicator for slip (89-108). Rust: booth.rs jog draws static platter, no rotation. Reference: Spinny.qml:124-136. Rust: booth.rs:354+ draws static circles.

4. **No VU meter decay/animation** — QML BoothMixer.qml:20-49 updates at 33ms, peak flashes warningColor. Rust: no VU meters anywhere. Reference: BoothMixer.qml:20-49. Rust: no mixer VU in booth.rs mixer().

5. **No waveform scrolling smoothing** — QML WaveformDisplay playposition updates at 33ms, overview marker moves. Rust: sugar.rs scrolling() draws immediate position, no interpolation. Reference: WaveformOverview.qml:53-59. Rust: sugar.rs:286-354.

6. **No beat indicator flashes** — QML FxStation beatControl (33ms) flashes section titles. Rust: station() has no beat flash. Reference: FxStation.qml:197-203. Rust: sugar.rs station().

7. **No pressed glow/animated press** — QML buttons (Tab, FxButton, SmallButton, Pad) all have pressed states with color changes. Rust: hit() registers press but paint uses `s.pressed(t)` for instant color only, no animation. Reference: BoothDeckPanel.qml:135-140 (Chip), BoothPerformanceDeck.qml:143-146 (Pad). Rust: booth.rs:203-218.

### HIGH (Major Features Missing)

8. **No BEAT FX panel/overlay** — QML has BoothBeatFx (docked tablet, overlay phone) with focused slot, mix/meta sliders, CH SELECT, ON/OFF. Rust: only toolbar "DECK FX" toggle shows effects row (sugar.rs:245-262), no BeatFx panel. Reference: BoothBeatFx.qml:1-315. Rust: sugar.rs:245-262.

9. **No FX Station** — QML FxStation.qml (1296 lines) with Scene/Isolate/X-Pad/Release, beat-sync, param knobs. Rust: station() stub only. Reference: FxStation.qml. Rust: sugar.rs:121-124 calls station().

10. **No XY Pad** — QML XYPad.qml (563 lines) with filter/mix surface, deck routing, hold/snap. Rust: View::Xy enum exists (state.rs:66) but no paint function. Reference: XYPad.qml. Rust: state.rs:66, sugar.rs no xy().

11. **No STEMS UI** — QML StemsOverlay.qml with queue popup + toast animations. Rust: State has stems fields but no UI. Reference: StemsOverlay.qml. Rust: state.rs:131-139 Station struct only.

12. **No SPLIT (headphone splitter) toggle** — QML Top bar SPLIT tab controls `[Master]headSplit`. Rust: no headSplit control. Reference: MainWindow.qml:564-568. Rust: toolbar items lack SPLIT.

13. **No REC with timer** — QML MobileRecordButton shows timer when recording. Rust: REC button shows "REC ON" but no timer. Reference: MainWindow.qml:571-578. Rust: sugar.rs:186-190.

14. **No DECK INFO column (key, barsLeft, quantize, slip)** — QML DeckInfo component shows these beside waveforms. Rust: waveforms() draws only scrolling waveforms. Reference: MainWindow.qml:306-478. Rust: sugar.rs:279-283.

15. **No BROWSE view separate from LIB** — QML has viewMode=2 (BoothBrowse) with section column, level navigation, info panel, controller browsing. Rust: View::Lib only. Reference: BoothBrowse.qml. Rust: state.rs:100 View::Lib.

16. **No LOAD 1/2 deck-targeted browsing** — QML loadTarget property, LOAD tabs open browser for specific deck. Rust: LOAD 1/2 buttons set browser target but no dedicated browse view. Reference: MainWindow.qml:139-147, 544-555. Rust: sugar.rs:191-193.

17. **No controller browsing integration** — QML LibraryComponent.Control handles sidebar focus, knob navigation, load buttons. Rust: no controller library control. Reference: BoothBrowse.qml:271-317. Rust: library.rs has no controller handling.

18. **No gradient rail on section column** — QML BoothBrowse.qml:534-542 indigo-violet gradient. Rust: sidebar is flat. Reference: BoothBrowse.qml:534-542. Rust: library.rs.

19. **No info panel on browse (cover, title, artist, BPM, KEY, ADDED)** — QML BoothBrowse.qml:744-839. Rust: library() no right panel. Reference: BoothBrowse.qml:744-839.

20. **No stem pad mode** — QML BoothPerformanceDeck padMode="stem" shows 4 stems x2 (MUTE/FX) with colors. Rust: Mode enum lacks Stem. Reference: BoothPerformanceDeck.qml:356-407, 441-458. Rust: state.rs:189-196 Mode enum.

21. **No sampler pad mode per deck** — QML BoothPerformanceDeck padMode="sampler" shows 8 SamplerPad per deck side. Rust: Mode::Samples shows global 16 samplers in toolbar row. Reference: BoothPerformanceDeck.qml:460-479. Rust: sugar.rs:264-277 samplers().

22. **No beatjump pad mode** — QML beatjump shows left/right triangles, 8 sizes. Rust: no beatjump. Reference: BoothPerformanceDeck.qml:174-180, 274-290.

23. **No beatloop pad mode** — QML beatloop shows 8 loop sizes, lit when active. Rust: Mode::Loops only shows fixed LOOP_BEATS. Reference: BoothPerformanceDeck.qml:179, 191.

24. **No GRID button on deck (beatgrid popup)** — QML BoothDeckPanel BPM tap + BoothPerformanceDeck GRID button open BeatgridPopup. Rust: Target::Grid exists but no button in transport. Reference: BoothDeckPanel.qml:463-471, BoothPerformanceDeck.qml:660-675. Rust: booth.rs:306.

25. **No SLIP button on deck** — QML BoothPerformanceDeck SLIP button. Rust: Target::Slip exists but no button. Reference: BoothPerformanceDeck.qml:676-682. Rust: booth.rs:307.

### MEDIUM (Polish & Parity)

26. **No Q (quantize) / MT (master tempo) buttons on jog** — QML BoothPerformanceDeck:860-879. Rust: missing.

27. **No headphones knobs (MIXING/LEVEL) in DJ view bottom bar** — QML BoothDjView:64-94. Rust: bottom() only has AUTO DJ, crossfader, REC.

28. **No MASTER knob in DJ view bottom bar** — QML BoothDjView:105-119. Rust: missing.

29. **No SYNC leader (MASTER) hold on SYNC button** — QML BoothDeckPanel Chip SYNC onPressAndHold, BoothPerformanceDeck SmallButton SYNC onPressAndHold. Rust: booth.rs:312 Target::Sync only toggles.

30. **No tempo fader center mark** — QML BoothFader centerMark:true for tempo. Rust: fader() supports centre but booth.rs:187 doesn't pass true for tempo.

31. **No YouTube download progress bars on deck title** — QML BoothDeckPanel:166-175, 244-258 animated bars. Rust: no YouTube integration.

32. **No track key display on deck title bar** — QML BoothDeckPanel keyLabel. Rust: minis() shows key but not on deck panel.

33. **No remaining/elapsed time tap toggle** — QML BoothDeckPanel time MouseArea toggles showElapsed. Rust: no time display on deck panel.

34. **No tempo range display** — QML BoothDeckPanel rangeText shows rateRange%. Rust: missing.

35. **No pitch value display** — QML BoothDeckPanel pitch text with ±. Rust: missing.

36. **No BPM MASTER badge** — QML DeckInfo/BoothDeckPanel show MASTER when sync_leader. Rust: no sync leader display.

37. **No loop IN/OUT armed states** — QML BoothPerformanceDeck IN/OUT buttons show armed. Rust: booth.rs:302-303 has armed but no visual distinction.

38. **No hotcue color letters on pads** — QML Pad component draws colored square with letter. Rust: booth.rs:220-227 draws letter but square only when lit.

39. **No hotcue time display on pad** — QML Pad shows time when hotcue set. Rust: booth.rs:228-236 shows time but only when lit and wide enough.

40. **No beatjump triangles on pads** — QML Pad beatjump mode shows triangles. Rust: missing.

41. **No stem colors (blue/accent/yellow/activeColor)** — QML StemPad uses 4 stem colors. Rust: no stem mode.

42. **No sampler pad play/cue hold repeat** — QML SamplerPad TapHandler onPressedChanged cues while held. Rust: samplers are buttons only.

43. **No mixer FILTER knob opacity when off** — QML BoothMixer FILTER opacity 0.4 when quickEffect off. Rust: mixer() draws all knobs same.

44. **No mixer CUE button with pfl state** — QML BoothMixer CUE button border/bg change on pfl. Rust: mixer() no CUE button.

45. **No level meter peak flash** — QML LevelMeter peakControl flashes warningColor. Rust: no VU meters.

46. **No FX Station beat-sync (tap, nudge, auto, quantize)** — QML FxStation has full tempo control row. Rust: station() missing.

47. **No FX Station parameter knobs per effect type** — QML SceneSection has param1/param2 knobs with labels per effect. Rust: missing.

48. **No XY Pad hold/snap modes** — QML XYPad has holdMode, SNAP/GRID buttons, state save/restore. Rust: missing.

49. **No storage access popup for USB drives** — QML BoothBrowse storageAccessPopup. Rust: missing.

50. **No native menu bar (File/Edit/View)** — QML MainMenuBar.qml. Rust: no menu bar.

### LOW (Details)

51. **Colors differ** — midGray values, warningColor, library* colors missing. Reference: Theme/+android/Theme.qml. Rust: sugar.rs:7-28.

52. **Fonts differ** — QML uses Open Sans, Rust uses Inter. Sizing formulas differ.

53. **No compact/normal breakpoints** — QML `compact = height < 520` used everywhere. Rust: `compact = r.h < 520.0` only in dj().

54. **No landscape/portrait specific layouts** — QML MainWindow adjusts top bar tabs, DJ view stacks. Rust: sugar.rs:73 `tall = h > w*1.1` but limited.

55. **No cutout insets handling** — QML Skin.CutoutInsets for camera notches. Rust: missing.

56. **No scrollbar styling** — QML ScrollBar custom colors (libraryScrollHandle*). Rust: no scrollbars.

57. **No tooltip/long-press context menus on all controls** — QML TapHandler pressAndHold on pads, chips, etc. Rust: only LONG_PRESS_MS constant.

58. **No keyboard shortcuts / accelerator keys** — QML MenuItem shortcuts. Rust: none.

59. **No high-DPI scale factor handling** — QML main.cpp QT_SCALE_FACTOR logic. Rust: scale passed to resize().

60. **No skin theme injection** — QML skinTheme property applied to shared components. Rust: hardcoded colors.

---

## 22. TOP 25 GAPS (by Visual Impact)

| # | Gap | Reference | Rust Location |
|---|-----|-----------|---------------|
| 1 | No spring animations (splitters, panels, items) | MainWindow.qml:404, Deck.qml:707 | sugar.rs, booth.rs - no animation system |
| 2 | No fade transitions (popups, overlays) | Deck.qml:447, MainWindow.qml:286 | All paint functions - instant show/hide |
| 3 | No spinning platters (TouchSpinny + ghost slip) | Spinny.qml:124-136, 89-108 | booth.rs:354+ - static circles only |
| 4 | No VU meters with peak flash | BoothMixer.qml:20-49 | booth.rs mixer() - no VU at all |
| 5 | No BEAT FX panel/overlay (docked + phone) | BoothBeatFx.qml:1-315 | sugar.rs:245-262 - only toolbar row |
| 6 | No FX Station (Scene/Isolate/X-Pad/Release) | FxStation.qml:1-1296 | sugar.rs:121-124 - stub only |
| 7 | No XY Pad (filter/mix surface) | XYPad.qml:1-563 | state.rs:66 View::Xy - no paint |
| 8 | No STEMS UI (queue + toast) | StemsOverlay.qml:1-139 | state.rs:131-139 - no UI |
| 9 | No waveform scrolling smoothing (33ms interpolated) | WaveformOverview.qml:53-59 | sugar.rs:286-354 - immediate |
| 10 | No beat indicator flashes (FX Station, decks) | FxStation.qml:197-203 | station() - no beat flash |
| 11 | No DECK INFO column (key, barsLeft, quantize, slip) | MainWindow.qml:306-478 | sugar.rs:279-283 - waveforms only |
| 12 | No BROWSE view (section column, levels, info panel) | BoothBrowse.qml:1-919 | state.rs:100 View::Lib only |
| 13 | No controller browsing integration | BoothBrowse.qml:271-317 | library.rs - no controller handling |
| 14 | No stem/sampler/beatjump/beatloop pad modes | BoothPerformanceDeck.qml:169-479 | state.rs:189-196 Mode enum incomplete |
| 15 | No GRID/SLIP/Q/MT buttons on deck transport | BoothPerformanceDeck.qml:660-682 | booth.rs transport() - missing |
| 16 | No SYNC leader (MASTER) hold | BoothDeckPanel.qml:288-293 | booth.rs:312 - toggle only |
| 17 | No headphones/MASTER knobs in DJ bottom bar | BoothDjView.qml:64-119 | booth.rs:135-153 - missing |
| 18 | No remaining/elapsed time toggle on deck | BoothDeckPanel.qml:298-356 | booth.rs - no time display |
| 19 | No tempo range/pitch display on deck | BoothDeckPanel.qml:360-400 | booth.rs - missing |
| 20 | No BPM MASTER badge on deck | BoothDeckPanel.qml:403-442 | minis() - no leader display |
| 21 | No gradient rail on library sections | BoothBrowse.qml:534-542 | library.rs - flat bg |
| 22 | No info panel on browse (cover, metadata) | BoothBrowse.qml:744-839 | library() - no right panel |
| 23 | No LOAD 1/2 deck-targeted browse tabs | MainWindow.qml:139-147, 544-555 | sugar.rs:191-193 - basic only |
| 24 | No SPLIT/REC timer/STEMS tabs in toolbar | MainWindow.qml:564-603 | sugar.rs:191-198 - subset only |
| 25 | No pressed glow/animated press on any control | All QML buttons/pads | booth.rs/sugar.rs - instant color only |

---

*Generated from exhaustive analysis of `/home/ubuntu/djsugar/res/qml/Skins/Booth/*.qml`, shared QML under `/home/ubuntu/djsugar/res/qml/`, and Rust app `/home/ubuntu/djsugar-next/crates/app/src/`.*
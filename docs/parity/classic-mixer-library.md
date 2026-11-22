# DJ Sugar Android 'Classic' - MIXER & LIBRARY Parity Spec

Generated from `/home/ubuntu/djsugar/res/qml/+android/` and shared `/home/ubuntu/djsugar/res/qml/` sources.
All Theme.xxx references resolved to hex from `Theme.qml`.

---

## 1. Color Palette (Theme.qml)

| Token | Hex | Used By |
|-------|-----|---------|
| `accentColor` | `#3a60be` | Button active, slider bars, focus indicators |
| `backgroundColor` | `#1e1e20` | Root backgrounds |
| `blue` | `#01dcfc` | BPM slider, crossfader bar, volume slider bar |
| `buttonActiveColor` | `#D9D9D9` (white) | Active button text |
| `buttonNormalColor` | `#696969` (midGray) | Inactive button text |
| `crossfaderBarColor` | `#01dcfc` (blue) | Crossfader track |
| `crossfaderOrientationColor` | `#747474` (lightGray) | Crossfader deck buttons |
| `darkGray` | `#0f0f0f` | Deck background, sidebar rows |
| `darkGray2` | `#2e2e2e` | Toolbar, section backgrounds |
| `darkGray3` | `#3F3F3F` | Borders, disabled elements |
| `darkGray4` | `#202020` | ComboBox background, knob frames |
| `deckActiveColor` | `#D9D9D9` (white) | Active deck button |
| `deckBackgroundColor` | `#0f0f0f` (darkGray) | Deck area |
| `deckTextColor` | `#b0b0b0` (lightGray2) | Primary text |
| `eqFxColor` | `#ea2a4e` (red) | Quick FX knob arc |
| `eqHighColor` | `#D9D9D9` (white) | EQ High knob |
| `eqMidColor` | `#D9D9D9` (white) | EQ Mid knob |
| `eqLowColor` | `#D9D9D9` (white) | EQ Low knob |
| `fontFamily` | `Open Sans` | All text |
| `gainKnobColor` | `#01dcfc` (blue) | Gain knob arc |
| `green` | `#85c85b` | VU meter green segment |
| `knobBackgroundColor` | `#262626` | Knob frame rect |
| `libraryPanelSplitterBackground` | `#1e1e1e` | SplitView handle track |
| `libraryPanelSplitterHandle` | `#5f5f5f` | SplitView handle idle |
| `libraryPanelSplitterHandleActive` | `#7a7a7a` | SplitView handle hover/press |
| `lightGray` | `#747474` | Waveform beat, crossfader orientation |
| `lightGray2` | `#b0b0b0` | Deck text, panel splitter active |
| `lightGray3` | `#939393` | Disabled button text |
| `midGray` | `#696969` | Button normal, borders |
| `midGray2` | `#676767` | Sidebar top-level row bg |
| `midGray3` | `#626262` | Beatjump/loop backgrounds |
| `panelSplitterBackground` | `#1e1e20` (backgroundColor) | Generic splitter |
| `panelSplitterHandle` | `#696969` (midGray) | Generic splitter handle |
| `panelSplitterHandleActive` | `#b0b0b0` (lightGray2) | Generic splitter handle active |
| `pflActiveButtonColor` | `#01dcfc` (blue) | PFL button active |
| `red` | `#ea2a4e` | Effect color, VU meter red segment |
| `sunkenBackgroundColor` | `#0C0C0C` | Track list row alt, quick load bar |
| `textColor` | `#b0b0b0` (lightGray2) | Header text |
| `toolbarActiveColor` | `#D9D9D9` (white) | Toolbar active |
| `toolbarBackgroundColor` | `#2e2e2e` (darkGray2) | Toolbar bg |
| `volumeSliderBarColor` | `#01dcfc` (blue) | Volume fader track |
| `warningColor` | `#7D3B3B` | Warnings |
| `waveformBeatColor` | `#747474` (lightGray) | Waveform beats |
| `waveformCursorColor` | `#D9D9D9` (white) | Waveform cursor |
| `waveformMarkerDefault` | `#ff7a01` | Waveform markers |
| `waveformMarkerIntroOutroColor` | `#2c5c9a` | Waveform intro/outro |
| `waveformMarkerLoopColor` | `#00b400` | Waveform loop |
| `waveformPostrollColor` | `#696969` (midGray) | Waveform postroll |
| `waveformPrerollColor` | `#696969` (midGray) | Waveform preroll |
| `white` | `#D9D9D9` | White accents |
| `yellow` | `#fca001` | VU meter yellow segment |
| `librarySourceBarBackground` | (not in Theme, see Library.qml) | Tab bar bg |
| `librarySourceBarBorder` | (not in Theme) | Tab bar border |
| `libraryLocalTabColor` | (not in Theme) | LOCAL MUSIC active text |
| `libraryLocalTabBackground` | (not in Theme) | LOCAL MUSIC active bg |
| `libraryStreamingTabColor` | (not in Theme) | STREAMING HUB active text |
| `libraryStreamingTabBackground` | (not in Theme) | STREAMING HUB active bg |
| `librarySourceTabBackground` | (not in Theme) | Inactive tab bg |
| `librarySourceTabText` | (not in Theme) | Inactive tab text |
| `libraryActionText` | (not in Theme) | Action button text |
| `libraryActionBackground` | (not in Theme) | Action button bg |
| `libraryActionDisabledText` | (not in Theme) | Disabled action text |
| `libraryActionDisabledBackground` | (not in Theme) | Disabled action bg |
| `libraryConfirmBackground` | (not in Theme) | Confirm button bg |
| `libraryScrollHandle` | `#546e7a` | Scrollbar handle |
| `libraryScrollHandleHovered` | `#78909c` | Scrollbar handle hover |
| `libraryScrollHandlePressed` | `#00e5ff` | Scrollbar handle press |
| `libraryScrollTrack` | `#181a20` | Scrollbar track |

---

## 2. MIXER (`+android/Mixer.qml`)

### 2.1 Root Geometry
- **File**: `+android/Mixer.qml:7-16`
- `implicitWidth: 190` (fixed)
- `implicitHeight: content.height + crossfader.height`
- `width: implicitWidth`, `height: implicitHeight`
- Background: `Skin.SectionBackground` (border-image from `Theme.imgSectionBackground`, 10px border, stretched)

### 2.2 Content Area (`content` Item)
- **File**: `+android/Mixer.qml:24-185`
- `height: (show4decks ? eqDeck1.height * 2 : eqDeck1.height) + 10`
- `width: root.implicitWidth` (190)
- Internal margins: `anchors.margins: 5` (5px all sides)
- Two states: `2decks` (default) and `4decks` (when `show4decks=true`)

#### 2.2.1 EQ Columns (`Skin.EqColumn` from `+android/EqColumn.qml`)
- **Width**: 42px each
- **Height**: `eqDeck1.height` (sum of 3 EqKnobs + QuickFxKnob + ComboBox = 4x42 + 26 + spacing)
- **Spacing**: 4px (Column.spacing in EqColumn.qml:13)
- **2-deck layout**:
  - `eqDeck1` (group[0]): left=0, rightMargin=4, top=0
  - `eqDeck2` (group[1]): right=0, rightMargin=0, top=0
- **4-deck layout** (State `2decks`):
  - `eqDeck3` (group[2]): left=0, rightMargin=4, top=0, visible
  - `eqDeck1` (group[0]): left=4 (leftMargin), rightMargin=4, top=0
  - `eqDeck4` (group[3]): right=0, leftMargin=4, top=0, visible
  - `eqDeck2` (group[1]): right=4 (rightMargin), top=0
- Each EqColumn contains (top to bottom):
  1. `EqKnob` High - color `Theme.eqHighColor` (#D9D9D9), group `[EqualizerRack1_<deck>_Effect1]`, key `parameter3`, statusKey `button_parameter3`
  2. `EqKnob` Mid - color `Theme.eqMidColor` (#D9D9D9), key `parameter2`, statusKey `button_parameter2`
  3. `EqKnob` Low - color `Theme.eqLowColor` (#D9D9D9), key `parameter1`, statusKey `button_parameter1`
  4. `QuickFxKnob` - color `Theme.eqFxColor` (#ea2a4e), group `[QuickEffectRack1_<deck>]`, arcStyle `ShapePath.DashLine`, pattern `[2,2]`
  5. `ComboBox` (effectSelector) - height 26, font.pixelSize 11, model `Mixxx.EffectsManager.quickChainPresetModel`, popupMaxItem 8, popupWidth 160, width=parent.width, indicator.width=0, onActivated sets `fxSelect.value`

#### 2.2.2 Mixer Columns (`Skin.MixerColumn` from `MixerColumn.qml`)
- **Width**: 42px
- **Height**: `eqDeck2.height` (matches EQ column height)
- **2-deck layout**:
  - `mixerDeck1` (group[0]): left=eqDeck1.right + 4, top=0
  - `mixerDeck2` (group[1]): right=eqDeck2.left - 4, top=0
- **4-deck layout** (State `2decks`):
  - `mixerDeck3` (group[2]): left=0, rightMargin=4, top=eqDeck3.bottom
  - `mixerDeck1` (group[0]): left=mixerDeck3.right + 4, rightMargin=2, top=0
  - `mixerDeck4` (group[3]): right=0, leftMargin=4, top=eqDeck4.bottom
  - `mixerDeck2` (group[1]): leftMargin=2, right=eqDeck2.left - 4, top=0
- Each MixerColumn contains (top to bottom):
  1. **Gain Knob Frame** (Rectangle): color `Theme.knobBackgroundColor` (#262626), width=height, radius 5, anchors left/right/top to parent
     - `ControlKnob` inside: centered horizontally, top=parent.top, size 36x36, color `Theme.gainKnobColor` (#01dcfc), key `pregain`
  2. **VU Meters + Volume Fader** (Item): anchors bottom=pflButton.top -5, left/right/top=parent, topMargin=5 from gainKnobFrame.bottom
     - Left VU: x=15, width=4, height=parent.height-22, key `vu_meter_left`
     - Right VU: x=parent.width-width-15, width=4, same height, key `vu_meter_right`
     - Volume Fader (`ControlFader`): anchors.fill=parent, bar.color `Theme.volumeSliderBarColor` (#01dcfc), bg `Theme.imgVolumeSliderBackground`, key `volume`, handleImage.width=parent.width-4
  3. **PFL Button** (`ControlButton`): anchors bottom/left/right to parent, text `PFL`, toggleable, activeColor `Theme.pflActiveButtonColor` (#01dcfc), key `pfl`

### 2.3 Crossfader Area (`crossfader` Item)
- **File**: `+android/Mixer.qml:187-303`
- **Height**: 40px
- **Width**: 190px (root.implicitWidth)
- Background: `Skin.SectionBackground` (full), inner Item with margins=3 also has SectionBackground
- **Left Deck Assignment** (GridLayout): width = `show4decks ? 34 : 17`, columnSpacing=0, rowSpacing=0
  - Repeater model: `show4decks ? 4 : 2`
  - Layout: 4-deck = 2 columns x 2 rows; 2-deck = 1 column x 2 rows
  - Each cell: 17x17, SectionBackground, `ControlButton` (deckButton) margins=1, group `[ChannelN]`, key `orientation_left`, text=`N`, toggleable, activeColor `Theme.deckActiveColor` (#D9D9D9)
- **Crossfader Slider** (`ControlFader`): horizontal, group `[Master]`, key `crossfader`, bar.color `Theme.crossfaderBarColor` (#01dcfc), bar.start=0.5, bg `Theme.imgCrossfaderBackground`, fg `Theme.imgCrossfaderHandle`, handleImage.height=34, anchors left=leftDeckAssignment.right, right=rightDeckAssignment.left, verticalCenter=parent.verticalCenter
- **Right Deck Assignment** (GridLayout): mirrors left, key `orientation_right`, anchors.right=parent.right

---

## 3. EQ KNOB (`+android/EqKnob.qml` -> `EqKnob.qml`)

### 3.1 Structure
- **File**: `EqKnob.qml:6-37`
- Root: Rectangle 42x42, color `Theme.knobBackgroundColor` (#262626), radius 5
- Opacity: `statusControl.value ? 0.4 : 1` (dimmed when effect OFF)
- `ControlKnob` (knob): centered, 36x36
- Corner ON/OFF indicator: Rectangle 10x10 at top-right (margins=1), border white 1px, radius 5, color = `statusControl.value ? Theme.white : transparent`
- TapHandler on indicator (margin=8): toggles `statusControl.value`
- `ControlProxy` (statusControl): group from knob.statusGroup, key from statusKey (`button_parameterN`)

### 3.2 Knob Drawing (`Knob.qml` -> `ControlKnob.qml`)
- **Base**: `MixxxControls.Knob` (C++)
- `backgroundSource: Theme.imgKnob` (SVG), `shadowSource: Theme.imgKnobShadow`
- `angle: 116` (degrees from center-bottom)
- `arc: true`, `arcColor: root.color` (per-knob), `arcOffsetY: width*0.01`, `arcRadius: width*0.45`, `arcWidth: 2`
- Foreground indicator: Rectangle width=2, height=width/5, centered horizontally, y=height (bottom of knob), color=arcColor
- Shadow image: height=width*7/6, preserveAspectFit
- `ControlKnob.qml`: wraps Knob, binds `value` to `control.parameter`, `onTurned` writes back, `TapHandler` double-tap -> `control.reset()`

---

## 4. QUICK FX KNOB (`+android/QuickFxKnob.qml`)

- **File**: `+android/QuickFxKnob.qml:6-55`
- Rectangle 42x42, color `Theme.knobBackgroundColor` (#262626), radius 5
- Opacity: `statusControl.value ? 1 : 0.4` (inverted vs EqKnob: visible when ON)
- `ControlKnob` (knob): centered horizontally, top=root.top, 36x36, group from root, key `super1`
- Corner indicator: same 10x10 top-right, border white 1px, color = `statusControl.value ? Theme.white : transparent`
- TapHandler on indicator: toggles `statusControl.value` (enabled state)
- `ControlProxy` (statusControl): key `enabled`

---

## 5. EFFECT ROW (`+android/EffectRow.qml`)

- **File**: `+android/EffectRow.qml:6-65`
- Root Item, `implicitHeight`: columns.implicitHeight > 0 ? that : (show4units ? 170 : 85)
- `height: visible ? implicitHeight : 0`, `clip: true`
- Row with two Columns (50% width each):
  - Left column: EffectUnit 1 (always), EffectUnit 3 (visible when `show4units`)
  - Right column: EffectUnit 2 (always), EffectUnit 4 (visible when `show4units`)
- Two `SectionBackground` rectangles split vertically at horizontalCenter (z=-1)
- `ControlProxy` (showFourUnitsControl): group `[Skin]`, key `show_4effectunits`

---

## 6. LIBRARY (`+android/Library.qml`)

### 6.1 Root Properties
- `sidebar`: `librarySources.sidebar()`
- `librarySourceMode`: 0=Local, 1=Streaming Hub
- `acceptsController`: true unless covered by popup
- `showSidebar`: `Mixxx.MobileLibrary.sidebarVisible` (persisted)

### 6.2 Source Selector Tab Bar (`sourceSelectorBar`)
- **File**: `+android/Library.qml:100-157`
- Rectangle: height 26, anchors top/left/right, color `Theme.librarySourceBarBackground`, border `Theme.librarySourceBarBorder` width 1
- RowLayout (margins=2, spacing=4):
  1. **LOCAL MUSIC Button**: `Layout.preferredWidth: 90`, `fillHeight: true`, font.bold, pixelSize 10, text `LOCAL MUSIC`, activeColor `Theme.libraryLocalTabColor`, backgroundColor = mode 0 ? `Theme.libraryLocalTabBackground` : `Theme.librarySourceTabBackground`, normalColor = mode 0 ? `Theme.libraryLocalTabColor` : `Theme.librarySourceTabText`, onClicked -> `librarySourceMode=0`
  2. **STREAMING HUB Button**: `Layout.preferredWidth: 106`, same styling with streaming colors, onClicked -> `librarySourceMode=1`
  3. Spacer: `Layout.fillWidth: true`
  4. **SIDEBAR Button** (visible only in Local mode): `Layout.preferredWidth: 64`, fillHeight, font.bold, pixelSize 9, text `SIDEBAR`, activeColor `Theme.libraryActionText`, normalColor `Theme.librarySourceTabText`, highlight=`showSidebar`, onClicked -> toggle `Mixxx.MobileLibrary.sidebarVisible`

### 6.3 Content Container (`sourceContentContainer`)
- Anchors: top=sourceSelectorBar.bottom, bottom/left/right=parent

#### 6.3.1 Local Library Area (visible when mode=0)
- **SearchField** (TextField): anchors left/right/top to parent, placeholder `Search your local collection...`, onTextChanged -> `applySearch()`
- **SplitView** (`librarySplitView`): anchors bottom/left/right, top=searchField.bottom, orientation Horizontal
  - **Handle**: Rectangle 8x8, color `Theme.libraryPanelSplitterBackground` (#1e1e1e), containmentMask 8px wide
    - Handle dots: 3 circles, color = pressed/hovered ? `Theme.libraryPanelSplitterHandleActive` (#7a7a7a) : `Theme.libraryPanelSplitterHandle` (#5f5f5f), size = pressed/hovered ? 6 : 5, radius=size
  - **Sidebar SplitView** (`sideBarSplitView`): visible=`showSidebar`, orientation Vertical
    - maxWidth 400, minWidth = showSidebar ? 140 : 0, preferredWidth = showSidebar ? min(220, width*0.25) : 0
    - Handle: same style as horizontal
    - **Browser** (`LibraryComponent.Browser`): SplitView.fillHeight=true, minHeight 100, preferredHeight 500, model=`root.sidebar`, highlighted=`libraryControl.sidebarFocused`, onStorageAccessNeeded -> opens storageAccessPopup
  - **TrackList** (`LibraryComponent.TrackList`): SplitView.fillHeight=true, clip=true, model=`root.sidebar ? root.sidebar.tracklist : null`, sourceName=`browser.activeLabel`, sortNewestFirst=`browser.activeIsDrive`, highlighted=`!sidebarFocused && showSidebar`

#### 6.3.2 Streaming Hub Area (visible when mode=1)
- `StreamingComponent.StreamingHub`: anchors.fill=parent, acceptsController=`root.visible && root.acceptsController`

### 6.4 Storage Access Popup
- Popup centered, width=min(460, root.width-20), modal, padding=16
- Background: `Theme.darkGray2` (#2e2e2e), border `Theme.darkGray3` (#3F3F3F), radius 8
- Content: ColumnLayout spacing=12
  - Title: bold 15px, white, text depends on forFolders
  - Body: 13px, textColor, WordWrap, explains permission
  - Buttons Row (AlignRight, spacing=8):
    - NOT NOW: 110x44, default styling
    - GRANT PERMISSION: 150x44, activeColor `Theme.accentColor` (#3a60be), backgroundColor `Theme.libraryConfirmBackground`, normalColor `Theme.libraryActionText`, bold, onClicked -> requestFullStorageAccess()

---

## 7. LIBRARY COMPONENTS (Shared)

### 7.1 SourceTree (`Library/SourceTree.qml` + `Library/+android/SourceTree.qml`)
- **Columns** (defaultColumns): CoverArt (100px, autoHide 750), Title (fillSpan 3), Artist (fillSpan 2), Album (fillSpan 1, autoHide 690), Year (80px, autoHide 750), Bpm (60px), Key (70px), File Type (70px, autoHide 900), Bitrate (70px, autoHide 1200)
- Android adds Position column (46px) after CoverArt for playlists
- Delegate: `LibraryComponent.Cell` -> `LibraryComponent.Track` (drag + tap handlers)
- Sources: `LibraryAllTrackSource` (All...), plus playlists/crates/folders from MobileLibrary

### 7.2 Browser (`Library/Browser.qml` + `Library/+android/Browser.qml`)
- Root: Rectangle, color `Theme.backgroundColor` (#1e1e20), margins bottom=40, left=7, right=15, top=7, inner color `Theme.sunkenBackgroundColor` (#0C0C0C)
- TreeView with FocusScope delegates:
  - Row height: depth 0 = 42, else 35
  - Background: depth 0 = `Theme.midGray2` (#676767), else transparent
  - Selection highlight: `Theme.midGray` (#696969) 25px wide at left margin (10+15*depth)
  - Expand/collapse indicator: Triangle, rotation animated 100ms OutQuart (0<->90 deg)
  - Icons from model (Qt.UserRole+1), label from DisplayRole
  - Click: selects row, activates model, toggles expansion if hasChildren
  - Right-click: opens playlist menu
  - `highlighted` property: border 2px `Theme.accentColor` (#3a60be) when controller focuses sidebar
- Android additions: storage access check for rekordbox/enginedj/computer icons, playlist menu, copy popup, activeLabel/activeIsDrive tracking

### 7.3 TrackList (`Library/TrackList.qml` + `Library/+android/TrackList.qml`)
- Root: Rectangle, color `Theme.darkGray` (#0f0f0f)
- **HorizontalHeaderView**: anchors top/left/right, margins=5, syncView=tableView
  - Column delegate: Item with Text (font Medium, pixelSize 12, capitalize, color `Theme.textColor` (#b0b0b0), leftMargin 15), sort indicator Triangle (red, size buttonFontPixelSize*0.7, rotation +-90 deg), column resizer (1px `Theme.darkGray2`, drag threshold 2, on release updates preferredWidth)
  - Click header: toggles Asc/Desc if same column, else sets column Ascending
- **TableView**: anchors bottom=quickLoadBar.top, left/right margins=5, top=header.bottom, clip, vertical flick, StopAtBounds, reuseItems, keyNavigationEnabled=false, pointerNavigationEnabled=false
  - Column width provider: respects hidden, autoHideWidth, preferredWidth, fillSpan distribution
  - ScrollBar vertical: AlwaysOn, width 16, interactive, contentItem radius 6, colors: pressed `#00e5ff` / hovered `#78909c` / idle `#546e7a` (desktop) OR Theme.libraryScrollHandle* (android), track `#181a20` opacity 0.7 radius 4
  - Delegate: Loader (async) -> Cell component, implicitHeight 38, opacity animation 100ms Linear
  - SelectionModel: ItemSelectionModel with moveSelectionVertical (wraps modulo rowCount), selectRow (clamps, positions view), selectedTrackUrls()
  - TapHandler on view: double-tap -> loadSelectedTrackIntoNextAvailableDeck(false)
  - onWidthChanged: if updateColumnSize() changed -> forceLayout()
  - Component.onCompleted / onModelChanged: updateColumnSize()

#### 7.3.1 Cell (`Library/Cell.qml` + `Library/+android/Cell.qml`)
- Rectangle, anchors.fill=parent, Drag.dragType=None
- Color: selected ? `Theme.accentColor` (#3a60be) : (row%2==0 ? `Theme.sunkenBackgroundColor` (#0C0C0C) : `Theme.backgroundColor` (#1e1e20))
- Right border: 1px `Theme.darkGray2` (#2e2e2e)

#### 7.3.2 Track (`Library/Track.qml` + `Library/+android/Track.qml`)
- Item with `capabilities`, `drag` (TrackDrag), `tap` (TapHandler)
- TapHandler: acceptedButtons Left|Right, gesturePolicy DragThreshold, onTapped RightButton -> openContextMenu
- TrackDrag: analyze, labelProvider (Artist - Title), trackUrl=file_url
- DragHandler: enabled=false (touch drag via TrackDrag)

#### 7.3.3 Quick Load Bar (`quickLoadBar`)
- Rectangle: anchors bottom/left/right, height 48, color `Theme.sunkenBackgroundColor` (#0C0C0C), border top 1px `Theme.panelSplitterBackground`
- RowLayout (margins=5, spacing=8):
  - **Selected Song Box**: fillWidth, fillHeight, `Theme.darkGray3` (#3F3F3F), radius 4, clip
    - Inner RowLayout (margins=6): Text fillWidth, color = hasTrack ? white : lightGray3, bold if hasTrack, pixelSize 13, elideRight, text = filename or `Select a track to load`
    - Android adds YouTube download progress overlay (gradient bars, animated 140ms OutCubic)
  - **Deck 1 Button**: preferred 115x38, activeColor `#64b5f6`, bg = hasTrack ? `#1565c0` : `#262930`, normal = hasTrack ? white : `#78909c`, highlight=enabled=hasTrack, bold, text `? DECK 1`, onClick -> load to Channel1 or loadTarget
  - **Deck 2 Button**: preferred 115x38, activeColor `#ffb74d`, bg = hasTrack ? `#e65100` : `#262930`, normal = hasTrack ? white : `#78909c`, highlight=enabled=hasTrack, bold, text `DECK 2 ?`, onClick -> load to Channel2 or loadTarget
  - **Android extra buttons** (after Deck 2):
    - + PLAYLIST / + ALL: 110x38, activeColor accent, bg = hasTracks ? actionBg : disabledBg, normal = hasTracks ? actionText : disabledText, enabled=hasTracks, bold, onClick -> playlistPopup
    - ? COPY (rekordbox only): 100x38, shows progress or COPY, onClick -> copyPopup
    - ? STEMS: 100x38, shows ? if stems exist, onClick -> stemsPopup

---

## 8. SHARED CONTROLS

### 8.1 ComboBox (`+android/ComboBox.qml`)
- `clip` property, `footerItems`, `popupMaxItem` (default 6), `popupWidth` alias
- Background: Rectangle margins=4, color `Theme.darkGray4` (#202020), radius 10
- ContentItem: Text, color `Theme.deckTextColor` (#b0b0b0), elide=clip?None:Right, leftPadding=5, verticalAlign=VCenter
- ItemHeight = max(38, contentItem.height+16)
- Delegate (ItemDelegate): implicitHeight=itemHeight, width=ListView.view.width
  - Background: border highlighted? `#D85486` : transparent, width=1, color = down ? `#37474f` : (highlighted ? `#263238` : transparent), radius 4
  - ContentItem: Text, color highlighted? white : deckTextColor, elideRight, font.pixelSize=max(12, root.font.pixelSize), bold if highlighted, left/rightPadding=10, verticalAlign=VCenter
  - onClicked: set currentIndex, activated, close popup
- Popup: modal, dim=false, closePolicy=Escape|PressOutside, focus=true, width=root.width, height=min(itemHeight*min(popupMaxItem,count)+16+footerHeight, window.height*0.75)
  - x/y positioning: centers under combo, clamps to screen edges (8px margin)
  - background: `#181a20`, border `#37474f` width 1, radius 8
  - ListView: clip, highlightFollowsCurrentItem=false, boundsBehavior=StopAtBounds, pixelAligned, pressDelay=60, flickDeceleration=2500, maxFlickVelocity=6000
  - ScrollBar: policy=AlwaysOn/AsNeeded, width=8, contentItem color pressed `#D85486` else `#546e7a`, radius 4
  - Footer: repeats footerItems, each height=itemHeight, Text leftPadding=8, suffix Text rightPadding=8 pixelSize=16, MouseArea onPressed -> activateFooter(index), close

### 8.2 Button (`+android/Button.qml`)
- AbstractButton, implicitHeight=26, implicitWidth=max(52, label.implicitWidth+16)
- Properties: activeColor (default `Theme.buttonActiveColor`), normalColor (`Theme.buttonNormalColor`), pressedColor (default activeColor), backgroundColor (`Theme.darkGray4`), highlight (bool), arrowLeft/arrowRight (bool), arrowSize=round(label.font.pixelSize*0.7), arrowSpace=round(arrowSize*0.85+label.font.pixelSize*0.35)
- Background: Rectangle fill, color=backgroundColor, radius=10 (flat, no shadow)
- ContentItem:
  - Label: fill, color=normalColor, bold, AllUppercase, fontFamily=Theme.fontFamily, pixelSize=Theme.buttonFontPixelSize (10), hCenter, vCenter, left/rightPadding for arrows
  - Repeater for arrows: Triangle (Skin.Triangle), color=label.color, height=arrowSize, width=arrowSize*0.85, positioned relative to text contentStart
  - Image (icon) + ColorOverlay (color=normalColor)
- States:
  - **pressed**: backgroundColor = checked?accentColor:darkGray3 (#3F3F3F), label.color=pressedColor
  - **active** (highlight||checked && !pressed): backgroundColor=accentColor (#3a60be), label.color=activeColor
  - **inactive** (default): backgroundColor=backgroundColor, label.color=normalColor

### 8.3 ControlButton (`ControlButton.qml`)
- Wraps `Skin.Button` with `ControlProxyButtonBehavior`
- Required: group, key, toggleable (default false)
- highlight = controlBehavior.isActive
- onPressed -> pressPrimary(), onReleased -> releasePrimary()
- Behavior: handlePointerInput=false (controller-driven)

### 8.4 ControlFader (`ControlFader.qml` + `+android/ControlFader.qml`)
- **Desktop** (`ControlFader.qml`): extends `Skin.Fader` (C++), adds MouseArea fill:
  - `updateFromTouch(mouse)`: maps x/y to 0..1 parameter, respects vertical/horizontal
  - onPressed/onPositionChanged -> updateFromTouch, onDoubleClicked -> control.reset()
- **Android** (`+android/ControlFader.qml`): extends `Skin.Fader`, MouseArea with relative drag:
  - onPressed: records startParameter, startPosition (normalized touchPosition)
  - onPositionChanged: delta = currentPos - startPos, parameter = clamp(startParameter + delta, 0, 1)
  - onDoubleClicked -> control.reset()

### 8.5 SectionBackground (`SectionBackground.qml`)
- BorderImage: source `Theme.imgSectionBackground` (SVG), border 10px all sides, horizontal/verticalTileMode=Stretch

### 8.6 VuMeter (`VuMeter.qml`)
- Rectangle: required group, key, radius=width/2, color=black
- ControlProxy: updateInterval=33ms (frame-synced)
- **MeterMask** (Item, invisible): Rectangle bottom-anchored, height=parameter*(height-2), margins=1, radius=width/2, antialiasing=false
- **MeterGradient** (Rectangle, invisible): full size, Gradient (antialiasing=false):
  - 0.10: `Theme.red` (#ea2a4e)
  - 0.15: `Theme.yellow` (#fca001)
  - 0.25: `Theme.yellow` (#fca001)
  - 0.30: `Theme.green` (#85c85b)
  - 1.00: `Theme.green` (#85c85b)
- **OpacityMask**: source=meterGradient, maskSource=meterMask

---

## 9. ANIMATIONS & BEHAVIORS

| Element | Animation | Duration | Easing | Trigger |
|---------|-----------|----------|--------|---------|
| ComboBox popup ListView opacity (Loader) | NumberAnimation | 100ms | Linear | Loader.status==Ready |
| Sidebar expand/collapse indicator | NumberAnimation | 100ms | OutQuart | expandedChanged |
| QuickLoadBar YouTube progress bars | NumberAnimation | 140ms | OutCubic | width change |
| Android TrackList column resize | NumberAnimation | 140ms | OutCubic | width change |
| SplitView handle dots (size/color) | Immediate (state) | - | - | pressed/hovered |
| Button pressed/active/inactive | PropertyChanges (State) | Immediate | - | pressed/highlight/checked |
| Knob turning | C++ driven | - | - | Touch/drag |
| Fader drag | Immediate parameter update | - | - | MouseArea positionChanged |
| VU meter | Frame-synced (33ms) | - | - | ControlProxy updateInterval |
| EqKnob/QuickFxKnob opacity | Immediate (binding) | - | - | statusControl.value |

---

## 10. CONTROLLER INTEGRATION

### 10.1 Mixer
- Deck assignment buttons: `ControlButton` with `orientation_left`/`orientation_right` keys, toggleable
- Crossfader: `[Master]` crossfader
- EQ knobs: `[EqualizerRack1_<deck>_Effect1]` parameter1/2/3 with button_parameterN status
- Quick FX: `[QuickEffectRack1_<deck>]` super1 + enabled
- Gain: `[ChannelN]` pregain
- Volume: `[ChannelN]` volume
- VU meters: `[ChannelN]` vu_meter_left/right
- PFL: `[ChannelN]` pfl

### 10.2 Library
- `LibraryComponent.Control` (desktop) / `Library/+android/Control.qml` (android):
  - `FocusedWidgetControl`: WidgetKind.LibraryView or Sidebar
  - `[Library]` GoToItem -> load selected (or activate sidebar item)
  - `[Playlist]` LoadSelectedIntoFirstStopped -> load to next deck
  - `[Playlist]` SelectTrackKnob -> moveVertical(value)
  - `[Playlist]` SelectPrevTrack/SelectNextTrack -> moveVertical(+-1)
  - `[Library]` MoveVertical/MoveUp/MoveDown -> moveVertical(value)
  - `[App]` num_decks -> instantiates ControlLoadSelectedTrackHandler per deck
- Android `Control.qml` adds `enabled` guard, `sidebarFocused`/`libraryFocused` computed, `moveFocus(forward)`, `move(offset)`
- Browser: `moveSelection(offset)`, `activateCurrent()`, `collapseCurrent()`, `checkStorageAccess()`
- TrackList: `moveSelectionVertical(offset)` (batches via pendingMove + Qt.callLater), `loadSelectedTrack(group,play)`, `loadSelectedTrackIntoNextAvailableDeck(play)`
- Keys on TableView: Down/Up -> moveSelectionVertical, Enter/Return -> loadSelectedTrackIntoNextAvailableDeck

---

## 11. RESPONSIVE / CONDITIONAL LAYOUTS

| Condition | Effect |
|-----------|--------|
| `show4decks` (Mixer) | EqColumn/MixerColumn rearrange to 4-deck grid (2x2), crossfader deck assignment widens to 34px (2x2) |
| `show4units` (EffectRow) | Shows EffectUnit 3 & 4, height 170 vs 85 |
| `librarySourceMode` | Switches between LocalLibraryArea and StreamingHubArea |
| `showSidebar` (MobileLibrary.sidebarVisible) | Shows/hides sidebar SplitView, minWidth 0/140, preferredWidth 0/220 |
| `highlighted` (Browser/TrackList) | 2px accentColor border when controller focuses |
| `clip` (ComboBox) | Changes Text elide mode, popup positioning |
| `sortNewestFirst` (TrackList) | Default sort by DateAdded descending vs Position ascending |
| `isRekordboxPlaylist` (Android TrackList) | Shows COPY button |
| `ytProgress >= 0` (Android TrackList) | Shows download progress overlay on row and in quickLoadBar |

---

## 12. FONTS & TEXT

| Context | Family | Size | Weight | Case | Color |
|---------|--------|------|--------|------|-------|
| Button label | Open Sans | 10px | Bold | AllUppercase | normalColor/activeColor/pressedColor |
| ComboBox content | Open Sans | (inherited) | - | - | deckTextColor (#b0b0b0) |
| ComboBox delegate | Open Sans | max(12, font.pixelSize) | Bold if highlighted | - | highlighted?white:deckTextColor |
| Tab bar (LOCAL/STREAMING/SIDEBAR) | Open Sans | 10/10/9 | Bold | - | activeColor/normalColor |
| SearchField placeholder | Open Sans | (system) | - | - | - |
| TrackList header | Open Sans | 12px | Medium | Capitalize | textColor (#b0b0b0) |
| TrackList row (Cell) | Open Sans | (delegate) | - | - | selected?accentColor:- |
| QuickLoadBar song text | Open Sans | 13px | Bold if hasTrack | - | hasTrack?white:lightGray3 |
| QuickLoadBar deck buttons | Open Sans | (buttonFontPixelSize=10) | Bold | - | activeColor/normalColor |
| Sidebar TreeView | Open Sans | (system) | - | - | - |

---

## 13. ICONS & IMAGES

| Asset | Path (Theme) | Used By |
|-------|--------------|---------|
| `imgSectionBackground` | `images/section.svg` | SectionBackground (mixer, crossfader, library panes) |
| `imgKnob` | `images/knob.svg` | Knob background |
| `imgKnobShadow` | `images/knob_shadow.svg` | Knob drop shadow |
| `imgKnobMini` | `images/miniknob.svg` | MiniKnob background |
| `imgKnobMiniShadow` | `images/miniknob_shadow.svg` | MiniKnob shadow |
| `imgCrossfaderBackground` | `images/slider_crossfader.svg` | Crossfader track |
| `imgCrossfaderHandle` | `images/slider_handle_crossfader.svg` | Crossfader handle |
| `imgVolumeSliderBackground` | `images/slider_volume.svg` | Volume fader track |
| `imgSliderHandle` | `images/slider_handle.svg` | Generic fader handle |
| `imgBpmSliderBackground` | `images/slider_bpm.svg` | BPM slider |
| `imgMicDuckingSlider` | `images/slider_micducking.svg` | Mic ducking slider |
| `imgButton` | `images/button.svg` | Popup background (legacy) |
| `imgButtonPressed` | `images/button_pressed.svg` | Pressed button (legacy) |
| `imgPopupBackground` | = `imgButton` | Popup background |

---

## 14. FILE REFERENCE SUMMARY

| Spec Section | Source Files |
|--------------|--------------|
| Mixer root/layout | `+android/Mixer.qml` |
| EqColumn | `+android/EqColumn.qml`, `EqKnob.qml`, `Knob.qml`, `ControlKnob.qml` |
| QuickFxKnob | `+android/QuickFxKnob.qml` |
| MixerColumn | `MixerColumn.qml`, `ControlFader.qml`, `+android/ControlFader.qml`, `VuMeter.qml`, `ControlButton.qml` |
| Crossfader | `+android/Mixer.qml:187-303`, `ControlFader.qml` |
| EffectRow | `+android/EffectRow.qml` |
| Library root | `+android/Library.qml` |
| SourceTree | `Library/SourceTree.qml`, `Library/+android/SourceTree.qml` |
| Browser | `Library/Browser.qml`, `Library/+android/Browser.qml` |
| TrackList | `Library/TrackList.qml`, `Library/+android/TrackList.qml` |
| Cell/Track | `Library/Cell.qml`, `Library/+android/Cell.qml`, `Library/Track.qml`, `Library/+android/Track.qml` |
| ComboBox | `+android/ComboBox.qml` |
| Button | `+android/Button.qml` |
| ControlButton | `ControlButton.qml` |
| SectionBackground | `SectionBackground.qml` |
| Theme colors | `Theme/Theme.qml` |

---

## 15. IMPLEMENTATION NOTES

1. **No C++ Knob/Fader reimplementation needed** - `MixxxControls.Knob` and `Skin.Fader` are C++ QtQuick items; QML only configures appearance and binds parameters.
2. **VU Meter uses OpacityMask** - requires `Qt5Compat.GraphicalEffects` (OpacitMask) for the gradient+mask composition.
3. **SectionBackground is a BorderImage** - 9-slice scaling from SVG; border=10px.
4. **ComboBox popup positioning** - clamps to screen edges (8px margin), opens upward if no space below.
5. **Android ControlFader uses relative drag** - prevents value jump on touch; desktop uses absolute mapping.
6. **TrackList column auto-hide** - columns with `autoHideWidth` hide when view narrower than threshold.
7. **SelectionModel moveSelectionVertical** - desktop wraps (modulo), android clamps (no wrap).
8. **QuickLoadBar** - always visible at bottom, updates on selectionModel.selectionChanged.
9. **Sidebar persistence** - `Mixxx.MobileLibrary.sidebarVisible` survives restarts.
10. **Storage access popup** - triggered by Browser when rekordbox/enginedj/computer nodes clicked without full storage permission.
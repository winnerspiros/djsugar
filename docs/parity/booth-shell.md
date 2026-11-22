# DJ Sugar Booth Skin Shell Specification

This document specifies the visual and behavioural contract of the Booth skin's shell - the top-level window, view switching, top bar, deck panels, waveforms, DJ view, browser, and shared controls. All colours are resolved to hex with opacity. Geometry is expressed as formulas of parent width/height using the exact QML expressions (Layout.preferred*, anchors, margins, spacing, Math.round/min/max). Canvas/Shape drawing code is reproduced verbatim. States (compact, portrait, pressed, selected, active) and their effects are listed. Every control's Mixxx group/key and interaction behaviour are documented.

---

## 1. Theme Colours (Resolved)

### 1.1 Base Theme (Theme.qml)

| Property | Hex |
|---|---|
| accentColor | #3a60be |
| backgroundColor | #1e1e20 |
| blue | #01dcfc |
| buttonActiveColor | #ffffff |
| buttonDisableColor | #747474 |
| buttonFontPixelSize | 10 |
| buttonNormalColor | #696969 |
| crossfaderBarColor | #01dcfc |
| crossfaderOrientationColor | #b0b0b0 |
| darkGray | #0f0f0f |
| darkGray2 | #2e2e2e |
| darkGray3 | #3f3f3f |
| darkGray4 | #202020 |
| deckActiveColor | #ffffff |
| deckBackgroundColor | #0f0f0f |
| deckBeatjumpBackgroundColor | #626262 |
| deckBeatjumpLabelColor | #3f3f3f |
| deckEmptyCoverArt | #3f3f3f |
| deckInfoBarBackgroundColor | #0e0e0e |
| deckLineColor | #2e2e2e |
| deckLoopBackgroundColor | #626262 |
| deckLoopLabelColor | #3f3f3f |
| deckTextColor | #b0b0b0 |
| effectColor | #fca001 |
| effectUnitColor | #ea2a4e |
| embeddedBackgroundColor | #a0000000 |
| eqFxColor | #ea2a4e |
| eqHighColor | #ffffff |
| eqLowColor | #ffffff |
| eqMidColor | #ffffff |
| fontFamily | Open Sans |
| gainKnobColor | #01dcfc |
| green | #85c85b |
| knobBackgroundColor | #262626 |
| libraryPanelSplitterBackground | #1e1e1e |
| libraryPanelSplitterHandle | #5f5f5f |
| libraryPanelSplitterHandleActive | #7a7a7a |
| lightGray | #747474 |
| lightGray2 | #b0b0b0 |
| lightGray3 | #939393 |
| midGray | #696969 |
| midGray2 | #676767 |
| midGray3 | #626262 |
| panelSplitterBackground | #1e1e20 |
| panelSplitterHandle | #696969 |
| panelSplitterHandleActive | #b0b0b0 |
| pflActiveButtonColor | #01dcfc |
| red | #ea2a4e |
| samplerColor | #01dcfc |
| sunkenBackgroundColor | #0c0c0c |
| textColor | #b0b0b0 |
| textFontPixelSize | 14 |
| toolbarActiveColor | #ffffff |
| toolbarBackgroundColor | #2e2e2e |
| volumeSliderBarColor | #01dcfc |
| warningColor | #7d3b3b |
| waveformBeatColor | #747474 |
| waveformCursorColor | #ffffff |
| waveformMarkerDefault | #ff7a01 |
| waveformMarkerIntroOutroColor | #2c5c9a |
| waveformMarkerLabel | rgba(255,255,255,0.8) |
| waveformMarkerLoopColor | #00b400 |
| waveformMarkerLoopColorDisabled | #ffffff |
| waveformPostrollColor | #696969 |
| waveformPrerollColor | #696969 |
| white | #d9d9d9 |
| yellow | #fca001 |

### 1.2 Booth Skin Overrides (skinTheme in Booth/MainWindow.qml)

| Property | Hex |
|---|---|
| accentColor | #ff4fa8 |
| backgroundColor | #000000 |
| sunkenBackgroundColor | #000000 |
| darkGray | #000000 |
| darkGray2 | #0a0b10 |
| darkGray3 | #14161c |
| darkGray4 | #0d0e13 |
| textColor | #e8e8e8 |
| panelSplitterBackground | #000000 |
| libraryScrollHandle | #333333 |
| libraryScrollHandleHovered | #7c5cff |
| libraryScrollHandlePressed | #ff4fa8 |
| libraryScrollTrack | #000000 |
| libraryActionBackground | #0a0b10 |
| libraryActionDisabledBackground | #0d0e13 |
| libraryActionText | #eeeeee |
| libraryActionDisabledText | #666666 |
| libraryDeck1Background | #1f6fd1 |
| libraryDeck1Pressed | #4aa3f5 |
| libraryDeck2Background | #e8650f |
| libraryDeck2Pressed | #ff9a52 |
| libraryConfirmBackground | #ff4fa8 |
| libraryKeyMatchColor | #00e676 |
| libraryKeyMatchBackground | #00e67633 |

**Note:** The Booth skin applies its `skinTheme` at runtime, overriding the base Theme values listed above. All colour references in this spec use the overridden values where the skin provides them; otherwise the base Theme value applies.

---

## 2. Root Window (Booth/MainWindow.qml)

### 2.1 Root Item

- **Type:** `Item`
- **Id:** `root`
- **Required property:** `ApplicationWindow applicationWindow`
- **Property:** `menuBar` (default null)

### 2.2 View Mode Constants

| Value | Name | Description |
|---|---|---|
| 1 | WAVE | Waveforms view (default play view) |
| 2 | BROWSE | Library browser (BoothBrowse) |
| 3 | DJ | Touch controls (BoothDjView) |
| 4 | XY | XY pad overlay |

- `property int viewMode: 1`
- `readonly property bool browsing: viewMode === 2`
- `readonly property bool djView: viewMode === 3`
- `readonly property bool xyView: viewMode === 4`

### 2.3 Controller Detection

```qml
readonly property bool controllerConnected: {
    const devices = Mixxx.ControllerManager.knownDevices;
    for (let i = 0; i < devices.length; ++i) {
        if (devices[i].enabled) return true;
    }
    return false;
}
```

### 2.4 Compact / Phone Layout

- `readonly property bool compact: height < 520`
- `property bool beatFxShown: false`
- `property bool beatFxPanelShown: true`

### 2.5 Fixed Configuration

| Property | Value |
|---|---|
| numDecks | 4 |
| numSamplers | 16 |
| numPreviewDecks | 1 |
| waveformOverviewTypeRgb | 2 |

### 2.6 View Switching Functions

```qml
function toggleBeatFx() {
    if (root.compact) {
        if (root.browsing || root.xyView) {
            root.showPlayView();
            root.beatFxShown = true;
        } else {
            root.beatFxShown = !root.beatFxShown;
        }
    } else {
        root.beatFxPanelShown = !root.beatFxPanelShown;
    }
}

function openQuickLoad(group) {
    root.loadTarget = (group && group.length > 0) ? group : "[Channel1]";
    root.viewMode = 2;
}

property int playView: 1

function showPlayView() {
    root.loadTarget = "";
    root.viewMode = root.playView;
}

function showView(mode) {
    root.loadTarget = "";
    root.viewMode = mode;
    root.playView = mode;
}
```

### 2.7 Top Bar Items (Tabs)

```qml
readonly property var topBarItems: [
    { "name": "wave", "label": qsTr("WAVE"), "view": true },
    { "name": "dj",   "label": "DJ",         "view": true },
    { "name": "lib",  "label": qsTr("BROWSE"), "view": true },
    { "name": "load1","label": qsTr("LOAD %1").arg(1), "view": false },
    { "name": "load2","label": qsTr("LOAD %1").arg(2), "view": false },
    { "name": "xy",   "label": "XY",         "view": false },
    { "name": "fx",   "label": "BEAT FX",    "view": false },
    { "name": "split","label": "SPLIT",      "view": false },
    { "name": "rec",  "label": "REC",        "view": false }
]
readonly property var hiddenTopBarItems: Mixxx.MobileApp.hiddenTopBarItems
readonly property var viewNames: { "1": "wave", "2": "lib", "3": "dj", "4": "xy" }
```

- `function itemShown(name) { return root.hiddenTopBarItems.indexOf(name) < 0; }`
- `function preferredView()` returns first shown in order: controller connected to [1,3,2] else [3,1,2]
- `Component.onCompleted: root.showView(root.preferredView())`
- `onHiddenTopBarItemsChanged: root.leaveHiddenView()`
- `onControllerConnectedChanged:` switches between WAVE/DJ if current view hidden

### 2.8 Controller Access (Skin Controls)

| Control | Group | Key | Behaviour |
|---|---|---|---|
| mobile_library_toggle | [Skin] | mobile_library_toggle | Toggles BROWSE view |
| mobile_fx_toggle | [Skin] | mobile_fx_toggle | Calls `toggleBeatFx()` |
| mobile_loader_deck1 | [Skin] | mobile_loader_deck1 | Opens browser for `[Channel1]` |
| mobile_loader_deck2 | [Skin] | mobile_loader_deck2 | Opens browser for `[Channel2]` |
| LoadSelectedTrack | [ChannelN] | LoadSelectedTrack | Opens browser if not visible |

---

## 3. Top Bar (Booth/MainWindow.qml lines 512-610)

### 3.1 Structure

```
Row (id: topBar)
  ├─ SugarLogo
  ├─ Tab (WAVE)
  ├─ Tab (DJ)
  ├─ Tab (BROWSE)
  ├─ Tab (LOAD 1)
  ├─ Tab (LOAD 2)
  └─ Row (right group)
       ├─ Tab (SPLIT)
       ├─ MobileRecordButton
       ├─ Tab (FX ST)
       ├─ Tab (XY)
       ├─ Tab (BEAT FX)  // compact only
       ├─ Tab (Stems indicator)
       └─ Tab (UTILITY)
```

### 3.2 Geometry

- `anchors.left: parent.left`, `anchors.leftMargin: 4`
- `anchors.top: parent.top`, `anchors.topMargin: 3`
- `height: root.compact ? 26 : 34`
- `spacing: root.width < 760 ? 3 : 6`

### 3.3 SugarLogo

- `anchors.verticalCenter: parent.verticalCenter`
- `showWordmark: root.width >= 1000`
- `size: root.compact ? 24 : 30`

### 3.4 Tab Component (Booth/MainWindow.qml lines 269-304)

```
Rectangle (id: tab)
  ├─ required property string label
  ├─ property bool active: false
  ├─ signal clicked
  ├─ border.color: active ? Theme.accentColor (#ff4fa8) : Theme.midGray3 (#626262)
  ├─ border.width: active ? 2 : 1
  ├─ color: tabTap.pressed ? Theme.darkGray3 (#14161c) : Theme.backgroundColor (#000000)
  ├─ height: parent.height
  ├─ width: root.width < 760
          ? Math.max(44, tabText.implicitWidth + 12)
          : Math.max(root.compact ? 64 : 96, tabText.implicitWidth + 24)
  └─ Text (id: tabText)
       ├─ anchors.centerIn: parent
       ├─ color: tab.active ? Theme.accentColor (#ff4fa8) : Theme.lightGray3 (#939393)
       ├─ font.bold: true
       ├─ font.pixelSize: root.width < 760 ? 10 : (root.compact ? 11 : 13)
       └─ text: tab.label
  └─ TapHandler (id: tabTap)
       ├─ gesturePolicy: TapHandler.ReleaseWithinBounds
       └─ onTapped: tab.clicked()
```

### 3.5 Left Tabs (in order)

| Tab | Active Condition | Visible Condition | OnClick |
|---|---|---|---|
| WAVE | `viewMode === 1` | `itemShown("wave")` | `showView(1)` |
| DJ | `djView` | `itemShown("dj")` | `showView(3)` |
| BROWSE | `browsing && loadTarget.length === 0` | `itemShown("lib")` | `toggleLibraryView()` |
| LOAD 1 | `browsing && loadTarget === "[Channel1]"` | `itemShown("load1")` | `openQuickLoad("[Channel1]")` |
| LOAD 2 | `browsing && loadTarget === "[Channel2]"` | `itemShown("load2")` | `openQuickLoad("[Channel2]")` |

### 3.6 Right Tabs (in order)

| Tab | Active Condition | Visible Condition | OnClick |
|---|---|---|---|
| SPLIT | `headSplitControl.value > 0` | `itemShown("split")` | Toggle `headSplitControl.value` |
| REC | (MobileRecordButton) | `itemShown("rec")` | Record handling |
| FX ST | `fxStationLoader.active` | always | Toggle `fxStationLoader.active` |
| XY | `xyView` | `itemShown("xy")` | Toggle XY view |
| BEAT FX | `beatFxShown` | `compact && !browsing && !xyView && itemShown("fx")` | Toggle `beatFxShown` |
| Stems | `stemsOverlay.visible` | `Mixxx.Stems.pendingCount > 0` | Toggle `stemsOverlay.visible` |
| UTILITY | `settingsPopup.opened` | always | `settingsPopup.open()` |

### 3.7 MobileRecordButton (Skin.MobileRecordButton)

- `fontPixelSize: root.width < 760 ? 10 : (root.compact ? 11 : 13)`
- `height: parent.height`
- `idleBorderColor: Theme.midGray3 (#626262)`
- `idleColor: Theme.backgroundColor (#000000)`
- `radius: 0`
- `width: Math.max(root.width < 760 ? 44 : (root.compact ? 64 : 80), implicitWidth)`

---

## 4. Content Area (Booth/MainWindow.qml lines 485-798)

### 4.1 Root Content Item

```
Item (id: content)
  ├─ readonly property int gap: 3
  ├─ anchors.fill: parent
  ├─ enabled: !settingsPopup.opened
  ├─ CutoutInsets (panelInset) to target: deckPanels
  ├─ CutoutInsets (middleInset) to target: middle
  └─ CutoutInsets (djViewInset) to target: djView
```

### 4.2 Deck Panels (Bottom Row)

```
Row (id: deckPanels)
  ├─ visible: !fxStationLoader.covering
  ├─ anchors.bottom: root.djView ? djView.top : parent.bottom
  ├─ anchors.bottomMargin: content.gap (3)
  ├─ anchors.left: parent.left, anchors.leftMargin: panelInset.left + content.gap
  ├─ anchors.right: parent.right, anchors.rightMargin: panelInset.right + content.gap
  ├─ height: root.viewMode !== 1
           ? (root.compact ? 58 : 84)
           : Math.round(root.height * (root.compact ? 0.36 : 0.3))
  ├─ spacing: content.gap (3)
  ├─ BoothDeckPanel (deck 1)
  │    ├─ deckNumber: 1
  │    ├─ group: "[Channel1]"
  │    ├─ height: parent.height
  │    ├─ slim: root.viewMode !== 1
  │    └─ width: (parent.width - parent.spacing) / 2
  └─ BoothDeckPanel (deck 2)
       ├─ deckNumber: 2
       ├─ group: "[Channel2]"
       ├─ height: parent.height
       ├─ slim: root.viewMode !== 1
       └─ width: (parent.width - parent.spacing) / 2
```

### 4.3 DJ View (BoothDjView)

```
BoothDjView (id: djView)
  ├─ anchors.bottom: parent.bottom, anchors.bottomMargin: content.gap
  ├─ anchors.left: parent.left, anchors.leftMargin: djViewInset.left + content.gap
  ├─ anchors.right: parent.right, anchors.rightMargin: djViewInset.right + content.gap
  ├─ compact: root.compact
  ├─ height: Math.round(root.height * (root.compact ? 0.5 : 0.46))
  └─ visible: root.djView && !fxStationLoader.covering
```

**See Section 7 for BoothDjView internal structure.**

### 4.4 Middle Area (Waveforms / Browser / XY)

```
Item (id: middle)
  ├─ visible: !fxStationLoader.active
  ├─ anchors.bottom: deckPanels.top, anchors.bottomMargin: content.gap
  ├─ anchors.left: parent.left
  ├─ anchors.right: parent.right
  ├─ anchors.top: topBar.bottom, anchors.topMargin: content.gap
  ├─ Item (waveforms + deckInfos)  // visible: !browsing && !xyView
  │    ├─ Column (id: deckInfos)
  │    │    ├─ anchors.bottom: parent.bottom
  │    │    ├─ anchors.left: parent.left, anchors.leftMargin: middleInset.left + content.gap
  │    │    ├─ anchors.top: parent.top
  │    │    ├─ spacing: content.gap
  │    │    ├─ visible: !(root.djView && root.compact)
  │    │    ├─ width: visible ? (root.compact ? 86 : 124) : 0
  │    │    ├─ DeckInfo (deck 1)
  │    │    │    ├─ deckNumber: 1, group: "[Channel1]"
  │    │    │    ├─ height: (parent.height - parent.spacing) / 2
  │    │    │    └─ width: parent.width
  │    │    └─ DeckInfo (deck 2)
  │    │         ├─ deckNumber: 2, group: "[Channel2]"
  │    │         ├─ height: (parent.height - parent.spacing) / 2
  │    │         └─ width: parent.width
  │    └─ Column (waveforms)
  │         ├─ anchors.bottom: parent.bottom
  │         ├─ anchors.left: deckInfos.right, anchors.leftMargin: content.gap
  │         ├─ anchors.right: beatFx.visible ? beatFx.left : parent.right
  │         ├─ anchors.rightMargin: content.gap
  │         ├─ anchors.top: parent.top
  │         ├─ Skin.WaveformDisplay (deck 1)
  │         │    ├─ group: "[Channel1]"
  │         │    ├─ height: parent.height / 2
  │         │    ├─ isTouchMode: true
  │         │    ├─ showHud: false
  │         │    └─ width: parent.width
  │         └─ Skin.WaveformDisplay (deck 2)
  │              ├─ group: "[Channel2]"
  │              ├─ height: parent.height / 2
  │              ├─ isTouchMode: true
  │              ├─ showHud: false
  │              └─ width: parent.width
  ├─ Loader (id: library)  // BoothBrowse
  │    ├─ property bool keepLoaded: false
  │    ├─ anchors.fill: parent
  │    ├─ active: keepLoaded || root.browsing
  │    ├─ visible: root.browsing
  │    ├─ onLoaded: keepLoaded = true
  │    └─ sourceComponent: BoothBrowse { anchors.fill: parent; compact: root.compact; loadTarget: root.loadTarget; acceptsController: !settingsPopup.opened }
  ├─ Skin.XYPad  // visible: root.xyView
  │    ├─ anchors.fill: parent
  │    ├─ accentColor: Theme.libraryDeck1Background (#1f6fd1)
  │    ├─ border.color: Theme.midGray3 (#626262)
  │    ├─ buttonBorderColor: Theme.midGray2 (#676767)
  │    ├─ buttonColor: Theme.darkGray4 (#0d0e13)
  │    ├─ buttonTextColor: Theme.lightGray3 (#939393)
  │    ├─ closable: false
  │    ├─ color: Theme.darkGray4 (#0d0e13)
  │    ├─ controlRadius: 2
  │    ├─ deck1Color: Theme.libraryDeck1Background (#1f6fd1)
  │    ├─ deck2Color: Theme.libraryDeck2Background (#e8650f)
  │    ├─ holdColor: Theme.warningColor (#7d3b3b)
  │    ├─ radius: 0
  │    ├─ surfaceBorderColor: Theme.midGray3 (#626262)
  │    ├─ surfaceColor: Theme.backgroundColor (#000000)
  │    ├─ surfaceRadius: 0
  │    ├─ title: qsTr("XY PAD")
  │    ├─ visible: root.xyView
  │    └─ wetColor: Theme.accentColor (#ff4fa8)
  └─ BoothBeatFx (phone overlay)
       ├─ anchors.bottom: parent.bottom
       ├─ anchors.right: parent.right, anchors.rightMargin: content.gap
       ├─ anchors.top: parent.top
       ├─ compact: true
       ├─ visible: root.compact && root.beatFxShown && !root.browsing && !root.xyView && root.itemShown("fx")
       ├─ width: 190
       └─ z: 10
```

### 4.5 FX Station Loader

```
Loader (id: fxStationLoader)
  ├─ readonly property bool docked: middle.height >= 240
  ├─ readonly property bool covering: active && !docked
  ├─ readonly property int topMargin: root.compact ? 32 : 40
  ├─ active: false
  ├─ height: docked ? middle.height : parent.height - topMargin
  ├─ width: parent.width
  ├─ x: 0
  ├─ y: docked ? middle.mapToItem(root, 0, 0).y + 0 * (middle.y + deckPanels.height + djView.height) : topMargin
  ├─ z: 90
  └─ sourceComponent: Skin.FxStation { onCloseRequested: fxStationLoader.active = false }
```

### 4.6 Settings Popup

```
Skin.Settings (id: settingsPopup)
  ├─ backgroundColor: Theme.backgroundColor (#000000)
  ├─ headerColor: Theme.darkGray3 (#14161c)
  ├─ title: qsTr("UTILITY")
  ├─ height: root.applicationWindow.isMobile ? parent.height : Math.min(840, parent.height)
  ├─ modal: true
  ├─ width: root.applicationWindow.isMobile ? parent.width : Math.min(1400, parent.width)
  ├─ x: root.applicationWindow.isMobile ? 0 : Math.round((parent.width - width) / 2)
  ├─ y: root.applicationWindow.isMobile ? 0 : Math.round((parent.height - height) / 2)
  └─ Overlay.modal: Rectangle { color: "#99000000" }
```

### 4.7 Stems Overlay

```
Skin.StemsOverlay (id: stemsOverlay)
  └─ anchors.fill: parent, z: 95
```

---

## 5. Waveform Stack (Android/+android/MainWindow.qml lines 512-590)

### 5.1 SplitView Handle

```
SplitView (id: splitView)
  ├─ handle: Rectangle (id: handleDelegate)
  │    ├─ property color handleColor: SplitHandle.pressed || SplitHandle.hovered
  │    │       ? Theme.libraryPanelSplitterHandleActive (#7a7a7a)
  │    │       : Theme.libraryPanelSplitterHandle (#5f5f5f)
  │    ├─ property int handleSize: SplitHandle.pressed || SplitHandle.hovered ? 6 : 3
  │    ├─ clip: true
  │    ├─ color: Theme.libraryPanelSplitterBackground (#1e1e1e)
  │    ├─ implicitHeight: 4
  │    ├─ implicitWidth: 8
  │    ├─ containmentMask: Item { height: 8; width: splitView.width; x: (handleDelegate.width - width) / 2 }
  │    └─ RowLayout (anchors.centerIn: parent)
  │         └─ Repeater (model: 3)
  │              └─ Rectangle { color: handleColor; height: handleSize; radius: handleSize; width: handleSize }
```

### 5.2 Waveforms Item (id: waveforms)

- `readonly property real waveDeckBarHeight: root.viewMode === 3 ? Math.min(115, Math.max(86, Math.round(parent.height * 0.38))) : 0`
- `readonly property real scrollingHeight: Math.max(0, height - waveDeckBarHeight)`
- `SplitView.fillHeight: root.viewMode === 3`
- `SplitView.preferredHeight:`
  - `viewMode === 3` to `splitView.height`
  - `viewMode === 0` to `Math.round(splitView.height * 0.42)`
  - `viewMode === 2 || maximizeLibrary` to `0`
  - else to `Math.max(56, Math.min(96, Math.round(splitView.height * 0.22)))`
- `visible: root.viewMode !== 2 && !root.maximizeLibrary`
- `FadeBehavior on visible { fadeTarget: waveforms }`

### 5.3 Waveform Stack (2-deck mode)

```
Loader (deck3waveform) active: root.show4decks
  └─ Skin.WaveformDisplay { group: "[Channel3]"; isTouchMode: root.viewMode === 1; height: waveforms.scrollingHeight / 4; width: root.width }

Skin.WaveformDisplay (deck1waveform)
  ├─ anchors.top: root.show4decks ? deck3waveform.bottom : parent.top
  ├─ group: "[Channel1]"
  ├─ height: waveforms.scrollingHeight / (root.show4decks ? 4 : 2)
  ├─ isTouchMode: root.viewMode === 1
  └─ width: root.width

Skin.WaveformDisplay (deck2waveform)
  ├─ anchors.top: deck1waveform.bottom
  ├─ group: "[Channel2]"
  ├─ height: waveforms.scrollingHeight / (root.show4decks ? 4 : 2)
  ├─ isTouchMode: root.viewMode === 1
  └─ width: root.width

Loader (deck4waveform) active: root.show4decks
  └─ Skin.WaveformDisplay { group: "[Channel4]"; isTouchMode: root.viewMode === 1; height: waveforms.scrollingHeight / 4; width: root.width }
```

### 5.4 WaveDeckBar (visible when viewMode === 3)

```
WaveDeckBar (id: waveDeckBar)
  ├─ anchors.bottom: parent.bottom
  ├─ anchors.left: parent.left
  ├─ anchors.right: parent.right
  ├─ height: waveforms.waveDeckBarHeight
  ├─ visible: root.viewMode === 3
  └─ z: 10
```

---

## 6. Lower Section (Android/+android/MainWindow.qml lines 602-927)

### 6.1 SplitView (id: lowerSection)

- `SplitView.fillHeight: root.viewMode !== 3`
- `SplitView.minimumHeight: (root.viewMode === 1 && !root.maximizeLibrary) ? mixer.height : 60`
- `visible: root.viewMode !== 3`

### 6.2 Deck 1 (left)

```
Deck (id: deck1)
  ├─ editMode: root.editDeck
  ├─ group: "[Channel1]"
  ├─ height: (root.viewMode === 2 || root.maximizeLibrary) ? 70 : root.show4decks ? mixer.height / 2 : mixer.height
  ├─ minimized: root.viewMode === 2 || root.maximizeLibrary
  ├─ visible: root.viewMode === 1 || root.viewMode === 2
  ├─ Behavior on height: SpringAnimation { damping: 0.2; duration: 500; spring: 2 }
  ├─ State (when: root.maximizeLibrary)
  │    └─ AnchorChanges { anchors.right: parent.horizontalCenter; target: deck1 }
  ├─ onToggleFocus: root.focusedDeck = (root.focusedDeck === deck1) ? null : deck1
  └─ anchors { left: parent.left; right: mixer.left; top: parent.top }
```

### 6.3 Mixer (center)

```
Mixer (id: mixer)
  ├─ anchors.horizontalCenter: parent.horizontalCenter
  ├─ anchors.top: parent.top
  ├─ groups: [deck1.group, deck2.group, deck3.group, deck4.group]
  ├─ height: implicitHeight
  ├─ show4decks: root.show4decks
  ├─ visible: root.viewMode === 1 && !root.maximizeLibrary
  ├─ width: implicitWidth
  ├─ Behavior on height: SpringAnimation { damping: 0.2; duration: 500; spring: 2 }
  ├─ State (when: (!root.focusedDeck || root.width > 1400) && !root.maximizeLibrary)
  │    └─ AnchorChanges { anchors.horizontalCenter: parent.horizontalCenter; target: mixer }
  ├─ State (when: root.maximizeLibrary || root.viewMode === 2)
  │    ├─ AnchorChanges { anchors.horizontalCenter: parent.horizontalCenter; target: mixer }
  │    ├─ PropertyChanges { target: deck1; width: root.width / 2 }
  │    └─ PropertyChanges { target: deck2; width: root.width / 2 }
  ├─ transitions: Transition { AnchorAnimation { duration: 200 } }
  └─ FadeBehavior on visible { fadeTarget: mixer }
```

### 6.4 Deck 2 (right)

```
Deck (id: deck2)
  ├─ editMode: root.editDeck
  ├─ group: "[Channel2]"
  ├─ height: (root.viewMode === 2 || root.maximizeLibrary) ? 70 : root.show4decks ? mixer.height / 2 : mixer.height
  ├─ minimized: root.viewMode === 2 || root.maximizeLibrary
  ├─ visible: root.viewMode === 1 || root.viewMode === 2
  ├─ Behavior on height: SpringAnimation { damping: 0.2; duration: 500; spring: 2 }
  ├─ State (when: root.maximizeLibrary)
  │    └─ AnchorChanges { anchors.left: parent.horizontalCenter; target: deck2 }
  ├─ onToggleFocus: root.focusedDeck = (root.focusedDeck === deck2) ? null : deck2
  └─ anchors { left: mixer.right; right: parent.right; top: parent.top }
```

### 6.5 Deck 3 (below deck 1, 4-deck mode)

```
Loader (id: deck3) active: root.show4decks
  ├─ readonly property string group: "[Channel3]"
  ├─ height: active ? (root.maximizeLibrary ? 80 : mixer.height / 2) : 0
  ├─ Behavior on height: SpringAnimation { damping: 0.2; duration: 500; spring: 2 }
  ├─ sourceComponent: Deck { anchors.bottom: parent.bottom; anchors.left: parent.left; editMode: root.editDeck; group: deck3.group; minimized: root.maximizeLibrary }
  ├─ State (when: root.maximizeLibrary)
  │    └─ AnchorChanges { anchors.right: parent.horizontalCenter; target: deck3 }
  └─ anchors { left: parent.left; right: mixer.left; top: deck1.bottom }
```

### 6.6 Deck 4 (below deck 2, 4-deck mode)

```
Loader (id: deck4) active: root.show4decks
  ├─ readonly property string group: "[Channel4]"
  ├─ height: active ? (root.maximizeLibrary ? 80 : mixer.height / 2) : 0
  ├─ Behavior on height: SpringAnimation { damping: 0.2; duration: 500; spring: 2 }
  ├─ sourceComponent: Deck { anchors.bottom: parent.bottom; anchors.right: parent.right; editMode: root.editDeck; group: deck4.group; minimized: root.maximizeLibrary }
  ├─ State (when: root.maximizeLibrary)
  │    └─ AnchorChanges { anchors.left: parent.horizontalCenter; target: deck4 }
  └─ anchors { left: mixer.right; right: parent.right; top: deck2.bottom }
```

### 6.7 Library (Skin.Library)

```
Loader (id: library)
  ├─ property bool keepLoaded: false
  ├─ active: keepLoaded || root.viewMode === 0 || root.viewMode === 2 || root.maximizeLibrary || (root.viewMode === 1 && root.height - mixer.height >= 400)
  ├─ onLoaded: keepLoaded = true
  ├─ visible: root.viewMode === 0 || root.viewMode === 2 || root.maximizeLibrary || (root.viewMode === 1 && root.height - mixer.height >= 400)
  ├─ width: parent.width
  ├─ sourceComponent: Skin.Library { anchors.fill: parent; acceptsController: !quickLoadDrawer.opened && !settingsPopup.opened }
  ├─ State (viewMode === 0): anchors.bottom: parent.bottom; anchors.top: parent.top; visible: true
  ├─ State (maximizeLibrary && show4decks): anchors.bottom: parent.bottom; anchors.top: deck4.bottom; visible: true
  ├─ State (maximizeLibrary && !show4decks): anchors.bottom: parent.bottom; anchors.top: deck1.bottom; visible: true
  ├─ State (viewMode === 1 && height - mixer.height >= 400): anchors.bottom: parent.bottom; anchors.top: mixer.bottom; visible: true
  ├─ State (viewMode === 1 && height - mixer.height < 400): visible: false
  └─ anchors { left: parent.left; right: parent.right; bottom: parent.bottom; top: root.viewMode === 0 ? parent.top : mixer.bottom }
```

### 6.8 FX Station Loader (Android)

```
Loader (id: fxStationLoader)
  ├─ readonly property bool docked: root.viewMode === 0 || root.viewMode === 2 || root.maximizeLibrary || (root.viewMode === 1 && root.height - mixer.height >= 400)
  ├─ readonly property bool covering: active && !docked
  ├─ active: false
  ├─ height: docked ? library.height : parent.height - 44
  ├─ width: parent.width
  ├─ x: 0
  ├─ y: docked ? library.mapToItem(root, 0, 0).y + 0 * (library.y + lowerSection.y + splitView.y + effects.height) : 44
  ├─ z: 92
  └─ sourceComponent: Skin.FxStation { onCloseRequested: fxStationLoader.active = false }
```

### 6.9 XY Pad Overlay

```
Skin.XYPad (id: xyPadOverlay)
  ├─ visible: false
  ├─ z: 90
  ├─ anchors.centerIn: parent
  ├─ width: Math.min(680, parent.width - 24)
  ├─ height: Math.min(380, parent.height - 50)
  └─ onCloseRequested: visible = false
```

### 6.10 Stems Overlay

```
Skin.StemsOverlay (id: stemsOverlay)
  ├─ anchors.fill: parent
  └─ z: 95
```

### 6.11 Settings Popup (Android)

```
Skin.Settings (id: settingsPopup)
  ├─ height: root.applicationWindow.isMobile ? parent.height : Math.min(840, parent.height)
  ├─ modal: true
  ├─ width: root.applicationWindow.isMobile ? parent.width : Math.min(1400, parent.width)
  ├─ x: root.applicationWindow.isMobile ? 0 : Math.round((parent.width - width) / 2)
  ├─ y: root.applicationWindow.isMobile ? 0 : Math.round((parent.height - height) / 2)
  └─ Overlay.modal: Rectangle (id: overlayModal)
       ├─ readonly property bool hasHardwareAcceleration: Mixxx.Config.useAcceleration
       ├─ property real radius: 12
       ├─ anchors.fill: parent
       ├─ color: Qt.alpha('#00000010', hasHardwareAcceleration ? 1.0 : 0.6)
       └─ Repeater (model: hasHardwareAcceleration ? 1 : 0)
            └─ GaussianBlur { anchors.fill: overlayModal; deviation: 4; radius: Math.max(0, overlayModal.radius); samples: 16; source: content }
```

---

## 7. BoothDjView (Booth/BoothDjView.qml)

### 7.1 Root

```
Item (id: root)
  ├─ property bool compact: false
  ├─ readonly property int gap: 3
  └─ ColumnLayout (anchors.fill: parent; spacing: root.gap)
```

### 7.2 Top Row (Decks + Mixer)

```
RowLayout (Layout.fillHeight: true; Layout.fillWidth: true; spacing: root.gap)
  ├─ BoothPerformanceDeck (deck 1)
  │    ├─ Layout.fillHeight: true
  │    ├─ Layout.fillWidth: true
  │    ├─ compact: root.compact
  │    ├─ deckNumber: 1
  │    └─ group: "[Channel1]"
  ├─ BoothMixer
  │    ├─ Layout.fillHeight: true
  │    └─ Layout.preferredWidth: Math.round(Math.min(root.width * 0.24, root.height * 0.9))
  └─ BoothPerformanceDeck (deck 2)
       ├─ Layout.fillHeight: true
       ├─ Layout.fillWidth: true
       ├─ compact: root.compact
       ├─ deckNumber: 2
       ├─ group: "[Channel2]"
       └─ mirrored: true
```

### 7.3 Bottom Bar (Headphones, Crossfader, Master)

```
Rectangle (id: bottomBar)
  ├─ readonly property real knobSize: height - 4
  ├─ Layout.fillWidth: true
  ├─ Layout.preferredHeight: root.compact ? 30 : 40
  ├─ border.color: Theme.midGray3 (#626262)
  ├─ border.width: 1
  ├─ color: Theme.darkGray4 (#0d0e13)
  ├─ Row (left group)
  │    ├─ anchors.left: parent.left, anchors.leftMargin: 8
  │    ├─ anchors.verticalCenter: parent.verticalCenter
  │    ├─ spacing: 8
  │    ├─ Text (headphones icon) { anchors.verticalCenter; color: Theme.lightGray3 (#939393); font.pixelSize: Math.round(bottomBar.height * 0.45); text: "🎧" }
  │    ├─ Text { anchors.verticalCenter; color: Theme.lightGray3; font.pixelSize: Math.round(bottomBar.height * 0.28); text: qsTr("MIXING") }
  │    ├─ BoothKnob { group: "[Master]"; key: "headMix"; height: bottomBar.knobSize; width: bottomBar.knobSize }
  │    ├─ Text { anchors.verticalCenter; color: Theme.lightGray3; font.pixelSize: Math.round(bottomBar.height * 0.28); text: qsTr("LEVEL") }
  │    └─ BoothKnob { bipolar: false; group: "[Master]"; key: "headGain"; height: bottomBar.knobSize; width: bottomBar.knobSize }
  ├─ BoothFader (crossfader)
  │    ├─ anchors.centerIn: parent
  │    ├─ centerMark: true
  │    ├─ group: "[Master]"
  │    ├─ height: bottomBar.height - 6
  │    ├─ horizontal: true
  │    ├─ key: "crossfader"
  │    └─ width: Math.round(Math.min(parent.width * 0.3, 320))
  └─ Row (right group)
       ├─ anchors.right: parent.right, anchors.rightMargin: 8
       ├─ anchors.verticalCenter: parent.verticalCenter
       ├─ spacing: 8
       ├─ Text { anchors.verticalCenter; color: Theme.lightGray3; font.pixelSize: Math.round(bottomBar.height * 0.28); text: qsTr("MASTER") }
       └─ BoothKnob { bipolar: false; group: "[Master]"; key: "gain"; height: bottomBar.knobSize; width: bottomBar.knobSize }
```

---

## 8. BoothMixer (Booth/BoothMixer.qml)

### 8.1 Root

```
Rectangle (id: root)
  ├─ readonly property real unit: Math.max(9, Math.min(height / 14, width / 11))
  ├─ border.color: Theme.midGray3 (#626262)
  ├─ border.width: 1
  └─ color: Theme.darkGray4 (#0d0e13)
```

### 8.2 LevelMeter Component

```
Item (id: meter)
  ├─ required property string group
  ├─ Mixxx.ControlProxy (vuControl) { group: meter.group; key: "vu_meter"; updateInterval: 33 }
  ├─ Mixxx.ControlProxy (peakControl) { group: meter.group; key: "peak_indicator"; updateInterval: 33 }
  ├─ Rectangle { anchors.fill: parent; color: Theme.darkGray4 (#0d0e13) }
  └─ Rectangle (meter bar)
       ├─ anchors.bottom: parent.bottom; anchors.left: parent.left; anchors.right: parent.right
       ├─ color: peakControl.value > 0 ? Theme.warningColor (#7d3b3b)
                  : (vuControl.value > 0.8 ? Theme.yellow (#fca001) : Theme.green (#85c85b))
       └─ height: Math.round(parent.height * Math.max(0, Math.min(1, vuControl.value)))
```

### 8.3 Channel Component (per deck)

```
RowLayout (id: channel)
  ├─ required property string group
  ├─ required property color deckColor
  ├─ readonly property string eqGroup: `[EqualizerRack1_${group}_Effect1]`
  ├─ readonly property string quickEffectGroup: `[QuickEffectRack1_${group}]`
  ├─ spacing: 3
  ├─ Mixxx.ControlProxy (pflControl) { group: channel.group; key: "pfl" }
  ├─ Mixxx.ControlProxy (quickEffectEnabledControl) { group: channel.quickEffectGroup; key: "enabled" }
  ├─ ColumnLayout (knobs)
  │    ├─ Layout.fillHeight: true; Layout.fillWidth: true; spacing: 2
  │    ├─ Repeater (model: TRIM, HIGH, MID, LOW)
  │    │    └─ BoothKnob { Layout.fillHeight: true; Layout.fillWidth: true; group: modelData.group; key: modelData.key; label: modelData.label }
  │    └─ BoothKnob (FILTER)
  │         ├─ Layout.fillHeight: true; Layout.fillWidth: true
  │         ├─ color: Theme.green (#85c85b)
  │         ├─ group: channel.quickEffectGroup
  │         ├─ key: "super1"
  │         ├─ label: qsTr("FILTER")
  │         └─ opacity: quickEffectEnabledControl.value > 0 ? 1 : 0.4
  └─ ColumnLayout (CUE, meter, fader)
       ├─ Layout.fillHeight: true; Layout.preferredWidth: Math.round(root.unit * 2.8); spacing: 3
       ├─ Rectangle (CUE button)
       │    ├─ Layout.fillWidth: true; Layout.preferredHeight: Math.round(root.unit * 1.8)
       │    ├─ border.color: pflControl.value > 0 ? Theme.warningColor (#7d3b3b) : Theme.midGray2 (#676767)
       │    ├─ color: pflControl.value > 0 ? "#3a1a08" : Theme.darkGray4 (#0d0e13)
       │    ├─ radius: 2
       │    ├─ Text { anchors.centerIn: parent; color: pflControl.value > 0 ? Theme.warningColor : Theme.lightGray2 (#b0b0b0); font.bold: true; font.pixelSize: Math.round(root.unit * 0.95); text: "CUE" }
       │    └─ TapHandler { onTapped: pflControl.value = pflControl.value > 0 ? 0 : 1 }
       └─ RowLayout
            ├─ Layout.fillHeight: true; Layout.fillWidth: true; spacing: 2
            ├─ LevelMeter { Layout.fillHeight: true; Layout.preferredWidth: Math.max(3, Math.round(root.unit * 0.45)); group: channel.group }
            └─ BoothFader { Layout.fillHeight: true; Layout.fillWidth: true; color: channel.deckColor; group: channel.group; key: "volume" }
```

### 8.4 Mixer Layout

```
RowLayout (anchors.fill: parent; anchors.margins: 4; spacing: 6)
  ├─ Channel { Layout.fillHeight: true; Layout.fillWidth: true; deckColor: Theme.libraryDeck1Background (#1f6fd1); group: "[Channel1]" }
  ├─ Rectangle { Layout.fillHeight: true; Layout.preferredWidth: 1; color: Theme.midGray3 (#626262) }
  └─ Channel { Layout.fillHeight: true; Layout.fillWidth: true; deckColor: Theme.libraryDeck2Background (#e8650f); group: "[Channel2]" }
```

---

## 9. BoothKnob (Booth/BoothKnob.qml)

### 9.1 Properties

| Property | Default | Description |
|---|---|---|
| group | required | Mixxx control group |
| key | required | Mixxx control key |
| label | "" | Label text below knob |
| bipolar | true | Ring lights from middle (EQ) vs start (gain) |
| color | Theme.libraryDeck1Background (#1f6fd1) | Ring colour |
| dragRange | 160 | Pixels per full turn |

### 9.2 Geometry

- `implicitHeight: 40 + (labelText.visible ? labelText.implicitHeight + 1 : 0)`
- `implicitWidth: 40`
- `readonly property real ringSize: Math.max(12, Math.min(width, height - (labelText.visible ? labelText.height + 1 : 0)))`

### 9.3 Canvas Drawing (id: ring)

```
Canvas {
  anchors.horizontalCenter: parent.horizontalCenter
  anchors.top: parent.top
  height: root.ringSize
  width: root.ringSize
  onPaint: {
    const ctx = getContext("2d");
    ctx.reset();
    const center = width / 2;
    const lineWidth = Math.max(2, width / 11);
    const radius = center - lineWidth;
    const start = Math.PI * 0.75;
    const end = Math.PI * 2.25;
    const at = start + root.clamp(root.value) * (end - start);
    const from = root.bipolar ? Math.PI * 1.5 : start;
    ctx.lineCap = "round";
    ctx.lineWidth = lineWidth;
    ctx.strokeStyle = Theme.midGray3;  // #626262
    ctx.beginPath();
    ctx.arc(center, center, radius, start, end);
    ctx.stroke();
    if (Math.abs(at - from) > 0.01) {
      ctx.strokeStyle = root.color;  // e.g. #1f6fd1
      ctx.beginPath();
      ctx.arc(center, center, radius, Math.min(from, at), Math.max(from, at));
      ctx.stroke();
    }
    ctx.lineWidth = Math.max(1.5, width / 18);
    ctx.strokeStyle = Theme.buttonActiveColor;  // #ffffff
    ctx.beginPath();
    ctx.moveTo(center + Math.cos(at) * radius * 0.3, center + Math.sin(at) * radius * 0.3);
    ctx.lineTo(center + Math.cos(at) * radius * 0.8, center + Math.sin(at) * radius * 0.8);
    ctx.stroke();
  }
}
```

### 9.4 Interaction

- **DragHandler** (vertical only): `onTranslationChanged: control.parameter = clamp(startValue - translation.y / dragRange)`
- **TapHandler** (double-tap): `control.reset()`

### 9.5 Label

```
Text (id: labelText)
  ├─ anchors.bottom: parent.bottom
  ├─ anchors.horizontalCenter: parent.horizontalCenter
  ├─ color: Theme.lightGray3 (#939393)
  ├─ font.pixelSize: Math.max(8, Math.min(13, Math.round(root.width * 0.22)))
  ├─ text: root.label
  └─ visible: root.label.length > 0
```

---

## 10. BoothFader (Booth/BoothFader.qml)

### 10.1 Properties

| Property | Default | Description |
|---|---|---|
| group | required | Mixxx control group |
| key | required | Mixxx control key |
| horizontal | false | Orientation |
| inverted | false | Top/left = start of range |
| color | Theme.lightGray2 (#b0b0b0) | Handle accent colour |
| centerMark | false | Show centre tick |

### 10.2 Geometry

- `readonly property real position: inverted ? 1 - control.parameter : control.parameter`
- `readonly property real handleLength: Math.max(14, Math.round((horizontal ? height : width) * 0.6))`
- `readonly property real travel: Math.max(1, (horizontal ? width : height) - handleLength)`
- `implicitHeight: horizontal ? 28 : 120`
- `implicitWidth: horizontal ? 120 : 28`

### 10.3 Visual Tree (paint order)

```
Item (root)
  ├─ Rectangle (id: track)
  │    ├─ anchors.centerIn: parent
  │    ├─ color: Theme.midGray3 (#626262)
  │    ├─ height: horizontal ? 4 : parent.height - handleLength / 2
  │    ├─ radius: 2
  │    └─ width: horizontal ? parent.width - handleLength / 2 : 4
  ├─ Rectangle (centre mark)
  │    ├─ anchors.centerIn: parent
  │    ├─ color: Theme.midGray3 (#626262)
  │    ├─ height: horizontal ? parent.height * 0.6 : 1
  │    ├─ visible: root.centerMark
  │    └─ width: horizontal ? 1 : parent.width * 0.6
  └─ Rectangle (id: handle)
       ├─ border.color: dragHandler.active ? Theme.buttonActiveColor (#ffffff) : Theme.lightGray3 (#939393)
       ├─ border.width: 1
       ├─ color: Theme.midGray2 (#676767)
       ├─ height: horizontal ? parent.height : handleLength * 0.55
       ├─ radius: 2
       ├─ width: horizontal ? handleLength * 0.55 : parent.width
       ├─ x: horizontal ? position * (root.width - width) : 0
       ├─ y: horizontal ? 0 : (1 - position) * (root.height - height)
       └─ Rectangle (accent line)
            ├─ anchors.centerIn: parent
            ├─ color: root.color
            ├─ height: horizontal ? parent.height - 6 : 2
            └─ width: horizontal ? 2 : parent.width - 6
```

### 10.4 Interaction

- **DragHandler**: `onTranslationChanged: setPosition(startPosition + moved)` where `moved = horizontal ? translation.x / (width - handle.width) : -translation.y / (height - handle.height)`
- **TapHandler** (double-tap): `control.reset()`

---

## 11. BoothBrowse (Booth/BoothBrowse.qml)

### 11.1 Root Properties

| Property | Type | Default |
|---|---|---|
| loadTarget | string | "" |
| acceptsController | bool | true |
| compact | bool | false |
| streaming | bool | false |

### 11.2 State Properties

| Property | Description |
|---|---|
| cover, title, artist, bpm, key, added | Filled by selected row cells |
| levelPath | Entry path whose children are listed (null = top level) |
| levelIndex | Model index of levelPath |
| showTracks | true = show tracks, false = show entries |
| activeSection | Top-level source index of shown tracks |
| activeLabel | Name of active playlist/crate/folder |
| activeIsDrive | true if active entry is a USB drive |
| cursor | Controller browse knob position (-1 = ALL TRACKS) |

### 11.3 Key Functions

```qml
function iconOf(index) { return sidebar?.data(index, Qt.UserRole + 1) ?? ""; }
function sectionLabel(icon, label) { /* maps icon to uppercase label */ }
function pathOf(index) { /* builds path array from index to root */ }
function indexOf(path) { /* resolves path to fresh model index */ }
function sectionIcon(path) { return iconOf(indexOf(path.slice(0,1))); }
function pathIsDrive(path) { return path.length === 2 && (sectionIcon(path) === "rekordbox" || sectionIcon(path) === "enginedj"); }
function refreshLevel() { /* updates levelIndex from levelPath */ }
function checkStorageAccess(path) { /* requests USB/storage permission */ }
function activate(path) { /* sets activeLabel/Section/IsDrive; sidebar.activate() */ }
function open(path) { /* navigates: shows tracks or lists entries */ }
function openEntry(row) { return open(levelPath.concat([row])); }
function openAllTracks() { activate(levelPath); refreshLevel(); showTracks = true; }
function back() { /* goes up one level or shows tracks */ }
function moveCursor(offset) { /* controller browse knob */ }
function activateCursor() { /* controller select */ }
```

### 11.4 Layout (implicit from source)

- Left: **BoothSourceTree** (sidebar sources)
- Middle: **Track list** (TableView with custom columns)
- Right: **Info panel** (cover art + metadata from selected row)

### 11.5 Track Row Interaction

- Tap to selects row
- Double-tap to loads into `loadTarget` deck (or next available)
- Long press to opens track menu

---

## 12. BoothSourceTree (Booth/BoothSourceTree.qml)

### 12.1 Columns (playlistColumns)

| Column | Role | Width | Delegate |
|---|---|---|---|
| Cover | Cover | 40 | Image (PreserveAspectCrop, margins 3) |
| Track | Title | fillSpan 3 | TextCell (infoKey: "title") |
| Artist | Artist | fillSpan 2 | TextCell (dim, infoKey: "artist") |
| BPM | BPM | 66 | TextCell (alignRight, infoKey: "bpm") |
| Key | Key | 58 | TextCell (alignRight, infoKey: "key") |
| Added | DateAdded | 96 (autoHideWidth 760) | TextCell (alignRight, dim, infoKey: "added") |
| Position | Position | 40 | TextCell (alignRight, dim) |

### 12.2 Sources (in order)

1. LibraryAllTrackSource (columns: defaultColumns + positionColumn after cover)
2. LibraryFeatureSource: Playlists (playlistColumns)
3. LibraryFeatureSource: Crates (defaultColumns)
4. LibraryFeatureSource: History (playlistColumns)
5. LibraryFeatureSource: Browse (defaultColumns)
6. LibraryFeatureSource: Rekordbox (playlistColumns)
7. LibraryFeatureSource: EngineDJ (playlistColumns)
8. LibraryFeatureSource: Recordings (defaultColumns)

### 12.3 RowCell (track row background)

```
Rectangle (id: rowCell)
  ├─ readonly property var caps: capabilities
  ├─ color: selected ? Theme.activeColor (#ff4fa8) : (row % 2 === 0 ? Theme.darkGray4 (#0d0e13) : Theme.darkGray2 (#0a0b10))
  ├─ implicitHeight: 40
  ├─ LibraryComponent.Track (anchors.fill: parent)
  │    ├─ tap.onDoubleTapped: select row + loadSelectedTrack
  │    └─ tap.onTapped: select row
  └─ Rectangle (divider)
       ├─ anchors.bottom: parent.bottom; anchors.left: parent.left; anchors.right: parent.right
       ├─ color: Theme.backgroundColor (#000000)
       └─ height: 1
```

### 12.4 TextCell

```
TextCell (extends RowCell)
  ├─ property bool alignRight: false
  ├─ property bool dim: false
  ├─ property string infoKey: ""
  ├─ Text { anchors.fill: parent; anchors.leftMargin: 10; anchors.rightMargin: 10; color: selected ? Theme.buttonActiveColor (#ffffff) : (dim ? Theme.lightGray3 (#939393) : Theme.textColor (#e8e8e8)); elide: Text.ElideRight; font.pixelSize: 15; horizontalAlignment: alignRight ? AlignRight : AlignLeft; text: display; verticalAlignment: AlignVCenter }
  └─ Binding { target: root.infoTarget; property: infoKey; value: display; when: selected && infoTarget && infoKey.length > 0 }
```

---

## 13. Android Host (+android/MainWindow.qml)

### 13.1 ApplicationWindow

| Property | Value |
|---|---|
| id | root |
| readonly isMobile | `Qt.platform.os === "android" || Qt.platform.os === "ios"` |
| readonly designWidth | 1792 |
| readonly designHeight | 1008 |
| bottomPadding | 0 |
| color | Theme.backgroundColor (#1e1e20 base, overridden by skin) |
| flags | Mobile: `Window | FramelessWindowHint | MaximizeUsingFullscreenGeometryHint | ExpandedClientAreaHint`; Desktop: `Window` |
| height | Mobile: `Screen.height`; Desktop: `designHeight` |
| leftPadding | 0 |
| menuBar | `content.item?.menuBar` |
| minimumHeight | Mobile: 0; Desktop: 300 |
| minimumWidth | Mobile: 0; Desktop: 680 |
| rightPadding | 0 |
| topPadding | 0 |
| visibility | Mobile: `Window.FullScreen`; Desktop: `Window.Windowed` |
| width | Mobile: `Screen.width`; Desktop: `designWidth` |

### 13.2 Quit Handling

- `property bool quitConfirmed: false`
- `onClosing:` mobile to show quitPopup
- `Shortcut (Back):` mobile + no overlay to quitPopup
- `function quit() { quitConfirmed = true; Qt.quit(); }`

### 13.3 Skin Loading

```qml
function skinSource(skin) {
    if (skin === "Classic" || MobileApp.skinsIn(Qt.resolvedUrl("Skins/")).indexOf(skin) < 0) {
        return Qt.resolvedUrl("MainWindow.qml");
    }
    return Qt.resolvedUrl("Skins/" + skin + "/MainWindow.qml");
}

function applySkinTheme(skinTheme) {
    // Saves base Theme defaults on first call, then restores them before applying new skinTheme
}

function loadSkin() {
    if (!Mixxx.Core.ready) return;
    content.setSource(skinSource(Mixxx.MobileApp.skin), { "applicationWindow": root });
}
```

### 13.4 Content Loader

```
Loader (id: content)
  ├─ source: root.skinSource(Mixxx.MobileApp.skin)
  ├─ onLoaded: root.applySkinTheme(content.item?.skinTheme)
  └─ Connections { target: Mixxx.MobileApp; onSkinChanged: root.loadSkin() }
```

---

## 14. Android Top Bar (+android/MainWindow.qml lines 243-455)

### 14.1 Toolbar Background

```
Rectangle (id: toolbar)
  ├─ color: Theme.toolbarBackgroundColor (#2e2e2e base, #000000 skin)
  ├─ height: 36
  ├─ radius: 1
  ├─ width: parent.width
  ├─ z: 20
  └─ CutoutInsets (toolbarInset) to target: toolbar
```

### 14.2 Left Section

- SugarLogo (anchors.left: toolbarInset.left + 10; verticalCenter; size: 26)
- StemsIndicator (Button, right of preferences, visible when pendingCount > 0)
- PreferencesButton (gear icon, 54×40, right margin toolbarInset.right + 6)

### 14.3 Centre Flickable (View Tabs)

```
Flickable (id: toolbarFlickable)
  ├─ anchors.left: logoText.right, leftMargin: 8
  ├─ anchors.right: stemsIndicator.active ? stemsIndicator.left : showPreferencesButton.left, rightMargin: 8
  ├─ anchors.top: parent.top; anchors.bottom: parent.bottom
  ├─ contentWidth: toolbarRow.implicitWidth
  ├─ flickableDirection: HorizontalFlick
  ├─ boundsBehavior: StopAtBounds
  ├─ clip: true
  └─ RowLayout (id: toolbarRow)
       ├─ Rectangle (tab bar background)
       │    ├─ Layout.preferredHeight: 40
       │    ├─ Layout.preferredWidth: 300 * root.shownTopBarViews / 4
       │    ├─ visible: shownTopBarViews > 0
       │    ├─ color: Theme.darkGray3 (#3f3f3f base, #14161c skin)
       │    ├─ radius: 4
       │    ├─ border.color: Theme.midGray3
       │    ├─ border.width: 1
       │    └─ RowLayout (margins 2, spacing 2)
       │         ├─ Skin.Button (DJ) { activeColor: accentColor; backgroundColor: viewMode===1?accentColor:transparent; normalColor: viewMode===1?white:lightGray3; text: "DJ"; font.bold; font.pixelSize: 12; onClicked: viewMode=1; visible: topBarItemShown("dj") }
       │         ├─ Skin.Button (WAVE) { ... backgroundColor: viewMode===3?accentColor:transparent; text: "WAVE"; onClicked: viewMode=3; visible: topBarItemShown("wave") }
       │         ├─ Skin.Button (LIB) { ... backgroundColor: viewMode===2?accentColor:transparent; text: "LIB"; onClicked: viewMode=2; visible: topBarItemShown("lib") }
       │         └─ Skin.Button (CTL) { ... backgroundColor: viewMode===0?accentColor:transparent; text: "CTL"; onClicked: viewMode=0; visible: topBarItemShown("ctl") }
       ├─ Skin.Button (LOAD 1) { visible: topBarItemShown("load1"); activeColor: libraryDeck1Background; backgroundColor: libraryDeck1Background; normalColor: white; text: "LOAD 1"; onClicked: openQuickLoad("[Channel1]"); Layout.preferredWidth: 62 }
       ├─ Skin.Button (LOAD 2) { visible: topBarItemShown("load2"); activeColor: libraryDeck2Background; backgroundColor: #e65100; normalColor: white; text: "LOAD 2"; onClicked: openQuickLoad("[Channel2]"); Layout.preferredWidth: 62 }
       ├─ Skin.Button (FX) { checkable; checked: showEffects; activeColor: #e040fb; backgroundColor: showEffects?#4a148c:darkGray3; normalColor: showEffects?activeColor:lightGray2; text: "FX"; onClicked: showEffects=!showEffects; visible: topBarItemShown("fx"); Layout.preferredWidth: 42 }
       ├─ Skin.Button (SMPL) { checkable; checked: showSamplers; activeColor: #FF6600; backgroundColor: showSamplers?#663300:darkGray3; normalColor: showSamplers?#FFB340:lightGray2; text: "SMPL"; onClicked: showSamplers=!showSamplers; visible: topBarItemShown("smpl"); Layout.preferredWidth: 54 }
       ├─ Skin.Button (XY) { checkable; checked: xyPadOverlay.visible; activeColor: #D85486; backgroundColor: xyPadOverlay.visible?#006064:darkGray3; normalColor: xyPadOverlay.visible?#D85486:lightGray2; text: "XY"; onClicked: xyPadOverlay.visible=!xyPadOverlay.visible; visible: topBarItemShown("xy"); Layout.preferredWidth: 54 }
       ├─ Skin.Button (FX ST) { checkable; checked: fxStationLoader.active; activeColor: #ff4081; backgroundColor: active?#5c1030:#222733; normalColor: active?#ff80ab:#b0bec5; text: "FX ST"; font.pixelSize: 10; onClicked: fxStationLoader.active=!fxStationLoader.active; Layout.preferredWidth: 64; Layout.preferredHeight: 30 }
       ├─ Skin.Button (SPLIT) { checkable; checked: headSplitControl.value>0; activeColor: libraryDeck2Background; backgroundColor: headSplit>0?#e65100:darkGray3; normalColor: headSplit>0?white:lightGray2; text: "SPLIT"; onClicked: toggle headSplit; visible: topBarItemShown("split"); Layout.preferredWidth: 52 }
       ├─ Skin.MobileRecordButton (id: recButton) { visible: topBarItemShown("rec"); Layout.preferredHeight: 40; Layout.preferredWidth: implicitWidth }
       ├─ Skin.Button (Edit) { checkable; text: "Edit"; visible: !isMobile; activeColor: white; Layout.preferredWidth: 42 }
       └─ Skin.Button (4D) { checkable; text: "4D"; visible: height>515 && topBarItemShown("4d"); activeColor: white; Layout.preferredWidth: 42 }
```

### 14.4 EffectRow & SamplerRow

```
Skin.EffectRow (id: effects) { visible: root.showEffects && !fxStationLoader.covering; width: parent.width }
Skin.SamplerRow (id: samplers) { visible: root.showSamplers; width: parent.width; samplerOffset: 0 }
```

---

## 15. View Mode Summary

| Mode | Booth (MainWindow.qml) | Android (+android/MainWindow.qml) |
|---|---|---|
| 0 | - | CTL (controller view) |
| 1 | WAVE (waveforms) | DJ (touch controls) |
| 2 | BROWSE (library) | LIB (library) |
| 3 | DJ (touch controls) | WAVE (waveforms) |
| 4 | XY (XY pad) | XY (overlay) |

**Note:** The view mode numbers differ between the two hosts. The Android host maps its top bar buttons to modes 0-3, while the Booth skin uses 1-4.

---

## 16. Controller Skin Controls (Both Hosts)

| Control | Group | Key | Booth Action | Android Action |
|---|---|---|---|---|
| Library toggle | [Skin] | mobile_library_toggle | `toggleLibraryView()` | `toggleLibraryView()` (with viewModeBeforeLibrary) |
| FX toggle | [Skin] | mobile_fx_toggle | `toggleBeatFx()` | `showEffects = !showEffects` |
| Loader deck 1 | [Skin] | mobile_loader_deck1 | `openQuickLoad("[Channel1]")` | `openQuickLoad("[Channel1]")` |
| Loader deck 2 | [Skin] | mobile_loader_deck2 | `openQuickLoad("[Channel2]")` | `openQuickLoad("[Channel2]")` |
| LoadSelectedTrack | [ChannelN] | LoadSelectedTrack | Opens browser if not visible | Opens quickLoadDrawer or browser |

---

## 17. Compact / Portrait Breakpoints

| Condition | Effect |
|---|---|
| `height < 520` | `compact = true` (Booth) |
| `width < 760` | Narrow tabs, smaller fonts (both) |
| `compact` (Booth) | Top bar 26px; deck panels 58px/36% height; DJ view 50% height; tab fonts 11px; knob sizes reduced |
| `compact` (Android) | Top bar 32px margin for FX Station; tab fonts 11px |
| `isMobile` (Android) | Fullscreen, frameless, zero padding, safe-area insets via CutoutInsets |

---

## 18. Colours Quick Reference (Skin-Overridden)

| Semantic Role | Hex |
|---|---|
| Background | #000000 |
| Toolbar / Panel BG | #000000 |
| Dark Surface | #0d0e13 (darkGray4) |
| Darker Surface | #14161c (darkGray3) |
| Border / Divider | #626262 (midGray3) |
| Text Primary | #e8e8e8 (textColor) |
| Text Muted | #939393 (lightGray3) |
| Accent (primary) | #ff4fa8 (accentColor) |
| Deck 1 Blue | #1f6fd1 (libraryDeck1Background) |
| Deck 1 Blue Pressed | #4aa3f5 |
| Deck 2 Orange | #e8650f (libraryDeck2Background) |
| Deck 2 Orange Pressed | #ff9a52 |
| Green (meter ok) | #85c85b |
| Yellow (meter hot) | #fca001 |
| Red / Warning | #7d3b3b (warningColor) |
| White | #ffffff |
| Knob Ring (default) | #1f6fd1 |
| Knob Track | #626262 |
| Knob Indicator | #ffffff |
| Fader Track | #626262 |
| Fader Handle BG | #676767 |
| Fader Handle Border (active) | #ffffff |
| Fader Handle Border (idle) | #939393 |
| CUE Active BG | #3a1a08 |
| CUE Active Border/Text | #7d3b3b |
| CUE Inactive BG | #0d0e13 |
| CUE Inactive Border | #676767 |
| CUE Inactive Text | #b0b0b0 |
| Tab Active Border/Text | #ff4fa8 |
| Tab Inactive Border | #626262 |
| Tab Inactive Text | #939393 |
| Tab Pressed BG | #14161c |
| Tab Active BG | transparent (Booth) / accentColor (Android) |
| Button Active BG | #ff4fa8 |
| Scroll Handle | #333333 / #7c5cff / #ff4fa8 |
| Key Match | #00e676 / #00e67633 |

---

## 19. Canvas/Shape Drawing Code Reference

### 19.1 BoothKnob Ring (verbatim from BoothKnob.qml:48-77)

```javascript
onPaint: {
    const ctx = getContext("2d");
    ctx.reset();
    const center = width / 2;
    const lineWidth = Math.max(2, width / 11);
    const radius = center - lineWidth;
    const start = Math.PI * 0.75;
    const end = Math.PI * 2.25;
    const at = start + root.clamp(root.value) * (end - start);
    const from = root.bipolar ? Math.PI * 1.5 : start;
    ctx.lineCap = "round";
    ctx.lineWidth = lineWidth;
    ctx.strokeStyle = Theme.midGray3;
    ctx.beginPath();
    ctx.arc(center, center, radius, start, end);
    ctx.stroke();
    if (Math.abs(at - from) > 0.01) {
        ctx.strokeStyle = root.color;
        ctx.beginPath();
        ctx.arc(center, center, radius, Math.min(from, at), Math.max(from, at));
        ctx.stroke();
    }
    ctx.lineWidth = Math.max(1.5, width / 18);
    ctx.strokeStyle = Theme.buttonActiveColor;
    ctx.beginPath();
    ctx.moveTo(center + Math.cos(at) * radius * 0.3, center + Math.sin(at) * radius * 0.3);
    ctx.lineTo(center + Math.cos(at) * radius * 0.8, center + Math.sin(at) * radius * 0.8);
    ctx.stroke();
}
```

### 19.2 SplitView Handle Dots (verbatim from +android/MainWindow.qml:496-509)

```qml
RowLayout {
    anchors.centerIn: parent
    Repeater {
        model: 3
        Rectangle {
            color: handleColor
            height: handleSize
            radius: handleSize
            width: handleSize
        }
    }
}
```

---

## 20. Interaction Summary per Control

| Control | Group/Key | Drag | Tap | Double-Tap | Long Press |
|---|---|---|---|---|---|
| BoothKnob | group/key | Vertical drag to parameter | - | Reset to default | - |
| BoothFader | group/key | Horizontal/vertical drag to parameter | - | Reset to default | - |
| BoothMixer CUE | [ChannelN]/pfl | - | Toggle PFL | - | - |
| BoothMixer Filter | [QuickEffectRack1_N]/super1 | Vertical drag to parameter | - | Reset | - |
| BoothMixer EQ/Trim | [ChannelN]/[EqualizerRack1]/pregain/parameter1-3 | Vertical drag to parameter | - | Reset | - |
| BoothMixer Fader | [ChannelN]/volume | Vertical drag to parameter | - | Reset | - |
| TopBar Tab | - | - | Switch view | - | - |
| LOAD 1/2 | - | - | Open browser for deck | - | - |
| SPLIT | [Master]/headSplit | - | Toggle split cue | - | - |
| FX ST | - | - | Toggle FX Station | - | - |
| XY | - | - | Toggle XY pad | - | - |
| BEAT FX | - | - | Toggle Beat FX panel | - | - |
| REC | [Master]/record | - | Start/stop recording (with confirm) | - | - |
| UTILITY | - | - | Open settings | - | - |
| Browser Row | - | - | Select | Load into deck | Track menu |
| Browser Sidebar | - | - | Activate source/entry | - | - |
| Controller Browse Knob | - | moveCursor(offset) | activateCursor() | - | - |

---

## 21. File Output

This specification is written to:
**/home/ubuntu/djsugar-next/docs/parity/booth-shell.md**

Line count: approximately 1,200 lines.
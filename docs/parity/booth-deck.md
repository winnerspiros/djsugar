# Booth Skin DECK Side - Visual & Behaviour Specification

This document specifies the DJ Sugar Booth skin DECK side for reimplementation in a Rust immediate-mode GPU painter. Every colour is resolved to hex with opacity. Geometry is expressed as formulas of parent width/height using the original Layout.preferred*, anchors, margins, spacing, Math.round/min/max exactly as written. Canvas/Shape drawing code is noted exactly (paths, angles). Controls list their Mixxx group/key, click/drag/long-press behaviour, and drag sensitivity.

## 1. Theme Colours (resolved)

| Token | Hex / RGBA | Notes |
|-------|------------|-------|
| accentColor | #3a60be | |
| backgroundColor | #1e1e20 | |
| blue | #01dcfc | |
| bpmSliderBarColor | #01dcfc | = blue |
| buttonActiveColor | #d9d9d9 | = white |
| buttonDisableColor | #747474 | = lightGray |
| buttonFontPixelSize | 10 | px |
| buttonNormalColor | #696969 | = midGray |
| crossfaderBarColor | #01dcfc | = blue |
| crossfaderOrientationColor | #747474 | = lightGray |
| darkGray | #0f0f0f | |
| darkGray2 | #2e2e2e | |
| darkGray3 | #3f3f3f | |
| darkGray4 | #202020 | |
| deckActiveColor | #d9d9d9 | = white |
| deckBackgroundColor | #0f0f0f | = darkGray |
| deckBeatjumpBackgroundColor | #626262 | = midGray3 |
| deckBeatjumpLabelColor | #3f3f3f | = darkGray3 |
| deckEmptyCoverArt | #3f3f3f | = darkGray3 |
| deckInfoBarBackgroundColor | #0e0e0e | |
| deckLineColor | #2e2e2e | = darkGray2 |
| deckLoopBackgroundColor | #626262 | = midGray3 |
| deckLoopLabelColor | #3f3f3f | = darkGray3 |
| deckTextColor | #b0b0b0 | = lightGray2 |
| effectColor | #fca001 | = yellow |
| effectUnitColor | #ea2a4e | = red |
| embeddedBackgroundColor | #a0000000 | 62.7% black |
| eqFxColor | #ea2a4e | = red |
| eqHighColor | #d9d9d9 | = white |
| eqLowColor | #d9d9d9 | = white |
| eqMidColor | #d9d9d9 | = white |
| fontFamily | Open Sans | |
| gainKnobColor | #01dcfc | = blue |
| green | #85c85b | |
| knobBackgroundColor | #262626 | |
| libraryDeck1Background | #1f6fd1 | Deck 1 cyan-blue |
| libraryDeck2Background | #e8650f | Deck 2 orange |
| lightGray | #747474 | |
| lightGray2 | #b0b0b0 | |
| lightGray3 | #939393 | |
| midGray | #696969 | |
| midGray2 | #676767 | |
| midGray3 | #626262 | |
| panelSplitterBackground | #1e1e20 | = backgroundColor |
| panelSplitterHandle | #696969 | = midGray |
| panelSplitterHandleActive | #b0b0b0 | = lightGray2 |
| pflActiveButtonColor | #01dcfc | = blue |
| red | #ea2a4e | |
| samplerColor | #01dcfc | = blue |
| sunkenBackgroundColor | #0c0c0c | |
| textColor | #b0b0b0 | = lightGray2 |
| textFontPixelSize | 14 | px |
| toolbarActiveColor | #d9d9d9 | = white |
| toolbarBackgroundColor | #2e2e2e | = darkGray2 |
| volumeSliderBarColor | #01dcfc | = blue |
| warningColor | #7d3b3b | |
| waveformBeatColor | #747474 | = lightGray |
| waveformCursorColor | #d9d9d9 | = white |
| waveformMarkerDefault | #ff7a01 | |
| waveformMarkerIntroOutroColor | #2c5c9a | |
| waveformMarkerLabel | rgba(255,255,255,0.8) | |
| waveformMarkerLoopColor | #00b400 | |
| waveformMarkerLoopColorDisabled | #ffffff | |
| waveformPostrollColor | #696969 | = midGray |
| waveformPrerollColor | #696969 | = midGray |
| white | #d9d9d9 | |
| yellow | #fca001 | |
| activeColor | #3a60be | = accentColor (used in BoothBeatFx) |

---

## 2. BoothPerformanceDeck.qml

Root: `Rectangle` (id: `root`)

### 2.1 Root Properties & Geometry
- Required: `group` (string), `deckNumber` (int)
- `mirrored` (bool, default false) - Deck 2 mirrored, pads on outer side
- `compact` (bool, default false) - Landscape phones
- `padMode` (string, default "hotcue") - "hotcue" | "beatjump" | "beatloop" | "stem" | "sampler"
- `deckColor`: `deckNumber === 1 ? #1f6fd1 : #e8650f`
- `loaded`: `trackLoadedControl.value > 0`
- `unit`: `Math.max(9, Math.min(height / 14, width / 34))` - base unit for all sizing
- Border: `color: #626262` (midGray3), `width: 1`
- Background: `color: #202020` (darkGray4)

### 2.2 Control Proxies (Mixxx)
| Id | Group | Key | UpdateInterval |
|----|-------|-----|----------------|
| trackLoadedControl | root.group | track_loaded | - |
| playControl | root.group | play | 100 ms |
| cueControl | root.group | cue_default | - |
| cueIndicatorControl | root.group | cue_indicator | - |
| syncControl | root.group | sync_enabled | - |
| syncLeaderControl | root.group | sync_leader | - |
| quantizeControl | root.group | quantize | - |
| keylockControl | root.group | keylock | - |
| slipControl | root.group | slip_enabled | - |
| bpmControl | root.group | bpm | - |
| rateRatioControl | root.group | rate_ratio | - |
| rateRangeControl | root.group | rateRange | - |
| loopEnabledControl | root.group | loop_enabled | - |
| loopSizeControl | root.group | beatloop_size | - |
| sampleRateControl | root.group | track_samplerate | - |
| loopActivate | root.group | beatloop_activate | - |
| reloopToggle | root.group | reloop_toggle | - |
| loopHalve | root.group | loop_halve | - |
| loopDouble | root.group | loop_double | - |
| loopIn | root.group | loop_in | - |
| loopOut | root.group | loop_out | - |
| jumpControl | root.group | (dynamic) | - |

### 2.3 Component: SmallButton
- Rectangle, `radius: 2`, `border.width: 1`
- Border colour: `on ? onColor : #676767` (midGray2)
- Fill: `pressed ? #626262 (midGray3) : (on ? alpha(onColor, 0.18) : #202020)`
- `onColor` default: `#7d3b3b` (warningColor)
- Label: `Text` anchored fill, `fontSizeMode: Fit`, `bold`, `pixelSize: round(unit * 1.05)`, min 6, color `on ? onColor : #b0b0b0`
- TapHandler: `gesturePolicy: WithinBounds`, signals `pressed`, `released`, `pressAndHold` (onLongPressed)

### 2.4 Component: Pad (hotcue / beatjump / beatloop)
- Rectangle, `border.width: 1`, `radius: 2`
- Border colour: `pressed ? #d9d9d9 : (lit ? litColor : #626262)`
- Fill: `pressed ? #626262 : #202020`
- `lit` logic:
  - hotcue: `hotcueStatus.value > 0`
  - beatloop: `loopEnabledControl.value > 0 && loopSizeControl.value === loopSizes[index]`
  - else: false
- `litColor`: hotcue to `hotcueColor` (parsed from RGB int), else `#1f6fd1`
- `hotcueColor`: from `hotcue_${number}_color` control; if < 0 to `#85c85b` (green)
- Label `Text`: centered vertically, left margin 3 (hotcue) or 0 (others), `pixelSize: round(unit * 1.05)`, color `lit ? #d9d9d9 : #939393`
  - hotcue: time `formatTime(hotcuePosition / sampleRate / 2)` if set
  - beatjump: `jumpLabels[sizeIndex]` from `[FINE, 1/8, 1/4, 1/2, 1, 2, 4, 8]`
  - beatloop: `loopLabels[index]` from `[1/4, 1/2, 1, 2, 4, 8, 16, 32]`
- Hotcue letter: `Rectangle` left margin 3, `size: min(height-6, unit*1.5)`, square, visible only in hotcue mode
  - Fill: `hotcueSet ? hotcueColor : transparent`
  - Inner `Text`: centered, `pixelSize: round(letter.height * 0.75)`, `bold`, char `A+index`, color `hotcueSet ? #000000 : #626262`
- Beatjump direction triangles (when padMode === beatjump): two `Skin.Triangle` (from Skin module), height `round(unit * 1.1)`, width `round(height * 0.8)`, color `#fca001` (warningColor), left triangle points left (backward), right points right (forward), margins `round(width * 0.12)`
- TapHandler `padTap`:
  - `gesturePolicy: WithinBounds`
  - `onLongPressed` (hotcue only, if set): opens `HotcuePopup` at tap position clamped to root
  - `onPressedChanged`:
    - hotcue: `hotcueActivate.value = pressed ? 1 : 0`
    - beatjump: on press, `back = point.x < width/2`, `jumpControl.key = beatjump_${jumpSizes[sizeIndex]}_${back?backward:forward}`, trigger
    - beatloop: on press, `beatloopToggle.trigger()`

### 2.5 Component: StemPad (padMode === "stem")
- Rectangle, `radius: 2`, `border.width: 1`
- `stemNumber = (padIndex % 4) + 1`, `isFxPad = padIndex >= 4`
- `stemLabels`: `[DRUMS, BASS, INST, VOCAL]`
- `stemColors`: `[#01dcfc (blue), #3a60be (accent), #fca001 (yellow), #3a60be (activeColor)]` - note activeColor = accentColor
- `padColor = stemColors[stemNumber-1]`
- `stemActive`: isFxPad ? `stemFxControl.value > 0` : `stemMuteControl.value === 0`
- Border: `pressed ? #d9d9d9 : (stemActive ? padColor : #626262)`
- Fill: `pressed ? #626262 : (stemActive ? alpha(padColor, 0.22) : #202020)`
- Label `Text`: centered, `pixelSize: round(unit * 0.9)`, `bold`, color `stemActive ? #ffffff : #939393`, text: isFxPad ? `${label} FX` : label
- TapHandler: onTapped toggles `stemFxControl` (FX) or `stemMuteControl` (MUTE)

### 2.6 Main Layout: RowLayout (anchors.fill, margins 4, layoutDirection mirrored? RTL:LTR, spacing 4)

#### 2.6.1 Left Column: Pads + Mode Button (ColumnLayout)
- `Layout.fillHeight: true`, `fillWidth: true`, `minWidth: unit*10`, `prefWidth: unit*16`, `spacing: 3`

##### GridLayout (visible: padMode not sampler/stem)
- `fillHeight/Width`, `columns: 2`, `colSpacing: 3`, `rowSpacing: 3`
- Repeater model 8 to `Pad` (fillHeight/Width)

##### GridLayout (visible: padMode === "stem")
- Same grid, Repeater model 8 to `StemPad` (group: root.group, padIndex: index)

##### GridLayout (visible: padMode === "sampler")
- Same grid, Repeater model 8 to `SamplerPad` (mirrored: root.mirrored)

##### Mode Button (Rectangle)
- `fillWidth`, `prefHeight: round(unit * 1.8)`, `border.color: #626262`, `border.width: 1`
- Fill: `pressed ? #626262 : #202020`
- Label `Text`: centered, `pixelSize: round(unit * 1.0)`, color `#b0b0b0`, text cycles through modes with arrow suffix
- TapHandler `modeTap`: onTapped cycles `padMode` through `[hotcue, beatjump, beatloop, stem, sampler]`

#### 2.6.2 Loop Column (ColumnLayout)
- `readonly looping = loopEnabledControl.value > 0`
- `fillHeight: true`, `fillWidth: false`, `minWidth: round(unit*5.4)`, `prefWidth: round(unit*5.4)`, `spacing: 3`
- Label "LOOP": `fillWidth`, color `looping ? #fca001 : #939393`, `bold`, `letterSpacing: 1`, `pixelSize: round(unit * 0.95)`, centered
- Loop Size Button (Rectangle):
  - `fillWidth`, `prefHeight: round(unit * 3.8)`, `radius: 3`
  - Border: `looping ? #ff6a13 : #626262`, width `looping ? 2 : 1`
  - Fill: `pressed ? #5a2a08 : (looping ? #fca001 : #202020)`
  - Column centered, spacing 0:
    - Size text: `pixelSize: round(unit * 1.8)`, `bold`, color `looping ? #000000 : #e8650f` (libDeck2Bg), text: `size >= 1 ? round(size) : "1/" + round(1/size)`
    - Subtext: `pixelSize: round(unit * 0.8)`, `bold`, color `looping ? #000000 : #939393`, text: `looping ? "EXIT" : "BEATS"`
  - TapHandler `loopTap`: onTapped to `looping ? reloopToggle.trigger() : loopActivate.trigger()`
- RowLayout (1/2 / x2): `prefHeight: round(unit * 2.4)`, spacing 3, two `SmallButton` (label "1/2", "x2"), onPressed trigger `loopHalve` / `loopDouble`
- RowLayout (IN / OUT): `prefHeight: round(unit * 2.4)`, spacing 3, two `SmallButton` (label "IN", "OUT"), `on` bound to `loopIn/Out.value > 0`, onPressed=1, onReleased=0
- Spacer `Item fillHeight: true`
- GRID Button (`SmallButton`): `fillWidth`, `prefHeight: round(unit * 2.2)`, label "GRID", onPressed opens `BeatgridPopup` if loaded
- SLIP Button (`SmallButton`): `fillWidth`, `prefHeight: round(unit * 2.2)`, label "SLIP", `on: slipControl.value > 0`, onPressed toggles

#### 2.6.3 Transport Column: SYNC, CUE, PLAY (ColumnLayout)
- `readonly buttonSize = min(round(unit * 4.2), (height - 12) / 3.4)`
- `fillHeight: true`, `prefWidth: buttonSize`, `spacing: 4`
- SYNC Button (`SmallButton`): `fillWidth`, `prefHeight: round(unit * 2.2)`, label `syncLeader ? "MASTER" : "SYNC"`, `on: sync || syncLeader`, `onColor: syncLeader ? #fca001 : #1f6fd1`, onPressAndHold toggles leader, onPressed toggles sync
- Spacer `Item fillHeight: true`
- CUE Button (Rectangle):
  - `prefHeight/Width: buttonSize`, `radius: width/2` (circle), `border.width: 2`
  - Border color: `cueIndicator > 0 ? #ff9a1f : #8a5a20`
  - Fill: `pressed ? #3a2408 : #202020`
  - Label "CUE": centered, `pixelSize: round(height * 0.28)`, `bold`, color `cueIndicator > 0 ? #ff9a1f : #c08040`
  - TapHandler `cueTap`: `gesturePolicy: WithinBounds`, onPressedChanged to `cueControl.value = pressed ? 1 : 0`
- PLAY Button (Rectangle):
  - Same geometry as CUE
  - Border: `playing ? #85c85b : #2a6a2a`, width 2
  - Fill: `pressed ? #0c2a0c : #202020`
  - Content: when playing to Row centered, two vertical bars (Rectangles) `color: #85c85b`, `height: round(height * 0.34)`, `width: round(barHeight * 0.3)`, spacing `round(barHeight * 0.3)`; when stopped to `Skin.Triangle` centered (horizontalCenterOffset `round(width * 0.1)`), `color: #85c85b`, `height: round(height * 0.36)`, `width: round(height * 0.9)`, points right
  - TapHandler `playTap`: onPressedChanged to if pressed `playControl.value = playing ? 0 : 1`

#### 2.6.4 Jog Area (Item)
- `readonly size = min(width, height)`
- `fillHeight/Width`, `minWidth: unit*8`, `prefWidth: unit*14`
- Platter (Rectangle):
  - Centered, `border.color: #676767` (midGray2), `border.width: max(2, round(size * 0.04))`, `color: #1e1e20` (backgroundColor), `height/width: size`, `radius: width/2`
  - Inner ring: Rectangle `anchors.fill margins: round(size * 0.1)`, `border.color: #3f3f3f` (darkGray3), `border.width: 1`, `transparent`, `radius: width/2`
  - `DeckComponent.TouchSpinny` (anchors.fill, group: root.group, indicatorVisible: loaded)
    - Indicator: Rectangle top center, `topMargin: 2`, `color: deckColor`, `height: round(size * 0.12)`, `width: max(3, round(size * 0.035))`, `radius: width/2`
  - Center Column:
    - Row: BPM whole `pixelSize: round(size * 0.17)`, color `syncLeader ? #fca001 : #d9d9d9`, text `loaded ? floor(bpm) : "--"`
    - BPM fraction: `pixelSize: round(size * 0.09)`, same color, text `loaded ? "." + pad(floor((bpm%1)*100),2) : ".--"`
    - Rate text: `pixelSize: round(size * 0.075)`, color `#939393`, text `(rateRatio-1)*100` with sign + " %  +-" + `round(rateRange*100)`
- Bottom Column (Quantize / MT):
  - Anchored bottom, left if mirrored else right, spacing 3
  - Q Button (`SmallButton`): `height: round(unit*2)`, `width: round(unit*2.6)`, label "Q", `on: quantize>0`, onPressed toggles
  - MT Button (`SmallButton`): same, label "MT", `on: keylock>0`, onPressed toggles

#### 2.6.5 Tempo Fader (BoothFader)
- `fillHeight: true`, `prefWidth: round(unit * 2.6)`, `centerMark: true`, `color: deckColor`, `group: root.group`, `key: "rate"`, `inverted: true`

---

## 3. BoothDeckPanel.qml

Root: `Rectangle` (id: `root`)

### 3.1 Root Properties
- Required: `group` (string), `deckNumber` (int)
- `slim` (bool, default false) - only title bar + overview
- `deckPlayer = Mixxx.PlayerManager.getPlayer(group)`
- `currentTrack = deckPlayer?.currentTrack`
- `loaded = trackLoadedControl.value > 0`
- `playing = playControl.value > 0`
- `leader = syncLeaderControl.value > 0`
- `showElapsed` (bool, default false)
- `seconds`: if showElapsed to position, else duration - position
- `unit = Math.max(9, Math.min(height / 11, width / 30))`
- Background: `Theme.backgroundColor` (#1e1e20)
- Border: `color: playing ? #b0b0b0 : #626262`, `width: 1`

### 3.2 Control Proxies
| Id | Group | Key | UpdateInterval |
|----|-------|-----|----------------|
| trackLoadedControl | root.group | track_loaded | - |
| playControl | root.group | play | - |
| durationControl | root.group | duration | - |
| positionControl | root.group | playposition | 100 ms |
| bpmControl | root.group | bpm | - |
| rateControl | root.group | rate | - |
| rateRangeControl | root.group | rateRange | - |
| syncControl | root.group | sync_enabled | - |
| syncLeaderControl | root.group | sync_leader | - |
| keylockControl | root.group | keylock | - |

### 3.3 Component: Caption (Text)
- `color: #939393` (lightGray3)
- `font.pixelSize: round(unit * 0.95)`

### 3.4 Component: Chip (Rectangle)
- Required: `label` (string), `on` (bool), `onColor` (default `#7d3b3b`)
- `border.color: on ? onColor : #939393`, `border.width: 1`
- `color: on ? alpha(onColor, 0.18) : transparent`
- `height: round(unit * 1.8)`, `width: round(unit * 6.6)`
- Label `Text`: fill, color `on ? onColor : #b0b0b0`, `fontSizeMode: HorizontalFit`, `pixelSize: round(unit * 0.95)`, min 7, centered
- MouseArea `anchors.fill margins: -3`: onClicked to `clicked()`, onPressAndHold to `pressAndHold()`

### 3.5 Title Bar (Rectangle)
- Anchors: left/right/top margins 1, `color: #3f3f3f` (darkGray3), `height: round(unit * 2.3)`, `clip: true`
- YouTube download progress bar (visible when `ytDeckProgress >= 0`): left anchored, height full, `width: parent.width * clamp(ytDeckProgress, 0.04, 1.0)`, color `#2ae63380` (140 ms OutCubic animation)
- Deck accent line: left/right/top anchored, `height: 2`, `color: deckNumber===1 ? #1f6fd1 : #e8650f`
- Deck Label (Column): left margin 6, verticalCenter, spacing `-round(unit * 0.2)`
  - "DECK": `color: #939393`, `pixelSize: round(unit * 0.62)`, centered
  - Deck number: `color: playing ? #fca001 : #d9d9d9`, `bold`, `pixelSize: round(unit * 1.0)`, text `playing ? "((N))" : "N"`
- Track Title (Text): left of deckLabel.right + 10, right of keyLabel.left - 8, verticalCenter, elideRight, `pixelSize: round(unit * 1.45)`
  - If downloading: `#ff99ca`, text `Downloading XX% · title`
  - Else if loaded: `#d9d9d9`, text `note title`
  - Else empty
- Key Label (Text): right margin 8, verticalCenter, `color: #d9d9d9`, `pixelSize: round(unit * 1.3)`, text `loaded ? keyText : ""`
- Download gradient bar (bottom of titleBar, visible when downloading): height 3, gradient horizontal `#3a60be` to `#ff99ca` to `#01dcfc`, animated width

### 3.6 Info Area (Item, visible: !slim)
- Anchors: left/right margins 6, top `titleBar.bottom + round(unit * 0.5)`, `height: round(unit * 4.4)`
- Chips Column (left, verticalCenter, spacing `round(unit * 0.5)`):
  - MASTER TEMPO Chip: `on: keylock>0`, onClicked toggles keylock
  - SYNC Chip: `on: sync>0`, `onColor: #1f6fd1`, onClicked toggles sync, onPressAndHold toggles leader
- Time Area (Item): bottom anchored, left `chips.right + round(unit*0.8)`, right `tempo.left - 6`, top anchored
  - Row (top): three Captions: "• REMAIN" (color `showElapsed ? #626262 : #d8dadd`), "/" (default), "TIME" (color `showElapsed ? #d8dadd : #626262`)
  - Row (bottom, id `timeDigits`): scaled to fit (`scale: min(1, width/implicitWidth)`, transformOrigin BottomLeft)
    - Minutes:Seconds `Text`: `pixelSize: round(unit * 3.4)`, color `loaded ? #ffffff : #626262`, text `MM:SS`
    - Milliseconds `Text`: baseline aligned, `pixelSize: round(unit * 1.7)`, same color, text `.SSS`
  - MouseArea fill: onClicked toggles `showElapsed`
- Tempo Area (Item): bottom anchored, right `bpmBox.left - round(unit*0.8)`, width `round(unit * 6.2)`
  - Caption "TEMPO" top left
  - Range Box (Rectangle top right): `color: #e8650f`, `height: rangeText.implicitHeight + 2`, `width: rangeText.implicitWidth + 8`
    - Text: `pixelSize: round(unit * 0.95)`, `color: #d9d9d9`, text `round(rateRange*100)%`
  - Pitch Text (bottom right): `pixelSize: round(unit * 1.45)`, color `loaded ? #d8dadd : #626262`, text `(rate*rateRange*100)` with sign, 2 decimals + "%"
- BPM Box (Rectangle): bottom/right anchored, top anchored, `border.width: 1`, `border.color: leader ? #fca001 : #b0b0b0`, `color: transparent`, `width: max(round(unit*6.2), bpmCaption.width + masterText.width + 12)`
  - Top Row: Caption "BPM" (leftPadding 4, rightPadding 4, color `#b0b0b0`), if leader to Rectangle `color: #fca001`, height = caption height, width = masterText.width+8, Text "MASTER" centered, `pixelSize: round(unit * 0.95)`, `color: #1e1e20`
  - Bottom Row: right margin 5
    - Whole BPM: `pixelSize: round(unit * 2.6)`, color `leader ? #ff6a13 : (loaded ? #ffffff : #626262)`, text `loaded ? floor(bpm) : "--"`
    - Fraction: baseline aligned, `pixelSize: round(unit * 1.5)`, same color, text `loaded ? "." + floor((bpm%1)*10) : ".-"`
  - MouseArea fill: onClicked opens `BeatgridPopup`
  - `DeckComponent.BeatgridPopup` (group: root.group)

### 3.7 Waveform Overview (Skin.WaveformOverview)
- Anchors: bottom margin 3, left/right margins 4, top `slim ? titleBar.bottom : info.bottom` + 4
- Group: root.group

---

## 4. SamplerPad.qml

Root: `Rectangle` (id: `padItem`)

### 4.1 Properties
- Required: `index` (0-7), `mirrored` (bool)
- `samplerGroup = "[Sampler" + (index + 1 + (mirrored ? 8 : 0)) + "]"`
- Control Proxies: `loadedControl` (track_loaded), `playControl` (play), `cueControl` (cue_gotoandplay)
- `lit = loadedControl.value > 0`
- `playing = playControl.value > 0`

### 4.2 Visual
- `border.width: playing ? 2 : 1`
- Border color: `pressed ? #d9d9d9 : (playing ? #7d3b3b : (lit ? #e8650f : #626262))`
- Fill: `pressed ? #626262 : (playing ? alpha(#7d3b3b, 0.3) : (lit ? alpha(#e8650f, 0.18) : #202020))`
- `radius: 2`
- Label `Text`: centered, `bold`, `pixelSize: round(height * 0.28)`, color `playing ? #d9d9d9 : (lit ? #e8650f : #626262)`, text `samplerGroup.replace("[Sampler","S").replace("]","")` to "S1".."S16"

### 4.3 Behaviour
- TapHandler `padTap`: `gesturePolicy: WithinBounds`
- `onPressedChanged: cueControl.value = pressed ? 1 : 0` (hold repeats from cue)

---

## 5. BoothBeatFx.qml

Root: `Rectangle` (id: `root`)

### 5.1 Properties
- `unitGroup = "[EffectRack1_EffectUnit1]"`
- `focusedSlot = clamp(focusedControl.value, 1, 3)`
- `slotGroup = `[EffectRack1_EffectUnit1_Effect${focusedSlot}]``
- `accent = #3a60be` (activeColor)
- `compact` (bool, default false)
- Background: `#1e1e20`, border `#676767` (midGray2), width 1

### 5.2 Component: SmallLabel (Text)
- `color: #939393`, `pixelSize: compact ? 9 : 10`, `AlignHCenter`, `Layout.fillWidth`

### 5.3 Component: ToggleBox (Rectangle)
- Required: `group`, `key`, `label`
- `onColor` default root.accent
- `Layout.fillWidth`, `prefHeight: compact ? 24 : 30`
- Border: `control.value>0 ? onColor : #626262`, width 1
- Fill: `control.value>0 ? onColor : #1e1e20`
- Label `Text`: centered, `pixelSize: compact ? 11 : 13`, color `control.value>0 ? #d9d9d9 : #b0b0b0`
- MouseArea fill: onClicked toggles control

### 5.4 Control Proxies
| Id | Group | Key |
|----|-------|-----|
| focusedControl | unitGroup | focused_effect |
| mixControl | unitGroup | mix |
| metaControl | slotGroup | meta |
| enabledControl | slotGroup | enabled |
| nextEffectControl | slotGroup | next_effect |
| prevEffectControl | slotGroup | prev_effect |

### 5.5 Layout: ColumnLayout (anchors margins compact?5:8, top header.bottom, spacing compact?4:6)

#### Header (Rectangle)
- Anchors left/right/top margins 1, `color: #626262`, `height: compact?20:26`
- Text "BEAT FX" centered, `pixelSize: compact?11:13`, color `#b0b0b0`

#### Row 1: Effect Select (RowLayout, prefHeight compact?30:40, spacing 4)
- Prev Button (Rectangle): `fillHeight`, `prefWidth: 24`, `color: pressed ? #3f3f3f : #202020`, `radius: 3`
  - `Skin.Triangle` centered, height 11, width 8, pointsRight false
  - MouseArea: onClicked `prevEffectControl.value = 1; = 0`
- Effect Name Box (Rectangle): `fillHeight/Width`, `border #626262`, width 1, `color: #202020`
  - Text: fill margins 3, `color: #d9d9d9`, `elideRight`, `HorizontalFit`, `pixelSize: compact?14:17`, centered, text `slot.loaded ? slot.effectName : "-"`
- Next Button: same as prev but `pointsRight: true`, onClicked `nextEffectControl.value = 1; = 0`

#### Row 2: Slot Indicators (RowLayout, spacing 4)
- Repeater model 3 (slots 1-3):
  - Rectangle `slotBox`: `fillWidth`, `prefHeight: compact?20:24`
  - Border: `focused ? #d8dadd : #626262`, width 1
  - Fill: `slotEnabled>0 ? #123a5c : #1e1e20`
  - Text: centered, `pixelSize: 11`, color `focused ? #d9d9d9 : #939393`, `bold: focused`, text `index+1`
  - MouseArea: onClicked `focusedControl.value = index+1`

#### Row 3: Mix / Meta Sliders
- SmallLabel: "LEVEL/DEPTH XX%"
- Slider: `fillWidth`, `prefHeight: compact?22:32`, 0-1, value=mixControl, onMoved sets value
- SmallLabel: "EFFECT AMOUNT XX%"
- Slider: same, value=metaControl

#### Row 4: Channel Select (RowLayout, spacing 4)
- Three ToggleBoxes: CH 1, CH 2, MASTER (group unitGroup, keys `group_[Channel1]_enable`, etc.)

#### Spacer (Item fillHeight)

#### ON/OFF Button (Rectangle)
- `fillWidth`, `prefHeight: compact?34:44`
- Border: `enabled>0 ? #ff6a13 : #626262`, width 1
- Fill: `enabled>0 ? #fca001 : #1e1e20`
- Text: centered, `bold`, `pixelSize: compact?13:15`, color `enabled>0 ? #1e1e20 : #b0b0b0`, text "ON / OFF"
- MouseArea: onClicked toggles enabledControl

---

## 6. BoothKnob.qml

Root: `Item` (id: `root`)

### 6.1 Properties
- Required: `group`, `key`
- `label` (string, default "")
- `bipolar` (bool, default true) - ring lights from middle (EQ, trim) or from start
- `color` (default `#1f6fd1` = libraryDeck1Background)
- `dragRange` (default 160) - pixels for full 0-1 turn
- `value = control.parameter` (readonly)
- `ringSize = max(12, min(width, height - (labelVisible ? labelHeight+1 : 0)))`
- `implicitHeight: 40 + (labelVisible ? labelHeight+1 : 0)`, `implicitWidth: 40`

### 6.2 Control Proxy
- `control` (group, key)

### 6.3 Canvas (id: `ring`)
- Anchors: horizontalCenter, top, `height/width: ringSize`
- `onPaint`:
  - `ctx.reset()`
  - `center = w/2`, `lineWidth = max(2, w/11)`, `radius = center - lineWidth`
  - `start = PI * 0.75` (bottom-left), `end = PI * 2.25` (bottom-right)
  - `at = start + clamp(value) * (end - start)`
  - `from = bipolar ? PI * 1.5 (top) : start`
  - `lineCap = "round"`, `lineWidth = lineWidth`
  - Background arc: `strokeStyle = #626262`, `arc(center, center, radius, start, end)`, stroke
  - Value arc: if `|at - from| > 0.01`, `strokeStyle = root.color`, `arc(center, center, radius, min(from,at), max(from,at))`, stroke
  - Indicator tick: `lineWidth = max(1.5, w/18)`, `strokeStyle = #d9d9d9`
    - `moveTo(center + cos(at)*radius*0.3, center + sin(at)*radius*0.3)`
    - `lineTo(center + cos(at)*radius*0.8, center + sin(at)*radius*0.8)`, stroke

### 6.4 Interaction
- DragHandler (target null, xAxis disabled):
  - `onActiveChanged`: if active `startValue = control.parameter`
  - `onTranslationChanged`: if active `control.parameter = clamp(startValue - translation.y / dragRange)`
- TapHandler: `onDoubleTapped: control.reset()`

### 6.5 Label (Text)
- Anchors bottom, horizontalCenter
- `color: #939393`, `pixelSize: max(8, min(13, round(width * 0.22)))`, text = label, visible if label.length > 0

---

## 7. BoothFader.qml

Root: `Item` (id: `root`)

### 7.1 Properties
- Required: `group`, `key`
- `horizontal` (bool, default false)
- `inverted` (bool, default false) - top/left is start of range
- `color` (default `#b0b0b0` = lightGray2)
- `centerMark` (bool, default false)
- `position = inverted ? 1 - control.parameter : control.parameter` (readonly)
- `handleLength = max(14, round((horizontal?height:width) * 0.6))`
- `travel = max(1, (horizontal?width:height) - handleLength)`
- `implicitHeight: horizontal ? 28 : 120`, `implicitWidth: horizontal ? 120 : 28`

### 7.2 Control Proxy
- `control` (group, key)

### 7.3 Visual
- Track (Rectangle): `anchors.centerIn`, `color: #626262`, `radius: 2`
  - `height: horizontal ? 4 : parent.height - handleLength/2`
  - `width: horizontal ? parent.width - handleLength/2 : 4`
- Center Mark (Rectangle, visible if centerMark): `anchors.centerIn`, `color: #626262`
  - `height: horizontal ? parent.height*0.6 : 1`, `width: horizontal ? 1 : parent.width*0.6`
- Handle (Rectangle):
  - `border.color: dragHandler.active ? #d9d9d9 : #939393`, `border.width: 1`
  - `color: #676767` (midGray2)
  - `height: horizontal ? parent.height : handleLength*0.55`, `width: horizontal ? handleLength*0.55 : parent.width`, `radius: 2`
  - `x: horizontal ? position*(width-handle.width) : 0`
  - `y: horizontal ? 0 : (1-position)*(height-handle.height)`
  - Inner indicator (Rectangle centered): `color: root.color`, `height: horizontal ? parent.height-6 : 2`, `width: horizontal ? 2 : parent.width-6`

### 7.4 Interaction
- DragHandler:
  - `onActiveChanged`: if active `startPosition = root.position`
  - `onTranslationChanged`: if active
    - `moved = horizontal ? translation.x/(width-handle.width) : -translation.y/(height-handle.height)`
    - `setPosition(startPosition + moved)`
- `setPosition(pos)`: `clamped = clamp(pos)`, `control.parameter = inverted ? 1-clamped : clamped`
- TapHandler: `onDoubleTapped: control.reset()`

---

## 8. State Summary per Component

| Component | States | Visual Changes |
|-----------|--------|----------------|
| BoothPerformanceDeck root | loaded, playing, mirrored, compact, padMode | Border color (playingto#b0b0b0), layoutDirection, pad grid visibility, unit scaling |
| Pad | lit, pressed, padMode (hotcue/beatjump/beatloop) | Border color (littolitColor/pressedto#d9d9d9), label content, letter visible, triangles visible |
| StemPad | stemActive, pressed, isFxPad | Border/fill from padColor, label text (FX suffix) |
| SmallButton | on, pressed | Border/fill from onColor, label color |
| Loop Size Button | looping, pressed | Border color/width, fill, label text (size/EXIT/BEATS), subtext color |
| SYNC Button | sync, syncLeader | Label (SYNC/MASTER), onColor, on state |
| CUE Button | cueIndicator, pressed | Border color, fill, label color |
| PLAY Button | playing, pressed | Border color, fill, pause bars vs play triangle |
| Platter | loaded, playing | Spinny indicator visible, BPM text color (leaderto#fca001) |
| BoothDeckPanel root | loaded, playing, leader, slim, showElapsed | Border color, time display, info area visibility, BPM box border |
| Chip | on, pressed | Border/fill from onColor, label color |
| Title Bar | downloading | Progress bar, gradient bar, title text |
| Time Digits | loaded | Color (#ffffff vs #626262) |
| BPM Box | leader, loaded | Border color, whole/fraction color, MASTER badge |
| SamplerPad | lit, playing, pressed | Border width/color, fill alpha, label color |
| BoothBeatFx ToggleBox | on, pressed | Border/fill from onColor, label color |
| Slot Box | focused, enabled | Border color, fill, text bold/color |
| ON/OFF Button | enabled | Border/fill/text color |
| BoothKnob | value, bipolar, pressed (drag) | Value arc angle, indicator tick angle |
| BoothFader | position, inverted, centerMark, dragged | Handle position, track length, center mark visibility |

---

## 9. Control Bindings Summary (Mixxx Group/Key)

### BoothPerformanceDeck
| Control | Group | Key |
|---------|-------|-----|
| track_loaded | [ChannelN] | track_loaded |
| play | [ChannelN] | play |
| cue_default | [ChannelN] | cue_default |
| cue_indicator | [ChannelN] | cue_indicator |
| sync_enabled | [ChannelN] | sync_enabled |
| sync_leader | [ChannelN] | sync_leader |
| quantize | [ChannelN] | quantize |
| keylock | [ChannelN] | keylock |
| slip_enabled | [ChannelN] | slip_enabled |
| bpm | [ChannelN] | bpm |
| rate_ratio | [ChannelN] | rate_ratio |
| rateRange | [ChannelN] | rateRange |
| loop_enabled | [ChannelN] | loop_enabled |
| beatloop_size | [ChannelN] | beatloop_size |
| track_samplerate | [ChannelN] | track_samplerate |
| hotcue_X_status | [ChannelN] | hotcue_X_status |
| hotcue_X_color | [ChannelN] | hotcue_X_color |
| hotcue_X_position | [ChannelN] | hotcue_X_position |
| hotcue_X_activate | [ChannelN] | hotcue_X_activate |
| beatloop_Y_toggle | [ChannelN] | beatloop_Y_toggle |
| beatjump_Y_forward/backward | [ChannelN] | beatjump_Y_forward/backward |
| beatloop_activate | [ChannelN] | beatloop_activate |
| reloop_toggle | [ChannelN] | reloop_toggle |
| loop_halve | [ChannelN] | loop_halve |
| loop_double | [ChannelN] | loop_double |
| loop_in | [ChannelN] | loop_in |
| loop_out | [ChannelN] | loop_out |
| rate | [ChannelN] | rate |

### BoothDeckPanel
| Control | Group | Key |
|---------|-------|-----|
| track_loaded | [ChannelN] | track_loaded |
| play | [ChannelN] | play |
| duration | [ChannelN] | duration |
| playposition | [ChannelN] | playposition |
| bpm | [ChannelN] | bpm |
| rate | [ChannelN] | rate |
| rateRange | [ChannelN] | rateRange |
| sync_enabled | [ChannelN] | sync_enabled |
| sync_leader | [ChannelN] | sync_leader |
| keylock | [ChannelN] | keylock |

### SamplerPad
| Control | Group | Key |
|---------|-------|-----|
| track_loaded | [SamplerX] | track_loaded |
| play | [SamplerX] | play |
| cue_gotoandplay | [SamplerX] | cue_gotoandplay |

### BoothBeatFx
| Control | Group | Key |
|---------|-------|-----|
| focused_effect | [EffectRack1_EffectUnit1] | focused_effect |
| mix | [EffectRack1_EffectUnit1] | mix |
| meta | [EffectRack1_EffectUnit1_EffectX] | meta |
| enabled | [EffectRack1_EffectUnit1_EffectX] | enabled |
| next_effect | [EffectRack1_EffectUnit1_EffectX] | next_effect |
| prev_effect | [EffectRack1_EffectUnit1_EffectX] | prev_effect |
| group_[Channel1]_enable | [EffectRack1_EffectUnit1] | group_[Channel1]_enable |
| group_[Channel2]_enable | [EffectRack1_EffectUnit1] | group_[Channel2]_enable |
| group_[Master]_enable | [EffectRack1_EffectUnit1] | group_[Master]_enable |

### BoothKnob
| Control | Group | Key |
|---------|-------|-----|
| (param) | [ChannelN] or [EqualizerRack1_[ChannelN]] | per key prop |

### BoothFader
| Control | Group | Key |
|---------|-------|-----|
| (param) | [ChannelN] or [Master] | per key prop (rate, volume, crossfader) |

---

## 10. Canvas / Shape Drawing Reference

### BoothKnob Ring (Canvas 2D)
```
center = w/2
lineWidth = max(2, w/11)
radius = center - lineWidth
start = PI * 0.75          // -135 deg (bottom-left)
end   = PI * 2.25          // +135 deg (bottom-right)
at    = start + clamp(value) * (end - start)
from  = bipolar ? PI * 1.5 : start   // -90 deg (top) if bipolar

// Background track
strokeStyle = #626262
arc(center, center, radius, start, end)
stroke()

// Value arc
if |at - from| > 0.01:
  strokeStyle = knob.color
  arc(center, center, radius, min(from,at), max(from,at))
  stroke()

// Indicator tick
lineWidth = max(1.5, w/18)
strokeStyle = #d9d9d9
moveTo(center + cos(at)*radius*0.3, center + sin(at)*radius*0.3)
lineTo(center + cos(at)*radius*0.8, center + sin(at)*radius*0.8)
stroke()
```

### BoothPerformanceDeck Platter (TouchSpinny indicator)
- Rectangle at top center of platter: `topMargin: 2`, `color: deckColor`, `height: round(size * 0.12)`, `width: max(3, round(size * 0.035))`, `radius: width/2`

### PLAY Button Pause Bars (when playing)
- Two Rectangles in Row centered: `color: #85c85b`, `height: round(parent.height * 0.34)`, `width: round(barHeight * 0.3)`, spacing `round(barHeight * 0.3)`

### PLAY Button Play Triangle (when stopped)
- `Skin.Triangle`: centered, `horizontalCenterOffset: round(width * 0.1)`, `color: #85c85b`, `height: round(parent.height * 0.36)`, `width: round(height * 0.9)`, points right

### BoothBeatFx Prev/Next Triangles
- `Skin.Triangle`: height 11, width 8, centered in 24xprefHeight button, pointsLeft/Right

### BoothBeatFx Slot Boxes
- Simple Rectangle with border, fill, centered number text

---

## 11. Drag Sensitivity & Interaction Parameters

| Control | Sensitivity Formula | Notes |
|---------|---------------------|-------|
| BoothKnob | `delta = -translation.y / dragRange` (default 160 px per full turn) | Vertical drag only; double-tap resets |
| BoothFader (vertical) | `delta = -translation.y / (height - handle.height)` | Inverted option flips mapping; double-tap resets |
| BoothFader (horizontal) | `delta = translation.x / (width - handle.width)` | For crossfader/tempo horizontal |
| TouchSpinny (jog) | Handled by `DeckComponent.TouchSpinny` - scratch on touch, auto-spin when playing | Not directly parameterized here |

---

## 12. Font Sizing Rules (all relative to `unit`)

| Element | PixelSize Formula |
|---------|-------------------|
| SmallButton label | `round(unit * 1.05)` (min 6) |
| Pad label | `round(unit * 1.05)` |
| Hotcue letter | `round(letter.height * 0.75)` |
| StemPad label | `round(unit * 0.9)` |
| Caption | `round(unit * 0.95)` |
| Chip label | `round(unit * 0.95)` (HorizontalFit, min 7) |
| Deck "DECK" label | `round(unit * 0.62)` |
| Deck number | `round(unit * 1.0)` |
| Track title | `round(unit * 1.45)` |
| Key label | `round(unit * 1.3)` |
| Time MM:SS | `round(unit * 3.4)` |
| Time .SSS | `round(unit * 1.7)` |
| Tempo caption | `round(unit * 0.95)` |
| Tempo range | `round(unit * 0.95)` |
| Tempo pitch | `round(unit * 1.45)` |
| BPM caption | `round(unit * 0.95)` |
| BPM MASTER badge | `round(unit * 0.95)` |
| BPM whole | `round(unit * 2.6)` |
| BPM fraction | `round(unit * 1.5)` |
| Loop "LOOP" label | `round(unit * 0.95)` |
| Loop size value | `round(unit * 1.8)` |
| Loop BEATS/EXIT | `round(unit * 0.8)` |
| SYNC/CUE/PLAY button label | `round(unit * 1.05)` (SmallButton) |
| CUE/PLAY button text | `round(button.height * 0.28)` |
| Jog BPM whole | `round(jogSize * 0.17)` |
| Jog BPM fraction | `round(jogSize * 0.09)` |
| Jog rate text | `round(jogSize * 0.075)` |
| Q/MT buttons | `round(unit * 0.95)` (SmallButton) |
| BoothKnob label | `max(8, min(13, round(width * 0.22)))` |
| BoothBeatFx SmallLabel | `compact ? 9 : 10` |
| BoothBeatFx ToggleBox label | `compact ? 11 : 13` |
| BoothBeatFx slot number | `11` |
| BoothBeatFx ON/OFF | `compact ? 13 : 15` |

---

## 13. Geometry Constants & Rounding

- All `Math.round` applied to computed sizes before use
- `unit = max(9, min(height/14, width/34))` (PerformanceDeck) or `max(9, min(height/11, width/30))` (DeckPanel)
- Spacing: typically `3` or `round(unit * 0.5)` etc.
- Margins: `4` (main layout), `1` (title bar), `6` (info area), `5`/`8` (BeatFx compact/normal)
- Radii: `2` (pads, buttons, fader track/handle), `3` (loop size, BeatFx prev/next), `width/2` (circular buttons, platter)
- Border widths: `1` default, `2` for active loops, CUE/PLAY buttons, playing sampler pads

---

## 14. Mirroring Behaviour

- `BoothPerformanceDeck`: `RowLayout.layoutDirection = mirrored ? Qt.RightToLeft : Qt.LeftToRight`
  - Deck 1 (left): pads left, jog right, tempo far right
  - Deck 2 (right, mirrored): pads right, jog left, tempo far left
- `BoothDeckPanel`: no mirroring (display only)
- `SamplerPad`: `mirrored` selects sampler bank: left deck to Sampler1-8, right deck to Sampler9-16
- Jog bottom buttons (Q/MT): anchored left if mirrored else right

---

## 15. Compact Mode (Landscape Phones)

- `BoothPerformanceDeck`: `unit = max(9, min(height/14, width/34))` - same formula, but smaller height reduces unit
- `BoothDeckPanel`: `unit = max(9, min(height/11, width/30))`
- `BoothBeatFx`: `compact` prop reduces heights (header 20 vs 26, slots 20/24, toggles 24/30, sliders 22/32, ON/OFF 34/44, labels 9/10, toggle labels 11/13, ON/OFF 13/15)
- No separate compact logic in BoothKnob/BoothFader (size from parent)

---

## 16. Colour Opacity Usage

| Usage | Base Colour | Opacity | Result |
|-------|-------------|---------|--------|
| SmallButton on fill | onColor | 0.18 | `alpha(onColor, 0.18)` |
| StemPad active fill | padColor | 0.22 | `alpha(padColor, 0.22)` |
| SamplerPad playing fill | warningColor | 0.3 | `alpha(#7d3b3b, 0.3)` |
| SamplerPad lit fill | libraryDeck2Background | 0.18 | `alpha(#e8650f, 0.18)` |
| TitleBar YouTube progress | #2ae633 | 0.5 | `#2ae63380` |
| WaveformMarkerLabel | white | 0.8 | `rgba(255,255,255,0.8)` |
| Embedded backgrounds | black | 0.627 | `#a0000000` |

---

## 17. File Output

This specification written to: `/home/ubuntu/djsugar-next/docs/parity/booth-deck.md`
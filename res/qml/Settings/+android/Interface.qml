import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import Qt5Compat.GraphicalEffects
import Mixxx 1.0 as Mixxx
import "." as Setting
import "../Deck" as DeckComponents
import "." as SettingComponents
import ".." as Skin
import "../Theme"

Category {
    id: root

    function load() {
        loadInterface();
        loadDeck();
        errorMessage.text = "";
    }

    function loadDeck() {
        cueModeInput.currentIndex = Mixxx.Config.controlCueDefault;
        setIntroStartToMainCueInput.selected = Mixxx.Config.controlSetIntroStartAtMainCue ? "on" : "off";
        trackTimeDisplayInput.selected = trackTimeDisplayInput.options[Mixxx.Config.controlPositionDisplay] || "elapsed";
        timeMode.currentIndex = Mixxx.Config.controlTimeFormat;
        doublePressLoadToCloneInput.selected = Mixxx.Config.controlCloneDeckOnLoadDoubleTap ? "on" : "off";
        trackLoadPointInput.currentIndex = Mixxx.Config.controlCueRecall;
        loadingTrackWhenPlayingInput.selected = loadingTrackWhenPlayingInput.options[Mixxx.Config.controlLoadWhenDeckPlaying] || "reject";
        resetOnTrackLoadInput.selected = resetOnTrackLoadInput.options[Mixxx.Config.controlSpeedAutoReset] || "none";
        sliderRangeInput.currentIndex = sliderRangeInput.values.indexOf(Mixxx.Config.controlRateRange);
        if (sliderRangeInput.currentIndex === -1) {
            sliderRangeInput.values.push(Mixxx.Config.controlRateRange);
            sliderRangeInput.model.push(`${Mixxx.Config.controlRateRange}%`);
            sliderRangeInput.currentIndex = sliderRangeInput.model.length - 1;
        }
        syncModeInput.selected = syncModeInput.options[Mixxx.Config.bpmSyncLockAlgorithm] || "follow soft leader";
        sliderOrientationInput.selected = Mixxx.Config.controlRateDir === -1 ? "down" : "up";
        keylockModeInput.selected = keylockModeInput.options[Mixxx.Config.controlKeylockMode] || "original key";
        keyunlockModeInput.selected = keyunlockModeInput.options[Mixxx.Config.controlKeyunlockMode] || "reset key";
        pitchBendBehaviourInput.selected = pitchBendBehaviourInput.options[Mixxx.Config.controlPitchBendBehaviour] || "abrupt jump";
        adjustmentButtonsTemporaryCoarseInput.value = Mixxx.Config.controlRateTempCoarse * 100;
        adjustmentButtonsTemporaryFineInput.value = Mixxx.Config.controlRateTempFine * 100;
        adjustmentButtonsPermanentCoarseInput.value = Mixxx.Config.controlRatePermCoarse * 100;
        adjustmentButtonsPermanentFineInput.value = Mixxx.Config.controlRatePermFine * 100;
        rampingSensitivityInput.value = Mixxx.Config.controlRateRampSensitivity;
        decksTab.dirty = false;
    }

    function loadInterface() {
        tooltipsInput.selected = tooltipsInput.options[Mixxx.Config.libraryTooltips] || "off";
        disableScreensaverInput.selected = disableScreensaverInput.options[Mixxx.Config.libraryInhibitScreensaver] || "no";
        startFullscreenInput.selected = Mixxx.Config.configStartInFullscreenKey ? "on" : "off";
        autoHideMenuBarInput.selected = Mixxx.Config.libraryHideMenuBar ? "on" : "off";
        searchCompletionInput.selected = Mixxx.Config.libraryEnableSearchCompletions ? "on" : "off";
        searchHistoryKeyboardInput.selected = Mixxx.Config.libraryEnableSearchHistoryShortcuts ? "on" : "off";
        bpmPrecisionInput.value = Mixxx.Config.libraryBpmColumnPrecision;
        libraryRowHeightInput.value = Mixxx.Config.libraryRowHeight;
        trackPaletteComboBox.currentIndex = Math.max(0, trackPaletteComboBox.model.indexOf(Mixxx.Config.configTrackColorPalette));
        hotcuePaletteComboBox.currentIndex = Math.max(0, hotcuePaletteComboBox.model.indexOf(Mixxx.Config.configHotcueColorPalette));
        keyPaletteInput.selected = Mixxx.Config.configKeyColorsEnabled ? "on" : "off";
        keyPaletteComboBox.currentIndex = Math.max(0, keyPaletteComboBox.model.indexOf(Mixxx.Config.configKeyColorPalette));

        colorPane.hotcuePaletteColorIndex = Mixxx.Config.controlHotcueDefaultColorIndex;
        if (colorPane.hotcuePaletteColorIndex < 0)
            colorPane.hotcuePaletteColorIndex = colorPane.defaultPalette.length - 1;
        hotcuePaletteInput.currentIndex = colorPane.hotcuePaletteColorIndex;

        colorPane.loopPaletteColorIndex = Mixxx.Config.controlLoopDefaultColorIndex;
        if (colorPane.loopPaletteColorIndex < 0)
            colorPane.loopPaletteColorIndex = colorPane.defaultPalette.length - 2;
        loopPaletteInput.currentIndex = colorPane.loopPaletteColorIndex;

        colorPane.jumpPaletteColorIndex = colorPane.defaultPalette.length - 3;
        jumpPaletteInput.currentIndex = colorPane.jumpPaletteColorIndex;

        themeColorTab.dirty = false;
    }

    function resetDeck() {
        loadDeck();
    }

    function resetInterface() {
        loadInterface();
    }

    function saveDeck() {
        Mixxx.Config.controlCueDefault = cueModeInput.currentIndex;
        Mixxx.Config.controlSetIntroStartAtMainCue = setIntroStartToMainCueInput.selected === "on";
        Mixxx.Config.controlPositionDisplay = trackTimeDisplayInput.options.indexOf(trackTimeDisplayInput.selected);
        Mixxx.Config.controlTimeFormat = timeMode.currentIndex;
        Mixxx.Config.controlCloneDeckOnLoadDoubleTap = doublePressLoadToCloneInput.selected === "on";
        Mixxx.Config.controlCueRecall = trackLoadPointInput.currentIndex;
        Mixxx.Config.controlLoadWhenDeckPlaying = loadingTrackWhenPlayingInput.options.indexOf(loadingTrackWhenPlayingInput.selected);
        Mixxx.Config.controlSpeedAutoReset = resetOnTrackLoadInput.options.indexOf(resetOnTrackLoadInput.selected);
        for (let i = 0; i < deckRateRange.count; i++) {
            deckRateRange.objectAt(i).value = sliderRangeInput.values[sliderRangeInput.currentIndex];
        }
        Mixxx.Config.controlRateRange = sliderRangeInput.values[sliderRangeInput.currentIndex];
        Mixxx.Config.bpmSyncLockAlgorithm = syncModeInput.options.indexOf(syncModeInput.selected);
        for (let i = 0; i < deckRateDirection.count; i++) {
            deckRateDirection.objectAt(i).value = sliderOrientationInput.selected === "down" ? -1 : 1;
        }
        Mixxx.Config.controlRateDir = sliderOrientationInput.selected === "down" ? -1 : 1;
        Mixxx.Config.controlKeylockMode = keylockModeInput.options.indexOf(keylockModeInput.selected);
        Mixxx.Config.controlKeyunlockMode = keyunlockModeInput.options.indexOf(keyunlockModeInput.selected);
        Mixxx.Config.controlPitchBendBehaviour = pitchBendBehaviourInput.options.indexOf(pitchBendBehaviourInput.selected);
        Mixxx.Config.controlRateTempCoarse = adjustmentButtonsTemporaryCoarseInput.value / 100;
        Mixxx.Config.controlRateTempFine = adjustmentButtonsTemporaryFineInput.value / 100;
        Mixxx.Config.controlRatePermCoarse = adjustmentButtonsPermanentCoarseInput.value / 100;
        Mixxx.Config.controlRatePermFine = adjustmentButtonsPermanentFineInput.value / 100;
        Mixxx.Config.controlRateRampSensitivity = rampingSensitivityInput.value;
        loadDeck();
    }

    function saveInterface() {
        Mixxx.Config.libraryTooltips = tooltipsInput.options.indexOf(tooltipsInput.selected);
        Mixxx.Config.libraryInhibitScreensaver = disableScreensaverInput.options.indexOf(disableScreensaverInput.selected);
        Mixxx.Config.configStartInFullscreenKey = startFullscreenInput.selected === "on";
        Mixxx.Config.libraryHideMenuBar = autoHideMenuBarInput.selected === "on";
        Mixxx.Config.libraryEnableSearchCompletions = searchCompletionInput.selected === "on";
        Mixxx.Config.libraryEnableSearchHistoryShortcuts = searchHistoryKeyboardInput.selected === "on";
        Mixxx.Config.libraryBpmColumnPrecision = bpmPrecisionInput.value;
        Mixxx.Config.libraryRowHeight = libraryRowHeightInput.value;
        Mixxx.Config.configTrackColorPalette = trackPaletteComboBox.model[trackPaletteComboBox.currentIndex];
        Mixxx.Config.configHotcueColorPalette = hotcuePaletteComboBox.model[hotcuePaletteComboBox.currentIndex];
        Mixxx.Config.configKeyColorsEnabled = keyPaletteInput.selected === "on";
        Mixxx.Config.configKeyColorPalette = keyPaletteComboBox.model[keyPaletteComboBox.currentIndex];
        Mixxx.Config.controlHotcueDefaultColorIndex = hotcuePaletteInput.currentIndex;
        Mixxx.Config.controlLoopDefaultColorIndex = loopPaletteInput.currentIndex;
        loadInterface();
    }

    label: qsTr("Interface")
    selectedIndex: 0
    tabs: [qsTr("Appearance & Colors"), qsTr("Decks & Playback")]

    Component.onCompleted: {
        root.load();
    }

    Mixxx.ControlProxy {
        id: numDecks
        group: "[App]"
        key: "num_decks"
    }

    Instantiator {
        id: deckRateRange
        model: numDecks.value
        Mixxx.ControlProxy {
            required property int index
            group: `[Channel${index + 1}]`
            key: "rateRange"
        }
    }

    Instantiator {
        id: deckRateDirection
        model: numDecks.value
        Mixxx.ControlProxy {
            required property int index
            group: `[Channel${index + 1}]`
            key: "rate_dir"
        }
    }

    // MAIN CONTENT CONTAINER
    Item {
        anchors.bottom: buttonActions.top
        anchors.bottomMargin: 8
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top

        // ================= TAB 0: APARIENCIA & COLORES =================
        Item {
            id: themeColorTab
            property bool dirty: false
            anchors.fill: parent
            visible: root.selectedIndex === 0

            ScrollView {
                id: themeScrollView
                anchors.fill: parent
                clip: true
                contentWidth: availableWidth

                ColumnLayout {
                    width: themeScrollView.availableWidth
                    spacing: 12

                    // CARD 1: APARIENCIA GENERAL
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: card1Col.implicitHeight + 20
                        color: "#181b24"
                        radius: 8
                        border.color: "#282d3b"
                        border.width: 1

                        ColumnLayout {
                            id: card1Col
                            anchors.fill: parent
                            anchors.margins: 12
                            spacing: 10

                            Text {
                                text: qsTr("🎨 APPEARANCE & DISPLAY")
                                font.bold: true
                                font.pixelSize: 13
                                color: "#00e5ff"
                            }

                            // Applied immediately, independently of the Save button
                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Language"); color: Theme.white; font.pixelSize: 13 }
                                Skin.ComboBox {
                                    id: languageInput

                                    function syncCurrentIndex() {
                                        currentIndex = Math.max(0, indexOfValue(Mixxx.Language.locale));
                                    }

                                    Layout.preferredWidth: 220
                                    popupMaxItem: 10
                                    textRole: "label"
                                    valueRole: "locale"
                                    model: [{
                                            "locale": "",
                                            "label": qsTr("System default")
                                        }].concat(Mixxx.Language.availableLocales.map(entry => ({
                                                "locale": entry.locale,
                                                "label": entry.nativeName === entry.name ? entry.name : `${entry.nativeName} – ${entry.name}`
                                            })))

                                    Component.onCompleted: syncCurrentIndex()
                                    onModelChanged: Qt.callLater(syncCurrentIndex)
                                    // Wait for the popup to close: the texts of its items are
                                    // rebuilt by the change.
                                    onActivated: {
                                        const locale = currentValue;
                                        Qt.callLater(() => {
                                            if (locale !== Mixxx.Language.locale) {
                                                languageRestartHint.visible = true;
                                                Mixxx.Language.locale = locale;
                                            }
                                        });
                                    }

                                    Connections {
                                        function onLocaleChanged() {
                                            languageInput.syncCurrentIndex();
                                        }

                                        target: Mixxx.Language
                                    }
                                }
                            }

                            Text {
                                id: languageRestartHint

                                Layout.fillWidth: true
                                visible: false
                                text: qsTr("Library and effect names change after restarting Mixxx.")
                                color: "#ffb74d"
                                font.pixelSize: 11
                                wrapMode: Text.WordWrap
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Skin"); color: Theme.white; font.pixelSize: 13 }
                                // Applied immediately, like the language: the
                                // interface is reloaded with the other skin
                                Skin.ComboBox {
                                    id: skinInput

                                    readonly property var skins: Mixxx.MobileApp.skinsIn(Qt.resolvedUrl("../Skins/"))

                                    Layout.preferredWidth: 180
                                    model: skins.map(skin => skin === "Classic" ? qsTr("Classic") : skin)
                                    currentIndex: Math.max(0, skins.indexOf(Mixxx.MobileApp.skin))
                                    onActivated: index => {
                                        Mixxx.MobileApp.skin = skins[index];
                                    }
                                }
                            }

                            RowLayout {
                                // Not used on Android (no light theme yet)
                                visible: false
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Theme color"); color: Theme.white; font.pixelSize: 13 }
                                Skin.ComboBox {
                                    id: colorInput
                                    Layout.preferredWidth: 180
                                    model: [qsTr("Dark"), qsTr("Light")]
                                    onCurrentIndexChanged: themeColorTab.dirty = true
                                }
                            }

                            RowLayout {
                                // Not used on Android (the skins have one layout)
                                visible: false
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Layout"); color: Theme.white; font.pixelSize: 13 }
                                Skin.ComboBox {
                                    id: layoutInput
                                    Layout.preferredWidth: 180
                                    model: [qsTr("Performance"), qsTr("Broadcast")]
                                    onCurrentIndexChanged: themeColorTab.dirty = true
                                }
                            }

                            RowLayout {
                                // Not used on Android (no mouse to hover with)
                                visible: false
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Tooltips"); color: Theme.white; font.pixelSize: 13 }
                                RatioChoice {
                                    id: tooltipsInput
                                    options: ["off", "library", "all"]
                                    onSelectedChanged: themeColorTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Keep screen on (screensaver)"); color: Theme.white; font.pixelSize: 13 }
                                RatioChoice {
                                    id: disableScreensaverInput
                                    options: ["no", "while running", "while playing"]
                                    onSelectedChanged: themeColorTab.dirty = true
                                }
                            }

                            RowLayout {
                                // Not used on Android (always fullscreen)
                                visible: false
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Start in fullscreen"); color: Theme.white; font.pixelSize: 13 }
                                RatioChoice {
                                    id: startFullscreenInput
                                    options: ["on", "off"]
                                    onSelectedChanged: themeColorTab.dirty = true
                                }
                            }

                            RowLayout {
                                // Not used on Android (the top bar of the skins is always shown)
                                visible: false
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Auto-hide top bar"); color: Theme.white; font.pixelSize: 13 }
                                RatioChoice {
                                    id: autoHideMenuBarInput
                                    options: ["on", "off"]
                                    onSelectedChanged: themeColorTab.dirty = true
                                }
                            }
                        }
                    }

                    // Items shown in the top bar. Applied immediately.
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: topBarCol.implicitHeight + 20
                        color: "#181b24"
                        radius: 8
                        border.color: "#282d3b"
                        border.width: 1

                        ColumnLayout {
                            id: topBarCol
                            anchors.fill: parent
                            anchors.margins: 12
                            spacing: 10

                            Text {
                                text: qsTr("🧭 TOP BAR")
                                font.bold: true
                                font.pixelSize: 13
                                color: "#00e5ff"
                            }
                            Text {
                                Layout.fillWidth: true
                                text: qsTr("Choose the views and buttons shown at the top of the screen. At least one view stays visible.")
                                color: "#90a4ae"
                                font.pixelSize: 11
                                wrapMode: Text.WordWrap
                            }

                            Flow {
                                Layout.fillWidth: true
                                spacing: 6

                                Repeater {
                                    id: topBarItems

                                    // The items of the Classic skin; other skins list theirs
                                    // in a topBarItems property of their main window
                                    readonly property var classicItems: [
                                        {name: "dj", label: "DJ", view: true},
                                        {name: "wave", label: qsTr("WAVE"), view: true},
                                        {name: "lib", label: qsTr("LIB"), view: true},
                                        {name: "ctl", label: qsTr("CTL"), view: true},
                                        {name: "load1", label: qsTr("LOAD %1").arg(1), view: false},
                                        {name: "load2", label: qsTr("LOAD %1").arg(2), view: false},
                                        {name: "fx", label: "FX", view: false},
                                        {name: "xy", label: "XY", view: false},
                                        {name: "split", label: "SPLIT", view: false},
                                        {name: "rec", label: "REC", view: false},
                                        {name: "4d", label: "4D", view: false}
                                    ]
                                    readonly property var skinItems: ApplicationWindow.window?.skinItem?.topBarItems

                                    model: skinItems ?? classicItems

                                    Skin.Button {
                                        required property var modelData
                                        readonly property bool shown: Mixxx.MobileApp.hiddenTopBarItems.indexOf(modelData.name) < 0
                                        readonly property int shownViews: topBarItems.model.filter(item => item.view && Mixxx.MobileApp.hiddenTopBarItems.indexOf(item.name) < 0).length

                                        implicitWidth: 72
                                        implicitHeight: 34
                                        activeColor: Theme.accentColor
                                        backgroundColor: shown ? "#1e3a5f" : "#222733"
                                        normalColor: shown ? "#ffffff" : "#607d8b"
                                        font.bold: true
                                        font.pixelSize: 11
                                        text: (shown ? "✓ " : "") + modelData.label
                                        // The last view shown can't be hidden
                                        enabled: !(shown && modelData.view && shownViews <= 1)
                                        onClicked: {
                                            let hidden = Mixxx.MobileApp.hiddenTopBarItems.slice();
                                            if (shown) {
                                                hidden.push(modelData.name);
                                            } else {
                                                hidden = hidden.filter(name => name !== modelData.name);
                                            }
                                            Mixxx.MobileApp.hiddenTopBarItems = hidden;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // CARD 2: BIBLIOTECA & COLECCIÓN
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: card2Col.implicitHeight + 20
                        color: "#181b24"
                        radius: 8
                        border.color: "#282d3b"
                        border.width: 1

                        ColumnLayout {
                            id: card2Col
                            anchors.fill: parent
                            anchors.margins: 12
                            spacing: 10

                            Text {
                                text: qsTr("📁 LIBRARY & COLLECTION")
                                font.bold: true
                                font.pixelSize: 13
                                color: "#80d8ff"
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Search completions"); color: Theme.white; font.pixelSize: 13 }
                                RatioChoice {
                                    id: searchCompletionInput
                                    options: ["on", "off"]
                                    onSelectedChanged: themeColorTab.dirty = true
                                }
                            }

                            RowLayout {
                                // Not used on Android (keyboard shortcuts)
                                visible: false
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Search history"); color: Theme.white; font.pixelSize: 13 }
                                RatioChoice {
                                    id: searchHistoryKeyboardInput
                                    options: ["on", "off"]
                                    onSelectedChanged: themeColorTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("BPM column decimals"); color: Theme.white; font.pixelSize: 13 }
                                SettingComponents.SpinBox {
                                    id: bpmPrecisionInput
                                    Layout.preferredWidth: 140
                                    editable: false
                                    max: 4
                                    min: 0
                                    precision: 0
                                    realValue: 1
                                    onValueChanged: themeColorTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Track row height"); color: Theme.white; font.pixelSize: 13 }
                                SettingComponents.Slider {
                                    id: libraryRowHeightInput
                                    Layout.preferredWidth: 200
                                    markers: [24, 30, 38, 50]
                                    max: 60
                                    min: 24
                                    suffix: "px"
                                    value: 38
                                    onValueChanged: themeColorTab.dirty = true
                                }
                            }
                        }
                    }

                    // CARD 3: PALETAS DE COLOR
                    Rectangle {
                        id: colorPane
                        readonly property var defaultPalette: Mixxx.Config.getHotcueColorPalette(hotcuePaletteComboBox.currentText)
                        property int hotcuePaletteColorIndex: 0
                        property int jumpPaletteColorIndex: 0
                        property int loopPaletteColorIndex: 0

                        Layout.fillWidth: true
                        implicitHeight: card3Col.implicitHeight + 20
                        color: "#181b24"
                        radius: 8
                        border.color: "#282d3b"
                        border.width: 1

                        ColumnLayout {
                            id: card3Col
                            anchors.fill: parent
                            anchors.margins: 12
                            spacing: 10

                            Text {
                                text: qsTr("🌈 COLOR PALETTES")
                                font.bold: true
                                font.pixelSize: 13
                                color: "#ea80fc"
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Track palette"); color: Theme.white; font.pixelSize: 13 }
                                ColorPaletteComboBox {
                                    id: trackPaletteComboBox
                                    Layout.preferredWidth: 220
                                    onCurrentIndexChanged: themeColorTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Hotcue palette"); color: Theme.white; font.pixelSize: 13 }
                                ColorPaletteComboBox {
                                    id: hotcuePaletteComboBox
                                    Layout.preferredWidth: 220
                                    onCurrentIndexChanged: themeColorTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Default hotcue color"); color: Theme.white; font.pixelSize: 13 }
                                DefaultColorSelector {
                                    id: hotcuePaletteInput
                                    Layout.preferredWidth: colorPane.defaultPalette.length * 18
                                    currentIndex: colorPane.hotcuePaletteColorIndex
                                    palette: colorPane.defaultPalette
                                    onCurrentIndexChanged: themeColorTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Default loop color"); color: Theme.white; font.pixelSize: 13 }
                                DefaultColorSelector {
                                    id: loopPaletteInput
                                    Layout.preferredWidth: colorPane.defaultPalette.length * 18
                                    currentIndex: colorPane.loopPaletteColorIndex
                                    palette: colorPane.defaultPalette
                                    onCurrentIndexChanged: themeColorTab.dirty = true
                                }
                            }

                            RowLayout {
                                // Not used on Android (not saved by Mixxx)
                                visible: false
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Default beatjump color"); color: Theme.white; font.pixelSize: 13 }
                                DefaultColorSelector {
                                    id: jumpPaletteInput
                                    Layout.preferredWidth: colorPane.defaultPalette.length * 18
                                    currentIndex: colorPane.jumpPaletteColorIndex
                                    palette: colorPane.defaultPalette
                                    onCurrentIndexChanged: themeColorTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Key colors"); color: Theme.white; font.pixelSize: 13 }
                                RatioChoice {
                                    id: keyPaletteInput
                                    options: ["on", "off"]
                                    onSelectedChanged: themeColorTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                visible: keyPaletteInput.selected === "on"
                                Text { Layout.fillWidth: true; text: qsTr("Key palette"); color: Theme.white; font.pixelSize: 13 }
                                ColorPaletteComboBox {
                                    id: keyPaletteComboBox
                                    Layout.preferredWidth: 220
                                    onCurrentIndexChanged: themeColorTab.dirty = true
                                }
                            }
                        }
                    }
                }
            }
        }

        // ================= TAB 1: DECKS & REPRODUCCIÓN =================
        Item {
            id: decksTab
            property bool dirty: false
            anchors.fill: parent
            visible: root.selectedIndex === 1

            ScrollView {
                id: decksScrollView
                anchors.fill: parent
                clip: true
                contentWidth: availableWidth

                ColumnLayout {
                    width: decksScrollView.availableWidth
                    spacing: 12

                    // CARD 1: CUE & COMPORTAMIENTO DE CARGA
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: deckCard1Col.implicitHeight + 20
                        color: "#181b24"
                        radius: 8
                        border.color: "#282d3b"
                        border.width: 1

                        ColumnLayout {
                            id: deckCard1Col
                            anchors.fill: parent
                            anchors.margins: 12
                            spacing: 10

                            Text {
                                text: qsTr("🎛️ CUE MODES & LOADING BEHAVIOR")
                                font.bold: true
                                font.pixelSize: 13
                                color: "#00e5ff"
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Cue mode"); color: Theme.white; font.pixelSize: 13 }
                                Skin.ComboBox {
                                    id: cueModeInput
                                    Layout.preferredWidth: 190
                                    model: ["Mixxx", qsTr("Mixxx (no blinking)"), "Pioneer", "Denon", "Numark", "CUP"]
                                    onCurrentIndexChanged: decksTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Track load point"); color: Theme.white; font.pixelSize: 13 }
                                Skin.ComboBox {
                                    id: trackLoadPointInput
                                    Layout.preferredWidth: 190
                                    model: [qsTr("Main cue"), qsTr("Beginning of track"), qsTr("First sound"), qsTr("Intro start"), qsTr("First hotcue")]
                                    onCurrentIndexChanged: decksTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Set intro start to main cue"); color: Theme.white; font.pixelSize: 13 }
                                RatioChoice {
                                    id: setIntroStartToMainCueInput
                                    options: ["on", "off"]
                                    onSelectedChanged: decksTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Loading a track when deck is playing"); color: Theme.white; font.pixelSize: 13 }
                                RatioChoice {
                                    id: loadingTrackWhenPlayingInput
                                    options: ["reject", "allow", "when stopped"]
                                    onSelectedChanged: decksTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Double-tap LOAD to clone deck"); color: Theme.white; font.pixelSize: 13 }
                                RatioChoice {
                                    id: doublePressLoadToCloneInput
                                    options: ["on", "off"]
                                    onSelectedChanged: decksTab.dirty = true
                                }
                            }
                        }
                    }

                    // CARD 2: FORMATO DE TIEMPO
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: deckCard2Col.implicitHeight + 20
                        color: "#181b24"
                        radius: 8
                        border.color: "#282d3b"
                        border.width: 1

                        ColumnLayout {
                            id: deckCard2Col
                            anchors.fill: parent
                            anchors.margins: 12
                            spacing: 10

                            Text {
                                text: qsTr("⏱️ TIME FORMAT & DISPLAY")
                                font.bold: true
                                font.pixelSize: 13
                                color: "#80d8ff"
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Time format"); color: Theme.white; font.pixelSize: 13 }
                                Skin.ComboBox {
                                    id: timeMode
                                    Layout.preferredWidth: 200
                                    model: [
                                        DeckComponents.TrackTime.Mode.Traditional,
                                        DeckComponents.TrackTime.Mode.TraditionalCoarse,
                                        DeckComponents.TrackTime.Mode.Seconds,
                                        DeckComponents.TrackTime.Mode.SecondsLong,
                                        DeckComponents.TrackTime.Mode.KiloSeconds,
                                        DeckComponents.TrackTime.Mode.HectoSeconds
                                    ]
                                    contentItem: Content {
                                        anchors.fill: parent
                                        anchors.leftMargin: 10
                                        anchors.rightMargin: 20
                                        mode: timeMode.model[timeMode.currentIndex]
                                    }
                                    onCurrentIndexChanged: decksTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Track time display"); color: Theme.white; font.pixelSize: 13 }
                                RatioChoice {
                                    id: trackTimeDisplayInput
                                    options: ["elapsed", "remaining", "both"]
                                    onSelectedChanged: decksTab.dirty = true
                                }
                            }
                        }
                    }

                    // CARD 3: PITCH, SINCRONIZACIÓN & KEYLOCK
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: deckCard3Col.implicitHeight + 20
                        color: "#181b24"
                        radius: 8
                        border.color: "#282d3b"
                        border.width: 1

                        ColumnLayout {
                            id: deckCard3Col
                            anchors.fill: parent
                            anchors.margins: 12
                            spacing: 10

                            Text {
                                text: qsTr("🎚️ PITCH, SPEED & SYNC")
                                font.bold: true
                                font.pixelSize: 13
                                color: "#ffb74d"
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Pitch slider range"); color: Theme.white; font.pixelSize: 13 }
                                Skin.ComboBox {
                                    id: sliderRangeInput
                                    readonly property list<int> values: [4, 6, 8, 10, 16, 24, 50, 90]
                                    Layout.preferredWidth: 200
                                    model: ["4%", qsTr("6% (semitone)"), "8% (Technics)", "10%", "16%", "24%", "50%", "90%"]
                                    onCurrentIndexChanged: decksTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Pitch fader orientation"); color: Theme.white; font.pixelSize: 13 }
                                RatioChoice {
                                    id: sliderOrientationInput
                                    options: ["down", "up"]
                                    onSelectedChanged: decksTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("BPM sync algorithm"); color: Theme.white; font.pixelSize: 13 }
                                RatioChoice {
                                    id: syncModeInput
                                    options: ["follow soft leader", "use steady"]
                                    onSelectedChanged: decksTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Reset speed on track load"); color: Theme.white; font.pixelSize: 13 }
                                RatioChoice {
                                    id: resetOnTrackLoadInput
                                    options: ["none", "key", "both", "tempo"]
                                    onSelectedChanged: decksTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Keylock mode"); color: Theme.white; font.pixelSize: 13 }
                                RatioChoice {
                                    id: keylockModeInput
                                    options: ["original key", "current key"]
                                    onSelectedChanged: decksTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Keyunlock mode"); color: Theme.white; font.pixelSize: 13 }
                                RatioChoice {
                                    id: keyunlockModeInput
                                    options: ["reset key", "keep key"]
                                    onSelectedChanged: decksTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Pitch bend behavior"); color: Theme.white; font.pixelSize: 13 }
                                RatioChoice {
                                    id: pitchBendBehaviourInput
                                    options: ["abrupt jump", "smooth ramping"]
                                    onSelectedChanged: decksTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Ramping sensitivity"); color: Theme.white; font.pixelSize: 13 }
                                SettingComponents.Slider {
                                    id: rampingSensitivityInput
                                    Layout.preferredWidth: 200
                                    max: 2500
                                    min: 100
                                    value: 250
                                    wheelStep: 50
                                    onValueChanged: decksTab.dirty = true
                                }
                            }
                        }
                    }

                    // CARD 4: BOTONES DE AJUSTE TEMPORAL Y PERMANENTE
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: deckCard4Col.implicitHeight + 20
                        color: "#181b24"
                        radius: 8
                        border.color: "#282d3b"
                        border.width: 1

                        ColumnLayout {
                            id: deckCard4Col
                            anchors.fill: parent
                            anchors.margins: 12
                            spacing: 10

                            Text {
                                text: qsTr("⚡ SPEED / PITCH BEND BUTTON STEPS")
                                font.bold: true
                                font.pixelSize: 13
                                color: "#b0bec5"
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Temporary fine (%)"); color: Theme.white; font.pixelSize: 13 }
                                SettingComponents.SpinBox {
                                    id: adjustmentButtonsTemporaryFineInput
                                    Layout.preferredWidth: 140
                                    max: 10
                                    min: 0.01
                                    precision: 2
                                    realValue: 2
                                    suffix: "%"
                                    onValueChanged: decksTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Temporary coarse (%)"); color: Theme.white; font.pixelSize: 13 }
                                SettingComponents.SpinBox {
                                    id: adjustmentButtonsTemporaryCoarseInput
                                    Layout.preferredWidth: 140
                                    max: 10
                                    min: 0.01
                                    precision: 2
                                    realValue: 4
                                    suffix: "%"
                                    onValueChanged: decksTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Permanent fine (%)"); color: Theme.white; font.pixelSize: 13 }
                                SettingComponents.SpinBox {
                                    id: adjustmentButtonsPermanentFineInput
                                    Layout.preferredWidth: 140
                                    max: 10
                                    min: 0.01
                                    precision: 2
                                    realValue: 0.05
                                    suffix: "%"
                                    onValueChanged: decksTab.dirty = true
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                Text { Layout.fillWidth: true; text: qsTr("Permanent coarse (%)"); color: Theme.white; font.pixelSize: 13 }
                                SettingComponents.SpinBox {
                                    id: adjustmentButtonsPermanentCoarseInput
                                    Layout.preferredWidth: 140
                                    max: 10
                                    min: 0.01
                                    precision: 2
                                    realValue: 0.5
                                    suffix: "%"
                                    onValueChanged: decksTab.dirty = true
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // ================= BARRA INFERIOR FIJA: ACCIONES Y ESTADO =================
    Rectangle {
        id: buttonActions
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        height: 58
        color: "#181b24"
        border.color: "#282d3c"
        border.width: 1
        radius: 8

        readonly property bool hasChanges: (root.selectedIndex === 0 && themeColorTab.dirty) || (root.selectedIndex === 1 && decksTab.dirty)

        RowLayout {
            anchors.fill: parent
            anchors.margins: 10
            spacing: 12

            // Status indicator pill
            Rectangle {
                Layout.preferredHeight: 34
                Layout.preferredWidth: statusRow.implicitWidth + 24
                radius: 17
                color: buttonActions.hasChanges ? "#332200" : "#0d2b1a"
                border.color: buttonActions.hasChanges ? "#ffb300" : "#00c853"
                border.width: 1

                RowLayout {
                    id: statusRow
                    anchors.centerIn: parent
                    spacing: 8
                    Text {
                        text: buttonActions.hasChanges ? "●" : "✓"
                        font.bold: true
                        font.pixelSize: 13
                        color: buttonActions.hasChanges ? "#ffb300" : "#00e676"
                    }
                    Text {
                        text: buttonActions.hasChanges ? qsTr("Unsaved changes") : qsTr("Interface configured")
                        font.bold: true
                        font.pixelSize: 12
                        color: buttonActions.hasChanges ? "#ffe082" : "#b9f6ca"
                    }
                }
            }

            // Error message if any
            Text {
                id: errorMessage
                Layout.alignment: Qt.AlignVCenter
                visible: text.length > 0
                color: "#ff5252"
                font.pixelSize: 12
                text: ""
            }

            Item { Layout.fillWidth: true }

            // Botón Restablecer
            Rectangle {
                Layout.preferredHeight: 38
                Layout.preferredWidth: 140
                radius: 6
                color: resetMouse.pressed ? "#5c2b2b" : (resetMouse.containsMouse ? "#4a2222" : "#371e1e")
                border.color: "#772b2b"
                border.width: 1

                RowLayout {
                    anchors.centerIn: parent
                    spacing: 6
                    Text { text: "↺"; color: "#ff8a80"; font.pixelSize: 14 }
                    Text {
                        text: qsTr("Reset")
                        font.bold: true
                        font.pixelSize: 12
                        color: "#ffcdd2"
                    }
                }

                MouseArea {
                    id: resetMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        switch (root.selectedIndex) {
                        case 0:
                            root.resetInterface();
                            break;
                        case 1:
                            root.resetDeck();
                            break;
                        }
                    }
                }
            }

            // Botón Descartar
            Rectangle {
                Layout.preferredHeight: 38
                Layout.preferredWidth: 110
                radius: 6
                visible: buttonActions.hasChanges
                color: discardMouse.pressed ? "#424242" : (discardMouse.containsMouse ? "#333333" : "#262626")
                border.color: "#555555"
                border.width: 1

                Text {
                    anchors.centerIn: parent
                    text: qsTr("Discard")
                    font.bold: true
                    font.pixelSize: 13
                    color: "#e0e0e0"
                }

                MouseArea {
                    id: discardMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        errorMessage.text = "";
                        switch (root.selectedIndex) {
                        case 0:
                            root.loadInterface();
                            break;
                        case 1:
                            root.loadDeck();
                            break;
                        }
                    }
                }
            }

            // Botón Guardar Interfaz
            Rectangle {
                Layout.preferredHeight: 40
                Layout.preferredWidth: 220
                radius: 6
                color: buttonActions.hasChanges
                       ? (saveMouse.pressed ? "#009624" : (saveMouse.containsMouse ? "#00e676" : "#00c853"))
                       : (saveMouse.pressed ? "#0d47a1" : (saveMouse.containsMouse ? "#1e88e5" : "#1976d2"))
                border.color: buttonActions.hasChanges ? "#69f0ae" : "#64b5f6"
                border.width: 1.5

                RowLayout {
                    anchors.centerIn: parent
                    spacing: 8

                    Text {
                        text: buttonActions.hasChanges ? "💾" : "✓"
                        font.pixelSize: 14
                        color: "#ffffff"
                    }

                    Text {
                        text: buttonActions.hasChanges ? qsTr("Save interface") : qsTr("Saved")
                        font.bold: true
                        font.pixelSize: 13
                        color: "#ffffff"
                    }
                }

                MouseArea {
                    id: saveMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        errorMessage.text = "";
                        switch (root.selectedIndex) {
                        case 0:
                            root.saveInterface();
                            break;
                        case 1:
                            root.saveDeck();
                            break;
                        }
                    }
                }
            }
        }
    }

    component Content: RowLayout {
        required property var mode

        Text {
            Layout.fillWidth: true
            color: Theme.deckTextColor
            elide: Text.ElideRight
            text: {
                switch (parent.mode) {
                case DeckComponents.TrackTime.Mode.TraditionalCoarse:
                    return qsTr("Traditional (Coarse)");
                case DeckComponents.TrackTime.Mode.Seconds:
                    return qsTr("Seconds");
                case DeckComponents.TrackTime.Mode.SecondsLong:
                    return qsTr("Seconds (Long)");
                case DeckComponents.TrackTime.Mode.KiloSeconds:
                    return qsTr("Kiloseconds");
                case DeckComponents.TrackTime.Mode.HectoSeconds:
                    return qsTr("Hectoseconds");
                default:
                case DeckComponents.TrackTime.Mode.Traditional:
                    return qsTr("Traditional");
                }
            }
            verticalAlignment: Text.AlignVCenter
        }
        DeckComponents.TrackTime {
            Layout.rightMargin: 17
            display: trackTimeDisplayInput.selected === "elapsed" ? DeckComponents.TrackTime.Display.Elapsed : trackTimeDisplayInput.selected === "remaining" ? DeckComponents.TrackTime.Display.Remaining : DeckComponents.TrackTime.Display.Both
            elapsed: 83.45
            mode: parent.mode
            remaining: 147.23
        }
    }
}

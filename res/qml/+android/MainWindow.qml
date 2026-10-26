import "." as Skin
import Mixxx 1.0 as Mixxx
import QtQuick 2.12
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import QtQuick.Shapes
import Qt5Compat.GraphicalEffects
import "Theme"

Item {
    id: root

    required property ApplicationWindow applicationWindow
    property alias menuBar: nativeApplicationMenuLoader.item

    property alias editDeck: editDeckButton.checked
    property var focusedDeck: null
    property int viewMode: 1
    // Items of the top bar hidden in Settings → Interface
    readonly property var hiddenTopBarItems: Mixxx.MobileApp.hiddenTopBarItems
    function topBarItemShown(name) {
        return root.hiddenTopBarItems.indexOf(name) < 0;
    }
    // The views of the top bar, by view mode
    readonly property var topBarViews: [{mode: 1, name: "dj"}, {mode: 3, name: "wave"}, {mode: 2, name: "lib"}, {mode: 0, name: "ctl"}]
    readonly property int shownTopBarViews: root.topBarViews.filter(view => root.topBarItemShown(view.name)).length
    // A hidden view is left for the first view still shown
    function leaveHiddenView() {
        const current = root.topBarViews.find(view => view.mode === root.viewMode);
        if (!current || root.topBarItemShown(current.name)) {
            return;
        }
        const first = root.topBarViews.find(view => root.topBarItemShown(view.name));
        if (first) {
            root.viewMode = first.mode;
        }
    }
    onHiddenTopBarItemsChanged: root.leaveHiddenView()
    Component.onCompleted: root.leaveHiddenView()
    property bool maximizeLibrary: viewMode === 2
    readonly property int numDecks: 4
    readonly property int numPreviewDecks: 1
    readonly property int numSamplers: 16
    readonly property int waveformOverviewTypeRgb: 2
    readonly property bool show4decks: show4DecksButton.checked && show4DecksButton.visible
    property bool showEffects: false
    property bool showSamplers: false

    function openQuickLoad(group) {
        quickLoadDrawer.targetDeck = (group && group.length > 0) ? group : "[Channel1]";
        quickLoadDrawer.open();
    }

    Loader {
        id: nativeApplicationMenuLoader

        active: Qt.platform.os === "osx" || (Mixxx.Application.supportsGlobalMenuBar && root.applicationWindow.visibility !== Window.FullScreen)

        sourceComponent: Skin.MainMenuBar {
            actions: applicationMenuActions
        }
    }
    Skin.ApplicationMenuCommands {
        id: applicationMenuCommands

        applicationWindow: root.applicationWindow

        onShowDeveloperToolsRequested: {
            devToolsWindow.show();
            devToolsWindow.raise();
            devToolsWindow.requestActivate();
        }
    }
    Skin.ApplicationMenuActions {
        id: applicationMenuActions

        applicationWindow: root.applicationWindow
        commands: applicationMenuCommands
        numberOfDecks: root.show4decks ? root.numDecks : 2

        onFocusLibrarySearchRequested: {
            // The library stays loaded while hidden, show it first
            if (!library.visible) {
                root.viewMode = 2;
            }
            Qt.callLater(() => {
                if (library.item) {
                    library.item.focusSearch();
                }
            });
        }
    }
    Skin.LibraryScanSummaryDialog {
    }
    Mixxx.ControlProxy {
        group: "[App]"
        key: "num_decks"

        onInitializedChanged: {
            value = root.numDecks;
        }
    }
    Mixxx.ControlProxy {
        group: "[App]"
        key: "num_samplers"

        onInitializedChanged: {
            value = root.numSamplers;
        }
    }
    Mixxx.ControlProxy {
        group: "[App]"
        key: "num_preview_decks"

        onInitializedChanged: {
            value = root.numPreviewDecks;
        }
    }
    Mixxx.ControlProxy {
        group: "[Waveform]"
        key: "WaveformOverviewType"

        onInitializedChanged: {
            value = root.waveformOverviewTypeRgb;
        }
    }
    Mixxx.ControlProxy {
        id: headSplitControl
        group: "[Master]"
        key: "headSplit"
    }
    Column {
        id: content

        anchors.fill: parent
        enabled: !quickLoadDrawer.opened && !settingsPopup.opened

        move: Transition {
            NumberAnimation {
                duration: 150
                properties: "x,y"
            }
        }

        Rectangle {
            id: toolbar

            color: Theme.toolbarBackgroundColor
            height: 36
            radius: 1
            width: parent.width
            z: 20

            Text {
                id: logoText
                anchors.left: parent.left
                anchors.leftMargin: 10
                anchors.verticalCenter: parent.verticalCenter
                color: Theme.accentColor
                font.bold: true
                font.pixelSize: 14
                font.letterSpacing: 1.5
                text: "MIXXX"
            }

            Skin.Button {
                id: showPreferencesButton
                anchors.right: parent.right
                anchors.rightMargin: 6
                anchors.verticalCenter: parent.verticalCenter
                height: 30
                width: 36
                activeColor: Theme.accentColor
                checked: settingsPopup.opened
                icon.height: 18
                icon.source: "images/gear.svg"
                icon.width: 18

                onClicked: {
                    if (!settingsPopup.opened) {
                        settingsPopup.open();
                    }
                }
                onPressAndHold: {
                    Mixxx.PreferencesDialog.show();
                }
            }

            Flickable {
                id: toolbarFlickable
                anchors.left: logoText.right
                anchors.leftMargin: 8
                anchors.right: showPreferencesButton.left
                anchors.rightMargin: 8
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                contentWidth: toolbarRow.implicitWidth
                flickableDirection: Flickable.HorizontalFlick
                boundsBehavior: Flickable.StopAtBounds
                clip: true

                RowLayout {
                    id: toolbarRow
                    anchors.verticalCenter: parent.verticalCenter
                    height: parent.height
                    spacing: 4

                    Rectangle {
                        Layout.preferredHeight: 30
                        Layout.preferredWidth: 236 * root.shownTopBarViews / 4
                        visible: root.shownTopBarViews > 0
                        color: "#181a20"
                        radius: 4
                        border.color: "#2a2e39"
                        border.width: 1

                        RowLayout {
                            anchors.fill: parent
                            anchors.margins: 2
                            spacing: 2

                            Skin.Button {
                                Layout.fillHeight: true
                                Layout.fillWidth: true
                                activeColor: Theme.accentColor
                                backgroundColor: root.viewMode === 1 ? Theme.accentColor : "transparent"
                                normalColor: root.viewMode === 1 ? "#ffffff" : "#90a4ae"
                                text: "DJ"
                                font.bold: true
                                font.pixelSize: 10
                                onClicked: root.viewMode = 1
                                visible: root.topBarItemShown("dj")
                            }

                            Skin.Button {
                                Layout.fillHeight: true
                                Layout.fillWidth: true
                                activeColor: Theme.accentColor
                                backgroundColor: root.viewMode === 3 ? Theme.accentColor : "transparent"
                                normalColor: root.viewMode === 3 ? "#ffffff" : "#90a4ae"
                                text: qsTr("WAVE")
                                font.bold: true
                                font.pixelSize: 10
                                onClicked: root.viewMode = 3
                                visible: root.topBarItemShown("wave")
                            }

                            Skin.Button {
                                Layout.fillHeight: true
                                Layout.fillWidth: true
                                activeColor: Theme.accentColor
                                backgroundColor: root.viewMode === 2 ? Theme.accentColor : "transparent"
                                normalColor: root.viewMode === 2 ? "#ffffff" : "#90a4ae"
                                text: qsTr("LIB")
                                font.bold: true
                                font.pixelSize: 10
                                onClicked: root.viewMode = 2
                                visible: root.topBarItemShown("lib")
                            }

                            Skin.Button {
                                Layout.fillHeight: true
                                Layout.fillWidth: true
                                activeColor: Theme.accentColor
                                backgroundColor: root.viewMode === 0 ? Theme.accentColor : "transparent"
                                normalColor: root.viewMode === 0 ? "#ffffff" : "#90a4ae"
                                text: qsTr("CTL")
                                font.bold: true
                                font.pixelSize: 10
                                onClicked: root.viewMode = 0
                                visible: root.topBarItemShown("ctl")
                            }
                        }
                    }

                    Skin.Button {
                        id: quickLoadD1Button
                        visible: root.topBarItemShown("load1")
                        Layout.preferredHeight: 30
                        Layout.preferredWidth: 62
                        activeColor: "#64b5f6"
                        backgroundColor: "#1565c0"
                        normalColor: "#ffffff"
                        font.bold: true
                        font.pixelSize: 10
                        text: qsTr("LOAD %1").arg(1)
                        onClicked: root.openQuickLoad("[Channel1]")
                    }

                    Skin.Button {
                        id: quickLoadD2Button
                        visible: root.topBarItemShown("load2")
                        Layout.preferredHeight: 30
                        Layout.preferredWidth: 62
                        activeColor: "#ffb74d"
                        backgroundColor: "#e65100"
                        normalColor: "#ffffff"
                        font.bold: true
                        font.pixelSize: 10
                        text: qsTr("LOAD %1").arg(2)
                        onClicked: root.openQuickLoad("[Channel2]")
                    }

                    Skin.Button {
                        id: showEffectsButton
                        visible: root.topBarItemShown("fx")
                        Layout.preferredHeight: 30
                        Layout.preferredWidth: 42
                        activeColor: "#e040fb"
                        backgroundColor: root.showEffects ? "#4a148c" : "#222733"
                        normalColor: root.showEffects ? "#ea80fc" : "#b0bec5"
                        checkable: true
                        checked: root.showEffects
                        text: "FX"
                        font.bold: true
                        font.pixelSize: 10
                        onClicked: root.showEffects = !root.showEffects
                    }

                    Skin.Button {
                        id: xyPadButton
                        visible: root.topBarItemShown("xy")
                        Layout.preferredHeight: 30
                        Layout.preferredWidth: 54
                        activeColor: "#00e5ff"
                        backgroundColor: xyPadOverlay.visible ? "#006064" : "#222733"
                        normalColor: xyPadOverlay.visible ? "#00e5ff" : "#b0bec5"
                        checkable: true
                        checked: xyPadOverlay.visible
                        text: "XY"
                        font.bold: true
                        font.pixelSize: 10
                        onClicked: xyPadOverlay.visible = !xyPadOverlay.visible
                    }

                    Skin.Button {
                        id: splitCueButton
                        visible: root.topBarItemShown("split")
                        Layout.preferredHeight: 30
                        Layout.preferredWidth: 52
                        activeColor: "#ff9100"
                        backgroundColor: headSplitControl.value > 0 ? "#e65100" : "#222733"
                        normalColor: headSplitControl.value > 0 ? "#ffffff" : "#b0bec5"
                        checkable: true
                        checked: headSplitControl.value > 0
                        text: "SPLIT"
                        font.bold: true
                        font.pixelSize: 10
                        onClicked: {
                            headSplitControl.value = (headSplitControl.value > 0) ? 0 : 1;
                        }
                    }

                    // Asks before stopping, keeps its time across skin switches
                    Skin.MobileRecordButton {
                        id: recButton
                        visible: root.topBarItemShown("rec")
                        Layout.preferredHeight: 30
                        Layout.preferredWidth: implicitWidth
                    }

                    Skin.Button {
                        id: editDeckButton
                        Layout.preferredHeight: 30
                        Layout.preferredWidth: 42
                        activeColor: Theme.white
                        checkable: true
                        text: qsTr("Edit")
                        visible: !root.applicationWindow.isMobile
                    }

                    Skin.Button {
                        id: show4DecksButton
                        Layout.preferredHeight: 30
                        Layout.preferredWidth: 42
                        activeColor: Theme.white
                        checkable: true
                        text: "4D"
                        visible: root.height > 515 && root.topBarItemShown("4d")
                    }
                }
            }
        }

        Skin.EffectRow {
            id: effects
            visible: root.showEffects
            width: parent.width
        }

        SplitView {
            id: splitView

            height: parent.height - y
            orientation: Qt.Vertical
            width: parent.width

            handle: Rectangle {
                id: handleDelegate

                property color handleColor: SplitHandle.pressed || SplitHandle.hovered ? Theme.libraryPanelSplitterHandleActive : Theme.libraryPanelSplitterHandle
                property int handleSize: SplitHandle.pressed || SplitHandle.hovered ? 6 : 3

                clip: true
                color: Theme.libraryPanelSplitterBackground
                implicitHeight: 4
                implicitWidth: 8

                containmentMask: Item {
                    height: 8
                    width: splitView.width
                    x: (handleDelegate.width - width) / 2
                }

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
            }

            Item {
                id: waveforms

                readonly property real waveDeckBarHeight: root.viewMode === 3 ? Math.min(115, Math.max(86, Math.round(parent.height * 0.38))) : 0
                readonly property real scrollingHeight: Math.max(0, height - waveDeckBarHeight)

                SplitView.fillHeight: root.viewMode === 3
                SplitView.preferredHeight: {
                    if (root.viewMode === 3) return splitView.height;
                    if (root.viewMode === 0) return Math.round(splitView.height * 0.42);
                    if (root.viewMode === 2 || root.maximizeLibrary) return 0;
                    return Math.max(80, Math.min(130, Math.round(splitView.height * 0.32)));
                }
                visible: root.viewMode !== 2 && !root.maximizeLibrary

                FadeBehavior on visible {
                    fadeTarget: waveforms
                }

                Loader {
                    id: deck3waveform

                    readonly property string group: "[Channel3]"

                    active: root.show4decks
                    anchors.top: parent.top
                    height: waveforms.scrollingHeight / 4
                    width: root.width

                    sourceComponent: Component {
                        Skin.WaveformDisplay {
                            group: deck3waveform.group
                            isTouchMode: root.viewMode === 1

                            FadeBehavior on visible {
                                fadeTarget: deck3waveform
                            }
                        }
                    }
                }
                Skin.WaveformDisplay {
                    id: deck1waveform

                    anchors.top: root.show4decks ? deck3waveform.bottom : parent.top
                    group: "[Channel1]"
                    height: waveforms.scrollingHeight / (root.show4decks ? 4 : 2)
                    isTouchMode: root.viewMode === 1
                    width: root.width
                }
                Skin.WaveformDisplay {
                    id: deck2waveform

                    anchors.top: deck1waveform.bottom
                    group: "[Channel2]"
                    height: waveforms.scrollingHeight / (root.show4decks ? 4 : 2)
                    isTouchMode: root.viewMode === 1
                    width: root.width
                }
                Loader {
                    id: deck4waveform

                    readonly property string group: "[Channel4]"

                    active: root.show4decks
                    anchors.top: deck2waveform.bottom
                    height: waveforms.scrollingHeight / 4
                    width: root.width

                    sourceComponent: Component {
                        Skin.WaveformDisplay {
                            group: deck4waveform.group
                            isTouchMode: root.viewMode === 1

                            FadeBehavior on visible {
                                fadeTarget: deck4waveform
                            }
                        }
                    }
                }

                WaveDeckBar {
                    id: waveDeckBar
                    anchors.bottom: parent.bottom
                    anchors.left: parent.left
                    anchors.right: parent.right
                    height: waveforms.waveDeckBarHeight
                    visible: root.viewMode === 3
                    z: 10
                }
            }
            Item {
                id: lowerSection

                SplitView.fillHeight: root.viewMode !== 3
                SplitView.minimumHeight: (root.viewMode === 1 && !root.maximizeLibrary) ? mixer.height : 60
                visible: root.viewMode !== 3

                Deck {
                    id: deck1

                    editMode: root.editDeck
                    group: "[Channel1]"
                    height: (root.viewMode === 2 || root.maximizeLibrary) ? 70 : root.show4decks ? mixer.height / 2 : mixer.height
                    minimized: root.viewMode === 2 || root.maximizeLibrary
                    visible: root.viewMode === 1 || root.viewMode === 2

                    Behavior on height {
                        SpringAnimation {
                            id: deck1HeightAnimation

                            damping: 0.2
                            duration: 500
                            spring: 2
                        }
                    }
                    states: [
                        State {
                            when: root.maximizeLibrary

                            AnchorChanges {
                                anchors.right: parent.horizontalCenter
                                target: deck1
                            }
                        }
                    ]

                    onToggleFocus: {
                        root.focusedDeck = (root.focusedDeck === deck1) ? null : deck1;
                    }

                    anchors {
                        left: parent.left
                        right: mixer.left
                        top: parent.top
                    }
                }
                Mixer {
                    id: mixer

                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.top: parent.top
                    groups: [deck1.group, deck2.group, deck3.group, deck4.group]
                    height: implicitHeight
                    show4decks: root.show4decks
                    visible: root.viewMode === 1 && !root.maximizeLibrary
                    width: implicitWidth

                    Behavior on height {
                        SpringAnimation {
                            id: mixerHeightAnimation

                            damping: 0.2
                            duration: 500
                            spring: 2
                        }
                    }
                    states: [
                        State {
                            when: (!root.focusedDeck || root.width > 1400) && !root.maximizeLibrary

                            AnchorChanges {
                                anchors.horizontalCenter: parent.horizontalCenter
                                target: mixer
                            }
                        },
                        State {
                            when: root.maximizeLibrary || root.viewMode === 2

                            AnchorChanges {
                                anchors.horizontalCenter: parent.horizontalCenter
                                target: mixer
                            }
                            PropertyChanges {
                                target: deck1
                                width: root.width / 2
                            }
                            PropertyChanges {
                                target: deck2
                                width: root.width / 2
                            }
                        }
                    ]
                    transitions: Transition {
                        AnchorAnimation {
                            duration: 200
                        }
                    }
                    FadeBehavior on visible {
                        fadeTarget: mixer
                    }
                }
                Deck {
                    id: deck2

                    editMode: root.editDeck
                    group: "[Channel2]"
                    height: (root.viewMode === 2 || root.maximizeLibrary) ? 70 : root.show4decks ? mixer.height / 2 : mixer.height
                    minimized: root.viewMode === 2 || root.maximizeLibrary
                    visible: root.viewMode === 1 || root.viewMode === 2

                    Behavior on height {
                        SpringAnimation {
                            id: deck2HeightAnimation

                            damping: 0.2
                            duration: 500
                            spring: 2
                        }
                    }
                    states: [
                        State {
                            when: root.maximizeLibrary

                            AnchorChanges {
                                anchors.left: parent.horizontalCenter
                                target: deck2
                            }
                        }
                    ]

                    onToggleFocus: {
                        root.focusedDeck = (root.focusedDeck === deck2) ? null : deck2;
                    }

                    anchors {
                        left: mixer.right
                        right: parent.right
                        top: parent.top
                    }
                }
                Loader {
                    id: deck3

                    readonly property string group: "[Channel3]"

                    active: root.show4decks
                    height: active ? (root.maximizeLibrary ? 80 : mixer.height / 2) : 0

                    Behavior on height {
                        SpringAnimation {
                            id: deck3HeightAnimation

                            damping: 0.2
                            duration: 500
                            spring: 2
                        }
                    }
                    sourceComponent: Component {
                        Deck {
                            anchors.bottom: parent.bottom
                            anchors.left: parent.left
                            editMode: root.editDeck
                            group: deck3.group
                            minimized: root.maximizeLibrary
                        }
                    }
                    states: [
                        State {
                            when: root.maximizeLibrary

                            AnchorChanges {
                                anchors.right: parent.horizontalCenter
                                target: deck3
                            }
                        }
                    ]

                    anchors {
                        left: parent.left
                        right: mixer.left
                        top: deck1.bottom
                    }
                }
                Loader {
                    id: deck4

                    readonly property string group: "[Channel4]"

                    active: root.show4decks
                    height: active ? (root.maximizeLibrary ? 80 : mixer.height / 2) : 0

                    Behavior on height {
                        SpringAnimation {
                            id: deck4HeightAnimation

                            damping: 0.2
                            duration: 500
                            spring: 2
                        }
                    }
                    sourceComponent: Component {
                        Deck {
                            anchors.bottom: parent.bottom
                            anchors.right: parent.right
                            editMode: root.editDeck
                            group: deck4.group
                            minimized: root.maximizeLibrary
                        }
                    }
                    states: [
                        State {
                            when: root.maximizeLibrary

                            AnchorChanges {
                                anchors.left: parent.horizontalCenter
                                target: deck4
                            }
                        }
                    ]

                    anchors {
                        left: mixer.right
                        right: parent.right
                        top: deck2.bottom
                    }
                }

                // Skin.SamplerRow {
                //     id: samplers
                //     visible: root.showSamplers
                //     width: parent.width

                //     Skin.FadeBehavior on visible {
                //         fadeTarget: samplers
                //     }
                // }
                Loader {
                    id: library

                    // Once loaded, the library stays loaded (only hidden) when
                    // switching to another view, so it keeps its search,
                    // selection and scroll position, like the deck loaders.
                    // Hidden, it ignores the controller (see Library.qml).
                    property bool keepLoaded: false

                    active: keepLoaded || root.viewMode === 0 || root.viewMode === 2 || root.maximizeLibrary || root.height - mixer.height >= 400
                    onLoaded: keepLoaded = true
                    visible: root.viewMode === 0 || root.viewMode === 2 || root.maximizeLibrary || (root.viewMode === 1 && root.height - mixer.height >= 400)
                    width: parent.width

                    sourceComponent: Component {
                        Skin.Library {
                            anchors.fill: parent
                            acceptsController: !quickLoadDrawer.opened && !settingsPopup.opened
                        }
                    }
                    states: [
                        State {
                            when: root.viewMode === 0

                            AnchorChanges {
                                anchors.bottom: parent.bottom
                                anchors.top: parent.top
                                target: library
                            }
                            PropertyChanges {
                                target: library
                                visible: true
                            }
                        },
                        State {
                            when: (root.maximizeLibrary || root.viewMode === 2) && root.show4decks

                            AnchorChanges {
                                anchors.bottom: parent.bottom
                                anchors.top: deck4.bottom
                                target: library
                            }
                            PropertyChanges {
                                target: library
                                visible: true
                            }
                        },
                        State {
                            when: (root.maximizeLibrary || root.viewMode === 2) && !root.show4decks

                            AnchorChanges {
                                anchors.bottom: parent.bottom
                                anchors.top: deck1.bottom
                                target: library
                            }
                            PropertyChanges {
                                target: library
                                visible: true
                            }
                        },
                        State {
                            when: root.viewMode === 1 && root.height - mixer.height >= 400

                            AnchorChanges {
                                anchors.bottom: parent.bottom
                                anchors.top: mixer.bottom
                                target: library
                            }
                            PropertyChanges {
                                target: library
                                visible: true
                            }
                        },
                        State {
                            when: root.viewMode === 1 && root.height - mixer.height < 400

                            PropertyChanges {
                                target: library
                                visible: false
                            }
                        }
                    ]

                    anchors {
                        left: parent.left
                        right: parent.right
                        bottom: parent.bottom
                        top: root.viewMode === 0 ? parent.top : mixer.bottom
                    }
                }
            }
        }
    }
    Skin.Settings {
        id: settingsPopup

        height: root.applicationWindow.isMobile ? parent.height : Math.min(840, parent.height)
        modal: true
        width: root.applicationWindow.isMobile ? parent.width : Math.min(1400, parent.width)
        x: root.applicationWindow.isMobile ? 0 : Math.round((parent.width - width) / 2)
        y: root.applicationWindow.isMobile ? 0 : Math.round((parent.height - height) / 2)

        Overlay.modal: Rectangle {
            id: overlayModal

            readonly property bool hasHardwareAcceleration: Mixxx.Config.useAcceleration
            property real radius: 12

            anchors.fill: parent
            color: Qt.alpha('#00000010', hasHardwareAcceleration ? 1.0 : 0.6)

            Repeater {
                model: hasHardwareAcceleration ? 1 : 0

                GaussianBlur {
                    anchors.fill: overlayModal
                    deviation: 4
                    radius: Math.max(0, overlayModal.radius)
                    samples: 16
                    source: content
                }
            }
        }
    }

    // ================= CONTROLLER ACCESS TO THE SCREENS =================
    // [Skin] controls that controller mappings can use:
    // - mobile_library_toggle: shows the LIB view, or returns to the previous
    //   view if it is shown (SHIFT+LOAD on the DDJ-400 and DDJ-FLX4)
    // - mobile_loader_deck1/2: opens the deck loader of deck 1/2
    // Besides, LOAD 1/2 open the deck loader when no library is on screen,
    // instead of loading the track selected in a library that can't be seen.
    property int viewModeBeforeLibrary: 1
    // A ControlProxy only connects to its control if it exists when the
    // proxy is created, so the proxies of these [Skin] controls only get
    // their key once their SkinControlCreator has created them.
    property bool mobileControlsCreated: false

    function toggleLibraryView() {
        if (root.viewMode === 2) {
            root.viewMode = root.viewModeBeforeLibrary === 2 ? 1 : root.viewModeBeforeLibrary;
        } else {
            root.viewModeBeforeLibrary = root.viewMode;
            root.viewMode = 2;
        }
    }

    Mixxx.SkinControlCreator {
        group: "[Skin]"
        key: "mobile_library_toggle"

        Component.onCompleted: root.mobileControlsCreated = true
    }

    Mixxx.ControlProxy {
        group: "[Skin]"
        key: root.mobileControlsCreated ? "mobile_library_toggle" : ""
        onValueChanged: value => {
            if (value > 0) {
                quickLoadDrawer.close();
                root.toggleLibraryView();
            }
        }
    }

    // Shows or hides the effects (FX), e.g. SHIFT + BEAT ▶ of the DDJ-FLX4
    property bool fxControlCreated: false

    Mixxx.SkinControlCreator {
        group: "[Skin]"
        key: "mobile_fx_toggle"

        Component.onCompleted: root.fxControlCreated = true
    }
    Mixxx.ControlProxy {
        group: "[Skin]"
        key: root.fxControlCreated ? "mobile_fx_toggle" : ""
        onValueChanged: value => {
            if (value > 0) {
                root.showEffects = !root.showEffects;
            }
        }
    }

    Repeater {
        model: 2

        Item {
            id: deckLoaderAccess

            required property int index
            readonly property string group: `[Channel${index + 1}]`
            property bool controlCreated: false

            Mixxx.SkinControlCreator {
                group: "[Skin]"
                key: `mobile_loader_deck${deckLoaderAccess.index + 1}`

                Component.onCompleted: deckLoaderAccess.controlCreated = true
            }

            Mixxx.ControlProxy {
                group: "[Skin]"
                key: deckLoaderAccess.controlCreated ? `mobile_loader_deck${deckLoaderAccess.index + 1}` : ""
                onValueChanged: value => {
                    if (value > 0) {
                        root.openQuickLoad(deckLoaderAccess.group);
                    }
                }
            }

            Mixxx.ControlProxy {
                group: deckLoaderAccess.group
                key: "LoadSelectedTrack"
                onValueChanged: value => {
                    if (value <= 0) {
                        return;
                    }
                    if (quickLoadDrawer.opened) {
                        // The library of the deck loader loads the track,
                        // close it afterwards
                        Qt.callLater(quickLoadDrawer.close);
                    } else if (!library.visible) {
                        // Deferred, so the library of the deck loader doesn't
                        // also handle this press and load a track right away
                        Qt.callLater(root.openQuickLoad, deckLoaderAccess.group);
                    }
                }
            }
        }
    }

    Skin.QuickLoadDrawer {
        id: quickLoadDrawer

        height: root.applicationWindow.isMobile ? parent.height : Math.min(840, parent.height)
        width: root.applicationWindow.isMobile ? parent.width : Math.min(1400, parent.width)
        x: root.applicationWindow.isMobile ? 0 : Math.round((parent.width - width) / 2)
        y: root.applicationWindow.isMobile ? 0 : Math.round((parent.height - height) / 2)
    }

    Skin.XYPad {
        id: xyPadOverlay
        visible: false
        z: 90
        anchors.centerIn: parent
        width: Math.min(680, parent.width - 24)
        height: Math.min(380, parent.height - 50)
        onCloseRequested: visible = false
    }
}

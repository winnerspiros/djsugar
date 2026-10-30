import "../.." as Skin
import "." as Booth
import Mixxx 1.0 as Mixxx
import QtQuick
import QtQuick.Controls

// Booth skin: a main window in the style of the screen of a booth player,
// for landscape phones and tablets. The waveforms are stacked over the
// whole width, the deck displays are below them and BEAT FX (effect unit 1)
// is on the right. BROWSE shows the skin's browser (BoothBrowse.qml) in
// place of the waveforms; LOAD 1/2 open it for one deck. DJ shows touch
// controls for playing without a controller (BoothDjView.qml), and is
// shown when no controller is used; XY the XY pad. UTILITY opens the
// shared settings in the skin's frame.
//
// A skin is a folder in res/qml/Skins with a MainWindow.qml whose root item
// takes the application window, see packaging/android/README.md.
Item {
    id: root

    required property ApplicationWindow applicationWindow
    property var menuBar: null

    // 1: waveforms (WAVE), 2: the browser (BROWSE), 3: touch controls
    // (DJ), 4: the XY pad (XY)
    property int viewMode: 1
    readonly property bool browsing: viewMode === 2
    readonly property bool djView: viewMode === 3
    readonly property bool xyView: viewMode === 4
    // A controller with an enabled mapping is connected. Without one the DJ
    // view is shown, as the decks can't be played otherwise.
    readonly property bool controllerConnected: {
        const devices = Mixxx.ControllerManager.knownDevices;
        for (let i = 0; i < devices.length; ++i) {
            if (devices[i].enabled) {
                return true;
            }
        }
        return false;
    }
    // The deck LOAD 1/2 opened the browser for, or "". Loading a track into
    // it returns to the waveforms.
    property string loadTarget: ""
    readonly property var loadTargetPlayer: loadTarget.length > 0 ? Mixxx.PlayerManager.getPlayer(loadTarget) : null
    // Landscape phones
    readonly property bool compact: height < 520
    // BEAT FX is shown on demand on phones, and can be hidden on tablets
    property bool beatFxShown: false
    property bool beatFxPanelShown: true

    // [Skin],mobile_fx_toggle, e.g. SHIFT + BEAT ▶ of the DDJ-FLX4
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
    readonly property int numDecks: 4
    readonly property int numSamplers: 16
    readonly property int numPreviewDecks: 1
    readonly property int waveformOverviewTypeRgb: 2

    // The colors of the shared parts (library, settings, popups) in this
    // skin, applied by main.qml; see Theme/+android/Theme.qml
    readonly property var skinTheme: ({
            "accentColor": "#1e5fb4",
            "backgroundColor": "#000000",
            "sunkenBackgroundColor": "#0c0d0f",
            "darkGray": "#000000",
            "darkGray2": "#1c1e22",
            "darkGray3": "#2a2c30",
            "darkGray4": "#141518",
            "textColor": "#e6e8eb",
            "panelSplitterBackground": "#000000",
            "libraryScrollHandle": "#3a3c40",
            "libraryScrollHandleHovered": "#5c6068",
            "libraryScrollHandlePressed": "#d8dadd",
            "libraryScrollTrack": "#000000",
            "libraryActionBackground": "#1c1e22",
            "libraryActionDisabledBackground": "#111214",
            "libraryActionText": "#ffffff",
            "libraryActionDisabledText": "#5c6068",
            "libraryDeck1Background": "#1f6fd1",
            "libraryDeck1Pressed": "#5a9be8",
            "libraryDeck2Background": "#e8650f",
            "libraryDeck2Pressed": "#ff9a52",
            "libraryConfirmBackground": "#1e5fb4",
            "libraryKeyMatchColor": "#3ad33a",
            "libraryKeyMatchBackground": "#1a3ad33a"
        })

    // The deck loader of the Classic skin is the browser here
    function openQuickLoad(group) {
        root.loadTarget = (group && group.length > 0) ? group : "[Channel1]";
        root.viewMode = 2;
    }
    // The view to return to after browsing
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

    // The tabs of the top bar, which can be hidden in Settings → Interface
    // → TOP BAR (Mixxx.MobileApp.hiddenTopBarItems, shared with the Classic
    // skin, whose names are used where the tabs match). The settings list
    // these instead of the ones of the Classic skin. At least one view
    // stays shown; UTILITY always does.
    readonly property var topBarItems: [
        {
            "name": "wave",
            "label": qsTr("WAVE"),
            "view": true
        },
        {
            "name": "dj",
            "label": "DJ",
            "view": true
        },
        {
            "name": "lib",
            "label": qsTr("BROWSE"),
            "view": true
        },
        {
            "name": "load1",
            "label": qsTr("LOAD %1").arg(1),
            "view": false
        },
        {
            "name": "load2",
            "label": qsTr("LOAD %1").arg(2),
            "view": false
        },
        {
            "name": "xy",
            "label": "XY",
            "view": false
        },
        {
            "name": "fx",
            "label": "BEAT FX",
            "view": false
        },
        {
            "name": "split",
            "label": "SPLIT",
            "view": false
        },
        {
            "name": "rec",
            "label": "REC",
            "view": false
        }
    ]
    readonly property var hiddenTopBarItems: Mixxx.MobileApp.hiddenTopBarItems
    // The name of the tab of each view
    readonly property var viewNames: ({
            "1": "wave",
            "2": "lib",
            "3": "dj",
            "4": "xy"
        })

    function itemShown(name) {
        return root.hiddenTopBarItems.indexOf(name) < 0;
    }
    // The view to play in: DJ without a controller, else the waveforms; the
    // other one or the browser if that one is hidden
    function preferredView() {
        const order = root.controllerConnected ? [1, 3, 2] : [3, 1, 2];
        const mode = order.find(mode => root.itemShown(root.viewNames[mode]));
        return mode ?? 2;
    }
    // A hidden view is left, except the browser opened by LOAD 1/2 or a
    // controller
    function leaveHiddenView() {
        if (root.browsing || root.itemShown(root.viewNames[root.viewMode])) {
            return;
        }
        root.showView(root.preferredView());
    }

    Component.onCompleted: root.showView(root.preferredView())
    onHiddenTopBarItemsChanged: root.leaveHiddenView()
    // A controller plugged in or out mid-set: its view
    onControllerConnectedChanged: {
        if ((root.viewMode === 1 || root.viewMode === 3) && root.viewMode !== root.preferredView()) {
            root.showView(root.preferredView());
        }
    }
    function toggleLibraryView() {
        if (root.browsing && root.loadTarget.length === 0) {
            root.showPlayView();
        } else {
            root.loadTarget = "";
            root.viewMode = 2;
        }
    }

    Connections {
        function onTrackChanged() {
            if (root.browsing) {
                root.showPlayView();
            }
        }

        ignoreUnknownSignals: true
        target: root.loadTargetPlayer
    }

    // ---- The same engine setup as the Classic skin ----
    Mixxx.ControlProxy {
        group: "[App]"
        key: "num_decks"

        onInitializedChanged: value = root.numDecks
    }
    Mixxx.ControlProxy {
        group: "[App]"
        key: "num_samplers"

        onInitializedChanged: value = root.numSamplers
    }
    Mixxx.ControlProxy {
        group: "[App]"
        key: "num_preview_decks"

        onInitializedChanged: value = root.numPreviewDecks
    }
    Mixxx.ControlProxy {
        group: "[Waveform]"
        key: "WaveformOverviewType"

        onInitializedChanged: {
            // The C++ side owns this control (read-only, seeded from
            // config at startup). Only write back when the skin setting
            // actually differs, to avoid read-only write warnings.
            if (initialized && value !== root.waveformOverviewTypeRgb) {
                value = root.waveformOverviewTypeRgb;
            }
        }
    }
    Skin.LibraryScanSummaryDialog {
    }
    // Split headphones: the left channel plays the headphones cue, the
    // right one the master (both mono), for a splitter cable
    Mixxx.ControlProxy {
        id: headSplitControl

        group: "[Master]"
        key: "headSplit"
    }

    // A tab of the top bar, like OVERVIEW / BROWSE on a player
    component Tab: Rectangle {
        id: tab

        required property string label
        property bool active: false
        signal clicked

        border.color: active ? "#d4e157" : "#5c6068"
        border.width: active ? 2 : 1
        color: (tabMouse.pressed || tabTap.pressed) ? "#1c1e22" : "#000000"

        height: parent.height
        // Narrow phones: smaller tabs, so that both rows of the top bar fit
        width: root.width < 760
                ? Math.max(44, tabText.implicitWidth + 12)
                : Math.max(root.compact ? 64 : 96, tabText.implicitWidth + 24)

        Text {
            id: tabText

            anchors.centerIn: parent
            color: tab.active ? "#d4e157" : "#9a9ea6"
            font.bold: true
            font.pixelSize: root.width < 760 ? 10 : (root.compact ? 11 : 13)
            text: tab.label
        }
        // Belt and suspenders: Qt on some Android builds delivers the tap
        // to only one handler kind (driver-dependent). Both fire the same
        // signal; the assignment is idempotent so a double fire is harmless
        // (same viewMode twice). Press color listens to both.
        TapHandler {
            id: tabTap

            gesturePolicy: TapHandler.ReleaseWithinBounds
            onTapped: tab.clicked()
        }
        MouseArea {
            id: tabMouse

            anchors.fill: parent
            onClicked: tab.clicked()
        }
    }
    // Left of a waveform: deck, key, bars left, quantize and slip
    component DeckInfo: Item {
        id: deckInfo

        required property string group
        required property int deckNumber
        // Kept in a property, or the player object can be collected
        readonly property var deckPlayer: Mixxx.PlayerManager.getPlayer(group)
        readonly property var currentTrack: deckPlayer?.currentTrack
        readonly property bool loaded: loadedControl.value > 0
        readonly property real rowHeight: Math.max(18, Math.min(34, (height - header.height) / 4))
        // Bars and beats to the end of the track, as a player counts down to
        // the next memory cue
        readonly property string barsLeft: {
            if (!loaded || bpmControl.value <= 0) {
                return "--.-";
            }
            const seconds = durationControl.value * (1 - Math.max(0, Math.min(1, positionControl.value)));
            const beats = Math.floor(seconds * bpmControl.value / 60);
            const bars = Math.floor(beats / 4);
            return (bars < 10 ? "0" : "") + bars + "." + (beats % 4 + 1);
        }

        Mixxx.ControlProxy {
            id: loadedControl

            group: deckInfo.group
            key: "track_loaded"
        }
        Mixxx.ControlProxy {
            id: bpmControl

            group: deckInfo.group
            key: "bpm"
        }
        Mixxx.ControlProxy {
            id: durationControl

            group: deckInfo.group
            key: "duration"
        }
        Mixxx.ControlProxy {
            id: positionControl

            group: deckInfo.group
            key: "playposition"
        }
        Mixxx.ControlProxy {
            id: quantizeControl

            group: deckInfo.group
            key: "quantize"
        }
        Mixxx.ControlProxy {
            id: slipControl

            group: deckInfo.group
            key: "slip_enabled"
        }

        Rectangle {
            id: header

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            color: "#2a2c30"
            height: root.compact ? 18 : 24

            Text {
                anchors.left: parent.left
                anchors.leftMargin: 8
                anchors.verticalCenter: parent.verticalCenter
                color: "#d8dadd"
                font.pixelSize: root.compact ? 10 : 12
                text: qsTr("DECK %1").arg(deckInfo.deckNumber)
            }
            Rectangle {
                anchors.bottom: parent.bottom
                anchors.left: parent.left
                anchors.right: parent.right
                color: deckInfo.deckNumber === 1 ? "#1f6fd1" : "#e8650f"
                height: 2
            }
        }
        Column {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: header.bottom

            Repeater {
                model: [
                    {
                        "text": deckInfo.loaded ? (deckInfo.currentTrack?.keyText || "--") : "--",
                        "color": "#ffffff",
                        "unit": "♯"
                    },
                    {
                        "text": deckInfo.barsLeft,
                        "color": "#ff6a13",
                        "unit": qsTr("Bars")
                    }
                ]

                Rectangle {
                    required property var modelData

                    border.color: "#2a2c30"
                    border.width: 1
                    color: "#141518"
                    height: deckInfo.rowHeight
                    width: parent.width

                    Text {
                        id: rowValue

                        anchors.left: parent.left
                        anchors.leftMargin: 8
                        anchors.verticalCenter: parent.verticalCenter
                        color: modelData.color
                        font.pixelSize: Math.round(deckInfo.rowHeight * 0.55)
                        text: modelData.text
                    }
                    Text {
                        anchors.baseline: rowValue.baseline
                        anchors.left: rowValue.right
                        anchors.leftMargin: 4
                        color: modelData.color
                        font.pixelSize: Math.round(deckInfo.rowHeight * 0.36)
                        text: modelData.unit === "♯" ? "" : modelData.unit
                    }
                }
            }
            Repeater {
                model: [
                    {
                        "label": qsTr("QUANTIZE"),
                        "control": quantizeControl
                    },
                    {
                        "label": qsTr("SLIP"),
                        "control": slipControl
                    }
                ]

                Rectangle {
                    id: toggleRow

                    required property var modelData
                    readonly property bool on: modelData.control.value > 0

                    border.color: on ? "#ff6a13" : "#2a2c30"
                    border.width: 1
                    color: on ? "#3a1a08" : "#141518"
                    height: deckInfo.rowHeight
                    width: parent.width

                    Text {
                        anchors.left: parent.left
                        anchors.leftMargin: 8
                        anchors.verticalCenter: parent.verticalCenter
                        color: toggleRow.on ? "#ff6a13" : "#9a9ea6"
                        font.pixelSize: Math.round(deckInfo.rowHeight * 0.42)
                        text: toggleRow.modelData.label
                    }
                    MouseArea {
                        anchors.fill: parent
                        onClicked: toggleRow.modelData.control.value = toggleRow.on ? 0 : 1
                    }
                }
            }
        }
    }

    Rectangle {
        anchors.fill: parent
        color: "#000000"
    }

    Item {
        id: content

        readonly property int gap: 3

        anchors.fill: parent
        enabled: !settingsPopup.opened

        // ---- Top bar: tabs, deck loaders, settings ----
        Row {
            id: topBar

            anchors.left: parent.left
            anchors.leftMargin: 4
            anchors.top: parent.top
            anchors.topMargin: 3
            height: root.compact ? 26 : 34
            spacing: root.width < 760 ? 3 : 6

            Tab {
                active: root.viewMode === 1
                label: qsTr("WAVE")
                visible: root.itemShown("wave")
                onClicked: root.showView(1)
            }
            Tab {
                active: root.djView
                label: "DJ"
                visible: root.itemShown("dj")
                onClicked: root.showView(3)
            }
            Tab {
                active: root.browsing && root.loadTarget.length === 0
                label: qsTr("BROWSE")
                visible: root.itemShown("lib")
                onClicked: root.toggleLibraryView()
            }
            Tab {
                active: root.browsing && root.loadTarget === "[Channel1]"
                label: qsTr("LOAD %1").arg(1)
                visible: root.itemShown("load1")
                onClicked: root.openQuickLoad("[Channel1]")
            }
            Tab {
                active: root.browsing && root.loadTarget === "[Channel2]"
                label: qsTr("LOAD %1").arg(2)
                visible: root.itemShown("load2")
                onClicked: root.openQuickLoad("[Channel2]")
            }
        }
        Row {
            anchors.right: parent.right
            anchors.rightMargin: 4
            anchors.top: topBar.top
            height: topBar.height
            spacing: 6

            Tab {
                active: headSplitControl.value > 0
                label: "SPLIT"
                visible: root.itemShown("split")
                onClicked: headSplitControl.value = headSplitControl.value > 0 ? 0 : 1
            }
            // Asks before stopping, keeps its time across skin switches
            Skin.MobileRecordButton {
                fontPixelSize: root.width < 760 ? 10 : (root.compact ? 11 : 13)
                height: parent.height
                idleBorderColor: "#5c6068"
                idleColor: "#000000"
                radius: 0
                visible: root.itemShown("rec")
                width: Math.max(root.width < 760 ? 44 : (root.compact ? 64 : 80), implicitWidth)
            }
            Tab {
                active: root.xyView
                label: "XY"
                visible: root.itemShown("xy")
                onClicked: root.xyView ? root.showView(root.preferredView()) : root.showView(4)
            }
            Tab {
                active: root.beatFxShown
                label: "BEAT FX"
                visible: root.compact && !root.browsing && !root.xyView && root.itemShown("fx")
                onClicked: root.beatFxShown = !root.beatFxShown
            }
            Tab {
                active: settingsPopup.opened
                label: qsTr("UTILITY")
                onClicked: settingsPopup.open()
            }
        }

        // ---- Deck displays ----
        Row {
            id: deckPanels

            anchors.bottom: root.djView ? djView.top : parent.bottom
            anchors.bottomMargin: content.gap
            anchors.left: parent.left
            anchors.leftMargin: content.gap
            anchors.right: parent.right
            anchors.rightMargin: content.gap
            height: root.viewMode !== 1 ? (root.compact ? 58 : 84) : Math.round(root.height * (root.compact ? 0.36 : 0.3))
            spacing: content.gap

            BoothDeckPanel {
                deckNumber: 1
                group: "[Channel1]"
                height: parent.height
                slim: root.viewMode !== 1
                width: (parent.width - parent.spacing) / 2
            }
            BoothDeckPanel {
                deckNumber: 2
                group: "[Channel2]"
                height: parent.height
                slim: root.viewMode !== 1
                width: (parent.width - parent.spacing) / 2
            }
        }

        // ---- Touch controls ----
        BoothDjView {
            id: djView

            anchors.bottom: parent.bottom
            anchors.bottomMargin: content.gap
            anchors.left: parent.left
            anchors.leftMargin: content.gap
            anchors.right: parent.right
            anchors.rightMargin: content.gap
            compact: root.compact
            height: Math.round(root.height * (root.compact ? 0.5 : 0.46))
            visible: root.djView
        }

        // ---- Waveforms, the library while browsing, or the XY pad ----
        Item {
            id: middle

            anchors.bottom: deckPanels.top
            anchors.bottomMargin: content.gap
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: topBar.bottom
            anchors.topMargin: content.gap

            Item {
                anchors.fill: parent
                visible: !root.browsing && !root.xyView

                Column {
                    id: deckInfos

                    anchors.bottom: parent.bottom
                    anchors.left: parent.left
                    anchors.leftMargin: content.gap
                    anchors.top: parent.top
                    spacing: content.gap
                    visible: !(root.djView && root.compact)
                    width: visible ? (root.compact ? 86 : 124) : 0

                    DeckInfo {
                        deckNumber: 1
                        group: "[Channel1]"
                        height: (parent.height - parent.spacing) / 2
                        width: parent.width
                    }
                    DeckInfo {
                        deckNumber: 2
                        group: "[Channel2]"
                        height: (parent.height - parent.spacing) / 2
                        width: parent.width
                    }
                }
                Column {
                    anchors.bottom: parent.bottom
                    anchors.left: deckInfos.right
                    anchors.leftMargin: content.gap
                    anchors.right: beatFx.visible ? beatFx.left : parent.right
                    anchors.rightMargin: content.gap
                    anchors.top: parent.top

                    Skin.WaveformDisplay {
                        group: "[Channel1]"
                        height: parent.height / 2
                        isTouchMode: true
                        showHud: false
                        width: parent.width
                    }
                    Skin.WaveformDisplay {
                        group: "[Channel2]"
                        height: parent.height / 2
                        isTouchMode: true
                        showHud: false
                        width: parent.width
                    }
                }
                BoothBeatFx {
                    id: beatFx

                    // The DJ view leaves less height
                    compact: root.djView
                    anchors.bottom: parent.bottom
                    anchors.right: parent.right
                    anchors.rightMargin: content.gap
                    anchors.top: parent.top
                    visible: !root.compact && root.itemShown("fx") && root.beatFxPanelShown
                    width: 180
                }
            }

            // The browser stays loaded once opened, like the library of the
            // Classic skin
            Loader {
                id: library

                property bool keepLoaded: false

                anchors.fill: parent
                active: keepLoaded || root.browsing
                visible: root.browsing
                onLoaded: keepLoaded = true

                sourceComponent: Component {
                    BoothBrowse {
                        acceptsController: !settingsPopup.opened
                        anchors.fill: parent
                        compact: root.compact
                        loadTarget: root.loadTarget
                    }
                }
            }

            // The XY pad of the Classic skin, in the colors of this one
            Skin.XYPad {
                anchors.fill: parent
                accentColor: "#2f9bff"
                border.color: "#2a2c30"
                buttonBorderColor: "#3a3c40"
                buttonColor: "#141518"
                buttonTextColor: "#9a9ea6"
                closable: false
                color: "#0c0d0f"
                controlRadius: 2
                deck1Color: "#1f6fd1"
                deck2Color: "#e8650f"
                holdColor: "#ff6a13"
                radius: 0
                surfaceBorderColor: "#2a2c30"
                surfaceColor: "#000000"
                surfaceRadius: 0
                title: qsTr("XY PAD")
                visible: root.xyView
                wetColor: "#d4e157"
            }

            // BEAT FX of phones, over the waveforms
            BoothBeatFx {
                anchors.bottom: parent.bottom
                anchors.right: parent.right
                anchors.rightMargin: content.gap
                anchors.top: parent.top
                compact: true
                visible: root.compact && root.beatFxShown && !root.browsing && !root.xyView && root.itemShown("fx")
                width: 190
                z: 10
            }
        }
    }

    Skin.Settings {
        id: settingsPopup

        backgroundColor: "#000000"
        headerColor: "#1c1e22"
        title: qsTr("UTILITY")
        height: root.applicationWindow.isMobile ? parent.height : Math.min(840, parent.height)
        modal: true
        width: root.applicationWindow.isMobile ? parent.width : Math.min(1400, parent.width)
        x: root.applicationWindow.isMobile ? 0 : Math.round((parent.width - width) / 2)
        y: root.applicationWindow.isMobile ? 0 : Math.round((parent.height - height) / 2)

        Overlay.modal: Rectangle {
            color: "#99000000"
        }
    }

    // ================= CONTROLLER ACCESS TO THE SCREENS =================
    // The same [Skin] controls as the Classic skin (see its MainWindow.qml):
    // mobile_library_toggle switches BROWSE, mobile_fx_toggle BEAT FX,
    // mobile_loader_deck1/2 open the browser for a deck, and LOAD does that
    // too when the browser isn't shown.
    property bool mobileControlsCreated: false

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
                root.toggleLibraryView();
            }
        }
    }
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
                root.toggleBeatFx();
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
                    if (!library.visible) {
                        Qt.callLater(root.openQuickLoad, deckLoaderAccess.group);
                    }
                }
            }
        }
    }
}

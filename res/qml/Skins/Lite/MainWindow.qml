import "../.." as Skin
import "../Booth" as Booth
import Mixxx 1.0 as Mixxx
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Lite skin: minimal drawing for slow devices and battery saving. Two decks,
// no waveforms (5/s position bar), flat buttons/Canvas knobs, no FX/XY pad.
// BROWSE loads Booth browser only while shown.
Item {
    id: root

    required property ApplicationWindow applicationWindow
    property var menuBar: null

// Two decks on start; 16 samplers (controllers map to them, idle cost is negligible).
    readonly property int numDecks: 2
    readonly property int numSamplers: 16
    readonly property int numPreviewDecks: 1

property int viewMode: 1 // 1=DECKS, 2=BROWSE
    readonly property bool browsing: viewMode === 2
    // Deck that opened the browser via LOAD 1/2. Loading a track returns to decks.
    property string loadTarget: ""
    readonly property var loadTargetPlayer: loadTarget.length > 0 ? Mixxx.PlayerManager.getPlayer(loadTarget) : null
    readonly property bool compact: height < 520

// Shared colours for library, settings, popups. See Theme/+android/Theme.qml
    readonly property var skinTheme: ({
            "accentColor": "#2f6fb4",
            "backgroundColor": "#0b0c0e",
            "sunkenBackgroundColor": "#0e0f11",
            "darkGray": "#0b0c0e",
            "darkGray2": "#1b1d21",
            "darkGray3": "#2a2c30",
            "darkGray4": "#141518",
            "textColor": "#e6e8eb",
            "panelSplitterBackground": "#0b0c0e",
            "libraryScrollHandle": "#3a3c40",
            "libraryScrollHandleHovered": "#5c6068",
            "libraryScrollHandlePressed": "#d8dadd",
            "libraryScrollTrack": "#0b0c0e",
            "libraryActionBackground": "#1b1d21",
            "libraryActionDisabledBackground": "#111214",
            "libraryActionText": "#ffffff",
            "libraryActionDisabledText": "#5c6068",
            "libraryDeck1Background": "#2f6fb4",
            "libraryDeck1Pressed": "#5a9be8",
            "libraryDeck2Background": "#d8661a",
            "libraryDeck2Pressed": "#ff9a52",
            "libraryConfirmBackground": "#2f6fb4",
            "libraryKeyMatchColor": "#3ad33a",
            "libraryKeyMatchBackground": "#1a3ad33a"
        })

// Top bar tabs (Settings → Interface → TOP BAR can hide these).
    readonly property var topBarItems: [
        {
            "name": "dj",
            "label": qsTr("DECKS"),
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
            "name": "rec",
            "label": "REC",
            "view": false
        }
    ]
    readonly property var hiddenTopBarItems: Mixxx.MobileApp.hiddenTopBarItems

    function itemShown(name) {
        return root.hiddenTopBarItems.indexOf(name) < 0;
    }
    // The deck loader of the Classic skin is the browser here
    function openQuickLoad(group) {
        root.loadTarget = (group && group.length > 0) ? group : "[Channel1]";
        root.viewMode = 2;
    }
    function showDecks() {
        root.loadTarget = "";
        root.viewMode = 1;
    }
    function toggleLibraryView() {
        if (root.browsing && root.loadTarget.length === 0) {
            root.showDecks();
        } else {
            root.loadTarget = "";
            root.viewMode = 2;
        }
    }

    Connections {
        function onTrackChanged() {
            if (root.browsing) {
                root.showDecks();
            }
        }

        ignoreUnknownSignals: true
        target: root.loadTargetPlayer
    }

// ---- Engine setup ----
    Mixxx.ControlProxy {
        group: "[App]"
        key: "num_decks"

        onInitializedChanged: value = Math.max(value, root.numDecks)
    }
    Mixxx.ControlProxy {
        group: "[App]"
        key: "num_samplers"

        onInitializedChanged: value = Math.max(value, root.numSamplers)
    }
    Mixxx.ControlProxy {
        group: "[App]"
        key: "num_preview_decks"

        onInitializedChanged: value = Math.max(value, root.numPreviewDecks)
    }
    Skin.LibraryScanSummaryDialog {
    }

component Tab: LiteButton {
        Layout.fillHeight: true
        Layout.preferredWidth: Math.max(root.compact ? 64 : 92, implicitWidth)
        fontPixelSize: root.compact ? 12 : 14
        litColor: "#d4e157"
    }

    Rectangle {
        anchors.fill: parent
        color: "#0b0c0e"
    }

    Item {
        id: content

        anchors.fill: parent
        anchors.margins: 4
        enabled: !settingsPopup.opened

        // ---- Top bar ----
        RowLayout {
            id: topBar

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            height: root.compact ? 32 : 40
            spacing: 4

            Tab {
                lit: !root.browsing
                text: qsTr("DECKS")
                visible: root.itemShown("dj")
                onClicked: root.showDecks()
            }
            Tab {
                lit: root.browsing && root.loadTarget.length === 0
                text: qsTr("BROWSE")
                visible: root.itemShown("lib")
                onClicked: root.toggleLibraryView()
            }
            Tab {
                lit: root.browsing && root.loadTarget === "[Channel1]"
                text: qsTr("LOAD %1").arg(1)
                visible: root.itemShown("load1")
                onClicked: root.openQuickLoad("[Channel1]")
            }
            Tab {
                lit: root.browsing && root.loadTarget === "[Channel2]"
                text: qsTr("LOAD %1").arg(2)
                visible: root.itemShown("load2")
                onClicked: root.openQuickLoad("[Channel2]")
            }
            Item {
                Layout.fillWidth: true
            }
// Stem separation progress
            Tab {
                lit: Mixxx.Stems.hold !== ""
                text: Mixxx.Stems.hold !== "" ? `🎚 ⏸ ${Mixxx.Stems.pendingCount}` : `🎚 ${Math.round(Mixxx.Stems.progress * 100)}%`
                visible: Mixxx.Stems.pendingCount > 0
                onClicked: stemsOverlay.openQueue()
            }
Skin.MobileRecordButton {
                Layout.fillHeight: true
                Layout.preferredWidth: Math.max(root.compact ? 64 : 80, implicitWidth)
                fontPixelSize: root.compact ? 12 : 14
                idleBorderColor: "#3a3d44"
                idleColor: "#1b1d21"
                radius: 3
                visible: root.itemShown("rec")
            }
            Tab {
                lit: settingsPopup.opened
                text: qsTr("UTILITY")
                onClicked: settingsPopup.open()
            }
        }

// ---- Decks and mixer ----
        RowLayout {
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: topBar.bottom
            anchors.topMargin: 4
            spacing: 6
            visible: !root.browsing

            LiteDeck {
                Layout.fillHeight: true
                Layout.fillWidth: true
                deckColor: "#2f9bff"
                deckNumber: 1
                faderOnLeft: true
                group: "[Channel1]"
            }
            LiteMixer {
                Layout.fillHeight: true
                Layout.preferredWidth: Math.round(Math.min(root.width * 0.26, root.compact ? 200 : 300))
            }
            LiteDeck {
                Layout.fillHeight: true
                Layout.fillWidth: true
                deckColor: "#ff7a1f"
                deckNumber: 2
                faderOnLeft: false
                group: "[Channel2]"
            }
        }

// ---- Booth browser (only while shown) ----
        Loader {
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: topBar.bottom
            anchors.topMargin: 4
            active: root.browsing

            sourceComponent: Component {
                Booth.BoothBrowse {
                    acceptsController: !settingsPopup.opened
                    compact: root.compact
                    loadTarget: root.loadTarget
                }
            }
        }
    }

    // The queue of the stem separation and its notices
    Skin.StemsOverlay {
        id: stemsOverlay

        anchors.fill: parent
        z: 95
    }

    Skin.Settings {
        id: settingsPopup

        backgroundColor: "#0b0c0e"
        headerColor: "#1b1d21"
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

// ================= [Skin] Controls =================
    // Same as other skins: mobile_library_toggle, mobile_loader_deck1/2,
    // mobile_fx_toggle (created for mappings; this skin has no FX panel).
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
    Mixxx.SkinControlCreator {
        group: "[Skin]"
        key: "mobile_fx_toggle"
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
                    if (!root.browsing) {
                        Qt.callLater(root.openQuickLoad, deckLoaderAccess.group);
                    }
                }
            }
        }
    }
}

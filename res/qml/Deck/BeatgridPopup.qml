import Mixxx 1.0 as Mixxx
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import ".." as Skin
import "../Theme"

// Corrects the beatgrid of the track loaded in a deck, e.g. when the
// analyzer put the beats on the off-beats and quantized play starts half a
// beat off. The changes are saved with the track.
Popup {
    id: root

    required property string group
    readonly property bool trackLoaded: trackLoadedProxy.value > 0
    readonly property bool locked: bpmLockProxy.value > 0
    readonly property bool editable: root.trackLoaded && !root.locked
    // Kept in a property, or the player object can be collected
    readonly property var player: Mixxx.PlayerManager.getPlayer(root.group)

    // Pressing a button of the grid controls: they act when set to 1
    component GridButton: Skin.Button {
        id: gridButton

        required property string key

        Layout.fillWidth: true
        Layout.preferredHeight: 44
        activeColor: Theme.accentColor
        backgroundColor: "#2e3440"
        normalColor: "#ffffff"
        enabled: root.editable
        font.bold: true
        font.pixelSize: 12

        Mixxx.ControlProxy {
            id: gridControl

            group: root.group
            key: gridButton.key
        }

        onClicked: {
            gridControl.value = 1;
            gridControl.value = 0;
        }
    }

    parent: Overlay.overlay
    anchors.centerIn: Overlay.overlay
    width: Math.min(440, (parent ? parent.width : 440) - 20)
    modal: true
    focus: true
    padding: 12
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    background: Rectangle {
        color: Theme.darkGray2
        border.color: Theme.darkGray3
        radius: 8
    }

    Mixxx.ControlProxy {
        id: trackLoadedProxy

        group: root.group
        key: "track_loaded"
    }
    Mixxx.ControlProxy {
        id: bpmLockProxy

        group: root.group
        key: "bpmlock"
    }
    Mixxx.ControlProxy {
        id: bpmProxy

        group: root.group
        key: "file_bpm"
    }
    Mixxx.ControlProxy {
        id: undoPossibleProxy

        group: root.group
        key: "beats_undo_possible"
    }
    Mixxx.ControlProxy {
        id: playProxy

        group: root.group
        key: "play"
    }

    contentItem: ColumnLayout {
        spacing: 8

        RowLayout {
            Layout.fillWidth: true

            Text {
                Layout.fillWidth: true
                color: Theme.white
                elide: Text.ElideRight
                font.bold: true
                font.pixelSize: 15
                text: qsTr("Beatgrid – Deck %1").arg(root.group.replace(/\D/g, ""))
            }
            Text {
                color: Theme.lightGray2
                font.pixelSize: 13
                text: root.trackLoaded ? qsTr("%1 BPM").arg(Number(bpmProxy.value).toFixed(2)) : ""
            }
        }

        Text {
            Layout.fillWidth: true
            color: Theme.lightGray2
            font.pixelSize: 11
            text: !root.trackLoaded ? qsTr("Load a track first.") : qsTr("If quantized play or cues start between the beats, the grid is off: try ½ BEAT first. To place it exactly, stop on a downbeat (e.g. the first kick) and tap SET DOWNBEAT HERE. Changes are saved with the track.")
            wrapMode: Text.WordWrap
        }

        // A locked BPM ignores all grid changes
        Skin.Button {
            Layout.fillWidth: true
            Layout.preferredHeight: 40
            visible: root.trackLoaded && root.locked
            activeColor: "#ffab40"
            backgroundColor: "#3a2a14"
            normalColor: "#ffd180"
            font.bold: true
            text: qsTr("🔒 Grid locked – tap to unlock")
            onClicked: bpmLockProxy.value = 0
        }

        GridLayout {
            Layout.fillWidth: true
            columns: 2
            columnSpacing: 6
            rowSpacing: 6

            GridButton {
                key: "beats_translate_half"
                text: qsTr("½ BEAT")
            }
            GridButton {
                key: "beats_translate_curpos"
                text: qsTr("SET DOWNBEAT HERE")
            }
            GridButton {
                key: "beats_translate_earlier"
                arrowLeft: true
                text: qsTr("EARLIER")
            }
            GridButton {
                key: "beats_translate_later"
                arrowRight: true
                text: qsTr("LATER")
            }
            GridButton {
                key: "beats_set_halve"
                text: qsTr("BPM ÷2")
            }
            GridButton {
                key: "beats_set_double"
                text: qsTr("BPM ×2")
            }
            GridButton {
                key: "beats_translate_match_alignment"
                text: qsTr("MATCH OTHER DECK")
            }
            GridButton {
                key: "beats_undo_adjustment"
                enabled: root.editable && undoPossibleProxy.value > 0
                text: qsTr("UNDO")
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 6

            Skin.Button {
                Layout.fillWidth: true
                Layout.preferredHeight: 40
                visible: root.trackLoaded && !root.locked
                activeColor: "#ffab40"
                font.bold: true
                text: qsTr("🔒 LOCK GRID")
                onClicked: bpmLockProxy.value = 1
            }
            // Analyzes the track again, e.g. to get the grid lined up with
            // the kicks by the current analyzer; only while the deck is stopped
            Skin.Button {
                Layout.fillWidth: true
                Layout.preferredHeight: 40
                visible: root.trackLoaded && !root.locked
                enabled: playProxy.value === 0
                activeColor: Theme.accentColor
                font.bold: true
                text: qsTr("🔄 RE-ANALYZE")
                onClicked: Mixxx.MobileLibrary.reanalyzeBeats(root.player.currentTrack)
            }
            Skin.Button {
                Layout.fillWidth: true
                Layout.preferredHeight: 40
                activeColor: Theme.accentColor
                font.bold: true
                text: qsTr("CLOSE")
                onClicked: root.close()
            }
        }
    }
}

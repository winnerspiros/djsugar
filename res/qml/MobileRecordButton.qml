import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Mixxx 1.0 as Mixxx
import "." as Skin
import "Theme"

// REC of the top bar of the Android skins. It starts recording the mix at
// once and asks before stopping it, as a tap by mistake would end the
// recording of a set. The time comes from the recording manager, so it stays
// right after switching the skin.
Rectangle {
    id: root

    readonly property bool recording: statusControl.value > 0
    property int fontPixelSize: 10
    property color idleColor: "#222733"
    property color idleBorderColor: "#374151"

    implicitHeight: 30
    implicitWidth: Math.max(recording ? 76 : 50, recRow.implicitWidth + 16)
    radius: 4
    color: recording ? "#b71c1c" : (recMouse.pressed ? "#2e2124" : idleColor)
    border.color: recording ? "#ff5252" : idleBorderColor
    border.width: 1

    Mixxx.ControlProxy {
        id: statusControl

        group: "[Recording]"
        key: "status"
    }
    Mixxx.ControlProxy {
        id: toggleControl

        group: "[Recording]"
        key: "toggle_recording"
    }
    Timer {
        id: blinkTimer

        property bool blink: false

        interval: 500
        repeat: true
        running: root.recording

        onTriggered: blink = !blink
    }

    RowLayout {
        id: recRow

        anchors.centerIn: parent
        spacing: 4

        Rectangle {
            Layout.preferredHeight: 8
            Layout.preferredWidth: 8
            radius: 4
            color: root.recording ? (blinkTimer.blink ? "#ff1744" : "#ff8a80") : "#78909c"
        }
        Text {
            text: root.recording ? Mixxx.Recording.durationText : "REC"
            font.bold: true
            font.pixelSize: root.fontPixelSize
            color: root.recording ? "#ffffff" : "#b0bec5"
        }
    }

    MouseArea {
        id: recMouse

        anchors.fill: parent
        onClicked: {
            if (root.recording) {
                stopPopup.open();
            } else {
                toggleControl.trigger();
            }
        }
    }

    Popup {
        id: stopPopup

        anchors.centerIn: Overlay.overlay
        width: Math.min(440, Overlay.overlay ? Overlay.overlay.width - 40 : 440)
        modal: true
        dim: true
        padding: 18

        background: Rectangle {
            color: "#181b24"
            radius: 10
            border.color: "#ff5252"
            border.width: 1
        }

        contentItem: ColumnLayout {
            spacing: 12

            Text {
                Layout.fillWidth: true
                text: qsTr("Stop the recording?")
                font.bold: true
                font.pixelSize: 15
                color: Theme.white
                wrapMode: Text.WordWrap
            }
            Text {
                Layout.fillWidth: true
                text: qsTr("%1 recorded. It is in REC of the library.").arg(Mixxx.Recording.durationText)
                font.pixelSize: 12
                color: "#b0bec5"
                wrapMode: Text.WordWrap
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 10

                Skin.Button {
                    Layout.preferredHeight: 44
                    Layout.preferredWidth: 140
                    text: qsTr("Cancel")
                    onClicked: stopPopup.close()
                }
                Item {
                    Layout.fillWidth: true
                }
                Skin.Button {
                    Layout.preferredHeight: 44
                    Layout.preferredWidth: 140
                    activeColor: "#ff5252"
                    backgroundColor: "#4a1c1c"
                    normalColor: "#ff8a80"
                    font.bold: true
                    text: qsTr("STOP")
                    onClicked: {
                        if (root.recording) {
                            toggleControl.trigger();
                        }
                        stopPopup.close();
                    }
                }
            }
        }
    }
}

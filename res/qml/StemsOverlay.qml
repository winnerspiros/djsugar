import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Mixxx 1.0 as Mixxx
import "." as Skin
import "Theme"
import "stems.js" as StemsText

// The queue of the stem separation and the notice when a track was
// separated, shared by the skins (Android). The skin shows its own
// indicator, which calls openQueue().
Item {
    id: root

    function openQueue() {
        stemsQueuePopup.open();
    }

    // The queue of the stem separation
    Popup {
        id: stemsQueuePopup

        anchors.centerIn: parent
        width: Math.min(640, parent.width - 20)
        height: Math.min(stemsQueueColumn.implicitHeight + 40, parent.height - 20)
        modal: true
        padding: 16

        background: Rectangle {
            color: "#14171f"
            radius: 10
            border.color: "#2a3648"
            border.width: 1
        }

        Flickable {
            anchors.fill: parent
            contentHeight: stemsQueueColumn.implicitHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds

            ColumnLayout {
                id: stemsQueueColumn

                width: parent.width
                spacing: 10

                RowLayout {
                    Layout.fillWidth: true

                    Text {
                        Layout.fillWidth: true
                        text: qsTr("🎚 Stems queue")
                        color: "#ffffff"
                        font.bold: true
                        font.pixelSize: 15
                    }
                    Skin.Button {
                        implicitWidth: 90
                        implicitHeight: 32
                        activeColor: Theme.accentColor
                        backgroundColor: "#2e3440"
                        normalColor: "#ffffff"
                        font.bold: true
                        text: qsTr("CLOSE")
                        onClicked: stemsQueuePopup.close()
                    }
                }

                Skin.StemsQueue {
                    Layout.fillWidth: true
                }
            }
        }
    }

    // Tells when a track was separated into stems
    Rectangle {
        id: stemsToast

        property string text: ""
        property bool isError: false

        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 60
        z: 95
        visible: opacity > 0
        opacity: stemsToastTimer.running ? 1 : 0
        width: Math.min(560, parent.width - 40)
        implicitHeight: stemsToastText.implicitHeight + 20
        radius: 8
        color: isError ? "#4a1e1e" : "#1e3a2a"
        border.color: isError ? "#ef5350" : "#66bb6a"
        border.width: 1

        Behavior on opacity {
            NumberAnimation {
                duration: 250
            }
        }

        Text {
            id: stemsToastText

            anchors.fill: parent
            anchors.margins: 10
            wrapMode: Text.WordWrap
            color: "#ffffff"
            font.pixelSize: 12
            text: stemsToast.text
        }

        MouseArea {
            anchors.fill: parent
            onClicked: stemsToastTimer.stop()
        }

        Timer {
            id: stemsToastTimer

            interval: 5000
        }

        Connections {
            target: Mixxx.Stems

            function onTrackFinished(location, name, error) {
                if (error === "canceled")
                    return;
                stemsToast.isError = error !== "";
                stemsToast.text = error === ""
                        ? qsTr("🎚 Stems ready: %1. If it is loaded on a deck, load it again to use them.").arg(name)
                        : qsTr("🎚 %1 couldn't be separated: %2").arg(name).arg(StemsText.errorText(error));
                stemsToastTimer.restart();
            }
        }
    }
}

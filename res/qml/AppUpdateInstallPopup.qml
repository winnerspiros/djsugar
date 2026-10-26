import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Mixxx 1.0 as Mixxx
import "." as Skin
import "Theme"

// Asks before opening the Android installer of the downloaded update:
// Mixxx closes while it is installed
Popup {
    id: root

    // Android first asked for the permission to install apps from Mixxx
    property bool needsPermission: false

    anchors.centerIn: Overlay.overlay
    width: Math.min(480, Overlay.overlay ? Overlay.overlay.width - 40 : 480)
    modal: true
    dim: true
    padding: 18

    onAboutToShow: root.needsPermission = false

    background: Rectangle {
        color: "#181b24"
        radius: 10
        border.color: "#00e5ff"
        border.width: 1
    }

    contentItem: ColumnLayout {
        spacing: 12

        Text {
            Layout.fillWidth: true
            text: qsTr("Install Mixxx %1?").arg(Mixxx.AppUpdate.latestVersion)
            font.bold: true
            font.pixelSize: 15
            color: Theme.white
            wrapMode: Text.WordWrap
        }

        Text {
            Layout.fillWidth: true
            text: qsTr("Mixxx closes while Android installs the update, so the music stops. Your library and settings are kept.")
            font.pixelSize: 12
            color: "#b0bec5"
            wrapMode: Text.WordWrap
        }

        Text {
            Layout.fillWidth: true
            visible: root.needsPermission
            text: qsTr("Android has opened its setting to install apps from Mixxx: allow it, come back and press INSTALL again.")
            font.pixelSize: 12
            color: "#ffcc80"
            wrapMode: Text.WordWrap
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 10

            Skin.Button {
                Layout.preferredHeight: 44
                Layout.preferredWidth: 140
                text: qsTr("Cancel")
                onClicked: root.close()
            }

            Item {
                Layout.fillWidth: true
            }

            Skin.Button {
                Layout.preferredHeight: 44
                Layout.preferredWidth: 140
                activeColor: "#00e5ff"
                backgroundColor: "#0d3640"
                normalColor: "#80deea"
                font.bold: true
                text: qsTr("INSTALL")
                onClicked: {
                    if (Mixxx.AppUpdate.install()) {
                        root.close();
                    } else {
                        root.needsPermission = true;
                    }
                }
            }
        }
    }
}

import QtQuick
import QtQuick.Layouts
import QtQuick.Window
import Mixxx 1.0 as Mixxx
import "." as Skin

// A small bar at the top of the screen when the check at startup found a
// newer version. It never blocks the decks: the update is only downloaded
// and installed when the user asks, and the bar can be closed at any time.
Rectangle {
    id: root

    readonly property var update: Mixxx.AppUpdate
    // Shown from the check at startup until the user closes it
    property bool active: false

    signal installRequested

    visible: active && update.state !== Mixxx.AppUpdate.Idle && update.state !== Mixxx.AppUpdate.Checking && update.state !== Mixxx.AppUpdate.UpToDate
    // In a Loader, which takes the size of the bar
    width: Math.min(720, Window.width - 24)
    height: 48
    radius: 8
    color: "#e6181b24"
    border.color: "#00e5ff"
    border.width: 1

    Connections {
        target: root.update
        function onStateChanged() {
            if (root.update.state === Mixxx.AppUpdate.Available && root.update.automaticCheck && !root.update.latestSkipped) {
                root.active = true;
            }
        }
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 14
        anchors.rightMargin: 6
        spacing: 8

        Text {
            text: "⬆"
            font.pixelSize: 16
            color: "#00e5ff"
        }

        Skin.AppUpdateStatusText {
            Layout.fillWidth: true
            elide: Text.ElideRight
            maximumLineCount: 2
            font.pixelSize: 12
        }

        Skin.Button {
            Layout.preferredHeight: 36
            Layout.preferredWidth: 110
            visible: root.update.state === Mixxx.AppUpdate.Available
            text: qsTr("Skip")
            onClicked: {
                root.update.skipLatest();
                root.active = false;
            }
        }

        Skin.Button {
            Layout.preferredHeight: 36
            Layout.preferredWidth: 130
            visible: root.update.state === Mixxx.AppUpdate.Available || root.update.state === Mixxx.AppUpdate.Downloaded
            activeColor: "#00e5ff"
            normalColor: "#80deea"
            font.bold: true
            text: root.update.state === Mixxx.AppUpdate.Downloaded ? qsTr("INSTALL") : qsTr("DOWNLOAD")
            onClicked: {
                if (root.update.state === Mixxx.AppUpdate.Downloaded) {
                    root.installRequested();
                } else {
                    root.update.download();
                }
            }
        }

        // Closes the bar; a download goes on (see Settings → About)
        Skin.Button {
            Layout.preferredHeight: 36
            Layout.preferredWidth: 40
            // The fonts of Android have no "✕"
            text: "×"
            font.pixelSize: 20
            onClicked: root.active = false
        }
    }
}

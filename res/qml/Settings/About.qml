import QtQuick
import QtQuick.Layouts
import QtQuick.Controls
import Mixxx 1.0 as Mixxx
import ".." as Skin
import "../Theme"

Category {
    id: root

    label: qsTr("About")
    selectedIndex: 0

    Flickable {
        anchors.fill: parent
        contentHeight: aboutColumn.implicitHeight + 30
        clip: true
        boundsBehavior: Flickable.StopAtBounds

        ColumnLayout {
            id: aboutColumn
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.margins: 12
            spacing: 12

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 96
                radius: 8
                color: "#161b24"
                border.color: "#2a3648"
                border.width: 1

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 4

                    Text {
                        Layout.fillWidth: true
                        color: "#ffffff"
                        font.bold: true
                        font.pixelSize: 20
                        text: "DJSugar"
                    }
                    Text {
                        Layout.fillWidth: true
                        color: "#9a9ea6"
                        font.pixelSize: 13
                        text: qsTr("Mixxx %1 (package %2)").arg(Mixxx.Application.version).arg(Mixxx.MobileApp.packageVersion)
                    }
                }
            }

            Skin.Button {
                Layout.fillWidth: true
                Layout.preferredHeight: 44
                text: qsTr("Mixxx homepage")
                onClicked: Qt.openUrlExternally("https://mixxx.org")
            }
            Skin.Button {
                Layout.fillWidth: true
                Layout.preferredHeight: 44
                text: qsTr("Manual")
                onClicked: Qt.openUrlExternally("https://docs.mixxx.org")
            }
        }
    }
}

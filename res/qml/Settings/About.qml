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

// ================= UPDATES =================
            Rectangle {
                Layout.fillWidth: true
                radius: 8
                color: "#161b24"
                border.color: "#2a3648"
                border.width: 1
                implicitHeight: updatesCol.implicitHeight + 24

                ColumnLayout {
                    id: updatesCol
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 8

                    RowLayout {
                        spacing: 8
                        Text {
                            text: "⬆"
                            font.pixelSize: 15
                            color: "#00e5ff"
                        }
                        Text {
                            text: qsTr("UPDATES")
                            font.bold: true
                            font.pixelSize: 12
                            color: "#00e5ff"
                            font.letterSpacing: 0.5
                        }
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 10

                        Skin.AppUpdateStatusText {
                            Layout.fillWidth: true
                            font.pixelSize: 12
                        }

                        Skin.Button {
                            Layout.preferredHeight: 40
                            Layout.preferredWidth: 150
                            visible: [Mixxx.AppUpdate.Idle, Mixxx.AppUpdate.UpToDate].includes(Mixxx.AppUpdate.state) || (Mixxx.AppUpdate.state === Mixxx.AppUpdate.Failed && Mixxx.AppUpdate.latestVersion === "")
                            text: qsTr("CHECK NOW")
                            onClicked: Mixxx.AppUpdate.check()
                        }
                        Skin.Button {
                            Layout.preferredHeight: 40
                            Layout.preferredWidth: 150
                            visible: Mixxx.AppUpdate.state === Mixxx.AppUpdate.Available || (Mixxx.AppUpdate.state === Mixxx.AppUpdate.Failed && Mixxx.AppUpdate.latestVersion !== "")
                            activeColor: "#00e5ff"
                            normalColor: "#80deea"
                            font.bold: true
                            text: qsTr("DOWNLOAD")
                            onClicked: Mixxx.AppUpdate.download()
                        }
                        Skin.Button {
                            Layout.preferredHeight: 40
                            Layout.preferredWidth: 150
                            visible: Mixxx.AppUpdate.state === Mixxx.AppUpdate.Downloading
                            text: qsTr("Cancel")
                            onClicked: Mixxx.AppUpdate.cancelDownload()
                        }
                        Skin.Button {
                            Layout.preferredHeight: 40
                            Layout.preferredWidth: 150
                            visible: Mixxx.AppUpdate.state === Mixxx.AppUpdate.Downloaded
                            activeColor: "#00e5ff"
                            normalColor: "#80deea"
                            font.bold: true
                            text: qsTr("INSTALL")
                            onClicked: updateInstallPopup.open()
                        }
                    }

                    // Download progress
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 6
                        visible: Mixxx.AppUpdate.state === Mixxx.AppUpdate.Downloading
                        radius: 3
                        color: "#0e1117"
                        border.color: "#283444"
                        border.width: 1

                        Rectangle {
                            anchors.left: parent.left
                            anchors.top: parent.top
                            anchors.bottom: parent.bottom
                            width: parent.width * Mixxx.AppUpdate.progress
                            radius: 3
                            color: "#00e5ff"
                        }
                    }

                    // What's new
                    Text {
                        Layout.fillWidth: true
                        visible: Mixxx.AppUpdate.releaseNotes !== "" && [Mixxx.AppUpdate.Available, Mixxx.AppUpdate.Downloading, Mixxx.AppUpdate.Verifying, Mixxx.AppUpdate.Downloaded].includes(Mixxx.AppUpdate.state)
                        text: Mixxx.AppUpdate.releaseNotes
                        textFormat: Text.MarkdownText
                        font.pixelSize: 11
                        color: "#b0bec5"
                        linkColor: "#80deea"
                        wrapMode: Text.WordWrap
                        onLinkActivated: link => Qt.openUrlExternally(link)
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 10

                        Switch {
                            id: autoCheckSwitch

                            checked: Mixxx.AppUpdate.autoCheck
                            onToggled: Mixxx.AppUpdate.autoCheck = checked

                            // The Basic style is grey on and off
                            indicator: Rectangle {
                                x: autoCheckSwitch.leftPadding
                                y: (autoCheckSwitch.height - height) / 2
                                implicitWidth: 48
                                implicitHeight: 26
                                radius: 13
                                color: autoCheckSwitch.checked ? "#0d3640" : "#1a202c"
                                border.color: autoCheckSwitch.checked ? "#00e5ff" : "#546e7a"
                                border.width: 1

                                Rectangle {
                                    x: autoCheckSwitch.checked ? parent.width - width - 3 : 3
                                    y: 3
                                    width: 20
                                    height: 20
                                    radius: 10
                                    color: autoCheckSwitch.checked ? "#00e5ff" : "#78909c"
                                }
                            }
                        }
                        Text {
                            Layout.fillWidth: true
                            text: qsTr("Look for updates when Mixxx starts (once a day at most). The versions are read from the GitHub releases of DJ Sugar, and nothing is downloaded without asking.")
                            font.pixelSize: 10
                            color: "#90a4ae"
                            wrapMode: Text.WordWrap
                        }
                    }
                }

            }

            Skin.AppUpdateInstallPopup {
                id: updateInstallPopup
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

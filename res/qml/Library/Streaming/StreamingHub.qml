import QtQuick
import QtQuick.Controls 2.15
import QtQuick.Layouts
import Mixxx 1.0 as Mixxx
import "../../Theme"
import "../.." as Skin

Item {
    id: root

    property string targetDeck: "[Channel1]"
    property int activePlatform: 0 // 0: Navidrome, 1: Audius, 2: URL, 3: YouTube

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // ================= MULTI-PLATFORM SELECTOR BAR =================
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 46
            color: "#12151c"
            border.color: "#222734"
            border.width: 1

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 8
                anchors.rightMargin: 8
                spacing: 8

                // Platforms tabs
                RowLayout {
                    Layout.fillHeight: true
                    spacing: 8

                    Skin.Button {
                        Layout.fillHeight: true
                        Layout.preferredWidth: 170
                        activeColor: "#00e5ff"
                        backgroundColor: root.activePlatform === 0 ? "#00363a" : "#1a212d"
                        normalColor: root.activePlatform === 0 ? "#00e5ff" : "#80d8ff"
                        font.bold: true
                        font.pixelSize: 11
                        text: "☁️ NAVIDROME / SUBSONIC"
                        onClicked: root.activePlatform = 0
                    }

                    Skin.Button {
                        Layout.fillHeight: true
                        Layout.preferredWidth: 130
                        activeColor: "#ea80fc"
                        backgroundColor: root.activePlatform === 1 ? "#4a148c" : "#1a212d"
                        normalColor: root.activePlatform === 1 ? "#ea80fc" : "#e1bee7"
                        font.bold: true
                        font.pixelSize: 11
                        text: qsTr("🔥 AUDIUS (free)")
                        onClicked: root.activePlatform = 1
                    }

                    Skin.Button {
                        Layout.fillHeight: true
                        Layout.preferredWidth: 150
                        activeColor: "#ffb300"
                        backgroundColor: root.activePlatform === 2 ? "#4a3b00" : "#1a212d"
                        normalColor: root.activePlatform === 2 ? "#ffb300" : "#ffe082"
                        font.bold: true
                        font.pixelSize: 11
                        text: qsTr("🔗 DIRECT URL / RADIO")
                        onClicked: root.activePlatform = 2

                    Skin.Button {
                        Layout.fillHeight: true
                        Layout.preferredWidth: 130
                        activeColor: "#ff0000"
                        backgroundColor: root.activePlatform === 3 ? "#4a0000" : "#1a212d"
                        normalColor: root.activePlatform === 3 ? "#ff5252" : "#ff8a80"
                        font.bold: true
                        font.pixelSize: 11
                        text: qsTr("▶️ YOUTUBE")
                        onClicked: root.activePlatform = 3
                    }
                    }
                }

                Item { Layout.fillWidth: true }

                // Quick Deck Destination Indicator
                Rectangle {
                    Layout.preferredHeight: 32
                    Layout.preferredWidth: 120
                    radius: 4
                    color: root.targetDeck === "[Channel1]" ? "#0d47a1" : "#bf360c"
                    border.color: root.targetDeck === "[Channel1]" ? "#00b0ff" : "#ff6d00"
                    border.width: 1

                    RowLayout {
                        anchors.fill: parent
                        anchors.margins: 4
                        spacing: 4

                        Text {
                            text: qsTr("Target:")
                            color: "#b0bec5"
                            font.pixelSize: 10
                        }

                        Text {
                            text: root.targetDeck === "[Channel1]" ? "DECK 1" : "DECK 2"
                            color: "#ffffff"
                            font.bold: true
                            font.pixelSize: 11
                        }
                    }

                    MouseArea {
                        anchors.fill: parent
                        onClicked: {
                            root.targetDeck = (root.targetDeck === "[Channel1]") ? "[Channel2]" : "[Channel1]";
                        }
                    }
                }
            }
        }

        // ================= PLATFORM VIEWS CONTAINER =================
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            SubsonicView {
                anchors.fill: parent
                visible: root.activePlatform === 0
            }

            OnlineStreamView {
                anchors.fill: parent
                visible: root.activePlatform === 1
            }

            CustomUrlView {
                anchors.fill: parent
                targetDeck: root.targetDeck
                visible: root.activePlatform === 2
            }

            YouTubeView {
                anchors.fill: parent
                targetDeck: root.targetDeck
                visible: root.activePlatform === 3
            }
        }
    }
}

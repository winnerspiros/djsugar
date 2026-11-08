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
    property bool acceptsController: false
    readonly property var platformViews: [subsonicView, onlineStreamView, null, youTubeView]

    // BROWSE/LOAD on the controller act on the service that is shown
    StreamingControl {
        enabled: root.visible && root.acceptsController
        onMoveTab: (offset) => {
            const count = root.platformViews.length;
            root.activePlatform = (root.activePlatform + offset + count) % count;
        }
        onMoveResult: (offset) => {
            const view = root.platformViews[root.activePlatform];
            if (view && typeof view.controllerMove === "function")
                view.controllerMove(offset);
        }
        onLoadResult: (group, play) => {
            const view = root.platformViews[root.activePlatform];
            if (view && typeof view.controllerLoad === "function")
                view.controllerLoad(group || root.targetDeck, play);
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // ================= MULTI-PLATFORM SELECTOR BAR =================
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 26
            color: Theme.backgroundColor
            border.color: Theme.darkGray3
            border.width: 1

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 4
                anchors.rightMargin: 4
                spacing: 4

                // Platforms tabs
                RowLayout {
                    Layout.fillHeight: true
                    spacing: 4

                    Skin.Button {
                        Layout.fillHeight: true
                        Layout.preferredWidth: 132
                        activeColor: "#00e5ff"
                        normalColor: root.activePlatform === 0 ? "#00e5ff" : "#80d8ff"
                        font.bold: true
                        font.pixelSize: 9
                        text: "☁️ NAVIDROME / SUBSONIC"
                        onClicked: root.activePlatform = 0
                    }

                    Skin.Button {
                        Layout.fillHeight: true
                        Layout.preferredWidth: 100
                        activeColor: "#ea80fc"
                        normalColor: root.activePlatform === 1 ? "#ea80fc" : "#e1bee7"
                        font.bold: true
                        font.pixelSize: 9
                        text: qsTr("🔥 AUDIUS (free)")
                        onClicked: root.activePlatform = 1
                    }

                    Skin.Button {
                        Layout.fillHeight: true
                        Layout.preferredWidth: 122
                        activeColor: "#ffb300"
                        normalColor: root.activePlatform === 2 ? "#ffb300" : "#ffe082"
                        font.bold: true
                        font.pixelSize: 9
                        text: qsTr("🔗 DIRECT URL / RADIO")
                        onClicked: root.activePlatform = 2
                    }

                    Skin.Button {
                        Layout.fillHeight: true
                        Layout.preferredWidth: 96
                        activeColor: "#ff0000"
                        normalColor: root.activePlatform === 3 ? "#ff5252" : "#ff8a80"
                        font.bold: true
                        font.pixelSize: 9
                        text: qsTr("YOUTUBE")
                        onClicked: root.activePlatform = 3

                        // YouTube logo: red rounded plate with a white play
                        // triangle. Drawn in QML (Triangle, not a ▶ glyph that
                        // Android renders as emoji) so it needs no image asset.
                        Rectangle {
                            anchors.left: parent.left
                            anchors.leftMargin: 5
                            anchors.verticalCenter: parent.verticalCenter
                            width: 18
                            height: 12
                            radius: 3
                            color: "#ff0000"

                            Skin.Triangle {
                                anchors.centerIn: parent
                                anchors.horizontalCenterOffset: 1
                                color: "#ffffff"
                                height: 7
                                pointsRight: true
                                width: 6
                            }
                        }
                    }
                }

                Item { Layout.fillWidth: true }

                // Quick Deck Destination Indicator
                Rectangle {
                    Layout.preferredHeight: 20
                    Layout.preferredWidth: 90
                    radius: 4
                    color: root.targetDeck === "[Channel1]" ? "#0d47a1" : "#bf360c"
                    border.color: root.targetDeck === "[Channel1]" ? "#00b0ff" : "#ff6d00"
                    border.width: 1

                    RowLayout {
                        anchors.fill: parent
                        anchors.margins: 2
                        spacing: 3

                        Text {
                            text: qsTr("Target:")
                            color: "#b0bec5"
                            font.pixelSize: 9
                        }

                        Text {
                            text: root.targetDeck === "[Channel1]" ? "DECK 1" : "DECK 2"
                            color: "#ffffff"
                            font.bold: true
                            font.pixelSize: 9
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
                id: subsonicView

                anchors.fill: parent
                visible: root.activePlatform === 0
            }

            OnlineStreamView {
                id: onlineStreamView

                anchors.fill: parent
                visible: root.activePlatform === 1
            }

            CustomUrlView {
                anchors.fill: parent
                targetDeck: root.targetDeck
                visible: root.activePlatform === 2
            }

            YouTubeView {
                id: youTubeView

                anchors.fill: parent
                targetDeck: root.targetDeck
                visible: root.activePlatform === 3
            }
        }
    }
}

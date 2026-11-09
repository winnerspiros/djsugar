import QtQuick
import QtQuick.Controls 2.15
import QtQuick.Layouts
import Mixxx 1.0 as Mixxx
import "../../Theme"
import "../.." as Skin

Item {
    id: root

    property string targetDeck: "[Channel1]"
    property int activePlatform: 0 // 0: Navidrome / Audius / URL, 1: YouTube, 2: SoundCloud, 3: Spotify
    property bool acceptsController: false
    readonly property var platformViews: [webView, youTubeView, soundCloudView, spotifyView]

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

                    // Navidrome / Subsonic, Audius and direct URL / radio in
                    // one tab (WebView): it works out what the box is fed
                    Skin.Button {
                        Layout.fillHeight: true
                        Layout.preferredWidth: 212
                        activeColor: "#00e5ff"
                        normalColor: root.activePlatform === 0 ? "#00e5ff" : "#80d8ff"
                        font.bold: true
                        font.pixelSize: 9
                        text: qsTr("NAVIDROME · AUDIUS · URL")
                        onClicked: root.activePlatform = 0

                        Row {
                            anchors.left: parent.left
                            anchors.leftMargin: 4
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: -6

                            Repeater {
                                model: ["navidrome", "audius"]

                                delegate: Image {
                                    required property string modelData

                                    width: 18
                                    height: 18
                                    source: `qrc:/images/library/ic_library_${modelData}.svg`
                                    sourceSize.width: 40
                                    sourceSize.height: 40
                                }
                            }
                        }
                    }

                    Skin.Button {
                        Layout.fillHeight: true
                        Layout.preferredWidth: 96
                        activeColor: "#ff0000"
                        normalColor: root.activePlatform === 1 ? "#ff5252" : "#ff8a80"
                        font.bold: true
                        font.pixelSize: 9
                        text: qsTr("YOUTUBE")
                        onClicked: root.activePlatform = 1

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

                    // SoundCloud: orange cloud icon (ic_library_soundcloud.svg)
                    Skin.Button {
                        Layout.fillHeight: true
                        // wide enough that the centered text starts right
                        // of the 20 px cloud icon instead of under it
                        Layout.preferredWidth: 128
                        activeColor: "#ff5500"
                        normalColor: root.activePlatform === 2 ? "#ff7733" : "#ffab80"
                        font.bold: true
                        font.pixelSize: 9
                        text: qsTr("SOUNDCLOUD")
                        onClicked: root.activePlatform = 2

                        Image {
                            anchors.left: parent.left
                            anchors.leftMargin: 6
                            anchors.verticalCenter: parent.verticalCenter
                            width: 20
                            height: 20
                            source: "qrc:/images/library/ic_library_soundcloud.svg"
                            sourceSize.width: 40
                            sourceSize.height: 40
                        }
                    }

                    // Spotify: green disc with three arcs (ic_library_spotify.svg)
                    Skin.Button {
                        Layout.fillHeight: true
                        Layout.preferredWidth: 104
                        activeColor: "#1db954"
                        normalColor: root.activePlatform === 3 ? "#1ed760" : "#a7e8bd"
                        font.bold: true
                        font.pixelSize: 9
                        text: qsTr("SPOTIFY")
                        onClicked: root.activePlatform = 3

                        Image {
                            anchors.left: parent.left
                            anchors.leftMargin: 6
                            anchors.verticalCenter: parent.verticalCenter
                            width: 18
                            height: 18
                            source: "qrc:/images/library/ic_library_spotify.svg"
                            sourceSize.width: 40
                            sourceSize.height: 40
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

            WebView {
                id: webView

                anchors.fill: parent
                targetDeck: root.targetDeck
                visible: root.activePlatform === 0
            }

            YouTubeView {
                id: youTubeView

                anchors.fill: parent
                targetDeck: root.targetDeck
                visible: root.activePlatform === 1
            }

            SoundCloudView {
                id: soundCloudView

                anchors.fill: parent
                targetDeck: root.targetDeck
                visible: root.activePlatform === 2
            }

            SpotifyView {
                id: spotifyView

                anchors.fill: parent
                targetDeck: root.targetDeck
                visible: root.activePlatform === 3
            }
        }
    }
}

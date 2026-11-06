import Mixxx 1.0 as Mixxx
import QtQuick
import QtQuick.Layouts
import "../.." as Skin

// YouTube search tab of the Booth streaming hub: searches the built-in
// YouTube service and loads results straight into a deck. Downloads are
// cached by the YouTube feature; loading via URL lets it download/stream.
Item {
    id: root

    property string targetDeck: "[Channel1]"
    property var results: []
    property string statusMessage: qsTr("Search YouTube and load straight into a deck")
    property bool isBusy: false

    function doSearch() {
        if (searchField.text.length === 0) {
            return;
        }
        root.isBusy = true;
        root.statusMessage = qsTr("Searching YouTube...");
        Mixxx.YouTube.search(searchField.text);
    }

    Connections {
        target: Mixxx.YouTube

        function onSearchFinished(results) {
            root.isBusy = false;
            root.results = results;
            root.statusMessage = results.length > 0
                ? qsTr("%1 results").arg(results.length)
                : qsTr("No results");
        }
        function onSearchFailed(error) {
            root.isBusy = false;
            root.statusMessage = qsTr("Search error: %1").arg(error);
        }
        function onDownloadFinished(videoId, path) {
            root.statusMessage = qsTr("Downloaded, loading...");
            const player = Mixxx.PlayerManager.getPlayer(root.targetDeck);
            if (player) {
                player.loadTrackFromLocation(path, false);
            }
        }
        function onDownloadFailed(videoId, error) {
            root.statusMessage = qsTr("Download error: %1").arg(error);
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 14
        spacing: 12

        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 44
                color: "#000000"
                border.color: searchField.activeFocus ? "#ff5252" : "#5c6068"
                border.width: 1
                radius: 4

                TextInput {
                    id: searchField

                    anchors.fill: parent
                    anchors.margins: 10
                    color: "#ffffff"
                    font.pixelSize: 14
                    verticalAlignment: TextInput.AlignVCenter
                    onAccepted: root.doSearch()
                }
                Text {
                    anchors.fill: parent
                    anchors.margins: 10
                    color: "#6c7078"
                    font.pixelSize: 14
                    text: qsTr("SEARCH YOUTUBE")
                    verticalAlignment: Text.AlignVCenter
                    visible: searchField.text.length === 0
                }
            }
            Skin.Button {
                Layout.preferredHeight: 44
                Layout.preferredWidth: 120
                activeColor: "#ff5252"
                normalColor: "#ffffff"
                font.bold: true
                font.pixelSize: 12
                text: root.isBusy ? "..." : qsTr("SEARCH")
                enabled: !root.isBusy
                onClicked: root.doSearch()
            }
        }

        Text {
            Layout.fillWidth: true
            color: "#9a9ea6"
            font.pixelSize: 12
            text: root.statusMessage
            elide: Text.ElideRight
        }

        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: root.results
            spacing: 4
            delegate: Rectangle {
                id: videoCard
                required property var modelData
                required property int index

                width: ListView.view.width
                height: 68
                color: index % 2 === 0 ? "#0c0d0f" : "#16181b"
                border.color: isDownloading ? "#e63380" : "#2a2c30"
                border.width: 1
                radius: 6
                clip: true

                property var video: modelData
                readonly property real downloadProg: {
                    const map = Mixxx.YouTube.downloadProgressByVideo;
                    return (video && video.id && map && map[video.id] !== undefined)
                        ? Number(map[video.id])
                        : -1;
                }
                readonly property bool isDownloading: downloadProg >= 0 && downloadProg < 1.0

                // Subtle translucent progress fill across the song box while downloading
                Rectangle {
                    anchors.left: parent.left
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    visible: videoCard.downloadProg >= 0
                    width: parent.width * Math.max(0.04, Math.min(1.0, videoCard.downloadProg))
                    color: "#22e63380"
                    Behavior on width {
                        NumberAnimation { duration: 140; easing.type: Easing.OutCubic }
                    }
                }

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 8
                    anchors.bottomMargin: 11
                    spacing: 2

                    Text {
                        Layout.fillWidth: true
                        color: "#ffffff"
                        font.pixelSize: 13
                        font.bold: true
                        text: video.title ?? ""
                        elide: Text.ElideRight
                    }
                    Text {
                        Layout.fillWidth: true
                        color: videoCard.downloadProg >= 0 ? "#ff99ca" : "#9a9ea6"
                        font.pixelSize: 11
                        font.bold: videoCard.downloadProg >= 0
                        text: {
                            const base = (video.uploader ?? "") + (video.durationSec > 0 ? " - " + Math.floor(video.durationSec / 60) + ":" + String(video.durationSec % 60).padStart(2, "0") : "");
                            if (videoCard.downloadProg >= 0) {
                                return qsTr("⬇ Downloading %1% · ").arg(Math.round(videoCard.downloadProg * 100)) + base;
                            }
                            return base;
                        }
                        elide: Text.ElideRight
                    }
                }
                RowLayout {
                    anchors.right: parent.right
                    anchors.rightMargin: 8
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 6

                    Skin.Button {
                        Layout.preferredHeight: 36
                        Layout.preferredWidth: 64
                        activeColor: "#64b5f6"
                        normalColor: "#ffffff"
                        font.bold: true
                        font.pixelSize: 12
                        text: videoCard.isDownloading ? `${Math.round(videoCard.downloadProg * 100)}%` : "D1"
                        onClicked: {
                            const player = Mixxx.PlayerManager.getPlayer("[Channel1]");
                            if (player && video.file_url) {
                                player.loadTrackFromLocationUrl(video.file_url);
                            } else if (player) {
                                Mixxx.YouTube.downloadToDeck(video.id, "[Channel1]");
                            }
                        }
                    }
                    Skin.Button {
                        Layout.preferredHeight: 36
                        Layout.preferredWidth: 64
                        activeColor: "#ffb74d"
                        normalColor: "#ffffff"
                        font.bold: true
                        font.pixelSize: 12
                        text: videoCard.isDownloading ? `${Math.round(videoCard.downloadProg * 100)}%` : "D2"
                        onClicked: {
                            const player = Mixxx.PlayerManager.getPlayer("[Channel2]");
                            if (player && video.file_url) {
                                player.loadTrackFromLocationUrl(video.file_url);
                            } else if (player) {
                                Mixxx.YouTube.downloadToDeck(video.id, "[Channel2]");
                            }
                        }
                    }
                }

                // Linear progress bar across bottom of song box when downloading
                Rectangle {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    height: 4
                    color: "#1e222a"
                    visible: videoCard.downloadProg >= 0

                    Rectangle {
                        anchors.left: parent.left
                        anchors.top: parent.top
                        anchors.bottom: parent.bottom
                        width: parent.width * Math.max(0.04, Math.min(1.0, videoCard.downloadProg))
                        radius: 2
                        gradient: Gradient {
                            orientation: Gradient.Horizontal
                            GradientStop { position: 0.0; color: "#e63380" }
                            GradientStop { position: 0.55; color: "#ff99ca" }
                            GradientStop { position: 1.0; color: "#00e5ff" }
                        }
                        Behavior on width {
                            NumberAnimation { duration: 140; easing.type: Easing.OutCubic }
                        }
                    }
                }
            }
        }
    }
}

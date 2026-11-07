import Mixxx 1.0 as Mixxx
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../.." as Skin
import "../../Theme"

// YouTube search tab of the Booth streaming hub: searches the built-in
// YouTube service and loads results straight into a deck. Downloads are
// cached by the YouTube feature; loading via URL lets it download/stream.
Item {
    id: root

    property string targetDeck: "[Channel1]"
    property var results: []
    property string statusMessage: qsTr("Search YouTube and load straight into a deck")
    property bool isBusy: false
    property string selectedGenre: ""
    readonly property var genreOptions: ["", "EDM", "House", "Techno", "Trance", "DnB", "Dubstep", "Hip Hop", "Rap", "R&B", "Pop", "Rock", "Lo-fi", "Chillout", "Ambient", "Synthwave", "Classical", "Jazz", "Blues", "Country", "Reggae", "K-Pop", "J-Pop", "Latin", "Afrobeat"]

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
        // The proxy loads each finished download into the deck its button
        // was pressed for; the view only reports status.
        function onDownloadFinished(videoId, path) {
            root.statusMessage = qsTr("Downloaded and loaded");
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
                border.color: searchField.activeFocus ? Theme.accentColor : Theme.midGray3
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
                    color: Theme.lightGray3
                    font.pixelSize: 14
                    text: qsTr("SEARCH YOUTUBE")
                    verticalAlignment: Text.AlignVCenter
                    visible: searchField.text.length === 0
                }
            }
            Skin.Button {
                Layout.preferredHeight: 44
                Layout.preferredWidth: 120
                activeColor: Theme.accentColor
                normalColor: "#ffffff"
                font.bold: true
                font.pixelSize: 12
                text: root.isBusy ? "..." : qsTr("SEARCH")
                enabled: !root.isBusy
                onClicked: root.doSearch()
            }
        }
        ComboBox {
                id: genreFilter
                Layout.preferredHeight: 44
                Layout.preferredWidth: 140
                model: root.genreOptions
                currentText: root.selectedGenre
                onCurrentTextChanged: root.selectedGenre = genreFilter.currentText
                background: Rectangle {
                    color: "#0c0d0f"
                    border.color: activeFocus ? "Theme.accentColor" : "Theme.midGray3"
                    border.width: 1
                    radius: 4
                }
                contentItem: Text {
                    leftPadding: 10
                    rightPadding: 10
                    verticalAlignment: Text.AlignVCenter
                    color: "#ffffff"
                    font.pixelSize: 12
                    text: control.currentText === "" ? qsTr("ALL GENRES") : control.currentText
                }
            }
            Switch {
                id: sponsorBlockSwitch
                Layout.preferredHeight: 44
                Layout.preferredWidth: 160
                checked: Mixxx.YouTube.sponsorBlockAutoSkip
                onToggled: Mixxx.YouTube.sponsorBlockAutoSkip = sponsorBlockSwitch.checked
                background: Rectangle {
                    implicitWidth: 60
                    implicitHeight: 28
                    radius: 14
                    color: sponsorBlockSwitch.checked ? "Theme.accentColor" : "Theme.midGray3"
                    border.color: "Theme.midGray2"
                    border.width: 1
                }
                indicator: Rectangle {
                    x: sponsorBlockSwitch.checked ? width - height : 0
                    y: 0
                    width: height
                    height: height
                    radius: 14
                    color: "#ffffff"
                    border.color: "Theme.midGray2"
                    border.width: 1
                    Behavior on x { NumberAnimation { duration: 150 } }
                }
                contentItem: Text {
                    leftPadding: 8
                    rightPadding: 8
                    verticalAlignment: Text.AlignVCenter
                    color: "#ffffff"
                    font.pixelSize: 11
                    font.bold: true
                    text: qsTr("AUTO SKIP")
                    elide: Text.ElideRight
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
            model: root.selectedGenre === "" ? root.results : root.results.filter(r => r.genre && r.genre.toLowerCase().includes(root.selectedGenre.toLowerCase()))
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

                // Thumbnail straight from YouTube: Qt caches it, so scrolling
                // back is instant and no download is needed for browsing.
                Rectangle {
                    id: thumbFrame

                    anchors.left: parent.left
                    anchors.leftMargin: 6
                    anchors.verticalCenter: parent.verticalCenter
                    width: 88
                    height: 50
                    radius: 4
                    color: "#1e222a"
                    clip: true

                    Image {
                        anchors.fill: parent
                        source: video.id ? "https://i.ytimg.com/vi/" + video.id + "/mqdefault.jpg" : ""
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                        cache: true
                        sourceSize.width: 176
                        sourceSize.height: 100
                    }
                }

                ColumnLayout {
                    anchors.left: thumbFrame.right
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    anchors.leftMargin: 10
                    anchors.rightMargin: 8
                    anchors.topMargin: 8
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
                            const genrePart = video.genre ? (video.genre + " · ") : "";
                            const base = genrePart + (video.uploader ?? "") + (video.durationSec > 0 ? " - " + Math.floor(video.durationSec / 60) + ":" + String(video.durationSec % 60).padStart(2, "0") : "");
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

                    Repeater {
                        model: [
                            { deck: "[Channel1]", label: "D1", color: Qt.lighter(Theme.accentColor, 1.2) },
                            { deck: "[Channel2]", label: "D2", color: "#e8650f" }
                        ]
                        delegate: Skin.Button {
                            required property var modelData
                            Layout.preferredHeight: 36
                            Layout.preferredWidth: 64
                            activeColor: modelData.color
                            normalColor: "#ffffff"
                            font.bold: true
                            font.pixelSize: 12
                            text: videoCard.isDownloading
                                ? `${Math.round(videoCard.downloadProg * 100)}%`
                                : modelData.label
                            enabled: !videoCard.isDownloading
                            onClicked: Mixxx.YouTube.downloadToDeck(videoCard.video.id, modelData.deck)
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

import Mixxx 1.0 as Mixxx
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../.." as Skin
import "../../Theme"

// SoundCloud search. Results download through the same yt-dlp path as
// YouTube (ids "sc-<track id>"), so deck loads, the download cache, the
// library rows and the played/BPM/key chips all work the same way.
// Go+ tracks are marked and can't be loaded: without the subscription
// SoundCloud only serves a 30 s preview of them.
Item {
    id: root

    property string targetDeck: "[Channel1]"
    property string statusMessage: qsTr("Search SoundCloud: remixes, edits, DJ mixes")
    property bool isBusy: false
    property bool isLoadingMore: false

    function doSearch() {
        const query = searchField.text.trim();
        if (query.length === 0)
            return;
        root.isBusy = true;
        root.statusMessage = qsTr("Searching...");
        Mixxx.YouTube.soundCloudSearch(query);
    }
    function maybeLoadMore() {
        if (root.isBusy || root.isLoadingMore || !Mixxx.YouTube.soundCloudHasMore())
            return;
        if (resultList.contentHeight - resultList.contentY - resultList.height > 900)
            return;
        root.isLoadingMore = true;
        Mixxx.YouTube.soundCloudMore();
    }
    function formatDuration(sec) {
        const m = Math.floor(sec / 60);
        const s = sec % 60;
        return m + ":" + (s < 10 ? "0" : "") + s;
    }
    function formatPlays(plays) {
        if (plays >= 1000000)
            return (plays / 1000000).toFixed(1) + "M";
        if (plays >= 1000)
            return Math.round(plays / 1000) + "K";
        return Math.round(plays).toString();
    }
    function fitsDeck(bpm) {
        return bpm > 0 && deckBpm.value > 0 && Math.abs(bpm - deckBpm.value) / deckBpm.value <= 0.06;
    }

    // Controller: BROWSE moves the highlighted result, LOAD downloads it to
    // the deck the button belongs to
    function controllerMove(offset) {
        if (resultsModel.count === 0)
            return;
        const next = resultList.currentIndex < 0 ? 0 : resultList.currentIndex + offset;
        resultList.currentIndex = Math.max(0, Math.min(resultsModel.count - 1, next));
        resultList.positionViewAtIndex(resultList.currentIndex, ListView.Contain);
        root.maybeLoadMore();
    }
    function controllerLoad(group, _play) {
        const row = resultList.currentIndex;
        if (row < 0 || row >= resultsModel.count)
            return;
        const track = resultsModel.get(row);
        if (track.preview) {
            root.statusMessage = qsTr("Go+ track: SoundCloud only gives a 30 s preview");
            return;
        }
        Mixxx.YouTube.downloadToDeck(track.vid, group);
    }

    ListModel {
        id: resultsModel
    }

    Mixxx.ControlProxy {
        id: deckBpm

        group: root.targetDeck
        key: "bpm"
    }

    Mixxx.ControlProxy {
        id: numDecks

        group: "[App]"
        key: "num_decks"
    }

    Connections {
        function onSoundCloudResults(results, append) {
            root.isBusy = false;
            root.isLoadingMore = false;
            if (!append) {
                resultsModel.clear();
                resultList.currentIndex = -1;
            }
            for (const track of results) {
                resultsModel.append({
                    "vid": track.id ?? "",
                    "title": track.title ?? "",
                    "uploader": track.uploader ?? "",
                    "durationSec": track.durationSec ?? 0,
                    "genre": track.genre ?? "",
                    "artwork": track.artwork ?? "",
                    "plays": track.plays ?? 0,
                    "preview": track.preview ?? false,
                    "timesPlayed": track.timesPlayed ?? 0,
                    "bpm": track.bpm ?? 0,
                    "musicKey": track.key ?? ""
                });
            }
            if (!append)
                resultList.positionViewAtBeginning();
            root.statusMessage = resultsModel.count > 0 ? qsTr("%1 results").arg(resultsModel.count) : qsTr("No results");
        }
        function onSoundCloudFailed(error) {
            root.isBusy = false;
            root.isLoadingMore = false;
            root.statusMessage = qsTr("SoundCloud error: %1").arg(error);
        }
        function onDownloadFailed(videoId, error) {
            if (videoId.startsWith("sc-"))
                root.statusMessage = qsTr("Download error: %1").arg(error);
        }

        target: Mixxx.YouTube
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 6
        spacing: 4

        RowLayout {
            Layout.fillWidth: true
            spacing: 6

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 30
                color: "#000000"
                border.color: searchField.activeFocus ? "#ff5500" : Theme.midGray3
                border.width: 1
                radius: 4

                TextInput {
                    id: searchField

                    anchors.fill: parent
                    anchors.margins: 6
                    color: "#ffffff"
                    font.pixelSize: 12
                    verticalAlignment: TextInput.AlignVCenter
                    onAccepted: root.doSearch()
                }
                Text {
                    anchors.fill: parent
                    anchors.margins: 6
                    color: Theme.lightGray3
                    font.pixelSize: 12
                    text: qsTr("SEARCH SOUNDCLOUD")
                    verticalAlignment: Text.AlignVCenter
                    visible: searchField.text.length === 0
                }
            }
            Skin.Button {
                Layout.preferredHeight: 30
                Layout.preferredWidth: 80
                activeColor: "#ff5500"
                normalColor: "#ffffff"
                font.bold: true
                font.pixelSize: 10
                text: root.isBusy ? "..." : qsTr("SEARCH")
                enabled: !root.isBusy
                onClicked: root.doSearch()
            }
        }

        Text {
            Layout.fillWidth: true
            color: "#9a9ea6"
            elide: Text.ElideRight
            font.pixelSize: 11
            text: root.statusMessage
        }

        ListView {
            id: resultList

            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: resultsModel
            spacing: 2
            currentIndex: -1
            highlightFollowsCurrentItem: true
            highlightMoveDuration: 0
            onContentYChanged: root.maybeLoadMore()

            highlight: Rectangle {
                z: 2
                color: "transparent"
                border.color: "#ff5500"
                border.width: 2
                radius: 4
            }
            footer: Item {
                width: ListView.view.width
                height: root.isLoadingMore ? 40 : 0
                visible: root.isLoadingMore

                Text {
                    anchors.centerIn: parent
                    color: "#9a9ea6"
                    font.pixelSize: 12
                    text: qsTr("Loading more results...")
                }
            }
            delegate: Rectangle {
                id: trackRow

                required property int index
                required property string vid
                required property string title
                required property string uploader
                required property int durationSec
                required property string genre
                required property string artwork
                required property double plays
                required property bool preview
                required property int timesPlayed
                required property double bpm
                required property string musicKey
                readonly property double downloadProg: Mixxx.YouTube.downloadProgressByVideo[vid] ?? -1
                readonly property bool isDownloading: downloadProg >= 0

                width: ListView.view.width
                height: 48
                radius: 4
                color: index % 2 === 0 ? "#0c0d0f" : "#16181b"
                border.color: isDownloading ? "#ff5500" : "#2a2c30"
                border.width: 1
                opacity: preview ? 0.55 : 1.0

                MouseArea {
                    anchors.fill: parent
                    onClicked: resultList.currentIndex = trackRow.index
                }

                RowLayout {
                    anchors.fill: parent
                    anchors.margins: 4
                    spacing: 6

                    Rectangle {
                        Layout.preferredWidth: 40
                        Layout.preferredHeight: 40
                        color: "#1e222a"
                        radius: 3

                        Image {
                            anchors.fill: parent
                            asynchronous: true
                            cache: true
                            fillMode: Image.PreserveAspectCrop
                            source: trackRow.artwork
                            sourceSize.width: 80
                            sourceSize.height: 80
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 1

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 4

                            // Genre bubble, like the YouTube and Audius lists
                            Rectangle {
                                visible: trackRow.genre !== ""
                                Layout.preferredHeight: 15
                                Layout.preferredWidth: Math.min(110, genreText.implicitWidth + 10)
                                radius: 7
                                color: "#2a1640"
                                border.color: "#ea80fc"
                                border.width: 1

                                Text {
                                    id: genreText

                                    anchors.centerIn: parent
                                    width: parent.width - 8
                                    color: "#f3c4fb"
                                    elide: Text.ElideRight
                                    font.pixelSize: 9
                                    horizontalAlignment: Text.AlignHCenter
                                    text: trackRow.genre
                                }
                            }
                            Text {
                                Layout.fillWidth: true
                                color: "#ffffff"
                                elide: Text.ElideRight
                                font.pixelSize: 12
                                font.bold: true
                                text: trackRow.title
                            }
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 4

                            Text {
                                Layout.fillWidth: true
                                color: "#9a9ea6"
                                elide: Text.ElideRight
                                font.pixelSize: 10
                                text: trackRow.isDownloading ? qsTr("Downloading %1%").arg(Math.round(trackRow.downloadProg * 100)) : trackRow.uploader + "  ·  " + root.formatDuration(trackRow.durationSec) + (trackRow.plays > 0 ? "  ·  ▶ " + root.formatPlays(trackRow.plays) : "")
                            }
                            Repeater {
                                model: [
                                    {
                                        "show": trackRow.preview,
                                        "text": qsTr("GO+ preview"),
                                        "fg": "#ffcc80",
                                        "bg": "#3a2410"
                                    },
                                    {
                                        "show": trackRow.timesPlayed > 0,
                                        "text": qsTr("▶ played"),
                                        "fg": "#a5d6a7",
                                        "bg": "#14301a"
                                    },
                                    {
                                        "show": trackRow.bpm > 0,
                                        "text": trackRow.bpm.toFixed(0) + " BPM",
                                        "fg": root.fitsDeck(trackRow.bpm) ? "#ffe082" : "#b0bec5",
                                        "bg": root.fitsDeck(trackRow.bpm) ? "#3a2f0a" : "#1e252b"
                                    },
                                    {
                                        "show": trackRow.musicKey !== "",
                                        "text": trackRow.musicKey,
                                        "fg": "#90caf9",
                                        "bg": "#0f2233"
                                    }
                                ]

                                delegate: Rectangle {
                                    required property var modelData

                                    visible: modelData.show
                                    Layout.preferredHeight: 14
                                    Layout.preferredWidth: chipText.implicitWidth + 8
                                    radius: 3
                                    color: modelData.bg

                                    Text {
                                        id: chipText

                                        anchors.centerIn: parent
                                        color: modelData.fg
                                        font.pixelSize: 9
                                        text: modelData.text
                                    }
                                }
                            }
                        }
                    }

                    // One button per deck; the download lands on that deck
                    Repeater {
                        model: Math.max(2, numDecks.value)

                        delegate: Skin.Button {
                            required property int index

                            Layout.preferredHeight: 30
                            Layout.preferredWidth: 34
                            activeColor: "#ff5500"
                            normalColor: "#ffffff"
                            font.bold: true
                            font.pixelSize: 11
                            text: (index + 1).toString()
                            enabled: !trackRow.isDownloading && !trackRow.preview
                            onClicked: {
                                resultList.currentIndex = trackRow.index;
                                Mixxx.YouTube.downloadToDeck(trackRow.vid, "[Channel" + (index + 1) + "]");
                            }
                        }
                    }
                }
            }
        }
    }
}

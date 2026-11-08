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
    // Every video the search has delivered so far, in order.
    property var allResults: []
    property string statusMessage: qsTr("Search YouTube and load straight into a deck")
    property bool isBusy: false
    property bool isLoadingMore: false
    property bool hasMore: false
    // A fetch that ended empty is not retried until this time, so a flagged or
    // offline connection is not hammered while the user keeps scrolling.
    property double retryMoreAfter: 0

    function toRow(video) {
        return {
            "vid": video.id ?? "",
            "title": video.title ?? "",
            "uploader": video.uploader ?? "",
            "durationSec": video.durationSec ?? 0,
            "genre": video.genre ?? "",
            "timesPlayed": video.timesPlayed ?? 0,
            "bpm": video.bpm ?? 0,
            "musicKey": video.key ?? "",
            "versions": 1
        };
    }

    // Same song, another upload: artist and title without the "(Official
    // Video)" / "[Lyrics]" decorations, and a length within a few seconds.
    function songKey(video) {
        return (video.title ?? "").toLowerCase()
            .replace(/[\(\[][^\)\]]*[\)\]]/g, " ")
            .replace(/\b(official|music|video|lyrics?|audio|hd|4k|visuali[sz]er|clip)\b/g, " ")
            .replace(/[^\p{L}\p{N}]+/gu, " ")
            .trim();
    }
    function findSameSong(video) {
        const key = root.songKey(video);
        for (let i = 0; i < resultsModel.count; ++i) {
            const row = resultsModel.get(i);
            if (row.vid === video.sameAs) {
                return i;
            }
            if (key.length > 0 && root.songKey(row) === key
                    && (video.durationSec <= 0 || row.durationSec <= 0 || Math.abs(video.durationSec - row.durationSec) <= 8)) {
                return i;
            }
        }
        return -1;
    }

    // Adds a batch below what is already listed, skipping videos the search
    // has already delivered. Returns how many were new.
    function appendResults(batch) {
        const seen = new Set(root.allResults.map(v => v.id));
        const fresh = batch.filter(v => v.id && !seen.has(v.id));
        root.allResults = root.allResults.concat(fresh);
        let shown = 0;
        for (const video of fresh) {
            // Other uploads of a listed song are folded into its row.
            const same = root.findSameSong(video);
            if (same >= 0) {
                const row = resultsModel.get(same);
                resultsModel.setProperty(same, "versions", row.versions + 1);
                if ((video.timesPlayed ?? 0) > row.timesPlayed) {
                    resultsModel.setProperty(same, "timesPlayed", video.timesPlayed);
                }
                continue;
            }
            resultsModel.append(root.toRow(video));
            ++shown;
        }
        return shown;
    }

    // After a batch lands, keep fetching while the list is still too short to
    // scroll, but only if the batch put something new on screen.
    function continueIfShort(shownBefore) {
        if (resultsModel.count > shownBefore) {
            root.maybeLoadMore();
        }
    }

    function doSearch() {
        if (searchField.text.length === 0) {
            return;
        }
        root.isBusy = true;
        root.isLoadingMore = false;
        root.hasMore = false;
        root.statusMessage = qsTr("Searching YouTube...");
        Mixxx.YouTube.search(searchField.text);
    }

    // Infinite scroll: the next batch is fetched when the list nears its end.
    function maybeLoadMore() {
        if (root.isBusy || root.isLoadingMore || !root.hasMore || !root.visible || root.allResults.length === 0 || Date.now() < root.retryMoreAfter) {
            return;
        }
        if (resultList.contentHeight - resultList.contentY - resultList.height > 900) {
            return;
        }
        root.isLoadingMore = true;
        Mixxx.YouTube.loadMore();
    }

    // Controller: BROWSE moves the highlighted result, LOAD downloads it
    // to the deck (play = start it once loaded is not supported by the
    // download path, it loads paused)
    function controllerMove(offset) {
        if (resultsModel.count === 0)
            return;
        const next = resultList.currentIndex < 0 ? 0 : resultList.currentIndex + offset;
        resultList.currentIndex = Math.max(0, Math.min(resultsModel.count - 1, next));
        resultList.positionViewAtIndex(resultList.currentIndex, ListView.Contain);
        Mixxx.YouTube.prefetchLink(resultsModel.get(resultList.currentIndex).vid);
    }
    function controllerLoad(group, _play) {
        const row = resultList.currentIndex;
        if (row < 0 || row >= resultsModel.count)
            return;
        Mixxx.YouTube.downloadToDeck(resultsModel.get(row).vid, group);
    }

    ListModel {
        id: resultsModel
    }

    // Tempo of the deck results load into, to flag tracks that mix with it.
    Mixxx.ControlProxy {
        id: deckBpm
        group: root.targetDeck
        key: "bpm"
    }
    function fitsDeck(bpm) {
        if (bpm <= 0 || deckBpm.value <= 0) {
            return false;
        }
        for (const factor of [1, 2, 0.5]) {
            if (Math.abs(bpm * factor - deckBpm.value) / deckBpm.value <= 0.06) {
                return true;
            }
        }
        return false;
    }

    Connections {
        target: Mixxx.YouTube

        function onSearchFinished(results) {
            root.isBusy = false;
            root.isLoadingMore = false;
            root.allResults = [];
            resultsModel.clear();
            root.appendResults(results);
            root.hasMore = Mixxx.YouTube.hasMore();
            root.retryMoreAfter = 0;
            root.statusMessage = root.allResults.length > 0
                ? qsTr("%1 results").arg(root.allResults.length)
                : qsTr("No results");
            resultList.positionViewAtBeginning();
            root.continueIfShort(0);
        }
        function onSearchMoreFinished(results, hasMore) {
            root.isLoadingMore = false;
            const shownBefore = resultsModel.count;
            const added = root.appendResults(results);
            root.hasMore = hasMore;
            if (added === 0) {
                root.retryMoreAfter = Date.now() + 4000;
            }
            root.statusMessage = qsTr("%1 results").arg(root.allResults.length);
            root.continueIfShort(shownBefore);
        }
        function onGenreResolved(videoId, genre) {
            for (const video of root.allResults) {
                if (video.id === videoId) {
                    video.genre = genre;
                }
            }
            for (let i = 0; i < resultsModel.count; ++i) {
                if (resultsModel.get(i).vid === videoId) {
                    resultsModel.setProperty(i, "genre", genre);
                }
            }
        }
        function onSearchFailed(error) {
            root.isBusy = false;
            root.isLoadingMore = false;
            root.statusMessage = qsTr("Search error: %1").arg(error);
        }
        // The feature loads each finished download into the deck its button
        // was pressed for; the view only reports status.
        function onDownloadFinished(videoId, path) {
            root.statusMessage = qsTr("Download finished");
        }
        function onDownloadFailed(videoId, error) {
            root.statusMessage = qsTr("Download error: %1").arg(error);
        }
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
                border.color: searchField.activeFocus ? Theme.accentColor : Theme.midGray3
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
                    text: qsTr("SEARCH YOUTUBE")
                    verticalAlignment: Text.AlignVCenter
                    visible: searchField.text.length === 0
                }
            }
            Skin.Button {
                Layout.preferredHeight: 30
                Layout.preferredWidth: 80
                activeColor: Theme.accentColor
                normalColor: "#ffffff"
                font.bold: true
                font.pixelSize: 10
                text: root.isBusy ? "..." : qsTr("SEARCH")
                enabled: !root.isBusy
                onClicked: root.doSearch()
            }
            Switch {
                id: sponsorBlockSwitch

                Layout.preferredHeight: 30
                Layout.preferredWidth: 150
                checked: Mixxx.YouTube.sponsorBlockAutoSkip
                onToggled: Mixxx.YouTube.sponsorBlockAutoSkip = sponsorBlockSwitch.checked

                indicator: Rectangle {
                    x: sponsorBlockSwitch.leftPadding
                    y: (sponsorBlockSwitch.height - height) / 2
                    implicitWidth: 38
                    implicitHeight: 20
                    radius: 10
                    color: sponsorBlockSwitch.checked ? Theme.accentColor : Theme.midGray3
                    border.color: Theme.midGray2
                    border.width: 1

                    Rectangle {
                        x: sponsorBlockSwitch.checked ? parent.width - width - 2 : 2
                        y: 2
                        width: parent.height - 4
                        height: width
                        radius: width / 2
                        color: "#ffffff"

                        Behavior on x {
                            NumberAnimation {
                                duration: 150
                            }
                        }
                    }
                }
                contentItem: Text {
                    leftPadding: sponsorBlockSwitch.indicator.width + 6
                    verticalAlignment: Text.AlignVCenter
                    color: "#ffffff"
                    font.bold: true
                    font.pixelSize: 9
                    text: qsTr("AUTO SKIP SPONSORS")
                    elide: Text.ElideRight
                }
            }
        }
        Text {
            Layout.fillWidth: true
            color: "#9a9ea6"
            font.pixelSize: 10
            text: root.statusMessage
            elide: Text.ElideRight
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
            highlight: Rectangle {
                z: 2
                color: "transparent"
                border.color: "#00e5ff"
                border.width: 2
                radius: 4
            }
            onContentYChanged: root.maybeLoadMore()
            footer: Item {
                width: ListView.view.width
                height: root.isLoadingMore ? 48 : 0
                visible: root.isLoadingMore

                Text {
                    anchors.centerIn: parent
                    color: "#9a9ea6"
                    font.pixelSize: 12
                    text: qsTr("Loading more results...")
                }
            }
            delegate: Rectangle {
                id: videoCard
                required property string vid
                required property string title
                required property string uploader
                required property int durationSec
                required property string genre
                required property int timesPlayed
                required property double bpm
                required property string musicKey
                required property int versions
                required property int index

                width: ListView.view.width
                height: 48
                color: index % 2 === 0 ? "#0c0d0f" : "#16181b"
                border.color: isDownloading ? "#e63380" : "#2a2c30"
                border.width: 1
                radius: 6
                clip: true

                readonly property var video: ({
                    "id": vid,
                    "title": title,
                    "uploader": uploader,
                    "durationSec": durationSec,
                    "genre": genre
                })
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
                    width: 64
                    height: 36
                    radius: 4
                    color: "#1e222a"
                    clip: true

                    Image {
                        anchors.fill: parent
                        source: video.id ? "https://i.ytimg.com/vi/" + video.id + "/mqdefault.jpg" : ""
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true
                        cache: true
                        sourceSize.width: 128
                        sourceSize.height: 72
                    }
                }

                ColumnLayout {
                    anchors.left: thumbFrame.right
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    anchors.leftMargin: 8
                    // Clear of the D1, D2 and download buttons
                    anchors.rightMargin: 162
                    anchors.topMargin: 5
                    anchors.bottomMargin: 6
                    spacing: 1

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 5

                        // Genre bubble, like the Audius list
                        Rectangle {
                            visible: videoCard.genre !== ""
                            Layout.preferredHeight: 14
                            Layout.preferredWidth: genreLabel.implicitWidth + 10
                            Layout.maximumWidth: 110
                            radius: 7
                            color: "#2a1640"
                            border.color: "#ea80fc"
                            border.width: 1

                            Text {
                                id: genreLabel
                                anchors.centerIn: parent
                                width: Math.min(implicitWidth, 100)
                                color: "#f3c4fb"
                                font.pixelSize: 8
                                font.bold: true
                                text: videoCard.genre
                                elide: Text.ElideRight
                            }
                        }
                        Text {
                            Layout.fillWidth: true
                            color: "#ffffff"
                            font.pixelSize: 11
                            font.bold: true
                            text: video.title ?? ""
                            elide: Text.ElideRight
                        }
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 4

                        // Played this session (this upload or another of the song)
                        Repeater {
                            model: [
                                { show: videoCard.timesPlayed > 0, text: qsTr("▶ played"), fg: "#a5d6a7", bg: "#14301a" },
                                { show: videoCard.bpm > 0, text: videoCard.bpm.toFixed(0) + " BPM", fg: root.fitsDeck(videoCard.bpm) ? "#ffe082" : "#b0bec5", bg: root.fitsDeck(videoCard.bpm) ? "#3a2f0a" : "#1e252b" },
                                { show: videoCard.musicKey !== "", text: videoCard.musicKey, fg: "#90caf9", bg: "#0f2233" },
                                { show: videoCard.versions > 1, text: qsTr("%1 versions").arg(videoCard.versions), fg: "#cfd8dc", bg: "#262b31" }
                            ]
                            delegate: Rectangle {
                                required property var modelData
                                visible: modelData.show
                                Layout.preferredHeight: 12
                                Layout.preferredWidth: chipLabel.implicitWidth + 8
                                radius: 6
                                color: modelData.bg

                                Text {
                                    id: chipLabel
                                    anchors.centerIn: parent
                                    color: modelData.fg
                                    font.pixelSize: 8
                                    font.bold: true
                                    text: modelData.text
                                }
                            }
                        }
                    Text {
                        Layout.fillWidth: true
                        color: videoCard.downloadProg >= 0 ? "#ff99ca" : "#9a9ea6"
                        font.pixelSize: 9
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
                }
                RowLayout {
                    anchors.right: parent.right
                    anchors.rightMargin: 6
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 4

                    Repeater {
                        model: [
                            { deck: "[Channel1]", label: "D1", color: Qt.lighter(Theme.accentColor, 1.2) },
                            { deck: "[Channel2]", label: "D2", color: "#e8650f" }
                        ]
                        delegate: Skin.Button {
                            required property var modelData
                            Layout.preferredHeight: 30
                            Layout.preferredWidth: 46
                            activeColor: modelData.color
                            normalColor: "#ffffff"
                            font.bold: true
                            font.pixelSize: 10
                            text: videoCard.isDownloading
                                ? `${Math.round(videoCard.downloadProg * 100)}%`
                                : modelData.label
                            enabled: !videoCard.isDownloading
                            onClicked: Mixxx.YouTube.downloadToDeck(videoCard.video.id, modelData.deck)
                        }
                    }

                    // Download only: keeps the song in the library without
                    // loading it on a deck.
                    AbstractButton {
                        id: saveButton

                        Layout.preferredHeight: 30
                        Layout.preferredWidth: 46
                        enabled: !videoCard.isDownloading
                        opacity: enabled ? 1.0 : 0.5
                        onClicked: {
                            root.statusMessage = qsTr("Downloading to your library...");
                            Mixxx.YouTube.download(videoCard.video.id);
                        }

                        background: Rectangle {
                            color: saveButton.pressed ? "#1500e676" : "transparent"
                            border.color: "#00e676"
                            border.width: 2
                            radius: 10
                        }
                        contentItem: Item {
                            // Arrow down: a stem over a triangle (the shared
                            // Triangle turned 90 degrees to point down)
                            Item {
                                anchors.centerIn: parent
                                height: 22
                                width: 18

                                Rectangle {
                                    color: "#00e676"
                                    height: 11
                                    width: 4
                                    x: 7
                                    y: 0
                                }
                                Skin.Triangle {
                                    color: "#00e676"
                                    height: 16
                                    pointsRight: true
                                    rotation: 90
                                    width: 9
                                    x: 4.5
                                    y: 6.5
                                }
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

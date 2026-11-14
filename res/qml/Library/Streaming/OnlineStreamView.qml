import QtQuick
import QtQuick.Controls 2.15
import QtQuick.Layouts
import Mixxx 1.0 as Mixxx
import "../../Theme"
import ".." as Library
import "../.." as Skin

Item {
    id: root

    property string currentGenre: "Electronic"
    property bool isCustomUrlMode: false
    property var tracksModel: []
    property string statusMessage: ""
    property bool isBusy: false
    // Last Audius search; empty while trending is shown
    property string lastQuery: ""

    // Pull to refresh / F5: what is shown is fetched again
    function refreshList() {
        root.isBusy = true;
        if (root.lastQuery !== "")
            Mixxx.StreamingManager.searchAudius(root.lastQuery);
        else
            Mixxx.StreamingManager.getAudiusTrending(root.currentGenre);
    }
    property var downloadProgressMap: ({})

    Connections {
        target: Mixxx.StreamingManager

        function onAudiusSearchFinished(success, tracks, errorString) {
            root.isBusy = false;
            if (success) {
                root.tracksModel = tracks;
                root.statusMessage = qsTr("%1 tracks found on streaming").arg(tracks.length);
            } else {
                root.statusMessage = qsTr("❌ Error: %1").arg(errorString);
            }
        }

        function onAudiusTrendingFinished(success, tracks, errorString) {
            root.isBusy = false;
            if (success) {
                root.tracksModel = tracks;
                root.statusMessage = qsTr("%1 trending tracks (%2)").arg(tracks.length).arg(root.currentGenre);
            } else {
                root.statusMessage = qsTr("❌ Error: %1").arg(errorString);
            }
        }

        function onDownloadProgress(trackId, progress, bytesReceived, bytesTotal) {
            var updated = Object.assign({}, root.downloadProgressMap);
            updated[trackId] = progress;
            root.downloadProgressMap = updated;
        }

        function onDownloadFinished(trackId, localPath, targetDeck, autoPlay) {
            var updated = Object.assign({}, root.downloadProgressMap);
            delete updated[trackId];
            root.downloadProgressMap = updated;

            var currentTracks = root.tracksModel.slice();
            for (var i = 0; i < currentTracks.length; i++) {
                if (currentTracks[i].id === trackId) {
                    currentTracks[i].isCached = true;
                    break;
                }
            }
            root.tracksModel = currentTracks;
            root.statusMessage = qsTr("✔ Track ready and loaded into %1").arg(targetDeck === "[Channel1]" ? qsTr("Deck 1") : (targetDeck === "[Channel2]" ? qsTr("Deck 2") : qsTr("cache")));
        }

        function onDownloadFailed(trackId, errorMessage) {
            var updated = Object.assign({}, root.downloadProgressMap);
            delete updated[trackId];
            root.downloadProgressMap = updated;
            root.statusMessage = qsTr("❌ Download error: %1").arg(errorMessage);
        }
    }

    Component.onCompleted: {
        root.isBusy = true;
        root.lastQuery = "";
        Mixxx.StreamingManager.getAudiusTrending("Electronic");
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // ================= TOP HEADER & BRAND =================
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 33
            color: Theme.darkGray3
            border.color: Theme.midGray3
            border.width: 1

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 10
                anchors.rightMargin: 10
                spacing: 8

                Text {
                    text: qsTr("🔥 AUDIUS STREAMING (FREE MUSIC)")
                    font.bold: true
                    font.pixelSize: 11
                    font.letterSpacing: 1
                    color: Theme.activeColor
                }

                Rectangle {
                    Layout.preferredHeight: 22
                    Layout.preferredWidth: liveText.implicitWidth + 14
                    radius: 11
                    color: "#00e67622"
                    border.color: "#00e676"
                    border.width: 1

                    Text {
                        id: liveText
                        anchors.centerIn: parent
                        font.pixelSize: 10
                        font.bold: true
                        color: "#00e676"
                        text: qsTr("● Free online")
                    }
                }

                Item { Layout.fillWidth: true }

                Text {
                    visible: root.statusMessage !== ""
                    text: root.statusMessage
                    font.pixelSize: 9
                    color: Theme.lightGray2
                    elide: Text.ElideRight
                    Layout.maximumWidth: 320
                }

                Skin.Button {
                    Layout.preferredHeight: 23
                    Layout.preferredWidth: 96
                    activeColor: Theme.activeColor
                    normalColor: "#ffffff"
                    font.bold: true
                    font.pixelSize: 9
                    text: root.isBusy ? qsTr("🔄 Loading...") : qsTr("🔄 Trending")
                    enabled: !root.isBusy
                    onClicked: {
                        root.isBusy = true;
                        root.lastQuery = "";
                        Mixxx.StreamingManager.getAudiusTrending(root.currentGenre);
                    }
                }
            }
        }

        // ================= SEARCH & GENRE PILLS =================
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 33
            color: Theme.darkGray3
            border.color: Theme.midGray3
            border.width: 1

            RowLayout {
                anchors.fill: parent
                anchors.margins: 5
                spacing: 6

                // Search Input
                TextField {
                    id: searchInput
                    Layout.preferredWidth: 208
                    Layout.preferredHeight: 24
                    placeholderText: qsTr("Search tracks, remixes, acapellas...")
                    Skin.SearchClearButton {}
                    color: "#ffffff"
                    font.pixelSize: 11
                    background: Rectangle {
                        color: Theme.darkGray2
                        border.color: searchInput.activeFocus ? Theme.libraryDeck2Background : Theme.midGray3
                        border.width: 1
                        radius: 4
                    }
                    onAccepted: {
                        if (text.trim() !== "") {
                            root.lastQuery = text;
                            root.isBusy = true;
                            root.statusMessage = qsTr("Searching Audius...");
                            Mixxx.StreamingManager.searchAudius(text);
                        }
                    }
                }

                Skin.Button {
                    Layout.preferredHeight: 24
                    Layout.preferredWidth: 38
                    normalColor: "#ffffff"
                    text: "🔍"
                    onClicked: searchInput.accepted()
                }

                // Genre Chips Row (Flickable for small screens)
                Flickable {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 24
                    contentWidth: genreRow.implicitWidth
                    clip: true
                    boundsBehavior: Flickable.StopAtBounds

                    Row {
                        id: genreRow
                        spacing: 6

                        Repeater {
                            model: [
                                { name: qsTr("🔥 Trending"), genre: "Electronic" },
                                { name: "🎧 Techno", genre: "Techno" },
                                { name: "🎹 House", genre: "House" },
                                { name: "🥁 Drum & Bass", genre: "Drum & Bass" },
                                { name: "⚡ Electro", genre: "Electro" },
                                { name: "🎤 Trap / Hip-Hop", genre: "Trap" }
                            ]

                            delegate: Rectangle {
                                width: chipText.implicitWidth + 20
                                height: 23
                                radius: 16
                                color: root.currentGenre === modelData.genre && searchInput.text === "" ? "#e65100" : Theme.darkGray2
                                border.color: root.currentGenre === modelData.genre && searchInput.text === "" ? Theme.libraryDeck2Background : Theme.midGray3
                                border.width: 1

                                Text {
                                    id: chipText
                                    anchors.centerIn: parent
                                    text: modelData.name
                                    font.bold: true
                                    font.pixelSize: 9
                                    color: "#ffffff"
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    onClicked: {
                                        searchInput.text = "";
                                        root.currentGenre = modelData.genre;
                                        root.isBusy = true;
                                        root.statusMessage = qsTr("Loading trending tracks...");
                                        root.lastQuery = "";
                                        Mixxx.StreamingManager.getAudiusTrending(modelData.genre);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // ================= TRACKS LIST =================
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            ListView {
                id: tracksList
                anchors.fill: parent
                clip: true
                boundsBehavior: Flickable.DragOverBounds
                model: root.tracksModel

                ScrollBar.vertical: ScrollBar {
                    id: onlineScroll
                    width: 14
                    policy: ScrollBar.AsNeeded
                    contentItem: Rectangle {
                        implicitWidth: 10
                        implicitHeight: 40
                        radius: 5
                        color: onlineScroll.pressed ? Theme.activeColor : Theme.midGray2
                    }
                }

                delegate: Rectangle {
                    id: trackDelegate
                    width: tracksList.width
                    height: 42
                    color: index % 2 === 0 ? Theme.darkGray3 : Theme.darkGray4
                    border.color: Theme.darkGray2
                    border.width: 1

                    readonly property var track: modelData
                    readonly property real downloadProg: root.downloadProgressMap[track.id] !== undefined ? root.downloadProgressMap[track.id] : -1
                    readonly property bool isDownloading: downloadProg >= 0


                    Library.TrackDrag {
                        label: trackDelegate.track.title
                        loadTo: group => Mixxx.StreamingManager.downloadAndLoad(trackDelegate.track.streamUrl, "audius", trackDelegate.track.id, trackDelegate.track.title, trackDelegate.track.artist, group, false)
                        fetch: () => Mixxx.StreamingManager.downloadAndLoad(trackDelegate.track.streamUrl, "audius", trackDelegate.track.id, trackDelegate.track.title, trackDelegate.track.artist, "", false)
                        fetchId: trackDelegate.track.id
                        fetchSource: Mixxx.StreamingManager
                    }

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 8
                        anchors.rightMargin: 8
                        spacing: 8

                        // Cover Art Thumbnail
                        Rectangle {
                            Layout.preferredWidth: 42
                            Layout.preferredHeight: 30
                            radius: 4
                            color: Theme.darkGray2
                            clip: true

                            Image {
                                id: coverImage
                                anchors.fill: parent
                                source: track.coverArtUrl ? track.coverArtUrl : ""
                                fillMode: Image.PreserveAspectCrop
                                asynchronous: true
                                // Decode at thumbnail size, not the full cover
                                sourceSize.width: 96
                                sourceSize.height: 96
                                visible: status === Image.Ready

                                onStatusChanged: {
                                    if (status === Image.Error) {
                                        console.warn("Audius cover art load failed:", track.coverArtUrl);
                                    }
                                }
                            }

                            Text {
                                anchors.centerIn: parent
                                text: "🔥"
                                font.pixelSize: 14
                                visible: !track.coverArtUrl || coverImage.status === Image.Error
                            }
                        }

                        // Title, Artist
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 2

                            Text {
                                Layout.fillWidth: true
                                text: track.title
                                font.bold: true
                                font.pixelSize: 11
                                color: "#ffffff"
                                elide: Text.ElideRight
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 6

                                Text {
                                    text: track.artist
                                    font.pixelSize: 9
                                    color: Theme.yellow
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                            }
                        }

                        // Genre Pill
                        Rectangle {
                            visible: track.genre !== undefined && track.genre !== ""
                            Layout.preferredHeight: 18
                            Layout.preferredWidth: genreText.implicitWidth + 8
                            color: Theme.darkGray2
                            radius: 3

                            Text {
                                id: genreText
                                anchors.centerIn: parent
                                text: track.genre || ""
                                font.pixelSize: 9
                                font.bold: true
                                color: Theme.libraryDeck2Background
                            }
                        }

                        // Duration
                        Text {
                            text: {
                                var s = track.duration || 0;
                                var m = Math.floor(s / 60);
                                var sec = Math.floor(s % 60);
                                return m + ":" + (sec < 10 ? "0" : "") + sec;
                            }
                            font.pixelSize: 9
                            color: Theme.lightGray3
                        }

                        // Cache status badge
                        Rectangle {
                            Layout.preferredHeight: 20
                            Layout.preferredWidth: cacheStatusText.implicitWidth + 10
                            radius: 10
                            color: track.isCached ? "#00e67622" : "#ff910022"
                            border.color: track.isCached ? "#00e676" : Theme.libraryDeck2Background
                            border.width: 1

                            Text {
                                id: cacheStatusText
                                anchors.centerIn: parent
                                font.pixelSize: 9
                                font.bold: true
                                color: track.isCached ? "#00e676" : Theme.libraryDeck2Background
                                text: track.isCached ? qsTr("💾 Ready") : qsTr("☁️ Stream")
                            }
                        }

                        // Load Deck 1 Button
                        Skin.Button {
                            Layout.preferredHeight: 24
                            Layout.preferredWidth: 80
                            activeColor: Theme.libraryDeck1Background
                            normalColor: "#ffffff"
                            font.bold: true
                            font.pixelSize: 9
                            arrowLeft: true
                            text: "D1"
                            enabled: !trackDelegate.isDownloading
                            onClicked: {
                                Mixxx.StreamingManager.downloadAndLoad(
                                    track.streamUrl, "audius", track.id, track.title, track.artist, "[Channel1]", false
                                );
                            }
                        }

                        // Load Deck 2 Button
                        Skin.Button {
                            Layout.preferredHeight: 24
                            Layout.preferredWidth: 80
                            activeColor: Theme.libraryDeck2Background
                            normalColor: "#ffffff"
                            font.bold: true
                            font.pixelSize: 9
                            arrowRight: true
                            text: "D2"
                            enabled: !trackDelegate.isDownloading
                            onClicked: {
                                Mixxx.StreamingManager.downloadAndLoad(
                                    track.streamUrl, "audius", track.id, track.title, track.artist, "[Channel2]", false
                                );
                            }
                        }
                    }

                    // Progress bar across bottom when downloading
                    Rectangle {
                        anchors.bottom: parent.bottom
                        anchors.left: parent.left
                        height: 3
                        visible: trackDelegate.isDownloading
                        width: parent.width * Math.max(0.05, trackDelegate.downloadProg)
                        color: Theme.libraryDeck2Background
                    }
                }

                // Empty / Placeholder View
                Rectangle {
                    anchors.centerIn: parent
                    width: parent.width * 0.7
                    height: 86
                    color: "transparent"
                    visible: tracksList.count === 0 && !root.isBusy

                    ColumnLayout {
                        anchors.centerIn: parent
                        spacing: 8

                        Text {
                            text: qsTr("🔥 Explore music by genre or search for any track or artist")
                            font.pixelSize: 11
                            color: Theme.lightGray3
                            horizontalAlignment: Text.AlignHCenter
                            Layout.alignment: Qt.AlignHCenter
                        }
                    }
                }

                Skin.PullToRefresh {
                    flickable: tracksList
                    busy: root.isBusy
                    onRefresh: root.refreshList()
                }
            }

            // Loading overlay
            Rectangle {
                anchors.centerIn: parent
                width: 140
                height: 29
                radius: 6
                color: Theme.darkGray3
                border.color: Theme.libraryDeck2Background
                border.width: 1
                visible: root.isBusy

                RowLayout {
                    anchors.centerIn: parent
                    spacing: 8

                    Text {
                        text: "🔄"
                        font.pixelSize: 14
                        color: Theme.libraryDeck2Background
                        RotationAnimation on rotation {
                            from: 0
                            to: 360
                            duration: 1000
                            loops: Animation.Infinite
                            running: root.isBusy
                        }
                    }

                    Text {
                        text: qsTr("Connecting...")
                        font.bold: true
                        font.pixelSize: 10
                        color: "#ffffff"
                    }
                }
            }
        }
    }
}

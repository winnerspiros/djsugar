import QtQuick
import QtQuick.Controls 2.15
import QtQuick.Layouts
import Mixxx 1.0 as Mixxx
import "../../Theme"
import "../.." as Skin

Item {
    id: root

    property string currentGenre: "Electronic"
    property bool isCustomUrlMode: false
    property var tracksModel: []
    property string statusMessage: ""
    property bool isBusy: false
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
        Mixxx.StreamingManager.getAudiusTrending("Electronic");
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // ================= TOP HEADER & BRAND =================
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 46
            color: "#1c202a"
            border.color: "#282d3b"
            border.width: 1

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 10
                anchors.rightMargin: 10
                spacing: 8

                Text {
                    text: qsTr("🔥 AUDIUS STREAMING (FREE MUSIC)")
                    font.bold: true
                    font.pixelSize: 13
                    font.letterSpacing: 1
                    color: "#ea80fc"
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
                    font.pixelSize: 11
                    color: "#b0bec5"
                    elide: Text.ElideRight
                    Layout.maximumWidth: 320
                }

                Skin.Button {
                    Layout.preferredHeight: 32
                    Layout.preferredWidth: 120
                    activeColor: "#ea80fc"
                    backgroundColor: "#2a3040"
                    normalColor: "#ffffff"
                    font.bold: true
                    font.pixelSize: 11
                    text: root.isBusy ? qsTr("🔄 Loading...") : qsTr("🔄 Trending")
                    enabled: !root.isBusy
                    onClicked: {
                        root.isBusy = true;
                        Mixxx.StreamingManager.getAudiusTrending(root.currentGenre);
                    }
                }
            }
        }

        // ================= SEARCH & GENRE PILLS =================
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 46
            color: "#181b24"
            border.color: "#232834"
            border.width: 1

            RowLayout {
                anchors.fill: parent
                anchors.margins: 5
                spacing: 6

                // Search Input
                TextField {
                    id: searchInput
                    Layout.preferredWidth: 260
                    Layout.preferredHeight: 34
                    placeholderText: qsTr("Search tracks, remixes, acapellas...")
                    color: "#ffffff"
                    font.pixelSize: 13
                    background: Rectangle {
                        color: "#212632"
                        border.color: searchInput.activeFocus ? "#ff9100" : "#2e3646"
                        border.width: 1
                        radius: 4
                    }
                    onAccepted: {
                        if (text.trim() !== "") {
                            root.isBusy = true;
                            root.statusMessage = qsTr("Searching Audius...");
                            Mixxx.StreamingManager.searchAudius(text);
                        }
                    }
                }

                Skin.Button {
                    Layout.preferredHeight: 34
                    Layout.preferredWidth: 38
                    backgroundColor: "#2a3140"
                    normalColor: "#ffffff"
                    text: "🔍"
                    onClicked: searchInput.accepted()
                }

                // Genre Chips Row (Flickable for small screens)
                Flickable {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 34
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
                                height: 32
                                radius: 16
                                color: root.currentGenre === modelData.genre && searchInput.text === "" ? "#e65100" : "#222734"
                                border.color: root.currentGenre === modelData.genre && searchInput.text === "" ? "#ff9100" : "#323a4d"
                                border.width: 1

                                Text {
                                    id: chipText
                                    anchors.centerIn: parent
                                    text: modelData.name
                                    font.bold: true
                                    font.pixelSize: 11
                                    color: "#ffffff"
                                }

                                MouseArea {
                                    anchors.fill: parent
                                    onClicked: {
                                        searchInput.text = "";
                                        root.currentGenre = modelData.genre;
                                        root.isBusy = true;
                                        root.statusMessage = qsTr("Loading trending tracks...");
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
                boundsBehavior: Flickable.StopAtBounds
                model: root.tracksModel

                ScrollBar.vertical: ScrollBar {
                    id: onlineScroll
                    width: 14
                    policy: ScrollBar.AsNeeded
                    contentItem: Rectangle {
                        implicitWidth: 10
                        implicitHeight: 40
                        radius: 5
                        color: onlineScroll.pressed ? "#ea80fc" : "#4a5568"
                    }
                }

                delegate: Rectangle {
                    id: trackDelegate
                    width: tracksList.width
                    height: 58
                    color: index % 2 === 0 ? "#171a22" : "#13161e"
                    border.color: "#222734"
                    border.width: 1

                    readonly property var track: modelData
                    readonly property real downloadProg: root.downloadProgressMap[track.id] !== undefined ? root.downloadProgressMap[track.id] : -1
                    readonly property bool isDownloading: downloadProg >= 0

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 8
                        anchors.rightMargin: 8
                        spacing: 8

                        // Cover Art Thumbnail
                        Rectangle {
                            Layout.preferredWidth: 42
                            Layout.preferredHeight: 42
                            radius: 4
                            color: "#262d3d"
                            clip: true

                            Image {
                                anchors.fill: parent
                                source: track.coverArtUrl ? track.coverArtUrl : ""
                                fillMode: Image.PreserveAspectCrop
                                asynchronous: true
                                visible: status === Image.Ready
                            }

                            Text {
                                anchors.centerIn: parent
                                text: "🔥"
                                font.pixelSize: 16
                                visible: !track.coverArtUrl
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
                                font.pixelSize: 13
                                color: "#ffffff"
                                elide: Text.ElideRight
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 6

                                Text {
                                    text: track.artist
                                    font.pixelSize: 11
                                    color: "#ffab40"
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
                            color: "#2d1f0a"
                            radius: 3

                            Text {
                                id: genreText
                                anchors.centerIn: parent
                                text: track.genre || ""
                                font.pixelSize: 9
                                font.bold: true
                                color: "#ffb74d"
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
                            font.pixelSize: 11
                            color: "#78909c"
                        }

                        // Cache status badge
                        Rectangle {
                            Layout.preferredHeight: 20
                            Layout.preferredWidth: cacheStatusText.implicitWidth + 10
                            radius: 10
                            color: track.isCached ? "#00e67622" : "#ff910022"
                            border.color: track.isCached ? "#00e676" : "#ff9100"
                            border.width: 1

                            Text {
                                id: cacheStatusText
                                anchors.centerIn: parent
                                font.pixelSize: 9
                                font.bold: true
                                color: track.isCached ? "#00e676" : "#ff9100"
                                text: track.isCached ? qsTr("💾 Ready") : qsTr("☁️ Stream")
                            }
                        }

                        // Load Deck 1 Button
                        Skin.Button {
                            Layout.preferredHeight: 34
                            Layout.preferredWidth: 80
                            activeColor: "#64b5f6"
                            backgroundColor: "#1565c0"
                            normalColor: "#ffffff"
                            font.bold: true
                            font.pixelSize: 11
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
                            Layout.preferredHeight: 34
                            Layout.preferredWidth: 80
                            activeColor: "#ffb74d"
                            backgroundColor: "#e65100"
                            normalColor: "#ffffff"
                            font.bold: true
                            font.pixelSize: 11
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
                        color: "#ff9100"
                    }
                }

                // Empty / Placeholder View
                Rectangle {
                    anchors.centerIn: parent
                    width: parent.width * 0.7
                    height: 120
                    color: "transparent"
                    visible: tracksList.count === 0 && !root.isBusy

                    ColumnLayout {
                        anchors.centerIn: parent
                        spacing: 8

                        Text {
                            text: qsTr("🔥 Explore music by genre or search for any track or artist")
                            font.pixelSize: 13
                            color: "#78909c"
                            horizontalAlignment: Text.AlignHCenter
                            Layout.alignment: Qt.AlignHCenter
                        }
                    }
                }
            }

            // Loading overlay
            Rectangle {
                anchors.centerIn: parent
                width: 140
                height: 40
                radius: 6
                color: "#1e2433"
                border.color: "#ff9100"
                border.width: 1
                visible: root.isBusy

                RowLayout {
                    anchors.centerIn: parent
                    spacing: 8

                    Text {
                        text: "🔄"
                        font.pixelSize: 16
                        color: "#ff9100"
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
                        font.pixelSize: 12
                        color: "#ffffff"
                    }
                }
            }
        }
    }
}

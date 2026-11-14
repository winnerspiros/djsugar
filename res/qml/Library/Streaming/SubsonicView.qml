import QtQuick
import QtQuick.Controls 2.15
import QtQuick.Layouts
import Mixxx 1.0 as Mixxx
import "../../Theme"
import ".." as Library
import "../.." as Skin

Item {
    id: root

    property bool showSettings: !Mixxx.StreamingManager.isSubsonicConnected
    property string activeMode: "search" // "search", "playlists", "playlist_tracks"
    property var tracksModel: []
    property var playlistsModel: []
    property string currentPlaylistName: ""
    property var currentPlaylistId
    // The search shown; refresh repeats it, not what the field holds now
    property string searchedQuery: ""
    property string statusMessage: ""
    property bool isBusy: false
    property var downloadProgressMap: ({})

    // Smart box of the streaming hub (WebView)
    function search(query) {
        searchInput.text = query;
        root.searchedQuery = query;
        root.activeMode = "search";
        root.isBusy = true;
        root.statusMessage = qsTr("Searching...");
        Mixxx.StreamingManager.searchSubsonic(query);
    }
    // Pull to refresh / F5: what is shown is fetched again
    function refreshList() {
        if (root.activeMode === "playlists") {
            root.isBusy = true;
            Mixxx.StreamingManager.getSubsonicPlaylists();
        } else if (root.activeMode === "playlist_tracks") {
            root.isBusy = true;
            Mixxx.StreamingManager.getSubsonicPlaylistSongs(root.currentPlaylistId);
        } else if (root.searchedQuery.trim() !== "") {
            root.isBusy = true;
            Mixxx.StreamingManager.searchSubsonic(root.searchedQuery);
        }
    }
    function openServer(url) {
        // Only the field: Connect / editingFinished save it, a stray link
        // never replaces a working server
        serverUrlField.text = url;
        root.showSettings = true;
        userField.forceActiveFocus();
    }

    Connections {
        target: Mixxx.StreamingManager

        function onSubsonicConnectionTested(success, message, version) {
            root.isBusy = false;
            root.statusMessage = success
                ? ("✔ " + message + (version ? " (" + version + ")" : ""))
                : ("❌ " + message);
            if (success) {
                root.showSettings = false;
            }
        }

        function onSubsonicSearchFinished(success, songs, errorString) {
            root.isBusy = false;
            if (success) {
                root.tracksModel = songs;
                root.activeMode = "search";
                root.statusMessage = qsTr("%1 tracks found").arg(songs.length);
            } else {
                root.statusMessage = qsTr("❌ Error: %1").arg(errorString);
            }
        }

        function onSubsonicPlaylistsFinished(success, playlists, errorString) {
            root.isBusy = false;
            if (success) {
                root.playlistsModel = playlists;
                root.activeMode = "playlists";
                root.statusMessage = qsTr("%1 playlists available").arg(playlists.length);
            } else {
                root.statusMessage = qsTr("❌ Error: %1").arg(errorString);
            }
        }

        function onSubsonicPlaylistSongsFinished(success, songs, errorString) {
            root.isBusy = false;
            if (success) {
                root.tracksModel = songs;
                root.activeMode = "playlist_tracks";
                root.statusMessage = qsTr("%1 tracks in %2").arg(songs.length).arg(root.currentPlaylistName);
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

            // QmlStreamingProxy already loaded the track safely into targetDeck in C++

            // Refresh cached indicator in tracksModel
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

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // ================= TOP HEADER & TOGGLE BAR =================
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 33
            color: "#1c202a"
            border.color: "#282d3b"
            border.width: 1

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 10
                anchors.rightMargin: 10
                spacing: 8

                Text {
                    text: "☁️ NAVIDROME / SUBSONIC"
                    font.bold: true
                    font.pixelSize: 11
                    font.letterSpacing: 1
                    color: "#00e5ff"
                }

                Rectangle {
                    Layout.preferredHeight: 22
                    Layout.preferredWidth: statusText.implicitWidth + 14
                    radius: 11
                    color: Mixxx.StreamingManager.isSubsonicConnected ? "#00e67622" : "#ff525222"
                    border.color: Mixxx.StreamingManager.isSubsonicConnected ? "#00e676" : "#ff5252"
                    border.width: 1

                    Text {
                        id: statusText
                        anchors.centerIn: parent
                        font.pixelSize: 10
                        font.bold: true
                        color: Mixxx.StreamingManager.isSubsonicConnected ? "#00e676" : "#ff5252"
                        text: Mixxx.StreamingManager.isSubsonicConnected ? qsTr("● Connected") : qsTr("● Disconnected")
                    }
                }

                Item { Layout.fillWidth: true }

                Text {
                    visible: root.statusMessage !== ""
                    text: root.statusMessage
                    font.pixelSize: 9
                    color: "#b0bec5"
                    elide: Text.ElideRight
                    Layout.maximumWidth: 280
                }

                Skin.Button {
                    Layout.preferredHeight: 23
                    Layout.preferredWidth: 80
                    activeColor: "#00e5ff"
                    normalColor: "#ffffff"
                    font.bold: true
                    text: root.showSettings ? qsTr("▲ Hide") : qsTr("⚙ Settings")
                    onClicked: root.showSettings = !root.showSettings
                }
            }
        }

        // ================= SETTINGS DRAWER =================
        Rectangle {
            id: settingsDrawer
            Layout.fillWidth: true
            Layout.preferredHeight: root.showSettings ? implicitHeight : 0
            implicitHeight: 90
            visible: root.showSettings
            clip: true
            color: "#141720"
            border.color: "#262c3a"
            border.width: 1

            Behavior on Layout.preferredHeight {
                NumberAnimation { duration: 200 }
            }

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 8
                spacing: 6

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    TextField {
                        id: serverUrlField
                        Layout.fillWidth: true
                        Layout.preferredHeight: 24
                        placeholderText: qsTr("Server URL (e.g. http://192.168.1.50:4533)")
                        text: Mixxx.StreamingManager.subsonicUrl
                        color: "#ffffff"
                        font.pixelSize: 10
                        background: Rectangle {
                            color: "#1f2430"
                            border.color: serverUrlField.activeFocus ? "#00e5ff" : "#374151"
                            border.width: 1
                            radius: 4
                        }
                        onEditingFinished: Mixxx.StreamingManager.subsonicUrl = text
                    }

                    TextField {
                        id: userField
                        Layout.preferredWidth: 104
                        Layout.preferredHeight: 24
                        placeholderText: qsTr("Username")
                        text: Mixxx.StreamingManager.subsonicUsername
                        color: "#ffffff"
                        font.pixelSize: 10
                        background: Rectangle {
                            color: "#1f2430"
                            border.color: userField.activeFocus ? "#00e5ff" : "#374151"
                            border.width: 1
                            radius: 4
                        }
                        onEditingFinished: Mixxx.StreamingManager.subsonicUsername = text
                    }

                    TextField {
                        id: passField
                        Layout.preferredWidth: 104
                        Layout.preferredHeight: 24
                        placeholderText: qsTr("Password")
                        echoMode: TextInput.Password
                        text: Mixxx.StreamingManager.subsonicPassword
                        color: "#ffffff"
                        font.pixelSize: 10
                        background: Rectangle {
                            color: "#1f2430"
                            border.color: passField.activeFocus ? "#00e5ff" : "#374151"
                            border.width: 1
                            radius: 4
                        }
                        onEditingFinished: Mixxx.StreamingManager.subsonicPassword = text
                    }

                    Skin.Button {
                        Layout.preferredHeight: 24
                        Layout.preferredWidth: 88
                        activeColor: "#00e676"
                        normalColor: "#ffffff"
                        font.bold: true
                        text: root.isBusy ? qsTr("🔄 Connecting") : qsTr("✔ Connect")
                        enabled: !root.isBusy
                        onClicked: {
                            Mixxx.StreamingManager.subsonicUrl = serverUrlField.text;
                            Mixxx.StreamingManager.subsonicUsername = userField.text;
                            Mixxx.StreamingManager.subsonicPassword = passField.text;
                            root.isBusy = true;
                            root.statusMessage = qsTr("Connecting to the server...");
                            Mixxx.StreamingManager.testSubsonicConnection();
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    Text {
                        text: qsTr("💡 Compatible with Navidrome, Funkwhale, Ampache, Subsonic, Airsonic and Nextcloud Music.")
                        font.pixelSize: 9
                        color: "#78909c"
                        Layout.fillWidth: true
                    }

                    Skin.Button {
                        Layout.preferredHeight: 26
                        Layout.preferredWidth: 112
                        normalColor: "#90a4ae"
                        font.pixelSize: 10
                        text: qsTr("🧹 Clear cache")
                        onClicked: {
                            Mixxx.StreamingManager.clearCache();
                            root.statusMessage = qsTr("Streaming cache cleared");
                        }
                    }
                }
            }
        }

        // ================= SEARCH & NAVIGATION TOOLBAR =================
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 33
            color: "#181b24"
            border.color: "#232834"
            border.width: 1

            RowLayout {
                anchors.fill: parent
                anchors.margins: 5
                spacing: 8

                Skin.Button {
                    Layout.preferredHeight: 24
                    Layout.preferredWidth: 80
                    activeColor: "#00e5ff"
                    normalColor: "#ffffff"
                    font.bold: true
                    text: qsTr("🔍 Search")
                    onClicked: {
                        root.activeMode = "search";
                        if (searchInput.text.trim() !== "") {
                            root.searchedQuery = searchInput.text;
                            root.isBusy = true;
                            Mixxx.StreamingManager.searchSubsonic(searchInput.text);
                        }
                    }
                }

                Skin.Button {
                    Layout.preferredHeight: 24
                    Layout.preferredWidth: 84
                    activeColor: "#00e5ff"
                    normalColor: "#ffffff"
                    font.bold: true
                    text: qsTr("📜 Playlists")
                    onClicked: {
                        root.isBusy = true;
                        Mixxx.StreamingManager.getSubsonicPlaylists();
                    }
                }

                // Search Box
                TextField {
                    id: searchInput
                    Layout.fillWidth: true
                    Layout.preferredHeight: 24
                    placeholderText: qsTr("Search artists, tracks or albums in your cloud...")
                    Skin.SearchClearButton {}
                    color: "#ffffff"
                    font.pixelSize: 11
                    background: Rectangle {
                        color: "#212632"
                        border.color: searchInput.activeFocus ? "#00e5ff" : "#2e3646"
                        border.width: 1
                        radius: 4
                    }
                    onAccepted: {
                        if (text.trim() !== "") {
                            root.searchedQuery = text;
                            root.isBusy = true;
                            root.statusMessage = qsTr("Searching...");
                            Mixxx.StreamingManager.searchSubsonic(text);
                        }
                    }
                }

                Skin.Button {
                    Layout.preferredHeight: 24
                    Layout.preferredWidth: 40
                    normalColor: "#ffffff"
                    text: "🔍"
                    onClicked: searchInput.accepted()
                }
            }
        }

        // ================= PLAYLIST HEADER (if in playlist tracks) =================
        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: root.activeMode === "playlist_tracks" ? 34 : 0
            visible: root.activeMode === "playlist_tracks"
            color: "#1c2833"
            clip: true

            RowLayout {
                anchors.fill: parent
                anchors.margins: 4
                spacing: 8

                Skin.Button {
                    Layout.preferredHeight: 26
                    Layout.preferredWidth: 80
                    normalColor: "#ffffff"
                    font.bold: true
                    arrowLeft: true
                    text: qsTr("Back")
                    onClicked: root.activeMode = "playlists"
                }

                Text {
                    text: "📁 " + root.currentPlaylistName
                    font.bold: true
                    font.pixelSize: 11
                    color: "#80d8ff"
                    Layout.fillWidth: true
                }
            }
        }

        // ================= CONTENT AREA =================
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            // --- TRACKS LISTVIEW (Search & Playlist Tracks) ---
            ListView {
                id: tracksList
                anchors.fill: parent
                visible: root.activeMode !== "playlists"
                clip: true
                boundsBehavior: Flickable.DragOverBounds
                model: root.tracksModel

                ScrollBar.vertical: ScrollBar {
                    id: subScroll
                    width: 14
                    policy: ScrollBar.AsNeeded
                    contentItem: Rectangle {
                        implicitWidth: 10
                        implicitHeight: 40
                        radius: 5
                        color: subScroll.pressed ? "#00e5ff" : "#4a5568"
                    }
                }

                delegate: Rectangle {
                    id: trackDelegate
                    width: tracksList.width
                    height: 40
                    color: index % 2 === 0 ? "#171a22" : "#13161e"
                    border.color: "#222734"
                    border.width: 1

                    readonly property var song: modelData
                    readonly property real downloadProg: root.downloadProgressMap[song.id] !== undefined ? root.downloadProgressMap[song.id] : -1
                    readonly property bool isDownloading: downloadProg >= 0

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 8
                        anchors.rightMargin: 8
                        spacing: 8

                        // Cover Art Thumbnail or Icon

                    Library.TrackDrag {
                        label: trackDelegate.song.title
                        loadTo: group => Mixxx.StreamingManager.downloadAndLoad(trackDelegate.song.streamUrl, "subsonic", trackDelegate.song.id, trackDelegate.song.title, trackDelegate.song.artist, group, false)
                        fetch: () => Mixxx.StreamingManager.downloadAndLoad(trackDelegate.song.streamUrl, "subsonic", trackDelegate.song.id, trackDelegate.song.title, trackDelegate.song.artist, "", false)
                        fetchId: trackDelegate.song.id
                        fetchSource: Mixxx.StreamingManager
                    }
                        Rectangle {
                            Layout.preferredWidth: 40
                            Layout.preferredHeight: 29
                            radius: 4
                            color: "#262d3d"
                            clip: true

                            Image {
                                anchors.fill: parent
                                source: song.coverArtUrl ? song.coverArtUrl : ""
                                fillMode: Image.PreserveAspectCrop
                                asynchronous: true
                                // Decode at thumbnail size, not the full cover
                                sourceSize.width: 96
                                sourceSize.height: 96
                                visible: status === Image.Ready
                            }

                            Text {
                                anchors.centerIn: parent
                                text: "🎵"
                                font.pixelSize: 14
                                visible: !song.coverArtUrl
                            }
                        }

                        // Title, Artist, Album
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 2

                            Text {
                                Layout.fillWidth: true
                                text: song.title
                                font.bold: true
                                font.pixelSize: 11
                                color: "#ffffff"
                                elide: Text.ElideRight
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 6

                                Text {
                                    text: song.artist + (song.album ? (" • " + song.album) : "")
                                    font.pixelSize: 9
                                    color: "#90a4ae"
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                            }
                        }

                        // Format / Bitrate Pill
                        Rectangle {
                            Layout.preferredHeight: 18
                            Layout.preferredWidth: formatText.implicitWidth + 8
                            color: "#1e293b"
                            radius: 3

                            Text {
                                id: formatText
                                anchors.centerIn: parent
                                text: (song.suffix ? song.suffix : "MP3") + (song.bitRate ? (" " + song.bitRate + "k") : "")
                                font.pixelSize: 9
                                font.bold: true
                                color: "#38bdf8"
                            }
                        }

                        // Duration
                        Text {
                            text: {
                                var s = song.duration || 0;
                                var m = Math.floor(s / 60);
                                var sec = Math.floor(s % 60);
                                return m + ":" + (sec < 10 ? "0" : "") + sec;
                            }
                            font.pixelSize: 9
                            color: "#78909c"
                        }

                        // Offline Cache Status Pill
                        Rectangle {
                            Layout.preferredHeight: 20
                            Layout.preferredWidth: cacheStatusText.implicitWidth + 10
                            radius: 10
                            color: song.isCached ? "#00e67622" : "#0288d122"
                            border.color: song.isCached ? "#00e676" : "#0288d1"
                            border.width: 1

                            Text {
                                id: cacheStatusText
                                anchors.centerIn: parent
                                font.pixelSize: 9
                                font.bold: true
                                color: song.isCached ? "#00e676" : "#0288d1"
                                text: song.isCached ? qsTr("💾 Ready") : qsTr("☁️ Cloud")
                            }
                        }

                        // Load Deck 1 Button
                        Skin.Button {
                            Layout.preferredHeight: 24
                            Layout.preferredWidth: 80
                            activeColor: "#64b5f6"
                            normalColor: "#ffffff"
                            font.bold: true
                            font.pixelSize: 9
                            arrowLeft: true
                            text: "D1"
                            enabled: !trackDelegate.isDownloading
                            onClicked: {
                                Mixxx.StreamingManager.downloadAndLoad(
                                    song.streamUrl, "subsonic", song.id, song.title, song.artist, "[Channel1]", false
                                );
                            }
                        }

                        // Load Deck 2 Button
                        Skin.Button {
                            Layout.preferredHeight: 24
                            Layout.preferredWidth: 80
                            activeColor: "#ffb74d"
                            normalColor: "#ffffff"
                            font.bold: true
                            font.pixelSize: 9
                            arrowRight: true
                            text: "D2"
                            enabled: !trackDelegate.isDownloading
                            onClicked: {
                                Mixxx.StreamingManager.downloadAndLoad(
                                    song.streamUrl, "subsonic", song.id, song.title, song.artist, "[Channel2]", false
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
                        color: "#00e5ff"
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
                            text: Mixxx.StreamingManager.isSubsonicConnected
                                ? qsTr("🔍 Type an artist, title or album to search your server")
                                : qsTr("☁️ Connect your Navidrome / Subsonic server in the settings above")
                            font.pixelSize: 11
                            color: "#78909c"
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

            // --- PLAYLISTS LISTVIEW ---
            ListView {
                id: playlistsList
                anchors.fill: parent
                visible: root.activeMode === "playlists"
                clip: true
                boundsBehavior: Flickable.DragOverBounds
                model: root.playlistsModel

                ScrollBar.vertical: ScrollBar {
                    width: 6
                    policy: ScrollBar.AsNeeded
                }

                delegate: Rectangle {
                    width: playlistsList.width
                    height: 37
                    color: index % 2 === 0 ? "#171a22" : "#13161e"
                    border.color: "#222734"
                    border.width: 1

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 12
                        anchors.rightMargin: 12
                        spacing: 6

                        Text {
                            text: "📜"
                            font.pixelSize: 18
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 2

                            Text {
                                text: modelData.name
                                font.bold: true
                                font.pixelSize: 12
                                color: "#ffffff"
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }

                            Text {
                                text: qsTr("%1 tracks").arg(modelData.songCount)
                                font.pixelSize: 9
                                color: "#90a4ae"
                            }
                        }

                        Skin.Button {
                            Layout.preferredHeight: 23
                            Layout.preferredWidth: 76
                            activeColor: "#00e5ff"
                            normalColor: "#ffffff"
                            font.bold: true
                            font.pixelSize: 9
                            arrowRight: true
                            text: qsTr("Show tracks")
                            onClicked: {
                                root.currentPlaylistName = modelData.name;
                                root.currentPlaylistId = modelData.id;
                                root.isBusy = true;
                                root.statusMessage = qsTr("Loading %1...").arg(modelData.name);
                                Mixxx.StreamingManager.getSubsonicPlaylistSongs(modelData.id);
                            }
                        }
                    }
                }

                Skin.PullToRefresh {
                    flickable: playlistsList
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
                color: "#1e2433"
                border.color: "#00e5ff"
                border.width: 1
                visible: root.isBusy

                RowLayout {
                    anchors.centerIn: parent
                    spacing: 8

                    Text {
                        text: "🔄"
                        font.pixelSize: 14
                        color: "#00e5ff"
                        RotationAnimation on rotation {
                            from: 0
                            to: 360
                            duration: 1000
                            loops: Animation.Infinite
                            running: root.isBusy
                        }
                    }

                    Text {
                        text: qsTr("Loading...")
                        font.bold: true
                        font.pixelSize: 10
                        color: "#ffffff"
                    }
                }
            }
        }
    }
}

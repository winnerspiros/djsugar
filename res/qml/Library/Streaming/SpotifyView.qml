import Mixxx 1.0 as Mixxx
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../.." as Skin
import "../../Theme"
import ".." as Library

// Spotify: log in once on Spotify's login page, then search, Liked Songs
// and playlists. D1/D2 download the track's own Spotify
// audio (Ogg Vorbis 320 with Premium, else from YouTube Music) and load it to the deck.
Item {
    id: root

    readonly property color spotifyGreen: "#1db954"
    property string targetDeck: "[Channel1]"
    property string statusMessage: ""
    property bool isBusy: false
    property bool isLoadingMore: false
    // "search", "liked", "playlists", "playlist"
    property string listKind: "liked"
    property string playlistName: ""
    property string playlistId: ""
    // What the rows in resultsModel are; listKind changes before they do
    property bool rowsArePlaylists: false
    property var progress: ({})

    function openList(kind) {
        root.listKind = kind;
        root.isBusy = true;
        root.statusMessage = qsTr("Loading...");
        if (kind === "liked")
            Mixxx.Spotify.likedSongs();
        else if (kind === "playlists")
            Mixxx.Spotify.playlists();
    }
    // The search shown; refresh repeats it, not what the field holds now
    property string searchedQuery: ""
    function doSearch(text = searchField.text) {
        const query = text.trim();
        if (query.length === 0)
            return;
        root.searchedQuery = query;
        root.listKind = "search";
        root.isBusy = true;
        root.statusMessage = qsTr("Searching...");
        Mixxx.Spotify.search(query);
    }
    // Pull to refresh / F5: what is shown is fetched again
    function refreshList() {
        if (root.listKind === "search")
            root.doSearch(root.searchedQuery);
        else if (root.listKind === "playlist")
            root.openPlaylist(root.playlistId, root.playlistName);
        else
            root.openList(root.listKind);
    }
    function openPlaylist(id, name) {
        root.playlistId = id;
        root.listKind = "playlist";
        root.playlistName = name;
        root.isBusy = true;
        root.statusMessage = qsTr("Loading %1...").arg(name);
        Mixxx.Spotify.playlistTracks(id);
    }
    function maybeLoadMore() {
        if (root.listKind === "playlists" || root.isBusy || root.isLoadingMore || !Mixxx.Spotify.hasMore())
            return;
        if (resultList.contentHeight - resultList.contentY - resultList.height > 900)
            return;
        root.isLoadingMore = true;
        Mixxx.Spotify.loadMore();
    }
    function formatDuration(sec) {
        const m = Math.floor(sec / 60);
        const s = sec % 60;
        return m + ":" + (s < 10 ? "0" : "") + s;
    }
    function loadRow(row, group) {
        const item = resultsModel.get(row);
        if (root.rowsArePlaylists) {
            root.openPlaylist(item.sid, item.title);
            return;
        }
        Mixxx.Spotify.loadToDeck({
            "id": item.sid,
            "uri": item.uri,
            "title": item.title,
            "artist": item.artist,
            "album": item.album
        }, group);
    }

    // Controller: BROWSE moves the highlighted row, LOAD opens a playlist or
    // loads the track to the deck the button belongs to
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
        if (row >= 0 && row < resultsModel.count)
            root.loadRow(row, group);
    }

    Component.onCompleted: {
        if (Mixxx.Spotify.loggedIn)
            root.openList("liked");
    }

    ListModel {
        id: resultsModel
    }

    Connections {
        function onLoginChanged() {
            if (Mixxx.Spotify.loggedIn && resultsModel.count === 0 && !root.isBusy)
                root.openList("liked");
        }
        function onLoginFailed(error) {
            root.isBusy = false;
            root.isLoadingMore = false;
            root.statusMessage = qsTr("Spotify login failed: %1").arg(error);
        }
        function onTracksLoaded(tracks, append) {
            root.isBusy = false;
            root.isLoadingMore = false;
            if (!append) {
                root.rowsArePlaylists = false;
                resultsModel.clear();
                resultList.currentIndex = -1;
            }
            for (const track of tracks) {
                resultsModel.append({
                    "sid": track.id,
                    "uri": track.uri,
                    "title": track.title,
                    "artist": track.artist,
                    "album": track.album,
                    "durationSec": track.durationSec,
                    "artwork": track.artwork,
                    "count": 0
                });
            }
            if (!append)
                resultList.positionViewAtBeginning();
            root.statusMessage = resultsModel.count > 0 ? qsTr("%1 tracks").arg(resultsModel.count) : qsTr("No tracks");
        }
        function onPlaylistsLoaded(playlists) {
            root.isBusy = false;
            root.rowsArePlaylists = true;
            resultsModel.clear();
            resultList.currentIndex = -1;
            for (const playlist of playlists) {
                resultsModel.append({
                    "sid": playlist.id,
                    "uri": "",
                    "title": playlist.name,
                    "artist": "",
                    "album": "",
                    "durationSec": 0,
                    "artwork": playlist.artwork,
                    "count": playlist.count
                });
            }
            root.statusMessage = qsTr("%1 playlists").arg(resultsModel.count);
        }
        function onFailed(error) {
            root.isBusy = false;
            root.isLoadingMore = false;
            root.statusMessage = qsTr("Spotify: %1").arg(error);
        }
        function onDownloadProgressChanged(spotifyId, value) {
            const next = Object.assign({}, root.progress);
            if (value < 0)
                delete next[spotifyId];
            else
                next[spotifyId] = value;
            root.progress = next;
        }
        function onDownloadFailed(spotifyId, error) {
            root.statusMessage = qsTr("Could not load the track: %1").arg(error);
        }

        target: Mixxx.Spotify
    }

    // ================= NOT LOGGED IN =================
    ColumnLayout {
        anchors.centerIn: parent
        spacing: 10
        visible: !Mixxx.Spotify.loggedIn
        width: Math.min(parent.width - 24, 420)

        Text {
            Layout.fillWidth: true
            color: "#ffffff"
            font.bold: true
            font.pixelSize: 16
            horizontalAlignment: Text.AlignHCenter
            text: qsTr("Spotify")
        }
        Text {
            Layout.fillWidth: true
            color: "#9a9ea6"
            font.pixelSize: 12
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            text: !Mixxx.Spotify.available ? qsTr("Spotify is not part of this build.") : Mixxx.Spotify.loggingIn ? qsTr("Log in on the Spotify page, then come back here.") : qsTr("Log in once with your Spotify account.")
        }
        Skin.Button {
            Layout.alignment: Qt.AlignHCenter
            Layout.preferredHeight: 40
            Layout.preferredWidth: 240
            activeColor: root.spotifyGreen
            normalColor: root.spotifyGreen
            font.bold: true
            font.pixelSize: 12
            visible: Mixxx.Spotify.available
            text: Mixxx.Spotify.loggingIn ? qsTr("OPEN LOGIN PAGE AGAIN") : qsTr("LOG IN TO SPOTIFY")
            onClicked: Mixxx.Spotify.login()
        }
        Skin.Button {
            Layout.alignment: Qt.AlignHCenter
            Layout.preferredHeight: 28
            Layout.preferredWidth: 120
            activeColor: root.spotifyGreen
            normalColor: "#9a9ea6"
            font.pixelSize: 11
            visible: Mixxx.Spotify.loggingIn
            text: qsTr("CANCEL")
            onClicked: Mixxx.Spotify.cancelLogin()
        }
        Text {
            Layout.fillWidth: true
            color: "#ff8a80"
            font.pixelSize: 11
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            text: root.statusMessage
            visible: root.statusMessage !== ""
        }
    }

    // ================= LOGGED IN =================
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 6
        spacing: 4
        visible: Mixxx.Spotify.loggedIn

        RowLayout {
            Layout.fillWidth: true
            spacing: 6

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 30
                color: "#000000"
                border.color: searchField.activeFocus ? root.spotifyGreen : Theme.midGray3
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
                Skin.SearchClearButton {
                    field: searchField
                }
                Text {
                    anchors.fill: parent
                    anchors.margins: 6
                    color: Theme.lightGray3
                    font.pixelSize: 12
                    text: qsTr("SEARCH SPOTIFY")
                    verticalAlignment: Text.AlignVCenter
                    visible: searchField.text.length === 0
                }
            }
            Skin.Button {
                Layout.preferredHeight: 30
                Layout.preferredWidth: 80
                activeColor: root.spotifyGreen
                normalColor: "#ffffff"
                font.bold: true
                font.pixelSize: 10
                text: root.isBusy && root.listKind === "search" ? "..." : qsTr("SEARCH")
                enabled: !root.isBusy
                onClicked: root.doSearch()
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 4

            Repeater {
                model: [
                    {
                        "kind": "liked",
                        "label": qsTr("♥ LIKED SONGS")
                    },
                    {
                        "kind": "playlists",
                        "label": qsTr("PLAYLISTS")
                    }
                ]

                delegate: Skin.Button {
                    required property var modelData

                    Layout.preferredHeight: 24
                    Layout.preferredWidth: 100
                    activeColor: root.spotifyGreen
                    normalColor: root.listKind === modelData.kind || (modelData.kind === "playlists" && root.listKind === "playlist") ? root.spotifyGreen : "#b3b3b3"
                    font.bold: true
                    font.pixelSize: 9
                    text: modelData.label
                    onClicked: root.openList(modelData.kind)
                }
            }
            Text {
                Layout.fillWidth: true
                color: "#9a9ea6"
                elide: Text.ElideRight
                font.pixelSize: 11
                text: (root.listKind === "playlist" ? root.playlistName + "  ·  " : "") + root.statusMessage
            }
            Text {
                color: "#b3b3b3"
                font.pixelSize: 10
                text: Mixxx.Spotify.userName
            }
            Skin.Button {
                Layout.preferredHeight: 24
                Layout.preferredWidth: 64
                activeColor: "#ff5252"
                normalColor: "#b3b3b3"
                font.pixelSize: 9
                text: qsTr("LOG OUT")
                onClicked: {
                    resultsModel.clear();
                    root.statusMessage = "";
                    Mixxx.Spotify.logout();
                }
            }
        }

        ListView {
            id: resultList

            boundsBehavior: Flickable.DragOverBounds

            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: resultsModel
            spacing: 2
            currentIndex: -1
            highlightFollowsCurrentItem: true
            highlightMoveDuration: 0
            // Later, not inside the scroll: the footer it shows changes the
            // list height, which changed the scroll again (binding loop)
            onContentYChanged: Qt.callLater(root.maybeLoadMore)

            highlight: Rectangle {
                z: 2
                color: "transparent"
                border.color: root.spotifyGreen
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
                    text: qsTr("Loading more...")
                }
            }
            delegate: Rectangle {
                id: trackRow

                required property int index
                required property string sid
                required property string title
                required property string artist
                required property string album
                required property int durationSec
                required property string artwork
                required property int count
                readonly property bool isPlaylist: root.rowsArePlaylists
                readonly property double downloadProg: root.progress[sid] ?? -1
                readonly property bool isDownloading: downloadProg >= 0

                width: ListView.view.width
                height: 48
                radius: 4

                Library.TrackDrag {
                    enabled: !trackRow.isPlaylist
                    label: trackRow.title
                    loadTo: group => root.loadRow(trackRow.index, group)
                    fetch: () => Mixxx.Spotify.download({
                            "id": trackRow.sid,
                            "uri": resultsModel.get(trackRow.index).uri,
                            "title": trackRow.title,
                            "artist": trackRow.artist,
                            "album": trackRow.album
                        })
                    fetchId: trackRow.sid
                    fetchSource: Mixxx.Spotify
                }
                color: index % 2 === 0 ? "#0c0d0f" : "#16181b"
                border.color: isDownloading ? root.spotifyGreen : "#2a2c30"
                border.width: 1

                MouseArea {
                    anchors.fill: parent
                    onClicked: {
                        resultList.currentIndex = trackRow.index;
                        if (trackRow.isPlaylist)
                            root.openPlaylist(trackRow.sid, trackRow.title);
                    }
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

                        Text {
                            Layout.fillWidth: true
                            color: "#ffffff"
                            elide: Text.ElideRight
                            font.pixelSize: 12
                            font.bold: true
                            text: trackRow.title
                        }
                        Text {
                            Layout.fillWidth: true
                            color: "#9a9ea6"
                            elide: Text.ElideRight
                            font.pixelSize: 10
                            text: trackRow.isPlaylist ? qsTr("%1 tracks").arg(trackRow.count) : trackRow.isDownloading ? qsTr("Downloading %1%").arg(Math.round(trackRow.downloadProg * 100)) : trackRow.artist + "  ·  " + root.formatDuration(trackRow.durationSec) + (trackRow.album !== "" ? "  ·  " + trackRow.album : "")
                        }
                    }

                    Repeater {
                        model: trackRow.isPlaylist ? [] : [
                            {
                                "deck": "[Channel1]",
                                "label": "D1",
                                "color": Qt.lighter(Theme.accentColor, 1.2)
                            },
                            {
                                "deck": "[Channel2]",
                                "label": "D2",
                                "color": "#e8650f"
                            }
                        ]

                        delegate: Skin.Button {
                            required property var modelData

                            Layout.preferredHeight: 30
                            Layout.preferredWidth: 46
                            activeColor: modelData.color
                            normalColor: "#ffffff"
                            font.bold: true
                            font.pixelSize: 10
                            text: trackRow.isDownloading ? `${Math.round(trackRow.downloadProg * 100)}%` : modelData.label
                            enabled: !trackRow.isDownloading
                            onClicked: {
                                resultList.currentIndex = trackRow.index;
                                root.loadRow(trackRow.index, modelData.deck);
                            }
                        }
                    }
                }
            }


            Skin.PullToRefresh {
                flickable: resultList
                busy: root.isBusy
                onRefresh: root.refreshList()
            }

        }
    }
}

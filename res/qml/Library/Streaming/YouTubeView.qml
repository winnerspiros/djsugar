import Mixxx 1.0 as Mixxx
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../.." as Skin
import "../../Theme"
import ".." as Library

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

    // The search shown; refresh repeats it, not what the field holds now
    property string searchedQuery: ""
    function doSearch(query = searchField.text) {
        if (query.length === 0) {
            return;
        }
        root.searchedQuery = query;
        root.listKind = "search";
        root.isBusy = true;
        root.isLoadingMore = false;
        root.hasMore = false;
        root.statusMessage = qsTr("Searching YouTube...");
        Mixxx.YouTube.search(query);
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

    property bool wasSignedIn: false
    // The account's playlists [{id, name}] and channels [{name, pageId, selected}]
    property var playlists: []
    property var channels: []
    // "search", "liked", "playlists" (the account's playlists listed),
    // "playlist" (one open)
    property string listKind: "search"
    readonly property color ytRed: "#ff0033"
    property string playlistName: ""
    property string listId: ""
    function openList(listId, name) {
        root.listId = listId;
        root.listKind = listId === "liked" ? "liked" : "playlist";
        root.playlistName = name;
        root.isBusy = true;
        root.hasMore = false;
        root.statusMessage = qsTr("Loading %1...").arg(name);
        if (listId === "liked")
            Mixxx.YouTube.youTubeLiked();
        else
            Mixxx.YouTube.youTubePlaylist(listId);
    }
    function openAccount() {
        Mixxx.YouTube.youTubeAccounts();
        root.openList("liked", qsTr("Liked songs"));
        Mixxx.YouTube.youTubePlaylists();
    }
    // Pull to refresh / F5: what is shown is fetched again
    function refreshList() {
        if (root.listKind === "search")
            root.doSearch(root.searchedQuery);
        else if (root.listKind === "playlists")
            root.showPlaylists();
        else
            root.openList(root.listId, root.playlistName);
    }
    function showPlaylists() {
        root.listKind = "playlists";
        root.statusMessage = root.playlists.length > 0
            ? qsTr("%1 playlists").arg(root.playlists.length)
            : qsTr("Loading playlists...");
        Mixxx.YouTube.youTubePlaylists();
    }
    Component.onCompleted: {
        root.wasSignedIn = Mixxx.YouTube.youTubeSignedIn;
        if (root.wasSignedIn)
            root.openAccount();
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
        function onAccountsChanged() {
            // (a plain value: a binding would already be true here)
            const signedIn = Mixxx.YouTube.youTubeSignedIn;
            if (signedIn && !root.wasSignedIn)
                root.openAccount();
            if (!signedIn) {
                root.playlists = [];
                root.channels = [];
                if (root.listKind !== "search") {
                    root.listKind = "search";
                    root.playlistName = "";
                    root.allResults = [];
                    root.hasMore = false;
                    resultsModel.clear();
                }
            }
            root.wasSignedIn = signedIn;
        }
        function onYouTubePlaylistsLoaded(playlists) {
            root.playlists = playlists;
            if (root.listKind === "playlists")
                root.statusMessage = playlists.length > 0
                    ? qsTr("%1 playlists").arg(playlists.length)
                    : qsTr("No playlists");
        }
        function onYouTubeAccountsLoaded(accounts) {
            // One highlight: the first selected only
            let found = false;
            root.channels = accounts.map(c => {
                const on = c.selected && !found;
                found = found || on;
                return Object.assign({}, c, {
                    "selected": on
                });
            });
        }
        function onAccountFailed(error) {
            root.isBusy = false;
            root.isLoadingMore = false;
            root.statusMessage = qsTr("YouTube account: %1").arg(error);
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

            // The YouTube mark
            Rectangle {
                Layout.preferredHeight: 30
                Layout.preferredWidth: 40
                radius: 8
                color: root.ytRed

                Skin.Triangle {
                    anchors.centerIn: parent
                    anchors.horizontalCenterOffset: 1
                    width: 11
                    height: 13
                    color: "#ffffff"
                    pointsRight: true
                }
            }
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 30
                color: "#000000"
                border.color: searchField.activeFocus ? root.ytRed : Theme.midGray3
                border.width: 1
                radius: 15

                TextInput {
                    id: searchField

                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    clip: true
                    color: "#ffffff"
                    font.pixelSize: 12
                    verticalAlignment: TextInput.AlignVCenter
                    onAccepted: root.doSearch()
                    // connection to YouTube opened while typing
                    onActiveFocusChanged: {
                        if (activeFocus)
                            Mixxx.YouTube.warmUp();
                    }
                }
                Skin.SearchClearButton {
                    field: searchField
                }
                Text {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    color: Theme.lightGray3
                    font.pixelSize: 12
                    elide: Text.ElideRight
                    text: qsTr("Search YouTube: songs, sets, remixes")
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
            // The account: sign in (Google's page, saved passwords offered);
            // signed in, its channels and sign out (liked songs and
            // playlists: the row below)
            AbstractButton {
                id: ytAccountButton

                Layout.preferredHeight: 30
                Layout.preferredWidth: Math.min(170, ytAccountText.implicitWidth + 40)
                background: Rectangle {
                    radius: 15
                    color: Mixxx.YouTube.youTubeSignedIn
                        ? (ytAccountButton.pressed ? "#3a0d0d" : "#240a0a")
                        : (ytAccountButton.pressed ? "#cc0000" : root.ytRed)
                    border.color: root.ytRed
                    border.width: 1
                }
                contentItem: Item {
                    Row {
                        anchors.centerIn: parent
                        spacing: 6

                        // The account's initial
                        Rectangle {
                            visible: Mixxx.YouTube.youTubeSignedIn
                            anchors.verticalCenter: parent.verticalCenter
                            width: 22
                            height: 22
                            radius: 11
                            color: root.ytRed

                            Text {
                                anchors.centerIn: parent
                                color: "#ffffff"
                                font.bold: true
                                font.pixelSize: 11
                                text: (Mixxx.YouTube.youTubeAccount || "Y").charAt(0).toUpperCase()
                            }
                        }
                        Text {
                            id: ytAccountText

                            anchors.verticalCenter: parent.verticalCenter
                            width: Math.min(implicitWidth, 120)
                            color: "#ffffff"
                            elide: Text.ElideRight
                            font.bold: true
                            font.pixelSize: 10
                            text: Mixxx.YouTube.youTubeSignedIn
                                ? (Mixxx.YouTube.youTubeAccount.length > 0 ? Mixxx.YouTube.youTubeAccount : qsTr("ACCOUNT"))
                                : qsTr("SIGN IN")
                        }
                    }
                }
                onClicked: {
                    if (Mixxx.YouTube.youTubeSignedIn) {
                        Mixxx.YouTube.youTubeAccounts();
                        accountMenu.open();
                    } else {
                        Mixxx.YouTube.signIn("youtube");
                    }
                }

                // The account's channels and sign out, in the view's style
                Popup {
                    id: accountMenu

                    y: parent.height + 2
                    x: parent.width - width
                    width: 240
                    padding: 8
                    modal: false
                    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

                    background: Rectangle {
                        color: "#121419"
                        border.color: "#2c3038"
                        border.width: 1
                        radius: 12
                    }
                    contentItem: Column {
                        spacing: 6

                        Text {
                            width: parent.width
                            leftPadding: 6
                            visible: root.channels.length > 0
                            color: "#9a9ea6"
                            font.bold: true
                            font.pixelSize: 9
                            text: qsTr("SWITCH ACCOUNT")
                        }
                        Repeater {
                            model: root.channels

                            delegate: Rectangle {
                                id: channelRow

                                required property int index
                                required property var modelData

                                width: parent.width
                                height: 40
                                radius: 8
                                color: modelData.selected ? "#3a1016" : (channelTap.pressed ? "#23262e" : "#1b1e24")
                                border.color: modelData.selected ? root.ytRed : "#2c3038"
                                border.width: 1

                                Rectangle {
                                    id: channelInitial

                                    anchors.left: parent.left
                                    anchors.leftMargin: 8
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: 26
                                    height: 26
                                    radius: 13
                                    color: channelRow.modelData.selected ? root.ytRed : "#3a3e48"

                                    Text {
                                        anchors.centerIn: parent
                                        color: "#ffffff"
                                        font.bold: true
                                        font.pixelSize: 12
                                        text: (channelRow.modelData.name || "?").charAt(0).toUpperCase()
                                    }
                                }
                                Column {
                                    anchors.left: channelInitial.right
                                    anchors.leftMargin: 8
                                    anchors.right: parent.right
                                    anchors.rightMargin: 8
                                    anchors.verticalCenter: parent.verticalCenter

                                    Text {
                                        width: parent.width
                                        color: "#ffffff"
                                        elide: Text.ElideRight
                                        font.bold: channelRow.modelData.selected
                                        font.pixelSize: 12
                                        text: channelRow.modelData.name
                                    }
                                    Text {
                                        width: parent.width
                                        visible: text.length > 0
                                        color: "#9a9ea6"
                                        elide: Text.ElideRight
                                        font.pixelSize: 9
                                        text: channelRow.modelData.handle ?? ""
                                    }
                                }
                                MouseArea {
                                    id: channelTap

                                    anchors.fill: parent
                                    onClicked: {
                                        // Reassigning the channels destroys this
                                        // row: everything else first
                                        const view = root;
                                        const row = channelRow.index;
                                        accountMenu.close();
                                        Mixxx.YouTube.selectYouTubeAccount(channelRow.modelData.pageId, channelRow.modelData.name);
                                        view.playlists = [];
                                        view.openList("liked", qsTr("Liked songs"));
                                        view.channels = view.channels.map((c, i) => Object.assign({}, c, {
                                            "selected": i === row
                                        }));
                                    }
                                }
                            }
                        }
                        Skin.Button {
                            width: parent.width
                            height: 30
                            activeColor: "#ff5252"
                            normalColor: "#ff5252"
                            font.bold: true
                            font.pixelSize: 10
                            text: qsTr("SIGN OUT")
                            onClicked: {
                                accountMenu.close();
                                Mixxx.YouTube.signOut("youtube");
                                root.statusMessage = qsTr("Signed out of YouTube");
                            }
                        }
                    }
                }
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
        // Signed in: the account's liked songs and playlists, as on Spotify
        RowLayout {
            Layout.fillWidth: true
            spacing: 4

            Repeater {
                model: Mixxx.YouTube.youTubeSignedIn ? [
                    {
                        "kind": "liked",
                        "label": qsTr("♥ LIKED SONGS")
                    },
                    {
                        "kind": "playlists",
                        "label": qsTr("PLAYLISTS")
                    }
                ] : []

                delegate: Skin.Button {
                    required property var modelData

                    Layout.preferredHeight: 24
                    Layout.preferredWidth: 100
                    activeColor: root.ytRed
                    normalColor: root.listKind === modelData.kind || (modelData.kind === "playlists" && root.listKind === "playlist") ? root.ytRed : "#b3b3b3"
                    font.bold: true
                    font.pixelSize: 9
                    text: modelData.label
                    onClicked: {
                        if (modelData.kind === "liked")
                            root.openList("liked", qsTr("Liked songs"));
                        else
                            root.showPlaylists();
                    }
                }
            }
            Text {
                Layout.fillWidth: true
                color: "#9a9ea6"
                font.pixelSize: 10
                text: (root.listKind === "playlist" ? root.playlistName + "  ·  " : "") + root.statusMessage
                elide: Text.ElideRight
            }
        }

        // The account's playlists; a tap opens one
        ListView {
            id: playlistList
            boundsBehavior: Flickable.DragOverBounds

            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: root.listKind === "playlists"
            clip: true
            model: root.playlists
            spacing: 2
            delegate: Rectangle {
                id: playlistRow

                required property var modelData

                width: ListView.view.width
                height: 48
                radius: 4
                color: playlistTap.pressed ? "#2a2f38" : "#1e222a"

                Image {
                    id: playlistArt

                    anchors.left: parent.left
                    anchors.leftMargin: 4
                    anchors.verticalCenter: parent.verticalCenter
                    width: 40
                    height: 40
                    asynchronous: true
                    fillMode: Image.PreserveAspectCrop
                    source: playlistRow.modelData.artwork ?? ""
                }
                Column {
                    anchors.left: playlistArt.right
                    anchors.leftMargin: 8
                    anchors.right: parent.right
                    anchors.rightMargin: 8
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 2

                    Text {
                        width: parent.width
                        color: "#ffffff"
                        elide: Text.ElideRight
                        font.bold: true
                        font.pixelSize: 12
                        text: playlistRow.modelData.name
                    }
                    Text {
                        width: parent.width
                        color: "#9a9ea6"
                        elide: Text.ElideRight
                        font.pixelSize: 10
                        text: playlistRow.modelData.subtitle ?? ""
                    }
                }
                MouseArea {
                    id: playlistTap

                    anchors.fill: parent
                    onClicked: root.openList(playlistRow.modelData.id, playlistRow.modelData.name)
                }
            }

            Skin.PullToRefresh {
                flickable: playlistList
                busy: false
                onRefresh: root.refreshList()
            }
        }

        ListView {
            id: resultList

            boundsBehavior: Flickable.DragOverBounds

            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: root.listKind !== "playlists"
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

                Library.TrackDrag {
                    label: videoCard.title
                    loadTo: group => Mixxx.YouTube.downloadToDeck(videoCard.vid, group)
                    fetch: () => Mixxx.YouTube.download(videoCard.vid)
                    fetchId: videoCard.vid
                    fetchSource: Mixxx.YouTube
                }

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

            // Nothing listed: what this view is for, and the way in
            Column {
                parent: resultList
                anchors.fill: parent
                visible: root.listKind !== "playlists" && resultsModel.count === 0 && !root.isBusy
                spacing: 10
                topPadding: 40

                Rectangle {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: 56
                    height: 40
                    radius: 10
                    color: root.ytRed

                    Skin.Triangle {
                        anchors.centerIn: parent
                        anchors.horizontalCenterOffset: 2
                        width: 16
                        height: 18
                        color: "#ffffff"
                        pointsRight: true
                    }
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: Math.min(parent.width - 20, 380)
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.WordWrap
                    color: "#c8ccd4"
                    font.pixelSize: 12
                    text: root.listKind === "liked" ? qsTr("No liked songs here yet: like songs on YouTube Music or YouTube and they show up")
                        : root.listKind === "playlist" ? qsTr("This playlist is empty")
                        : Mixxx.YouTube.youTubeSignedIn ? qsTr("Search above, or open your liked songs and playlists")
                        : qsTr("Search above. Sign in for your liked songs and playlists")
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

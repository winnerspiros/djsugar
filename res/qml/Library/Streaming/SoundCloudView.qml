import Mixxx 1.0 as Mixxx
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "../.." as Skin
import "../../Theme"
import ".." as Library

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
    // "search" or "likes"
    property string listKind: "search"
    readonly property color scOrange: "#ff5500"

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
        Mixxx.YouTube.soundCloudSearch(query);
    }
    function openLikes() {
        root.listKind = "likes";
        root.isBusy = true;
        root.statusMessage = qsTr("Loading your likes...");
        Mixxx.YouTube.soundCloudLikes();
    }
    Component.onCompleted: {
        if (Mixxx.YouTube.soundCloudSignedIn)
            root.openLikes();
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
            root.statusMessage = root.listKind === "likes"
                ? (resultsModel.count > 0 ? qsTr("%1 liked tracks").arg(resultsModel.count) : qsTr("No likes yet"))
                : (resultsModel.count > 0 ? qsTr("%1 results").arg(resultsModel.count) : qsTr("No results"));
        }
        function onAccountsChanged() {
            if (Mixxx.YouTube.soundCloudSignedIn && resultsModel.count === 0 && !root.isBusy)
                root.openLikes();
        }
        function onAccountFailed(error) {
            root.isBusy = false;
            root.statusMessage = error;
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

        // Header: the SoundCloud mark, search, the account
        RowLayout {
            Layout.fillWidth: true
            spacing: 6

            Rectangle {
                Layout.preferredHeight: 30
                Layout.preferredWidth: 30
                radius: 6
                color: root.scOrange

                Text {
                    anchors.centerIn: parent
                    color: "#ffffff"
                    font.bold: true
                    font.pixelSize: 15
                    text: "☁"
                }
            }
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 30
                color: "#000000"
                border.color: searchField.activeFocus ? root.scOrange : Theme.midGray3
                border.width: 1
                radius: 15

                TextInput {
                    id: searchField

                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    color: "#ffffff"
                    font.pixelSize: 12
                    verticalAlignment: TextInput.AlignVCenter
                    clip: true
                    onAccepted: root.doSearch()
                }
                Skin.SearchClearButton {
                    field: searchField
                }
                Text {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    color: Theme.lightGray3
                    font.pixelSize: 12
                    text: qsTr("Search SoundCloud: remixes, edits, DJ mixes")
                    verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideRight
                    visible: searchField.text.length === 0
                }
            }
            Skin.Button {
                Layout.preferredHeight: 30
                Layout.preferredWidth: 80
                activeColor: root.scOrange
                normalColor: "#ffffff"
                font.bold: true
                font.pixelSize: 10
                text: root.isBusy && root.listKind === "search" ? "..." : qsTr("SEARCH")
                enabled: !root.isBusy
                onClicked: root.doSearch()
            }
            // The account: SIGN IN, or the user's avatar and name (tap:
            // the account popup)
            AbstractButton {
                id: accountButton

                Layout.preferredHeight: 30
                Layout.preferredWidth: Math.min(170, accountRow.implicitWidth + 16)
                onClicked: {
                    if (Mixxx.YouTube.soundCloudSignedIn)
                        scAccountMenu.open();
                    else
                        Mixxx.YouTube.signIn("soundcloud");
                }

                background: Rectangle {
                    radius: 15
                    color: Mixxx.YouTube.soundCloudSignedIn
                        ? (accountButton.pressed ? "#3a1a08" : "#24140b")
                        : (accountButton.pressed ? "#cc4400" : root.scOrange)
                    border.color: root.scOrange
                    border.width: 1
                }
                contentItem: Item {
                    Row {
                        id: accountRow

                        anchors.centerIn: parent
                        spacing: 6

                        Rectangle {
                            visible: Mixxx.YouTube.soundCloudSignedIn
                            anchors.verticalCenter: parent.verticalCenter
                            width: 22
                            height: 22
                            radius: 11
                            clip: true
                            color: "#3a3d44"

                            Image {
                                anchors.fill: parent
                                asynchronous: true
                                fillMode: Image.PreserveAspectCrop
                                source: Mixxx.YouTube.soundCloudAvatar
                                sourceSize.width: 44
                                sourceSize.height: 44
                            }
                        }
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            width: Math.min(implicitWidth, 120)
                            color: "#ffffff"
                            elide: Text.ElideRight
                            font.bold: true
                            font.pixelSize: 10
                            text: !Mixxx.YouTube.soundCloudSignedIn
                                ? qsTr("SIGN IN")
                                : (Mixxx.YouTube.soundCloudAccount.length > 0 ? Mixxx.YouTube.soundCloudAccount : qsTr("ACCOUNT"))
                        }
                    }
                }

                Popup {
                    id: scAccountMenu

                    y: parent.height + 2
                    x: parent.width - width
                    width: 220
                    padding: 8
                    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

                    background: Rectangle {
                        color: "#14161b"
                        border.color: root.scOrange
                        border.width: 1
                        radius: 6
                    }
                    contentItem: Column {
                        spacing: 6

                        Text {
                            width: parent.width
                            color: "#9a9ea6"
                            font.pixelSize: 9
                            font.bold: true
                            text: qsTr("SIGNED IN AS")
                        }
                        Text {
                            width: parent.width
                            color: "#ffffff"
                            elide: Text.ElideRight
                            font.bold: true
                            font.pixelSize: 13
                            text: Mixxx.YouTube.soundCloudAccount
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
                                scAccountMenu.close();
                                Mixxx.YouTube.signOut("soundcloud");
                                resultsModel.clear();
                                root.listKind = "search";
                                root.statusMessage = qsTr("Signed out of SoundCloud");
                            }
                        }
                    }
                }
            }
        }

        // Likes / search results, as on Spotify and YouTube
        RowLayout {
            Layout.fillWidth: true
            spacing: 4

            Skin.Button {
                visible: Mixxx.YouTube.soundCloudSignedIn
                Layout.preferredHeight: 24
                Layout.preferredWidth: 90
                activeColor: root.scOrange
                normalColor: root.listKind === "likes" ? root.scOrange : "#b3b3b3"
                font.bold: true
                font.pixelSize: 9
                text: qsTr("♥ LIKES")
                enabled: !root.isBusy
                onClicked: root.openLikes()
            }
            Text {
                Layout.fillWidth: true
                color: "#9a9ea6"
                elide: Text.ElideRight
                font.pixelSize: 11
                text: root.statusMessage
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

                Library.TrackDrag {
                    label: trackRow.title
                    loadTo: group => Mixxx.YouTube.downloadToDeck(trackRow.vid, group)
                    fetch: () => Mixxx.YouTube.download(trackRow.vid)
                    fetchId: trackRow.vid
                    fetchSource: Mixxx.YouTube
                }
                opacity: preview ? 0.55 : 1.0

                MouseArea {
                    anchors.fill: parent
                    onClicked: resultList.currentIndex = trackRow.index
                }
                // SoundCloud's orange on the left edge
                Rectangle {
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    width: 3
                    height: parent.height - 12
                    radius: 1.5
                    color: trackRow.preview ? "#5a5d63" : root.scOrange
                }

                RowLayout {
                    anchors.fill: parent
                    anchors.margins: 4
                    spacing: 6

                    Rectangle {
                        Layout.leftMargin: 4
                        Layout.preferredWidth: 40
                        Layout.preferredHeight: 40
                        color: "#1e222a"
                        radius: 4
                        clip: true

                        Image {
                            anchors.fill: parent
                            asynchronous: true
                            cache: true
                            fillMode: Image.PreserveAspectCrop
                            // "-large" is 100 px: the 200 px one is sharp
                            source: trackRow.artwork.replace("-large.", "-t200x200.")
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

                    // D1/D2 like the YouTube list: the download lands on that
                    // deck and the button shows its progress meanwhile
                    Repeater {
                        model: [
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
                            enabled: !trackRow.isDownloading && !trackRow.preview
                            onClicked: {
                                resultList.currentIndex = trackRow.index;
                                Mixxx.YouTube.downloadToDeck(trackRow.vid, modelData.deck);
                            }
                        }
                    }

                    // Download only: keeps the track in the library (the
                    // same drawn arrow as YouTube; the font has no ⤓ glyph)
                    AbstractButton {
                        id: saveButton

                        Layout.preferredHeight: 30
                        Layout.preferredWidth: 46
                        enabled: !trackRow.isDownloading && !trackRow.preview
                        opacity: enabled ? 1.0 : 0.5
                        onClicked: {
                            root.statusMessage = qsTr("Downloading to your library...");
                            Mixxx.YouTube.download(trackRow.vid);
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
            }

            // Nothing listed yet: what this view is for, and the way in
            Column {
                parent: resultList
                anchors.fill: parent
                visible: resultsModel.count === 0 && !root.isBusy
                spacing: 10
                topPadding: 40

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    color: root.scOrange
                    font.pixelSize: 40
                    text: "☁"
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: Math.min(parent.width - 20, 360)
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.WordWrap
                    color: "#c8ccd4"
                    font.pixelSize: 12
                    text: Mixxx.YouTube.soundCloudSignedIn
                        ? qsTr("Search above, or open your likes")
                        : qsTr("Search above. Sign in for your likes, and Go+ tracks in full")
                }
            }

            Skin.PullToRefresh {
                flickable: resultList
                busy: root.isBusy
                onRefresh: root.listKind === "likes" ? root.openLikes() : root.doSearch(root.searchedQuery)
            }

        }
    }
}

import "../.." as Skin
import "../../Library" as LibraryComponent
import "../../Library/Streaming" as StreamingComponent
import "." as Booth
import Mixxx 1.0 as Mixxx
import Qt5Compat.GraphicalEffects
import QtQml.Models
import QtQuick
import QtQuick.Controls

// BROWSE of the booth skin, laid out like the browser of a player: a column
// of sources on the left, the tracks in the middle and the selected track
// on the right. A source with entries (playlists, folders, drives) first
// lists them in place of the tracks, one level at a time, with ← to go
// back. The data, the track menu, + ALL, COPY and the controller browsing
// are the ones of the shared library (Library/), only the columns and the
// look are the skin's (BoothSourceTree.qml).
//
// LOAD 1/2 open it with a target deck (loadTarget): a double tap loads the
// track into that deck.
Item {
    id: root

    // The deck LOAD 1/2 opened the browser for, or ""
    property string loadTarget: ""
    property bool acceptsController: true
    property bool compact: false
    property bool streaming: false
    readonly property var sidebar: sources.sidebar()

    // Filled by the cells of the selected row
    property url cover: ""
    property string title: ""
    property string artist: ""
    property string bpm: ""
    property string key: ""
    property string added: ""

    // The entry whose entries are listed, as the rows from the top level
    // down, or null. Model indexes of the sources are not kept: opening a
    // source can rebuild its entries (e.g. a Rekordbox drive) and an old
    // index would point to deleted entries. levelIndex is looked up again
    // from levelPath after every change.
    property var levelPath: null
    property var levelIndex: null
    // The tracks are shown rather than the entries of levelPath
    property bool showTracks: true
    // The row the controller's browse knob is on: in the list of entries
    // (-1 is ALL TRACKS) or, without one, in the column of sources
    property int cursor: 0
    // The top level source the tracks shown belong to
    property int activeSection: 0
    // The name of the playlist, crate or folder whose tracks are shown
    property string activeLabel: ""
    // A whole USB drive (Rekordbox or Engine DJ) rather than a playlist
    property bool activeIsDrive: false
    readonly property bool showLevel: root.levelIndex !== null && !root.showTracks
    // levelIndex, or the invisible root of the sources
    readonly property var listRoot: root.levelIndex ?? root.sidebar?.index(-1, -1)
    // Entries below a top level source can show their own tracks too
    readonly property bool levelHasAll: root.levelPath !== null && root.levelPath.length > 1
    // The entries listed are drives, which list their playlists once read
    readonly property bool levelIsDrives: {
        if (root.levelPath === null || root.levelPath.length !== 1 || root.levelIndex === null) {
            return false;
        }
        const icon = root.iconOf(root.levelIndex);
        return icon === "rekordbox" || icon === "enginedj";
    }

    // SidebarModel::IconNameRole
    function iconOf(index) {
        return root.sidebar?.data(index, Qt.UserRole + 1) ?? "";
    }
    function sectionLabel(icon, label) {
        switch (icon) {
        case "tracks":
            return qsTr("TRACK");
        case "playlist":
            return qsTr("PLAYLIST");
        case "crates":
            return qsTr("CRATE");
        case "history":
            return qsTr("HISTORY");
        case "computer":
            return qsTr("FOLDER");
        case "rekordbox":
            return "REKORDBOX";
        case "enginedj":
            return "ENGINE DJ";
        case "recordings":
            return qsTr("REC");
        }
        return (label ?? "").toString().toUpperCase();
    }
    function pathOf(index) {
        const path = [];
        for (let i = index; i.valid; i = i.parent) {
            path.unshift(i.row);
        }
        return path;
    }
    // A fresh index for a path, or null if the entry is gone
    function indexOf(path) {
        // The sidebar model object exists from C++ construction, but it is
        // empty until componentComplete wires the sources: index(-1,-1) on
        // an empty model is a crash. A tap that lands in that window (fast
        // BROWSE right after the browser first shows) must be dropped.
        if (!root.sidebar || root.sidebar.rowCount(root.sidebar.index(-1, -1)) === 0) {
            return null;
        }
        let index = root.sidebar.index(-1, -1);
        for (let i = 0; i < path.length; ++i) {
            if (path[i] < 0 || path[i] >= root.sidebar.rowCount(index)) {
                return null;
            }
            index = root.sidebar.index(path[i], 0, index);
        }
        return index.valid ? index : null;
    }
    function sectionIcon(path) {
        const section = root.indexOf(path.slice(0, 1));
        return section ? root.iconOf(section) : "";
    }
    function pathIsDrive(path) {
        const icon = root.sectionIcon(path);
        return path.length === 2 && (icon === "rekordbox" || icon === "enginedj");
    }
    function refreshLevel() {
        if (root.levelPath === null || root.sidebar === null) {
            root.levelIndex = null;
            return;
        }
        const index = root.indexOf(root.levelPath);
        if (index === null) {
            // The entry went away, e.g. an unplugged drive
            root.levelPath = null;
            root.levelIndex = null;
            root.showTracks = true;
            return;
        }
        root.levelIndex = index;
    }
    // Reading the databases on USB drives needs more than the music
    // permission, as in Library/Browser.qml
    // True when the drive at path may be read; false opens the grant
    // popup and STOPS the open: without all-files access the USB scan
    // blocks the UI thread up to 29s waiting for a permission that can
    // never arrive (see androidusbdrive waitForPermission). Music-library
    // browsing only needs READ_MEDIA_AUDIO (granted at startup) and is
    // unaffected: this gate only fires for USB-drive sections.
    function checkStorageAccess(path) {
        const icon = root.sectionIcon(path);
        if (icon !== "rekordbox" && icon !== "enginedj") {
            return true;
        }
        if (!Mixxx.MobileLibrary.hasFullStorageAccess()) {
            storageAccessPopup.open();
            return false;
        }
        Mixxx.MobileLibrary.allowUsbPermissionRequests();
        return true;
    }
    // Shows the tracks of the entry at path
    function activate(path) {
        const index = root.indexOf(path);
        if (index === null) {
            return;
        }
        const label = root.sidebar.data(index, Qt.DisplayRole);
        root.activeLabel = label ? label.toString() : "";
        root.activeIsDrive = root.pathIsDrive(path);
        root.activeSection = path[0];
        // May rebuild the entries below it: no index is used after this
        root.sidebar.activate(index);
    }
    // Opens a source or an entry. Returns true if its tracks are shown,
    // false if its entries are listed.
    // True while open()/activate() is on the stack: the TapHandler and
    // the MouseArea of one tap can both fire (driver-dependent), and an
    // open() re-entered mid-model-rebuild reads half-built indexes.
    property bool opening: false
    function open(path) {
        // USB drives without all-files access would hang the UI thread in
        // the scan below; the popup is shown and the open is abandoned.
        // The user taps the section again after granting.
        if (root.opening || !root.checkStorageAccess(path)) {
            return false;
        }
        root.opening = true;
        // Order matters, and levelIndex must be cleared FIRST: activate()
        // replaces the model's tracklist (tracklistChanged), which rebuilds
        // the track TableView; canFetchMore/fetchMore below then signal the
        // model again, and our onDataChanged/onRowsInserted handlers call
        // refreshLevel(). If levelIndex still held a pre-activate index
        // during that, a handler reads a stale/deleted index.
        // Guarded by deferredRefresh (below), never run inline.
        root.levelIndex = null;
        root.activate(path);
        const index = root.indexOf(path);
        if (index === null) {
            root.levelPath = null;
            root.showTracks = true;
            root.deferredRefresh();
            root.opening = false;
            return true;
        }
        if (root.sidebar.canFetchMore(index)) {
            root.sidebar.fetchMore(index);
        }
        // Drives list their playlists once they have been read
        if (root.sidebar.hasChildren(index) || root.pathIsDrive(path)) {
            root.levelPath = path;
            root.deferredRefresh();
            root.showTracks = false;
            root.cursor = root.levelHasAll ? -1 : 0;
            root.opening = false;
            return false;
        }
        root.showTracks = true;
        root.opening = false;
        return true;
    }
    function openEntry(row) {
        return root.open(root.levelPath.concat([row]));
    }
    function openAllTracks() {
        root.activate(root.levelPath);
        root.showTracks = true;
        root.deferredRefresh();
    }
    function back() {
        if (root.showTracks) {
            if (root.levelPath !== null) {
                // Stale index check only; refresh is routed through the
                // coalesced deferred path so this stays re-entrancy free.
                root.levelIndex = null;
                root.deferredRefresh();
                root.showTracks = true;
            }
            return;
        }
        if (root.levelHasAll) {
            const row = root.levelPath[root.levelPath.length - 1];
            root.levelPath = root.levelPath.slice(0, -1);
            root.deferredRefresh();
            root.cursor = row;
        }
    }
    // The controller's browse knob
    function moveCursor(offset) {
        if (root.showLevel) {
            root.cursor = Math.max(root.levelHasAll ? -1 : 0, Math.min(levelEntries.count - 1, root.cursor + offset));
            if (root.cursor >= 0) {
                levelList.positionViewAtIndex(root.cursor, ListView.Contain);
            }
        } else {
            root.showTracks = true;
            root.levelPath = null;
            root.levelIndex = null;
            root.deferredRefresh();
            root.cursor = Math.max(0, Math.min(sections.count - 1, root.cursor + offset));
        }
    }
    function activateCursor() {
        if (root.showLevel) {
            if (root.cursor < 0) {
                root.openAllTracks();
                return true;
            }
            return root.openEntry(root.cursor);
        }
        return root.openSection(root.cursor);
    }
    function openSection(row) {
        root.streaming = false;
        root.cursor = row;
        const tracksShown = root.open([row]);
        if (tracksShown) {
            root.levelPath = null;
            root.deferredRefresh();
        }
        return tracksShown;
    }
    function loadSelected(tableView) {
        if (root.loadTarget.length > 0) {
            tableView.loadSelectedTrack(root.loadTarget, false);
        } else {
            tableView.loadSelectedTrackIntoNextAvailableDeck(false);
        }
        // Return to the play view after a load from LOAD 1/2
        if (root.loadTarget.length > 0) {
            root.loadTarget = "";
        }
    }
    function applySearch() {
        if (root.sidebar && root.sidebar.tracklist && !root.streaming) {
            root.sidebar.tracklist.search(searchField.text);
        }
    }

    BoothSourceTree {
        id: sources

        infoTarget: root
    }

    // The controller's browse knob, as in the shared library
    LibraryComponent.Control {
        id: libraryControl

        enabled: root.visible && root.acceptsController

        onActivateSidebarItem: {
            if (root.activateCursor()) {
                focusWidget = Skin.FocusedWidgetControl.WidgetKind.LibraryView;
            }
        }
        onCollapseSidebarItem: root.back()
        onFocusWidgetChanged: {
            if (!enabled || !root.visible || root.streaming) {
                return;
            }
            switch (focusWidget) {
            case Skin.FocusedWidgetControl.WidgetKind.LibraryView:
                trackList.focusList();
                break;
            case Skin.FocusedWidgetControl.WidgetKind.Sidebar:
                root.streaming = false;
                if (root.levelPath !== null) {
                    root.levelIndex = null;
                    root.deferredRefresh();
                    root.showTracks = true;
                } else {
                    root.cursor = root.activeSection;
                }
                break;
            }
        }
        onLoadSelectedTrack: (group, play) => {
            trackList.loadSelectedTrack(group, play);
        }
        onLoadSelectedTrackIntoNextAvailableDeck: play => {
            if (root.loadTarget.length > 0) {
                trackList.loadSelectedTrack(root.loadTarget, play);
            } else {
                trackList.loadSelectedTrackIntoNextAvailableDeck(play);
            }
        }
        onMoveSidebar: offset => {
            root.moveCursor(offset);
        }
        onMoveVertical: offset => {
            trackList.moveSelectionVertical(offset);
        }
    }
    Connections {
        function onTracklistChanged() {
            root.cover = "";
            root.title = "";
            root.artist = "";
            root.bpm = "";
            root.key = "";
            root.added = "";
            if (searchField.text.length > 0) {
                root.applySearch();
            }
        }

        target: root.sidebar
    }

    // Opened lazily: activate([0]) rebuilds the model one entry at a time
    // (each source's activate() emits signals), and doing that inside
    // component creation - before the Loader's item tree is wired - hung
    // the app on BROWSE. Runs once the browser is actually shown.
    property bool initDone: false
    onVisibleChanged: {
        if (root.visible && !root.initDone) {
            root.initDone = true;
            // Deferred past first paint, and through open() so a USB
            // drive at [0] without all-files access shows the grant popup
            // instead of hanging the UI thread in the scan.
            Qt.callLater(function() { root.open([0]); });
        }
    }

    // While the sources change no index of them may be used: the list is
    // emptied first and looked up again afterwards. All refreshes are
    // deferred to the event loop AND coalesced: open()/activate() emit
    // model signals synchronously (each feature activate, fetchMore,
    // Rekordbox child inserts), inline refreshLevel() inside a signal
    // handler re-enters indexOf() on a half-rebuilt model, an invalid
    // QModelIndex crosses into C++ -> assert/hang of the UI thread.
    property bool refreshQueued: false
    function deferredRefresh() {
        if (root.refreshQueued) {
            return;
        }
        root.refreshQueued = true;
        Qt.callLater(function() {
            root.refreshQueued = false;
            root.refreshLevel();
        });
    }
    Connections {
        // The playlists of a Rekordbox drive are added while it is read,
        // without telling the model; only a repaint (dataChanged) follows.
        // The list is then built again.
        function onDataChanged() {
            if (root.levelIndex !== null && root.sidebar.rowCount(root.levelIndex) !== levelEntries.count) {
                root.levelIndex = null;
                root.deferredRefresh();
            }
        }
        function onLayoutAboutToBeChanged() {
            root.levelIndex = null;
        }
        function onLayoutChanged() {
            root.deferredRefresh();
        }
        function onModelAboutToBeReset() {
            root.levelIndex = null;
        }
        function onModelReset() {
            root.deferredRefresh();
        }
        function onRowsAboutToBeRemoved() {
            root.levelIndex = null;
        }
        function onRowsInserted() {
            root.deferredRefresh();
        }
        function onRowsRemoved() {
            root.deferredRefresh();
        }

        target: root.sidebar
    }

    Rectangle {
        anchors.fill: parent
        color: "#000000"
    }

    // ---- Header: where we are, the load target, search, streaming ----
    Rectangle {
        id: header

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        color: "#1c1e22"
        height: root.compact ? 30 : 38

        Row {
            id: headerLeft

            anchors.left: parent.left
            anchors.leftMargin: 8
            anchors.verticalCenter: parent.verticalCenter
            spacing: 8

            // ← one level up, or from the tracks to their list
            Rectangle {
                readonly property bool usable: !root.streaming && (root.showTracks ? root.levelIndex !== null : root.levelHasAll)

                anchors.verticalCenter: parent.verticalCenter
                border.color: usable ? "#8a8d93" : "#2a2c30"
                border.width: 1
                color: backMouse.pressed ? "#2a2c30" : "transparent"
                height: header.height - 8
                width: height * 1.3

                Text {
                    anchors.centerIn: parent
                    color: parent.usable ? "#d8dadd" : "#3a3c40"
                    font.pixelSize: root.compact ? 14 : 17
                    text: "←"
                }
                TapHandler {
                    gesturePolicy: TapHandler.ReleaseWithinBounds
                    onTapped: root.back()
                }
                MouseArea {
                    id: backMouse

                    anchors.fill: parent
                    enabled: parent.usable
                    onClicked: root.back()
                }
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                color: "#d8dadd"
                elide: Text.ElideMiddle
                font.pixelSize: root.compact ? 12 : 14
                text: {
                    if (root.streaming) {
                        return qsTr("STREAMING");
                    }
                    if (root.showLevel) {
                        const names = [];
                        for (let index = root.levelIndex; index.valid; index = index.parent) {
                            names.unshift(index.parent.valid ? root.sidebar.data(index, Qt.DisplayRole) : root.sectionLabel(root.iconOf(index), root.sidebar.data(index, Qt.DisplayRole)));
                        }
                        return names.join(" › ");
                    }
                    return root.activeLabel.length > 0 ? root.activeLabel : qsTr("BROWSE");
                }
                width: Math.min(implicitWidth, root.width * 0.3)
            }
            // LOAD 1/2: the deck the tracks go to
            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                color: root.loadTarget === "[Channel2]" ? "#e8650f" : "#1f6fd1"
                height: loadText.implicitHeight + 6
                visible: root.loadTarget.length > 0
                width: loadText.implicitWidth + 14

                Text {
                    id: loadText

                    anchors.centerIn: parent
                    color: "#ffffff"
                    font.bold: true
                    font.pixelSize: root.compact ? 11 : 12
                    text: qsTr("LOAD TO DECK %1").arg(root.loadTarget === "[Channel2]" ? 2 : 1)
                }
            }
        }
        TextField {
            id: searchField

            anchors.left: headerLeft.right
            anchors.leftMargin: 12
            anchors.right: streamingTab.left
            anchors.rightMargin: 8
            anchors.verticalCenter: parent.verticalCenter
            color: "#ffffff"
            font.pixelSize: root.compact ? 12 : 14
            height: parent.height - 6
            placeholderText: qsTr("SEARCH")
            placeholderTextColor: "#6c7078"
            visible: !root.streaming

            background: Rectangle {
                border.color: searchField.activeFocus ? "#d8dadd" : "#3a3c40"
                border.width: 1
                color: "#000000"
            }

            onTextChanged: root.applySearch()
        }
        Rectangle {
            id: streamingTab

            anchors.right: parent.right
            anchors.rightMargin: 6
            anchors.verticalCenter: parent.verticalCenter
            border.color: root.streaming ? "#d4e157" : "#5c6068"
            border.width: 1
            color: "#000000"
            height: parent.height - 8
            width: streamingText.implicitWidth + 20

            Text {
                id: streamingText

                anchors.centerIn: parent
                color: root.streaming ? "#d4e157" : "#9a9ea6"
                font.bold: true
                font.pixelSize: root.compact ? 10 : 12
                text: qsTr("STREAMING")
            }
            TapHandler {
                gesturePolicy: TapHandler.ReleaseWithinBounds
                onTapped: root.streaming = !root.streaming
            }
            MouseArea {
                anchors.fill: parent
                onClicked: root.streaming = !root.streaming
            }
        }
    }

    // ---- Sources | tracks | selected track ----
    Item {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: header.bottom
        anchors.topMargin: 3
        visible: !root.streaming

        // The sources, as the column of a player's browser
        Rectangle {
            id: sectionColumn

            readonly property real buttonHeight: Math.min(root.compact ? 52 : 66, height / Math.max(1, sections.count))

            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.top: parent.top
            border.color: libraryControl.sidebarFocused && !root.showLevel ? "#d8dadd" : "transparent"
            border.width: 1
            color: "#1c1e22"
            width: root.compact ? 64 : 78

            Column {
                anchors.fill: parent

                Repeater {
                    model: DelegateModel {
                        id: sections

                        model: root.sidebar

                        delegate: Rectangle {
                            id: section

                            required property int index
                            required property var model
                            readonly property bool active: root.activeSection === index
                            readonly property bool atCursor: libraryControl.sidebarFocused && !root.showLevel && root.cursor === index

                            color: active ? "#3a3c40" : (sectionMouse.pressed ? "#2a2c30" : "transparent")
                            height: sectionColumn.buttonHeight
                            width: sectionColumn.width

                            Rectangle {
                                anchors.bottom: parent.bottom
                                anchors.left: parent.left
                                anchors.right: parent.right
                                color: "#000000"
                                height: 1
                            }
                            Rectangle {
                                anchors.fill: parent
                                border.color: "#1e88e5"
                                border.width: 2
                                color: "transparent"
                                visible: section.atCursor
                            }
                            Image {
                                id: sectionIcon

                                anchors.horizontalCenter: parent.horizontalCenter
                                anchors.top: parent.top
                                anchors.topMargin: Math.round(parent.height * 0.14)
                                source: section.model.icon ? "qrc:/images/library/ic_library_" + section.model.icon + ".svg" : ""
                                sourceSize.height: Math.round(parent.height * 0.42)
                                sourceSize.width: Math.round(parent.height * 0.42)
                                visible: false
                            }
                            // White line icons, as on a player
                            ColorOverlay {
                                anchors.fill: sectionIcon
                                color: section.active ? "#ffffff" : "#b0b3b8"
                                source: sectionIcon
                            }
                            Text {
                                anchors.bottom: parent.bottom
                                anchors.bottomMargin: Math.round(parent.height * 0.08)
                                anchors.horizontalCenter: parent.horizontalCenter
                                color: section.active ? "#ffffff" : "#b0b3b8"
                                elide: Text.ElideRight
                                font.pixelSize: root.compact ? 9 : 10
                                horizontalAlignment: Text.AlignHCenter
                                text: root.sectionLabel(section.model.icon, section.model.label)
                                width: parent.width - 4
                            }
                            TapHandler {
                                gesturePolicy: TapHandler.ReleaseWithinBounds
                                onTapped: root.openSection(section.index)
                            }
                            MouseArea {
                                id: sectionMouse

                                anchors.fill: parent
                                onClicked: root.openSection(section.index)
                            }
                        }
                    }
                }
            }
        }

        // The entries of a source, one level at a time
        Rectangle {
            id: level

            anchors.bottom: parent.bottom
            anchors.left: sectionColumn.right
            anchors.leftMargin: 3
            anchors.right: trackList.right
            anchors.top: parent.top
            border.color: libraryControl.sidebarFocused ? "#d8dadd" : "transparent"
            border.width: 1
            color: "#0c0d0f"
            visible: root.showLevel

            ListView {
                id: levelList

                anchors.fill: parent
                anchors.margins: 1
                boundsBehavior: Flickable.StopAtBounds
                clip: true

                ScrollBar.vertical: ScrollBar {
                }

                // The tracks of the entry itself, e.g. all of a drive
                header: Rectangle {
                    color: root.cursor === -1 ? "#1e5fb4" : (allMouse.pressed ? "#2a2c30" : "#16181b")
                    height: root.levelHasAll ? (root.compact ? 40 : 46) : 0
                    visible: root.levelHasAll
                    width: ListView.view.width

                    Text {
                        anchors.left: parent.left
                        anchors.leftMargin: 14
                        anchors.verticalCenter: parent.verticalCenter
                        color: "#ffffff"
                        font.pixelSize: root.compact ? 14 : 16
                        text: "♪  " + qsTr("ALL TRACKS")
                    }
                    TapHandler {
                        gesturePolicy: TapHandler.ReleaseWithinBounds
                        onTapped: root.openAllTracks()
                    }
                    MouseArea {
                        id: allMouse

                        anchors.fill: parent
                        onClicked: root.openAllTracks()
                    }
                }
                model: DelegateModel {
                    id: levelEntries

                    model: root.sidebar
                    rootIndex: root.listRoot

                    delegate: Rectangle {
                        id: entry

                        required property int index
                        required property var model
                        required property bool hasModelChildren

                        color: root.cursor === index ? "#1e5fb4" : (entryMouse.pressed ? "#2a2c30" : (index % 2 === 0 ? "#0c0d0f" : "#16181b"))
                        height: root.compact ? 40 : 46
                        width: ListView.view.width

                        Text {
                            anchors.left: parent.left
                            anchors.leftMargin: 14
                            anchors.right: arrow.left
                            anchors.verticalCenter: parent.verticalCenter
                            color: "#ffffff"
                            elide: Text.ElideRight
                            font.pixelSize: root.compact ? 15 : 17
                            text: entry.model.label ?? ""
                        }
                        Text {
                            id: arrow

                            anchors.right: parent.right
                            anchors.rightMargin: 14
                            anchors.verticalCenter: parent.verticalCenter
                            color: "#8a8d93"
                            font.pixelSize: root.compact ? 15 : 17
                            text: entry.hasModelChildren || root.levelIsDrives ? "›" : ""
                        }
                        TapHandler {
                            gesturePolicy: TapHandler.ReleaseWithinBounds
                            onTapped: {
                            root.cursor = entry.index;
                            root.openEntry(entry.index);
                            }
                        }
                        MouseArea {
                            id: entryMouse

                            anchors.fill: parent
                            onClicked: {
                                root.cursor = entry.index;
                                root.openEntry(entry.index);
                            }
                        }
                    }
                }
            }
        }

        LibraryComponent.TrackList {
            id: trackList

            anchors.bottom: parent.bottom
            anchors.left: sectionColumn.right
            anchors.leftMargin: 3
            anchors.right: info.visible ? info.left : parent.right
            anchors.rightMargin: info.visible ? 3 : 0
            anchors.top: parent.top
            clip: true
            highlighted: libraryControl.sidebarFocused === false
            model: root.sidebar ? root.sidebar.tracklist : null
            sortNewestFirst: root.activeIsDrive
            sourceName: root.activeLabel
            visible: !root.showLevel
        }

        // The selected track, like the info panel of a player
        Rectangle {
            id: info

            anchors.bottom: parent.bottom
            anchors.right: parent.right
            anchors.top: parent.top
            border.color: "#2a2c30"
            border.width: 1
            color: "#0c0d0f"
            visible: !root.compact && root.width > 900
            width: 210

            Column {
                anchors.left: parent.left
                anchors.margins: 10
                anchors.right: parent.right
                anchors.top: parent.top
                spacing: 6

                Rectangle {
                    color: "#16181b"
                    height: width
                    width: parent.width

                    Image {
                        anchors.fill: parent
                        asynchronous: true
                        fillMode: Image.PreserveAspectCrop
                        source: root.cover
                    }
                    Text {
                        anchors.centerIn: parent
                        color: "#3a3c40"
                        font.pixelSize: 40
                        text: "♪"
                        visible: root.cover.toString().length === 0
                    }
                }
                Text {
                    color: "#ffffff"
                    font.pixelSize: 15
                    maximumLineCount: 2
                    text: root.title
                    width: parent.width
                    wrapMode: Text.Wrap
                }
                Text {
                    color: "#9a9ea6"
                    elide: Text.ElideRight
                    font.pixelSize: 13
                    text: root.artist
                    width: parent.width
                }
                Repeater {
                    model: [
                        {
                            "label": "BPM",
                            "value": root.bpm
                        },
                        {
                            "label": qsTr("KEY"),
                            "value": root.key
                        },
                        {
                            "label": qsTr("ADDED"),
                            "value": root.added
                        }
                    ]

                    Item {
                        required property var modelData

                        height: 22
                        visible: modelData.value.length > 0
                        width: parent.width

                        Text {
                            anchors.left: parent.left
                            anchors.verticalCenter: parent.verticalCenter
                            color: "#9a9ea6"
                            font.pixelSize: 12
                            text: parent.modelData.label
                        }
                        Text {
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            color: "#ffffff"
                            font.pixelSize: 15
                            text: parent.modelData.value
                        }
                    }
                }
            }
        }
    }

    Loader {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: header.bottom
        // The hub builds networked views on creation; keep it out of the
        // item tree until streaming is actually opened.
        active: root.streaming
        visible: root.streaming

        sourceComponent: StreamingComponent.StreamingHub {
        }
    }

    // Reading Rekordbox and Engine DJ drives needs access to all files
    Popup {
        id: storageAccessPopup

        anchors.centerIn: parent
        modal: true
        padding: 16
        width: Math.min(460, root.width - 20)

        background: Rectangle {
            border.color: "#5c6068"
            border.width: 1
            color: "#16181b"
        }
        contentItem: Column {
            spacing: 12

            Text {
                color: "#ffffff"
                font.bold: true
                font.pixelSize: 15
                text: qsTr("Permission to read USB")
                width: parent.width
            }
            Text {
                color: "#d8dadd"
                font.pixelSize: 13
                text: qsTr("To read Rekordbox and Engine DJ playlists from a USB drive, Android has to grant Mixxx access to all files. Enable it on the screen that opens, then tap the section again.")
                width: parent.width
                wrapMode: Text.WordWrap
            }
            Row {
                anchors.right: parent.right
                spacing: 8

                Skin.Button {
                    height: 44
                    text: qsTr("NOT NOW")
                    width: 110

                    onClicked: storageAccessPopup.close()
                }
                Skin.Button {
                    activeColor: "#ffffff"
                    backgroundColor: "#1e5fb4"
                    font.bold: true
                    height: 44
                    normalColor: "#ffffff"
                    text: qsTr("GRANT PERMISSION")
                    width: 150

                    onClicked: {
                        storageAccessPopup.close();
                        Mixxx.MobileLibrary.requestFullStorageAccess();
                    }
                }
            }
        }
    }
}

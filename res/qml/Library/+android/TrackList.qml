import ".." as Skin
import "." as LibraryComponent
import Mixxx 1.0 as Mixxx
import Qt.labs.qmlmodels
import QtQml
import QtQuick
import QtQml.Models
import QtQuick.Layouts
import QtQuick.Controls 2.15
import "../Theme"

Rectangle {
    id: root

    required property var model

    color: Theme.darkGray

    // Shows that the controller's browse knob moves through this list
    property bool highlighted: false
    // The name of the playlist, crate or folder shown, if known
    property string sourceName: ""
    // Sort a list without a playlist position by "date added", newest first,
    // e.g. a whole USB drive
    property bool sortNewestFirst: false

    // A new list starts in playlist order if it has a position column,
    // otherwise newest first if requested, otherwise in its own order
    function applyDefaultSort() {
        horizontalHeader.sortingColumn = -1;
        if (!root.model || !root.model.columns) {
            return;
        }
        const columns = root.model.columns;
        let positionIndex = -1;
        let dateAddedIndex = -1;
        for (let i = 0; i < columns.length; ++i) {
            if (columns[i].columnIdx === Mixxx.TrackListColumn.SQLColumns.Position) {
                positionIndex = i;
            } else if (columns[i].columnIdx === Mixxx.TrackListColumn.SQLColumns.DateAdded) {
                dateAddedIndex = i;
            }
        }
        let index = -1;
        let order = Qt.AscendingOrder;
        if (positionIndex >= 0 && !root.sortNewestFirst) {
            index = positionIndex;
        } else if (dateAddedIndex >= 0 && root.sortNewestFirst) {
            index = dateAddedIndex;
            order = Qt.DescendingOrder;
        }
        if (index < 0) {
            return;
        }
        horizontalHeader.sortingColumn = index;
        horizontalHeader.sortingOrder = order;
        root.model.sort(index, order);
    }
    onModelChanged: Qt.callLater(root.applyDefaultSort)
    readonly property bool isRekordboxPlaylist: Mixxx.MobileLibrary.isRekordboxPlaylist(root.model)

    function focusList() {
        view.forceActiveFocus();
    }
    function loadSelectedTrack(group, play) {
        view.loadSelectedTrack(group, play);
    }
    function loadSelectedTrackIntoNextAvailableDeck(play) {
        view.loadSelectedTrackIntoNextAvailableDeck(play);
    }
    // Knob turns that arrive together are applied at once, so fast turns
    // don't queue up a relayout per step
    property int pendingMove: 0

    function applyPendingMove() {
        const offset = root.pendingMove;
        root.pendingMove = 0;
        view.selectionModel.moveSelectionVertical(offset);
    }
    function moveSelectionVertical(offset) {
        root.pendingMove += offset;
        Qt.callLater(root.applyPendingMove);
    }

    border.color: Theme.accentColor
    border.width: highlighted ? 2 : 0
    HorizontalHeaderView {
        id: horizontalHeader

        property int sortingColumn: -1
        property var sortingOrder: Qt.Descending

        anchors.left: parent.left
        anchors.margins: 5
        anchors.right: parent.right
        anchors.top: parent.top
        syncView: view

        delegate: Item {
            id: column

            required property string display
            required property int index

            implicitHeight: columnName.contentHeight + 5
            implicitWidth: columnName.contentWidth + 5

            MouseArea {
                id: columnMouseHandler

                acceptedButtons: Qt.LeftButton
                anchors.fill: parent

                onClicked: {
                    if (horizontalHeader.sortingColumn == index) {
                        horizontalHeader.sortingOrder = horizontalHeader.sortingOrder == Qt.DescendingOrder ? Qt.AscendingOrder : Qt.DescendingOrder;
                    } else {
                        horizontalHeader.sortingColumn = index;
                        horizontalHeader.sortingOrder = Qt.AscendingOrder;
                    }
                    view.model.sort(horizontalHeader.sortingColumn, horizontalHeader.sortingOrder);
                }
            }
            Text {
                id: columnName

                anchors.fill: parent
                anchors.leftMargin: 15
                color: Theme.textColor
                elide: Text.ElideRight
                font.capitalization: Font.Capitalize
                font.family: Theme.fontFamily
                font.pixelSize: 12
                font.weight: Font.Medium
                horizontalAlignment: Text.AlignLeft
                text: display
                verticalAlignment: Text.AlignVCenter
            }
            Item {
                anchors {
                    bottom: parent.bottom
                    left: parent.left
                    leftMargin: 5
                    top: parent.top
                }
                // ▲ or ▼, drawn as Android draws ▶ as an emoji
                Skin.Triangle {
                    id: sortIndicator

                    anchors.centerIn: parent
                    color: "red"
                    height: Math.round(Theme.buttonFontPixelSize * 0.7)
                    rotation: horizontalHeader.sortingOrder == Qt.AscendingOrder ? 90 : -90
                    visible: horizontalHeader.sortingColumn == index
                    width: Math.round(height * 0.85)
                }
            }
            Rectangle {
                id: columnResizer

                color: Theme.darkGray2
                width: 1

                anchors {
                    bottom: parent.bottom
                    right: parent.right
                    top: parent.top
                }
                MouseArea {
                    id: columnResizeHandler

                    property int sizeOffset: 0

                    anchors.fill: parent
                    cursorShape: Qt.SizeHorCursor
                    preventStealing: true

                    onMouseXChanged: {
                        if (drag.active) {
                            column.width += mouseX;
                            sizeOffset += mouseX;
                        }
                    }

                    drag {
                        axis: Drag.XAxis
                        target: parent
                        threshold: 2

                        onActiveChanged: {
                            if (!drag.active && columnResizeHandler.sizeOffset !== 0) {
                                view.model.columns[index].preferredWidth = column.width;
                                columnResizeHandler.sizeOffset = 0;
                                view.updateColumnSize();
                                view.forceLayout();
                            }
                        }
                    }
                }
            }
        }
    }
    TableView {
        id: view

        property int dynamicColumnCount: 0
        property int usedWidth: 0

        function loadSelectedTrack(group, play) {
            const urls = this.selectionModel.selectedTrackUrls();
            if (urls.length == 0)
                return;

            Mixxx.PlayerManager.getPlayer(group).loadTrackFromLocationUrl(urls[0], play);
        }
        function loadSelectedTrackIntoNextAvailableDeck(play) {
            const urls = this.selectionModel.selectedTrackUrls();
            if (urls.length == 0)
                return;

            Mixxx.PlayerManager.loadLocationUrlIntoNextAvailableDeck(urls[0], play);
        }
        function updateColumnSize() {
            const oldUsedWidth = usedWidth;
            const oldDynamicColumnCount = dynamicColumnCount;
            usedWidth = 0;
            dynamicColumnCount = 0;
            if (model == null) {
                return;
            }
            for (let c = 0; c < model.columns.length; c++) {
                if (model.columns[c].hidden || model.columns[c].autoHideWidth > view.width) {
                    continue;
                } else if (model.columns[c].preferredWidth > 0) {
                    usedWidth += model.columns[c].preferredWidth;
                } else {
                    dynamicColumnCount += model.columns[c].fillSpan || 1;
                }
            }
            return oldDynamicColumnCount != dynamicColumnCount || oldUsedWidth != usedWidth;
        }

        anchors.bottom: quickLoadBar.top
        anchors.left: parent.left
        anchors.margins: 5
        anchors.right: parent.right
        anchors.top: horizontalHeader.bottom
        clip: true
        columnWidthProvider: function (column) {
            const columnDef = view.model.columns[column];
            if (columnDef.hidden) {
                return 0;
            }
            if (columnDef.autoHideWidth > 0 && columnDef.autoHideWidth > view.width) {
                return 0;
            }
            if (columnDef.preferredWidth >= 0) {
                return columnDef.preferredWidth;
            }
            const span = columnDef.fillSpan || 1;
            return span * (view.width - view.usedWidth) / view.dynamicColumnCount;
        }
        flickableDirection: Flickable.VerticalFlick
        boundsBehavior: Flickable.StopAtBounds
        interactive: true
        keyNavigationEnabled: false
        model: root.model
        pointerNavigationEnabled: false
        reuseItems: true

        ScrollBar.vertical: ScrollBar {
            id: verticalScrollBar
            policy: ScrollBar.AlwaysOn
            width: 16
            interactive: true
            contentItem: Rectangle {
                implicitWidth: 12
                implicitHeight: 48
                radius: 6
                color: verticalScrollBar.pressed ? Theme.libraryScrollHandlePressed : (verticalScrollBar.hovered ? Theme.libraryScrollHandleHovered : Theme.libraryScrollHandle)
            }
            background: Rectangle {
                implicitWidth: 16
                color: Theme.libraryScrollTrack
                opacity: 0.7
                radius: 4
            }
        }
        delegate: Item {
            id: item

            required property url cover_art
            required property color decoration
            required property var display
            required property string file_url
            required property int row
            required property bool selected

            implicitHeight: 38

            Loader {
                id: loader

                property var capabilities: root.model ? root.model.getCapabilities() : Mixxx.LibraryTrackListModel.Capability.None
                property url cover_art: item.cover_art
                property color decoration: item.decoration
                property var display: item.display
                property url file_url: item.file_url
                property int row: item.row
                property bool selected: item.selected
                property var tableView: view

                anchors.fill: parent
                focus: true
                sourceComponent: delegate

                onLoaded:
                // Workaround needed for WaveformOverview column to load the data
                //     if (track)
                //         Mixxx.Library.analyze(track)
                {}
            }
            // Workaround needed for WaveformOverview column to load the data
            // TableView.onReused: {
            //     if (track)
            //         Mixxx.Library.analyze(track)
            // }
        }
        Timer {
            id: relayoutTimer

            interval: 150
            onTriggered: view.forceLayout()
        }

        selectionModel: ItemSelectionModel {
            function moveSelectionVertical(value) {
                if (value == 0)
                    return;

                const selected = this.selectedIndexes;
                const oldRow = (selected.length == 0) ? 0 : selected[0].row;
                this.selectRow(oldRow + value);
            }
            function selectRow(row) {
                const rowCount = this.model.rowCount();
                if (rowCount == 0) {
                    this.clear();
                    return;
                }
                // Stay at the first or last track instead of wrapping around
                const newRow = Math.max(0, Math.min(rowCount - 1, row));
                this.select(this.model.index(newRow, 0), ItemSelectionModel.Rows | ItemSelectionModel.Select | ItemSelectionModel.Clear | ItemSelectionModel.Current);
                // Keep the selection visible, e.g. when moving it with the controller
                view.positionViewAtRow(newRow, TableView.Contain);
            }
            function selectedTrackUrls() {
                return this.selectedIndexes.map(index => {
                    return this.model.getUrl(index.row);
                });
            }

            model: view.model
        }

        Component.onCompleted: this.updateColumnSize()
        Keys.onDownPressed: root.moveSelectionVertical(1)
        Keys.onEnterPressed: this.loadSelectedTrackIntoNextAvailableDeck(false)
        Keys.onReturnPressed: this.loadSelectedTrackIntoNextAvailableDeck(false)
        Keys.onUpPressed: root.moveSelectionVertical(-1)
        onModelChanged: this.updateColumnSize()
        // The library grows when switching to the LIB view, rows have to be
        // created for the new space once the layout has settled
        onHeightChanged: relayoutTimer.restart()
        onWidthChanged: {
            if (view.updateColumnSize()) {
                // forceLayout is costly - only invoke if there was a change in the column layouts
                view.forceLayout();
            }
        }
    }
    Rectangle {
        id: quickLoadBar

        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        color: Theme.sunkenBackgroundColor
        height: 48
        border.color: Theme.panelSplitterBackground
        border.width: 1

        readonly property var selectedUrls: view.selectionModel.selectedTrackUrls()
        readonly property bool hasTrack: selectedUrls && selectedUrls.length > 0
        // Without a selection, "+ ALL" adds the whole list to a playlist
        readonly property bool hasTracks: view.rows > 0

        RowLayout {
            anchors.fill: parent
            anchors.margins: 5
            spacing: 8

            Rectangle {
                Layout.fillHeight: true
                Layout.fillWidth: true
                color: Theme.darkGray3
                radius: 4

                RowLayout {
                    anchors.fill: parent
                    anchors.margins: 6

                    Text {
                        Layout.fillWidth: true
                        color: quickLoadBar.hasTrack ? Theme.white : Theme.lightGray3
                        elide: Text.ElideRight
                        font.bold: quickLoadBar.hasTrack
                        font.pixelSize: 13
                        text: {
                            if (!quickLoadBar.hasTrack)
                                return qsTr("Select a track to load");
                            const urlStr = quickLoadBar.selectedUrls[0].toString();
                            const filename = decodeURIComponent(urlStr.substring(urlStr.lastIndexOf('/') + 1));
                            return filename;
                        }
                    }
                }
            }

            Skin.Button {
                id: addToPlaylistBtn

                Layout.preferredHeight: 38
                Layout.preferredWidth: 110
                activeColor: Theme.accentColor
                backgroundColor: quickLoadBar.hasTracks ? Theme.libraryActionBackground : Theme.libraryActionDisabledBackground
                normalColor: quickLoadBar.hasTracks ? Theme.libraryActionText : Theme.libraryActionDisabledText
                enabled: quickLoadBar.hasTracks
                font.bold: true
                text: quickLoadBar.hasTrack ? qsTr("+ PLAYLIST") : qsTr("+ ALL")

                onClicked: playlistPopup.openFor(quickLoadBar.hasTrack ? quickLoadBar.selectedUrls[0] : "")
            }

            // Copies a Rekordbox playlist of a USB drive to the device
            Skin.Button {
                Layout.preferredHeight: 38
                Layout.preferredWidth: 100
                visible: root.isRekordboxPlaylist
                activeColor: Theme.accentColor
                backgroundColor: Theme.libraryActionBackground
                normalColor: Theme.libraryActionText
                font.bold: true
                text: Mixxx.MobileLibrary.playlistCopyRunning ? "%1/%2".arg(Mixxx.MobileLibrary.playlistCopyDone).arg(Mixxx.MobileLibrary.playlistCopyTotal) : qsTr("⬇ COPY")

                onClicked: copyPopup.openForList()
            }

            Skin.Button {
                id: loadDeck1Btn

                Layout.preferredHeight: 38
                Layout.preferredWidth: 115
                activeColor: Theme.libraryDeck1Pressed
                backgroundColor: quickLoadBar.hasTrack ? Theme.libraryDeck1Background : Theme.libraryActionDisabledBackground
                normalColor: quickLoadBar.hasTrack ? Theme.libraryActionText : Theme.libraryActionDisabledText
                highlight: quickLoadBar.hasTrack
                enabled: quickLoadBar.hasTrack
                font.bold: true
                arrowLeft: true
                text: "DECK 1"

                onClicked: {
                    view.loadSelectedTrack("[Channel1]", false);
                }
            }

            Skin.Button {
                id: loadDeck2Btn

                Layout.preferredHeight: 38
                Layout.preferredWidth: 115
                activeColor: Theme.libraryDeck2Pressed
                backgroundColor: quickLoadBar.hasTrack ? Theme.libraryDeck2Background : Theme.libraryActionDisabledBackground
                normalColor: quickLoadBar.hasTrack ? Theme.libraryActionText : Theme.libraryActionDisabledText
                highlight: quickLoadBar.hasTrack
                enabled: quickLoadBar.hasTrack
                font.bold: true
                arrowRight: true
                text: "DECK 2"

                onClicked: {
                    view.loadSelectedTrack("[Channel2]", false);
                }
            }
        }
    }

    Popup {
        id: playlistPopup

        property url trackUrl
        // All tracks of the list instead of the selected one
        property bool allTracks: false
        property var playlists: []
        property string message: ""
        property bool messageIsError: true

        // Without a track URL, all tracks of the list are added
        function openFor(url) {
            trackUrl = url;
            allTracks = url.toString().length === 0;
            message = "";
            newPlaylistName.text = "";
            playlists = Mixxx.MobileLibrary.playlists();
            open();
        }
        function addTo(playlistId, name) {
            if (!allTracks) {
                if (Mixxx.MobileLibrary.addTrackToPlaylist(playlistId, trackUrl)) {
                    close();
                } else {
                    messageIsError = true;
                    message = qsTr("Could not add to %1").arg(name);
                }
                return;
            }
            const count = Mixxx.MobileLibrary.addAllTracksToPlaylist(playlistId, view.model);
            messageIsError = count < 0;
            message = count < 0 ? qsTr("Could not add to %1").arg(name) : qsTr("Added %1 tracks to %2").arg(count).arg(name);
            playlists = Mixxx.MobileLibrary.playlists();
        }

        anchors.centerIn: parent
        width: Math.min(380, root.width - 20)
        height: Math.min(440, root.height - 20)
        modal: true
        focus: true
        padding: 10

        background: Rectangle {
            color: Theme.darkGray2
            border.color: Theme.darkGray3
            radius: 8
        }

        contentItem: ColumnLayout {
            spacing: 8

            Text {
                Layout.fillWidth: true
                color: Theme.white
                font.bold: true
                font.pixelSize: 15
                text: playlistPopup.allTracks ? qsTr("Add all tracks of the list to a playlist") : qsTr("Add to playlist")
                wrapMode: Text.WordWrap
            }
            // With a selected track, choose between it and the whole list
            RowLayout {
                Layout.fillWidth: true
                spacing: 6
                visible: playlistPopup.trackUrl.toString().length > 0

                Skin.Button {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 36
                    activeColor: Theme.libraryActionText
                    highlight: !playlistPopup.allTracks
                    text: qsTr("SELECTED TRACK")
                    onClicked: playlistPopup.allTracks = false
                }
                Skin.Button {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 36
                    activeColor: Theme.libraryActionText
                    highlight: playlistPopup.allTracks
                    text: qsTr("ALL TRACKS")
                    onClicked: playlistPopup.allTracks = true
                }
            }
            ListView {
                Layout.fillHeight: true
                Layout.fillWidth: true
                clip: true
                model: playlistPopup.playlists
                spacing: 4

                delegate: Skin.Button {
                    required property var modelData

                    width: ListView.view.width
                    height: 44
                    enabled: !modelData.locked
                    backgroundColor: Theme.libraryActionDisabledBackground
                    normalColor: Theme.white
                    activeColor: Theme.accentColor
                    font.pixelSize: 14
                    text: modelData.locked ? qsTr("%1 (locked)").arg(modelData.name) : modelData.name
                    onClicked: playlistPopup.addTo(modelData.id, modelData.name)
                }

                Text {
                    anchors.centerIn: parent
                    visible: playlistPopup.playlists.length === 0
                    color: Theme.lightGray2
                    font.pixelSize: 13
                    text: qsTr("You have no playlists yet")
                }
            }
            Text {
                Layout.fillWidth: true
                visible: playlistPopup.message.length > 0
                color: playlistPopup.messageIsError ? Theme.red : Theme.librarySuccessColor
                font.pixelSize: 12
                text: playlistPopup.message
                wrapMode: Text.WordWrap
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Skin.TextField {
                    id: newPlaylistName

                    Layout.fillWidth: true
                    Layout.preferredHeight: 44
                    font.pixelSize: 14
                    placeholderText: qsTr("New playlist")
                    onAccepted: createPlaylistBtn.clicked()
                }
                Skin.Button {
                    id: createPlaylistBtn

                    Layout.preferredHeight: 44
                    Layout.preferredWidth: 90
                    enabled: newPlaylistName.text.trim().length > 0
                    activeColor: Theme.accentColor
                    backgroundColor: Theme.libraryConfirmBackground
                    normalColor: Theme.libraryActionText
                    font.bold: true
                    text: qsTr("CREATE")

                    onClicked: {
                        const name = newPlaylistName.text.trim();
                        if (name.length === 0) {
                            return;
                        }
                        const playlistId = Mixxx.MobileLibrary.createNamedPlaylist(name);
                        if (playlistId < 0) {
                            playlistPopup.messageIsError = true;
                            playlistPopup.message = qsTr("A playlist named %1 already exists. Choose it in the list above to add to it.").arg(name);
                            return;
                        }
                        // The keyboard's Enter and the button both create:
                        // only the first one does
                        newPlaylistName.text = "";
                        playlistPopup.addTo(playlistId, name);
                    }
                }
            }
        }
    }
    // Copies the files of a Rekordbox playlist to the device and makes a
    // Mixxx playlist of them, see library/rekordbox/rekordboxplaylistcopy.h
    Popup {
        id: copyPopup

        property string message: ""
        property bool messageIsError: false

        function openForList() {
            if (!Mixxx.MobileLibrary.playlistCopyRunning) {
                message = "";
                copyName.text = root.sourceName;
            }
            open();
        }

        anchors.centerIn: parent
        width: Math.min(420, root.width - 20)
        modal: true
        focus: true
        padding: 12

        background: Rectangle {
            color: Theme.darkGray2
            border.color: Theme.darkGray3
            radius: 8
        }

        Connections {
            target: Mixxx.MobileLibrary

            function onPlaylistCopyFinished(message, error) {
                copyPopup.message = message;
                copyPopup.messageIsError = error;
            }
        }

        contentItem: ColumnLayout {
            spacing: 8

            Text {
                Layout.fillWidth: true
                color: Theme.white
                font.bold: true
                font.pixelSize: 15
                text: qsTr("Copy the playlist to this device")
                wrapMode: Text.WordWrap
            }
            Text {
                Layout.fillWidth: true
                color: Theme.lightGray2
                font.pixelSize: 12
                text: qsTr("Copies the files to the folder below and creates a Mixxx playlist with this name, in the order of the Rekordbox playlist. Beatgrids, cues, keys and colors from Rekordbox are kept. Copying the playlist again only copies new files and updates the Mixxx playlist. Other playlists and folders with the same name are never replaced.")
                wrapMode: Text.WordWrap
            }
            Skin.TextField {
                id: copyName

                Layout.fillWidth: true
                Layout.preferredHeight: 44
                enabled: !Mixxx.MobileLibrary.playlistCopyRunning
                font.pixelSize: 14
                placeholderText: qsTr("Playlist name")
            }
            Text {
                Layout.fillWidth: true
                color: Theme.lightGray2
                elide: Text.ElideMiddle
                font.pixelSize: 12
                text: Mixxx.MobileLibrary.rekordboxPlaylistCopyDirectory(copyName.text)
            }
            ProgressBar {
                Layout.fillWidth: true
                visible: Mixxx.MobileLibrary.playlistCopyRunning
                from: 0
                to: Math.max(1, Mixxx.MobileLibrary.playlistCopyTotal)
                value: Mixxx.MobileLibrary.playlistCopyDone
            }
            Text {
                Layout.fillWidth: true
                visible: Mixxx.MobileLibrary.playlistCopyRunning
                color: Theme.white
                font.pixelSize: 12
                text: qsTr("Copying %1 of %2 files…").arg(Math.min(Mixxx.MobileLibrary.playlistCopyDone + 1, Mixxx.MobileLibrary.playlistCopyTotal)).arg(Mixxx.MobileLibrary.playlistCopyTotal)
            }
            Text {
                Layout.fillWidth: true
                visible: copyPopup.message.length > 0
                color: copyPopup.messageIsError ? Theme.red : Theme.librarySuccessColor
                font.pixelSize: 12
                text: copyPopup.message
                wrapMode: Text.WordWrap
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Skin.Button {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 44
                    activeColor: Theme.accentColor
                    font.bold: true
                    text: Mixxx.MobileLibrary.playlistCopyRunning ? qsTr("STOP") : qsTr("CLOSE")
                    onClicked: {
                        if (Mixxx.MobileLibrary.playlistCopyRunning) {
                            Mixxx.MobileLibrary.cancelPlaylistCopy();
                        } else {
                            copyPopup.close();
                        }
                    }
                }
                Skin.Button {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 44
                    visible: !Mixxx.MobileLibrary.playlistCopyRunning
                    enabled: copyName.text.trim().length > 0 && root.isRekordboxPlaylist
                    activeColor: Theme.accentColor
                    backgroundColor: Theme.libraryConfirmBackground
                    normalColor: Theme.libraryActionText
                    font.bold: true
                    text: qsTr("COPY")
                    onClicked: {
                        const error = Mixxx.MobileLibrary.copyRekordboxPlaylist(root.model, copyName.text.trim());
                        copyPopup.messageIsError = error.length > 0;
                        copyPopup.message = error;
                    }
                }
            }
        }
    }
}
